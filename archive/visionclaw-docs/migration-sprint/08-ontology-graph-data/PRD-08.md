# PRD-08 — Ontology & Graph Data

## 1. Capability statement

VisionClaw ingests a Logseq knowledge graph from GitHub, parses both its
prose markdown and its embedded OWL2-style ontology blocks, and materialises
a single coherent semantic graph that both the physics layer (Section 1) and
the rendering layer (Section 4) consume. The same graph is the substrate the
bots / agent telemetry (Section 7) overlay onto. The data store is migrating
from Neo4j to Oxigraph (Section 11) but the domain model in this section is
adapter-agnostic.

## 2. Why this exists

The baseline at `41979d33e` ships a dual-vocabulary data model that has
accreted accidentally:

- Files with `public:: true` produce `page` nodes; their `[[wikilinks]]`
  produce `linked_page` nodes; ontology constructs produce `owl_*` nodes
  (`OwlClass`, `OwlObjectProperty`, `OwlDatatypeProperty`, etc.).
- The class hierarchy among `OwlClass` nodes is materialised as
  `SUBCLASS_OF` edges, but those edges never reach the client graph. The
  symptom is the *ontology edge gap*: 623 `SUBCLASS_OF` relations from
  `OwlClass` nodes are excluded, leaving 62% of nodes isolated in the
  client view.
- A `linked_page` node carrying the same label as an `OwlClass` node is
  treated as a separate record. The two never resolve to one entity even
  though they semantically refer to the same thing.
- The `OwlClass` type label is being renamed to `OntologyClass` on `main`
  via migrations `0042…0045.cypher`. The rename is a stop on the path; the
  end state is a single domain concept, not a renamed dual table.

The freeze regression also surfaced a second-order data-model bug: node IDs
are numeric `u32` on the wire but get re-coerced to `String` at several
client boundaries inconsistently. This is a typing problem at the data
model level, not a client bug — see the memory entry on "Node ID type
mismatch".

The dual `OwlClass` / `GraphNode` representation is rejected up-front (see
ADR-08 D1). The PRD specifies the unified semantic model and lists the
capabilities that depend on it.

## 3. Users and use cases

- **Knowledge worker** browsing a public Logseq graph. Expects every
  `[[wikilink]]` in a public page to resolve to a clickable node and every
  ontology term referenced anywhere in the corpus to surface as a typed
  node with its definition visible in the side panel.
- **Ontology author** editing `### OntologyBlock` sections inside a page.
  Expects the ontology terms to appear regardless of whether the host page
  is public, and to be linked to whichever public page declared them.
- **Operator** running `FORCE_FULL_SYNC=1` after a structural change.
  Expects every markdown file to be reprocessed and every domain event to
  be re-emitted, with no SHA1 short-circuit.
- **Reasoner** (whelk-rs, EL profile) running over the asserted axioms.
  Expects to materialise inferred subclass relations as ordinary RDF
  triples in Oxigraph that flow into the same graph downstream consumers
  already use.
- **Physics consumer** (Section 1) reading `GraphTopology`. Expects a
  single typed graph with mass and class derivable from each node;
  expects ontology subclass edges to be present in the topology, not
  filtered out at the data-model boundary.
- **Rendering consumer** (Section 4) drawing nodes and edges. Expects
  every node in the topology to carry a stable `NodeClass` so geometry
  selection (Gem / CrystalOrb / AgentCapsule) is deterministic without
  fall-through "unknown type" branches.
- **Agent telemetry** (Section 7) overlaying its agents onto the same
  topology. Expects to inject `Agent` nodes via a documented hook
  rather than by writing into the same Neo4j tables ad hoc.

## 3a. Operational scenarios

The PRD pins acceptance against three concrete operational scenarios
that exercise the full ingestion → projection → consumer pipeline.

- **S1 — cold start on full corpus.** Container starts with empty
  Oxigraph. `GitHubSyncService::sync_graphs()` pulls the corpus,
  parses every file, populates the asserted graph, triggers a whelk
  inference run, and rebuilds the topology projection. End state:
  knowledge worker connects and sees a non-empty graph with all
  declared ontology classes wired via subclass edges.

- **S2 — incremental sync with one file changed.** A single file's
  SHA1 differs from the cached hash. Only that file is re-parsed.
  Affected wikilinks may resolve or upgrade placeholders; affected
  axioms may invalidate cached inference triples. The topology
  projection rebuilds. Other consumers see at most O(changed) event
  emissions, not O(corpus).

- **S3 — `FORCE_FULL_SYNC=1` after operator intervention.** Operator
  manually edited an upstream file or fixed a parser bug. The flag
  bypasses SHA1; every file is reprocessed; events re-emit; the
  topology projection rebuilds in full. End state is identical to S1
  but without re-pulling from GitHub.

## 4. Acceptance criteria

A1. **One concept per thing**. There is exactly one domain type for a
    class in the ontology: `OntologyClass`. There is no parallel
    `GraphNode` record that aliases it. Rendering reads `OntologyClass`
    via a projection, not via a sibling record.

A2. **Wikilinks resolve**. For every `[[wikilink]]` occurring in a
    `public:: true` page, the target resolves to (a) an existing public
    page if the slug matches, (b) an existing `OntologyClass` if the
    label matches, or (c) a placeholder `linked_page` node created on
    first reference. No wikilink is left unresolved.

A3. **Ontology blocks are independent of page visibility**. An
    `### OntologyBlock` with `ontology:: true` inside a non-public page
    produces full `OntologyClass` / `OntologyProperty` / `Axiom` records.
    The host page is recorded as the *defining page* via a `definedIn`
    triple, but ontology surfacing does not depend on host visibility.

A4. **The ontology edge gap closes**. All 623 `SUBCLASS_OF` relations
    among ontology classes are present in the topology snapshot that
    Section 1 receives. Isolation rate falls from 62% to ≤5% (the
    residual 5% are genuinely peripheral terms with no asserted
    relations).

A5. **Node IDs are typed**. A `NodeId` newtype exists; the wire form
    (a sequential `u32` with class-flag bits) and the domain form (the
    `NodeId` newtype) are convertible only via explicit `From`/`TryFrom`
    impls. No silent `String`↔`u32` coercion anywhere in the codebase.

A6. **The migration set translates cleanly**. The four Cypher migrations
    `0042…0045` (OwlClass→OntologyClass rename, `bridge_to`, render tier,
    ontology tier layout) translate to SPARQL Update operations against
    Oxigraph that produce the same end state. Translation evidence is
    attached to ADR-11 (persistence) and cross-referenced from ADR-08.

A7. **`FORCE_FULL_SYNC=1` is honoured end-to-end**. Setting the flag
    causes every file under `GITHUB_BASE_PATH` to be parsed and every
    domain event to be re-emitted. The flag returns to `0` after the run
    completes (operator responsibility; documented in the migration
    runbook).

A8. **`GITHUB_KG_PATH` stays dead**. The env var was removed; this PRD
    does not reintroduce it. `GITHUB_BASE_PATH` is the single root.

A9. **OntologyBlock parsing is deterministic and idempotent**. Parsing
    the same input markdown twice produces the same set of domain events
    in the same order. Re-parsing after no change produces zero events
    (subject to SHA1 incremental gating).

A10. **Inferred triples coexist with asserted triples**. Whelk-rs
     inference output is written back to Oxigraph as RDF triples in a
     named graph `inference`. Asserted triples live in the `asserted`
     named graph. Consumers default to the union view; the source can be
     attributed when needed.

A11. **Bridges resolve.** Every `Page` whose subject is also an
     `OntologyClass` (matched by normalised label) has a `BridgeTo`
     edge in the topology. The 0042-migration semantics are preserved.

A12. **Render tier and ontology tier survive the rename.** The
     `renderTier` and `ontologyTier` properties (migrations 0043, 0044)
     apply to `OntologyClass` aggregates and are present on the topology
     nodes as `render_tier` / `ontology_tier`. Layout assignment that
     depended on tier on `main` continues to behave the same after the
     translation.

A13. **No `linked_page` records leak to the topology after upgrade.**
     Once a `LinkedPage` placeholder upgrades to a `Page` or
     `OntologyClass`, the placeholder is excluded from `GraphTopology`.
     The upgrade event `LinkResolved` is observable in the event log
     for audit.

## 5. Non-goals

- SHACL validation or constraint enforcement at the graph level. Reasoning
  is restricted to OWL EL via whelk-rs; SHACL is a future workstream.
- Bidirectional editing of the ontology from the client. The graph is
  read-only from the client's perspective; mutation is via Logseq
  markdown commits on GitHub.
- Bringing `GITHUB_KG_PATH` back. The env var was dead and stays removed.
- Settings storage. Settings move to SQLite per Section 11; this section
  owns ontology + graph data only.
- Visual responsiveness ("only pink nodes move"). The perception was real
  but the root cause is on the physics side (Section 1), not the data
  model. This PRD does not specify per-type physics behaviour.
- Per-tenant or per-user ontology projections. The corpus is single-tenant
  for this sprint.

## 6. Acceptance evidence to gather during implementation

- A diff of the asserted-triples count between Neo4j (baseline) and
  Oxigraph (target) showing parity within ±1% for `OntologyClass` count,
  `SUBCLASS_OF` count, and `page`/`linked_page` count.
- A snapshot of the topology graph received by Section 1, with node and
  edge counts broken out by class. The 62% isolation figure should not
  reproduce.
- A test fixture (one public page, one private page, both with
  `### OntologyBlock` sections) parsed end-to-end with the resulting
  domain events captured. Re-parsing the fixture must produce zero new
  events when SHA1 is unchanged and the full event set when
  `FORCE_FULL_SYNC=1` is set.
- A static-typing audit showing every `NodeId` boundary crossing is
  explicit; `grep -RE 'as u32|as i64|\.parse::<u32>'` over the codebase
  reports only the points enumerated in ADR-08 D6.
- Whelk-rs inference run on the bundled ontology fixture, with the
  materialised triple count reported and a spot-check of one inferred
  `rdfs:subClassOf` triple traced from input axiom to output triple.

## 7. Out-of-scope smells flagged for ADR review

The baseline and `main` code contains data-model fragilities whose fixes
are deferred to ADR-08 to decide structurally:

- **Ontology edge gap.** `SUBCLASS_OF` edges from `OwlClass` are filtered
  out before reaching the client. Symptom is 62% isolation. Treat as a
  data-model unification issue, not a filter-fix.
- **Dual `OwlClass`/`GraphNode` records.** Two tables for one concept;
  always wrong; the rename to `OntologyClass` is the wedge but the
  unification is the real change.
- **Node ID coercion drift.** `String(id)` appears at the Map-key boundary
  and `=== ` boundary in client code but is missing at others; numeric
  IDs leak across as raw `u32`. Solve by introducing a `NodeId` newtype
  and a small set of conversion functions at adapter boundaries.
- **Sync flow ergonomics.** `GitHubSyncService::sync_graphs()` calls
  `EnhancedContentAPI::list_markdown_files("")` with a hardcoded empty
  path; the empty string is `GITHUB_BASE_PATH` by convention. Make the
  convention an explicit parameter or remove the parameter entirely.
- **Public-page filter coupled to ontology surfacing.** The current code
  treats `public:: true` as the gate for *both* page-node creation *and*
  ontology surfacing, even though `OntologyBlock` parsing should be
  independent. Separate the two filters at the parser boundary.
- **Class flag bits in node IDs.** The top 6 bits encode class
  (`0x80000000` agent, `0x40000000` knowledge, `0x1C000000` ontology
  subtypes), leaving 26 bits of identity. With sequential `u32` IDs from
  the atomic counter, the headroom is 67M IDs — fine — but the encoding
  is implicit. Make it explicit via the `NodeId` newtype.
- **`KnowledgeGraphParser` ownership.** The parser is in
  `src/services/parsers/knowledge_graph_parser.rs` but is invoked from
  `GitHubSyncService::sync_graphs()` and from the ontology pipeline
  separately. Decide where the parser sits in the hexagonal layout — it
  is a domain service, not an adapter.

## 8. Dependencies and sequencing

This section depends on, and is depended on by:

- **Section 11 (Persistence Strategy Migration)** — supplies the
  `OntologyRepository` adapter against Oxigraph. The port at
  `src/ports/ontology_repository.rs` is owned here; its implementation
  is owned there. The Cypher → SPARQL Update translation table (ADR-08
  D4) lives at the boundary; ADR-11 records the mechanical translation.

- **Section 10 (External Integrations)** — supplies the GitHub adapter
  that produces `ParsedMarkdown` value objects. The adapter contract is
  documented in Section 10; the value object schema is referenced here
  and in DDD-08's anti-corruption layer.

- **Section 1 (GPU Physics)** — consumes the `GraphTopology`
  projection. The projection's `NodeClass` enum and `EdgeKind` enum are
  the contract; mass derivation from class is Section 1's concern, not
  this section's.

- **Section 4 (Rendering)** — consumes the same projection. Geometry
  selection (Gem / CrystalOrb / AgentCapsule) is keyed on `NodeClass`;
  this section guarantees the class is always set.

- **Section 7 (Bots & Telemetry)** — injects `Agent` nodes into the
  topology via a documented hook. This section owns the hook signature.

Implementation order, per README phasing:

1. The persistence adapter (Section 11) lands first behind the existing
   `OntologyRepository` port. The port is unchanged; the adapter swaps.
2. The unified domain model from ADR-08 D1 lands next: `OntologyClass`
   subsumes `linked_page`; the topology projection picks up the 623
   missing `SUBCLASS_OF` edges.
3. The `NodeId` newtype (ADR-08 D6) lands as a forward-only change;
   adapter boundaries gain explicit conversions.
4. Migrations `0042…0044` translate to SPARQL Update files and run
   once against the Oxigraph target.
5. Inference materialisation (ADR-08 D9) becomes a scheduled operation
   triggered on `TopologyRebuilt { cause: PostIngest }`.
