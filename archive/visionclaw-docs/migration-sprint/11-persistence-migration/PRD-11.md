# PRD-11 — Persistence Strategy Migration

## 1. Capability statement

VisionClaw's durable state — ontology, knowledge-graph topology, inferred
axioms, per-user settings, and physics profiles — is held in **two
embedded stores**: an Oxigraph RDF dataset (SPARQL 1.1 + Update) for
graph and ontology data, and a SQLite database for settings. Both run
**in-process inside the `webxr` Rust binary**, share its filesystem,
and have no external service dependency. Neo4j is removed from the
deployment surface entirely.

## 2. Why this exists

The baseline at `41979d33e` and `main` both depend on a separately-deployed
Neo4j Community Edition container reached over Bolt. Across the 371-commit
delta this dependency has accumulated nine kinds of cost:

- **Operational**: a JVM service co-deployed with a Rust binary.
  Memory budget is doubled (heap + native), startup is ordered
  (Neo4j must be ready before `webxr` can bind its actors), and
  container size grows by ~600 MB for a feature set we use ~5% of.
- **Licensing fragility**: Neo4j Community is GPLv3. Embedding it as
  an in-process library is incompatible with our deployment posture.
  We have therefore always run it as a remote service over a network
  protocol, which adds latency without buying isolation that matters
  for our scale.
- **Schema drift**: Cypher migrations 0042–0045 spent significant
  effort reconciling a label rename (`OwlClass` → `OntologyClass`)
  because the historical writer wrote one label while the BRIDGE_TO
  MERGE-code read another, silently creating orphan stubs. RDF
  has no labels — only types as triples — so this entire class of
  drift is structurally impossible in the destination model.
- **Test isolation**: Integration tests against Neo4j require a
  testcontainer per run. Oxigraph opens a fresh dataset in
  `tempfile::tempdir()` in milliseconds, so test setup cost drops
  by ~2 orders of magnitude.
- **Reasoning round-trip**: `whelk-rs` already takes its input as
  OWL axioms and emits inferred axioms. Currently we translate
  Neo4j rows → OWL axioms → whelk → inferred axioms → Cypher
  MERGEs. With Oxigraph the inferred axioms ARE triples; the
  round-trip collapses to two SPARQL Update statements.
- **Query expressiveness**: SPARQL 1.1 with property paths
  (`rdfs:subClassOf+`, `(owl:imports|skos:broader)*`) is a closer
  fit to the ontology queries we already write in Cypher with
  variable-length match patterns. The translation is line-for-line
  in most cases.
- **Settings/graph coupling**: Settings ended up in Neo4j because
  Neo4j was already there. They are key-value documents with no
  graph structure, and they pay all of Neo4j's cost for none of
  its benefit. SQLite is the correct shape.
- **Per-user settings**: Per-Nostr-pubkey settings naturally key by
  pubkey hex. A SQLite `PRIMARY KEY (key, owner_pubkey)` is the
  right primary key. In Neo4j this was modelled as `:User
  {pubkey}` nodes with `[:OWNS]` relationships to `:Setting`
  nodes, which is graph-shaped overkill for a flat document.
- **Backup**: a tar of the on-disk Oxigraph dataset directory plus
  a `VACUUM INTO` of the SQLite file is the entire backup surface.
  No `neo4j-admin dump`, no transaction-log checkpointing.

## 3. Users and use cases

- **Operator deploying VisionClaw** wants a single Rust binary plus
  a data directory. No companion services. No external ports.
- **Developer running the test suite** wants persistence tests to
  start in <100 ms with no Docker.
- **Researcher loading a snapshot from another instance** wants to
  copy a directory (Oxigraph) and a file (SQLite) and have the
  exact same state.
- **Site administrator restoring from backup** wants `tar -xf`
  followed by binary restart. No multi-step recovery procedure.
- **Whelk inference loop** wants to read the current ontology,
  compute the EL closure, write the inferred axioms back, and
  have client SPARQL queries see them on the next read with no
  manual reload.

## 4. Acceptance criteria

A1. **Single-binary deployment.** The `webxr` binary, given a
    `--data-dir <path>` argument, opens (or creates) an Oxigraph
    dataset at `<path>/oxigraph/` and a SQLite database at
    `<path>/settings.sqlite3` and reaches steady-state without
    requiring any other process or container.

A2. **Port parity.** The current `OntologyRepository`,
    `GraphRepository`, and `SettingsRepository` trait surfaces
    (a combined 108+ methods) are implemented unchanged by the
    new adapters. No upstream caller — actor, handler, or
    service — changes by a single line as a consequence of the
    persistence swap.

A3. **Named-graph segregation.** The Oxigraph dataset uses three
    named graphs distinguished by IRI:
    - `<urn:visionclaw:graph:knowledge>` — KGNode and KGEdge triples
    - `<urn:visionclaw:graph:ontology>` — OntologyClass, OwlProperty,
       OwlAxiom triples
    - `<urn:visionclaw:graph:agent>` — agent telemetry triples
    Cross-graph BRIDGE_TO edges live in the default graph and are
    written as quads referencing the three named graphs explicitly.

A4. **Cypher migration parity.** Migrations 0042–0045 have a
    one-to-one SPARQL Update equivalent under
    `queries/migrations/sparql/`. Applying the SPARQL migrations
    against an Oxigraph dataset bulk-loaded from the Neo4j
    export produces a triple count and a relationship-equivalent
    count within ±0 of the post-Cypher Neo4j state on the same
    fixture.

A5. **Per-user settings.** A setting written by user
    `npub1...` (hex `0x4a8e...`) is invisible to other pubkeys
    and to the global (NULL owner_pubkey) namespace. Global
    settings are read by the application as a base layer; user
    settings override per key. The SettingsRepository contract
    surfaces this through unchanged method signatures — the
    pubkey is sourced from the per-request auth context, not
    from a new method parameter.

A6. **Whelk inference materialisation.** After `WhelkInferenceEngine`
    completes a closure pass, the inferred `SubClassOf` axioms are
    materialised as triples in a sub-graph
    `<urn:visionclaw:graph:ontology:inferred>` so that:
    - asserted vs inferred can be distinguished by named graph
    - inference can be invalidated by `CLEAR GRAPH <...:inferred>`
      without touching authored data
    - SPARQL queries over the union (default behaviour) see both

A7. **One-shot migration tool.** A binary `migrate-neo4j-to-oxigraph`
    (located at `tools/migrate-neo4j-to-oxigraph/`) takes a Neo4j
    Bolt URL and an Oxigraph data directory, exports all nodes and
    relationships as RDF, and loads them. Re-running the tool
    against the same target directory is idempotent (it replaces
    the dataset). The migration is one-way; no dual-write is
    supported in the destination architecture.

A8. **Backup and restore.** The procedure documented in
    `docs/operations/backup-restore.md` consists of two steps:
    1. `tar -czf backup.tar.gz <data-dir>/oxigraph/`
    2. `sqlite3 <data-dir>/settings.sqlite3 'VACUUM INTO "backup.sqlite3"'`
    Restoring is the inverse. No transaction-log replay is required
    because Oxigraph fsyncs on commit by default (RocksDB WAL) and
    SQLite is in WAL mode.

A9. **Performance ceiling documented.** PRD-11 §7 records the
    measured ceiling at which Oxigraph or SQLite stops being the
    right choice. At point of writing this is documented as
    O(100M) triples for Oxigraph (vendor sweet spot) and
    O(10⁵) settings rows for SQLite (effectively never reached).
    Our working set (~4.5k nodes, ~12k edges, ~10k ontology
    axioms, ~200 settings) is 3–4 orders of magnitude below
    these ceilings.

A10. **No live dual-running in destination.** The implementation
     phase may run Neo4j and Oxigraph side-by-side temporarily for
     correctness comparison. The destination architecture does
     not. Once the cutover is committed, the Neo4j adapter files
     under `src/adapters/neo4j_*.rs` are deleted, not feature-
     flagged.

## 5. Non-goals

- **Multi-writer.** Both stores are single-writer. The `webxr`
  binary is the sole writer. Horizontal scale-out is not a goal;
  if it becomes one, the answer is application-level sharding,
  not a different store.
- **Replication.** Operators wanting hot-standby take periodic
  backups (A8). Streaming replication is out of scope for this
  sprint.
- **Full OWL DL reasoning.** `whelk-rs` supports OWL EL only.
  DL-shaped axioms in user input are ignored at inference time
  with a warning, exactly as today.
- **SHACL validation.** SHACL is the right tool for the
  constraints that Neo4j currently expresses via `CREATE
  CONSTRAINT ... UNIQUE`. We will translate the uniqueness
  constraints into pre-write validation in the adapter rather
  than running a SHACL processor inline.
- **GraphQL endpoint.** Out of scope. The HTTP surface stays as
  REST over the existing handler layer.
- **Settings encryption at rest.** SQLite supports SEE/SQLCipher
  via build features. We do not adopt either; the data directory
  is protected at the filesystem layer by the deployment.

## 6. Acceptance evidence to gather during implementation

- Triple-count parity table: Neo4j node + relationship counts
  vs Oxigraph triple counts post-migration, grouped by named
  graph and predicate.
- SPARQL benchmark: each of the 32 hot Cypher queries in
  `src/adapters/neo4j_*.rs` translated and run against the
  loaded Oxigraph dataset; latency p50 + p99 in two tables
  (cold, warm). Acceptable budget: p99 ≤ 50 ms for any
  single-graph query at our scale.
- Whelk round-trip latency: time from `start_inference()` to
  `inferred_axioms_visible_to_sparql()` on the full ontology
  fixture. Target: <2 s.
- Snapshot size: on-disk size of the migrated Oxigraph dataset
  vs the source Neo4j dataset, both with `Vacuum` /
  `db.checkpoint()` applied. Information for capacity
  planning, not a target.
- Test suite duration delta: persistence-integration test
  module wall-clock before vs after. Target: ≥ 10×
  improvement (Neo4j testcontainer eliminated).

## 7. Performance budget and ceiling

| Metric                          | Working set | Documented ceiling     | Headroom |
|---------------------------------|-------------|------------------------|----------|
| Oxigraph triple count           | ~60k        | ~100M (vendor)         | 1600×    |
| Oxigraph dataset size on disk   | ~10 MB      | ~10 GB before tuning   | 1000×    |
| SPARQL query latency (p99)      | <10 ms      | 50 ms (budget)         | 5×       |
| SPARQL Update latency (p99)     | <5 ms       | 25 ms                  | 5×       |
| Whelk inference (full ontology) | <2 s        | 30 s                   | 15×      |
| SQLite settings rows            | ~200        | 10⁵ before VACUUM cost | 500×     |
| SQLite query latency (p99)      | <1 ms       | 5 ms                   | 5×       |
| Backup snapshot duration        | <1 s        | 10 s                   | 10×      |

The 100M-triple ceiling on Oxigraph is the upstream-documented
working range. Beyond it, vendors generally suggest an external
quad-store (Stardog, GraphDB, Apache Jena Fuseki). We are nowhere
near; this PRD does not entertain the question further.

## 8. Out-of-scope smells flagged for ADR review

Several aspects of the current Neo4j integration suggest underlying
fragility that the migration must not preserve:

- `neo4j_adapter.rs` (1,484 lines) contains a connection pool,
  retry loop, and BoltType ↔ Rust type conversion for every
  property. With Oxigraph the conversion is to/from RDF Term and
  is centralised in a single `triple_serde.rs` module.
- `neo4j_settings_repository.rs` (1,137 lines) implements 44
  methods. ~80% of this volume is Cypher string construction and
  parameter binding. The SQLite equivalent uses prepared
  statements bound once at adapter construction; expect a 5–6×
  reduction.
- The "label silently created an orphan stub" bug fixed by
  migration 0045 is the canonical example of a Neo4j-shaped
  failure mode that cannot occur in RDF. ADR-11 must commit to
  the structural fix (named graphs + typed IRIs), not to
  porting the orphan-cleanup logic.
- `cbac7532a fix: merge GPU positions with Neo4j on incremental
  graph upload` (referenced from PRD-01 §7) indicates positions
  were being persisted to Neo4j. The new architecture writes
  positions to RAM (GraphStateActor) and only periodically
  snapshots to Oxigraph as `:hasX/:hasY/:hasZ` literal triples
  via the `update_positions` port method. Live physics positions
  are never round-tripped through the disk store.

## 9. Risk classification

The migration is a Section-11-tagged P0-data event. Each risk
below is rated by probability × impact and mitigated explicitly:

| ID  | Risk                                                           | P  | I  | Mitigation                                                             |
|-----|----------------------------------------------------------------|----|----|------------------------------------------------------------------------|
| R1  | Cypher → SPARQL translation drops a property                   | M  | H  | Triple-count parity test (A4); per-predicate parity in migration tool. |
| R2  | Oxigraph upstream bug at our usage corner                      | M  | M  | Pin version; expose escape hatch to Apache Jena via adapter swap.      |
| R3  | SQLite WAL corruption on network filesystem                    | L  | H  | Startup check rejects non-local FS; operator doc warns explicitly.     |
| R4  | Per-user pubkey lost on background path → writes to global     | M  | H  | Debug-build assert; explicit `as_global()` adapter view.               |
| R5  | Whelk inferred axioms grow without bound                       | L  | M  | `CLEAR GRAPH <…:inferred>` before each rewrite; bounded by ontology.   |
| R6  | Backup tar of live RocksDB inconsistent                        | M  | M  | Document checkpoint API path; or stop writer for tar route.            |
| R7  | Migration tool fails halfway                                   | M  | M  | Tool writes to fresh tempdir then atomic rename; on failure leave src. |
| R8  | Position snapshot triple churn dominates write load            | L  | L  | 60s snapshot cadence at 5k nodes = ≤2k Updates/min; well-budgeted.     |
| R9  | Neo4j adapter files retained "just in case" cause drift        | M  | M  | A10 mandates deletion at cutover; CI fails on `neo4j_*` re-introduction. |
| R10 | SPARQL queries slower at p99 than Cypher equivalents           | L  | M  | Performance gates in CI; fallback is re-translation with hand-tuned indexes. |

## 10. Dependencies on other sections

- **Section 1 (GPU Physics)**: defines `update_positions` call
  cadence and the cold-start position-load contract. Persistence
  consumes this without negotiation.
- **Section 2 (Binary Protocol)**: not impacted. The wire protocol
  carries positions from RAM, never from disk.
- **Section 5 (Settings & Control Panel)**: depends on PRD-11 §3
  user-context + ADR-11 D5 schema. Section 5 ADR references this
  ADR for the storage shape.
- **Section 6 (Auth & Security)**: provides
  `current_owner_pubkey()` task-local. PRD-11 depends on Section
  6's NIP-98 middleware to populate it.
- **Section 8 (Ontology & Graph Data)**: depends on PRD-11 §3
  named-graph layout for the ontology and inferred sub-graphs.
  Section 8 ADR references ADR-11 D2 for the structural fix to
  the migration 0045 orphan-stub class of bug.
- **Section 9 (Ecosystem Services & Launch)**: depends on PRD-11
  §3 single-binary deployment + ADR-11 D10 backup procedure.
  Section 9's launch script removes Neo4j from the compose file
  as part of the cutover.

## 11. Cutover plan summary

Detailed plan in ADR-11 §"Implementation order". Summary:

1. Land empty `oxigraph_*` adapters compiling against existing
   trait surfaces. CI green.
2. Land read-path implementations against a Turtle fixture.
3. Land write-path implementations with constraint ASK guards.
4. Land position snapshot path.
5. Land whelk materialisation path.
6. Land migration tool. Run on a copy of production Neo4j. Verify
   parity table.
7. Land SPARQL migrations 0042–0045.
8. Cutover commit: wire actor system to `oxigraph_*` adapters,
   delete `neo4j_*` adapters, remove `neo4rs` from Cargo.toml,
   remove Neo4j from compose, document new `--data-dir`.
9. Land backup-restore documentation in `docs/operations/`.

No dual-running survives commit 8. The Neo4j container is
removed from the destination compose file in the same PR that
deletes the adapters.
