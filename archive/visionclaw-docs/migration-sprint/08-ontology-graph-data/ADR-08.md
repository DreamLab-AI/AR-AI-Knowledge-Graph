# ADR-08 — Ontology & Graph Data

Status      : Proposed
Date        : 2026-05-16
Supersedes  : Cypher migrations `0042 .. 0045` (translated, not replayed)
Related     : ADR-01 (Physics), ADR-07 (Bots & Telemetry), ADR-11 (Persistence)

## Context

The data model on `main` evolved from a Logseq-markdown-only design into
something with three intertwined vocabularies:

1. **Page vocabulary.** `page` nodes for `public:: true` markdown files;
   `linked_page` nodes generated from `[[wikilink]]` targets that may or
   may not correspond to existing public pages.
2. **Ontology vocabulary.** `OwlClass`, `OwlObjectProperty`,
   `OwlDatatypeProperty`, axioms (subclass, equivalence, restriction)
   produced from `### OntologyBlock` sections in markdown — *regardless*
   of host page visibility.
3. **Bot/agent vocabulary.** `agent` and `bot` nodes injected by Section 7,
   not produced by parsing.

The accident is that vocabularies 1 and 2 are *not unified*. A
`linked_page` node labelled `"Cybernetics"` and an `OwlClass` node
labelled `"Cybernetics"` are two records, two IDs, two render passes,
two physics positions. The `SUBCLASS_OF` edges among ontology classes are
filtered out before reaching the client graph (623 of them), leaving 62%
of nodes isolated in the client view. Migration `0045` on `main` renames
`OwlClass` to `OntologyClass` — a useful step, but a rename does not heal
the underlying duplication.

The persistence migration (Section 11) replaces Neo4j with Oxigraph + RDF
triples. That is a *good moment* to fix the model rather than translate
the accident. ADR-08 sets the canonical model; ADR-11 documents the
adapter that realises it.

## Decision

### D1. One concept per thing: `OntologyClass` subsumes `linked_page`

The dual-record pattern is rejected. There is a single domain type per
identifiable thing in the corpus:

- `Page` — a Logseq markdown file with `public:: true`. Has an IRI, body
  prose, metadata, and outbound wikilinks.
- `OntologyClass` — a class in the ontology. Has an IRI, a label, a
  natural-language definition, asserted axioms (subclass of, equivalent
  to, disjoint with, property restrictions), and a *defining page* if any.
- `OntologyProperty` — an object or datatype property. Has an IRI,
  domain, range, and characteristics (functional, transitive, etc.).
- `Axiom` — an assertion of a logical statement (e.g. `Cybernetics ⊑
  Systems`). Has an IRI minted from the asserted triple.
- `Agent` — injected by Section 7. Surfaces in the same graph but is not
  parsed from markdown.

What was a `linked_page` node is now either: (a) a `Page` if the wikilink
resolves to an existing public page, (b) an `OntologyClass` if the
wikilink resolves to an asserted class by label, or (c) a *placeholder
node* — domain type `LinkedPage` — created on first reference and
upgraded to `Page` or `OntologyClass` when a future parse run discovers
the definition. The placeholder is the only legitimate use of the old
`linked_page` label, and its lifecycle has a defined endpoint (upgrade
or stay dangling — and dangling is visible in the operator dashboard).

### D2. Graph topology is the projection, not the source

The physics layer (Section 1) and the rendering layer (Section 4)
consume a `GraphTopology` projection. The projection is built from the
domain aggregates by a single query against the ontology repository:

```
GraphTopology {
    nodes: Vec<TopologyNode { id: NodeId, class: NodeClass, label: String,
                              mass: f32, definition_summary: Option<String> }>,
    edges: Vec<TopologyEdge { source: NodeId, target: NodeId,
                              kind: EdgeKind, weight: f32 }>,
}
```

`NodeClass` is `{ Page, OntologyClass, OntologyProperty, LinkedPage,
Agent }`. `EdgeKind` is `{ Wikilink, SubClassOf, EquivalentClass,
PropertyAssertion, DefinedIn, BridgeTo, AgentControls }`.

The 623 `SUBCLASS_OF` edges are present in the projection by
construction: the SPARQL query that builds the topology selects
`?s rdfs:subClassOf ?o` over both
`<urn:visionclaw:graph:ontology:assert>` and
`<urn:visionclaw:graph:ontology:inferred>` named graphs (per ADR-11 §D2)
and emits a `SubClassOf` edge for each binding. The filtering
that produced the gap on `main` is rejected outright; there is no
filter step between repository and topology.

### D3. `OntologyBlock` parsing is independent of `public:: true`

Two filters at the parser boundary:

- `is_public_page(file) := file.metadata.public == true` — gates `Page`
  node creation and the wikilink-extraction pass for the file's prose.
- `has_ontology_blocks(file) := file.blocks.any(b.is_ontology_block)` —
  gates `OntologyClass` / `OntologyProperty` / `Axiom` event emission
  for the file's ontology blocks.

The two are evaluated independently. A non-public page with ontology
blocks produces ontology events; its host page is recorded as
`definedIn(class, host_page_iri)` even when no `Page` node is created
for the host. This matches the 199-of-998-public-files reality of the
corpus while keeping the ontology surface complete.

### D4. The four Cypher migrations are translated, not replayed

Migration set `0042 .. 0045`:

- `0042` — adds `bridge_to` relations between knowledge and ontology
  graphs.
- `0043` — adds `renderTier` property to nodes (visual layering hint).
- `0044` — adds `ontologyTier` property and seeds the layout assignment.
- `0045` — renames `OwlClass` to `OntologyClass`.

All four translate to SPARQL Update operations against the Oxigraph
target. `0045` is a no-op for *us*: under D1 there is no `OwlClass`
relation to rename — there is only `OntologyClass` from day one. The
operation is preserved in the migration record for replay-compatibility
with any deployment that retained the old vocabulary, but our forward
path skips it. Translation table:

| Cypher migration | Forward SPARQL Update                           |
|------------------|-------------------------------------------------|
| `0042` bridge_to | `INSERT { ?a :bridgeTo ?b } WHERE { ... }`      |
| `0043` renderTier| `INSERT { ?n :renderTier ?t } WHERE { ... }`    |
| `0044` ontoTier  | `INSERT { ?n :ontologyTier ?t } WHERE { ... }`  |
| `0045` rename    | no-op (model is unified from D1)                |

Migrations live in `queries/migrations/*.rq` (SPARQL Update files) after
the move; the Cypher files are retained under `queries/migrations/legacy/`
for archaeology.

### D5. `KnowledgeGraphParser` is a domain service

`src/services/parsers/knowledge_graph_parser.rs` is reclassified as a
domain service in the hexagonal model — it depends on no adapter; it
emits domain events. Its signature becomes:

```rust
pub trait KnowledgeGraphParser: Send + Sync {
    fn parse(&self, file: ParsedMarkdown) -> Vec<KnowledgeGraphEvent>;
}
```

`ParsedMarkdown` is a value object built upstream from a raw file by the
GitHub adapter (Section 10). `KnowledgeGraphEvent` is the domain event
enumeration (see DDD-08). The repository receives events through the
application layer, not through the parser.

### D6. `NodeId` is a newtype with explicit conversions

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct NodeId(u32);

impl NodeId {
    pub fn new(seq: u32, class: NodeClass) -> Self { ... }
    pub fn class(&self) -> NodeClass { ... }   // decode high 6 bits
    pub fn sequence(&self) -> u32 { ... }      // mask out the high 6 bits
    pub fn as_u32(&self) -> u32 { self.0 }
}

impl From<NodeId> for String { ... }   // explicit, named "wire form"
impl TryFrom<&str> for NodeId { ... }  // fallible parse, anchored on u32
```

Class bits encode `NodeClass` in the top 6 bits of the `u32` id:
`0x80000000 = Agent` (bit 31), `0x40000000 = Page` (bit 30). The
ontology region uses the mask `ONTOLOGY_TYPE_MASK = 0x1C000000`
(bits 26-28) with these allocations: `0x04000000 = OntologyClass`,
`0x08000000 = LinkedPage` (placeholder), `0x0C000000 = Axiom`,
`0x10000000 = OntologyProperty`. Values `0x14000000`, `0x18000000`,
`0x1C000000` are reserved for future ontology subtypes. The remaining
26 bits (`NODE_ID_MASK = 0x03FFFFFF`) are the per-class sequence,
allocated by the atomic counter in `GraphStateActor`. Per DDD-08 §C2
the sequence is stable across a `LinkedPage → Page` or
`LinkedPage → OntologyClass` upgrade — only the class bits change.

This mirrors the constants in `src/utils/binary_protocol.rs:16-27`,
which are the canonical wire-format source of truth.

The wire form (numeric `u32`) is used in the binary protocol and the
WebSocket payloads. The string form is used in JSON payloads and in
client-side `Map` keys. Conversions are explicit and happen *only* at
adapter boundaries; the domain never sees a raw `u32` or a raw `String`.

### D7. `GITHUB_BASE_PATH` is the only path env var

`GITHUB_KG_PATH` was dead and stays removed. `GitHubSyncService` reads
`GITHUB_BASE_PATH` once at construction and passes it as a parameter
into `EnhancedContentAPI::list_markdown_files`. The hardcoded `""` call
site is removed; if a caller wants the base path it asks for it.

### D8. `FORCE_FULL_SYNC=1` bypasses SHA1, not events

`FORCE_FULL_SYNC=1` causes the parser to re-parse every file and the
repository to *re-emit* every event, regardless of SHA1 incremental
gating. It does not cause the repository to *delete and recreate*
records; events are upserts, idempotent by IRI. Re-emission is enough to
flush the topology projection downstream.

### D9. Whelk-rs inference materialised into the inferred named graph

The Oxigraph dataset uses two named graphs:
`<urn:visionclaw:graph:ontology:assert>` for asserted triples and
`<urn:visionclaw:graph:ontology:inferred>` for whelk-rs-derived
inferences (see ADR-11 §D2). Whelk-rs runs over the asserted axioms
(OWL EL profile) and writes its output into the inferred named graph.
The default `GraphTopology` query unions the two graphs. When
attribution matters (e.g. operator tooling), a query parameter switches
the source.

### D10. Anti-corruption layer at the GitHub adapter boundary

The GitHub adapter produces a `ParsedMarkdown` value object with prose
content, parsed frontmatter, parsed ontology blocks, and outbound
wikilink references. The domain never sees `octocrab::Response` or raw
markdown strings. This isolates the corpus-format quirks (Logseq's
`public:: true`, `### OntologyBlock`, double-bracket wikilink syntax)
from the domain model. If the corpus migrates off Logseq (theoretically),
only the adapter changes.

## Options considered

### O1. Rename only: `OwlClass` → `OntologyClass`, keep dual records

Rejected. The rename is the wedge but not the change. Keeping
`OntologyClass` and `linked_page` as separate records perpetuates the
edge-gap bug and forces filter logic between repository and topology.
A renamed dual table is still a dual table.

### O2. Materialise `linked_page` placeholders as full ontology classes

Rejected. A `[[wikilink]]` to a freshly-typed page name should not
imply a class assertion. Wikilinks are referential, not definitional.
The `LinkedPage` placeholder class in D1 keeps the placeholder distinct
from a *declared* class until the declaration is observed.

### O3. Single domain concept with optional aspects (this ADR)

Adopted. `Page`, `OntologyClass`, `OntologyProperty`, `LinkedPage`,
`Axiom` are five domain types, projected into a single `GraphTopology`
node set. The projection is the single source of truth for the physics
and rendering layers. Subclass edges are present by construction.

### O4. Keep Cypher migrations, run them against an Oxigraph-backed
adapter

Rejected as architecturally incoherent. Cypher migrations are property-
graph operations; Oxigraph is an RDF triple store. The migrations
translate to SPARQL Update; running Cypher against an SPARQL store
would require an emulation layer with its own bug surface.

## Risks

- **R1**. The unification under D1 means every existing call site that
  reads `linked_page` records or `OwlClass` records must be audited.
  Mitigation: the topology projection (D2) is the single read path; the
  audit is mechanical — `grep` for `linked_page` and `OwlClass` and
  rewrite each site to use the projection.

- **R2**. The whelk-rs inference run may materialise more triples than
  Oxigraph can hold in memory on a small dev box. Mitigation: scope the
  inference to the `OntologyClass` and `OntologyProperty` subgraphs;
  the EL profile is well-bounded.

- **R3**. Translation of migration `0044` (ontology tier layout
  assignment) depends on per-class heuristics that may have been tuned
  by hand on `main`. Mitigation: capture the existing tier assignments
  as data, not as logic; the SPARQL Update inserts the tiers from a
  dump.

- **R4**. The `LinkedPage` placeholder introduces a third life stage
  (placeholder → upgraded). Mitigation: explicit domain event
  `LinkResolved` (see DDD-08) marks the upgrade; the operator dashboard
  shows the placeholder population so dangling references are visible.

- **R5**. `FORCE_FULL_SYNC=1` re-emits every event; on a large corpus
  this may saturate the event channel. Mitigation: the event channel
  has a backpressure budget; the sync service blocks on emit when the
  budget is exhausted. The operation is not interactive; latency is
  acceptable.

## Rejected from main as buggy / unjustified

- `(synthetic) filter SUBCLASS_OF before topology emit` — the source of
  the ontology edge gap. Removed by D2.
- `(synthetic) GITHUB_KG_PATH env read` — dead code. Stays dead per D7.
- `(synthetic) String coercion of node ID at fetchInitialData but
  numeric at others` — partial fix to a typing problem. Replaced by D6
  newtype.
- Migration `0045` as a *forward* operation — the rename is a no-op
  under the unified model.

## Bugs and smells at the reset point (41979d33e)

- The baseline has the `OwlClass` / `linked_page` dual representation
  from inception. D1 is a forward change, not a rollback restoration.
- The baseline ships a `KnowledgeGraphParser` at
  `src/services/parsers/knowledge_graph_parser.rs` that mixes parsing
  with repository writes. D5 separates the two.
- The baseline reads `GITHUB_BASE_PATH` and `GITHUB_KG_PATH`. D7
  preserves the former and finalises removal of the latter.
- `FORCE_FULL_SYNC=1` was already documented on `main` but with unclear
  semantics around event re-emission. D8 makes the semantics explicit.
- The `NodeId` newtype does not exist yet at baseline; node IDs are raw
  `u32` throughout. D6 introduces it as a forward change.
- Cypher migrations `0042 .. 0045` exist on `main` but not at baseline.
  D4 translates them on the forward path.
