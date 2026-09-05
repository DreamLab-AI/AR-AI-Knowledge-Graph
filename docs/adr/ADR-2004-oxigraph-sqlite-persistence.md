---
id: ADR-2004
title: Embedded Oxigraph plus per-writer SQLite is the sole persistence substrate
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: [Cargo.toml, src/app_state.rs]
owner: jjohare
review_trigger: a scale requirement that exceeds a single-node embedded store, or any proposal to reintroduce a networked graph database
repo: visionclaw
domain: BASELINE-architecture
lineage: Distils legacy ADR-132 (Neo4j removal, Oxigraph+SQLite adoption; cutover 2026-05-20) and its ADR-101 versioning regime / ADR-098-100 IRI-provenance migrations.
---

# ADR-2004 — Embedded Oxigraph plus per-writer SQLite is the sole persistence substrate

## Context

The graph/ontology data needs SPARQL 1.1 and durable triples; the non-triple
state (settings, enrichment lifecycle, liveness, KPI) needs a single-writer
transactional store. A networked graph DB (Neo4j) added an operational
dependency, a second query dialect, and a cross-process consistency problem the
deployment does not need. Prior state carried both. Lineage: ADR-132 cutover
(2026-05-20), ADR-101 versioning, ADR-098-100 IRI-provenance migrations.

## Decision

The canonical graph/ontology store is **embedded Oxigraph** (RocksDB-backed,
SPARQL 1.1), opened exactly once at `data/oxigraph` and shared: the graph
repository is derived `from_store(...)` off the same handle the ontology
repository opens. All non-triple state lives in **per-writer SQLite files** under
`DATA_DIR` (`settings`, `enrichment`, `liveness`, `kpi`.sqlite3), one file per
single-writer to keep migration and lock posture isolated. Neo4j and any
external or networked graph database are forbidden.

## Consequences

- No network hop, no second query language, no clustering to operate; the whole
  data plane is process-local and backs up as files.
- The store is bound to one node: horizontal scale-out is foreclosed without a
  new ADR. Oxigraph/RocksDB has no PITR (see ADR-2017 for backup posture).
- Sharing one handle means a corrupt or locked store takes down both
  repositories together — accepted for a single-operator deployment.

## Verification

`Cargo.toml:80` pins `oxigraph = { version = "0.4" }`; no `neo4rs` dependency
anywhere (the only match is a comment asserting its absence). `src/app_state.rs`
opens `OxigraphOntologyRepository::open(&oxigraph_path)` (~:451) and derives
`OxigraphGraphRepository::from_store(...)` (~:456). The four SQLite files are
opened under `data_dir` at ~:459 (settings), ~:472 (enrichment), ~:487
(liveness), plus the KPI store. Verified at `e0f8cd896`.

## Closeout extension — 2026-09-04

**Work package:** CP-01 / CP-06 / CP-08. **Owner:** existing owner above. Dependencies are
CP-01 revision/ownership mapping and the relevant corpus or authority contract.

**Current evidence:** Source confirms AppState opens one Oxigraph handle and shares it between ontology and graph repositories, plus separate SQLite files. This verifies storage composition, not a cross-store transaction or consistent serving generation.

See [runtime analysis](../../../VisionFlow/docs/estate-review/visionclaw-data-runtime.md),
[source hashes](../../../VisionFlow/docs/estate-review/evidence/visionclaw-data-snapshot.json)
and [backup receipt](../../../VisionFlow/docs/estate-review/evidence/visionclaw-backup-probe.json).
Source was inspected at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`. Earlier verification at `4fed5663dfbc0940c6b19a175dfcc8a9c67f2ab8`
remains historical evidence; this annex does not claim a new deployed activation
or complete verification of every older assertion.

**Acceptance still required:** Record per-store commit points, actor reload/generation identity and a restore manifest. Test startup failures and restart against separately restored stores; do not infer consistency from the shared Oxigraph handle.

### Re-verification 2026-09-05 (ADR-2004)

Re-checked at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` after `Cargo.toml` changed
since the previous `verified_commit` (`4fed5663`). `verified_paths` is emptied and
`verified_commit` set to current HEAD for this pass; **both must be restored at the
landing commit** (`verified_paths: [Cargo.toml, src/app_state.rs]` plus that
commit's SHA) so the staleness check regains its teeth.

Claim-by-claim against the Verification section above:

- **Oxigraph pin — line drift, claim holds.** The dependency is at
  **`Cargo.toml:82`**, not `:80`: `oxigraph = { version = "0.4" }`. The version
  constraint is unchanged.
- **No Neo4j — holds.** `grep -n "neo4rs" Cargo.toml` returns nothing; the only
  matches anywhere in the tree remain comments asserting its absence.
- **One shared Oxigraph handle — holds, with exact lines.** `src/app_state.rs:448`
  reads `DATA_DIR` (default `./data`), `:449` joins `oxigraph`, `:451` opens
  `OxigraphOntologyRepository::open(&oxigraph_path)`, and `:456` derives
  `OxigraphGraphRepository::from_store(oxigraph_store)` off that same handle. The
  Decision's "opened exactly once and shared" is exact at this commit.
- **Four per-writer SQLite files — holds; the KPI line is now recorded.** Under
  `data_dir`: `settings.sqlite3` path `:459`, `SqliteSettingsRepository::open`
  `:461`; `enrichment.sqlite3` `:472`, `SqliteEnrichmentRepository::open` `:474`;
  `liveness.sqlite3` `:487`, `SqliteCanaryRepository::open` `:489`; and the KPI
  store, previously cited only as "plus the KPI store", is `kpi.sqlite3` at
  **`:517`** with `SqliteKpiRepository::open` at **`:519`**.
- **New observation, relevant to the storage decision.** `Cargo.toml:250` sets
  `default = ["gpu", "ontology", "persistence-oxigraph", "solid-pod-embed"]`, and
  `persistence-oxigraph` (`:266`) is an empty marker feature — it gates nothing at
  this commit. The Oxigraph dependency and the open calls above are unconditional,
  so the feature name currently documents intent rather than enforcing a choice.
  A future alternative-substrate ADR must either wire this feature or delete it.

The 2026-09-04 closeout extension's limits are unaffected and are retained in full:
this re-verification confirms **storage composition only** — it establishes no
cross-store transaction, no actor reload/generation identity and no restore
correctness, and consistency must not be inferred from the shared handle.

**Re-confirmed after the 2026-09-05 remediation edits to `Cargo.toml`.** ADR-2066
removed the `quinn`/`rustls`/`rcgen` dependencies along with the unwired QUIC
transport server. Every citation above was re-read afterwards and all still land
on the claimed line: `Cargo.toml:82` `oxigraph = { version = "0.4" }`, `:250`
`default = ["gpu", "ontology", "persistence-oxigraph", "solid-pod-embed"]`, `:266`
`persistence-oxigraph = []`, and `src/app_state.rs:448/449/451/456` (`DATA_DIR`
read, `oxigraph` path join, `OxigraphOntologyRepository::open`,
`OxigraphGraphRepository::from_store`). `grep -n "neo4rs" Cargo.toml` remains
empty. The removed dependencies sat below the storage block, so no line shifted.
`node scripts/adr-index-gen.js docs/adr --check` → `ok: 72 ADR(s) valid`.
