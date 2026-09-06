---
id: VC-20
title: Ontology pipeline - OWL extraction, Oxigraph, Whelk reasoning, governed mutation
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2004, ADR-2071, ADR-2064, ADR-2066, ADR-2068]
sources:
  - src/bin/load_ontology.rs
  - src/services/owl_extractor_service.rs
  - src/services/ontology_pipeline_service.rs
  - src/adapters/oxigraph_graph_repository.rs
  - crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs
  - crates/visionclaw-adapters/src/whelk_inference_engine.rs
  - src/services/ontology_reasoner.rs
  - src/services/ontology_reasoning_service.rs
  - src/services/inferred_edge_materialiser.rs
  - src/services/ontology_class_index.rs
  - src/services/ontology_conflict_gate.rs
  - src/services/ontology_mutation_service.rs
  - src/services/ontology_query_service.rs
  - src/handlers/ontology_handler.rs
  - src/handlers/ontology_derived_handler.rs
  - src/handlers/ontology_class_count_handler.rs
  - src/handlers/ontology_agent_handler.rs
  - src/handlers/api_handler/ontology/mod.rs
  - src/actors/ontology_actor.rs
  - src/actors/messages/ontology_messages.rs
  - src/services/ontology_file_cache.rs
  - src/services/owl_validator.rs
  - src/inference/mod.rs
  - src/reasoning/mod.rs
  - src/services/github_sync_service.rs
  - src/main.rs
  - crates/visionclaw-adapters/src/sparql_migrations.rs
  - src/actors/gpu/gpu_manager_actor.rs
  - src/actors/gpu/ontology_constraint_actor.rs
  - src/actors/gpu/physics_supervisor.rs
  - src/settings/models.rs
verified_commit: 7a20db228
---

## VC-20.1 load_ontology bin - actual sample-data loader
```mermaid
sequenceDiagram
    autonumber
    participant Bin as main<br/>src/bin/load_ontology.rs:18
    participant Repo as OxigraphOntologyRepository<br/>crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs:591
    participant Store as Oxigraph Store

    Bin->>Repo: open(DATA_DIR/oxigraph) (line 31, defn line 589)
    Repo-->>Bin: Ok(Self)
    Note over Bin: RESOLVED ADR-2064 - the bin now walks the real authored corpus. Root resolves CLI arg then<br/>VAULT_ROOT/pages then /app/data/pages (load_ontology.rs:45-55,102), files are parsed with OntologyParser and<br/>persisted via save_ontology, then OwlExtractorService runs over the persisted classes. It exits non-zero when the<br/>corpus is absent or nothing was extracted. The five hardcoded OwlClass literals are gone.
    loop 5 sample_classes (lines 42-63)
        Bin->>Repo: add_owl_class(class) (line 76, defn line 1676)
        Repo->>Store: ASK FROM GRAPH_ONTOLOGY duplicate check (line 1679-1684)
        alt IRI already exists
            Store-->>Repo: ask true
            Repo-->>Bin: Err InvalidData IRI already exists (line 1686)
        else fresh IRI
            Store-->>Repo: ask false
            Repo->>Store: INSERT DATA GRAPH GRAPH_ONTOLOGY (line 1690-1693)
            Repo-->>Bin: Ok(iri)
        end
    end
    Bin->>Repo: add_owl_property(mv:worksAt) (line 93, defn line 2135)
    Bin->>Repo: add_axiom(SubClassOf mv:Company mv:Concept) (line 104, defn line 2415)
    Bin->>Repo: get_classes() (line 107, defn line 2321)
    Repo-->>Bin: Vec of OwlClass
    Note over Bin,Repo: no cache-hit path exists in this bin - OntologyParser::new() at line 36 is constructed but unused (_parser)
```

## VC-20.2 Oxigraph named-graph layout — authored and reasoner graphs
```mermaid
erDiagram
    GRAPH_ONTOLOGY_ASSERT {
        string iri "urn:ngm:graph:ontology:assert (oxigraph_ontology_repository.rs:48)"
        string contents "OwlClass/OwlProperty/OwlAxiom triples, vc: ns"
    }
    GRAPH_ONTOLOGY_INFERRED {
        string iri "urn:ngm:graph:ontology:inferred (line 47)"
        string contents "whelk-derived SubClassOf/EquivalentClass closure"
    }
    GRAPH_KNOWLEDGE {
        string iri "urn:ngm:graph:knowledge (line 48)"
        string contents "KGNode + KGEdge triples"
    }
    GRAPH_AGENT {
        string iri "urn:ngm:graph:agent (line 49)"
        string contents "agent-flagged nodes"
    }
    GRAPH_SHAPES {
        string iri "urn:ngm:graph:shapes (line 54)"
        string contents "SHACL shape triples loaded from ttl at startup"
    }
    GRAPH_PROVENANCE {
        string iri "urn:ngm:graph:provenance (line 55)"
        string contents "append-only PROV-O activity triples, see VC-22"
    }
    OxigraphOntologyRepository ||--o{ GRAPH_ONTOLOGY_ASSERT : "add_owl_class add_axiom lines 1676 2415"
    OxigraphOntologyRepository ||--o{ GRAPH_ONTOLOGY_INFERRED : "store_inference_results CLEAR+INSERT line 2466 ADR-11 D9"
    OxigraphGraphRepository ||--o{ GRAPH_KNOWLEDGE : "src/adapters/oxigraph_graph_repository.rs:6"
    OxigraphGraphRepository ||--o{ GRAPH_AGENT : "src/adapters/oxigraph_graph_repository.rs:16"
    OxigraphOntologyRepository ||--o{ GRAPH_SHAPES : "load_shacl_shapes line 408"
    OxigraphOntologyRepository ||--o{ GRAPH_PROVENANCE : "emit_provenance line 646 see VC-22"
```

## VC-20.3 Whelk EL++ reasoning cycle (github_sync post-sync)
```mermaid
sequenceDiagram
    autonumber
    participant Sync as GitHubSyncService::run_post_sync_reasoning<br/>src/services/github_sync_service.rs:1220
    participant Repo as OxigraphOntologyRepository<br/>crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs:2323
    participant Engine as WhelkInferenceEngine<br/>crates/visionclaw-adapters/src/whelk_inference_engine.rs:346
    participant Whelk as whelk-rs reasoner::assert<br/>crates/visionclaw-adapters/src/whelk_inference_engine.rs:425

    Sync->>Repo: get_classes() (line 1225, defn 2321)
    Sync->>Repo: get_axioms() (line 1230)
    Sync->>Sync: axioms.extend(ngm_property_hierarchy_axioms()) (line 1239)
    Sync->>Engine: load_ontology(classes, axioms) (line 1255, defn line 348)
    Engine->>Engine: compute_ontology_checksum(ontology) (line 368)
    alt checksum unchanged since last load
        Engine-->>Sync: reuse cached_subsumptions (line 379-381)
    else checksum changed
        Engine-->>Sync: cached_subsumptions cleared, fresh reasoning required (line 375-378)
    end
    Sync->>Engine: infer() (line 1260, defn line 397)
    alt cached_subsumptions present (line 406)
        Engine-->>Sync: cached InferenceResults, whelk-rs not invoked (line 407-418)
    else no cache
        Engine->>Whelk: translate_ontology(ontology) (line 422)
        Engine->>Whelk: reasoner::assert(whelk_axioms) (line 425)
        Whelk-->>Engine: named_subsumptions() (line 427)
        Engine-->>Sync: InferenceResults inferred_axioms inference_time_ms (line 445-450)
    end
    Note over Engine: DOC-DRIFT - module doc calls this bounded EL reasoning but infer() lines 397-452 has no tokio timeout or axiom-count cap
    Sync->>Repo: store_inference_results(results) (line 1270, defn line 2466)
    Repo->>Repo: CLEAR GRAPH GRAPH_ONTOLOGY_INFERRED then atomic INSERT (ADR-11 D9, ADR-099 D3, lines 2466-2560)
    Note over Sync: DIVERGENCE - OntologyReasoningService::infer_axioms (ontology_reasoning_service.rs:107) runs CustomReasoner<br/>not this Whelk engine, its WhelkInferenceEngine field is legacy (line 77) and OntologyReasoningService::new is<br/>never called outside tests
```

## VC-20.4 inferred-edge materialisation and class-summary index
```mermaid
sequenceDiagram
    autonumber
    participant Sync as GitHubSyncService::run_post_sync_reasoning<br/>src/services/github_sync_service.rs:1220
    participant Resolver as IriNodeResolver<br/>src/services/github_sync_service.rs:1285
    participant KG as KnowledgeGraphRepository::batch_add_edges<br/>src/services/github_sync_service.rs:1324
    participant Pipeline as OntologyPipelineService::materialise_inferred_edges_from_axioms<br/>src/services/ontology_pipeline_service.rs:473
    participant Mat as inferred_edge_materialiser<br/>src/services/inferred_edge_materialiser.rs:126
    participant Idx as ontology_class_index::maybe_refresh_after_sync<br/>src/services/ontology_class_index.rs:468

    rect rgb(235,235,255)
    Note over Sync,KG: PATH A - inline materialisation, runs unconditionally after Whelk infer
    Sync->>Sync: select_inferred_edges_for_sync(axioms, resolver, asserted) (line 1309, defn line 1141)
    Sync->>Mat: is_materialisable_subclass_pair drops self, owl#Nothing child, owl#Thing parent (line 1151, defn line 168)
    Sync->>Mat: immediate_parents_from_subclass_pairs reduces transitive ancestors to immediate parents (line 1160, defn line 180)
    loop each immediate (child_iri, parent_iri) pair
        Sync->>Resolver: resolve(child_iri) resolve(parent_iri) (lines 1170-1171)
        alt endpoint unresolved
            Sync->>Sync: unresolved_endpoints += 1 (lines 1173 1176)
        else both resolved
            Sync->>Sync: push (child_id, parent_id) candidate (line 1180)
        end
    end
    Sync->>Mat: asserted_pairs(graph.edges) both directions (line 1302, defn line 78)
    Sync->>Mat: select_inferred_edges drops self-loops, asserted pairs, caps per child at 8 (line 1189, defn line 126)
    Sync->>Mat: build_inferred_edge tags edge_type hierarchical and metadata inferred=true (line 1197, defn line 68)
    Sync->>KG: batch_add_edges(inferred_edges) (line 1324)
    Note over Sync: RESOLVED ADR-2071 (2026-09-05) - this live path now calls inferred_edge_materialiser for every selection rule,<br/>so the per-child cap of 8, asserted-pair suppression and the transitive-to-immediate reduction apply here<br/>exactly as on PATH B. Long-range grandparent edges are no longer emitted, and every edge carries<br/>metadata inferred=true, so edge_is_inferred classifies sync-produced edges onto the inferred channel.
    end
    rect rgb(255,245,225)
    Note over Pipeline,Mat: PATH B - OntologyPipelineService, gated OFF by default
    opt config.materialise_inferred_edges is true (default false, ontology_pipeline_service.rs:63)
        Pipeline->>Mat: immediate_inferred_parents(child_to_ancestors) (line 503, defn line 97)
        Pipeline->>Pipeline: resolve_nodes via get_nodes_by_owl_class_iri (lines 511-521)
        Pipeline->>Pipeline: graph_repo.load_graph() current asserted edges (line 528)
        Pipeline->>Mat: materialise(candidates, current.edges, cfg) (line 535, defn line 158)
        Mat->>Mat: select_inferred_edges drops self-loops, asserted pairs, caps per child at max_inferred_parents_per_child=8 (lines 126-152, DEFAULT_MAX_INFERRED_PARENTS_PER_CHILD line 37)
        Pipeline->>Pipeline: graph_repo.batch_add_edges(edges) (line 541)
    end
    end
    Sync->>Idx: maybe_refresh_after_sync(classes) (github_sync_service.rs:635, defn line 468)
    alt ONTOLOGY_CLASS_INDEX_ENABLED unset (default, ontology_class_index.rs:75)
        Idx-->>Sync: skip refresh (line 474)
    else enabled
        Idx->>Idx: refresh_class_index condense_class RuVector memory_store namespace ontology-classes (line 495, defn 378, DEFAULT_NAMESPACE line 33)
    end
```

## VC-20.5 governed mutation - propose_create
```mermaid
sequenceDiagram
    autonumber
    participant Handler as ontology_agent_handler::propose<br/>src/handlers/ontology_agent_handler.rs:217
    participant Mut as OntologyMutationService::propose_create<br/>src/services/ontology_mutation_service.rs:227
    participant Idem as idempotency.reserve<br/>src/services/ontology_mutation_service.rs:253
    participant Repo as OntologyRepository::list_owl_classes
    participant Gate as ontology_conflict_gate::evaluate<br/>src/services/ontology_conflict_gate.rs:454
    participant Whelk as WhelkInferenceEngine::check_axiom_set<br/>crates/visionclaw-adapters/src/whelk_inference_engine.rs:305
    participant PR as github_pr.create_ontology_pr<br/>src/services/ontology_mutation_service.rs:359

    rect rgb(255,230,230)
    Note over Handler,PR: trust boundary - RateLimit::per_minute(20) + RequireAuth::authenticated() on /ontology-agent/propose (ontology_agent_handler.rs:445-446)
    Handler->>Mut: propose_create(proposal, agent_ctx, idempotency_key, signature)
    Mut->>Mut: verify_envelope_precondition (line 254)
    Mut->>Idem: reserve(idem_key, phash) (line 256)
    alt Replay
        Idem-->>Mut: IdempotencyDecision::Replay(receipt)
        Mut-->>Handler: replay_result (line 259)
    else Conflict
        Idem-->>Mut: IdempotencyDecision::Conflict
        Mut-->>Handler: Err IDEMPOTENCY_CONFLICT (lines 262-265)
    else Fresh
        Idem-->>Mut: Fresh (line 267)
        Mut->>Repo: list_owl_classes() (line 290)
        Mut->>Gate: evaluate(corpus, candidate) (line 302, defn 454)
        alt blocking conflict - DuplicateConcept with 2+ corpus members, Cycle, TypeConflict, RelationContradiction
            Gate-->>Mut: ConflictReport blocking non-empty
            Mut-->>Handler: Err CONFLICT_BLOCKED plus report (lines 309-314)
        else no blocking conflicts
            Gate-->>Mut: ConflictReport blocking empty
            Mut->>Whelk: check_axiom_set(corpus, proposed_axioms) (line 318, defn 305)
            alt whelk subsumes a class under owl:Nothing
                Whelk-->>Mut: ConsistencyOutcome consistent false unsatisfiable_classes
                Mut-->>Handler: ProposalStatus::Rejected (line 337)
            else consistent
                Whelk-->>Mut: ConsistencyOutcome consistent true
                Mut->>Mut: generate_term_id generate_vault_markdown compute_quality_score (lines 344-352)
                Mut->>PR: create_ontology_pr(file_path, markdown, title, body, agent_ctx) (line 362)
                PR-->>Mut: pr_url
                Mut-->>Handler: ProposalResult status pr_url gates
            end
        end
    end
    end
```

## VC-20.6 SPARQL read path - /ontology/query
```mermaid
sequenceDiagram
    autonumber
    participant Client
    participant Route as POST /api/ontology/query<br/>src/handlers/api_handler/ontology/mod.rs:1668
    participant Handler as ontology_handler::query_ontology<br/>src/handlers/ontology_handler.rs:886
    participant Validator as validate_read_only_sparql<br/>src/handlers/ontology_handler.rs:752
    participant Clamp as clamp_sparql_limit<br/>src/handlers/ontology_handler.rs:833
    participant CQRS as QueryOntologyHandler
    participant Cap as cap_result_rows<br/>src/handlers/ontology_handler.rs:863

    Client->>Route: POST /ontology/query query
    Note over Route: gated RequireAuth::power_user().mutations_only() (api_handler/ontology/mod.rs:1657)
    Route->>Handler: query_ontology(auth, state, request) (line 886)
    Handler->>Validator: validate_read_only_sparql(query) (line 894, defn 752)
    alt forbidden keyword INSERT DELETE DROP CLEAR LOAD CREATE ADD MOVE COPY WITH SERVICE
        Validator-->>Handler: Err operation not permitted (lines 778-788)
        Handler-->>Client: 400 bad_request
    else no read form present
        Validator-->>Handler: Err only SELECT ASK CONSTRUCT DESCRIBE permitted (lines 797-802)
        Handler-->>Client: 400 bad_request
    else valid read query
        Validator-->>Handler: Ok
        Handler->>Clamp: clamp_sparql_limit(query) (line 905, defn 833)
        Clamp-->>Handler: LIMIT injected or clamped to MAX_SPARQL_ROWS 10000 (line 821-822)
        Handler->>CQRS: QueryOntologyHandler::handle(QueryOntology query) (line 911)
        CQRS-->>Handler: Vec of HashMap string string
        Handler->>Cap: cap_result_rows(results) (line 917, defn 863)
        Cap-->>Handler: rows capped at 10000, byte fence MAX_SPARQL_RESULT_BYTES 8388608, truncated flag (lines 866-882)
        Handler-->>Client: 200 results rowCount truncated
    end
    Note over Handler: ADR-2004 - handler-level fence mirrors adapter fence sparql_select_json (crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs:882)
```

## VC-20.7 ontology HTTP route families
```mermaid
sequenceDiagram
    autonumber
    participant Client
    participant Rbac as RbacGate on /api scope<br/>src/main.rs:1056
    participant OntScope as /api/ontology scope<br/>src/handlers/api_handler/ontology/mod.rs:1654
    participant DerivedScope as /api/ontology/derived scope<br/>src/handlers/ontology_derived_handler.rs:185
    participant CountScope as /api/ontology/class-count scope<br/>src/handlers/ontology_class_count_handler.rs:76
    participant AgentScope as /api/ontology-agent scope<br/>src/handlers/ontology_agent_handler.rs:432

    Client->>Rbac: any /api/* request
    rect rgb(230,255,230)
    Note over OntScope: registration order matters (mod.rs:1631-1642) - derived and class-count scopes register before this broader /ontology scope
    OntScope->>OntScope: POST graph classes properties axioms inference query sparql load load-axioms validate mapping apply, DELETE cache axioms iri classes iri (lines 1659-1679)
    OntScope->>OntScope: GET graph inferred classes properties axioms inference validate metrics inferences hierarchy state-at provenance reports id report health ws (lines 1681-1706)
    Note over OntScope: power_user().mutations_only() gate line 1657 - POST PUT DELETE gated, all GET public
    end
    rect rgb(255,230,230)
    DerivedScope->>DerivedScope: POST derived, POST derived/regenerate, GET derived which (line 189-191)
    Note over DerivedScope: RequireAuth::power_user() whole scope (line 187), fence detail see VC-22
    end
    rect rgb(230,230,255)
    CountScope->>CountScope: GET class-count (line 77)
    Note over CountScope: unauthenticated by design (class_count_handler.rs:24)
    end
    rect rgb(255,245,230)
    AgentScope->>AgentScope: POST discover read query traverse validate, GET status (lines 435-440)
    AgentScope->>AgentScope: POST ontology-agent/propose (line 447)
    Note over AgentScope: only propose sub-scope wrapped RateLimit::per_minute(20) plus RequireAuth::authenticated() lines 445-446, others unauthenticated
    end
    rect rgb(245,245,245)
    InfScope->>InfScope: POST run batch validate, GET results ontology_id classify ontology_id, DELETE cache ontology_id (lines 273-285)
    Note over OntScope: RESOLVED ADR-2066 - the /api/inference scope is gone. Its seven handlers extracted a web::Data InferenceService<br/>that was never constructed or registered, so every route 500'd at the extractor. inference_handler.rs,<br/>application/inference_service.rs, events/inference_triggers.rs and the main.rs registration were all removed.<br/>The live reasoning path is GitHubSyncService::run_post_sync_reasoning - see VC-20.3
    end
```

## VC-20.8 OntologyActor message set
```mermaid
classDiagram
    class OntologyActor
    class InitializeActor
    class JobCompleted
    class LoadOntologyAxioms
    class UpdateOntologyMapping
    class ValidateOntology
    class ApplyInferences
    class GetOntologyReport
    class GetOntologyHealth
    class ClearOntologyCaches
    class TriggerReasoning
    class GetCachedOntologies
    class ProcessOntologyData
    class ApplyMaterializedAxioms

    OntologyActor ..> InitializeActor : Handler ontology_actor.rs 707
    OntologyActor ..> JobCompleted : Handler line 739
    OntologyActor ..> LoadOntologyAxioms : Handler line 749
    OntologyActor ..> UpdateOntologyMapping : Handler line 781
    OntologyActor ..> ValidateOntology : Handler line 791
    OntologyActor ..> ApplyInferences : Handler line 849
    OntologyActor ..> GetOntologyReport : Handler line 871
    OntologyActor ..> GetOntologyHealth : Handler line 906
    OntologyActor ..> ClearOntologyCaches : Handler line 938
    OntologyActor ..> TriggerReasoning : Handler line 962
    OntologyActor ..> GetCachedOntologies : Handler line 985

    note for ValidateOntology "fields ontology_id, graph_data, mode, job_id (ontology_messages.rs:36-46), rtype<br/>Result of ValidationReport or String"
    note for ApplyInferences "fields rdf_triples, max_depth (ontology_messages.rs:50-55)"
    note for TriggerReasoning "fields ontology_id i64, source String (ontology_actor.rs:957-960), handler forwards<br/>to ReasoningActor per comment lines 971-973 which no longer exists"
    note for ApplyMaterializedAxioms "fields axioms, graph_data (ontology_messages.rs:85-88); NOT handled by OntologyActor<br/>- routed GPUManagerActor gpu_manager_actor.rs 645 to PhysicsSupervisor<br/>physics_supervisor.rs 732 to OntologyConstraintActor ontology_constraint_actor.rs 522"
    note for ProcessOntologyData "fields pages Vec of LogseqPage (ontology_messages.rs:69-72)"
```

## VC-20.9 OntologyFileCache read and invalidate
```mermaid
sequenceDiagram
    autonumber
    participant Caller
    participant Cache as OntologyFileCache<br/>src/services/ontology_file_cache.rs:109
    participant LRU as LruCache string CachedOntologyFile<br/>src/services/ontology_file_cache.rs:111
    participant Stats as OntologyCacheStats<br/>src/services/ontology_file_cache.rs:88

    Caller->>Cache: get(file_path, current_sha) (line 139)
    Cache->>LRU: get_mut(file_path) (line 143)
    alt entry present and is_valid_for(current_sha) true (line 75)
        LRU-->>Cache: Some entry
        Cache->>Cache: entry.touch() last_accessed access_count += 1 (lines 80-83)
        Cache->>Stats: hits += 1 (line 146)
        Cache-->>Caller: Some(CachedOntologyFile)
    else entry present but content_sha mismatch
        Cache->>LRU: pop(file_path) (line 150)
        Cache->>Stats: invalidations += 1 (line 151)
        Cache->>Stats: misses += 1 (line 155)
        Cache-->>Caller: None
    else no entry
        Cache->>Stats: misses += 1 (line 155)
        Cache-->>Caller: None
    end
    Caller->>Cache: put(file_path, entry) (line 160)
    alt cache.len() >= max_entries and not already cached
        Cache->>Stats: evictions += 1 (line 166, LRU capacity 500, config line 29)
    end
    Cache->>LRU: put(file_path, entry) (line 169)
    Caller->>Cache: invalidate(file_path) (line 174)
    Cache->>LRU: pop(file_path) (line 178)
    opt entry was present
        Cache->>Stats: invalidations += 1, current_size updated (lines 179-180)
    end
```

## VC-20.10 owl_validator shim and orphaned stubs
```mermaid
sequenceDiagram
    autonumber
    participant OntAct as OntologyActor<br/>src/actors/ontology_actor.rs:117
    participant Shim as owl_validator shim<br/>src/services/owl_validator.rs:2
    participant Real as visionclaw_ontology::services::owl_validator

    OntAct->>Shim: use crate::services::owl_validator::ValidatorTypes (ontology_actor.rs:25)
    Shim->>Real: pub use visionclaw_ontology::services::owl_validator::* (owl_validator.rs:2, ADR-090 Phase A4)
    Real-->>OntAct: OwlValidatorService ValidationReport ValidationConfig PropertyGraph RdfTriple ConstraintSummary Severity
    Note over Shim: shim comment cites ADR-090 Phase A4 crate-split move, mirrored by src/inference/mod.rs:2 and src/reasoning/mod.rs:2 shims into the same crate
    Note over OntAct: RESOLVED ADR-2065 - src/services/owl_validator_stubs.rs was orphaned dead code (a second CPU-only<br/>ValidationConfig PropertyGraph RdfTriple ConstraintSummary ValidationReport set with no mod declaration<br/>or use anywhere, never compiled into any build) and has been deleted. owl_validator is now the single<br/>validation definition.
```

## VC-20.11 ontology_physics.toml consumption
```mermaid
flowchart TD
    Toml["RESOLVED ADR-2068: ontology_physics.toml (repo root) deleted - it was read by no Rust code, no path literal for it existed in src/ or crates/. The live toggle is the Settings.ontology_physics bool."]
    Settings["Settings.ontology_physics bool<br/>src/settings/models.rs:63"]
    Enable["POST /api/ontology-physics/enable<br/>src/handlers/api_handler/ontology_physics/mod.rs:107"]
    Disable["POST /api/ontology-physics/disable<br/>src/handlers/api_handler/ontology_physics/mod.rs:387"]
    OntoActor["OntologyActor::GetOntologyReport<br/>src/actors/messages/ontology_messages.rs"]
    ConstraintActor["OntologyConstraintActor<br/>src/actors/gpu/ontology_constraint_actor.rs:522"]
    PipelineCfg["SemanticPhysicsConfig<br/>src/services/ontology_pipeline_service.rs:27"]
    SimParams["GPU SimParams / live-kernel constraint buffer"]

    Settings --> Enable
    Settings --> Disable
    Enable --> OntoActor
    OntoActor -->|"GetOntologyReport"| ConstraintActor
    ConstraintActor --> SimParams
    PipelineCfg -->|"constraint_strength, use_gpu_constraints, materialise_inferred_edges defaults (pipeline_service.rs:59-67)"| ConstraintActor
```

## VC-20.12 Oxigraph named-graph layout — fenced-derived, cache and migration graphs
```mermaid
erDiagram
    GRAPH_ONTOLOGY_SUMMARY {
        string iri "urn:ngm:graph:ontology:summary (oxigraph_ontology_repository.rs:63)"
        string contents "approval-driven summary triples, fenced write, see VC-22"
    }
    GRAPH_ONTOLOGY_OBSERVED {
        string iri "urn:ngm:graph:ontology:observed (line 62)"
        string contents "externally-observed facts, fenced write, see VC-22"
    }
    GRAPH_CACHE_SSSP {
        string iri "urn:ngm:graph:cache:sssp (line 71)"
        string contents "shortest-path cache quads, own sub-domain so CLEAR GRAPH invalidates atomically"
    }
    GRAPH_CACHE_APSP {
        string iri "urn:ngm:graph:cache:apsp (line 72)"
        string contents "all-pairs shortest-path cache quads"
    }
    GRAPH_MIGRATIONS {
        string iri "urn:ngm:graph:migrations (sparql_migrations.rs:44)"
        string contents "applied SPARQL migration ids"
    }
    OxigraphOntologyRepository ||--o{ GRAPH_ONTOLOGY_SUMMARY : "append_derived_quads line 728 fenced see VC-22"
    OxigraphOntologyRepository ||--o{ GRAPH_ONTOLOGY_OBSERVED : "append_derived_quads line 728 fenced see VC-22"
    SparqlMigrations ||--o{ GRAPH_MIGRATIONS : "sparql_migrations.rs:44"
```
