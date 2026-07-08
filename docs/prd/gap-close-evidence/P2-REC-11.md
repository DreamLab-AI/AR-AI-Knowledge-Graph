# P2 evidence — REC-11 data-moat consolidation (PRD-023 WP-12)

**Item:** One queryable trace joining agent-events, hook/trajectory records
(agentbox emits CTC fields since P1), broker decisions, and pod git-marks (the
solid-pod ADR-060 contract shape). The pod side is default-off; the join
**tolerates absent sources and reports which were present**. Implemented as a
**query layer** (`GET /api/trace`), not a new store, per ADR-130.
**Base commit verified against:** `774ffa05e` (`gap-close/2026-07`)
**Contract consumed:** `solid-pod-rs/crates/solid-pod-rs/docs/reference/provenance-trace-contract.md`
(the P1 agent authored it) — the trace keys on the `did:nostr` `agent_did`
attribution the contract fixes as the shared key (§2.3), and mirrors the
`GET /{pod}/_prov/` `marks[]` element shape.
**Maturity:** `planned` → `integrated` (P2). Two live source kinds join over the
real stores; the pod git-mark source is reported absent (default-off) and
incorporated only when a `--features git` pod supplies marks.
**Canary:** `CANARY-VC-REC11-TRACE` (one-shot, P2) — seeded in `P2_CANARIES`,
fired from `GET /api/trace` when the returned trace joins ≥2 live source kinds.

## What ships

The trace is a **read-time JOIN over stores that already exist** (ADR-130). No
new store: it reads the two live SQLite-backed sources and joins them in the
query layer on the `did:nostr` attribution. The pod source consumes the ADR-060
contract shape and is absent (default-off) in this deployment.

| Source kind | Store | `did:nostr` key | Live here |
|---|---|---|---|
| `agent_event` (agent-events / hook-trajectory, CTC since P1) | `kpi_agent_events` | `agent_did` (from the envelope `pubkey`/`source_urn`) | yes |
| `broker_decision` | `enrichment_decisions` | `owner_did` | yes |
| `pod_git_mark` (solid-pod ADR-060) | pod `_prov` enumeration | `commit.agent_did` | default-off → `sources_absent` |

| File | Change |
|---|---|
| `src/adapters/sqlite_kpi_repository.rs` | `kpi_agent_events` gains nullable identity + CTC columns (`agent_did`, `action_type_name`, `source_urn`, `target_urn`, `handoff_id`, `token_count`, `verification`) — additive (`CREATE_SCHEMA` + idempotent `apply_additive_migrations`), so the KPI volume count is untouched (it counts rows). New `record_agent_trajectory` (superset of `record_agent_event`) and `trajectories_since` for the trace. |
| `src/services/kpi_compute.rs` | The existing hub tap now captures identity + CTC via `record_agent_trajectory`: it derives `did:nostr` from the envelope `pubkey` (x-only hex → `did:nostr:<pubkey>`), falling back to a `did:nostr` `source_urn`. One tap, one durable capture of the `/wss/agent-events` wire feeds both REC-4 volume and REC-11 trace. |
| `src/adapters/sqlite_enrichment_repository.rs` | New `provenance_decisions_since` — carries `owner_did` (did:nostr) + the PROV-O `activity_urn` the trace joins on (distinct from the KPI numerator read). |
| `src/services/provenance_trace.rs` (new) | The join layer. `PodProvenanceMark` mirrors the contract's `marks[]` element with `from_contract_json`. Pure `build_trace(trajectories, decisions, pod_marks, pod_source_available, since_ms) → ProvenanceTrace` normalises all sources to `TraceRecord`, groups by `did:nostr`, and emits a `TraceJoin` for any identity spanning ≥2 distinct source kinds. `ProvenanceTraceService::query` reads both live stores and reports `pod_git_mark` absent. |
| `src/handlers/trace_handler.rs` (new) | `GET /api/trace?agent=<did:nostr>&window_ms=…`. Fires `CANARY-VC-REC11-TRACE` when `joins_multiple_source_kinds()` (observed traffic). |
| `src/services/liveness_harness.rs` | `CANARY_REC11_TRACE` const + seeded in `P2_CANARIES`. |
| `src/{services,handlers}/mod.rs`, `src/main.rs` | Module + route registration. |

## Falsification — how this survives it

The solid-pod contract's own falsification (§5) and WP-12's both apply:

- *"a REC-11 acceptance assumes provenance on a default build (pod git off)"* →
  the trace reports `pod_git_mark` under `sources_absent` when the pod source is
  not supplied; nothing here assumes on-by-default pod provenance. The
  `pod_source_incorporated_when_available` test proves the pod kind joins only
  when marks are actually supplied.
- *"asserted without the did:nostr agent_did attribution the consumers join on"*
  → the join is keyed on `did:nostr` (`agent_did` for trajectories/pod marks,
  `owner_did` for decisions); `PodProvenanceMark::from_contract_json` reads
  `commit.agent_did`.
- *"claimed delivered without a real join"* → `joins_multiple_source_kinds()` is
  true only when one `did:nostr` spans ≥2 distinct source kinds; the integration
  test drives it over the two real SQLite stores. Anonymous records never join.
- *"a new store, contradicting ADR-130"* → no new store; `ProvenanceTraceService`
  reads the existing `kpi.sqlite3` + `enrichment.sqlite3` and joins in Rust.

## Receipts

Base SHA `774ffa05e`; UTC `2026-07-08T16:25:59Z`.

Targeted unit tests (join layer + trajectory store):

```
$ cargo test --lib -- provenance_trace sqlite_kpi_repository sqlite_enrichment_repository
test services::provenance_trace::tests::joins_two_live_source_kinds_under_one_did ... ok
test services::provenance_trace::tests::single_source_does_not_join ... ok
test services::provenance_trace::tests::anonymous_records_never_join ... ok
test services::provenance_trace::tests::pod_source_incorporated_when_available ... ok
test services::provenance_trace::tests::pod_mark_parses_from_contract_json ... ok
test adapters::sqlite_enrichment_repository::tests::provenance_decisions_since_carries_owner_did_and_activity ... ok
test adapters::sqlite_kpi_repository::tests::agent_event_volume_window_count ... ok
...
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 779 filtered out; finished in 0.03s
```

End-to-end fixture (real `kpi.sqlite3` + `enrichment.sqlite3` joined by the service):

```
$ cargo test --test rec10_rec11_data_moat_test rec11_trace_joins_two_live_source_kinds_over_real_stores
running 1 test
test rec11_trace_joins_two_live_source_kinds_over_real_stores ... ok
test result: ok. 1 passed; 0 failed; ...
```

The fixture records one agent-event trajectory (`agent_did = did:nostr:aaaa…`)
and one broker decision (`owner_did = did:nostr:aaaa…`), then asserts:
`sources_present` = {`agent_event`, `broker_decision`}, `sources_absent` =
{`pod_git_mark`}, `max_join_span == 2`, and one `TraceJoin` under the shared
`did:nostr` spanning both source kinds (`record_count == 2`).

Binary compiles with `GET /api/trace` registered:

```
$ cargo check --bin visionclaw-server
    Finished `dev` profile [optimized + debuginfo] target(s) in 38.70s
```

## Adversary re-run

```
cargo test --lib -- provenance_trace
cargo test --test rec10_rec11_data_moat_test rec11_trace_joins_two_live_source_kinds_over_real_stores
```

Both must report `ok`. `GET /api/trace` is a live read that joins two live source
stores; `CANARY-VC-REC11-TRACE` fires only when the returned trace genuinely
joins ≥2 live source kinds under one `did:nostr`.
