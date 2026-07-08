# P0 — RES-a: KG liveness watchdog + sprint-wide LivenessHarness

- **Item:** RES-a (PRD-023 WP-11, ADR-130 Decision 3)
- **Canary:** `CANARY-VC-RESA-KG` (standing, P0)
- **Base SHA:** `6cf054347b83f030bf4fa7a8a6166081d0203595` (branch `gap-close/2026-07`)
- **Verified:** 2026-07-08T10:41:36Z (KG watchdog + harness core)
- **Gap-close addendum:** 2026-07-08T11:51:21Z — Nostr-relay tap added (WP-11 AC3 / ADR-130 D3), at SHA `c9f2e353912d1941c09cf110728cdb01a6e0d454`. See "Gap-close: Nostr-relay tap (WP-11 AC3)" below.
- **Maturity:** `planned` → `integrated` (harness core + watchdog wired and unit-verified; the standing-canary live-session fire is `pending-live-session` — it fires only when the running server's watchdog observes a real `/api/health` transition). The Nostr-relay tap for Nostr-only repositories is `integrated` in its pure decision core (mapper unit-tested, connection layer compiled and wired) and `pending-live-session` for an end-to-end fire from a real relay (no relay is configured in this environment — the tap is default-off).

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

## Gap-close: Nostr-relay tap (WP-11 AC3)

An adversarial verifier found WP-11 acceptance criterion 3 met only in half: `POST /api/canary/observe/{canary_id}` records HTTP fires, but the second clause — *"a Nostr-relay tap records fires from repositories that speak only Nostr (forum, solid-pod)"* (ADR-130 Decision 3; DDD cross-repo table rows for `nostr-rust-forum` and `solid-pod-rs`) — had no implementation. This addendum closes that clause.

### What was implemented

- `src/services/canary_nostr_tap.rs` — the tap, in two parts:
  - **Pure decision core** — `map_event_to_observation(&TapEvent, &[String]) -> TapDecision`. No IO, no relay, no crypto: it reads only its two arguments and decides accept/reject on signature validity, kind, the `["t","liveness-canary"]` tag, the pubkey allow-list, and the content shape. On accept it returns the `canary_id` plus an evidence string that discloses the tap provenance (source pubkey + event id) so the fire log is honest about where the fire came from. `TapEvent::from_value` reduces a parsed relay frame to the fields the mapper needs.
  - **Connection layer** — `CanaryNostrTap::{from_env,run}`. `from_env` returns `None` (tap disabled) unless `CANARY_TAP_RELAY_URL` is set; `CANARY_TAP_ALLOWED_PUBKEYS` is a comma-separated x-only-hex allow-list (empty ⇒ every fire rejected, fail-closed on identity). `run` connects over WSS, subscribes with a `["REQ", …, {"kinds":[1],"#t":["liveness-canary"],"authors":[…]}]` filter, verifies each event's BIP-340 signature with `nostr_sdk` (`Event::from_json` + `ev.verify()`, the same path `nostr_bridge.rs` uses), maps it, and on accept calls the SAME `LivenessHarness::observe` the HTTP route uses. Reconnects with exponential backoff (1s→60s, reset on a clean stream end); fail-open on every error.
- `src/services/mod.rs` — registers the module.
- `src/main.rs` — spawns the tap on its own detached task after the KG watchdog, gated on `from_env`; logs "spawned" or "not started" accordingly. It reuses the same `Arc<LivenessHarness>` handle, so the tap feeds the existing canaries and **registers nothing new** (an unregistered `canary_id` yields `CanaryStoreError::NotFound`, logged as a rejected fire).

### Wire contract

The child docs leave the exact tap wire format open ("by subscribing to the wires they already emit"), so the gap-close fixes it as: a fire is a **kind-1** note carrying `["t","liveness-canary"]`, `content` = JSON `{ "canary_id": "...", "evidence": "..." }`, from an allow-listed pubkey. Deliberately ACSP-adjacent (kinds 31400–31405, ADR-110) without colliding with a control kind.

### Falsification (AC3, Nostr clause) → how it is met

- *"a Nostr-only repository cannot fire a canary"* — the tap subscribes to a relay and drives an accepted event into `observe`, the identical path the HTTP route uses; `nostr-rust-forum` / `solid-pod-rs` need only publish the note.
- *"an unsigned or spoofed event fires a canary"* — an event whose BIP-340 signature does not verify is stamped `sig_verified = false` and rejected by the mapper; a pubkey absent from the allow-list is rejected; an empty allow-list rejects everything.
- *"the tap registers a new wire / synthesises a fire"* — the tap only ever calls `observe`; a `canary_id` not already registered is `NotFound` and logged as rejected. No synthetic probe (DDD invariant 5).
- *"a relay outage blocks the server"* — the tap is a detached task with `from_env`-gated startup, reconnect backoff, and fail-open error handling; it is default-off when `CANARY_TAP_RELAY_URL` is unset.

### Tested vs pending-live split (honest disclosure)

- **Tested (no relay):** the pure mapper and `from_value`, unit-tested by feeding parsed events — signature/kind/tag/allow-list/content gates, case-insensitive pubkey match, evidence-provenance and fallback, and shapeless-event rejection (receipt below). `cargo check -p visionclaw-server` compiles the connection layer + the `main.rs` spawn wiring.
- **Pending-live-session:** an actual end-to-end fire from a live relay is **not** exercised here — no `CANARY_TAP_RELAY_URL` is configured in this environment, so the tap is default-off and the WSS subscribe/verify/observe round-trip against a real relay remains unproven. The signature-verification step reuses the already-in-service `nostr_bridge.rs` path, but the socket loop itself is compile-verified only, not run against a relay.

### Receipt

```
$ cargo check -p visionclaw-server
    Finished `dev` profile [optimized + debuginfo] target(s) in 32.67s
  (15 pre-existing dead-code warnings, 0 errors)

$ cargo test -p visionclaw-server --test canary_nostr_tap_test
    Finished `test` profile [optimized + debuginfo] target(s) in 1m 48s
     Running tests/canary_nostr_tap_test.rs

running 12 tests
test accepts_a_valid_signed_allow_listed_fire ... ok
test allow_list_match_is_case_insensitive ... ok
test evidence_falls_back_when_the_repo_supplies_none ... ok
test from_value_rejects_a_shapeless_event ... ok
test from_value_parses_a_raw_relay_event_object ... ok
test rejects_a_missing_liveness_canary_tag ... ok
test rejects_a_pubkey_not_on_the_allow_list ... ok
test rejects_an_empty_canary_id ... ok
test rejects_an_unverified_signature ... ok
test rejects_every_fire_when_the_allow_list_is_empty ... ok
test rejects_the_wrong_kind ... ok
test rejects_malformed_content_json ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Both commands ran at 2026-07-08T11:51:21Z against SHA `c9f2e353912d1941c09cf110728cdb01a6e0d454`. `cargo test` compiled the whole crate (lib + bin, default `gpu` features), so the `main.rs` tap spawn and the connection layer compile; the 12 assertions above cover the pure decision core end-to-end without a relay.
