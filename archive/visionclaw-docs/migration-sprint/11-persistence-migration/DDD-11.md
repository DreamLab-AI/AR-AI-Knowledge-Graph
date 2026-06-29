# DDD-11 — Persistence Bounded Context

## Bounded context

The **Persistence** bounded context owns durable storage and retrieval
of all data that must survive process restart. It is sovereign over:

- The Oxigraph dataset (RDF quad store, RocksDB-backed) holding
  graph topology, ontology, and inferred axioms.
- The SQLite database holding settings and physics profiles.
- The on-disk layout under `<data-dir>/`.
- SPARQL query and update execution.
- IRI minting for nodes and edges.
- Per-write constraint validation (uniqueness ASK guards).
- Bulk-import migration mechanics from Neo4j to Oxigraph (one-shot).
- Backup snapshot semantics.

It does **not** own:

- Graph topology *as a domain concept* (Section 8 owns Ontology &
  Graph Data — what nodes and edges *mean*). Persistence speaks
  triples, not concepts.
- Live physics positions (Section 1). Persistence holds periodic
  snapshots only; the canonical position state is RAM in
  `GraphStateActor`.
- Broadcast cadence (Section 2). The store does not push.
- Settings semantics (Section 5). Persistence stores SettingValue
  documents; what they mean is upstream.
- Auth (Section 6). The pubkey for per-user settings arrives from
  the auth context; persistence does not verify it.
- Inference rules (Section 8 + `whelk-rs`). Persistence stores
  inference *results*, not the engine.

## Ubiquitous language

| Term                       | Definition                                                                 |
|----------------------------|----------------------------------------------------------------------------|
| **Triple**                 | An RDF statement `(subject, predicate, object)` with IRI or literal terms. |
| **Quad**                   | A triple plus a named-graph IRI: `(subject, predicate, object, graph)`.    |
| **Named graph**            | An IRI that identifies a sub-graph within the dataset. Acts as the "tier" axis (knowledge vs ontology vs agent vs inferred). |
| **Default graph**          | The unnamed graph. Holds cross-graph quads (e.g. BRIDGE_TO) and schema-level triples. |
| **IRI**                    | Internationalised Resource Identifier — the canonical name of every entity. Replaces Neo4j's per-label `id` integer plus property `iri`. |
| **Literal**                | A typed string value (xsd:string, xsd:dateTime, xsd:decimal, …) appearing as the object of a triple. |
| **Blank node**             | An anonymous subject local to a single graph. We avoid them: every persisted entity has a minted IRI. |
| **Dataset**                | The full quad-store on disk. One per VisionClaw instance.                 |
| **Repository trait**       | A Rust trait under `src/ports/*.rs` defining the persistence contract.    |
| **Adapter**                | A concrete `impl Trait for OxigraphFoo` (or `SqliteFoo`) in `src/adapters/`. |
| **Named-graph segregation**| The discipline that knowledge / ontology / agent / inferred axioms live in distinct named graphs and never mix subject-predicate-wise without the named-graph qualifier. |
| **Asserted axiom**         | An axiom authored by a human or ingest pipeline. Lives in `<...:ontology>`. |
| **Inferred axiom**         | An axiom produced by `whelk-rs` from the asserted axioms. Lives in `<...:ontology:inferred>` and is reproducible. |
| **Snapshot triple**        | A position triple `:hasX / :hasY / :hasZ` written periodically by `update_positions`. Persistence-owned; physics-RAM-canonical. |
| **Migration**              | A numbered SPARQL Update file under `queries/migrations/sparql/`, registered as applied in the SQLite `schema_migrations` table. |
| **Owner pubkey**           | A 64-character hex lowercase Nostr public key. Identifies a user for per-user settings. NULL means global / shared. |

## Aggregates

The persistence context contains **two** aggregates because there are
**two stores** and they have independent transactional boundaries.
Cross-store consistency is the application's concern (the
application orchestrates updates that span both), not the store's.

### Aggregate root: `OxigraphDataset`

The dataset is the consistency boundary for all triple operations.

```rust
pub struct OxigraphDataset {
    store: oxigraph::store::Store,         // RocksDB-backed; owns RW
    iri_minter: Arc<IriMinter>,            // pure; no state
    constraints: AdapterConstraintSet,     // see Invariants
}
```

Invariants:

- **Single-writer.** Exactly one `OxigraphDataset` instance per
  process; constructed once at startup. Concurrent reads are safe;
  concurrent writes serialise on RocksDB's column-family lock.
- **IRI uniqueness inside a typed graph.** No two triples
  `(?s, rdf:type, vc:OntologyClass)` exist in
  `<urn:visionclaw:graph:ontology>` with the same `?s`. Enforced by
  pre-write `ASK`.
- **Foreign-IRI integrity for BRIDGE_TO.** Every BRIDGE_TO quad
  `(?k, vc:bridgeTo, ?o, <default>)` must have `?k` typed as
  `vc:KGNode` in `<...:knowledge>` and `?o` typed as
  `vc:OntologyClass` in `<...:ontology>`. Enforced by pre-write
  `ASK`.
- **Inferred axioms are derivable.** No write directly modifies
  `<...:ontology:inferred>` except via
  `store_inference_results(...)`, which clears and rewrites the
  graph atomically inside a single SPARQL Update.
- **Position snapshot atomicity.** A call to `update_positions(N
  updates)` issues exactly one SPARQL Update with N×3 INSERTs +
  preceding DELETEs. Either all positions land or none do.

Operations (high-level, all going through SPARQL Update or `ASK`):

- `load_ontology_graph()` → SPARQL `CONSTRUCT WHERE GRAPH <...:ontology>`
- `save_ontology_graph(graph)` → batched `INSERT DATA` per type
- `add_owl_class(class)` → `ASK { ?s a vc:OntologyClass . FILTER(?s = <iri>) }` then `INSERT DATA`
- `get_owl_class(iri)` → `SELECT … WHERE { GRAPH <...:ontology> { <iri> ?p ?o } }`
- `add_axiom(axiom)` → `INSERT DATA { GRAPH <...:ontology> { <s> <p> <o> } }`
- `store_inference_results(results)` → `DELETE GRAPH <...:inferred>; INSERT DATA GRAPH <...:inferred> { … }` as one Update
- `update_positions(updates)` → one Update with N×3 literal triples (D4)
- `add_nodes(nodes)`, `add_edges(edges)` → batched INSERT per named graph

### Aggregate root: `SqliteSettingsStore`

The SQLite database is the consistency boundary for all settings
operations.

```rust
pub struct SqliteSettingsStore {
    conn: Arc<Mutex<rusqlite::Connection>>, // single conn; serialise writes
    read_pool: r2d2::Pool<SqliteConnectionManager>, // read replicas
    current_owner: tokio::task::JoinHandle<…>,      // task-local owner pubkey
}
```

Invariants:

- **Primary key uniqueness** on `(key, owner_pubkey)`. Enforced by
  SQLite directly.
- **Owner pubkey shape**. The adapter accepts only 64-character
  lowercase hex strings; any other input is rejected at the
  adapter boundary with `SettingsRepositoryError::InvalidValue`.
- **Resolution order**. A read for `(K, U)` returns `(K, U)` if
  present else `(K, NULL)` else `NotFound`. Writes always specify
  the owner explicitly.
- **WAL mode**. `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL`
  applied at adapter construction; required for safe concurrent
  reads alongside writes.
- **JSON-valued column**. The `value` column is a JSON-encoded
  `SettingValue`. Parse failures on read return
  `SettingsRepositoryError::SerializationError`, not silent
  default.

Operations:

- `get_setting(key)` / `set_setting(key, value, desc)` / `delete_setting(key)`
- `get_settings_batch(keys)` / `set_settings_batch(updates)` — both
  in a single transaction
- `list_settings(prefix)` — uses an index range scan on `key`
- `load_all_settings()` / `save_all_settings(settings)` — load
  is a single `SELECT`; save is a transaction-wrapped `INSERT
  OR REPLACE` over the keys in the serialised
  `AppFullSettings`
- `get_physics_settings(profile_name)` / `save_physics_settings(profile_name, settings)`
- `export_settings()` / `import_settings(json)` — full table
  dump / restore for operator use

## Domain events

Persistence is **below the level at which domain events are emitted**.
Domain events live in the application layer (Section 8 for
ontology events, Section 1 for physics events, Section 7 for agent
telemetry events). The persistence adapter neither emits nor
consumes events; it serves trait method calls.

The single conceptual signal that *might* warrant an event is
"backup-snapshot-complete", but backup is operator-initiated and
out-of-process (D10 in ADR-11), so the event is on the operator's
side and not in the running binary.

This is deliberate. Conflating "data was written" with "domain
events should fire" was a recurring smell in the Neo4j
integration — actors observed the write rather than the
domain decision that caused it. Section 8 reasserts the
separation: Section 8 emits `OwlClassAdded`, Section 11
materialises it as a SPARQL `INSERT DATA`.

## Commands accepted (adapter trait surface)

All commands map 1-to-1 onto methods already declared on the three
ports (`OntologyRepository`, `GraphRepository`,
`SettingsRepository`). The persistence context introduces **no new
methods**. This is the entire point of ADR-11's port-stability
discipline: the migration is transparent to callers.

The combined surface:

- **`OntologyRepository`** (40+ methods): load/save ontology
  graph, OWL class CRUD, OWL property CRUD, axiom CRUD, inference
  result CRUD, validation, query, metrics, pathfinding cache.
- **`GraphRepository`** (24+ methods): node CRUD, edge CRUD,
  position updates, dirty-node tracking, graph read, physics
  state read, bots graph read, constraints read, equilibrium
  status, shortest-path computation.
- **`SettingsRepository`** (44+ methods): per-key CRUD, batch
  ops, prefix listing, full-settings load/save, physics profile
  CRUD, export/import, cache control, health check.

## Anti-corruption layer to Section 8 (Ontology & Graph Data)

Section 8 owns the **semantic** model: what an `OwlClass` is, what
predicates exist on it, what a `BRIDGE_TO` edge means, what
"colocated" vs "candidate" vs "promoted" means. Section 11 owns the
**physical** representation: what IRI scheme is used, what named
graph holds what, how triples are laid out, how a SPARQL Update is
formed.

The ACL is the existing port traits. Section 8 calls
`add_owl_class(class)` with an `OwlClass` value; Section 11's
adapter is free to layout the triples however it likes, as long as
a subsequent `get_owl_class(iri)` returns the round-trippable same
`OwlClass`. The trait is the contract.

This separation is what makes the Cypher → SPARQL migration safe:
Section 8's actors don't know they're talking to a triple store, and
the bug-class "label written ≠ label read" (migration 0045) is
prevented structurally by D2.

## Anti-corruption layer to Section 5 (Settings & Control Panel)

Section 5 owns settings *semantics*: what `physics.gravity_strength`
means, what range it accepts, what the default is, what UI hint
applies, how it maps to a `SimParams` field. Section 11 owns the
*storage*: how it's serialised, where it's keyed, how per-user
override resolution works.

The ACL is `SettingsRepository`. Section 5's UI handler calls
`set_setting("physics.gravity_strength", SettingValue::Float(2.0),
Some("..."))`; Section 11's adapter writes a row. The per-user
pubkey context is threaded via task-local storage (D5/R4 in
ADR-11), invisible to Section 5's call sites.

## Anti-corruption layer to Section 1 (GPU Physics)

Physics positions live in RAM. Periodic snapshots — at most once
per 60 seconds of wall-clock — call `update_positions(updates)`
which writes literal triples atomically. The physics actor never
*reads* from persistence on the hot loop; it only *writes*. At
cold start, `get_node_positions()` provides the seed positions for
the new run. After that, physics is RAM-canonical until shutdown.

Section 1 and Section 11 share **no data structures**. The only
contact surface is the `GraphRepository` trait's position-shaped
methods. This is by design — colocating physics state with disk
state was the source of the `cbac7532a` clobbering bug.

## Anti-corruption layer to Section 6 (Auth & Security)

Per-user settings need the current user's pubkey. Section 11's
SQLite adapter reads it from a task-local set by Section 6's
middleware on every authenticated request. Section 11 does not
verify the pubkey; it trusts that if a pubkey is present in
task-local, Section 6 verified it.

The ACL is a single function:
`auth_context::current_owner_pubkey() -> Option<String>`. Defined
by Section 6, called by Section 11's adapter. No data structures
shared.

## Anti-corruption layer to whelk-rs (external library)

`WhelkInferenceEngine` is a trait at `src/ports/inference_engine.rs`
(already exists). Its current impl wraps `whelk-rs` and produces
`OwlAxiom` values. Section 11's adapter consumes those
`OwlAxiom` values via `store_inference_results(InferenceResults)`
and materialises them as triples in the inferred named graph.

`whelk-rs` does not know about Oxigraph, named graphs, IRI schemes,
or our minting rules. The translation from `OwlAxiom` to a triple is
the adapter's responsibility, isolated to ~30 LOC.

## SPARQL translation walkthrough

This section catalogues the four Cypher migrations and their SPARQL
Update equivalents. Migration files live at
`queries/migrations/sparql/0042_bridge_to.rq` etc. Bind-time
templating fills in `xsd:dateTime` literals; the bodies below show
the parameterised shape.

### 0042 — BRIDGE_TO baseline

**Cypher essence (paraphrased):**
- CREATE CONSTRAINT … KGNode.iri UNIQUE, OntologyClass.iri UNIQUE
- CREATE INDEX on r.kind, r.confidence, r.promoted_at on BRIDGE_TO
- CREATE INDEX on node_type, owl_kind, visibility
- MERGE BRIDGE_TO colocated edge for every (KGNode, OntologyClass)
  pair sharing an IRI
- Insert :SchemaMigration node

**SPARQL Update equivalent (sketch):**
```sparql
PREFIX vc:   <https://visionclaw.dreamlab/ns/>
PREFIX xsd:  <http://www.w3.org/2001/XMLSchema#>

# Constraints: enforced in-adapter, not as DDL. The migration
# applies them by *checking* current state.

# Colocation backfill
INSERT {
    GRAPH <urn:visionclaw:graph:default> {
        ?bridgeIri  a                vc:BridgeEdge ;
                    vc:from           ?k ;
                    vc:to             ?o ;
                    vc:kind           "colocated" ;
                    vc:confidence     "1.0"^^xsd:decimal ;
                    vc:createdAt      "2026-05-16T00:00:00Z"^^xsd:dateTime ;
                    vc:createdBy      "system" ;
                    vc:promotedAt     "2026-05-16T00:00:00Z"^^xsd:dateTime .
        ?k          vc:bridgeTo       ?o .
    }
}
WHERE {
    GRAPH <urn:visionclaw:graph:knowledge> { ?k a vc:KGNode . ?k vc:iri ?iri . }
    GRAPH <urn:visionclaw:graph:ontology>  { ?o a vc:OntologyClass . ?o vc:iri ?iri . }
    FILTER NOT EXISTS {
        GRAPH <urn:visionclaw:graph:default> { ?k vc:bridgeTo ?o }
    }
    BIND(IRI(CONCAT("https://visionclaw.dreamlab/ns/edge/",
            SHA256(CONCAT(STR(?k), "|bridgeTo|", STR(?o))))) AS ?bridgeIri)
};

# Migration marker
INSERT DATA { } ;  # marker rows live in SQLite schema_migrations, not in RDF
```

### 0043 — Render properties on ontology tier

**Cypher essence:**
- Seed deterministic x/y/z (pseudo-random hash of id) on OntologyClass
- Seed mass=1.0, size=1.2, color=#9B59B6, weight=1.0
- Seed metadata_id, label, node_type, owl_class_iri
- Backfill KGNode.iri from owl_class_iri where missing
- Add BRIDGE_TO colocated edges (IRI-strict + label-fallback)

**SPARQL Update equivalent (sketch):**
```sparql
# Position seed (one INSERT per axis; idempotent via FNE)
INSERT {
    GRAPH <urn:visionclaw:graph:ontology> {
        ?o vc:hasX ?px ; vc:hasY ?py ; vc:hasZ ?pz .
    }
}
WHERE {
    GRAPH <urn:visionclaw:graph:ontology> { ?o a vc:OntologyClass . }
    FILTER NOT EXISTS {
        GRAPH <urn:visionclaw:graph:ontology> { ?o vc:hasX ?anyX }
    }
    # Deterministic placement: hash-derived. SPARQL has no general hash on
    # IRIs, so the migration tool pre-computes positions during apply.
    BIND("…"^^xsd:float AS ?px)  # filled by template at apply-time
    BIND("…"^^xsd:float AS ?py)
    BIND("…"^^xsd:float AS ?pz)
};

# Render props seed
INSERT {
    GRAPH <urn:visionclaw:graph:ontology> {
        ?o vc:mass   "1.0"^^xsd:decimal ;
           vc:size   "1.2"^^xsd:decimal ;
           vc:color  "#9B59B6" ;
           vc:weight "1.0"^^xsd:decimal .
    }
}
WHERE {
    GRAPH <urn:visionclaw:graph:ontology> { ?o a vc:OntologyClass . }
    FILTER NOT EXISTS { GRAPH <urn:visionclaw:graph:ontology> { ?o vc:mass ?_ } }
};

# Label / node_type seed
# … analogous FNE inserts …

# IRI-strict BRIDGE_TO and label-fallback BRIDGE_TO: as in 0042 but with
# additional FILTER NOT EXISTS guards to skip pairs already bridged.
```

### 0044 — Ontology tier Fibonacci-sphere layout

The Fibonacci-sphere position calculation in Cypher uses `acos`,
`sin`, `cos`. SPARQL 1.1 lacks these functions in the standard.
**Resolution**: pre-compute positions in the migration tool (Rust)
and embed them as `xsd:float` literals in the generated SPARQL
Update file. The migration tool ranks OntologyClass instances by
IRI lexicographic order (deterministic), computes the Fibonacci-
sphere position for each, and emits:

```sparql
DELETE { GRAPH <urn:visionclaw:graph:ontology> {
    ?o vc:hasX ?_ . ?o vc:hasY ?_ . ?o vc:hasZ ?_ .
}}
INSERT {
    GRAPH <urn:visionclaw:graph:ontology> {
        <vc:onto/foo>  vc:hasX "12.34"^^xsd:float ; vc:hasY "…"^^xsd:float ; … .
        <vc:onto/bar>  vc:hasX "…"^^xsd:float ; … .
        …
    }
}
WHERE { }
```

Mass pump to 10.0 and `vc:groupName "ontology"` follow as further
INSERT-with-DELETE statements.

### 0045 — OwlClass → OntologyClass rename

In Neo4j, this migration was non-trivial: orphan stub redirection,
label addition, deferred label removal. In RDF/Oxigraph:

```sparql
# There are no "labels" in RDF — only rdf:type. The OntologyClass /
# OwlClass distinction at the Neo4j level was "two labels on the
# same node". In RDF this is two type triples; we choose one
# canonical type predicate and migrate all existing
# vc:OwlClass-typed entities to vc:OntologyClass type.

INSERT {
    GRAPH <urn:visionclaw:graph:ontology> { ?s a vc:OntologyClass . }
}
WHERE {
    GRAPH <urn:visionclaw:graph:ontology> { ?s a vc:OwlClass . }
    FILTER NOT EXISTS {
        GRAPH <urn:visionclaw:graph:ontology> { ?s a vc:OntologyClass }
    }
};

# Deferred: strip vc:OwlClass type after soak (manual step)
# DELETE { GRAPH <urn:visionclaw:graph:ontology> { ?s a vc:OwlClass } }
# WHERE  { GRAPH <urn:visionclaw:graph:ontology> { ?s a vc:OntologyClass } } ;
```

The orphan-stub redirect step from the Cypher version is **absent**.
The bug class doesn't exist: an entity in
`<urn:visionclaw:graph:ontology>` typed `vc:OwlClass` is the same
RDF subject IRI as the same entity typed `vc:OntologyClass`. There
is no "stub" to redirect to.

## Whelk inference loop

Pseudo-flow (the inference engine itself lives in Section 8; this
section's concern is only how results re-enter the store):

1. Section 8's `OntologyInferenceActor` triggers (on schedule or on
   demand).
2. Inference actor reads asserted axioms via `get_axioms()`
   (SPARQL `CONSTRUCT WHERE GRAPH <...:ontology> { ?s ?p ?o }`).
3. Actor invokes `WhelkInferenceEngine::infer(axioms) ->
   InferenceResults`.
4. Actor calls `store_inference_results(results)`.
5. Section 11's adapter issues one SPARQL Update:
   - `DELETE WHERE { GRAPH <...:inferred> { ?s ?p ?o } }`
   - `INSERT DATA { GRAPH <...:inferred> { ... materialised triples ... } }`
6. Oxigraph commits atomically. The next SPARQL `SELECT` without a
   `FROM` clause sees both asserted and inferred triples.
7. Section 8 emits `OntologyInferred { axiom_count, duration_ms }`
   for downstream observers. Section 11 emits nothing.

The asserted graph is never modified by inference. Re-running
inference is safe: the inferred graph is cleared and rewritten.
The default `OntologyRepository::get_inference_results()` returns
the `InferenceResults` from the most recent run, stored as a single
metadata triple alongside the inferred graph.

## Settings per-user resolution

A read for setting `K` by user `U`:

```sql
SELECT value
FROM settings
WHERE key = ?K AND (owner_pubkey = ?U OR owner_pubkey IS NULL)
ORDER BY owner_pubkey IS NULL ASC   -- pubkey-specific first
LIMIT 1;
```

The `ORDER BY owner_pubkey IS NULL ASC` clause ensures the
user-specific row wins over the global row when both exist.

A write by user `U` always specifies the owner explicitly:

```sql
INSERT INTO settings (key, owner_pubkey, value, description, updated_at)
VALUES (?K, ?U, ?V, ?D, unixepoch())
ON CONFLICT (key, owner_pubkey)
DO UPDATE SET value = excluded.value, description = excluded.description, updated_at = excluded.updated_at;
```

A write with `owner_pubkey = NULL` writes the global default;
permitted only from startup code paths gated by an
`allow_global = true` flag (see ADR-11 R4). All other writes
must come from a request context with `current_owner_pubkey()`
returning Some.

## Backup snapshot model

Two snapshots, one procedure:

- **Oxigraph** is RocksDB underneath. RocksDB supports
  `Checkpoint::create_checkpoint()` for hot snapshots that hardlink
  SSTs. The adapter exposes `snapshot_to(path)` which calls this.
  Alternatively, with the writer stopped, a plain `tar` of the
  data directory is sufficient.
- **SQLite** in WAL mode supports `VACUUM INTO 'path'` for a
  consistent online snapshot; or `sqlite3_backup_*` C API via
  `rusqlite::backup`.

The persistence context defines `Snapshotter::snapshot_to(dir)`
which calls both. Section 9 (Ecosystem Services) owns the
operator-facing schedule and the off-host transport.

## Migration tool (one-shot)

Owned by Section 11 but located in `tools/migrate-neo4j-to-oxigraph/`.
Conceptually one aggregate operation: read source, write target.
The tool is not part of the deployed binary; it is a one-time
artefact retained only until the cutover is committed and Neo4j
adapters are deleted. After cutover the tool is removed from the
workspace.

The tool uses the same IRI minting (`IriMinter`) the runtime
adapter uses, so post-migration the dataset is byte-identical to
what the adapter would produce if it had been writing from day
one.

## What this context does not contain

- **Search-vector storage**. RuVector (PostgreSQL) handles vector
  embeddings for memory tools. Out of scope for Section 11.
- **Session state for tools/agents**. Lives in RuVector
  `session_state` table. Out of scope.
- **Build artefacts / cached compilations**. Filesystem cache
  in `<data-dir>/build-cache/` (if present), owned by build
  tooling, not by persistence.
- **Log / metrics persistence**. Logs go to journald / stdout;
  metrics to Prometheus scrape. Persistence does not store them.

This boundary is deliberately narrow. The persistence context's
contract is: "given an `OwlClass` value, you can give me the same
`OwlClass` back later, after a process restart". Anything outside
that contract is somebody else's problem.
