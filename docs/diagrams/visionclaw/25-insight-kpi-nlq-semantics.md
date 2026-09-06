---
id: VC-25
title: Insight loop, KPI, briefing, NLQ and semantic classification
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2014, ADR-2040, ADR-2004, ADR-2063, ADR-2065]
sources:
  - src/services/insight_loop.rs
  - src/handlers/insight_loop_handler.rs
  - src/adapters/sqlite_enrichment_repository.rs
  - src/services/kpi_compute.rs
  - src/adapters/sqlite_kpi_repository.rs
  - src/handlers/kpi_handler.rs
  - src/services/briefing_service.rs
  - src/handlers/briefing_handler.rs
  - src/services/natural_language_query_service.rs
  - src/handlers/natural_language_query_handler.rs
  - src/services/perplexity_service.rs
  - src/services/semantic_analyzer.rs
  - src/services/semantic_type_registry.rs
  - src/services/edge_classifier.rs
  - src/services/ontology_content_analyzer.rs
  - crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs
  - src/actors/semantic_processor_actor.rs
  - src/actors/messages/physics_messages.rs
  - src/actors/messages/analytics_messages.rs
  - crates/visionclaw-actors/src/messages/analytics_messages.rs
  - src/actors/metadata_actor.rs
  - crates/visionclaw-actors/src/messages/graph_messages.rs
  - src/services/file_service.rs
  - src/services/graph_serialization.rs
  - src/app_state.rs
  - data/metadata/metadata.json
  - src/handlers/ontology_handler.rs
  - src/ports/knowledge_graph_repository.rs
  - src/services/github_sync_service.rs
  - src/services/liveness_harness.rs
  - src/services/local_file_sync_service.rs
  - src/services/management_api_client.rs
  - src/services/nostr_bead_publisher.rs
  - src/services/ontology_enrichment_service.rs
  - src/services/schema_service.rs
verified_commit: 7a20db228
---

## VC-25.1 Insight loop trace assembly (REC-10, compute-on-read)

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as insight_loop_handler<br/>src/handlers/insight_loop_handler.rs:53
    participant R as SqliteEnrichmentRepository<br/>src/adapters/sqlite_enrichment_repository.rs:647
    participant L as insight_loop::summarise<br/>src/services/insight_loop.rs:209
    participant B as insight_loop::build_trace<br/>src/services/insight_loop.rs:103
    participant LH as LivenessHarness<br/>src/services/liveness_harness.rs

    Note over H,B: DIVERGENCE: no tokio::time::interval scheduler exists for this loop -<br/>every stage is computed fresh on each GET (compute-on-read), not a periodic job
    C->>H: GET /api/insight-loop/trace?limit=
    H->>R: loop_traces(limit) - SELECT p JOIN d ON MAX(decided_at_ms)<br/>src/adapters/sqlite_enrichment_repository.rs:647-676
    R-->>H: Vec~LoopTraceRow~<br/>src/adapters/sqlite_enrichment_repository.rs:221
    H->>L: summarise(rows)
    loop for each LoopTraceRow in rows
        L->>B: build_trace(row)
        B->>B: stage_instants(row) - propose/queued from body or created_at*1000<br/>src/services/insight_loop.rs:88
        Note over B: INVARIANT: stage order propose(35)->queued(36)->broker_decision(37)->merged_enrichment(38)->amplification(39)
        alt writeback_committed_at_ms Some
            B->>B: merged status = complete, loop_closed = true
        else decided_at_ms Some and writeback_triggered Some(true)
            B->>B: merged status = pending
        else decided_at_ms Some
            B->>B: merged status = not_applicable (rejection/unattributed - never fabricated pending)
        else decided_at_ms None
            B->>B: merged status = pending (still queued)
        end
        B->>B: amplification stage = planned, at_ms=None always<br/>src/services/insight_loop.rs:159-165
        B->>B: monotonic = windows(2).all(w[0]<=w[1])<br/>src/services/insight_loop.rs:177
        B-->>L: InsightLoopTrace
    end
    L->>L: mesh_velocity_mean_ms = mean over closed loops only, None if none closed<br/>src/services/insight_loop.rs:213-217
    L-->>H: InsightLoopSummary
    opt a closed and monotonic trace is present
        H->>LH: observe(CANARY_REC10_LOOP, evidence)<br/>src/handlers/insight_loop_handler.rs:35-50
    end
    H-->>C: 200 InsightLoopSummary
```

## VC-25.2 insight_loop_handler routes

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant Cfg as configure_routes<br/>src/handlers/insight_loop_handler.rs:89
    participant T as traces<br/>src/handlers/insight_loop_handler.rs:53
    participant TC as trace_by_case<br/>src/handlers/insight_loop_handler.rs:68
    participant R as SqliteEnrichmentRepository<br/>src/adapters/sqlite_enrichment_repository.rs:682

    Note over Cfg: scope /insight-loop mounted under /api - src/handlers/insight_loop_handler.rs:89-94
    C->>T: GET /insight-loop/trace?limit=N
    Note right of T: limit.clamp(1, MAX_LIMIT=1000), default DEFAULT_LIMIT=100<br/>src/handlers/insight_loop_handler.rs:25-26,54
    T-->>C: 200 InsightLoopSummary
    C->>TC: GET /insight-loop/trace/{case_id}
    TC->>R: loop_trace_for(case_id)<br/>src/adapters/sqlite_enrichment_repository.rs:682
    alt Ok(Some(row))
        R-->>TC: LoopTraceRow
        TC-->>C: 200 InsightLoopTrace
    else Ok(None)
        TC-->>C: 404 not-found
    else Err(e)
        TC-->>C: 500 loop-trace read failed
    end
```

## VC-25.3 KPI compute and persist (kpi_snapshots / kpi_lineage / kpi_agent_events)

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant K as KpiComputeService::compute_and_persist<br/>src/services/kpi_compute.rs:216
    participant KR as SqliteKpiRepository<br/>src/adapters/sqlite_kpi_repository.rs:259
    participant ER as SqliteEnrichmentRepository<br/>src/adapters/sqlite_enrichment_repository.rs
    participant LH as LivenessHarness<br/>src/services/liveness_harness.rs

    Note over K: KPI_WINDOW_MS = 30 days rolling window (ADR-043 legacy ref, not in docs/adr/)<br/>src/services/kpi_compute.rs:42
    C->>K: compute_and_persist()
    K->>KR: count_agent_events_since(window_start) - kpi_agent_events<br/>src/adapters/sqlite_kpi_repository.rs:387
    KR-->>K: agent_volume
    K->>ER: decisions_since(window_start) - enrichment_decisions
    ER-->>K: decisions (outcome, activity_urn)
    K->>K: augmentation_ratio(agent_volume, escalation_volume)<br/>src/services/kpi_compute.rs:74
    alt escalation_volume == 0
        K->>K: (value=0.0, confidence=0.0) - undefined ratio, never Inf/NaN
    else escalation_volume > 0
        K->>K: value = agent_volume/escalation_volume, confidence = sample_confidence(sum)
    end
    critical persist augmentation_ratio snapshot + lineage in one transaction
        K->>KR: insert_snapshot_with_lineage(ar_snapshot, ar_lineage)<br/>src/adapters/sqlite_kpi_repository.rs:410
        KR->>KR: tx = c.transaction() then INSERT kpi_snapshots then INSERT kpi_lineage per row then tx.commit()<br/>src/adapters/sqlite_kpi_repository.rs:419-457
        KR-->>K: ar_id
    end
    K->>K: trust_variance(outcomes) - Gini-Simpson 1-sum(p^2) normalised<br/>src/services/kpi_compute.rs:90
    critical persist trust_variance snapshot + lineage in one transaction
        K->>KR: insert_snapshot_with_lineage(tv_snapshot, tv_lineage)
        KR-->>K: tv_id
    end
    K->>LH: observe(CANARY_REC4_KPI, evidence)<br/>src/services/kpi_compute.rs:317
    K-->>C: KpiSummary{ar_tile, tv_tile, mesh_tile(awaiting), hitl_tile(awaiting)}
    Note over K: mesh_tile and hitl_tile always status=awaiting_data_source, value=None - never fabricated<br/>src/services/kpi_compute.rs:352-361

    Note over KR: separate passive tap task (not this request path):
    loop rx.recv().await on /wss/agent-events hub - src/services/kpi_compute.rs:391
        KR->>KR: record_agent_trajectory - INSERT kpi_agent_events(event_id,source_agent_id,action_type,...,agent_did,handoff_id)<br/>src/adapters/sqlite_kpi_repository.rs:319-347
    end
```

## VC-25.4 kpi_handler read routes

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant Cfg as configure_routes<br/>src/handlers/kpi_handler.rs:47
    participant S as summary handler<br/>src/handlers/kpi_handler.rs:25
    participant Lin as lineage handler<br/>src/handlers/kpi_handler.rs:33
    participant K as KpiComputeService<br/>src/services/kpi_compute.rs:188

    Note over Cfg: scope /kpi mounted under /api - src/handlers/kpi_handler.rs:47-52
    C->>S: GET /kpi/summary
    S->>K: compute_and_persist() (see VC-25.3)
    alt Ok(summary)
        S-->>C: 200 KpiSummary
    else Err(e)
        S-->>C: 500 {error: e}
    end
    C->>Lin: GET /kpi/lineage/{snapshot_id}
    Lin->>K: lineage_for(snapshot_id)<br/>src/services/kpi_compute.rs:372
    K->>K: kpi_repo.lineage_for(snapshot_id) - SELECT kpi_lineage WHERE snapshot_id<br/>src/adapters/sqlite_kpi_repository.rs:488
    alt Ok(rows)
        Lin-->>C: 200 {snapshot_id, lineage: Vec~KpiLineageRow~}
    else Err(e)
        Lin-->>C: 500 {error: e}
    end
```

## VC-25.5 KPI SQLite schema (kpi.sqlite3)

```mermaid
erDiagram
    kpi_agent_events {
        INTEGER id PK
        INTEGER event_id
        INTEGER source_agent_id
        INTEGER action_type
        INTEGER observed_at_ms
        TEXT agent_did
        TEXT action_type_name
        TEXT source_urn
        TEXT target_urn
        TEXT handoff_id
        INTEGER token_count
        TEXT verification
    }
    kpi_snapshots {
        INTEGER id PK
        TEXT kpi
        REAL value
        REAL confidence
        REAL numerator
        REAL denominator
        INTEGER sample_count
        INTEGER window_start_ms
        INTEGER window_end_ms
        INTEGER computed_at_ms
        TEXT sha
    }
    kpi_lineage {
        INTEGER id PK
        INTEGER snapshot_id FK
        TEXT source_kind
        TEXT source_ref
        REAL contribution
    }
    schema_migrations {
        TEXT id PK
        INTEGER applied_at
    }
    kpi_snapshots ||--o{ kpi_lineage : "DERIVED_FROM (snapshot_id)"
```

## VC-25.6 Briefing service and handler (submit + debrief)

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as briefing_handler<br/>src/handlers/briefing_handler.rs:22
    participant BS as BriefingService<br/>src/services/briefing_service.rs:11
    participant MA as ManagementApiClient<br/>src/services/management_api_client.rs
    participant NP as NostrBeadPublisher<br/>src/services/nostr_bead_publisher.rs

    Note over H: routes: POST /briefs (submit_brief), POST /briefs/{brief_id}/debrief (request_debrief)<br/>src/handlers/briefing_handler.rs:116-122
    C->>H: POST /api/briefs {briefing, user_context}
    H->>BS: submit_brief(request, user_context)<br/>src/services/briefing_service.rs:24
    BS->>MA: create_brief(content, roles, user_context, version, brief_type, slug)
    MA-->>BS: brief_result{brief_id, brief_path, bead_id}
    BS->>MA: execute_brief(brief_id, brief_path, roles, user_context, bead_id)
    MA-->>BS: role_tasks: Vec~RoleTask~
    alt Ok(response)
        BS-->>H: BriefingResponse
        H-->>C: 201 Created {brief_id, brief_path, bead_id, role_tasks}
    else Err(BriefingError::ApiError(msg))
        BS-->>H: BriefingError::ApiError<br/>src/services/briefing_service.rs:104
        H-->>C: 502 BadGateway {error, message}
    end
    C->>H: POST /api/briefs/{brief_id}/debrief {role_tasks, user_context}
    H->>H: bead_id = first role_task.bead_id or brief_id<br/>src/handlers/briefing_handler.rs:69-74
    H->>BS: request_debrief(brief_id, role_tasks, user_context)<br/>src/services/briefing_service.rs:80
    BS->>MA: create_debrief(brief_id, role_tasks, user_context)
    alt Ok(debrief_path)
        opt nostr_publisher configured
            H->>NP: tokio::spawn publish_bead_complete(bead_id, brief_id, user_pubkey, debrief_path)<br/>src/handlers/briefing_handler.rs:88-97
            Note right of NP: fire-and-forget - does not affect the HTTP response
        end
        H-->>C: 201 Created {brief_id, debrief_path}
    else Err(BriefingError::ApiError(msg))
        H-->>C: 502 BadGateway {error, message}
    end
```

## VC-25.7 Natural language query: parse -> plan -> execute

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as natural_language_query_handler<br/>src/handlers/natural_language_query_handler.rs:90
    participant S as NaturalLanguageQueryService<br/>src/services/natural_language_query_service.rs:32
    participant SC as SchemaService<br/>src/services/schema_service.rs
    participant P as PerplexityService::chat_completion<br/>src/services/perplexity_service.rs:77

    Note over S,P: config-driven, not an env flag - api_url/api_key/model read from<br/>AppFullSettings.perplexity at call time (src/services/perplexity_service.rs:103-113)
    C->>H: POST /nl-query/translate {query, suggestAlternatives}
    H->>S: translate_to_sparql(query) or suggest_queries(query)<br/>src/services/natural_language_query_service.rs:54,78
    S->>SC: get_llm_context() - schema for the prompt
    SC-->>S: schema_context
    S->>S: get_system_prompt() then build_translation_prompt(query, schema_context)<br/>src/services/natural_language_query_service.rs:140,202
    Note over S: RESOLVED ADR-2063 - the prompt now describes the real Oxigraph named graphs and vocabulary<br/>and asks for read-only SPARQL. It previously claimed to be a SPARQL generator while instructing and<br/>validating for Cypher GraphNode/EDGE labels, so every generated query was unrunnable against the store.
    S->>P: chat_completion([(system,...), (user, prompt)])
    alt perplexity_config missing (api_url/api_key/model None)
        P-->>S: Err("Perplexity API URL/Key/model not configured")
        S-->>H: Err("LLM service error: ...")
        H-->>C: 200 {error: "Translation failed", message}
    else config present
        P-->>S: response text
        S->>S: extract_sparql_block(response) - find fenced sparql block or fallback fence<br/>src/services/natural_language_query_service.rs:277
        alt no fenced block found
            S-->>H: Err - no SPARQL query found in response
            H-->>C: 200 {error: "Translation failed", message}
        else block found
            S->>S: validate_sparql(sparql) - delegates to the shared read-only validator<br/>src/services/natural_language_query_service.rs:106
            S->>S: validate_read_only_sparql - SELECT/ASK/CONSTRUCT/DESCRIBE only<br/>src/handlers/ontology_handler.rs:752
            alt mutating form (INSERT/DELETE/DROP/CLEAR/LOAD)
                S-->>H: Err - rejected, not a read-only query
                H-->>C: 200 {error: "Translation failed", message}
            else valid read-only SPARQL
                S-->>H: QueryTranslation{sparql_query, explanation, confidence, warnings}
                H-->>C: 200 {translations: [...]}
            end
        end
    end
    Note over S,H: INVARIANT one validator - the NLQ path reuses ontology_handler::validate_read_only_sparql<br/>rather than defining a second read-only policy (ADR-2063)
    C->>H: POST /nl-query/validate {sparql}
    H->>S: validate_sparql(sparql)<br/>src/handlers/natural_language_query_handler.rs:196
    H-->>C: 200 {valid, errors}
    C->>H: POST /nl-query/explain {sparql}
    H->>S: validate_sparql then explain_sparql(sparql)<br/>src/handlers/natural_language_query_handler.rs:170
    alt rejected by the read-only validator
        H-->>C: 200 {error: "Invalid SPARQL", message}
    else valid
        S->>P: chat_completion - explain prompt<br/>src/services/natural_language_query_service.rs:111
        H-->>C: 200 {sparql, explanation}
    end
    C->>H: GET /nl-query/examples
    H-->>C: 200 {examples: QueryPatterns::examples()}
    Note over H: routes registered by configure_nl_query_routes - translate, examples, explain, validate<br/>src/handlers/natural_language_query_handler.rs:236-244
```

## VC-25.8 SemanticProcessorActor message handling

```mermaid
sequenceDiagram
    autonumber
    participant Caller as GraphServiceSupervisor / handler
    participant A as SemanticProcessorActor<br/>src/actors/semantic_processor_actor.rs:173

    Note over A: Actor started/stopped log config.enable_ai_features and final stats<br/>src/actors/semantic_processor_actor.rs:1374-1388

    Caller->>A: UpdateConstraints{constraint_data}<br/>src/actors/messages/physics_messages.rs:298
    A-->>Caller: Result<(),String> via handle_constraint_update<br/>src/actors/semantic_processor_actor.rs:1394-1401

    Caller->>A: GetConstraints<br/>src/actors/messages/physics_messages.rs:282
    A-->>Caller: ConstraintSet clone<br/>src/actors/semantic_processor_actor.rs:1403-1412

    Caller->>A: TriggerStressMajorization<br/>src/actors/messages/physics_messages.rs:327
    A->>A: web::block(stress optimisation) on thread pool<br/>src/actors/semantic_processor_actor.rs:1415
    alt thread pool Ok(Ok(()))
        A-->>Caller: Ok(())
    else Ok(Err(e)) or thread pool error
        A-->>Caller: Err(e) / "Thread pool error"
    end

    Caller->>A: RegenerateSemanticConstraints<br/>src/actors/messages/physics_messages.rs:347
    A->>A: deactivate semantic_similarity/semantic_clustering/importance_based/topic_based groups<br/>src/actors/semantic_processor_actor.rs:1451-1459
    A->>A: web::block(generate_semantic_constraints_blocking)
    A-->>Caller: ResponseFuture<Result<(),String>>

    Caller->>A: UpdateAdvancedParams{params}<br/>src/actors/messages/physics_messages.rs:270
    A->>A: rebuild StressMajorizationSolver::from_advanced_params, relationship_threshold = weight*0.1<br/>src/actors/semantic_processor_actor.rs:1501-1523

    Caller->>A: SetGraphData{graph_data}<br/>src/actors/semantic_processor_actor.rs:1524
    Caller->>A: ProcessMetadata{metadata_id, metadata}<br/>src/actors/semantic_processor_actor.rs:1539
    A->>A: web::block(process_metadata_blocking)<br/>src/actors/semantic_processor_actor.rs:1544-1567

    Caller->>A: GetSemanticStats<br/>src/actors/semantic_processor_actor.rs:1573
    A-->>Caller: SemanticStats clone

    Caller->>A: UpdateSemanticConfig{config}<br/>src/actors/semantic_processor_actor.rs:1585
    A-->>Caller: () via update_config

    Caller->>A: ComputeShortestPaths{source_node_id}<br/>src/actors/messages/analytics_messages.rs:148
    alt gpu_analyzer is None
        A-->>Caller: Err("GPU analyzer not available")<br/>src/actors/semantic_processor_actor.rs:1608-1610
    else gpu_analyzer present
        A->>A: gpu_analyzer.initialize(graph) then compute_shortest_paths(source_node_id)<br/>src/actors/semantic_processor_actor.rs:1617-1624
        A-->>Caller: Ok(PathfindingResult) / Err("Pathfinding failed")
    end

    Caller->>A: ComputeAllPairsShortestPaths<br/>crates/visionclaw-actors/src/messages/analytics_messages.rs:166
    alt gpu_analyzer is None
        A-->>Caller: Err("GPU analyzer not available")<br/>src/actors/semantic_processor_actor.rs:1656-1658
    else present
        A->>A: compute_all_pairs_shortest_paths() - GPU landmark approximation<br/>src/actors/semantic_processor_actor.rs:1665-1672
        A-->>Caller: Ok(HashMap<(u32,u32),Vec~u32~>) / Err("APSP failed")
    end
```

## VC-25.9 semantic_analyzer feature extraction + semantic_type_registry lookup/register

```mermaid
sequenceDiagram
    autonumber
    participant SPA as SemanticProcessorActor::process_metadata_blocking<br/>src/actors/semantic_processor_actor.rs:260
    participant SA as SemanticAnalyzer::analyze_metadata<br/>src/services/semantic_analyzer.rs:261
    participant GS as github_sync_service<br/>src/services/github_sync_service.rs:2017
    participant REG as SemanticTypeRegistry<br/>src/services/semantic_type_registry.rs:90

    SPA->>SA: analyze_metadata(metadata)
    alt config.enable_caching and id cached
        SA-->>SPA: cached SemanticFeatures clone<br/>src/services/semantic_analyzer.rs:264-267
    else not cached
        SA->>SA: extract_topics, classify_domains, extract_temporal/structural/content_features
        SA->>SA: calculate_importance_score(topics, temporal, structural)<br/>src/services/semantic_analyzer.rs:270-286
        SA->>SA: feature_cache.insert(id, features.clone())<br/>src/services/semantic_analyzer.rs:288-290
        SA-->>SPA: SemanticFeatures
    end
    Note over SA: compute_similarity weights: topic 0.4 + domain 0.2 + file_type 0.1 + depth 0.1 + temporal 0.1 + importance 0.1<br/>src/services/semantic_analyzer.rs:545-580

    GS->>REG: get_or_register_id(edge_type)<br/>src/services/semantic_type_registry.rs:625
    REG->>REG: get_id(uri) - read_uri_map lookup<br/>src/services/semantic_type_registry.rs:617-621
    alt uri already registered
        REG-->>GS: existing id
    else uri unknown
        REG->>REG: register(uri, RelationshipForceConfig::default())<br/>src/services/semantic_type_registry.rs:601
        REG->>REG: register_internal(uri, config) - assign next_id, push uri/config<br/>src/services/semantic_type_registry.rs:585
        REG-->>GS: new id
    end
    GS->>REG: get_config(reg_id) - strength*2.0 normalised to spring weight<br/>src/services/semantic_type_registry.rs:635,src/services/github_sync_service.rs:2020-2022
    Note over REG: version() = next_id atomic counter, used for hot-reload detection<br/>src/services/semantic_type_registry.rs:679
```

## VC-25.10 edge_classifier classification rules

```mermaid
flowchart TD
    Start(["classify_edge(source_label, target_label, source_class, target_class, context)<br/>src/services/edge_classifier.rs:189"]) --> Lower["context_lower = context.to_lowercase()"]
    Lower --> Loop["for each Pattern in patterns.values()<br/>src/services/edge_classifier.rs:202"]
    Loop --> Match{"keyword found in context_lower?"}
    Match -->|"yes, matches>0"| Score["avg_score = sum(pattern.confidence)/matches<br/>track best_match by highest avg_score"]
    Match -->|no| Loop
    Score --> Loop
    Loop --> Done{"best_match found after all patterns?"}
    Done -->|yes| ReturnMatch["return Some(property_iri) - e.g. mv:hasCEO(0.95), mv:worksAt(0.9), mv:hasFounder(0.9)<br/>src/services/edge_classifier.rs:38-152"]
    Done -->|no| Fallback{"source_class and target_class both Some?"}
    Fallback -->|yes| ClassPair["classify_by_class_pair(src,tgt)<br/>src/services/edge_classifier.rs:256"]
    ClassPair --> Pairs{"Person->Company / Person->Project /<br/>Company->Project / Project->Technology / Concept->Concept?"}
    Pairs -->|match| ReturnFallback["return Some(mapped property_iri)"]
    Pairs -->|no match| ReturnNone["return None - no classification"]
    Fallback -->|no| ReturnNone
    ReturnMatch --> Caller["ontology_enrichment_service::enrich_edges<br/>src/services/ontology_enrichment_service.rs:127"]
    ReturnFallback --> Caller
    ReturnNone --> Caller
```

## VC-25.11 ontology_content_analyzer content analysis

```mermaid
sequenceDiagram
    autonumber
    participant LFS as LocalFileSyncService::process_file_content<br/>src/services/local_file_sync_service.rs:414
    participant OCA as OntologyContentAnalyzer::analyze_content<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:80
    participant Shim as src/services/ontology_content_analyzer.rs<br/>src/services/ontology_content_analyzer.rs:2

    Note over Shim: shim re-exports visionclaw_ontology::services::ontology_content_analyzer::* (ADR-090 Phase A4, not in docs/adr/)<br/>src/services/ontology_content_analyzer.rs:1-2
    alt ontology_cache hit for (file_name, content_sha)
        LFS->>LFS: use cached analysis + metadata - stats.cache_hits+=1<br/>src/services/local_file_sync_service.rs:423-434
    else cache miss
        LFS->>OCA: analyze_content(content, file_name)
        OCA->>OCA: has_public_flag = first 20 lines match "public:: true"<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:84-87
        OCA->>OCA: has_ontology_block = contains "### OntologyBlock"<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:123-125
        OCA->>OCA: extract_term_ids via TERM_ID_PATTERN regex<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:143-148
        OCA->>OCA: detect_source_domain(term_ids) - majority DOMAIN_PREFIXES match (AI-,BC-,MV-,QC-,...)<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:151
        OCA->>OCA: extract_topics via TOPIC_PATTERN "topic:: [[x]]"<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:177
        alt has_ontology_block
            OCA->>OCA: extract_ontology_section then count_classes/count_properties/count_relationships<br/>crates/visionclaw-ontology/src/services/ontology_content_analyzer.rs:100-104,206-218
        else no ontology block
            OCA->>OCA: class_count/property_count/relationship_count stay 0 (ContentAnalysis::default)
        end
        OCA-->>LFS: ContentAnalysis{has_public_flag, has_ontology_block, source_domain, topics, counts}
        LFS->>LFS: stats.cache_misses+=1, build OntologyFileMetadata<br/>src/services/local_file_sync_service.rs:424,437-444
    end
```

## VC-25.12 MetadataActor message handling

```mermaid
sequenceDiagram
    autonumber
    participant Caller as AppState / handler
    participant MA as MetadataActor<br/>src/actors/metadata_actor.rs:23

    Note over MA: replaces Arc<RwLock<MetadataStore>> - started at src/app_state.rs:807 (BASELINE-architecture.md "Actor system topology")
    Caller->>MA: GetMetadata<br/>crates/visionclaw-actors/src/messages/graph_messages.rs:212
    MA-->>Caller: Ok(MetadataStore clone)<br/>src/actors/metadata_actor.rs:58-64
    Caller->>MA: UpdateMetadata{metadata}<br/>crates/visionclaw-actors/src/messages/graph_messages.rs:216
    MA->>MA: update_metadata(new_metadata) - self.metadata = new_metadata<br/>src/actors/metadata_actor.rs:36-39,66-73
    MA-->>Caller: Ok(())
    Note right of MA: RESOLVED ADR-2097 (2026-09-05): RefreshMetadata and refresh_metadata are deleted - the message had no senders<br/>and the actor holds no source. metadata.json is owned by FileService, which pushes rebuilt stores in via UpdateMetadata
```

## VC-25.13 file_service + graph_serialization + empty-graph guard

```mermaid
sequenceDiagram
    autonumber
    participant Boot as AppState::new (startup)
    participant FS as FileService::load_graph_from_files<br/>src/services/file_service.rs:1170
    participant Repo as KnowledgeGraphRepository (Oxigraph)<br/>src/ports/knowledge_graph_repository.rs
    participant GSS as GraphSerializationService::export_graph<br/>src/services/graph_serialization.rs:29

    Boot->>FS: load_graph_from_files(graph_repo)
    FS->>Repo: load_graph() - idempotency guard (ADR-2004, not in docs/adr/ ledger)<br/>src/services/file_service.rs:1177
    alt existing.nodes not empty
        Repo-->>FS: existing graph populated
        FS-->>Boot: Ok(()) - skip local-file seed, GitHub sync is authoritative<br/>src/services/file_service.rs:1178-1185
    else store empty or query failed
        FS->>FS: load_or_create_metadata()<br/>src/services/file_service.rs:1198
        alt metadata.is_empty()
            FS-->>Boot: Ok(()) - warn "no data to load", nothing seeded<br/>src/services/file_service.rs:1199-1202
        else metadata present
            FS->>FS: Phase 1 - build AppNode per file, classify ontology_node vs page via owl_class_iri<br/>src/services/file_service.rs:1215-1248
            FS->>FS: Phase 2 - wikilink regex extracts AppEdge set, dedup via seen_edges<br/>src/services/file_service.rs:1256-1271
            FS->>Repo: save_graph(&graph_data)<br/>src/services/file_service.rs:1291
            FS-->>Boot: Ok(())
        end
    end

    Note over FS: RESOLVED ADR-2065: src/services/empty_graph_check.rs (check_empty_graph) had zero call sites<br/>and has been deleted - the only live empty-graph guard is the idempotency check at file_service.rs:1177

    Note over GSS: distinct empty-graph handling path - export_graph has no explicit empty check,<br/>writes whatever GraphData it is given (src/services/graph_serialization.rs:29-79)
    Boot->>GSS: export_graph(graph, request)
    GSS->>GSS: serialize_to_json/gexf/graphml/csv/dot per ExportFormat<br/>src/services/graph_serialization.rs:46-51
    opt request.compress
        GSS->>GSS: compress_data via GzEncoder (compression_level=6)<br/>src/services/graph_serialization.rs:56-58
    end
    alt file_size > max_file_size (100 MiB)
        GSS-->>Boot: Err("Export file size exceeds limit")<br/>src/services/graph_serialization.rs:64-68
    else within limit
        GSS->>GSS: fs::write(file_path, final_data)
        GSS-->>Boot: ExportResponse{export_id, download_url, expires_at: now+24h}<br/>src/services/graph_serialization.rs:73-80
    end
```
