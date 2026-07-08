# P2 evidence — REC-10 Insight Ingestion Loop v1 (PRD-023 WP-12)

**Item:** Wire the five-stage Insight Ingestion Loop v1 across what ships —
`ontology_propose` (the governed write-back queue) → broker case → decision →
merged enrichment — with a **persisted timestamp at each stage** so Mesh Velocity
(insight-to-integration time) is computable, and expose the loop trace via a REST
read. The amplification stage is **planned** and labelled so.
**Base commit verified against:** `774ffa05e` (`gap-close/2026-07`)
**Maturity:** `planned` → `integrated` (P2). Stages 1–4 wired end to end with
persisted timestamps and a computed Mesh Velocity; stage 5 (amplification)
labelled `planned` in the trace, never fabricated.
**Canary:** `CANARY-VC-REC10-LOOP` (one-shot, P2) — seeded in `P2_CANARIES`, fired
from the loop-trace read when a closed monotonic loop is observed.

## What ships

The loop rides the stores that already exist (ADR-130: reuse, don't add a
store). The one additive persistence change is a stage-4 timestamp column.

| Stage | Timestamp source | Persisted where |
|---|---|---|
| 1 propose | `proposal_json.proposed_at_ms` (agentbox/discovery stamps the `ontology_propose` event), else the proposal row's `created_at` | `enrichment_proposals` |
| 2 queued | `proposal_json.queued_at_ms`, else `created_at` (the row is the queue entry) | `enrichment_proposals` |
| 3 broker_decision | `decided_at_ms` | `enrichment_decisions` |
| 4 merged_enrichment | **new** `writeback_committed_at_ms` (stamped when the fenced Oxigraph `:summary` write lands) | `enrichment_decisions` |
| 5 amplification | — | labelled `planned` (never a value) |

Mesh Velocity per closed loop = `merged_at − propose_at`.

| File | Change |
|---|---|
| `src/adapters/sqlite_enrichment_repository.rs` | `enrichment_decisions` gains `writeback_committed_at_ms INTEGER` (in `CREATE_SCHEMA` for fresh DBs + an idempotent `apply_additive_migrations` guarded `ALTER TABLE` for existing ones). `mark_writeback_committed` now takes `committed_at_ms` and writes it via `COALESCE(…, ?3)` so a re-mark never rewrites the first-commit instant. New reads `loop_traces(limit)` / `loop_trace_for(case_id)` join a proposal to its **terminal** decision (`MAX(decided_at_ms)`) into a `LoopTraceRow`. New `provenance_decisions_since` (feeds REC-11). |
| `src/services/insight_loop.rs` (new) | Pure five-stage assembler: `build_trace(&LoopTraceRow) → InsightLoopTrace` (five ordered `LoopStage`s, `loop_closed`, `mesh_velocity_ms`, `time_to_decision_ms`, `monotonic`) and `summarise(&[…]) → InsightLoopSummary` (aggregate Mesh Velocity over closed loops). A rejected/unattributed proposal marks the merge stage `not_applicable`, not a stuck `pending`; amplification is always `planned`. |
| `src/handlers/insight_loop_handler.rs` (new) | `GET /api/insight-loop/trace` (batch + aggregate Mesh Velocity) and `GET /api/insight-loop/trace/{case_id}`. Fires `CANARY-VC-REC10-LOOP` on a closed, monotonic loop (observed traffic, not a synthetic probe). |
| `src/handlers/enrichment_proposals_handler.rs` | The write-back commit path stamps the stage-4 instant (`mark_writeback_committed(&case_id, &activity_urn, now_ms())`). |
| `src/services/liveness_harness.rs` | `CANARY_REC10_LOOP` const + seeded in `P2_CANARIES`. |
| `src/services/kpi_compute.rs` (comment) | REC-4's Mesh Velocity tile already names REC-10 as its source (`ontology_propose → broker decision → merged enrichment`); this item supplies that source. |
| `src/{services,handlers}/mod.rs`, `src/main.rs` | Module + route registration under `/api`. |

## Falsification (PRD-023 WP-12) — how this survives it

The WP-12 falsification statement attacks REC-10 being *re-scoped as new work
rather than closed end to end*. The receipt below shows one insight closing the
loop across the real store with **monotonic** stage timestamps and a **computed**
Mesh Velocity — not a value asserted:

- *"REC-10 closes without a loop timestamped at each stage"* → each stage carries
  a persisted instant; the merged stage's `writeback_committed_at_ms` is the new
  column, set at commit time.
- *"Mesh Velocity is not computable"* → `mesh_velocity_ms = merged − propose` is
  returned per closed loop and aggregated in the summary; an open loop reports
  `None` (honest), never a fabricated figure.
- *"amplification is claimed built"* → stage 5 is `status: "planned"` with the
  detail `v1 scope: capture→queue→decide→merge; amplification is planned`.
- *"timestamps not monotonic"* → the assembler computes `monotonic` over the
  completed stages and the tests assert propose ≤ queued ≤ decided ≤ merged both
  at the store level and through the service.

## Receipts

Base SHA `774ffa05e`; UTC `2026-07-08T16:25:59Z`.

Targeted unit tests (loop assembler + store join):

```
$ cargo test --lib -- insight_loop provenance_trace sqlite_enrichment_repository sqlite_kpi_repository kpi_compute
running 29 tests
test services::insight_loop::tests::closed_loop_has_five_stages_monotonic_and_a_velocity ... ok
test services::insight_loop::tests::rejection_marks_merge_not_applicable_not_pending ... ok
test services::insight_loop::tests::pending_proposal_has_pending_decision_and_no_velocity ... ok
test services::insight_loop::tests::propose_instant_falls_back_to_created_at_when_body_unstamped ... ok
test services::insight_loop::tests::summary_means_velocity_over_closed_loops_only ... ok
test adapters::sqlite_enrichment_repository::tests::loop_trace_joins_proposal_to_terminal_decision_with_monotonic_stamps ... ok
test adapters::sqlite_enrichment_repository::tests::loop_trace_for_pending_proposal_has_no_decision ... ok
test adapters::sqlite_enrichment_repository::tests::mark_writeback_committed_flips_truth_bit ... ok
...
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 779 filtered out; finished in 0.03s
```

End-to-end fixture (real SQLite store + the insight-loop service):

```
$ cargo test --test rec10_rec11_data_moat_test rec10_insight_loop_closes_end_to_end_with_monotonic_stamps
running 1 test
test rec10_insight_loop_closes_end_to_end_with_monotonic_stamps ... ok
test result: ok. 1 passed; 0 failed; ...
```

The fixture seeds `proposed_at_ms=…000`, `queued_at_ms=…001`, `decided_at_ms=…002`,
commits the write-back at `…003`, then asserts five stages, `loop_closed`,
`monotonic`, `mesh_velocity_ms == 3_000`, and stage 5 = `planned`.

Binary compiles with the new routes registered:

```
$ cargo check --bin visionclaw-server
    Finished `dev` profile [optimized + debuginfo] target(s) in 38.70s
```

## Adversary re-run

```
cargo test --lib -- insight_loop
cargo test --test rec10_rec11_data_moat_test rec10_insight_loop_closes_end_to_end_with_monotonic_stamps
```

Both must report `ok`. The loop trace is a live read (`GET /api/insight-loop/trace`);
`CANARY-VC-REC10-LOOP` fires only when a closed monotonic loop is observed on that
read, never from a probe.
