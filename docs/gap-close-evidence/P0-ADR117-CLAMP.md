# P0 — ADR-117 WS-0: server-side SPARQL result clamp on /ontology/query

- **Item:** ADR-117 server-side SPARQL clamp — inject/clamp `LIMIT` and cap
  rows/bytes on the read-only query path so an authed caller cannot materialise an
  unbounded SELECT against Oxigraph (doc-drift audit §2 C3, §3 "ADR-117 clamp";
  anomaly **N-sparql-clamp-halfshipped**).
- **Repo:** VisionClaw server (`src/handlers/ontology_handler.rs`).
- **Verified:** 2026-07-22T15:01Z (HEAD `f4e82dc2`).
- **Maturity / tier:** `integrated` (code-proven) — the clamp + cap ship in the
  handler, the `/ontology/query` response now carries an explicit `truncated`
  flag, and 7/7 unit tests pass. A live-fire against a real >10k-row Oxigraph
  query is `pending-live-session` (see below).

## The proven gap (what the audit found)

The doc-drift audit (`docs/audit-doc-drift-2026-07-22.md` §2 C3, §5
N-sparql-clamp-halfshipped) found ADR-117 **half-shipped**:

1. `validate_read_only_sparql` enforced read-only (denylisting
   INSERT/DELETE/DROP/CLEAR/LOAD/CREATE/ADD/MOVE/COPY/WITH) and forbade `SERVICE`
   (SSRF/exfil), **but injected no default LIMIT and no row/byte cap** before
   handing the query to the store.
2. The CQRS `/ontology/query` path returned `Vec<HashMap<String,String>>`
   straight from `run_select` with **no internal fence** (unlike the sibling
   `/ontology/sparql` path, already fenced inside `sparql_select_json`).
3. So an **authed** caller could issue an unbounded `SELECT` against Oxigraph;
   the agentbox budget governor only compensated **client-side**, which a direct
   API caller bypasses.

## What was implemented (C3)

All in `src/handlers/ontology_handler.rs`:

- **Constants (the WS-0 invariant), `:789-791`:**
  - `DEFAULT_SPARQL_LIMIT = 10_000` — injected LIMIT for a no-LIMIT SELECT;
  - `MAX_SPARQL_ROWS = 10_000` — hard row cap (also the oversize-LIMIT rewrite
    target);
  - `MAX_SPARQL_RESULT_BYTES = 8 * 1024 * 1024` — 8 MiB serialised-byte cap.
- **`clamp_sparql_limit(query) -> String`, `:801-825`** — a pre-validated SELECT
  with no trailing top-level `LIMIT` gets `LIMIT 10000` appended; a SELECT whose
  trailing `LIMIT N` exceeds the cap is rewritten down to `10000` (preserving any
  trailing `OFFSET`); ASK/CONSTRUCT/DESCRIBE are returned unchanged. Applied at
  the handler at `:870` before dispatch.
- **`cap_result_rows(rows) -> (rows, bool)`, `:831-852`** — the parse-independent
  backstop: truncates to `MAX_SPARQL_ROWS` and runs a cumulative serialised-byte
  fence at 8 MiB, returning an explicit `truncated` flag. A sub-SELECT LIMIT that
  slipped past the string rewrite cannot slip past this — it caps what was
  actually materialised. Applied at the handler at `:884`.
- **`SERVICE` forbidden.** The read-only validator's `FORBIDDEN` array
  (`:746-749`) now includes `SERVICE` — the SSRF/exfil vector — cited to
  PRD-020 WS-0 / ADR-117 in the code comment (`:744-745`).

## Response-shape change (contract)

`/ontology/query` (`query_ontology`, `:854-906`) previously returned a **bare
JSON array** of rows. It now returns a **typed envelope** (`:891-895`):

```json
{ "results": [ … ], "rowCount": <usize>, "truncated": <bool> }
```

`truncated: true` is an **explicit** signal that the row and/or byte cap fired —
a truncation is surfaced, never silently cut. Per the Phase A landing, **zero
prior consumers used the old bare-array shape**, so the envelope change breaks no
caller.

## Tests — 7/7

`tests/ontology_sparql_clamp.rs` exercises the two pure helpers (`clamp_sparql_limit`,
`cap_result_rows`) exported from the handler:

| # | Test | Proves |
|---|---|---|
| 1 | `no_limit_query_gets_default_injected` | no-LIMIT SELECT → `LIMIT 10000` injected |
| 2 | `oversize_limit_is_reduced_to_cap` | `LIMIT 5000000` rewritten to `10000`; original value gone |
| 3 | `within_cap_limit_is_preserved` | `LIMIT 50` passes through unchanged |
| 4 | `ask_query_is_not_rewritten` | ASK left untouched (no result set to bound) |
| 5 | `byte_cap_truncation_is_flagged` | ~10.5 MiB / 5000 rows (under the row cap) → byte fence fires, `truncated` set, rows dropped |
| 6 | `row_count_cap_is_flagged` | `ROW_CAP + 5` rows → truncated to exactly `10000`, flag set |
| 7 | `under_cap_result_is_not_flagged` | 100 rows → intact, not flagged |

7/7 pass (Phase A landing). Test 5 deliberately keeps the row count under the row
cap so it **isolates the byte fence**; test 6 keeps rows tiny so it isolates the
row-count fence — the two caps are proven independently.

## Live-fire status (honest, pending-live-session)

The unit tests prove the pure clamp/cap logic. A **live-fire** — POSTing an
un-LIMITed `SELECT` that returns >10 000 real rows against a running Oxigraph
through `visionclaw-server:4000` and observing `truncated: true` + a ≤8 MiB body
on the wire — is **`pending-live-session`**:

```
$ curl -s -o /dev/null -w "%{http_code}" http://visionclaw-server:4000/api/canary/register
UNREACHABLE       # port 4000 unreachable from this container, 2026-07-22
```

The clamp is code-proven and unit-tested end-to-end on the helpers; the
full-stack live query against a real store waits for a live session. Disclosed,
not assumed.

## Falsification → how it is met

- **Primary:** *"POST an un-LIMITed SELECT returning >10000 rows; if the response
  lacks `truncated:true` or exceeds 8 MiB, this claim is false."* — **met in
  logic**: `clamp_sparql_limit` (`:801`) injects `LIMIT 10000` on a no-LIMIT
  SELECT before dispatch (`:870`), and `cap_result_rows` (`:831`) truncates to
  10 000 rows / 8 MiB and sets `truncated` on the materialised set (`:884`), which
  the handler returns as `{results, rowCount, truncated}` (`:891-895`). Tests 1,
  5, and 6 exercise each fence. The full-stack wire observation is
  `pending-live-session` (port 4000 unreachable).
- Falsified if the response were still a bare array — **met** (envelope shipped,
  `:891-895`; zero prior consumers).
- Falsified if `SERVICE` reached the store — **met** (added to `FORBIDDEN`,
  `:748`).
- Falsified if an oversize sub-SELECT LIMIT bypassed the fence — **met**
  (`cap_result_rows` is parse-independent and caps the materialised rows
  regardless of what the string rewrite saw; test 6).

## Receipts

```
$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-07-22T15:01:37Z
$ git rev-parse HEAD
f4e82dc2cb0aae4a8437b1e4d3e364da7c63e0de

# constants + helpers
$ grep -n "MAX_SPARQL_ROWS\|DEFAULT_SPARQL_LIMIT\|MAX_SPARQL_RESULT_BYTES\|fn clamp_sparql_limit\|fn cap_result_rows" \
      src/handlers/ontology_handler.rs
789:const MAX_SPARQL_ROWS: usize = 10_000;
790:const DEFAULT_SPARQL_LIMIT: usize = 10_000;
791:const MAX_SPARQL_RESULT_BYTES: usize = 8 * 1024 * 1024;
801:pub fn clamp_sparql_limit(query: &str) -> String {
831:pub fn cap_result_rows( … ) -> (Vec<…>, bool) {

# response envelope
$ grep -n '"results"\|"rowCount"\|"truncated"' src/handlers/ontology_handler.rs
891:            ok_json!(serde_json::json!({
892:                "results": results,
893:                "rowCount": row_count,
894:                "truncated": truncated,

# 7 tests
$ grep -c "#\[test\]" tests/ontology_sparql_clamp.rs
7
```

An adversarial verifier re-running the seven tests finds the clamp/cap logic
proven; the only un-proven arm is the full-stack >10k-row live query, honestly
tagged `pending-live-session` because port 4000 is unreachable from this
container.
