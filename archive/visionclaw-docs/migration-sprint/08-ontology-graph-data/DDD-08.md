# DDD-08 — Knowledge Graph Bounded Context

## Bounded context

The **Knowledge Graph** bounded context owns the semantic model of the
Logseq corpus and its embedded ontology. It is sovereign over:

- Ingestion and parsing of Logseq markdown into domain objects.
- The class hierarchy of the ontology and the property graph among
  pages.
- Wikilink resolution and the lifecycle of placeholder references.
- Inferred triples produced by whelk-rs reasoning.
- The `GraphTopology` projection consumed by physics and rendering.

It does not own:

- Force-directed positions or kinetic state (Section 1 — GPU Physics).
- Position broadcast cadence to clients (Section 2 — Binary Protocol).
- Visual rendering of nodes, edges or labels (Section 4 — Rendering).
- Agent / bot lifecycle and telemetry (Section 7 — Bots & Telemetry).
- The triple store implementation (Section 11 — Persistence). This
  section depends on the `OntologyRepository` *port* at
  `src/ports/ontology_repository.rs`; the adapter is Section 11's
  responsibility.

## Ubiquitous language

| Term                | Definition                                                              |
|---------------------|-------------------------------------------------------------------------|
| **Page**            | A Logseq markdown file with `public:: true` in its frontmatter.         |
| **Journal entry**   | A dated file under `/journals/`; not a `Page` even if `public:: true` is set. Surfaces only as a metadata reference. |
| **Linked page**     | Placeholder node created when a wikilink target is unresolved; resolves into a `Page` or an `OntologyClass` on later parse runs. |
| **Ontology class**  | An OWL2 class declared by an `### OntologyBlock` with `ontology:: true` in a markdown file (regardless of page visibility). |
| **Ontology property** | An OWL2 object or datatype property declared in an `### OntologyBlock`. |
| **Axiom**           | A logical statement among ontology classes/properties: subclass-of, equivalent, disjoint, property restriction. |
| **Bridge**          | A `bridgeTo` relation that links a `Page` to an `OntologyClass` representing its subject. |
| **Render tier**     | A visual layering hint stored on a node; consumed by Section 4.         |
| **Wikilink**        | A `[[target]]` reference inside page prose. References are by label, not IRI. |
| **Public page**     | A `Page` whose source file has `public:: true`. Synonym of `Page`; introduced for clarity at the parser boundary where the gating decision is made. |
| **Defining page**   | The `Page` (if any) whose `### OntologyBlock` declared a given ontology class or property. |
| **OntologyBlock**   | A markdown section starting with `### OntologyBlock` that contains `ontology:: true`, `definition::`, `subclassOf::`, etc. The unit of ontology assertion. |
| **Topology**        | The `GraphTopology` projection: a flat node/edge view of the domain assembled for physics and rendering consumers. |
| **Asserted graph**  | Named graph holding triples directly produced by parsing.               |
| **Inference graph** | Named graph holding triples materialised by whelk-rs reasoning.         |
| **Class flag bits** | The high 6 bits of a `NodeId` encoding `NodeClass`.                     |
| **Sequence**        | The low 26 bits of a `NodeId`; allocated by an atomic counter per class. |

## Aggregates

### Aggregate root: `Graph`

`Graph` is the consistency boundary over the entire semantic graph. It
holds, by reference, the populations of the four content aggregates and
provides the projection operation that yields `GraphTopology`.

Held collections (conceptual; backed by the repository, not in-memory):

- `pages: Set<Page>`
- `ontology_classes: Set<OntologyClass>`
- `ontology_properties: Set<OntologyProperty>`
- `axioms: Set<Axiom>`
- `linked_pages: Set<LinkedPage>`  (placeholders only)
- `agents: Set<AgentRef>` (read-only reference; owned by Section 7)

Invariants:

- **G1** — Every IRI is unique across the union of pages, ontology
  classes, ontology properties, axioms, and linked-page placeholders.
- **G2** — Every wikilink reference resolves to *exactly one* node:
  a `Page`, an `OntologyClass`, or a `LinkedPage` placeholder. No
  wikilink is left dangling without a placeholder.
- **G3** — A `LinkedPage` placeholder is *transient*: on observation of
  a matching `Page` or `OntologyClass` declaration, the placeholder is
  upgraded (its ID is preserved; its type changes). The upgrade emits
  `LinkResolved`.
- **G4** — Every `Axiom` references only declared ontology classes /
  properties or placeholders. Asserting an axiom against an unknown
  term creates the term as a placeholder ontology class first.

Operations:

- `ingest_page(file: ParsedMarkdown)` → emits `PageIngested` events,
  possibly `OntologyClassDefined` / `AxiomAsserted` if the file has
  `### OntologyBlock` sections.
- `ingest_ontology_only(file: ParsedMarkdown)` → emits ontology events
  even when `public:: true` is absent.
- `resolve_wikilinks(page_id)` → for each `[[target]]` in the page,
  resolves to existing node or creates `LinkedPage`; emits `LinkResolved`.
- `assert_inference(triples)` → materialises whelk-rs output into the
  `inference` named graph; emits `InferenceMaterialised`.
- `project_topology() -> GraphTopology` → builds the consumer view.

### Page aggregate

```
Page {
    id: NodeId,                  // class bits = Page
    iri: Iri,                    // urn:visionclaw:page:<slug>
    slug: String,                // canonical from filename
    title: String,               // from frontmatter or first heading
    public: bool,                // always true for a Page; gates creation
    body_excerpt: String,        // first ~200 chars of prose for label hover
    wikilinks: Vec<WikilinkRef>, // outbound references
    defines: Vec<OntologyClassRef>, // classes declared in its OntologyBlocks
    metadata: PageMetadata,      // tags, journal, custom frontmatter
    content_sha1: ContentHash,   // for FORCE_FULL_SYNC bypass and incremental
}
```

Invariants:

- **P1** — `id.class() == NodeClass::Page`.
- **P2** — `iri` is unique within `Graph`.
- **P3** — `slug` matches the filename (sans extension) under
  `${GITHUB_BASE_PATH}/mainKnowledgeGraph/pages/`.
- **P4** — `public == true`; a non-public file does not become a `Page`.

### OntologyClass aggregate

```
OntologyClass {
    id: NodeId,                  // class bits = OntologyClass
    iri: Iri,                    // from OntologyBlock IRI assertion
    label: String,
    definition: String,          // from `definition::` field
    defined_in: Option<PageRef>, // host markdown file (may be private)
    subclass_of: Vec<OntologyClassRef>,
    equivalent_to: Vec<OntologyClassRef>,
    disjoint_with: Vec<OntologyClassRef>,
    property_restrictions: Vec<RestrictionRef>,
    annotations: Vec<Annotation>,
}
```

Invariants:

- **C1** — `id.class() == NodeClass::OntologyClass`.
- **C2** — `iri` is unique within `Graph`; if a `LinkedPage` placeholder
  carried this IRI, the placeholder upgrades (its `NodeId` survives the
  upgrade; only the class bits and aggregate type change — this is the
  one approved exception to the "ID is immutable" rule, and it fires
  `LinkResolved` to make the transition observable).
- **C3** — Every entry in `subclass_of` / `equivalent_to` /
  `disjoint_with` resolves to a `OntologyClass` (concrete or
  placeholder).
- **C4** — `defined_in` is `Some` if any `### OntologyBlock` in the
  corpus declared this class; the host page may have `public:: false`.

### OntologyProperty aggregate

```
OntologyProperty {
    id: NodeId,
    iri: Iri,
    label: String,
    kind: PropertyKind,            // Object | Datatype
    domain: Option<OntologyClassRef>,
    range: Option<OntologyClassOrDatatypeRef>,
    characteristics: PropertyCharacteristics,  // Functional, Transitive, ...
    defined_in: Option<PageRef>,
}
```

Invariants:

- **PR1** — `id.class() == NodeClass::OntologyProperty`.
- **PR2** — `domain` and `range` resolve to declared terms or
  placeholders.
- **PR3** — If `kind == Datatype`, `range` is a datatype IRI, not a class.

### Axiom aggregate

```
Axiom {
    id: NodeId,                   // class bits = Axiom
    iri: Iri,                     // minted from the asserted triple hash
    kind: AxiomKind,              // SubClassOf | EquivalentClass |
                                  // DisjointWith | PropertyRestriction |
                                  // PropertyAssertion
    subject: OntologyClassRef,
    predicate: OntologyPropertyRef | RdfPredicate,
    object: OntologyClassRef | LiteralValue,
    source: AxiomSource,          // Asserted { page_id } | Inferred { from_axioms }
}
```

Invariants:

- **A1** — `id.class() == NodeClass::Axiom`.
- **A2** — All referenced classes and properties exist (concrete or
  placeholder).
- **A3** — `source` distinguishes asserted from inferred; the named
  graph in Oxigraph matches.

### LinkedPage placeholder aggregate

```
LinkedPage {
    id: NodeId,                   // class bits = LinkedPage
    iri: Iri,                     // urn:visionclaw:linked:<normalized-label>
    label: String,                // the wikilink target text, normalised
    first_seen_in: PageRef,       // page that first referenced it
    upgraded_to: Option<UpgradeTarget>,  // None | Page(id) | OntologyClass(id)
}
```

Invariants:

- **L1** — `id.class() == NodeClass::LinkedPage`.
- **L2** — If `upgraded_to == Some`, the IRI lookup returns the upgrade
  target, not the placeholder. The placeholder record is kept for
  audit history but is excluded from the topology projection.
- **L3** — A `LinkedPage` is not created if a concrete `Page` or
  `OntologyClass` exists for the normalised label at the moment of
  resolution.

## Domain events

Emitted by the `Graph` aggregate; consumed by the topology projection,
the physics layer (via the projection), and the operator dashboard.

```
PageIngested {
    page_id: NodeId,
    iri: Iri,
    slug: String,
    wikilink_count: usize,
    ontology_block_count: usize,
    content_sha1: ContentHash,
}

PageRevised {
    page_id: NodeId,
    prior_sha1: ContentHash,
    new_sha1: ContentHash,
}

OntologyClassDefined {
    class_id: NodeId,
    iri: Iri,
    label: String,
    defining_page: Option<NodeId>,
    superclasses: Vec<Iri>,
}

OntologyPropertyDefined {
    property_id: NodeId,
    iri: Iri,
    kind: PropertyKind,
    domain: Option<Iri>,
    range: Option<Iri>,
}

AxiomAsserted {
    axiom_id: NodeId,
    kind: AxiomKind,
    subject_iri: Iri,
    object_iri: Iri,
    defining_page: Option<NodeId>,
}

LinkResolved {
    placeholder_id: NodeId,        // the LinkedPage that upgraded
    upgraded_to: NodeId,           // the Page or OntologyClass
    upgrade_kind: UpgradeKind,     // ToPage | ToOntologyClass
}

LinkRegistered {
    placeholder_id: NodeId,
    iri: Iri,
    referenced_in: NodeId,         // first-seen Page
}

BridgeEstablished {
    page_id: NodeId,
    class_id: NodeId,
}

InferenceMaterialised {
    triple_count: usize,
    profile: ReasoningProfile,     // EL | RL | QL
    elapsed_ms: u32,
}

TopologyRebuilt {
    node_count: usize,
    edge_count: usize,
    cause: RebuildCause,           // PostIngest | PostInference | ForceFullSync
}
```

The event log is the durable record of how the graph reached its current
state. It is the input the operator dashboard uses to surface dangling
placeholders, isolated nodes, and the inference run history.

## Commands accepted

- `IngestPage { parsed_markdown }` — runs the parser, ingests page +
  ontology blocks. Idempotent against SHA1.
- `IngestOntologyOnly { parsed_markdown }` — ingests ontology blocks
  regardless of host visibility.
- `ResolveWikilinks { page_id }` — resolves pending references; may
  emit `LinkRegistered` or `LinkResolved`.
- `RunInference { profile }` — invokes whelk-rs; materialises into the
  `inference` named graph.
- `ForceFullSync` — clears SHA1 short-circuits and re-issues `IngestPage`
  for every file under `GITHUB_BASE_PATH`. The flag is set via
  `FORCE_FULL_SYNC=1` env at the application boundary.
- `RebuildTopology` — recomputes the `GraphTopology` projection; emits
  `TopologyRebuilt`.

## Anti-corruption layers

### To Section 10 (GitHub adapter)

The GitHub adapter produces `ParsedMarkdown` value objects with these
fields: `path`, `frontmatter`, `prose_blocks`, `ontology_blocks`,
`outbound_wikilinks`, `sha1`. The domain receives these via the
`IngestPage` / `IngestOntologyOnly` commands. The domain never sees:

- Raw HTTP responses.
- `octocrab` types.
- Logseq-specific frontmatter syntax (the adapter normalises
  `public:: true` into `frontmatter.public: bool`).
- Wikilink double-bracket syntax (the adapter parses
  `[[Target Label]]` into `WikilinkRef { label: "Target Label",
  span: ByteRange }`).

If the corpus migrates off Logseq (theoretically), only the adapter
changes; the `ParsedMarkdown` value object stays stable.

### To Section 11 (Persistence)

The `OntologyRepository` port at `src/ports/ontology_repository.rs` is
the only contract between this section and the triple store. The port
exposes domain operations (`upsert_page`, `upsert_class`,
`upsert_axiom`, `project_topology`, `assert_inferred_triples`,
`fetch_page_by_iri`, etc.). The adapter (Section 11) implements the
port against Oxigraph + SPARQL Update. The domain never builds SPARQL
strings or sees `oxrdf` types.

This separation makes the Neo4j → Oxigraph migration a Section 11
exercise; the domain model is invariant under the swap.

### To Section 1 (GPU Physics)

The physics layer consumes the `GraphTopology` projection as a
read-only snapshot. The physics layer never reads the ontology
repository directly and never writes back to the graph. Position
updates from physics flow through Section 2's broadcast layer, not
back into this section's aggregates.

### To Section 7 (Bots & Telemetry)

Agent and bot nodes are *injected* into the `GraphTopology`
projection by Section 7; they are not parsed from markdown. The
projection includes a hook `inject_external_nodes()` that Section 7
calls with its agent set. Bots never own ontology classes or
axioms; they reference them by IRI when needed (e.g. "this agent
handles all `:Cybernetics` queries").

## Bugs and smells at the reset point (41979d33e)

- Baseline already has `OwlClass` and `linked_page` as separate
  records. The unification under D1 (ADR-08) is a forward change.
- Baseline does not have the `LinkedPage → Page` upgrade event. It
  treats unresolved wikilinks by creating `linked_page` records that
  stay separate even when a matching `Page` later appears. The
  upgrade lifecycle in this DDD is a forward change.
- Baseline's `KnowledgeGraphParser` writes to the repository inline.
  D5 (ADR-08) separates parser-as-domain-service from repository
  writes; the parser becomes pure-functional in this DDD.
- Baseline's `OntologyBlock` parsing already runs regardless of
  `public:: true` (per the memory entry). This DDD codifies that
  behaviour; it is not a forward change but it is *documented* here.
- Baseline emits no domain events; state mutations go straight to
  Neo4j. The event taxonomy in this DDD is a forward change and
  ties into ADR-11's persistence model.
