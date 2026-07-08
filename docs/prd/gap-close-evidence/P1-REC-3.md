# P1 Evidence — REC-3: Contextual Transaction Cost envelope schema

**Item:** REC-3 (PRD-023 WP-7) · **Wave:** P1 · **Canary:** `CANARY-VC-REC3-CTC` (one-shot)
**Base SHA at verification:** `e0f582403` (working tree; committed under the gap-close/2026-07 commit that lands this file)
**Verified:** 2026-07-08T14:00Z

## What was built

The `/wss/agent-events` envelope (`src/agent_events/schema.rs`, `AgentActionEnvelope`)
gains three first-class **typed optional** CTC members — the contract PRD-023/ADR-130
fixed, kept aligned with what agentbox emits (its ADR-037):

```rust
#[serde(default)] pub handoff_count: Option<u32>,
#[serde(default)] pub token_burden: Option<u64>,
#[serde(default)] pub verification_outcome: Option<String>,
```

- **Additive + versioned + absence-tolerant:** each field is `#[serde(default)]`, so a
  pre-REC-3 producer that omits every CTC field still deserialises, and a consumer that
  does not read them is unaffected (verified by `ctc_absent_deserialises_as_none_and_has_ctc_false`).
- **First-class, not the blob:** `AgentActionEnvelope::has_ctc()` reads the typed fields
  directly — CTC data never rides only the untyped `metadata` value.
- **`token_burden` is `u64`:** the fixture uses `5_000_000_000` (> u32) to prove full
  width, mirroring the epoch-ms full-width handling already in the envelope.
- **Canary wiring:** the ingest (`src/agent_events/ingest.rs`) computes `ctc_present` per
  frame and fires `CANARY-VC-REC3-CTC` once per process (an `AtomicBool` latch) on the first
  CTC-bearing envelope, via the RES-a `LivenessHarness.observe` path — observed live traffic,
  not a synthetic probe. The canary is seeded in `P1_CANARIES` (`liveness_harness.rs`).

## Acceptance criteria (PRD-023 WP-7)

| # | Criterion | Status |
|---|---|---|
| 1 | `AgentActionEnvelope` gains typed optional `handoff_count`/`token_burden`/`verification_outcome`, each `#[serde(default)]` | **Met** — `schema.rs` |
| 2 | A real DAG emits an envelope carrying populated CTC fields, observed on the wire | **Instrumented** — ingest fires `CANARY-VC-REC3-CTC` on a CTC-bearing frame; the live fire is a sprint-end live-session observation (agentbox emits the fields this wave) |

## Falsification statement and how this survives it

> *WP-7 is falsified if CTC data still rides only the untyped `metadata` blob, or if REC-3
> closes without a live envelope carrying a populated typed CTC field.*

- CTC data is now three typed struct members read by `has_ctc()` without touching `metadata`
  — the blob-only condition is structurally impossible.
- Closure requires `CANARY-VC-REC3-CTC` to fire; the ingest fires it on a real CTC-bearing
  frame. A closure asserted without a fired canary is visibly `armed` in `GET /api/canary/status`.

## Execution receipts

### Rust unit tests (schema + ingest CTC)

```
$ cargo test --lib -- agent_events::schema agent_events::ingest kpi_compute sqlite_kpi_repository sqlite_enrichment_repository
running 27 tests
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 751 filtered out; finished in 0.01s
```

New tests exercising REC-3 specifically:
- `agent_events::schema::tests::ctc_fields_deserialise_typed_and_report_present`
- `agent_events::schema::tests::ctc_absent_deserialises_as_none_and_has_ctc_false`
- `agent_events::schema::tests::ctc_round_trips_through_serde`
- `agent_events::ingest::tests::ctc_bearing_frame_reports_ctc_present`
- `agent_events::ingest::tests::canonical_frame_publishes_to_hub_and_round_trips` (updated: asserts `!ctc_present` on a non-CTC frame)

### Library compile

```
$ cargo check --lib
# 0 errors (58 pre-existing warnings unchanged)
```

## Files

- `src/agent_events/schema.rs` — the three typed CTC fields + `has_ctc()` + tests
- `src/agent_events/ingest.rs` — `ctc_present` in `IngestOutcome::Published`; one-shot canary fire
- `src/services/liveness_harness.rs` — `CANARY_REC3_CTC` const + `P1_CANARIES` seed row
- `src/agent_events/provenance.rs`, `src/actors/agent_beam_actor.rs` — test fixtures updated for the new fields
