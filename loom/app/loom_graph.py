#!/usr/bin/env python3
"""loom_graph — the Loom's single read-truth graph store (pyoxigraph over the generation).

Loads the mirrored reasoned generation (ontology.ttl + ontology-inferred.ttl) into an
in-process pyoxigraph Store and serves read-only SPARQL + node search. This makes the Loom
the single QUERYABLE read-truth for the published reasoned corpus (VisionClaw ADR-135 D2/D3
consolidation): reasoning happened at CI build time, so the Loom serves the pre-reasoned
closure without running Whelk.

Optional dependency: if pyoxigraph is not installed, the store is disabled and the façade
falls back to the flat scaffold-index (retrieval still works) — fail-open, honestly reported
in /health.

Read-only + clamped, mirroring VisionClaw's SPARQL invariants (ADR-117):
- only SELECT / ASK / CONSTRUCT / DESCRIBE (no UPDATE/LOAD/CLEAR/DROP/INSERT/DELETE)
- SERVICE forbidden (SSRF)
- server-side LIMIT clamp + row cap
"""
from __future__ import annotations

import os
import re
import time
from typing import Any, Optional

try:
    import pyoxigraph as ox  # type: ignore
    _HAVE_OX = True
except Exception:  # noqa: BLE001
    _HAVE_OX = False

DEFAULT_LIMIT = int(os.environ.get("LOOM_SPARQL_LIMIT", "10000"))
MAX_ROWS = int(os.environ.get("LOOM_SPARQL_MAX_ROWS", "10000"))
_FORBIDDEN = re.compile(r"\b(INSERT|DELETE|LOAD|CLEAR|DROP|CREATE|COPY|MOVE|ADD|SERVICE)\b", re.IGNORECASE)
_READ_FORM = re.compile(r"\b(SELECT|ASK|CONSTRUCT|DESCRIBE)\b", re.IGNORECASE)
_LIMIT_RE = re.compile(r"\bLIMIT\s+\d+", re.IGNORECASE)


class LoomGraph:
    """In-process read-truth store loaded from the mirrored generation."""

    def __init__(self, data_dir: str):
        self.data_dir = data_dir
        self.store: Optional[Any] = None
        self.loaded_files: list[str] = []
        self.triples = 0
        self.loaded_at: Optional[str] = None
        self.error: Optional[str] = None

    @property
    def available(self) -> bool:
        return _HAVE_OX and self.store is not None

    def load(self) -> None:
        """Load ontology.ttl + ontology-inferred.ttl into a fresh in-memory store.

        INVARIANT (DDD BC24 I11): the Loom serves the PUBLISHED ONTOLOGY ONLY. It
        never loads, mirrors, or queries the working graph (personal/working notes,
        potentially multi-user/private). Only these two published-ontology artifacts
        are ever loaded — do NOT add workingGraph sources here. Uplift into the
        ontology happens via VisionClaw / the forum / agentic writes into the logseq
        corpus, never through the Loom.
        """
        if not _HAVE_OX:
            self.error = "pyoxigraph not installed — graph read-truth disabled (scaffold still works)"
            return
        try:
            store = ox.Store()  # in-memory
            n0 = 0
            for name in ("ontology.ttl", "ontology-inferred.ttl"):
                path = os.path.join(self.data_dir, name)
                if os.path.exists(path):
                    with open(path, "rb") as f:
                        store.bulk_load(f, "text/turtle")
                    self.loaded_files.append(name)
            self.store = store
            self.triples = len(store)
            self.loaded_at = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            self.error = None
            _ = n0
        except Exception as e:  # noqa: BLE001
            self.store = None
            self.error = f"graph load failed: {e}"

    # -- read-only clamped SPARQL ------------------------------------------

    def validate(self, query: str) -> Optional[str]:
        if _FORBIDDEN.search(query):
            return "forbidden keyword (write/SERVICE) — the Loom store is read-only"
        if not _READ_FORM.search(query):
            return "only SELECT/ASK/CONSTRUCT/DESCRIBE are permitted"
        return None

    def _clamp(self, query: str) -> str:
        # inject a LIMIT on SELECT if the caller omitted one
        if _READ_FORM.search(query) and re.match(r"\s*SELECT", query, re.IGNORECASE) and not _LIMIT_RE.search(query):
            return query.rstrip().rstrip(";") + f"\nLIMIT {DEFAULT_LIMIT}"
        return query

    def sparql(self, query: str) -> dict:
        if not self.available:
            return {"error": "graph_unavailable", "detail": self.error or "store not loaded"}
        bad = self.validate(query)
        if bad:
            return {"error": "bad_query", "detail": bad}
        q = self._clamp(query)
        try:
            res = self.store.query(q)
        except Exception as e:  # noqa: BLE001
            return {"error": "query_failed", "detail": str(e)}
        # SELECT → list of binding dicts; ASK → bool; CONSTRUCT/DESCRIBE → triples
        if isinstance(res, bool):
            return {"boolean": res}
        rows: list[dict] = []
        truncated = False
        try:
            variables = [str(v)[1:] if str(v).startswith("?") else str(v) for v in getattr(res, "variables", [])]
        except Exception:  # noqa: BLE001
            variables = []
        try:
            for i, sol in enumerate(res):
                if i >= MAX_ROWS:
                    truncated = True
                    break
                if hasattr(sol, "__iter__") and not isinstance(sol, (str, bytes)):
                    row = {}
                    for v in variables:
                        try:
                            term = sol[v]
                            row[v] = _term_str(term) if term is not None else None
                        except Exception:  # noqa: BLE001
                            pass
                    rows.append(row)
        except Exception:
            # CONSTRUCT/DESCRIBE yield triples, not solution mappings
            for i, tr in enumerate(res):
                if i >= MAX_ROWS:
                    truncated = True
                    break
                rows.append({"triple": str(tr)})
        return {"variables": variables, "rows": rows, "count": len(rows), "truncated": truncated}

    # -- node search over the store (ALL nodes, not just OWL classes) -------

    def search(self, q: str, limit: int = 20) -> dict:
        """Substring/label search over rdfs:label + skos:prefLabel + IRI local names."""
        if not self.available:
            return {"error": "graph_unavailable", "detail": self.error or "store not loaded"}
        needle = q.strip().lower()
        if not needle:
            return {"error": "empty query"}
        sq = f'''
        SELECT DISTINCT ?s ?label WHERE {{
          ?s ?p ?label .
          FILTER(?p IN (<http://www.w3.org/2000/01/rdf-schema#label>,
                        <http://www.w3.org/2004/02/skos/core#prefLabel>,
                        <https://narrativegoldmine.com/ns/v1#title>))
          FILTER(CONTAINS(LCASE(STR(?label)), "{_esc(needle)}"))
        }} LIMIT {int(limit)}
        '''
        out = self.sparql(sq)
        if "rows" in out:
            out["hits"] = [{"iri": r.get("s"), "label": r.get("label")} for r in out["rows"]]
            del out["rows"]
        return out

    def status(self) -> dict:
        return {
            "available": self.available,
            "engine": "pyoxigraph" if _HAVE_OX else None,
            "triples": self.triples,
            "loaded_files": self.loaded_files,
            "loaded_at": self.loaded_at,
            "error": self.error,
        }


def _term_str(term: Any) -> str:
    s = str(term)
    # pyoxigraph renders literals as "v"^^<type> / IRIs as <iri>; strip wrappers for readability
    if s.startswith("<") and s.endswith(">"):
        return s[1:-1]
    m = re.match(r'^"(.*)"(?:\^\^<.*>|@[a-zA-Z-]+)?$', s, re.DOTALL)
    return m.group(1) if m else s


def _esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')
