# P0 — REC-1a / REC-1b: already-closed correctness + regression canary

- **Item:** REC-1a, REC-1b (PRD-023 WP-12, verify-only)
- **Canary:** `CANARY-VC-REC1-ROUTE` (one-shot regression, P0)
- **Base SHA:** `6cf054347b83f030bf4fa7a8a6166081d0203595` (branch `gap-close/2026-07`)
- **Verified:** 2026-07-08T10:41:36Z
- **Maturity:** `integrated` (already closed on `main`; verified and now guarded by a regression test). NOT re-implemented.

## Prior closure (recorded, not re-scoped)

- **REC-1a** — `src/handlers/ontology_agent_handler.rs:344-362`: `/ontology-agent/propose`
  is a nested scope wrapped `RateLimit::per_minute(20)` + `RequireAuth::authenticated()`;
  the read routes (`/discover`,`/read`,`/query`,`/traverse`,`/validate`,`/status`)
  stay anonymous by design (WS-1/ADR-120).
- **REC-1b** — `src/handlers/api_handler/ontology/mod.rs:1361-1423`: the single
  `/ontology` scope is wrapped `RequireAuth::power_user().mutations_only()`; the
  axiom-ingest POSTs `/load` and `/load-axioms` are gated at `power_user`, while
  safe GET reads stay public. The weaker duplicate `authenticated()` route was
  removed (no `ontology_handler::config` export remains).

## Falsification (PRD-023 WP-12) → guard

*"WP-12 is falsified if a route dump reveals any unauthenticated ontology ingest
route … or if REC-1a/1b are re-scoped as new work rather than recorded at their
already-evidenced tier."*

`tests/rec1_route_guard.rs` is the one-shot regression canary. It builds the real
route scopes with a default `NostrService` (so the auth middleware runs its real
verification path) and asserts:

- unauthenticated `POST /ontology-agent/propose` → auth-rejected (403/401);
- unauthenticated `POST /ontology/load` and `/ontology/load-axioms` → auth-rejected;
- read-side `GET /ontology-agent/status` and `GET /ontology/classes` → NOT
  auth-rejected (proving the gate is mutation-specific, not a blanket lock).

## Receipt

```
$ cargo test -p visionclaw-server --test rec1_route_guard
     Running tests/rec1_route_guard.rs

running 4 tests
test ontology_agent_propose_rejects_unauthenticated_ingest ... ok
test ontology_agent_read_side_is_not_auth_gated ... ok
test ontology_read_get_stays_public ... ok
test ontology_load_rejects_unauthenticated_ingest ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```
