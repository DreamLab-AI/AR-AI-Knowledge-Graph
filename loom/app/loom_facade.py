#!/usr/bin/env python3
"""Loom façade — the single, deployment-agnostic contact endpoint (Deployment B sidecar).

Implements the stable façade contract from VisionClaw ADR-135 D1, minimal v1:
  GET  /health                 → liveness + generation stamp + backend reachability
  GET  /loom/generation        → the corpus generation identity this Loom serves
  POST /loom/scaffold          → direct budget-clamped ontology scaffold (NO LLM — proves
                                 the retrieval facet; testable from anywhere, no backend)
  POST /v1/chat/completions    → scaffold-inject the last user message, then delegate to
                                 DISTILL_BACKEND_URL (the model-swap seam — swap the model
                                 behind this door with zero consumer change)
  GET  /v1/models              → passthrough to the backend (identity probe)

STDLIB ONLY. The model is a URL, never baked into the contract (ADR-135 D1). Grounding is
static structured scaffold injection — the benchmark (PRD-025 §3.1) says feed the model,
don't send it traversing.
"""
from __future__ import annotations

import json
import os
import sys
import time
import urllib.request
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ontology_scaffold as osc  # noqa: E402

BACKEND = os.environ.get("DISTILL_BACKEND_URL", "").rstrip("/")  # e.g. http://192.168.2.132:8084/v1
INDEX = os.environ.get("ONTOLOGY_INDEX", "/app/data/scaffold-index.json")
PROSE = os.environ.get("ONTOLOGY_PROSE_INDEX", "/app/data/prose-index.json")
BUDGET = int(os.environ.get("ONTOLOGY_BUDGET", "1500"))
PORT = int(os.environ.get("LOOM_FACADE_PORT", "8080"))
TIMEOUT = float(os.environ.get("LOOM_TIMEOUT", "600"))  # distillation is slow by design

os.environ.setdefault("ONTOLOGY_INDEX", INDEX)
os.environ.setdefault("ONTOLOGY_PROSE_INDEX", PROSE)


def _generation() -> dict:
    """Corpus generation identity — from build-manifest if mirrored (WS-A), else the
    scaffold-index's own stamp (pre-manifest fallback)."""
    for path, keys in ((os.path.join(os.path.dirname(INDEX), "build-manifest.json"),
                        ("commitSha", "buildId", "generatedAt", "pipelineVersion")),):
        try:
            with open(path) as f:
                m = json.load(f)
            return {k: m.get(k) for k in keys} | {"source": "build-manifest"}
        except (OSError, ValueError):
            pass
    try:
        with open(INDEX) as f:
            d = json.load(f)
        return {"generatedAt": d.get("generated"), "classes": d.get("counts", {}).get("classes"),
                "commitSha": None, "source": "scaffold-index (pre-manifest)"}
    except (OSError, ValueError):
        return {"source": "unavailable"}


def _backend(path: str, body: bytes | None, method: str) -> tuple[int, bytes, str]:
    if not BACKEND:
        return 503, b'{"error":"no DISTILL_BACKEND_URL configured"}', "application/json"
    url = f"{BACKEND}{path[len('/v1'):]}" if path.startswith("/v1") else f"{BACKEND}{path}"
    req = urllib.request.Request(url, data=body, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as r:
            return r.status, r.read(), r.headers.get("Content-Type", "application/json")
    except urllib.error.HTTPError as e:
        return e.code, e.read(), "application/json"
    except Exception as e:  # noqa: BLE001
        return 502, json.dumps({"error": "backend_unreachable", "detail": str(e)}).encode(), "application/json"


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):  # quieter
        sys.stderr.write("[loom] " + (a[0] % a[1:]) + "\n")

    def _send(self, code, payload, ctype="application/json"):
        body = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def _read(self) -> dict:
        n = int(self.headers.get("Content-Length", 0) or 0)
        if not n:
            return {}
        try:
            return json.loads(self.rfile.read(n))
        except ValueError:
            return {}

    def do_GET(self):
        if self.path == "/health":
            idx = None
            try:
                idx = osc.get_index()
            except Exception:  # noqa: BLE001
                pass
            self._send(200, {
                "ok": True,
                "facet": "loom-facade",
                "mode": "scaffold",
                "backend": BACKEND or None,
                "backend_reachable": self._probe_backend() if BACKEND else None,
                "index_classes": len(idx.classes) if idx else None,
                "generation": _generation(),
            })
        elif self.path in ("/loom/generation", "/generation"):
            self._send(200, _generation())
        elif self.path.startswith("/v1/"):
            code, body, ctype = _backend(self.path, None, "GET")
            self._send(code, body, ctype)
        else:
            self._send(404, {"error": "not_found", "path": self.path})

    def do_POST(self):
        if self.path in ("/loom/scaffold", "/scaffold"):
            j = self._read()
            prompt = j.get("prompt") or j.get("query") or ""
            if not prompt:
                return self._send(400, {"error": "missing prompt"})
            budget = int(j.get("budget_tokens", BUDGET))
            prose = bool(j.get("prose", False))
            try:
                block = osc.scaffold(prompt, budget_tokens=budget,
                                     max_seeds=int(j.get("max_seeds", 4)),
                                     hops=int(j.get("hops", 1)), prose=prose)
            except Exception as e:  # noqa: BLE001
                return self._send(500, {"error": "scaffold_failed", "detail": str(e)})
            self._send(200, {
                "scaffold": block,
                "engaged": bool(block),
                "approx_tokens": (len(block) + 3) // 4,
                "prose": prose,
                "generation": _generation(),
            })
        elif self.path == "/v1/chat/completions":
            j = self._read()
            msgs = j.get("messages", [])
            injected = 0
            try:
                before = sum(len(str(m.get("content", ""))) for m in msgs)
                msgs = osc.scaffold_messages(msgs, budget_tokens=int(j.get("ontology_budget", BUDGET)),
                                             prose=bool(j.get("ontology_prose", False)))
                after = sum(len(str(m.get("content", ""))) for m in msgs)
                injected = max(0, (after - before + 3) // 4)
            except Exception as e:  # noqa: BLE001
                sys.stderr.write(f"[loom] scaffold skip: {e}\n")
            j["messages"] = msgs
            j.pop("stream", None)
            code, body, ctype = _backend("/v1/chat/completions", json.dumps(j).encode(), "POST")
            # annotate for bench accounting (fail-labelled honesty)
            if code == 200 and ctype.startswith("application/json"):
                try:
                    obj = json.loads(body)
                    obj["loom"] = {"mode": "scaffold", "injected_tokens": injected,
                                   "generation": _generation()}
                    body = json.dumps(obj).encode()
                except ValueError:
                    pass
            self._send(code, body, ctype)
        elif self.path.startswith("/v1/"):
            code, body, ctype = _backend(self.path, self._read_raw(), "POST")
            self._send(code, body, ctype)
        else:
            self._send(404, {"error": "not_found", "path": self.path})

    def _read_raw(self) -> bytes:
        n = int(self.headers.get("Content-Length", 0) or 0)
        return self.rfile.read(n) if n else b""

    def _probe_backend(self) -> bool:
        try:
            req = urllib.request.Request(f"{BACKEND}/models", method="GET")
            with urllib.request.urlopen(req, timeout=5):
                return True
        except Exception:  # noqa: BLE001
            return False


def main():
    # warm the index so first request is fast + fail loudly if the corpus is missing
    try:
        idx = osc.get_index()
        sys.stderr.write(f"[loom] index loaded: {len(idx.classes)} classes; generation={_generation()}\n")
    except Exception as e:  # noqa: BLE001
        sys.stderr.write(f"[loom] WARNING index not loaded: {e}\n")
    sys.stderr.write(f"[loom] façade on :{PORT}; backend={BACKEND or '(none)'}; budget={BUDGET}\n")
    ThreadingHTTPServer(("0.0.0.0", PORT), Handler).serve_forever()


if __name__ == "__main__":
    main()
