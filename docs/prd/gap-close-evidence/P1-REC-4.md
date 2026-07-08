# P1 Evidence — REC-4: Four-KPI dashboard (ADR-043 resurrection)

**Item:** REC-4 (PRD-023 WP-8, ADR-130 Decision 5) · **Wave:** P1 · **Canary:** `CANARY-VC-REC4-KPI` (standing)
**Base SHA at verification:** `e0f582403` (working tree; committed under the gap-close/2026-07 commit that lands this file)
**Verified:** 2026-07-08T14:00Z

## What was built

ADR-043 specified four KPIs on a Neo4j store the codebase does not run. ADR-130 Decision 5
re-targets storage to SQLite + computes two KPIs from existing sources. This commit lands:

- **`src/adapters/sqlite_kpi_repository.rs`** — a SQLite metrics store (`data/kpi.sqlite3`)
  following the `SqliteCanaryRepository`/`SqliteEnrichmentRepository` idiom verbatim. Three
  tables: `kpi_agent_events` (agent-action volume, fed by a passive hub tap), `kpi_snapshots`
  (persisted point-in-time values), `kpi_lineage` (the `DERIVED_FROM` model re-expressed
  relationally — a value's contributing source events, queryable).
- **`src/services/kpi_compute.rs`** — `KpiComputeService` + pure compute functions:
  - **Augmentation Ratio** = agent-action volume (`/wss/agent-events` window count) ÷ ACSP
    escalation volume (`enrichment_decisions` window count).
  - **Trust Variance** = normalised Gini-Simpson dispersion (`1 − Σ pᵢ²`) of decision
    outcomes over the 30-day rolling window.
  - `run_agent_event_tap` subscribes to the existing `agent_events::hub` (the same seam the
    render actor uses) — no new emit-site instrumentation (ADR-130 D5).
  - Each compute persists snapshots with lineage and fires `CANARY-VC-REC4-KPI`.
- **`src/handlers/kpi_handler.rs`** — `GET /api/kpi/summary` (compute + persist + return the
  four tiles) and `GET /api/kpi/lineage/{snapshot_id}` (the queryable `DERIVED_FROM` trail).
- **Client four-tile panel** — `client/src/features/control-center/kpi/{kpiSummary.ts,useKpiSummary.ts,KpiPanel.tsx}`,
  mounted in `MainLayout.tsx`. Augmentation Ratio + Trust Variance render value + confidence;
  **Mesh Velocity + HITL Precision render "awaiting data source" with the source named, never
  a fake number** (the honesty rule, enforced in `normaliseKpiTile` and unit-tested).
- **ADR-043 status header** updated to "Resurrection in progress" citing ADR-130 D5.

## Acceptance criteria (PRD-023 WP-8)

| # | Criterion | Status |
|---|---|---|
| 1 | At least one KPI computes from real source events and persists a snapshot | **Met** — Trust Variance computes from `enrichment_decisions`; Augmentation Ratio from agent-event volume ÷ decisions; both persist via `insert_snapshot_with_lineage` (test `snapshot_persists_with_queryable_lineage`) |
| 2 | A control-centre panel renders the computed KPI with its confidence, pushed over the existing pattern | **Met** — `KpiPanel` renders value + confidence%; `useKpiSummary` fetches `/api/kpi/summary` via `unifiedApiClient` (the broker-queue pattern) |
| 3 | Lineage from a KPI value to its contributing decision events is queryable | **Met** — `kpi_lineage` rows + `GET /api/kpi/lineage/{id}` (test asserts both source contributions traceable) |

## Falsification statement and how this survives it

> *WP-8 is falsified if the dashboard displays a KPI not computed from live source events, if
> the ADR-043 Neo4j assumption ships unchanged against a non-existent store, or if REC-4 closes
> with no snapshot traceable to its source events.*

- The two live tiles are computed by `compute_and_persist` from `kpi_agent_events` +
  `enrichment_decisions` — real rows, not constants. The two non-computable KPIs render
  `awaiting_data_source` with a named source, never a fabricated value.
- Storage is SQLite (`data/kpi.sqlite3`) + relational lineage — the Neo4j assumption is
  superseded (ADR-043 header, ADR-130 D5); no Neo4j code ships.
- Every snapshot writes its lineage in the SAME transaction (`insert_snapshot_with_lineage`),
  so a stored value is never without its `DERIVED_FROM` trail.

## Execution receipts

### Rust unit tests — KPI computation against fixture rows

```
$ cargo test --lib -- agent_events::schema agent_events::ingest kpi_compute sqlite_kpi_repository sqlite_enrichment_repository
running 27 tests
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 751 filtered out; finished in 0.01s
```

KPI-specific tests:
- `kpi_compute::tests::augmentation_ratio_divides_volume_by_escalations` — `(42, 12) → 3.5`, full confidence
- `kpi_compute::tests::augmentation_ratio_zero_escalations_is_zero_confidence`
- `kpi_compute::tests::augmentation_ratio_low_sample_scales_confidence` — `(3, 3) → 1.0`, conf 0.2
- `kpi_compute::tests::trust_variance_uniform_outcome_is_zero`
- `kpi_compute::tests::trust_variance_even_split_is_maximal` — 50/50 → normalised 1.0
- `kpi_compute::tests::trust_variance_three_way_even_is_maximal`
- `kpi_compute::tests::trust_variance_skewed_is_between_zero_and_one`
- `kpi_compute::tests::trust_variance_empty_is_zero`
- `sqlite_kpi_repository::tests::agent_event_volume_window_count`
- `sqlite_kpi_repository::tests::snapshot_persists_with_queryable_lineage`
- `sqlite_enrichment_repository::tests::decisions_since_windows_on_decided_at`

### Vitest — panel logic

```
$ npx vitest run src/features/control-center/kpi/__tests__/kpiSummary.test.ts
 ✓ src/features/control-center/kpi/__tests__/kpiSummary.test.ts (6 tests) 8ms
 Test Files  1 passed (1)
      Tests  6 passed (6)
```

Covers: envelope unwrapping (StandardResponse + bare), value formatting, computed-tile
rendering, **awaiting-tile renders "awaiting data source" with source named and NO value**,
computed-without-value degrades to awaiting, four-tile mapping (2 computed / 2 awaiting).

### Client typecheck

```
$ npx tsc --noEmit    # 0 errors total
```

## Files

- `src/adapters/sqlite_kpi_repository.rs`, `src/adapters/mod.rs` (exports)
- `src/adapters/sqlite_enrichment_repository.rs` (`decisions_since` window read + test)
- `src/services/kpi_compute.rs`, `src/services/mod.rs`
- `src/handlers/kpi_handler.rs`, `src/handlers/mod.rs`
- `src/app_state.rs` (KPI repo + service fields), `src/main.rs` (routes, app_data, tap spawn)
- `src/services/liveness_harness.rs` (`CANARY_REC4_KPI` + P1 seed)
- `client/src/features/control-center/kpi/{kpiSummary.ts,useKpiSummary.ts,KpiPanel.tsx,__tests__/kpiSummary.test.ts}`
- `client/src/app/MainLayout.tsx` (mount)
- `docs/adr/ADR-043-kpi-lineage-model.md` (status header)
