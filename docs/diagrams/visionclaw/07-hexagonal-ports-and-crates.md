---
id: VC-07
title: Hexagonal ports, adapters, the CQRS application layer and the crate split
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2004, ADR-2005, ADR-2016]
sources:
  - src/lib.rs
  - src/ports/mod.rs
  - src/ports/settings_repository.rs
  - src/ports/knowledge_graph_repository.rs
  - src/ports/graph_repository.rs
  - src/ports/physics_simulator.rs
  - src/adapters/mod.rs
  - src/adapters/actor_graph_repository.rs
  - src/adapters/oxigraph_graph_repository.rs
  - src/adapters/sqlite_settings_repository.rs
  - src/adapters/sqlite_enrichment_repository.rs
  - src/adapters/sqlite_canary_repository.rs
  - src/adapters/sqlite_kpi_repository.rs
  - src/adapters/actix_physics_adapter.rs
  - src/adapters/actix_semantic_adapter.rs
  - src/adapters/gpu_semantic_analyzer.rs
  - src/adapters/physics_orchestrator_adapter.rs
  - src/application/mod.rs
  - src/application/physics_service.rs
  - src/application/semantic_service.rs
  - src/application/events.rs
  - src/domain/mod.rs
  - src/domain/broker/mod.rs
  - src/domain/broker/broker_case.rs
  - src/domain/broker/broker_decision.rs
  - src/domain/broker/precedent_registry.rs
  - src/repositories/mod.rs
  - src/errors/mod.rs
  - src/validation/mod.rs
  - src/config/mod.rs
  - Cargo.toml
verified_commit: b00c28a0d
---

## VC-07.1 The hexagon — ports, adapters and where each canonical type lives
```mermaid
flowchart TB
    subgraph PORTS["src/ports — trait side"]
        P1["graph_repository::GraphRepository<br/>src/ports/graph_repository.rs — 88 lines, LEGACY"]
        P2["physics_simulator::PhysicsSimulator<br/>src/ports/physics_simulator.rs — 63 lines, LEGACY"]
        P3["REMOVED — semantic_analyzer::SemanticAnalyzer<br/>was src/ports/semantic_analyzer.rs, deleted by vc-knowledge<br/>the live seam is visionclaw_domain gpu_semantic_analyzer"]
        P4["knowledge_graph_repository::KnowledgeGraphRepository<br/>src/ports/knowledge_graph_repository.rs — 115 lines"]
        P5["settings_repository::SettingsRepository<br/>src/ports/settings_repository.rs — 14 lines, SHIM ONLY"]
    end
    subgraph SHIM["re-exported from visionclaw-domain by src/ports/mod.rs:25-45"]
        S1["ports::inference_engine::InferenceEngine"]
        S2["ports::ontology_repository::OntologyRepository"]
        S3["ports::gpu_physics_adapter — GpuPhysicsAdapter, PhysicsParameters,<br/>PhysicsStatistics, PhysicsStepResult, NodeForce, GpuDeviceInfo"]
        S4["ports::gpu_semantic_analyzer — GpuSemanticAnalyzer, ClusteringAlgorithm,<br/>CommunityDetectionResult, ImportanceAlgorithm, PathfindingResult,<br/>OptimizationResult, SemanticConstraintConfig, SemanticStatistics"]
    end
    subgraph ADAPT["src/adapters — implementation side"]
        A1["ActorGraphRepository<br/>src/adapters/actor_graph_repository.rs"]
        A2["OxigraphGraphRepository (ADR-11 canonical)<br/>src/adapters/oxigraph_graph_repository.rs"]
        A3["SqliteSettingsRepository<br/>src/adapters/sqlite_settings_repository.rs"]
        A4["SqliteEnrichmentRepository — data/enrichment.sqlite3 (WS-9)"]
        A5["SqliteCanaryRepository — data/liveness.sqlite3 (RES-a)"]
        A6["SqliteKpiRepository — data/kpi.sqlite3 (REC-4 ADR-130 D5)"]
        A7["ActixPhysicsAdapter — cfg(feature gpu)"]
        A8["ActixSemanticAdapter"]
        A9["GpuSemanticAnalyzerAdapter — cfg(feature gpu)"]
        A10["PhysicsOrchestratorAdapter"]
    end
    subgraph XCRATE["shims into visionclaw-adapters — src/adapters/mod.rs:21-35"]
        X1["whelk_inference_engine::WhelkInferenceEngine"]
        X2["messages"]
        X3["oxigraph_ontology_repository::OxigraphOntologyRepository"]
    end
    PORTS --> ADAPT
    SHIM --> ADAPT
    ADAPT --> XCRATE
    N1["INVARIANT (BASELINE) — persistence is Oxigraph (data/oxigraph) plus per-domain SQLite<br/>under DATA_DIR. One Oxigraph store is shared by the ontology and graph repositories.<br/>No networked graph DB. ADR-2004."]
    ADAPT --- N1
    N2["src/repositories/mod.rs is documentation only — all legacy Neo4j and SQL repositories<br/>were removed. Canonical adapters named there are OxigraphOntologyRepository,<br/>OxigraphGraphRepository, SqliteSettingsRepository (ADR-11)."]
    ADAPT --- N2
```

## VC-07.2 ADR-2005 extraction state — what is actually a shim
```mermaid
sequenceDiagram
    autonumber
    participant C as caller in src/
    participant SH as src/ shim module
    participant CR as workspace crate

    Note over C,CR: ADR-2005 hexagonal crate split is PARTIAL. src/ modules that are pure re-export shims —
    C->>SH: use crate::ports::settings_repository::{SettingsRepository, SettingValue, ...}
    SH->>CR: pub use visionclaw_domain::ports::settings_repository::* (src/ports/settings_repository.rs:8)
    Note over SH: 14 lines total, ADR-090 Phase A6 slice 3, also re-exports AppFullSettings (:14)
    C->>SH: use crate::validation::*
    SH->>CR: pub use visionclaw_ontology::validation::* (src/validation/mod.rs:2)
    Note over SH: 2 lines total, ADR-090 Phase A4
    C->>SH: use crate::config::*
    SH->>CR: pub use visionclaw_domain::config::{graph_type, validation, visualisation, system, xr, services}<br/>plus AppFullSettings, DeveloperConfig, FeatureFlags, UserPreferences (src/config/mod.rs:21-56)
    Note over SH: 70 lines, ADR-090 Phase A6 slice 3. Six sibling .rs files are orphaned — see VC-09.2
    C->>SH: use crate::adapters::{messages, oxigraph_ontology_repository, whelk_inference_engine}
    SH->>CR: pub use visionclaw_adapters::* (src/adapters/mod.rs:21-35)
    Note over SH: ADR-090 Phase A1+
    C->>SH: use crate::ports::{inference_engine, ontology_repository, gpu_physics_adapter, gpu_semantic_analyzer}
    SH->>CR: pub use visionclaw_domain::ports::* (src/ports/mod.rs:25-45)
    Note over SH: module-path aliases kept so legacy call sites keep resolving
    Note over C,CR: DIVERGENCE (BASELINE l.222) — actor extraction incomplete, 25 src/actors/*.rs<br/>vs 11 in crates/visionclaw-actors. The live tree runs from src/. See VC-02.
    Note over C,CR: DIVERGENCE (BASELINE 2026-09-04 crate and supervision closeout, l.277) — ADR-2005<br/>remains partial. The workspace adds converter and integration-test members to the census.
```

## VC-07.3 Workspace membership and the excluded contexts
```mermaid
flowchart TB
    W["[workspace] members — Cargo.toml"]
    W --> ROOT["'.' — the visionclaw-server root crate, src/"]
    W --> CORE["crates/visionclaw-domain — 51 src files, models/config/ports<br/>crates/visionclaw-ontology — 51 src files<br/>crates/visionclaw-actors — 11 src files<br/>crates/visionclaw-adapters — 7 src files"]
    W --> EDGE["crates/visionclaw-contracts — 6 src files<br/>crates/visionclaw-protocol — 5 src files<br/>crates/visionclaw-gpu — 5 src files<br/>crates/visionclaw-xr-presence — 9 src files"]
    W --> TAIL["crates/visionclaw-analytics-oracle — 1 src file<br/>crates/vault-migrate — 8 src files<br/>crates/visionclaw-integration-tests — 1 src file"]
    EX["[workspace] exclude"]
    EX --> E1["xr-client/rust — Godot gdext cdylib for the Quest APK<br/>own workspace context and target, PRD-008, see VC-30"]
    EX --> E2["agentbox/crates/headroom-napi — see the agentbox area"]
    ROOT --> EX
    ORPH["DIVERGENCE — crates/graph-cognition-extract is on disk<br/>but EMPTY and NOT a workspace member — orphan directory"]
    TAIL --- ORPH
    DEP["solid-pod-rs is a crates.io VERSION pin, not a git rev<br/>the former rev main resolution error no longer applies"]
    EDGE --- DEP
    IND["visionclaw-contracts is independently buildable<br/>cargo build --manifest-path crates/visionclaw-contracts/Cargo.toml"]
    EDGE --- IND
```

## VC-07.4 Root-crate module surface — src/lib.rs
```mermaid
flowchart TB
    L["src/lib.rs — 64 lines, the public module surface"]
    subgraph HEX["hexagonal layers"]
        H1["ports · adapters · application · domain · repositories"]
    end
    subgraph RUNTIME["runtime"]
        R1["actors · app_state · handlers · middleware · services · settings"]
    end
    subgraph DOMAINLOGIC["domain logic"]
        D1["constraints · layout · physics · gpu · inference · ontology · reasoning<br/>ADR-2066 — application/inference_service.rs and handlers/inference_handler.rs<br/>removed as dead code, the inference PORT and Whelk adapter remain live"]
    end
    subgraph WIRE["wire and identity"]
        W1["protocol · events · agent_events · types · models · uri · openapi · client"]
        W2["web_contract — ADR-124 gitmark/blocktrails substrate<br/>4-layer reducer/state/ledger/trail, validate/anchor/verify ritual<br/>identity-rail-agnostic, carries did:nostr unchanged"]
    end
    subgraph CROSS["cross-cutting"]
        C1["config · errors · validation · telemetry · utils (macro_use) · test_helpers"]
    end
    L --> HEX
    L --> RUNTIME
    L --> DOMAINLOGIC
    L --> WIRE
    L --> CROSS
    RE["re-exports at src/lib.rs:47-64 — ClientCoordinatorActor, MetadataActor,<br/>OptimizedSettingsActor, AppState, UserSettings; plus ADR-090 compatibility aliases<br/>MetadataStore, ProtectedSettings, SimulationParams from visionclaw_domain::models"]
    L --- RE
    U["utils re-exports — from_json, to_json, safe_json_number, time, HandlerResponse"]
    CROSS --- U
```

## VC-07.5 CQRS application layer — settings and knowledge-graph domains
```mermaid
classDiagram
    class SettingsDirectives {
      <<src/application/settings/directives.rs>>
      SaveAllSettings
      UpdateSetting
      UpdateSettingsBatch
      UpdatePhysicsSettings
      DeletePhysicsProfile
      ClearSettingsCache
    }
    class SettingsQueries {
      <<src/application/settings/queries.rs>>
      LoadAllSettings
      GetSetting
      GetSettingsBatch
      GetPhysicsSettings
      ListPhysicsProfiles
    }
    class KnowledgeGraphDirectives {
      <<src/application/knowledge_graph/directives.rs>>
      AddNode
      UpdateNode
      RemoveNode
      AddEdge
      UpdateEdge
      RemoveEdge
      BatchUpdatePositions
      SaveGraph
    }
    class KnowledgeGraphQueries {
      <<src/application/knowledge_graph/queries.rs>>
      LoadGraph
      GetNode
      GetNodeEdges
      GetNodesByMetadataId
      QueryNodes
      GetGraphStatistics
    }
    class QueryResult {
      <<src/application/knowledge_graph>>
    }
    class Handlers {
      <<one Handler per Directive and Query>>
      SaveAllSettingsHandler
      UpdateSettingHandler
      LoadAllSettingsHandler
      AddNodeHandler
      LoadGraphHandler
      QueryNodesHandler
    }
    SettingsDirectives ..> Handlers
    SettingsQueries ..> Handlers
    KnowledgeGraphDirectives ..> Handlers
    KnowledgeGraphQueries ..> Handlers
    KnowledgeGraphQueries ..> QueryResult
```

## VC-07.6 CQRS application layer — ontology, graph and services
```mermaid
classDiagram
    class OntologyDirectives {
      <<src/application/ontology/directives.rs>>
      AddOwlClass
      UpdateOwlClass
      RemoveOwlClass
      AddOwlProperty
      UpdateOwlProperty
      AddAxiom
      RemoveAxiom
      SaveOntologyGraph
      StoreInferenceResults
    }
    class OntologyQueries {
      <<src/application/ontology/queries.rs>>
      LoadOntologyGraph
      GetOwlClass
      GetOwlProperty
      ListOwlClasses
      ListOwlProperties
      GetClassAxioms
      GetInferenceResults
      GetOntologyMetrics
      QueryOntology
      ValidateOntology
    }
    class GraphQueries {
      <<src/application/graph/queries.rs>>
      GetGraphData
      GetBotsGraphData
      GetNodeMap
      GetPhysicsState
      GetConstraints
      GetEquilibriumStatus
      GetAutoBalanceNotifications
      ComputeShortestPaths
    }
    class PhysicsService {
      <<src/application/physics_service.rs>>
    }
    class SemanticService {
      <<src/application/semantic_service.rs>>
    }
    class InferenceService {
      <<REMOVED ADR-2066 — was src/application/inference_service.rs>>
      never constructed anywhere
      InferenceEvent
      InferenceServiceConfig
    }
    class DomainEvent {
      <<src/application/events.rs>>
    }
    OntologyDirectives ..> DomainEvent
    GraphQueries ..> PhysicsService
    OntologyQueries ..> InferenceService
    SemanticService ..> DomainEvent
    InferenceService ..> DomainEvent
```

## VC-07.7 A CQRS call in flight — GET /api/config
```mermaid
sequenceDiagram
    autonumber
    participant CL as client
    participant H as get_app_config<br/>src/handlers/api_handler/mod.rs:37
    participant EX as execute_in_thread<br/>src/handlers/utils.rs
    participant QH as LoadAllSettingsHandler<br/>src/application/settings
    participant PT as SettingsRepository port
    participant AD as SqliteSettingsRepository<br/>src/adapters/sqlite_settings_repository.rs:378

    CL->>H: GET /api/config
    H->>QH: LoadAllSettingsHandler::new(state.settings_repository.clone()) (src/handlers/api_handler/mod.rs:43)
    Note over H,QH: state.settings_repository is Arc<dyn SettingsRepository> — src/app_state.rs:309
    H->>EX: execute_in_thread(move || handler.handle(LoadAllSettings)) (:45)
    Note over EX: hexser QueryHandler trait — the direct dispatch path, no CQRS bus
    EX->>QH: handle(LoadAllSettings)
    QH->>PT: port call
    PT->>AD: SQLite read
    alt Ok(Ok(Some(settings)))
        AD-->>H: 200 with version, features{ragflow, perplexity, openai, kokoro, whisper},<br/>websocket{minUpdateRate, maxUpdateRate, motionThreshold, motionDamping},<br/>rendering{ambientLightIntensity, enableAmbientOcclusion, backgroundColor},<br/>xr{enabled, roomScale, spaceType} (:47-70)
    else Ok(Ok(None))
        AD-->>H: warn "No settings found, using defaults" then AppFullSettings::default() (:71-77)
    end
    Note over H,AD: comment src/application/mod.rs:70 — application services were removed,<br/>handlers use actors directly via CQRS or direct messaging. There is no dispatcher bus.
```

## VC-07.8 Storage-agnostic domain kernel — src/domain/broker
```mermaid
classDiagram
    class BrokerCase {
      <<src/domain/broker/broker_case.rs — 490 lines>>
    }
    class BrokerDecision {
      <<src/domain/broker/broker_decision.rs — 437 lines>>
    }
    class PrecedentRegistry {
      <<src/domain/broker/precedent_registry.rs — 101 lines>>
    }
    class BrokerModule {
      <<src/domain/broker/mod.rs — 43 lines>>
    }
    BrokerModule --> BrokerCase
    BrokerModule --> BrokerDecision
    BrokerModule --> PrecedentRegistry
    class SqliteEnrichmentRepository {
      <<src/adapters/sqlite_enrichment_repository.rs>>
      EnrichmentProposal
      StoredDecision
      EnrichmentStoreError
    }
    SqliteEnrichmentRepository ..> BrokerDecision
```

## VC-07.9 Broker kernel placement and the never-merged BrokerActor
```mermaid
sequenceDiagram
    autonumber
    participant H as REST handlers<br/>enrichment_proposals / broker_inbox / decision
    participant K as domain broker kernel<br/>src/domain/broker
    participant AD as SqliteEnrichmentRepository<br/>src/adapters/sqlite_enrichment_repository.rs
    participant AC as ACSP producer

    Note over K: src/domain/mod.rs:1-5 — pure aggregate and value-object logic with no transport<br/>or persistence dependency. Adapters (SQLite, Oxigraph, the ACSP producer, the REST<br/>handlers) sit OUTSIDE this module and depend inward on it.
    H->>K: aggregate operations on BrokerCase / BrokerDecision
    K-->>H: pure result — no I/O
    H->>AD: persist StoredDecision / EnrichmentProposal
    H->>AC: emit ACSP event
    Note over AC: stateless producer — there is no BrokerActor
    Note over H,AC: DIVERGENCE (BASELINE l.224) — BrokerActor was never merged. main uses a stateless<br/>ACSP producer plus a cherry-picked storage-agnostic domain broker kernel (~936 LOC).<br/>Measured here: broker_case.rs 490 + broker_decision.rs 437 = 927 lines, 1071 including<br/>mod.rs and precedent_registry.rs — consistent with the doc's ~936.
    Note over H,AC: DIVERGENCE (BASELINE ACSP workflow closeout 2026-09-04, l.273) — the retained kernel's<br/>presence does not prove integration into the elevation actor or the inbox DTO.<br/>Current source review does not certify a complete human-approval journey. See VC-05.
    Note over K,AD: ADR-2016 provenance append-only applies to the decision record — see VC-22
```

## VC-07.10 Error taxonomy — src/errors/mod.rs
```mermaid
classDiagram
    class VisionClawError {
      <<src/errors/mod.rs:20 — root enum>>
    }
    class ActorError { <<:60>> }
    class GPUError { <<:91>> }
    class DataTransferDirection { <<:117>> }
    class SettingsError { <<:123>> }
    class NetworkError { <<:145>> }
    class SpeechError { <<:177>> }
    class GitHubError { <<:190>> }
    class AudioError { <<:216>> }
    class ResourceError { <<:227>> }
    class PerformanceError { <<:240>> }
    class ProtocolError { <<:257>> }
    class DatabaseError { <<:268>> }
    class ValidationError { <<:283>> }
    class ParseError { <<:317>> }
    VisionClawError <|-- ActorError
    VisionClawError <|-- GPUError
    VisionClawError <|-- SettingsError
    VisionClawError <|-- NetworkError
    VisionClawError <|-- SpeechError
    VisionClawError <|-- GitHubError
    VisionClawError <|-- AudioError
    VisionClawError <|-- ResourceError
    VisionClawError <|-- PerformanceError
    VisionClawError <|-- ProtocolError
    VisionClawError <|-- DatabaseError
    VisionClawError <|-- ValidationError
    VisionClawError <|-- ParseError
    GPUError ..> DataTransferDirection
```

## VC-07.11 Feature-gated adapter slots
```mermaid
flowchart TB
    G["cfg(feature = 'gpu') — src/adapters/mod.rs"]
    G --> G1["gpu_semantic_analyzer (:19-20)<br/>GpuSemanticAnalyzerAdapter (:45-46)"]
    G --> G2["actix_physics_adapter (:37-38)<br/>ActixPhysicsAdapter (:75-76)"]
    NG["always compiled"]
    NG --> N1["actor_graph_repository::ActorGraphRepository (:17, :43)<br/>actix_semantic_adapter::ActixSemanticAdapter (:39, :77)<br/>physics_orchestrator_adapter (:41)"]
    NG --> N2["oxigraph_graph_repository::OxigraphGraphRepository (:49, :57)<br/>sqlite_settings_repository::SqliteSettingsRepository (:50, :71)"]
    NG --> N3["sqlite_enrichment_repository (:52) — WS-9 store<br/>sqlite_canary_repository (:54) — RES-a<br/>sqlite_kpi_repository (:56) — REC-4 ADR-130 D5"]
    G1 --> NG
    D["DOC-DRIFT — the adapters module doc src/adapters/mod.rs:8-15 lists seven<br/>modules as still in webxr, resolved in Phase A3. They are still there<br/>at this commit, so Phase A3 has not landed. ADR-2005 partial."]
    NG --- D
    N9["GPU internals behind these adapter slots see VC-10<br/>the dev build feature set is gpu,ontology,dev-auth — see VC-08"]
    G --- N9
```
