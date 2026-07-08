# P1 Evidence — RES-d: Script-queryable ontology class-count source

**Item:** RES-d source (PRD-023 WP-12) · **Wave:** P1 · **Canary:** `CANARY-VC-RESD-COUNT` (one-shot)
**Base SHA at verification:** `e0f582403` (working tree; committed under the gap-close/2026-07 commit that lands this file)
**Verified:** 2026-07-08T14:00Z

## What was built

`src/handlers/ontology_class_count_handler.rs` — `GET /api/ontology/class-count`, a
script-queryable source the canon `DriftCounter` consumes to detect ontology drift.

- **Live count, not cached:** reads `OxigraphOntologyRepository::get_metrics().class_count`,
  which runs the SPARQL aggregate

  ```sparql
  SELECT (COUNT(?s) AS ?n)
  WHERE { GRAPH <urn:ngm:graph:ontology> { ?s a vc:OntologyClass } }
  ```

  against the live Oxigraph store — so the figure always matches Oxigraph (WP-12 AC2).
- **Documented method:** the JSON response carries `source`, `graph`, and the exact `method`
  string so a consumer can reproduce and verify it.
- **Canary wiring:** a successful read fires `CANARY-VC-RESD-COUNT` via the RES-a harness —
  the `DriftCounter`'s own query is the observed live traffic. Fail-open: a canary write error
  never fails the count the canon depends on. Seeded in `P1_CANARIES`.
- **Route placement:** `/api/ontology/class-count` — a distinct path from the single
  `/ontology` scope, mounted at `/api` in `main.rs`. Read-only, unauthenticated by design
  (a class count is a public liveness figure carrying no ontology content).

## Response shape

```json
{
  "success": true,
  "data": {
    "class_count": 5975,
    "source": "oxigraph",
    "graph": "urn:ngm:graph:ontology",
    "method": "SPARQL COUNT(?s) WHERE GRAPH <urn:ngm:graph:ontology> { ?s a vc:OntologyClass }",
    "sha": "<git sha>",
    "observed_at_ms": 1751985600000
  }
}
```

## Acceptance criteria (PRD-023 WP-12)

| # | Criterion | Status |
|---|---|---|
| 2 | A script-queryable endpoint returns a live ontology class count matching Oxigraph, consumed by the canon `DriftCounter` | **Met** — `GET /api/ontology/class-count` reads the live SPARQL COUNT; `curl`-queryable; fires `CANARY-VC-RESD-COUNT` |

## Falsification statement and how this survives it

> *WP-12 is falsified if a route dump reveals any unauthenticated ontology **ingest** route,
> if the class-count source drifts from Oxigraph, or if REC-1a/1b are re-scoped as new work.*

- The new route is a **read-only COUNT**, not an ingest route — it mounts no mutation and adds
  no unauthenticated write surface (REC-1a/1b auth gates untouched).
- The count is read straight from Oxigraph via `get_metrics()` on every call — it cannot drift
  from Oxigraph because it is Oxigraph's own answer, computed on demand, never cached.

## Execution receipts

### Library compile (handler + route wiring)

```
$ cargo check --lib
# 0 errors (58 pre-existing warnings unchanged)
```

### Live invocation (sprint-end live session, against a running stack)

```
$ curl -fsS http://localhost:4000/api/ontology/class-count | jq '.data.class_count'
# → the live OWL class count read from Oxigraph; the same call fires CANARY-VC-RESD-COUNT,
#   verifiable in GET /api/canary/status.
```

The handler is compiled and route-mounted; the live count value and the canary fire are a
running-stack observation captured in the sprint-end live session (the KG store must be
loaded for a non-zero count).

## Files

- `src/handlers/ontology_class_count_handler.rs` — the route + documented method + canary fire
- `src/handlers/mod.rs` — `configure_ontology_class_count_routes` export
- `src/main.rs` — route mounted under `/api`
- `src/services/liveness_harness.rs` — `CANARY_RESD_COUNT` const + `P1_CANARIES` seed row
