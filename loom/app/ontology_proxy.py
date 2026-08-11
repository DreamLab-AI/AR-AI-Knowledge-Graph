#!/usr/bin/env python3
"""ontology_proxy — stdlib-only OpenAI-compatible proxy in front of llama-server.

Sits between clients and a llama.cpp ``llama-server`` (or any OpenAI-compatible
upstream) and grounds chat completions in the DreamLab ontology via the
``ontology_scaffold`` module (same directory).

Environment:
  ONTOLOGY_UPSTREAM    upstream base URL        (default http://127.0.0.1:8085)
  ONTOLOGY_PROXY_PORT  listen port              (default 8086; 0 = ephemeral)
  ONTOLOGY_PROXY_HOST  bind address             (default 0.0.0.0)
  ONTOLOGY_INDEX       scaffold-index.json path (see ontology_scaffold SCHEMA)
  ONTOLOGY_MODE        scaffold | tools | off   (default scaffold)
  ONTOLOGY_BUDGET      scaffold token budget    (default 1500)
  ONTOLOGY_TIMEOUT     upstream timeout seconds (default 600)

Behaviour on POST /v1/chat/completions:
  scaffold  messages := scaffold_messages(messages, budget); stream forced off;
            forward; return upstream JSON verbatim plus a top-level
            {"ontology": {"mode": "scaffold", "injected_tokens": N}}.
  tools     advertise ontology_search / ontology_class_get / ontology_neighbours
            (merged with any caller tools); execute tool_calls locally against
            the index; loop, max 4 tool rounds; return the final answer plus
            {"ontology": {"mode": "tools", "tool_calls": N}}. A tool_call for a
            caller-owned tool is handed back verbatim for the caller to run.
  off       pure passthrough.

GET /health -> {"ok": true, "mode": ..., "upstream": ..., "index_classes": N}.
Any other /v1/* GET/POST is transparently passed through (models list etc).
Upstream failures surface as 502 with detail; bad JSON bodies as 400; the
server loop never crashes on a request. Fail-open: a missing/broken index
degrades to passthrough rather than erroring.

STDLIB ONLY. Python 3.10+ (targets 3.14 on HP-Desktop). ThreadingHTTPServer —
llama-server serialises requests anyway.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Optional

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ontology_scaffold  # noqa: E402

MAX_TOOL_ROUNDS = 4
CHAT_PATH = "/v1/chat/completions"
SEARCH_LIMIT = 8

# Request headers worth forwarding upstream; hop-by-hop headers never relayed.
_FWD_REQ_HEADERS = ("authorization", "api-key", "x-api-key", "accept")
_HOP_HEADERS = frozenset(
    ("connection", "keep-alive", "transfer-encoding", "te", "trailer",
     "upgrade", "proxy-authenticate", "proxy-authorization", "content-length",
     "content-encoding", "date", "server")
)

ONTOLOGY_TOOL_DEFS: list[dict[str, Any]] = [
    {
        "type": "function",
        "function": {
            "name": "ontology_search",
            "description": (
                "Search the DreamLab ontology for classes matching a free-text "
                "query. Returns ranked matches with slug, title and definition."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Free-text search query."}
                },
                "required": ["query"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "ontology_class_get",
            "description": "Fetch the full ontology record for a class by slug.",
            "parameters": {
                "type": "object",
                "properties": {
                    "slug": {"type": "string",
                             "description": "Class slug, e.g. 'knowledge-graph'."}
                },
                "required": ["slug"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "ontology_neighbours",
            "description": (
                "List a class's graph neighbours: parents (sup), ancestors "
                "(isup) and typed relation targets (rel), each with titles."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "slug": {"type": "string", "description": "Class slug."}
                },
                "required": ["slug"],
            },
        },
    },
]
ONTOLOGY_TOOL_NAMES = frozenset(t["function"]["name"] for t in ONTOLOGY_TOOL_DEFS)


class Config:
    """Env-derived configuration; attributes may be overridden before build."""

    def __init__(self) -> None:
        self.upstream = os.environ.get(
            "ONTOLOGY_UPSTREAM", "http://127.0.0.1:8085"
        ).rstrip("/")
        self.host = os.environ.get("ONTOLOGY_PROXY_HOST", "0.0.0.0")
        try:
            self.port = int(os.environ.get("ONTOLOGY_PROXY_PORT", "8086"))
        except ValueError:
            self.port = 8086
        self.index_path = os.environ.get("ONTOLOGY_INDEX", "")
        mode = os.environ.get("ONTOLOGY_MODE", "scaffold").strip().lower()
        self.mode = mode if mode in ("scaffold", "tools", "off") else "scaffold"
        try:
            self.budget = int(os.environ.get("ONTOLOGY_BUDGET", "1500"))
        except ValueError:
            self.budget = 1500
        try:
            self.timeout = float(os.environ.get("ONTOLOGY_TIMEOUT", "600"))
        except ValueError:
            self.timeout = 600.0


def _load_index(cfg: Config) -> Optional["ontology_scaffold.ScaffoldIndex"]:
    """Load the scaffold index; None (fail-open) when absent or broken."""
    if not cfg.index_path:
        return None
    try:
        return ontology_scaffold.ScaffoldIndex.load(cfg.index_path)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(
            "ontology_proxy: WARNING: cannot load index %s: %s "
            "(continuing without ontology grounding)" % (cfg.index_path, exc),
            file=sys.stderr,
        )
        return None


def _content_chars(messages: Any) -> int:
    """Total characters of textual content across a messages list."""
    total = 0
    if not isinstance(messages, list):
        return 0
    for m in messages:
        if not isinstance(m, dict):
            continue
        c = m.get("content")
        if isinstance(c, str):
            total += len(c)
        elif isinstance(c, list):
            for part in c:
                if isinstance(part, dict) and part.get("type") == "text":
                    total += len(str(part.get("text", "")))
    return total


# ---------------------------------------------------------------------------
# Local tool execution (tools mode)
# ---------------------------------------------------------------------------

def execute_tool(index, name: str, args: dict) -> dict:
    """Run one ontology tool locally. Always returns a JSON-able dict."""
    if index is None:
        return {"error": "ontology index unavailable"}
    if name == "ontology_search":
        query = str(args.get("query", ""))
        seeds = index.match(query, max_seeds=SEARCH_LIMIT)
        return {
            "query": query,
            "matches": [
                {
                    "slug": slug,
                    "title": index.title_of(slug),
                    "definition": index.classes[slug].get("d", ""),
                    "score": round(score, 3),
                }
                for slug, score in seeds
            ],
        }
    if name == "ontology_class_get":
        slug = str(args.get("slug", ""))
        entry = index.classes.get(slug)
        if entry is None:
            return {"error": "unknown class slug: %s" % slug}
        return {"slug": slug, "title": index.title_of(slug), **entry}
    if name == "ontology_neighbours":
        slug = str(args.get("slug", ""))
        entry = index.classes.get(slug)
        if entry is None:
            return {"error": "unknown class slug: %s" % slug}

        def titled(refs) -> list:
            out = []
            for r in refs or []:
                s = ontology_scaffold._ref_to_slug(str(r))
                out.append({"slug": s, "title": index.title_of(s)})
            return out

        rel_out = []
        for rt, targets in (entry.get("rel") or {}).items():
            for t in titled(targets):
                rel_out.append({"type": rt, **t})
        return {
            "slug": slug,
            "title": index.title_of(slug),
            "sup": titled(entry.get("sup")),
            "isup": titled(entry.get("isup")),
            "rel": rel_out,
        }
    return {"error": "unknown tool: %s" % name}


# ---------------------------------------------------------------------------
# HTTP plumbing
# ---------------------------------------------------------------------------

class UpstreamError(Exception):
    def __init__(self, detail: str, status: Optional[int] = None, body: str = ""):
        super().__init__(detail)
        self.detail = detail
        self.status = status
        self.body = body[:2000]


class ProxyHandler(BaseHTTPRequestHandler):
    # Bound by build_server(): cfg (Config), index (ScaffoldIndex | None)
    cfg: Config = None  # type: ignore[assignment]
    index = None
    server_version = "OntologyProxy/1.0"
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))

    # -- low-level helpers -------------------------------------------------

    def _read_body(self) -> bytes:
        try:
            length = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            length = 0
        return self.rfile.read(length) if length > 0 else b""

    def _send_json(self, status: int, obj) -> None:
        payload = json.dumps(obj).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def _send_raw(self, status: int, body: bytes, headers) -> None:
        self.send_response(status)
        sent_ct = False
        for k, v in headers:
            kl = k.lower()
            if kl in _HOP_HEADERS:
                continue
            if kl == "content-type":
                sent_ct = True
            self.send_header(k, v)
        if not sent_ct:
            self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _upstream_request(self, path: str, body: Optional[bytes], method: str):
        """Forward to upstream. Returns (status, headers_items, body_bytes).

        HTTP error statuses are returned (not raised); transport failures
        raise UpstreamError.
        """
        url = self.cfg.upstream + path
        fwd_headers = {}
        for name in _FWD_REQ_HEADERS:
            val = self.headers.get(name)
            if val:
                fwd_headers[name] = val
        if body is not None:
            fwd_headers["Content-Type"] = self.headers.get(
                "Content-Type", "application/json"
            )
        req = urllib.request.Request(url, data=body, headers=fwd_headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=self.cfg.timeout) as resp:
                return resp.status, list(resp.headers.items()), resp.read()
        except urllib.error.HTTPError as exc:
            try:
                err_body = exc.read()
            except Exception:
                err_body = b""
            return exc.code, list(exc.headers.items()) if exc.headers else [], err_body
        except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as exc:
            raise UpstreamError("upstream unreachable: %s" % exc) from exc

    def _upstream_chat(self, body_obj: dict) -> dict:
        """POST a chat body upstream (stream forced off), parse JSON reply."""
        body_obj.pop("stream", None)
        body_obj.pop("stream_options", None)
        payload = json.dumps(body_obj).encode("utf-8")
        status, _headers, raw = self._upstream_request(CHAT_PATH, payload, "POST")
        if status >= 400:
            raise UpstreamError(
                "upstream returned HTTP %d" % status,
                status=status, body=raw.decode("utf-8", "replace"),
            )
        try:
            resp = json.loads(raw.decode("utf-8"))
        except (ValueError, UnicodeDecodeError) as exc:
            raise UpstreamError(
                "upstream returned non-JSON body: %s" % exc,
                status=status, body=raw.decode("utf-8", "replace"),
            )
        if not isinstance(resp, dict):
            raise UpstreamError("upstream returned non-object JSON", status=status)
        return resp

    def _bad_gateway(self, exc: UpstreamError) -> None:
        err: dict[str, Any] = {"message": exc.detail, "type": "upstream_error"}
        if exc.status is not None:
            err["upstream_status"] = exc.status
        if exc.body:
            err["upstream_body"] = exc.body
        self._send_json(502, {"error": err})

    def _safe_500(self, exc: Exception) -> None:
        try:
            self._send_json(500, {"error": {
                "message": "proxy internal error: %s" % exc, "type": "proxy_error"}})
        except Exception:
            pass  # client gone — nothing left to do

    # -- routing -----------------------------------------------------------

    def do_GET(self):
        try:
            if self.path == "/health":
                self._send_json(200, {
                    "ok": True,
                    "mode": self.cfg.mode,
                    "upstream": self.cfg.upstream,
                    "index_classes": len(self.index.classes) if self.index else 0,
                })
            elif self.path.startswith("/v1/"):
                self._passthrough("GET")
            else:
                self._send_json(404, {"error": {"message": "not found",
                                                "type": "not_found"}})
        except UpstreamError as exc:
            self._bad_gateway(exc)
        except Exception as exc:  # never crash the server loop
            self._safe_500(exc)

    def do_POST(self):
        try:
            if self.path == CHAT_PATH and self.cfg.mode != "off":
                self._chat_completions()
            elif self.path.startswith("/v1/"):
                self._passthrough("POST")
            else:
                self._send_json(404, {"error": {"message": "not found",
                                                "type": "not_found"}})
        except UpstreamError as exc:
            self._bad_gateway(exc)
        except Exception as exc:
            self._safe_500(exc)

    def _passthrough(self, method: str) -> None:
        body = self._read_body() if method == "POST" else None
        status, headers, raw = self._upstream_request(self.path, body, method)
        self._send_raw(status, raw, headers)

    # -- chat completions --------------------------------------------------

    def _chat_completions(self) -> None:
        raw = self._read_body()
        try:
            body = json.loads(raw.decode("utf-8"))
            if not isinstance(body, dict):
                raise ValueError("body must be a JSON object")
        except (ValueError, UnicodeDecodeError) as exc:
            self._send_json(400, {"error": {
                "message": "invalid JSON body: %s" % exc,
                "type": "invalid_request_error"}})
            return
        if self.cfg.mode == "scaffold":
            self._chat_scaffold(body)
        else:
            self._chat_tools(body)

    def _chat_scaffold(self, body: dict) -> None:
        messages = body.get("messages")
        injected_tokens = 0
        if isinstance(messages, list) and self.index is not None:
            before = _content_chars(messages)
            try:
                new_messages = ontology_scaffold.scaffold_messages(
                    messages, budget_tokens=self.cfg.budget, index=self.index
                )
            except Exception as exc:  # fail-open: never block the request
                print("ontology_proxy: WARNING: scaffold failed: %s" % exc,
                      file=sys.stderr)
                new_messages = messages
            injected_tokens = max(0, (_content_chars(new_messages) - before + 3) // 4)
            body["messages"] = new_messages
        resp = self._upstream_chat(body)
        resp["ontology"] = {"mode": "scaffold", "injected_tokens": injected_tokens}
        self._send_json(200, resp)

    def _chat_tools(self, body: dict) -> None:
        messages = list(body.get("messages") or [])

        caller_tools = list(body.get("tools") or [])
        caller_names = {
            (t.get("function") or {}).get("name")
            for t in caller_tools if isinstance(t, dict)
        }
        body["tools"] = caller_tools + [
            t for t in ONTOLOGY_TOOL_DEFS
            if t["function"]["name"] not in caller_names
        ]

        total_calls = 0
        resp: dict = {}
        for round_no in range(MAX_TOOL_ROUNDS + 1):
            body["messages"] = messages
            resp = self._upstream_chat(body)
            try:
                msg = resp["choices"][0]["message"]
            except (KeyError, IndexError, TypeError):
                break
            tool_calls = msg.get("tool_calls") or []
            if not tool_calls or round_no == MAX_TOOL_ROUNDS:
                break
            names = [
                ((tc.get("function") or {}).get("name") or "")
                for tc in tool_calls if isinstance(tc, dict)
            ]
            if not all(n in ONTOLOGY_TOOL_NAMES for n in names):
                # A caller-owned tool was requested — hand back verbatim so the
                # caller can execute it.
                break
            messages.append(msg)
            for tc in tool_calls:
                fn = tc.get("function") or {}
                try:
                    args = json.loads(fn.get("arguments") or "{}")
                    if not isinstance(args, dict):
                        args = {}
                except ValueError:
                    args = {}
                result = execute_tool(self.index, fn.get("name", ""), args)
                messages.append({
                    "role": "tool",
                    "tool_call_id": tc.get("id", ""),
                    "content": json.dumps(result),
                })
                total_calls += 1

        resp["ontology"] = {"mode": "tools", "tool_calls": total_calls}
        self._send_json(200, resp)


# ---------------------------------------------------------------------------
# Server bootstrap
# ---------------------------------------------------------------------------

def build_server(cfg: Optional[Config] = None):
    """Build (but do not start) the proxy. Returns (httpd, cfg).

    Reads env at call time via Config(); pass a Config to override. Port 0
    binds an ephemeral port (see ``httpd.server_address[1]``).
    """
    cfg = cfg or Config()
    index = _load_index(cfg)
    handler = type("BoundProxyHandler", (ProxyHandler,),
                   {"cfg": cfg, "index": index})
    httpd = ThreadingHTTPServer((cfg.host, cfg.port), handler)
    httpd.daemon_threads = True
    return httpd, cfg


def main() -> int:
    httpd, cfg = build_server()
    index = httpd.RequestHandlerClass.index
    print(
        "ontology_proxy: listening on %s:%d mode=%s upstream=%s "
        "index=%s (%d classes) budget=%d"
        % (cfg.host, httpd.server_address[1], cfg.mode, cfg.upstream,
           cfg.index_path or "<none>",
           len(index.classes) if index else 0, cfg.budget),
        flush=True,
    )
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        httpd.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
