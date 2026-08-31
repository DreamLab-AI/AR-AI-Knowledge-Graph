---
id: ADR-2004
title: Embedded Oxigraph plus per-writer SQLite is the sole persistence substrate
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
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
