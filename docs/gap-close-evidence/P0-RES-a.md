# P0 — RES-a: KG liveness watchdog + sprint-wide LivenessHarness

- **Item:** RES-a (PRD-023 WP-11, ADR-130 Decision 3)
- **Canary:** `CANARY-VC-RESA-KG` (standing, P0)
- **Base SHA:** `6cf054347b83f030bf4fa7a8a6166081d0203595` (branch `gap-close/2026-07`)
- **Verified:** 2026-07-08T10:41:36Z
- **Maturity:** `planned` → `integrated` (harness core + watchdog wired and unit-verified; the standing-canary live-session fire is `pending-live-session` — it fires only when the running server's watchdog observes a real `/api/health` transition).

## What was implemented

A central live-traffic observer in `visionclaw-server`, registrable from any repository, that records a `CanaryFired` only on observed traffic — never a synthetic probe (DDD invariant 5).

- `src/adapters/sqlite_canary_repository.rs` — durable registry (`liveness_canaries`) + append-only fire log (`canary_fires`) in `data/liveness.sqlite3`, mirroring the `SqliteEnrichmentRepository` `tokio-rusqlite` idiom (self-bootstrapping schema, `Arc<Connection>`, single-writer). `all_status` applies the staleness rule: a canary is `fired` only when a fire exists at the current git SHA within the 30-day window; a fire bound to an older SHA or older than the window re-arms it.
- `src/services/liveness_harness.rs` — the `LivenessHarness` service: `register`/`observe`/`status`, the `kg_backend_up` tri-state atomic gauge, `record_kg_state` (fires `CANARY-VC-RESA-KG` on every gauge transition), `seed_p0_canaries` (idempotent seed of the six P0 canary ids from PRD-023), `current_sha()` (build-time `VISIONCLAW_GIT_SHA` from `build.rs`, runtime-overridable), and `run_kg_watchdog` (tokio interval task self-polling `/api/health`, fail-open).
- `src/handlers/liveness_harness_handler.rs` — `POST /api/canary/register`, `POST /api/canary/observe/{canary_id}`, `GET /api/canary/status`.
- Wiring: `AppState.liveness_harness` (opened + seeded in `AppState::new`), registered as `web::Data`, routes mounted under `/api`, and the watchdog spawned in `main.rs` once the server is live. `build.rs` embeds the short git SHA.

## Falsification (PRD-023 WP-11) → how it is met

- *"a canary can be marked fired by a synthetic probe"* — `observe`/`record_kg_state` only ever record from an observed transition/HTTP call; the watchdog itself never marks a canary fired without a real gauge change.
- *"a foreign repository cannot register or fire a canary"* — `register`/`observe` are HTTP surfaces taking `owner_repo`; any repo reaching the service registers and fires.
- *"the KG backend can go unreachable without the gauge flipping and the canary raising"* — `probe_kg` treats a connection failure/timeout/non-2xx/`"unhealthy"` body as down; `record_kg_state(false)` flips the gauge and fires the canary (test below).
- *"a fired canary older than its SHA still counts toward closure"* — `all_status` binds validity to the current SHA + 30-day window (test below).

## Receipt

```
$ cargo test -p visionclaw-server --test liveness_harness_test
    Finished `test` profile [optimized + debuginfo] target(s) in 17.77s
     Running tests/liveness_harness_test.rs

running 4 tests
test kg_watchdog_gauge_transitions_fire_the_canary ... ok
test observe_unknown_canary_is_not_found ... ok
test register_is_idempotent_preserving_registration_sha ... ok
test register_observe_status_and_staleness_rule ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo test` compiled the whole crate (lib + bin, default `gpu` features, nvcc 12.9 present) to completion, so the `main.rs` watchdog spawn + route wiring + `AppState` field also compile.

- `tests/liveness_harness_test.rs::register_observe_status_and_staleness_rule` — a fire at `shaA` counts at `shaA` within the window, re-arms at `shaB`, and re-arms beyond +40 days.
- `tests/liveness_harness_test.rs::kg_watchdog_gauge_transitions_fire_the_canary` — `unknown→up` fires once (watchdog live), `up→up` no-ops, `up→down` (simulated loss) fires again; status shows `CANARY-VC-RESA-KG` fired with ≥2 observations.
