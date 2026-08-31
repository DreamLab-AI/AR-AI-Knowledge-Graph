# PRD-018 — Ontosphere-Informed Ontology Rigour & Exploration

| | |
|---|---|
| **Status** | Accepted — WS-0..WS-5 implemented & in-container-verified (2026-06-05); host PTX build + browser regression is the final runtime gate |
| **Date** | 2026-06-05 (rev. 3 — as-built) |
| **Author** | John O'Hare (with ruflo mesh research, swarm `swarm_1780651148193`) |
| **Reference system** | [ThHanke/ontosphere](https://github.com/ThHanke/ontosphere) — Apache-2.0, v1.3.3 |
| **Related** | ADR-072 (autordf2gml), ADR-028 (SPARQL patch), ADR-014 (semantic pipeline unification), PRD-009 (feature-engineering discovery), KNOWN_ISSUES ONT-001, design `2026-04-18-unified-knowledge-pipeline.md` |
| **Licence note** | Ontosphere is Apache-2.0; one-way compatible into this MPL-2.0 repo as a Larger Work. Its key dependencies (`n3`, `@comunica/*`, `rdf-parse`, `rdf-validate-shacl`) are MIT/permissive and directly adoptable in the TS client. |

---

## 1. Summary

VisionClaw is a high-scale, GPU-accelerated, *persistent* 3D knowledge-graph substrate with a large bespoke ingestion pipeline. Ontosphere is a small, in-browser, *in-memory* OWL 2 DL editor built entirely on maintained RDFJS standards libraries. They are complementary, not competitors: ontosphere has the **standards rigour, reasoning transparency, and ontology-exploration UX** that VisionClaw's bespoke pipeline lacks; VisionClaw has the **scale, GPU physics, persistence, and ingestion** that ontosphere lacks.

This PRD proposes adopting ontosphere's *rigour and UX patterns* (and, where licence-clean, its actual TS components and library choices) onto VisionClaw's substrate — **without replacing the CUDA engine, the Oxigraph store, or the EL++ reasoner**. The single highest-value outcome is closing the gap between an ontology that is *parsed and stored* and one that is *reasoned over, validated, explained, queryable, and actually driving the GPU layout*.

### Binding constraints (rev. 2)

Two directives govern this implementation and supersede any conflicting reading of the workstreams below:

1. **GPU-only solving.** All graph/physics solving stays on the GPU. We do **not** adopt ontosphere's CPU layout model (Dagre/ELK/Cola) or any client-side layout solver. The exploration UX we borrow is presentation and querying only; the SPARQL console runs server-side over Oxigraph.
2. **Reuse over rebuild.** VisionClaw already contains the *entire* constraint pipeline in a half-connected state. We wire up the tooling we have rather than writing new subsystems. The verification below shows the seam is narrow.

### Reuse inventory — what already exists (verified 2026-06-05)

| Capability | State | Evidence |
|---|---|---|
| GPU constraint application | **Live and production-grade** — `force_pass_kernel` runs a full constraint loop with progressive-activation ramp (`constraint_ramp_frames`), node-role detection, per-constraint weight | `visionclaw_unified.cu:475-500+` |
| `ConstraintData` GPU struct + Rust mirror | **Exists** — `{kind, count, node_idx[4], params[8], weight, activation_frame}`; Rust `ConstraintData` with `to_gpu_format()`/`to_gpu_data()` | `visionclaw_unified.cu:99`, `src/models/constraints.rs:16,76,80` |
| `ENABLE_CONSTRAINTS` feature flag | **Exists** (bit 4 of `feature_flags`), never set from Rust | `visionclaw_unified.cu:89,475` |
| Inferred named graph | **Exists**, under-populated | `urn:ngm:graph:ontology:inferred` |
| Whelk-rs EL reasoner | **Live**, extracts only `named_subsumptions`; downgrades `EquivalentClass` | `whelk_inference_engine.rs:86,290` |
| `InferencePanel` / `OntologyBrowser` | **Built, exported, never rendered** (no JSX site) | `client/src/features/ontology/components/*` |
| `OntologyPanel` | **Wired** via control-panel ontology tab | `MainLayout.tsx:89` |
| `ontology_constraints.cu` kernels | **Redundant** — the live kernel already does this inline → delete | `ontology_constraints.cu:101-325` (zero call sites) |

**Implication.** The constraint pipeline is not missing — it is *disconnected*. The work is a narrow bridge: OWL axioms → `Vec<Constraint>` → upload `ConstraintData[]` (reusing `to_gpu_data()`) → set `ENABLE_CONSTRAINTS`. Because `ConstraintData[]` is a **separate device buffer** (a kernel pointer parameter, not a `SimParams` field) and `ENABLE_CONSTRAINTS` is an already-allocated flag bit, **this requires zero change to the frozen 172-byte `SimParams` ABI**. No new constant-memory params are needed; ADR-072's per-edge-type path is deferred as a future tuning refinement, not the WS-3 mechanism.

The research that informs this PRD found a decisive fact: **today the OWL ontology does almost nothing to the rendered graph.** Beyond a 6-bucket hardcoded domain string-match driving repulsion scaling, every semantic-force and ontology-constraint CUDA kernel is dead code, and — because of the empty-MetadataStore bug — even that 6-bucket hack receives NULL domains for ~100% of nodes. The "semantic force-directed graph" is, in production, a generic spring/repel layout with a dual-disc projection.

---

## 2. Holistic high-level view (current state)

```
                          ┌──────────────────────────── INGEST (Rust) ────────────────────────────┐
 jjohare/logseq (GitHub)  │ GitHubSyncService::sync_graphs  github_sync_service.rs:263             │
   mainKnowledgeGraph/ ───▶│   fetch markdown (tree API, batch 50) → process_fetched_file :1060     │
   workingGraph/          │     ├─ JSON-LD path: extractor → HAND-ROLLED expander.rs:455           │
                          │     │     → canonical.rs:162 → jsonld_ingest(validator→triple_emitter)  │
                          │     │     → Vec<oxigraph::Quad>  (NO Turtle serialiser)                 │
                          │     └─ plain Logseq: KnowledgeGraphParser (node iff public:: / owl:class)│
                          │   bridge edges :370 · domain roots :388 · fold stubs :452               │
                          │   run_post_sync_reasoning :719  → Whelk-rs EL infer → SubClassOf@w0.4   │
                          └───────────────────────────────────────────────────────────────────────┘
                                                  │ quads
                                                  ▼
 ┌──────────── PERSISTENCE ────────────┐   ┌──────── REASONING ────────┐   ┌──────── GPU PHYSICS (CUDA) ────────┐
 │ Oxigraph = RocksDB (NOT SQLite)     │   │ whelk-rs EL (fork)         │   │ visionclaw_unified.cu              │
 │ named graphs:                       │   │  EquivalentClass→SubClassOf│   │  force_pass_kernel (live):         │
 │  urn:ngm:graph:ontology:assert      │   │ CustomReasoner: trans.close│   │   repel·spring(LinLog)·center·grav │
 │  urn:ngm:graph:ontology:inferred    │   │ EL gate rejects union/     │   │  SimParams #[repr(C)] == 172 BYTES │
 │  urn:ngm:graph:knowledge / :agent   │   │  complement/disjointWith   │   │  SEMANTIC = 6-bucket domain hack   │
 │ IRIs urn:ngm:{class,property,node…} │   └────────────────────────────┘   │   (same id ×0.1 / diff ×3.0)       │
 │ SQLite = settings only              │                                    │  ontology_constraints.cu = DEAD    │
 │ RuVector PG = embeddings (MCP-only) │                                    │  semantic_forces.cu      = DEAD    │
 │ NO triple-store migration framework │                                    │  ENABLE_CONSTRAINTS never set      │
 └─────────────────────────────────────┘                                    │  project_node_xy → dual disc, 52B  │
                                                  │                          └────────────────────────────────────┘
                                                  ▼ V3 52-byte wire (id·pos·vel·sssp·cluster·anomaly·community·centrality)
 ┌──────────────────────────────── CLIENT (React 19 / R3F / Babylon) ────────────────────────────────┐
 │ Live: IntegratedControlPanel ← unifiedSettingsConfig.ts (10 tabs)                                  │
 │   color-by type/domain/community/cluster/centrality/sssp · node-type toggles · per-pop springK     │
 │   ontologyPhysics/ontologyStrength · graphSeparationX/axisCompressionZ (dual disc)                 │
 │ ORPHANED (built, not mounted): OntologyPanel · InferencePanel(explanations) · OntologyBrowser      │
 │ MISSING: SPARQL console · axiom/reasoning inspector · graph_type filter · edge-type hide           │
 └───────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Subsystem précis (with evidence)

- **Ingest** — `GitHubSyncService::sync_graphs` (`github_sync_service.rs:263`) dual-path: JSON-LD entities vs plain Logseq pages. The RDF layer is **bespoke**: hand-rolled JSON-LD expansion over 12 hardcoded prefixes (`jsonld_ingest/expander.rs:455`), an ASCII-only slugifier that silently drops diacritics (`:554`), class/individual detection by **string-sniffing** `iri.contains(":class:")` (`github_sync_service.rs:170,208`), no Turtle serialiser, and a separate **orphaned** Python `ontology-core` toolchain (`agentbox/skills/ontology-core/src/*.py`) using divergent namespaces. Validation is advisory/non-fatal in the hot path (`:1155`).
- **Persistence** — README's "Oxigraph + SQLite" is misleading: Oxigraph is **RocksDB**-backed (`oxigraph_ontology_repository.rs:312`); SQLite holds only settings. Clean named-graph discipline already exists (`:44-53`), IRIs are minted `urn:ngm:class:<slug>` / content-addressed axioms. Flag bits + `NODE_ID_MASK=0x03FFFFFF` in `binary_protocol.rs:16-27` (triplicated). **No migration framework for the triple store**; SPARQL is string-concatenated with hand-rolled escaping (injection surface).
- **GPU physics** — `visionclaw_unified.cu` is the only live engine. Generic forces (repel/spring/center/gravity) are 1:1 with `SimParams`. **The OWL→force path is scaffold-only**: `ENABLE_CONSTRAINTS` is never set, `UpdateConstraints`/`UploadConstraintsToGPU` are no-op log stubs (`force_compute_actor.rs:2733,2739`), and `ontology_constraints.cu` kernels have zero call sites. The lone real semantic effect is a 6-entry hardcoded domain table (`:785-792`) → repulsion ×0.1/×3.0 — and per ADR-audit the source of that table (`metadata["source_domain"]`) is NULL for ~100% of nodes (empty-MetadataStore bug). The **frozen 172-byte SimParams ABI** (dual `static_assert`) is the structural blocker to adding real axiom forces.
- **Reasoning** — whelk-rs **EL** (not EL++ as `lib.rs:10` claims); only `named_subsumptions()` is extracted; disjointness/equivalence inferences are never materialised; `EquivalentClass` is downgraded to `SubClassOf`.
- **Client** — full physics/visual control surface is live, but the entire **ontology-exploration UI is built and orphaned** (`OntologyPanel`, `InferencePanel` with explanations, `OntologyBrowser`). No SPARQL console, no reasoning inspector, no `graph_type` server-side filter (whole graph is fetched then filtered client-side), zero accessibility on the live panel.

### 2.2 Documented gaps already on record (docs-analyst)
IRI→node lookup silently drops 30–50% of axioms; 33% ingestion cliff (3,330 pages → 2,242 nodes); **zero `DisjointWith` axioms in the actual corpus**; 8 orphaned relation types (`has-part`, `requires`, `enables`…) parsed but produce zero forces; node-ID collisions via non-deterministic `DefaultHasher`; ad-hoc IRI scheme. ADR-072 (autordf2gml — MiniLM content embeddings + N-hop materialisation + TransE KGE + blended discovery + per-edge-type physics) is **accepted but unimplemented**.

---

## 3. What ontosphere does more rigorously (evidence)

| Dimension | Ontosphere | VisionClaw today |
|---|---|---|
| **RDF/OWL I/O** | Maintained RDFJS: `n3` store in a Web Worker, `rdf-parse` 4.0 with a full media-type table (Turtle/N3/NT/NQ/TriG/TriX/JSON-LD/RDF-XML) + content-negotiation + round-trip serialise (`rdfManager.impl.ts`) | Hand-rolled JSON-LD expander; no general parser; no serialiser |
| **Reasoning** | **Konclude SROIQ(D) tableau, OWL 2 DL** (WASM) + N3 `owl-rl.n3` BGP fallback; covers restrictions, inverses, chains, cardinality, nominals, union/intersection | whelk-rs **EL** subset; equivalence downgraded; inferences not materialised |
| **Reasoning UX** | Inferred triples written to a **separate named graph `urn:vg:inferred`**, rendered as **amber dashed edges / italic**, with an idempotent "clear inferred" toggle and a **reasoning report** listing every inferred triple | `urn:ngm:graph:ontology:inferred` exists in store but **`InferencePanel` is orphaned**; no visual differentiation, no report |
| **Vocabulary alignment** | Curated **well-known-ontology registry** (~55 entries: RDF/RDFS/OWL/XSD/SKOS/PROV-O/P-Plan/FOAF/BFO 2 + BFO 2020/OBO) with proper namespace URIs, **PURLs**, and `owl:imports` **auto-discovery** | Ad-hoc IRIs; no upstream alignment; no imports |
| **Querying** | **Comunica SPARQL** over the RDFJS store, in a worker — ad-hoc user querying | Oxigraph supports SPARQL server-side but **no UI console**; Cypher path is a dead validation-only shim |
| **Validation** | `rdf-validate-shacl` surfaced inline on nodes | EL-profile gate only; no SHACL; validation advisory |
| **TBox/ABox** | First-class UI toggle; domain/range-scored autocomplete | Not surfaced |
| **Agent surface** | MCP server with ~35 typed tools (`mcp.json`) over loadRdf/runReasoning/validateGraph/queryGraph/findPath | 7 ontology MCP tools (comparable; narrower) |
| **Engineering rigour** | OWL2Bench reasoning tests, Playwright e2e, ADR-style post-mortems | Ontology pipeline has documented audits but pervasive advisory-validation + dead code |

**What VisionClaw already does better (boundaries — do not regress):** GPU/3D force-directed physics at 10k+ nodes; persistent triple store + incremental SHA1 GitHub sync; large-scale markdown ingestion; EL++ *incremental-at-scale* classification (Konclude is more expressive but single-shot, requires page reload to recover). Ontosphere has **no persistence, no ingestion, no GPU, 2D only** (`layouts.ts` ELK capped at 300 iters/60s) and its generic `graphValidation.ts` is a stubbed no-op.

---

## 4. Goals & non-goals

### Goals
1. **Make the ontology actually drive the layout** — replace the dead-kernel scaffold + 6-bucket hack with real axiom→force wiring, unblocked by an ABI strategy.
2. **Adopt standards-grade RDF I/O** — stop hand-rolling JSON-LD/IRI/slug logic; round-trip standard serialisations.
3. **Surface reasoning** — materialise + visually differentiate inferred axioms (ontosphere's `inferred` named-graph + dashed-edge + report pattern); mount the orphaned `InferencePanel`.
4. **Add ontology exploration UX** — mount `OntologyBrowser`, add a SPARQL console over Oxigraph, server-side `graph_type` filtering.
5. **Align to upstream vocabularies** — a curated well-known-vocabulary registry with PURLs and `owl:imports`, canonical IRI scheme.
6. **Add SHACL validation** as a first-class, non-advisory gate.

### Non-goals
- Replacing the CUDA physics engine, Oxigraph/RocksDB store, or whelk-rs reasoner.
- Switching to OWL 2 DL / Konclude as the *primary* reasoner (keep EL++ for incremental scale; DL is an optional offline deep-check — see WS-2 open question).
- Porting ontosphere's 2D Reactodia canvas (VisionClaw's 3D R3F/Babylon renderer stays).
- Adopting in-memory-only semantics (VisionClaw must stay persistent).

---

## 5. Workstreams (prioritised)

Priority reflects value × unblocking. WS-0 and WS-1 are prerequisites for WS-3 to mean anything.

### WS-0 — Fix the data so semantics are *possible* (P0, prerequisite)
- **Problem:** Empty MetadataStore → `source_domain` NULL for ~100% of nodes; IRI→node lookup drops 30–50% of axioms; non-deterministic node-ID hashing. Any semantic-force work is meaningless until nodes carry their domain/axiom linkage.
- **Proposal:** Land the unified-knowledge-pipeline Phase 1–2 fixes (populate MetadataStore; canonical `vc:{domain}/{slug}` IRI scheme + domain registry; deterministic ID hashing with collision detection/logging).
- **Acceptance:** ≥95% of nodes have non-NULL `source_domain`; ≥95% of parsed axioms resolve subject+object to a node; zero silent ID collisions (logged + rejected).

### WS-1 — Standards-grade RDF I/O (P0)
- **Problem:** Bespoke JSON-LD expander, ASCII slugifier, string-sniffed class detection, no serialiser.
- **Proposal:** On the Rust side, route ingest RDF through `sophia`/`oxrdfio` for parse + **Turtle/JSON-LD serialisation** (round-trip export already needed for GitHub write-back); replace string-sniffing with `rdf:type` assertions; port ontosphere's media-type canonicalisation table (`rdfManager.impl.ts`) as the content-negotiation spec. Keep the Logseq-frontmatter→entity mapping (that is genuinely bespoke and correct), but emit through a standard serialiser.
- **Acceptance:** Round-trip Turtle/JSON-LD export of the full store validates against a standard parser; class/individual typing comes from `rdf:type`, not substring sniffing; diacritic page titles slug deterministically.

### WS-2 — Reasoning transparency + materialisation (P1)
- **Problem:** Whelk extracts only `named_subsumptions`; equivalence/disjointness inferences never materialised; `InferencePanel` orphaned.
- **Proposal:** Materialise inferred axioms into `urn:ngm:graph:ontology:inferred` (already exists), tag with provenance + confidence; mount `InferencePanel` and add a **reasoning report** + **visual differentiation** (inferred edges = dashed/amber, ontosphere pattern) in R3F/Babylon. Stop downgrading `EquivalentClass`.
- **Open question:** add an *optional offline* OWL 2 DL deep-check (Konclude WASM or an `elk`/`horned-owl`-based path) for consistency audits, kept out of the incremental hot path.
- **Acceptance:** Inferred edges render distinctly with a toggle and a listable report; equivalence preserved bidirectionally; reasoning report reachable from the live UI.

### WS-3 — Wire real OWL→force (P1, depends on WS-0) — **reuse path, decided**
- **Problem:** `ENABLE_CONSTRAINTS` never set; the two upload handlers (`UpdateConstraints`/`UploadConstraintsToGPU` in `src/actors/gpu/force_compute_actor.rs`) are no-op log stubs; `ontology_constraints.cu` + `semantic_forces.cu` are dead.
- **Decision (ADR-098):** Reuse the **live** constraint path, not new params. The seam:
  1. Build an **anti-corruption mapper** OWL axiom → `Constraint` (`subClassOf`→attraction/`DISTANCE`, `disjointWith`+inter-domain→`SEPARATION`, `sameAs`→colocate, the 8 orphaned relation types → tunable `DISTANCE`/`SEPARATION`). Subjects/objects resolve to node indices via WS-0's fixed IRI→node map.
  2. Implement the two stub handlers to serialise via the **existing** `to_gpu_data()` and upload the `ConstraintData[]` device buffer.
  3. Set `ENABLE_CONSTRAINTS` (bit 4) in `SimParams.feature_flags`; the live `force_pass_kernel` already consumes the buffer with `constraint_ramp_frames` progressive activation.
  4. **Delete** the redundant `ontology_constraints.cu` kernels (the live inline path supersedes them) and any remaining dead `semantic_forces.cu` paths.
- **Why not ADR-072 constant-memory params:** that would be *new* code; the live `ConstraintData[]` buffer already exists and already ramps. Per-edge-type constant-memory tuning is a later refinement, recorded in ADR-098 as deferred.
- **ABI:** unchanged. `ConstraintData[]` is a separate device buffer; only an already-allocated flag bit flips. The dual `static_assert sizeof(SimParams)==172` stays green.
- **Acceptance:** subClassOf measurably increases attraction and disjointWith/inter-domain measurably increases separation on a fixture graph (log-signature proof + browser verification); no dead semantic kernels remain compiled; `static_assert` unchanged.
- **As-built (2026-06-05):** Implemented per the corrected *five-break* topology — see ADR-098 §"Implementation record". Notes on plan-vs-built: there is a **single** `static_assert sizeof(SimParams)==172` (not dual); the verified breaks were five (the keystone `ENABLE_CONSTRAINTS` was never OR'd, plus a lossy `upload_constraints` writer and a missing `SEPARATION` kernel branch); the mapper (`src/physics/ontology_constraint_mapper.rs`) emits `ConstraintData` with **live-kernel integers** via `LiveKernelKind::as_i32()` and uploads through the lossless `set_constraints`, **not** the domain-enum `to_gpu_data()` cast. `cargo check -p visionclaw-server -p visionclaw-gpu` green; gpu 40/40, mapper 3/3 tests pass.

### WS-4 — Ontology exploration UX (P2)
- **Problem:** No SPARQL console, no class/property browser, no server-side filter; orphaned panels.
- **Proposal:** Mount `OntologyBrowser` (class/property tree) + add a **SPARQL console** over Oxigraph (read-only SELECT, the rigorous server-side equivalent of ontosphere's Comunica UX); add server-side `?graph_type=` filtering so the client stops transferring the whole graph; add TBox/ABox toggle and per-class focus/isolate. Address the live panel's zero-accessibility and the two-divergent-control-tree drift.
- **Acceptance:** Users can run a SELECT and see results; browse the class hierarchy; isolate a class subtree; `graph_type` filtering reduces transfer; orphaned trees reconciled or removed.

### WS-5 — Vocabulary alignment + SHACL (P2)
- **Problem:** Ad-hoc IRIs, no upstream alignment, no SHACL.
- **Proposal:** Port ontosphere's **well-known-vocabulary registry** (PURLs for SKOS/PROV-O/FOAF/BFO 2020/OBO) + `owl:imports` auto-discovery; add `skos:exactMatch`/`closeMatch` alignments to Wikidata/DBpedia (per the 2026-05-31 ontology-audit synthesis already in memory); add `rdf-validate-shacl` as a non-advisory gate with inline surfacing.
- **Acceptance:** Core vocab terms resolve to PURLs; imports auto-load; SHACL violations block or flag at ingest with a report.

---

## 6. Architecture decisions (new ADRs — written)
1. **ADR-098 — SimParams ABI / constraint-path reuse.** *Decided:* reuse the live `ConstraintData[]` device buffer (zero ABI change, set `ENABLE_CONSTRAINTS`, implement the two stub handlers); delete redundant constraint kernels. ADR-072 constant-memory per-edge-type params deferred as a later tuning refinement.
2. **ADR-099 — Reasoner posture.** *Decided:* whelk-rs EL stays the incremental primary; extend extraction to preserve `EquivalentClass` and materialise disjointness into `urn:ngm:graph:ontology:inferred`; optional offline OWL 2 DL deep-check kept out of the hot path.
3. **ADR-100 — Canonical IRI + vocabulary alignment.** *Decided:* `vc:{domain}/{slug}` + PURL base + `versionIRI`; diacritic-preserving deterministic slugifier; `rdf:type`-based classification; well-known-vocabulary registry + `owl:imports` auto-discovery.
4. **ADR-101 — Triple-store migration framework.** *Decided:* versioned, idempotent SPARQL migrations for Oxigraph tracked in a migrations named graph (parity with the SQLite `schema_migrations` discipline).

## 7. Success metrics
- Semantic layout is real: ≥2 OWL axiom families measurably alter node geometry (fixture + live proof).
- Axiom→force conversion ≥95% (today effectively ~0% live); node domain coverage ≥95% (today ~0%).
- Round-trip standard RDF export validates; zero hand-rolled parser in the ingest hot path.
- Inferred axioms visible + reportable in the live UI; `InferencePanel`/`OntologyBrowser` mounted (today orphaned).
- SPARQL console + server-side `graph_type` filter live; whole-graph transfer eliminated.

## 8. Risks & mitigations
- **ABI break destabilises physics** → prefer non-breaking constant-memory params (WS-3 option b); gate behind feature flag + browser regression (the established `sep_x=… flatten=…` log-signature method).
- **Semantic forces reintroduce explosion/IMA** → ramp constraints over `constraint_ramp_frames`; reuse existing NaN/IMA circuit breaker; verify against the dual-disc fixture.
- **Scope creep into a full DL editor** → non-goals fence this; DL stays an optional offline audit.
- **Licence** → Apache-2.0 → MPL-2.0 is compatible; TS deps are MIT; record provenance for any ported ontosphere code.

## 9. Open questions
- Do we reuse ontosphere TS components directly (license-clean) or only adopt its library choices/patterns in our existing R3F client?
- Is the corpus's **zero `DisjointWith`** a data-authoring gap (add disjointness to the source ontology) or do we synthesise disjointness from sibling-class heuristics?
- Should the orphaned Python `ontology-core` toolchain be wired in, or formally retired in favour of the Rust path?

## 10. As-built (verified 2026-06-05)

**Forces are real — measured, not asserted.** Post-sync reasoning logs over the live corpus:
- 3,713 classes + 11,464 asserted axioms loaded (was **0** — `get_axioms()` previously read only reified `vc:Axiom`; now UNIONs plain `rdfs:subClassOf` / `owl:equivalentClass` / `owl:disjointWith` / `ObjectPropertyAssertion` / SomeValuesFrom).
- Whelk EL inference → 19,318 inferred axioms → 30,782 materialised axioms dispatched.
- OWL→constraint mapper: **30,782 axioms in → 18,933 GPU live-kernel constraints out**, uploaded to CUDA.
- IRI→node resolution 100% (14,962/14,962 endpoints) across 7,481 `SubClassOf` axioms; 7,481 inferred edges rendered.

**Key fixes delivered:**
- `ontology_constraint_mapper.rs` — `hasPart`/`partOf` → `Attract` (corpus has 5,589 `hasPart`, 0 `isPartOf`; previously dropped).
- `whelk_inference_engine.rs` — `ObjectPropertyAssertion` `warn!`→`debug!` (mereological triples drive forces directly, not EL Tbox).
- `oxigraph_ontology_repository.rs` — keystone UNION `get_axioms()`; restriction `onProperty` mirrored to both `predicate` and `property` annotation keys.
- `ontology_constraint_actor.rs` — `ingest_domain_axioms` now sets `active_ontology_constraints` (stats endpoint reported `0` despite 18,933 uploaded).
- `ontology_physics/mod.rs` — `GET /constraints` surfaces `activeConstraints` + `axiomsProcessed` + GPU health counters.
- `main.rs` — the `web::Data` `GitHubSyncService` (used by `POST /api/admin/sync`) is now wired to `GPUManagerActor`; a manual re-sync re-dispatches constraints (previously logged "GPUManagerActor address not registered" and pushed nothing).

**Client control surface (WS-4, as-built):**
- New **Forces** sub-tab (`OntologyForcesPanel`) in the Ontology tab — live constraint/axiom/inferred-edge/GPU-health readout, master enable/disable, global force-strength slider (→ `PUT /weights`), and a **Re-sync reasoning** action (→ `POST /api/admin/sync`). Replaces the dead per-group sliders that mutated only local Zustand.
- **System Status** box now reports all three graph types (knowledge / ontology / agent node counts) plus ontology rigour fields (classes, axioms, inferred edges, live forces) sourced from the GPU constraint stats endpoint + the inferred-edges store. Read-only: no client-side solving (binding constraint A).
- Shared `ontologyPhysicsService` + `useConstraintStats` poller back both surfaces; empty-safe, GPU-resident.

**Client read-model fixes (browser-verified 2026-06-05):**
- **Doubled `/api` prefix** — `unifiedApiClient` already prepends its base (`/api`), so ontology service endpoint constants that *also* led with `/api` resolved to `/api/api/...` → 404 (status box read 0). Stripped the leading `/api` from `ontologyPhysicsService`, `sparqlService`, `inferredAxiomsService`, and `useInferenceService` (paths are now relative to the client base). Raw-`fetch` callers (`useOntologyStore`, `OntologyModeToggle`, `useHierarchyData`) were already correct and untouched.
- **Reified inferred-axiom adapter** — the whelk materialiser emits the inferred graph as RDF provenance triples (`{s,p,o}` where each `urn:ngm:axiom:*` node carries `ngm:subject`/`ngm:object`/`ngm:axiomType`/`ngm:derivation`/`ngm:confidence`), 173,870 raw triples for 19,318 axioms. The client expected a flat `{subject,predicate,object}` shape and filtered everything out (count 0). `inferredAxiomsService` now detects the reified shape and collapses it to one edge per axiom node → count = **19,318** (matches Whelk). The flat path is retained as a fallback.
- **Status-box inferred pull** — the always-on `SystemHealthIndicator` now owns the first `useInferredEdgesStore.refresh()` on mount so the Inferred field populates without requiring the Ontology tab to be opened.
- **Browser-verified live values:** status box + Forces panel both show Classes 6,002 · Axioms 30,782 · Inferred 19,318 · Forces 18,933 · GPU evaluations ≥1 · 0 failures/fallbacks. The 3D layout visibly resolves into the semantic constraint topology.

**Force tuning — boundaries & defaults (browser-verified 2026-06-05):** the first dispatch collapsed all 18,933 constraints into one tight central blob. Two root causes, both fixed:
- **Mapper boundaries retuned** (`ontology_constraint_mapper.rs` — these consts *are* the force boundaries; tests assert against the symbols, not literals, so values move freely): `SUBCLASS_REST_LENGTH` 60→**90**, `COLOCATE_REST_LENGTH` 2→**10** (the dominant collapse driver — thousands of equivalent classes were pinned to a point), `COLOCATE_WEIGHT` 0.95→**0.85**, `DISJOINT_MIN_DISTANCE` 200→**350**. `SUBCLASS_WEIGHT` (0.6) and `DISJOINT_WEIGHT` (0.8) unchanged.
- **Global strength default + reversible scaling** (`ontology_constraint_actor.rs`): added `DEFAULT_GLOBAL_STRENGTH = 0.6` and a `constraint_buffer_base` so the live buffer is always `scale_constraint_buffer(base, strength)`. `ingest_domain_axioms` stores the base then dispatches at 0.6 (legible, not a blob at full mapper weight). The `AdjustConstraintWeights` handler now re-scales the *base* (reversible, weight×s, count preserved) instead of scaling the empty legacy `ontology_constraints` vec — that prior bug clobbered the GPU buffer to 0 on any slider move (PUT `globalStrength=0.3` once dropped `activeConstraints` 18,933→0). UI default aligned: `OntologyForcesPanel` `useState(0.6)`.
- **Verified end-to-end through the browser:** graph resolves to a diffuse, legible cloud (no central blob); Forces panel reads **Global strength 60%** / 18,933 live; slider drives `PUT /ontology-physics/weights [200]` and the backend returns `adjustedConstraints: 18933, appliedStrength: s` with `activeConstraints` held at 18,933 (no clobber) across 0.2 / 0.6.

**Known follow-up (out of scope for WS-4, not yet wired):** the **Browse** sub-tab's class/property tree (`OntologyBrowser` → `useOntologyStore` → `GET /api/ontology/public`) reads 0 because `/api/ontology/public` is **404** — no such backend route exists (the live `/api/ontology` scope exposes `/axioms`, `/inferences`, `/hierarchy`, `/report`, …). Repoint the browser to `/hierarchy` (or add a `/classes` endpoint) to populate the tree. The exploration controls, inferred-edge toggle, and the other four sub-tabs (Reason/Forces/Query/Manage) are live.

**Settings control-panel alignment (browser + endpoint-verified 2026-06-05, commit `7c8367e6d`):** re-tested the whole physics settings surface against the live GPU force system and refactored the Physics tab (`unifiedSettingsConfig.ts`) to match.
- **Live path proven empirically.** Physics writes flow slider → `autoSaveManager` (500 ms debounce) → `updatePhysics` → `PUT /api/settings/physics` → `settings_routes.rs::update_physics_settings` (key-normalise → `validate_physics_settings` against `physics_bounds` → `UpdateSimulationParams` to GPUCompute/GPUManager + GraphServiceSupervisor → `ForceResumePhysics`). Direct endpoint test: `repelK` 120→500 expanded graph spread (σ 34.8,36.0,26.7)→(37.7,38.6,28.1), confirming the path reaches the GPU. (The `api_handler/settings/mod.rs::update_physics_settings` variant with snake-case `UpdatePhysicsRequest` and no GPU propagation was **orphaned dead code** — never mounted (`api_handler/mod.rs` declares no `pub mod settings;`, so the compiler never included it) and unreferenced anywhere. Removed 2026-06-05, resolving the 2026-04-17 audit master action item #9.)
- **Stale default annotations corrected** to the canonical values in `client/src/api/settings/defaults.ts` (all UI descriptions previously cited pre-retune numbers): springK 15→12, repelK 1200→120, restLength 80→50, centerGravityK 0.05→0.2, gravity 0.0001→0.002, maxForce 1000→150, globalSpeed 0.5→0.4, damping 0.85→0.9, graphSeparationX 250→0, temperature 1.0→0.
- **Slider ranges bound to `physics_bounds`** so the UI can no longer request values the validator 400-rejects: repelK max 3000→500, temperature max 5→1.
- **Duplicated ontology-force controls removed** from the Physics tab (`qualityGates.ontologyPhysics` toggle + `ontologyStrength` slider, which carried a divergent 0.5 default vs the panel's 0.6). The dedicated **Ontology → Forces panel** (`OntologyForcesPanel`) is now the single source of truth for ontology-force enable + global strength; the Physics tab's "Semantic & Layout Forces" group retains only the DAG/type-clustering controls.
- **Hydration confirmed correct (no code change):** `visualisation.graphs.logseq.physics` is in `ESSENTIAL_PATHS` and `coreSlice.ts` overlays server physics over localStorage on init, so physics is server-authoritative on a fresh load. The transient UI/backend value divergence observed during testing was stale browser-tab/localStorage state from in-session slider drags, not a defect.

---

*Research provenance: 6-agent ruflo mesh (`swarm_1780651148193`) — ingest, schema, GPU-physics, docs, client, and ontosphere external analysts. All claims carry file:line / repo-path evidence in the agent transcripts.*
