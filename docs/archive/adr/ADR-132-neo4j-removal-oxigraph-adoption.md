# ADR-132 — Neo4j removal; Oxigraph + SQLite as the sole graph substrate

**Status:** Accepted — retroactive record 2026-07-22. The originating decision
that narrates the Neo4j → Oxigraph+SQLite migration, written to close the
governance gap the doc-drift audit found (§1e-2, claim-10): the removal was
executed in code in May 2026 but never had a governing ADR of its own, so every
"Neo4j is fully removed" claim in the docs was undercited to the auth ADR
(`ADR-011-auth-enforcement.md`) via the `ADR-11` (= Phase 11) shorthand collision.
**Date:** 2026-05-20 (cutover landed) · recorded 2026-07-22
**Decision-type:** Architecture (persistence keystone)
**Relates:** ADR-101 (triple-store migration framework — the versioning regime
built on top of Oxigraph), ADR-098/099/100 (IRI/provenance migrations that run
against Oxigraph), PRD-018 (the migration-sprint parent), ADR-050/051/066 (pod-
backed KGNode schema on the RDF store)

---

## 1. Context

VisionClaw's graph was originally persisted in **Neo4j** (Bolt + Cypher, an
external database server with a browser UI). Multiple design documents described
Neo4j as the live source of truth. Two forces made it the wrong substrate:

- The product's graph model is **RDF/OWL** (Whelk EL++ reasoning, `owl:Class`
  hierarchies, SHACL/PROV-O). Cypher is an impedance mismatch against W3C SPARQL
  1.1; every reasoning and provenance operation had to be re-expressed.
- Neo4j is an **external server**: a separate container, network endpoint,
  credentials (`NEO4J_*`), and Bolt port — operational weight the standalone-first
  posture (PRD-018) rejects.

The migration ran as a dedicated sprint (git archaeology):
- `b8300d94c` (2026-05-16) — migration-sprint docs, radical-rollback → main plan.
- `ae009fd49` (2026-05-16) — persistence-ports audit + adapter scaffold + parity
  test harness (hexagonal ports so the store is swappable behind a trait).
- `dcc204ff4` (2026-05-20) — **remove Neo4j adapter layer and dead utils**
  (`git log --diff-filter=D` confirms `src/adapters/neo4j*` deleted here).
- `676bb3b98` (2026-05-20) — **Neo4j→Oxigraph cutover** + solid-pod-rs.
- `61aac3e3e` (2026-05-20) — Oxigraph-only backend, expanded physics, Solid proxy.
- `9e92d216a` (2026-05-27) — T4: purge dead Neo4j config rot (`NEO4J_*` env).
- `abd2a9bd9` — docs: Neo4j→Oxigraph + webxr→visionclaw alignment.

## 2. Decision

Adopt **embedded Oxigraph (RocksDB-backed, SPARQL 1.1 Query+Update) as the sole
primary graph store**, with **SQLite** for non-triple state (settings, KPI
snapshots, enrichment audit trail). **Remove Neo4j entirely** — the adapter
layer, the Bolt dependency, and all `NEO4J_*` configuration.

- The graph is RDF at `${DATA_DIR}/oxigraph/`; in-process, no external server, no
  Bolt port, no Cypher engine, no database browser UI.
- Live code: `src/adapters/oxigraph_graph_repository.rs` (graph),
  `crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs` (ontology,
  named graphs). `Cargo.toml`: `oxigraph = { version = "0.4" }`,
  `persistence-oxigraph` in the default feature set; **no `neo4rs`/Bolt dependency
  exists**.
- Structural graph changes are versioned through the migration framework of
  **ADR-101** (`sparql_migrations.rs`, `migrations/sparql/`), the discipline that
  Oxigraph lacked and Neo4j never had.
- Non-triple state uses the SQLite repository pattern
  (`sqlite_settings_repository.rs`, `sqlite_kpi_repository.rs`,
  `sqlite_enrichment_repository.rs`).

## 3. Consequences

**Positive** — one embedded store, zero external DB ops; the graph model and the
storage model finally agree (RDF/OWL over SPARQL); provenance/IRI/consistency
migrations (ADR-098/099/100) run natively; standalone-first honoured.

**Negative** — RocksDB-backed Oxigraph has no built-in point-in-time backup UI;
backup of the SQLite stores + the Oxigraph dataset dir is an operator workstream
(tracked under ADR-131, not this ADR). SPARQL string-building was an injection
surface until ADR-101's parameterised migrations landed.

**Neutral** — historical documents naming Neo4j describe a removed state; they are
corrected by dated banners (append-only) and re-pointed at this ADR rather than
the `ADR-011` auth file. This ADR is the canonical governing record for the
removal claim in `docs/README.md`, `docs/reference/graph-schema.md`, and
`docs/reference/configuration.md`.

## 4. Standing trap (grep-truth)
Neo4j-dependent code survives only OUTSIDE `main`: the unmerged `crashbug` branch
(`neo4j_broker_adapter.rs`) and `_archive/2026-07-10/*` worktrees. Their deletion
(audit §2 C8/C9) is what makes `grep -ri neo4j` on a clean checkout match this
ADR's "fully removed" claim; until then the archives are the only reason an agent
still "finds" Neo4j.
