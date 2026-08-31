# ADR-100 — Canonical IRI Scheme, rdf:type Classification, and Vocabulary Alignment

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-05) |
| Drives | PRD-018 §5 WS-0, WS-1, WS-5, §6.3 |
| Companion ADRs | ADR-098 (constraint reuse — depends on stable IRI→node map), ADR-099 (reasoner), ADR-101 (migrations) |
| Affected paths | `crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs`, `crates/visionclaw-ontology/src/services/jsonld_ingest/{expander,canonical,triple_emitter}.rs`, `src/services/github_sync_service.rs`, vocab-registry module (new) |
| Evidence | `oxigraph_ontology_repository.rs:96-146`, `github_sync_service.rs:170,208,554`, `expander.rs:455,551,554`, PRD-018 §2.2, §3 |

## Context

The RDF layer mints identifiers and classifies entities by bespoke, fragile means:

- **String-sniffed classification.** Class vs individual is decided by `iri.contains(":class:")` (`github_sync_service.rs:170,208`). This breaks on any IRI shape that does not embed the literal substring and is not driven by `rdf:type`.
- **ASCII-only slugifier.** `expander.rs:554` drops diacritics silently, so `Café` and `Cafe` collide and non-Latin titles degrade.
- **Ad-hoc IRIs.** `urn:ngm:class:<slug>` / `urn:visionclaw:linked:<slug>` minting (`oxigraph_ontology_repository.rs:96-146`, `expander.rs:551`) with no domain namespacing, no PURL base, no `versionIRI`, and no upstream-vocabulary alignment.
- **No imports.** No `owl:imports`, no well-known-vocabulary registry; SKOS/PROV-O/FOAF/BFO terms are not resolved to their canonical namespaces.
- **Non-deterministic node IDs.** `DefaultHasher` over content gives different IDs across runs → collisions and broken IRI→node resolution (30–50% of axioms drop their endpoints).

ADR-098's constraint mapper and ADR-099's inference materialisation both **require a stable, total IRI→node map**: an axiom can only become a GPU constraint or a rendered inferred edge if its subject and object resolve to node indices. So the IRI scheme is a hard prerequisite, not cosmetic. Ontosphere's curated ~55-entry well-known-ontology registry (with namespace URIs, PURLs, and `owl:imports` auto-discovery) is the model to adapt.

The reuse directive applies: keep the existing named-graph discipline (`urn:ngm:graph:*`) and the existing content-addressed axiom IRIs; formalise and stabilise the rest.

## Decision

### D1 — Canonical IRI scheme `vc:{domain}/{slug}`

Entities mint as `vc:{domain}/{slug}` where `vc:` is the existing `https://narrativegoldmine.com/ns/v1#` base (or a PURL alias under D4), `{domain}` is the node's registered domain (ADR-domain registry), and `{slug}` is the deterministic slug (D2). Axiom IRIs keep their existing content-addressed `sha256-12` form (already deterministic — reuse). The ontology document carries a `versionIRI` bumped per sync generation. Existing `urn:ngm:graph:*` named graphs are unchanged.

### D2 — Deterministic, diacritic-preserving slugifier

Replace the ASCII-dropping slugifier with NFKD normalisation + deterministic transliteration (preserve information: `Café`→`cafe` only via an explicit, reversible transliteration table, with the original `rdfs:label` retained verbatim on the node). Identical inputs always produce identical slugs across runs and machines. Node IDs derive from a **deterministic, seeded hash** (not `DefaultHasher`); collisions are detected, logged, and rejected — never silently merged.

### D3 — `rdf:type`-based classification, not substring sniffing

Class/property/individual typing is read from `rdf:type` assertions (`owl:Class`, `owl:ObjectProperty`, `owl:NamedIndividual`, …), emitted explicitly by the ingest triple-emitter. The `iri.contains(":class:")` path (`:170,208`) is removed. The triple-emitter continues to emit both `owl:Class` and the `vc:OntologyClass` marker (reuse — existing dual emission), but classification consumers read `rdf:type`.

### D4 — Well-known-vocabulary registry + `owl:imports` auto-discovery

A curated registry (adapted from ontosphere's well-known-ontology table) maps prefixes to canonical namespace URIs and **PURLs**: RDF/RDFS/OWL/XSD/SKOS/PROV-O/FOAF/BFO 2020/OBO. On ingest, `owl:imports` are auto-discovered and the referenced vocabularies resolved via their PURLs (cached). Core vocab terms used in the corpus resolve to these canonical IRIs rather than ad-hoc local mints. `skos:exactMatch`/`closeMatch` alignment hooks to Wikidata/DBpedia are recorded where the source ontology provides them (per the 2026-05-31 ontology-audit synthesis).

### D5 — MetadataStore is the source of `{domain}`

WS-0 populates the MetadataStore so every node carries a non-NULL `source_domain` (the empty-MetadataStore bug is fixed here, not worked around). `{domain}` in D1 reads from this store. Acceptance: ≥95% of nodes have non-NULL `source_domain`; ≥95% of parsed axioms resolve subject+object to a node.

## Consequences

**Positive:**
- Stable, total IRI→node map unblocks ADR-098 (constraints) and ADR-099 (inferred edges) — without it both are meaningless.
- Standards-aligned identifiers and imports make the corpus interoperable and round-trippable (WS-1 serialisers).
- Determinism kills the node-ID collision class and the 30–50% axiom-endpoint drop.
- All within the existing named-graph and content-addressed-axiom discipline (reuse).

**Negative / risks:**
- Re-minting IRIs changes identifiers; handled by an ADR-101 migration that rewrites existing subjects/objects in one transaction (no dangling references) and is idempotent.
- PURL resolution adds a network dependency at ingest; mitigated by caching resolved vocabularies and treating resolution failure as non-fatal (vocab terms degrade to local mints with a logged warning).
- Transliteration tables are locale-sensitive; the original `rdfs:label` is always retained so no information is lost.

## Verification

- Unit: slugifier is deterministic and diacritic-preserving across a fixture of Latin/non-Latin titles; node-ID hash is stable across runs; collisions are rejected with a log line.
- Integration: ≥95% `source_domain` coverage and ≥95% axiom-endpoint resolution on the live corpus; `rdf:type` drives classification (no `:class:` substring path remains).
- Vocab: `owl:imports` auto-loads SKOS/PROV-O/FOAF/BFO via PURLs; core terms resolve to canonical namespaces.

### D4 implementation status (PRD-018 WS-5, 2026-06-05)

Implemented in `crates/visionclaw-ontology` (ingest/validation path; reuse-first):

- **Well-known-vocabulary registry** — `services/vocab_registry.rs`. `VocabularyRegistration { prefix, namespace_iri, purl }` value object + `by_prefix` / `by_namespace_iri` / `by_purl` / `resolve_import_target` / `all` lookups. Covers the D4 minimum set (RDF, RDFS, OWL, XSD, SKOS, PROV-O, FOAF, BFO 2020, OBO, DCTERMS) plus `schema`/`sh`. Namespace IRIs are imported from `jsonld_ingest::expander`'s `*_NS` constants (no divergent re-declaration); only the PURL column and XSD/BFO/OBO entries are added here. Tests: `registry_covers_adr100_minimum_set`, `prefix_namespace_purl_roundtrip_is_deterministic`, `namespace_iris_reuse_expander_constants`, `resolve_import_target_handles_purl_namespace_and_fragment`, `unknown_prefix_returns_none` (5/5 pass).
- **`owl:imports` auto-discovery + PURL resolution** — `services/jsonld_ingest/vocab_resolver.rs`. `discover_imports(&ExpandedDocument)` finds every `owl:imports` target; `VocabResolver<F: VocabFetcher>` resolves each via the registry and a cache. The fetch layer is the injectable `VocabFetcher` trait (`ReqwestFetcher` in prod, `StubFetcher` in tests). **Resolution is non-fatal by construction**: a registry miss → `ImportResolution::LocalMint` (warn); a registry hit with fetch failure → `ImportResolution::NamespaceOnly` (warn); success → `Resolved { cached }`. Ingest is never blocked on the network. Tests: `discovers_owl_imports_target`, `success_path_loads_terms_from_stub_fixture`, `failure_path_degrades_to_namespace_only_without_erroring`, `unknown_import_degrades_to_local_mint_without_erroring` (4/4 pass, all offline via stub).
- **`skos:exactMatch` / `skos:closeMatch` alignment hooks** — emitted in `services/jsonld_ingest/triple_emitter.rs::emit_alignment` (single documented emission point). Source-provided alignments to Wikidata/DBpedia are captured+emitted verbatim as object-property triples; alignments are never synthesised. Test: `emits_skos_alignment_to_wikidata` (1/1 pass).
- **SHACL gate** — `services/jsonld_ingest/shacl_gate.rs` wires the EXISTING `jsonld_validator::shacl_lite` engine (no second engine) into `pipeline::parse_and_emit`. Produces a `ShaclGateReport { violations, shapes_checked }` surfaced on `IngestOutcome::shacl_report`; `is_valid()` is the gate decision. The schema/profile validator still runs first as the hard gate; the SHACL gate adds a non-short-circuiting shape report. Tests: `valid_ontology_class_passes_gate`, `shape_violating_block_is_reported`, `gate_handles_graph_array`, `shacl_gate_passes_for_valid_block_and_surfaces_report`, `shacl_gate_passes_for_valid_ontology_class` (5/5 pass).

Deviation note: D4 says terms "resolve to canonical namespaces rather than ad-hoc local mints". The implementation keeps the canonical namespace on *both* the `Resolved` and `NamespaceOnly` outcomes (only the fetched document body is lost on a network failure), so a fetch failure does not regress IRIs to local mints — only an *unregistered* import degrades to a local mint. This is a strict superset of the ADR intent and matches the non-fatal mandate. `cargo check -p visionclaw-ontology` green; the WS-5 path adds no GPU/`github_sync_service` coupling.
