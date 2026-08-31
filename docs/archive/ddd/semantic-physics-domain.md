# DDD: Semantic Physics Domain (Ontology Rigour + Constraint-Driven Layout)

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-05) |
| Drives | PRD-018, ADR-098, ADR-099, ADR-100, ADR-101 |
| Companion DDD | `clustering-analytics-domain.md` (GPU-index/logical-id ACL pattern reused here) |

## Context

The Semantic Physics domain is the chain that turns authored OWL ontology into forces the GPU actually applies, and back into something a user can see and query. Today that chain is built but disconnected: axioms are parsed and stored, the GPU has a live constraint loop, and the client has reasoning panels — but the seams between them are no-op stubs, downgrades, and NULL metadata. This document fixes the ubiquitous language and boundaries so the reuse-first implementation (PRD-018) lands coherently rather than as five disjoint patches.

The governing constraints: **all solving is GPU-resident** (no client-side layout), and **we wire existing tooling** rather than building new subsystems.

## 1. Bounded Context Definition

### The "Semantic Physics" context

Owns the transformation `authored ontology → reasoned axioms → GPU constraints → applied forces` and the read-side `stored/inferred axioms → exploration UI`. It does **not** own the generic force-directed layout (that is the GPU Physics context's spring/repel/centre/gravity), nor the raw markdown ingestion (the Ingest context), nor clustering/analytics (the Graph Analytics context). It is the semantic *overlay* on top of generic physics.

### Neighbouring contexts and relationships

| Neighbour | Relationship | Boundary artefact |
|---|---|---|
| **Ingest** (`github_sync_service`, `jsonld_ingest`) | upstream supplier | emits canonical IRIs (ADR-100) + populated MetadataStore |
| **Triple Store** (Oxigraph/RocksDB) | shared kernel (named graphs) | `urn:ngm:graph:ontology:{assert,inferred}`, `:migrations` |
| **Reasoning** (whelk-rs EL) | upstream supplier | materialised inferred axioms with provenance (ADR-099) |
| **GPU Physics** (`visionclaw_unified.cu`) | downstream consumer via ACL | `ConstraintData[]` device buffer + `ENABLE_CONSTRAINTS` |
| **Graph Analytics** | sibling (independent buffers) | shares the GPU-index/logical-id ACL (below) |
| **Exploration UI** (client ontology feature) | downstream conformist | inferred-graph reads, SPARQL results, `graph_type` filter |

### The OWL-axiom vs GPU-ConstraintData anti-corruption boundary

The central ACL of this domain. OWL axioms speak in IRIs and logical relations (`rdfs:subClassOf`, `owl:disjointWith`, `owl:sameAs`). The GPU speaks in `ConstraintData { kind, node_idx[4], params[8], weight, activation_frame }` over **node indices**. The mapper (ADR-098 D1) is the anti-corruption layer: it resolves IRIs to node indices through the stable IRI→node map (ADR-100), chooses a `ConstraintKind`, and packs rest-length/strength into `params`. Axioms whose endpoints do not resolve are counted and logged — never silently dropped (the historical 30–50% drop is a boundary failure, not acceptable loss). This mirrors the GPU-index/logical-node-id ACL already documented in the Graph Analytics domain.

## 2. Ubiquitous Language Glossary

| Term | Meaning | Lives in |
|---|---|---|
| **Asserted axiom** | A triple authored in the source ontology | `urn:ngm:graph:ontology:assert` |
| **Inferred axiom** | A triple derived by the reasoner, provenance-tagged | `urn:ngm:graph:ontology:inferred` |
| **Canonical IRI** | `vc:{domain}/{slug}`, deterministic, diacritic-preserving | ADR-100 |
| **IRI→node map** | Total, stable mapping from canonical IRI to GPU node index | Ingest/MetadataStore |
| **Constraint** | A semantic force expressed over node indices | `src/models/constraints.rs` |
| **ConstraintData** | The 4-node/8-param GPU-resident form of a Constraint | `visionclaw_unified.cu:99` |
| **ENABLE_CONSTRAINTS** | Feature-flag bit gating the live constraint loop | `feature_flags` bit 4 |
| **Constraint ramp** | Progressive activation over `constraint_ramp_frames` | live `force_pass_kernel` |
| **Reasoning report** | The listable set of inferred axioms with justification | `InferencePanel` read-model |
| **Well-known vocabulary** | A registered upstream ontology with PURL + namespace | ADR-100 D4 registry |

## 3. Aggregates, Entities, and Value Objects

### Aggregate: `OntologySnapshot` (root)
The consistent set of asserted + inferred axioms at a sync generation, identified by `versionIRI`. Invariant: inferred axioms reference only IRIs present in the asserted set or the well-known registry; the inferred graph is wholly derivable and may be cleared and rebuilt.

### Aggregate: `ConstraintSet` (root)
The `Vec<Constraint>` derived from an `OntologySnapshot` for upload. Invariant: every `Constraint` resolves all its `node_idx` to live nodes at upload time; `activation_frame` is stamped at upload; the set is replaced wholesale on `OntologyModified`, never partially mutated on the GPU.

### Value Object: `Constraint`
`{ kind: ConstraintKind, node_idx: [i32;4], params: [f32;8], weight: f32 }`. Immutable; `to_gpu_format()`/`to_gpu_data()` are its only serialisers (reuse existing).

### Entity: `InferenceRun`
A reasoner invocation (whelk) producing inferred axioms; carries a run id used as `prov:wasGeneratedBy` on every inferred quad.

### Value Object: `VocabularyRegistration`
`{ prefix, namespaceIRI, purl }` from the well-known-vocabulary registry.

## 4. Domain Events

| Event | Emitted when | Consumers |
|---|---|---|
| `OntologyModified` | asserted axioms change (post-sync) | Reasoning (trigger `InferenceRun`), Semantic Physics (rebuild `ConstraintSet`) |
| `AxiomsInferred` | an `InferenceRun` materialises into the inferred graph | Exploration UI (reasoning report, inferred edges) |
| `ConstraintsUploaded` | `ConstraintData[]` pushed + `ENABLE_CONSTRAINTS` set | GPU Physics (live kernel begins applying with ramp) |
| `MigrationApplied` | a SPARQL migration records into `urn:ngm:graph:migrations` | Triple Store (startup ledger) |

## 5. Invariants and Policies

1. **No silent axiom loss.** Unresolved axiom endpoints are logged and counted; coverage ≥95% is a release gate (ADR-100).
2. **ABI is frozen.** `sizeof(SimParams)==172`; semantic forces flow only through the separate `ConstraintData[]` buffer (ADR-098). No new `SimParams` field.
3. **Inferred is derivable.** The inferred graph can always be cleared and regenerated from asserted + reasoner; "clear inferred" is a one-graph `CLEAR` (ADR-099).
4. **GPU-only solving.** No layout solving on the client; the SPARQL console is read-only server-side SELECT (PRD-018 binding constraint).
5. **Reuse-first.** New code is a mapper, two handler bodies, panel mounts, registry, and migrations — not new kernels or a new reasoner. Redundant kernels are deleted (ADR-098 D3).
6. **Determinism.** Slugs and node IDs are deterministic across runs; collisions rejected (ADR-100 D2).

## 6. Anti-Corruption and Boundaries

- **OWL→ConstraintData ACL** (ADR-098 D1): the only place IRIs become node indices for physics. Single, tested function.
- **Reasoner downgrade boundary removed** (ADR-099 D2): `EquivalentClass` is no longer rewritten to `SubClassOf` at the boundary; equivalence crosses intact.
- **rdf:type classification boundary** (ADR-100 D3): typing crosses from Ingest as explicit `rdf:type`, not substring inference.
- **Migration boundary** (ADR-101): all structural triple-store rewrites cross through the versioned migration ledger.

## 7. Context Map Sketch

```
 Ingest ──(canonical IRIs + MetadataStore.source_domain)──▶ Triple Store (assert)
   │                                                              │
   │                                                   OntologyModified
   ▼                                                              ▼
 (ADR-100 IRI→node map)                                   Reasoning (whelk EL)
   │                                                              │ AxiomsInferred
   │                                                              ▼
   └──────────────▶  OWL→ConstraintData ACL  ◀──── Triple Store (inferred)
                            │ ConstraintSet                       │
                            ▼ ConstraintsUploaded                 ▼
                     GPU Physics (live kernel,             Exploration UI
                     ConstraintData[] + ENABLE_CONSTRAINTS)  (InferencePanel,
                     ── applied forces, ramped ──▶ render     OntologyBrowser,
                                                              SPARQL console,
                                                              graph_type filter)
```

## Consequences

- The domain is wired end-to-end through one ACL and four events; each workstream (PRD-018 WS-0..WS-5) maps to a boundary or aggregate here, so the partitioned implementation agents have disjoint, well-named seams.
- Because semantics ride the separate `ConstraintData[]` buffer, the domain composes with Graph Analytics without ABI contention (both use the GPU-index/logical-id ACL, distinct buffers).

## Alternatives Considered

- **New per-edge-type constant-memory force params (ADR-072) as the primary mechanism** — rejected for WS-3 in favour of reusing the live `ConstraintData[]` path; kept as a deferred tuning layer (ADR-098 D4).
- **Adopting OWL 2 DL (Konclude) as primary reasoner** — rejected; incompatible with incremental sync. DL stays an offline audit (ADR-099 D5).
- **Client-side layout for exploration UX (ontosphere's Dagre/ELK/Cola)** — rejected by the GPU-only-solving constraint; exploration UI is presentation + server-side query only.
