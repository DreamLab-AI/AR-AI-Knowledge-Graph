# WORKTREE-PLAN — Phase 2: Ontology & KG Data Model

Worktree  : `visionclaw-worktrees/phase-2-ontology`
Branch    : `impl/phase-2-ontology` (off `radical-rollback @ d260a6158`)
Author    : worktree-planner
Date      : 2026-05-16
Sprint doc: `docs/migration-sprint/08-ontology-graph-data/{PRD,ADR,DDD}-08.md`

This plan covers the full implementation of PRD-08 / ADR-08 / DDD-08.
Phase 1 (Persistence — Section 11) must land before tasks marked [P1-DEP].
Do not commit any code in this worktree until Phase 1 adapters compile
and their port surfaces are stable.

---

## 1. Phase 2 Task Breakdown

Tasks are ordered by dependency. Each must be completable without touching tasks
later in the list, except where an explicit BLOCKS or NEEDS relation is stated.

---

### T-01  TC-1 — Rename `ONTOLOGY_INDIVIDUAL` to `LINKED_PAGE` + add `AXIOM` flag

Complexity: S (1 pt)
Dependencies: none — pure rename, no Phase 1 outputs needed
PRD acceptance: A5 (typed NodeId wire constants must match DDD-08 domain names)

Affected files:
- `src/utils/binary_protocol.rs` lines 22, 178, 191, 208, 214, 231, 378, 515,
  1049, 1055
- `src/handlers/socket_flow_handler/position_updates.rs` lines 109, 170, 519
- `src/actors/gpu/ontology_constraint_actor.rs` line 295 (comment only)

Work:
1. Rename constant `ONTOLOGY_INDIVIDUAL_FLAG` → `LINKED_PAGE_FLAG` (0x08000000,
   value unchanged per TC-1 Option A).
2. Add constant `AXIOM_FLAG: u32 = 0x0C000000`.
3. Rename `NodeType::OntologyIndividual` → `NodeType::LinkedPage`.
4. Add `NodeType::Axiom` variant; add `is_axiom(node_id: u32) -> bool` and
   `set_axiom_flag(node_id: u32) -> u32` helpers following the existing pattern.
5. Rename `set_ontology_individual_flag` → `set_linked_page_flag`; update all
   call sites.
6. Update `get_node_type` match arm.
7. In `position_updates.rs` lines 109, 170, 519: update call sites to
   `set_linked_page_flag`; add `NodeType::Axiom => "ontology"` branch to the
   match at line 519.
8. In `ontology_constraint_actor.rs` line 295: update the comment only.

Acceptance criterion: `grep -rn ONTOLOGY_INDIVIDUAL src/` returns zero matches.
The mask-coverage assertion from TC-1 passes:
`(ONTOLOGY_CLASS_FLAG | LINKED_PAGE_FLAG | AXIOM_FLAG | ONTOLOGY_PROPERTY_FLAG)
 & !ONTOLOGY_TYPE_MASK == 0`

---

### T-02  Introduce `NodeId` newtype with class-bit encoding

Complexity: M (3 pt)
Dependencies: T-01 (flag constants must be stable first)
PRD acceptance: A5

Affected files:
- `src/models/node.rs` (new `NodeId` struct alongside existing `Node`)
- `src/utils/binary_protocol.rs` (add `NodeId`-aware helpers)
- `src/ports/ontology_repository.rs` (update `OwlClass.iri` context note)

Work:
1. Add to `src/models/node.rs`:

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct NodeId(u32);

impl NodeId {
    pub fn new(seq: u32, class: NodeClass) -> Self {
        debug_assert!(seq <= NODE_ID_MASK, "sequence exceeds 26-bit limit");
        Self((seq & NODE_ID_MASK) | class.flag_bits())
    }
    pub fn class(&self) -> NodeClass { NodeClass::from_raw(self.0) }
    pub fn sequence(&self) -> u32   { self.0 & NODE_ID_MASK }
    pub fn as_u32(&self) -> u32     { self.0 }
}

impl From<NodeId> for String { fn from(n: NodeId) -> Self { n.0.to_string() } }

impl TryFrom<u32> for NodeId {
    type Error = &'static str;
    fn try_from(v: u32) -> Result<Self, Self::Error> { Ok(Self(v)) }
}

impl TryFrom<&str> for NodeId {
    type Error = std::num::ParseIntError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self(s.parse::<u32>()?))
    }
}
```

2. Add `NodeClass` enum to `src/models/node.rs` mapping to the flag bits
   defined in T-01 (`Page=0x40000000`, `Agent=0x80000000`,
   `OntologyClass=0x04000000`, `LinkedPage=0x08000000`,
   `Axiom=0x0C000000`, `OntologyProperty=0x10000000`).

3. The existing `Node.id: u32` field is NOT changed in this task — the
   newtype is introduced alongside it. Adapter boundaries (T-07, T-08)
   perform explicit conversion. This avoids a big-bang change.

Acceptance criterion: `src/models/node.rs` compiles with the new type; a unit
test round-trips `NodeId::new(1234, NodeClass::OntologyClass).as_u32()` and
verifies `class() == NodeClass::OntologyClass` and `sequence() == 1234`.

---

### T-03  Define `KnowledgeGraphEvent` domain event enum

Complexity: S (1 pt)
Dependencies: T-02 (NodeId type needed for event fields)
PRD acceptance: A9 (parser must emit deterministic events)

Affected files:
- `src/domain/events.rs` (new file — create under `src/domain/`)
- `src/domain/mod.rs` (new file or extend if exists)

Work:
Create the event taxonomy from DDD-08 §Domain events:
`PageIngested`, `PageRevised`, `OntologyClassDefined`, `OntologyPropertyDefined`,
`AxiomAsserted`, `LinkResolved`, `LinkRegistered`, `BridgeEstablished`,
`InferenceMaterialised`, `TopologyRebuilt`.

Each variant carries exactly the fields documented in DDD-08. Use `NodeId` for
all ID fields. No logic — this is a data type definition only.

Acceptance criterion: `cargo check` on this file with no errors. Every variant
in DDD-08 §Domain events is represented.

---

### T-04  Refactor `KnowledgeGraphParser` as pure-functional domain service

Complexity: M (3 pt)
Dependencies: T-03 (event enum), T-02 (NodeId)
PRD acceptance: A9, A3

Affected files:
- `src/services/parsers/knowledge_graph_parser.rs` (full refactor)
- `src/services/parsers/ontology_parser.rs` (adopt `ParsedMarkdown` input type)
- `src/services/parsers/mod.rs`

Work:
1. Define `ParsedMarkdown` value object in `src/domain/value_objects.rs`:
   ```rust
   pub struct ParsedMarkdown {
       pub path: String,
       pub sha1: ContentHash,
       pub is_public: bool,
       pub frontmatter: Frontmatter,
       pub prose_blocks: Vec<String>,
       pub ontology_blocks: Vec<OntologyBlock>,
       pub outbound_wikilinks: Vec<WikilinkRef>,
   }
   ```
   The GitHub adapter (Phase 1 or Section 10) constructs this; the parser
   receives it.

2. Define the `KnowledgeGraphParser` trait (ADR-08 D5):
   ```rust
   pub trait KnowledgeGraphParser: Send + Sync {
       fn parse(&self, file: ParsedMarkdown) -> Vec<KnowledgeGraphEvent>;
   }
   ```

3. Implement `DefaultKnowledgeGraphParser` that:
   - Applies the `is_public_page` gate for `Page` events.
   - Applies `has_ontology_blocks` independently for `OntologyClass` events
     (ADR-08 D3 — the two filters are decoupled).
   - Never writes to any repository — all side effects are expressed as events.
   - Returns events in deterministic order (stable sort by IRI).

4. Remove the inline repository write calls from the current parser
   (`src/services/parsers/knowledge_graph_parser.rs` lines referencing
   `KnowledgeGraphRepository` directly).

Acceptance criterion: The existing `OntologyParser` integration tests pass.
A new test feeds one public page with an `OntologyBlock` and one private page
with an `OntologyBlock`; verifies that `PageIngested` is only emitted for the
public page but `OntologyClassDefined` is emitted for both.

---

### T-05  Create `OntologyRepository` port extension for unified domain types  [P1-DEP]

Complexity: M (3 pt)
Dependencies: T-02, T-03 — **and Phase 1**: the Oxigraph adapter must expose
the `OntologyRepository` trait implementation before this task extends the port.
Phase 1 output needed: `src/adapters/oxigraph_ontology_repository.rs` must
exist and compile.

PRD acceptance: A1 (one concept per thing), A2 (wikilinks resolve), A4 (edge gap
closes), A11 (bridges resolve)

Affected files:
- `src/ports/ontology_repository.rs` (add unified domain methods)

Work:
Extend the `OntologyRepository` trait with the operations required by DDD-08
`Graph` aggregate:

```rust
async fn upsert_page(&self, page: Page) -> Result<()>;
async fn upsert_ontology_class(&self, class: OntologyClass) -> Result<()>;
async fn upsert_axiom(&self, axiom: Axiom) -> Result<()>;
async fn upsert_linked_page(&self, lp: LinkedPage) -> Result<()>;
async fn upgrade_linked_page(
    &self, placeholder_id: NodeId, upgrade_to: NodeId,
) -> Result<()>;
async fn fetch_page_by_iri(&self, iri: &Iri) -> Result<Option<Page>>;
async fn fetch_class_by_label(&self, label: &str) -> Result<Option<OntologyClass>>;
async fn project_topology(&self) -> Result<GraphTopology>;
async fn assert_inferred_triples(&self, triples: Vec<RdfTriple>) -> Result<usize>;
```

Do not remove the existing `OwlClass`-based methods yet — that is T-06.
This task adds the new surface; T-06 migrates callers; T-09 removes the old.

Acceptance criterion: `src/ports/ontology_repository.rs` compiles. The Phase 1
Oxigraph adapter's stub `todo!()` implementations for these new methods compile
without error (stubs are acceptable at this stage).

---

### T-06  Migrate callers from `OwlClass` to unified domain aggregates  [P1-DEP]

Complexity: L (8 pt)
Dependencies: T-05, and Phase 1 Oxigraph adapter must have non-stub
implementations of `upsert_page`, `upsert_ontology_class`, `upsert_axiom`
before this task's integration tests can pass.
Phase 1 outputs needed: `project_topology` and `upsert_*` methods on the
Oxigraph adapter.

PRD acceptance: A1, A4, A12

Affected files:
- `src/services/owl_extractor_service.rs`
- `src/services/ontology_converter.rs`
- `src/services/ontology_query_service.rs`
- `src/services/ontology_reasoner.rs`
- `src/services/parsers/ontology_parser.rs`
- `src/services/parsers/mod.rs`
- `src/application/ontology/queries.rs`
- `src/application/ontology/mod.rs`
- `src/application/inference_service.rs`

Work:
1. Replace every `OwlClass` reference in service files with the new
   `OntologyClass` aggregate from `src/domain/aggregates.rs` (which T-05
   depends on).
2. `ontology_converter.rs::create_node_from_class` is rewritten to
   consume `OntologyClass` and emit `TopologyNode` directly (no intermediate
   `Node` struct conversion). The `node_type: Some("linked_page")` branch
   is replaced with `NodeClass::LinkedPage` class-bit stamping on `NodeId`.
3. `ontology_query_service.rs` line 330: remove the `"OwlClass"` literal
   string guard; it now checks `NodeClass::OntologyClass` via the newtype.
4. `ontology_reasoner.rs` lines 439-488: `OwlClass { ... }` construction
   sites become `OntologyClass { ... }` with `NodeId::new(seq, NodeClass::OntologyClass)`.
5. `application/inference_service.rs`: update to call `assert_inferred_triples`
   on the repository instead of the legacy Neo4j path.

Acceptance criterion: `cargo check` on all affected files. The topology
projection's edge count includes `SUBCLASS_OF` edges (verified by a test
feeding known-subclass axioms and asserting `GraphTopology.edges.len() > 0`
with `EdgeKind::SubClassOf` variants present).

---

### T-07  Translate Cypher migration 0042 → SPARQL Update (bridge_to)

Complexity: S (1 pt)
Dependencies: none (doc-only; the SPARQL file is data, not compiled code)
PRD acceptance: A6, A11

Affected files:
- `queries/migrations/sparql/0042_bridge_to.rq` (new file, create directory)
- `queries/migrations/legacy/0042_bridge_to.cypher` (move if present)

Work:
The Cypher migration 0042 adds `bridge_to` relations between knowledge pages and
their ontology class counterparts. The SPARQL Update equivalent:

```sparql
PREFIX vf: <urn:visionclaw:>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

INSERT {
  GRAPH <urn:visionclaw:graph:ontology:assert> {
    ?pageIri vf:bridgeTo ?classIri .
  }
}
WHERE {
  GRAPH <urn:visionclaw:graph:ontology:assert> {
    ?pageIri rdf:type vf:Page ;
             vf:label ?pageLabel .
    ?classIri rdf:type vf:OntologyClass ;
              vf:label ?classLabel .
    FILTER(LCASE(STR(?pageLabel)) = LCASE(STR(?classLabel)))
    FILTER NOT EXISTS { ?pageIri vf:bridgeTo ?classIri }
  }
}
```

The join is by normalised label (LCASE comparison), matching the Cypher
migration's `WHERE n.label = m.label` semantics. The `FILTER NOT EXISTS`
makes the insert idempotent.

Write the SPARQL file to `queries/migrations/sparql/0042_bridge_to.rq`.
Document in a header comment: the source Cypher migration ID, date, and
the acceptance criterion it satisfies (A11).

Acceptance criterion: The `.rq` file parses without error when loaded via
Oxigraph's `sparql_update` API in a test fixture (verified in T-10).

---

### T-08  Translate Cypher migration 0043 → SPARQL Update (renderTier)

Complexity: S (1 pt)
Dependencies: T-07 (establish the `sparql/` directory convention)
PRD acceptance: A6, A12

Affected files:
- `queries/migrations/sparql/0043_render_tier.rq` (new)

Work:
Migration 0043 adds a `renderTier` integer property to nodes reflecting their
visual layering. The SPARQL Update:

```sparql
PREFIX vf: <urn:visionclaw:>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT {
  GRAPH <urn:visionclaw:graph:ontology:assert> {
    ?node vf:renderTier ?tier .
  }
}
WHERE {
  GRAPH <urn:visionclaw:graph:ontology:assert> {
    ?node a ?type .
    FILTER NOT EXISTS { ?node vf:renderTier ?existing }
    BIND(
      IF(?type = <urn:visionclaw:Page>,            "0"^^xsd:integer,
      IF(?type = <urn:visionclaw:OntologyClass>,   "1"^^xsd:integer,
      IF(?type = <urn:visionclaw:OntologyProperty>,"2"^^xsd:integer,
      IF(?type = <urn:visionclaw:LinkedPage>,      "3"^^xsd:integer,
                                                   "4"^^xsd:integer))))
      AS ?tier
    )
  }
}
```

Tier assignment matches the visual layering order on `main` (knowledge
pages front, ontology classes mid, properties and placeholders back).
The assignment is data-driven; operators may hand-tune by issuing a
subsequent `DELETE { ?n vf:renderTier ?old } INSERT { ?n vf:renderTier ?new }`
without re-running this migration.

Acceptance criterion: After execution against a fixture with at least one node
of each type, every node in the fixture graph has exactly one `vf:renderTier`
triple.

---

### T-09  Translate Cypher migration 0044 → SPARQL Update (ontologyTier)

Complexity: S (1 pt)
Dependencies: T-08
PRD acceptance: A6, A12

Affected files:
- `queries/migrations/sparql/0044_ontology_tier.rq` (new)

Work:
Migration 0044 seeds `ontologyTier` layout assignment — a second tier property
used by the ontology-specific layout engine (depth from owl:Thing root).

```sparql
PREFIX vf: <urn:visionclaw:>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT {
  GRAPH <urn:visionclaw:graph:ontology:assert> {
    ?cls vf:ontologyTier ?depth .
  }
}
WHERE {
  GRAPH <urn:visionclaw:graph:ontology:assert> {
    ?cls a <urn:visionclaw:OntologyClass> .
    FILTER NOT EXISTS { ?cls vf:ontologyTier ?existing }
    {
      SELECT ?cls (COUNT(?mid) AS ?depth)
      WHERE {
        ?cls rdfs:subClassOf* ?mid .
        ?mid a <urn:visionclaw:OntologyClass> .
      }
      GROUP BY ?cls
    }
    BIND(?depth AS ?depth)
  }
}
```

The `COUNT(?mid)` approximates the ancestry chain length from owl:Thing;
top-level classes (no `rdfs:subClassOf`) get tier 0. This replaces the
hand-tuned integer values on `main` with a derivable heuristic; the prior
exact values are captured as a dump in
`queries/migrations/legacy/0044_ontology_tier_seed_values.csv` for
operator reference if manual correction is needed.

Acceptance criterion: After execution, every `OntologyClass` node in the
fixture has a `vf:ontologyTier` triple. Classes with no `rdfs:subClassOf`
have tier 0 or 1; subclasses have strictly greater tier than their superclass.

---

### T-10  Translate Cypher migration 0045 → SPARQL no-op + migration record

Complexity: S (1 pt)
Dependencies: T-09 (complete the SPARQL migration series)
PRD acceptance: A6

Affected files:
- `queries/migrations/sparql/0045_rename_noop.rq` (new)

Work:
Under ADR-08 D1 and D4, migration 0045 (rename `OwlClass` → `OntologyClass`)
is a no-op on the forward path: the domain model uses `OntologyClass` from day
one; no `OwlClass` nodes are ever written.

Write a SPARQL file that documents this:

```sparql
# Migration 0045 — OwlClass → OntologyClass rename
#
# Status: NO-OP on the impl/phase-2-ontology forward path.
#
# Rationale: Under ADR-08 D1, the unified domain model writes OntologyClass
# nodes from the first ingest. No OwlClass nodes exist in the Oxigraph target.
#
# For deployments that retained the old vocabulary (e.g. a Neo4j export
# imported into Oxigraph without the D1 unification), apply:
#
DELETE { GRAPH ?g { ?n a <urn:visionclaw:OwlClass> } }
INSERT { GRAPH ?g { ?n a <urn:visionclaw:OntologyClass> } }
WHERE  { GRAPH ?g { ?n a <urn:visionclaw:OwlClass> } }
#
# On a clean forward-path deployment, the WHERE clause matches zero rows.
```

Acceptance criterion: The file is syntactically valid SPARQL Update. When
executed against the Oxigraph fixture populated by T-04/T-06 (no `OwlClass`
nodes), zero triples are modified.

---

### T-11  Write SPARQL integration tests for the migration series

Complexity: M (3 pt)
Dependencies: T-07, T-08, T-09, T-10, Phase 1: `OxigraphOntologyRepository`
must be constructible in tests.
Phase 1 output needed: the Oxigraph test harness / in-memory `Store` fixture.

PRD acceptance: A6 (evidence of clean migration)

Affected files:
- `tests/migrations/sparql_migration_tests.rs` (new)

Work:
1. Construct an in-memory Oxigraph `Store` (via the Phase 1 test harness).
2. Load a fixture: one `Page` node, two `OntologyClass` nodes with a
   `rdfs:subClassOf` relation, one `LinkedPage` node.
3. Execute `0042_bridge_to.rq` — assert one `vf:bridgeTo` triple is created.
4. Execute `0043_render_tier.rq` — assert every node has exactly one
   `vf:renderTier` triple.
5. Execute `0044_ontology_tier.rq` — assert subclass has greater tier than
   superclass.
6. Execute `0045_rename_noop.rq` — assert zero triples modified.
7. Execute all four again — assert idempotency (no duplicate triples).

Acceptance criterion: `cargo test migrations::` passes. The test is included
in CI.

---

### T-12  Implement `OntologyClass` / `LinkedPage` unification in parser  [P1-DEP]

Complexity: L (8 pt)
Dependencies: T-04, T-06, T-05, and Phase 1: `upgrade_linked_page` method on
Oxigraph adapter.
Phase 1 output needed: `upgrade_linked_page(placeholder_id, upgrade_to)`
implemented (not `todo!()`) in the Oxigraph adapter.

PRD acceptance: A1, A2, A3, A4, A13

Affected files:
- `src/services/parsers/knowledge_graph_parser.rs` (placeholder lifecycle)
- `src/services/github_sync_service.rs` (filter_linked_pages removal, upgrade
  path addition)
- `src/domain/aggregates.rs` (LinkedPage upgrade logic)

Work (the "placeholder-upgrade semantics" from DDD-08 §LinkedPage):

1. During `resolve_wikilinks` phase of parsing, for each `[[wikilink]]` target:
   a. Query `fetch_page_by_iri` — if found, no placeholder needed.
   b. Query `fetch_class_by_label` — if found, the wikilink resolves to an
      existing `OntologyClass`; emit `LinkResolved { upgrade_kind: ToOntologyClass }`.
   c. If neither found, call `upsert_linked_page` and emit `LinkRegistered`.

2. On subsequent parse runs, when a `Page` or `OntologyClass` is first seen
   whose normalised label matches an existing `LinkedPage` IRI:
   - Call `upgrade_linked_page(placeholder_id, new_id)` — only the class bits
     change in the NodeId; sequence is preserved (DDD-08 §C2, L2).
   - Emit `LinkResolved`.

3. Remove `github_sync_service.rs::filter_linked_pages` (lines 539-554).
   The filter was the proximate cause of the ontology edge gap. After
   unification, `LinkedPage` placeholders that have been upgraded are
   excluded from the topology by `project_topology` directly (ADR-08 D2,
   DDD-08 L2 — upgraded placeholders are in the repo but not in the
   topology snapshot).

4. Remove the `("OwlClass", "MATCH (n:OwlClass) DETACH DELETE n")` entry
   from `github_sync_service.rs` line 749 (Cypher delete path, no longer
   applicable under Oxigraph).

Acceptance criterion: A fixture with a private page containing an
`OntologyBlock` plus a separate public page with a `[[wikilink]]` to the
same class label is parsed twice. First parse: one `LinkedPage` placeholder,
one `OntologyClass`. Second parse: `LinkResolved` is emitted; placeholder
count is zero in the topology; the 623-edge gap does not reproduce (test
asserts `topology.edges.iter().filter(|e| e.kind == EdgeKind::SubClassOf).count() == fixture_subclass_count`).

---

### T-13  `GraphTopology` struct definition and `project_topology` SPARQL query

Complexity: M (3 pt)
Dependencies: T-05, T-06
PRD acceptance: A4 (all 623 subclass edges present)

Affected files:
- `src/domain/topology.rs` (new file)
- `src/ports/ontology_repository.rs` (trait method signature references this type)

Work:
Define `GraphTopology` and its child types as documented in ADR-08 D2:

```rust
pub struct GraphTopology {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

pub struct TopologyNode {
    pub id: NodeId,
    pub class: NodeClass,
    pub label: String,
    pub mass: f32,
    pub definition_summary: Option<String>,
    pub render_tier: u8,
    pub ontology_tier: Option<u8>,
}

pub struct TopologyEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub weight: f32,
}

pub enum NodeClass {
    Page, OntologyClass, OntologyProperty, LinkedPage, Agent,
}

pub enum EdgeKind {
    Wikilink, SubClassOf, EquivalentClass, PropertyAssertion,
    DefinedIn, BridgeTo, AgentControls,
}
```

The SPARQL SELECT that materialises `GraphTopology` queries the union of
`<urn:visionclaw:graph:ontology:assert>` and
`<urn:visionclaw:graph:ontology:inferred>` (CC-4 resolution — canonical IRI
from ADR-11, not the shorter `<urn:visionclaw:inference>` which ADR-08 D9
originally used). The query is stored at
`queries/topology/project_topology.rq` and loaded by the Oxigraph adapter
at repository construction time.

The query skeleton (to be completed by the backend-dev specialist):

```sparql
PREFIX vf:   <urn:visionclaw:>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?nodeId ?type ?label ?definition ?renderTier ?ontologyTier
       ?srcId ?tgtId ?edgeKind ?weight
FROM <urn:visionclaw:graph:ontology:assert>
FROM <urn:visionclaw:graph:ontology:inferred>
WHERE {
  { ?nodeId rdf:type ?type ; vf:label ?label .
    OPTIONAL { ?nodeId vf:definition ?definition }
    OPTIONAL { ?nodeId vf:renderTier ?renderTier }
    OPTIONAL { ?nodeId vf:ontologyTier ?ontologyTier }
    FILTER NOT EXISTS {
      ?nodeId vf:upgradedTo ?_ .   # exclude promoted placeholders
    }
  }
  UNION
  { ?srcId ?edgePred ?tgtId .
    BIND(STR(?edgePred) AS ?edgeKind)
    FILTER(?edgePred IN (rdfs:subClassOf, vf:bridgeTo, vf:wikilink,
                         vf:equivalentClass, vf:definedIn))
    BIND("1.0"^^<xsd:float> AS ?weight)
  }
}
```

Acceptance criterion: `GraphTopology` compiles. The `project_topology` port
method returns it. A unit test with a known fixture asserts that
`topology.edges` contains at least one `EdgeKind::SubClassOf` entry.

---

### T-14  `GitHubSyncService` update — Oxigraph adapter, `GITHUB_BASE_PATH` explicit  [P1-DEP]

Complexity: M (3 pt)
Dependencies: T-04, T-12, and Phase 1: Oxigraph adapter injected into
`GitHubSyncService` constructor.
Phase 1 output needed: `OxigraphOntologyRepository` must be injectable in
place of `Neo4jOntologyRepository` in `GitHubSyncService::new`.

PRD acceptance: A7 (`FORCE_FULL_SYNC=1` end-to-end), A8 (`GITHUB_KG_PATH` stays dead)

Affected files:
- `src/services/github_sync_service.rs`

Work:
1. Change the `onto_repo` field type from `Arc<Neo4jOntologyRepository>` to
   `Arc<dyn OntologyRepository>` — the port, not the concrete Neo4j adapter.
   This unblocks substitution with the Oxigraph adapter.
2. Apply ADR-08 D7: the call `self.content_api.list_markdown_files("")`
   becomes `self.content_api.list_markdown_files(&self.base_path)`.
   Add `base_path: String` field; read once from `std::env::var("GITHUB_BASE_PATH")`
   in the constructor. Remove the hardcoded `""` call site.
3. Verify `GITHUB_KG_PATH` is not referenced anywhere in this file
   (already removed per memory entry; add a `grep` assertion to CI).
4. `FORCE_FULL_SYNC=1` path (ADR-08 D8): wrap the SHA1 gate in
   `if std::env::var("FORCE_FULL_SYNC").as_deref() == Ok("1") { /* skip sha1 check */ }`.
   Ensure the flag causes every file to be re-parsed and every domain event
   to be re-emitted (upsert, not delete+recreate).
5. After sync, emit `TopologyRebuilt { cause: RebuildCause::PostIngest }`.

Acceptance criterion: With `Neo4jOntologyRepository` replaced by the Oxigraph
adapter in tests, `sync_graphs()` runs against a fixture GitHub response
without compile errors. `FORCE_FULL_SYNC=1` causes all 998 fixture files
to appear in the processed list regardless of SHA1 match.

---

### T-15  Wire whelk-rs inference into named graph `inferred`  [P1-DEP]

Complexity: M (3 pt)
Dependencies: T-13, T-14, and Phase 1: `assert_inferred_triples` method
implemented in Oxigraph adapter writing to `<urn:visionclaw:graph:ontology:inferred>`.
Phase 1 output needed: named-graph write confirmed by a Phase 1 test.

PRD acceptance: A10

Affected files:
- `src/application/inference_service.rs`
- `src/adapters/whelk_inference_engine.rs`

Work:
1. After `sync_graphs()` emits `TopologyRebuilt { cause: PostIngest }`,
   the `inference_service` subscribes to this event and triggers
   `RunInference { profile: ReasoningProfile::EL }`.
2. `WhelkInferenceEngine` runs over the triples in
   `<urn:visionclaw:graph:ontology:assert>`, producing a `Vec<RdfTriple>`.
3. `assert_inferred_triples` writes them to
   `<urn:visionclaw:graph:ontology:inferred>` (CC-4: canonical IRI).
4. Emit `InferenceMaterialised { triple_count, profile, elapsed_ms }`.
5. Trigger `RebuildTopology` — now the topology query unions both named graphs
   and subclass edges from inference are present.

Acceptance criterion: A test loads a fixture with two OntologyClass nodes and
one asserted `rdfs:subClassOf`; after inference, the inferred graph contains
at least one additional triple (the transitive closure, if any). The topology
returned by `project_topology` has `SubClassOf` edges from both graphs.

---

### T-16  Parity tests: OntologyClass / LinkedPage unification

Complexity: M (3 pt)
Dependencies: T-12, T-13, T-15
PRD acceptance: A1, A4, A13 (isolation rate ≤ 5%)

Affected files:
- `tests/ontology/parity_tests.rs` (new)

Work:
1. Build a fixture that mirrors the production corpus shape: ~199 public pages,
   ~62 OntologyClass nodes, ~623 subclass relations (can use a scaled-down
   representative fixture of 20 pages + 6 classes + 60 subclass edges).
2. Assert topology isolation rate = isolated nodes / total nodes ≤ 5%.
3. Assert zero `linked_page`-typed nodes appear in topology when a matching
   `Page` or `OntologyClass` exists.
4. Assert `SUBCLASS_OF` edge count equals the number of asserted
   `rdfs:subClassOf` triples in the fixture.
5. Assert re-parsing the fixture with unchanged SHA1 produces zero new events
   (idempotency — A9).
6. Assert re-parsing with `FORCE_FULL_SYNC=1` reproduces the full event set.

Acceptance criterion: All assertions pass. CI must run this test in the
`tests/ontology/` suite.

---

## 2. Cypher → SPARQL Translation Plan

The four files land under `queries/migrations/sparql/`. The Cypher originals
move to `queries/migrations/legacy/` for archaeology (ADR-08 D4).

SPARQL Update bodies are embedded in tasks T-07 through T-10 above. Summary:

| Migration | File | Semantic | Status |
|-----------|------|----------|--------|
| 0042 | `0042_bridge_to.rq` | INSERT bridgeTo by normalised label | Active |
| 0043 | `0043_render_tier.rq` | INSERT renderTier per node type | Active |
| 0044 | `0044_ontology_tier.rq` | INSERT ontologyTier via ancestry depth | Active |
| 0045 | `0045_rename_noop.rq` | No-op (documented for legacy deployments) | No-op |

All four are idempotent (FILTER NOT EXISTS guards or replace-then-insert).
All four target `<urn:visionclaw:graph:ontology:assert>` as per CC-4.

---

## 3. OntologyClass / LinkedPage Unification Plan

The concrete steps to collapse the dual-table `(OwlClass, linked_page)` into a
single `OntologyClass` concept per ADR-08 D1, with placeholder-upgrade
semantics per DDD-08.

### Step 1 — Domain aggregate definition (T-05)
Create `src/domain/aggregates.rs` with `Page`, `OntologyClass`,
`OntologyProperty`, `Axiom`, `LinkedPage` structs using `NodeId` fields.
These replace `OwlClass` from `src/ports/ontology_repository.rs`.

### Step 2 — Port extension (T-05)
Add unified CRUD methods to `OntologyRepository`. The `OwlClass`-based
methods remain for backward compile compatibility until step 4.

### Step 3 — Parser refactor (T-04)
`KnowledgeGraphParser` emits `OntologyClassDefined` / `LinkRegistered`
domain events instead of constructing `OwlClass` structs. The parser is
pure-functional (no repo writes).

### Step 4 — Caller migration (T-06)
All service files swap `OwlClass` for `OntologyClass`. The `OwlClass`
struct in `ontology_repository.rs` is deprecated (marked `#[deprecated]`)
but not yet removed — compilation must stay green.

### Step 5 — Placeholder lifecycle (T-12)
`resolve_wikilinks` creates `LinkedPage` placeholders on first reference
and upgrades them on subsequent runs when the target resolves. Upgrades
preserve the sequence bits of the `NodeId` (DDD-08 §C2). `LinkResolved`
is emitted and observable.

### Step 6 — Topology filter removal (T-12)
`filter_linked_pages` in `github_sync_service.rs` is deleted. Topology
exclusion of upgraded placeholders is handled by `project_topology`'s
`FILTER NOT EXISTS { ?nodeId vf:upgradedTo ?_ }` guard.

### Step 7 — Old `OwlClass` struct removal (after T-16 green)
After all parity tests pass, remove the `OwlClass` struct and all
deprecated methods from `ontology_repository.rs`. This is a final cleanup
task not listed separately; it is the `#[deprecated]` removal pass.

---

## 4. Node ID Class-Bit Migration (TC-1)

TC-1 (TENSIONS-RESOLVED.md) is resolved by T-01. Complete code site inventory:

| File | Lines | Change |
|------|-------|--------|
| `src/utils/binary_protocol.rs` | 22 | rename const `ONTOLOGY_INDIVIDUAL_FLAG` → `LINKED_PAGE_FLAG` |
| `src/utils/binary_protocol.rs` | 178, 191 | rename `NodeType::OntologyIndividual` → `NodeType::LinkedPage` |
| `src/utils/binary_protocol.rs` | 208, 214 | rename fn `set_ontology_individual_flag` → `set_linked_page_flag` |
| `src/utils/binary_protocol.rs` | 231 | update `is_ontology_individual` predicate name → `is_linked_page` |
| `src/utils/binary_protocol.rs` | 378, 515 | update call sites to `set_linked_page_flag` |
| `src/utils/binary_protocol.rs` | 1049, 1055 | update test to use `LINKED_PAGE_FLAG`, `NodeType::LinkedPage` |
| `src/handlers/socket_flow_handler/position_updates.rs` | 109, 170 | update call site |
| `src/handlers/socket_flow_handler/position_updates.rs` | 519 | add `NodeType::Axiom => "ontology"` match arm |
| `src/actors/gpu/ontology_constraint_actor.rs` | 295 | comment update only |

After T-01, add the mask-coverage unit test from TC-1 §Verification:
```rust
#[test]
fn ontology_bits_fit_within_mask() {
    assert_eq!(
        (ONTOLOGY_CLASS_FLAG | LINKED_PAGE_FLAG | AXIOM_FLAG | ONTOLOGY_PROPERTY_FLAG)
        & !ONTOLOGY_TYPE_MASK,
        0
    );
}
```

No wire-format change: `0x08000000` is unchanged, only the constant name
and enum variant rename. No CUDA change. No client change.

---

## 5. Graph Topology Read API

Per ADR-08 D2 and the anti-corruption layer to Section 1 (DDD-08 §ACL to
Section 1), the `GraphTopology` snapshot is the sole read contract between
this bounded context and its consumers.

### Struct definitions (T-13)

File: `src/domain/topology.rs`

```rust
pub struct GraphTopology {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

pub struct TopologyNode {
    pub id: NodeId,
    pub class: NodeClass,        // drives geometry selection in Section 4
    pub label: String,
    pub mass: f32,               // Section 1 reads this; derived from class
    pub definition_summary: Option<String>,
    pub render_tier: u8,         // from vf:renderTier (migration 0043)
    pub ontology_tier: Option<u8>, // from vf:ontologyTier (migration 0044)
}

pub struct TopologyEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub weight: f32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NodeClass {
    Page,
    OntologyClass,
    OntologyProperty,
    LinkedPage,      // placeholders not yet upgraded
    Agent,           // injected by Section 7 hook
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum EdgeKind {
    Wikilink,
    SubClassOf,
    EquivalentClass,
    PropertyAssertion,
    DefinedIn,
    BridgeTo,
    AgentControls,
}
```

### Materialisation by Oxigraph adapter (Phase 1 cooperation)

The Oxigraph adapter implements `project_topology()` by executing
`queries/topology/project_topology.rq` against the union of
`<urn:visionclaw:graph:ontology:assert>` and
`<urn:visionclaw:graph:ontology:inferred>`.

The adapter is responsible for:
- Mapping SPARQL result rows to `TopologyNode` / `TopologyEdge`.
- Filtering upgraded `LinkedPage` placeholders (the `FILTER NOT EXISTS` in the
  query excludes nodes that have a `vf:upgradedTo` predicate).
- Computing `mass` from `NodeClass` (Page=1.0, OntologyClass=2.0,
  OntologyProperty=1.5, LinkedPage=0.5, Agent=3.0 — values match the
  Section 1 physics defaults).
- Injecting `Agent` nodes via the Section 7 hook surface
  `inject_external_nodes(&mut self, agents: Vec<AgentRef>)` called by
  `GraphStateActor` after `project_topology` returns.

The Section 1 (GPU Physics) consumer receives the snapshot via
`GraphStateActor::GetGraphData` — the existing message path. The
`GraphTopology` replaces the current `GraphData` struct as the typed
input to the physics layer in a later phase (Phase 5); in Phase 2 the
topology is returned alongside the existing struct for parity verification.

---

## 6. GitHub Sync Flow Update (ADR-08 D7, D8, CC-15)

The sync flow update is T-14. Sequencing summary:

```
GitHubSyncService::sync_graphs()
  |
  +-- EnhancedContentAPI::list_markdown_files(&self.base_path)
  |     (base_path read from GITHUB_BASE_PATH at construction, not hardcoded "")
  |
  +-- for each file (batched, 50/batch):
  |     detect_file_type() -> KnowledgeGraph | Ontology | Skip
  |     if FORCE_FULL_SYNC=1: bypass sha1 gate
  |     else: compare file.sha1 against cached hash; skip if equal
  |
  +-- for each KnowledgeGraph file:
  |     DefaultKnowledgeGraphParser::parse(ParsedMarkdown) -> Vec<KnowledgeGraphEvent>
  |     apply events via OntologyRepository (Oxigraph adapter)
  |
  +-- for each Ontology file:
  |     DefaultKnowledgeGraphParser::parse(ParsedMarkdown) with is_public=false
  |     (OntologyBlock events emitted regardless of public:: true)
  |
  +-- emit TopologyRebuilt { cause: PostIngest }
  +-- trigger RunInference { profile: EL }
  +-- emit TopologyRebuilt { cause: PostInference }
```

CC-15 note: the GitHub adapter (Section 10) is the provider of `ParsedMarkdown`.
Phase 2 depends on Section 10's `ParsedMarkdown` value object being defined.
If Section 10 has not landed its `ParsedMarkdown` struct, Phase 2 defines a
local stub in `src/domain/value_objects.rs` (T-04 already does this) and
Section 10 adopts it when it lands.

`GITHUB_KG_PATH` verification: add to CI:
```bash
! grep -r 'GITHUB_KG_PATH' src/
```

---

## 7. Risk Register

### R-P2-01 — Cypher → SPARQL semantic drift in migration 0044 (HIGH)

The `ontologyTier` assignment on `main` may have been hand-tuned to values
that do not match the `COUNT(?mid)` heuristic in T-09. The ancestry-depth
approximation is a one-shot seed; if the rendered layout is wrong, an
operator must manually correct individual tiers via SPARQL UPDATE.

Mitigation: capture the existing Neo4j tier values as a CSV dump in
`queries/migrations/legacy/0044_ontology_tier_seed_values.csv` before
migration. Provide an operator runbook section: "If layout is wrong after
migration, restore from CSV using the provided SPARQL UPDATE template."

### R-P2-02 — OWL semantics drift in whelk-rs EL profile (MEDIUM)

The whelk-rs inference engine operates on OWL EL, which is decidable and
polynomially bounded. However, the asserted axiom set may contain constructs
outside EL (transitive properties, nominals) that whelk-rs silently ignores.
The inferred triple count may be lower than expected.

Mitigation: run whelk-rs over the bundled ontology fixture in a test and
record the expected triple count. If the count changes between runs, emit a
warning log. Document the EL profile restriction in the operator runbook.

### R-P2-03 — `LinkedPage` placeholder explosion on large corpus (MEDIUM)

With 998 files and ~199 public, the wikilink pass may create a large number
of `LinkedPage` placeholders for internal links. If the placeholder
population grows faster than upgrades, the topology has many
low-connectivity nodes that inflate the physics simulation.

Mitigation: the topology projection filters upgraded placeholders. Dangling
placeholders that remain after a full sync are visible in the operator
dashboard (T-16 parity test verifies isolation rate ≤ 5%).

### R-P2-04 — `Node.id: u32` and `NodeId` newtype coexistence creates two
type surfaces during the transition (MEDIUM)

Phase 2 introduces `NodeId` alongside the existing `Node.id: u32`. If a
developer writes new code using the old field, the compiler will not catch it
until an adapter boundary conversion is missing.

Mitigation: mark `Node.id` as `#[deprecated(note = "Use NodeId at adapter boundaries")]`
after T-02 lands. Add a CI clippy lint: `#[deny(deprecated)]` in
`src/adapters/` to force all adapter writers to notice. The domain types
(T-05 aggregates) use `NodeId` exclusively; the old field survives only in
`src/models/node.rs` until the Section 2/4 phases clean it up.

### R-P2-05 — Phase 1 slip gates T-05, T-06, T-11, T-12, T-14, T-15 (HIGH)

Six of the sixteen tasks in Phase 2 have hard dependencies on Phase 1
Oxigraph adapter methods. If Phase 1 slips, these six tasks cannot begin.

Mitigation: Phase 2 work that is Phase-1-independent (T-01 through T-04,
T-07 through T-10, T-13) is sequenced first and can proceed in parallel
with Phase 1. The Phase-1-dependent tasks are explicitly gated with [P1-DEP]
markers throughout this plan. Specialists may begin code-review and
fixture-writing for [P1-DEP] tasks without Phase 1, but cannot run
integration tests until Phase 1 adapter methods are non-stub.

---

## 8. Spawn Plan

When Phase 2 implementation begins, spawn three specialists in this order:

### Specialist 1 — ddd-domain-expert

Trigger: immediately at Phase 2 start, before any code is written.
Task: verify that the domain aggregate definitions in T-02, T-03, T-05,
and T-13 are faithful to DDD-08's ubiquitous language and invariants.
Specifically:
- Confirm `NodeId` class-bit allocation matches binary_protocol.rs constants
  after T-01.
- Confirm the five `NodeClass` variants in `GraphTopology` cover all DDD-08
  aggregate types with no omission.
- Confirm `EdgeKind` variants cover all relationship types in DDD-08 and
  the four migration semantics.
- Flag any invariant in DDD-08 (G1-G4, P1-P4, C1-C4, L1-L3) that the
  proposed structs do not enforce at compile time.
Output: a short written review (max 1 page) before T-05 proceeds to code.

### Specialist 2 — backend-dev

Trigger: after T-01 and T-03 are complete (flag constants and event enum stable).
Task: implement T-04, T-05, T-06, T-12, T-14 in that order.
Context to load:
- `src/services/parsers/knowledge_graph_parser.rs` (current state)
- `src/services/github_sync_service.rs` (current state)
- `src/ports/ontology_repository.rs` (current state)
- `src/adapters/neo4j_ontology_repository.rs` (reference for method signatures)
- This WORKTREE-PLAN.md (all task work descriptions)
Constraint: do not touch `binary_protocol.rs` (owned by T-01, already complete);
do not write any Cypher queries (T-07 to T-10 are doc-specialist territory).

### Specialist 3 — tester

Trigger: after T-05 port extension compiles (even with stub implementations).
Task: write T-11, T-16. Also write the unit tests specified in T-01
(mask-coverage) and T-02 (NodeId round-trip) if the backend-dev has not
already written them.
Context to load:
- `tests/ontology/` directory (current state)
- The four SPARQL `.rq` files from T-07 to T-10 (parity test input)
- PRD-08 §6 (acceptance evidence to gather)
Fixture requirement: construct a minimal fixture (20 pages, 6 classes,
60 subclass edges) that exercises all nine acceptance criteria (A1-A13)
without requiring the full 998-file GitHub corpus.

---

## Summary Statistics

| Metric | Value |
|--------|-------|
| Total tasks | 16 |
| S tasks (1 pt each) | 6 — T-01, T-03, T-07, T-08, T-09, T-10 |
| M tasks (3 pt each) | 8 — T-02, T-04, T-05, T-11, T-13, T-14, T-15, T-16 |
| L tasks (8 pt each) | 2 — T-06, T-12 |
| Total complexity points | 6×1 + 8×3 + 2×8 = 6 + 24 + 16 = **46 pts** |
| Tasks with hard Phase 1 dependency [P1-DEP] | 6 — T-05, T-06, T-11, T-12, T-14, T-15 |
| Specialists to spawn | 3 |

### The three tasks with the hardest Phase 1 dependencies

1. **T-06** — Migrate callers from `OwlClass` to unified domain aggregates.
   Requires Phase 1's `project_topology`, `upsert_page`, `upsert_ontology_class`,
   and `upsert_axiom` to be non-stub before integration tests can pass.

2. **T-12** — `OntologyClass` / `LinkedPage` unification in parser.
   Requires Phase 1's `upgrade_linked_page` to be implemented (not `todo!()`)
   before the placeholder-upgrade lifecycle can be tested end-to-end.

3. **T-15** — Wire whelk-rs inference into named graph.
   Requires Phase 1's `assert_inferred_triples` to write to
   `<urn:visionclaw:graph:ontology:inferred>` — verified by a Phase 1 test —
   before the topology union query returns inferred edges.
