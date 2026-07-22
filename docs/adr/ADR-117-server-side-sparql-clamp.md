# ADR-117 — Server-side SPARQL clamp (default LIMIT + row/byte cap) as a hard invariant

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-117). The server-side clamp — flagged half-shipped
in the doc-drift audit (§2 C3) — **shipped 2026-07-22** in
`src/handlers/ontology_handler.rs` (`clamp_sparql_limit` `:801`, `cap_result_rows`
`:831`) with a regression guard in `tests/ontology_sparql_clamp.rs` (7/7 pass).
**Date:** 2026-06-14 (decided under ADR-112, WS-0) · shipped + recorded 2026-07-22
**Decision-type:** Architecture (WS-0 hard prerequisite)
**Relates:** ADR-112 (keystone §2.5), ADR-116 (client-side budget governor this
backstops), ADR-118 (`/load` hardening — same WS-0 sweep), PRD-020

---

## 1. Context

ADR-112 §2.5 named a **server-side SPARQL LIMIT/row/byte clamp in VisionClaw** as
a hard prerequisite (WS-0): the client-side budget governor (ADR-116) lives in
agentbox and cannot protect Oxigraph if any *other* authed caller issues an
unbounded SELECT. The doc-drift audit (§2 C3) confirmed the gap was real: the
`/ontology/query` CQRS read path returned `Vec<HashMap>` straight from Oxigraph's
`run_select` with **no** LIMIT / row / byte bound (the sibling `/ontology/sparql`
path was already fenced inside `sparql_select_json`). Agentbox compensated
client-side only — an authed caller could still materialise an unbounded SELECT.

## 2. Decision

Enforce the clamp **at the VisionClaw handler boundary**, before the query
reaches Oxigraph, as a hard invariant on every read-SPARQL path:

- **`clamp_sparql_limit(query)`** injects a trailing `LIMIT DEFAULT_SPARQL_LIMIT`
  (`10_000`) on a SELECT that has none, and rewrites a trailing top-level LIMIT
  that exceeds the cap down to the hard cap. `ASK`/construct-shaped queries with
  no bindable result set are left untouched. Applied in both `query_ontology`
  (CQRS `/ontology/query`, `:870`) and `sparql_query` (`/ontology/sparql`, `:933`).
- **`cap_result_rows(results)`** is the row/byte fence behind the string rewrite:
  it truncates at the hard row cap (`10_000`) and an **8 MiB** byte cap, and
  returns an explicit `truncated` flag rather than silently cutting. A sub-SELECT
  LIMIT that slipped past the string clamp is caught here.
- The read response shape is now `{ results, rowCount, truncated }`
  (`ontology_handler.rs:892-895`) — no consumer relied on the old bare array.
- Read-only enforcement + `SERVICE` denial remain (pre-existing); the clamp is
  the missing *volume* bound on top of the *operation* bound.

## 3. Consequences

**Positive** — an authed unbounded SELECT can no longer materialise the whole
store; the overflow guarantee is now enforced on **both** sides of the wire
(agentbox budget governor ADR-116 + this server clamp), closing the WS-0
invariant honestly. The explicit `truncated` flag makes clamping observable to a
caller rather than a silent surprise.

**Negative** — a legitimate large analytical SELECT is capped at 10,000 rows /
8 MiB and must paginate; acceptable for an augmentation/read surface, and the
`truncated` flag signals when it bit.

**Neutral** — the LIMIT injection is a best-effort string rewrite (documented as
such at `:798`); the row/byte fence in `cap_result_rows` is the authoritative
backstop, so a rewrite that misses a pathological query still cannot overrun.

## 4. Verification
`tests/ontology_sparql_clamp.rs` (7/7): no-LIMIT gets the default injected; an
oversize LIMIT is rewritten to the cap and the original value is gone; a
within-cap LIMIT passes through unchanged; `ASK` is untouched; oversize result
sets truncate with the `truncated` flag set. Reconciles ADR-112 §7 item 1
(WS-0 route-dump: `SELECT ?s ?p ?o` returns ≤ cap).
