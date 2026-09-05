---
id: VC-02
title: Actor supervision tree, GraphServiceSupervisor routing and peer actor surfaces
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2005, ADR-2007, ADR-2045]
sources:
  - src/app_state.rs
  - src/main.rs
  - src/actors/graph_service_supervisor.rs
  - src/actors/event_coordination.rs
  - src/actors/graph_state_actor.rs
  - src/actors/physics_orchestrator_actor.rs
  - src/actors/client_coordinator_actor.rs
  - src/actors/client_filter.rs
  - src/actors/metadata_actor.rs
  - src/actors/ontology_actor.rs
  - src/actors/semantic_processor_actor.rs
  - src/actors/optimized_settings_actor.rs
  - src/actors/protected_settings_actor.rs
  - src/actors/workspace_actor.rs
  - src/actors/presence_actor.rs
  - src/actors/task_orchestrator_actor.rs
  - src/actors/elevation_actor.rs
  - src/actors/elevation_voice.rs
  - src/actors/decision_elevation_actor.rs
  - src/actors/voice_interface_actor.rs
  - src/actors/voice_commands.rs
  - src/actors/multi_mcp_visualization_actor.rs
  - src/actors/agent_monitor_actor.rs
  - src/actors/agent_beam_actor.rs
verified_commit: b00c28a0d
---

## VC-02.1 Supervision-tree topology — AppState::new boot order
```mermaid
flowchart TB
    APP["AppState::new<br/>src/app_state.rs:410"]
    CC["ClientCoordinatorActor<br/>src/app_state.rs:709<br/>Addr~ClientCoordinatorActor~"]
    AB["AgentBeamActor<br/>src/app_state.rs:719<br/>no Addr held — hub-subscription lifetime"]
    MD["MetadataActor<br/>src/app_state.rs:802<br/>Addr~MetadataActor~"]
    GSS["GraphServiceSupervisor<br/>src/app_state.rs:807-809<br/>Addr~GraphServiceSupervisor~"]
    REBIND["SetClientCoordinatorAddr<br/>src/app_state.rs:818"]
    GPU["GPUManagerActor boundary<br/>src/app_state.rs:965<br/>Note: GPU internals see VC-10"]
    SPA["gpu::ShortestPathActor<br/>src/app_state.rs:969<br/>Addr~ShortestPathActor~"]
    CCA["gpu::ConnectedComponentsActor<br/>src/app_state.rs:970<br/>Addr~ConnectedComponentsActor~"]
    SET["OptimizedSettingsActor<br/>src/app_state.rs:1170<br/>Addr~OptimizedSettingsActor~"]
    AM["AgentMonitorActor<br/>src/app_state.rs:1193<br/>Addr~AgentMonitorActor~"]
    PS["ProtectedSettingsActor<br/>src/app_state.rs:1206<br/>Addr~ProtectedSettingsActor~"]
    WS["WorkspaceActor<br/>src/app_state.rs:1209<br/>Addr~WorkspaceActor~"]
    ONT["OntologyActor<br/>src/app_state.rs:1222<br/>Option~Addr~OntologyActor~~"]
    TO["TaskOrchestratorActor<br/>src/app_state.rs:1272<br/>Addr~TaskOrchestratorActor~"]
    EL["ElevationActor<br/>src/app_state.rs:1292<br/>anon start — no Addr retained"]
    VI["VoiceInterfaceActor<br/>src/app_state.rs:1308<br/>anon start — no Addr retained"]
    DE["DecisionElevationActor<br/>src/main.rs:541<br/>started OUTSIDE AppState::new"]

    APP --> CC
    CC --> AB
    CC -.->|"rebind"| REBIND
    AB --> MD
    MD --> GSS
    GSS -.->|"do_send"| REBIND
    GSS --> GPU
    GPU --> SPA
    SPA --> CCA
    CCA --> SET
    SET --> AM
    AM --> PS
    PS --> WS
    WS --> ONT
    ONT --> TO
    TO --> EL
    EL --> VI
    VI -.->|"same process, later in main()"| DE

    N1["INVARIANT (BASELINE Invariants) — the live ClientCoordinatorActor clients register<br/>with must be the CC instance above, not GraphServiceSupervisor's own child.<br/>REBIND at :818 makes GSS forward broadcasts through CC's non-empty registry."]
    REBIND --- N1
```

## VC-02.2 GraphServiceSupervisor::initialize_actors — child start order and wiring
```mermaid
sequenceDiagram
    autonumber
    participant S as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:601
    participant CC as ClientCoordinatorActor<br/>src/actors/graph_service_supervisor.rs:730
    participant PO as PhysicsOrchestratorActor<br/>src/actors/graph_service_supervisor.rs:719
    participant SP as SemanticProcessorActor<br/>src/actors/graph_service_supervisor.rs:726
    participant GS as GraphStateActor<br/>src/actors/graph_service_supervisor.rs:709

    Note over S: initialize_actors src/actors/graph_service_supervisor.rs:601 — 4 ActorInfo rows inserted first
    S->>S: start_actor(ClientCoordinator) (src/actors/graph_service_supervisor.rs:657)
    S->>CC: ClientCoordinatorActor::new().start() (:730)
    S->>S: wire_physics_and_client() — fires on ClientCoordinator or PhysicsOrchestrator start (:734-737)
    S->>S: start_actor(PhysicsOrchestrator) (:658)
    S->>PO: PhysicsOrchestratorActor::new(SimulationParams::default(), None, None).start() (:719)
    S->>S: wire_physics_and_client() again
    S->>S: start_actor(SemanticProcessor) (:659)
    S->>SP: SemanticProcessorActor::new(config).start() (:726)
    S->>S: start_actor(GraphState) (:660)
    alt kg_repo present
        S->>GS: GraphStateActor::new(kg_repo.clone()).start() (:709)
    else kg_repo absent
        S->>S: error! "Cannot start GraphStateActor without kg_repo" (:712)
    end
    S->>S: ctx.run_interval(health_check_interval=30s, perform_health_check) (:665-667)
    S->>S: ctx.run_interval(15s, emit ActorHeartbeat per live child) (:672-689)
    S->>S: supervision_stats.actors_supervised = 4 (:691)
    Note over S: SetClientCoordinatorAddr src/actors/graph_service_supervisor.rs:1554-1560 —<br/>AppState sends this AFTER initialize_actors to overwrite the CC child above<br/>with the application's canonical ClientCoordinatorActor (see VC-02.1)
```

## VC-02.3 GraphServiceSupervisor message routing table
```mermaid
flowchart LR
    subgraph SUP["GraphServiceSupervisor handlers — grep impl Handler< src/actors/graph_service_supervisor.rs"]
        H1["SupervisorMessage :1432"]
        H2["ActorHeartbeat :1440"]
        H3["GetSupervisorStatus :1455"]
        H4["RestartActor :1463"]
        H5["RestartAllActors :1472"]
        H6["SetParentSupervisor :1481"]
        H7["msgs::GetGraphData :1495"]
        H8["NotifyGraphUpdated :1530"]
        H9["SetClientCoordinatorAddr :1554"]
        H10["msgs::ReloadGraphFromDatabase :1564"]
        H11["msgs::ComputeShortestPaths :1668"]
        H12["msgs::UpdateGraphData :1694"]
        H13["msgs::AddNodesFromMetadata :1724"]
        H14["msgs::StartSimulation :1750"]
        H15["msgs::SimulationStep :1767"]
        H16["msgs::GetBotsGraphData :1794"]
        H17["msgs::UpdateSimulationParams :1827"]
        H18["msgs::ForceResumePhysics :1855"]
        H19["msgs::InitializeGPUConnection :1884"]
        H20["msgs::SetAppGpuComputeAddr :1982"]
        H21["msgs::UpdateBotsGraph :1996"]
        H22["msgs::UpdateNodePositions :2013"]
        H23["msgs::NodeInteractionMessage :2040"]
        H24["msgs::GetGraphStateActor :2108"]
        H25["msgs::GetPhysicsOrchestratorActor :2117"]
        H26["msgs::GetNodeTypeArrays :2132"]
        H27["msgs::GetNodeIdMapping :2154"]
        H28["msgs::AddEdge :2176"]
    end
    GS["self.graph_state (GraphStateActor)"]
    PH["self.physics (PhysicsOrchestratorActor)"]
    CL["self.client (ClientCoordinatorActor)"]
    GM["self.gpu_manager (GPUManagerActor) — boundary, see VC-10"]
    SELF["self (in-place state)"]

    H7 --> GS
    H10 --> GS
    H11 --> GS
    H12 --> GS
    H13 --> GS
    H16 --> GS
    H21 --> GS
    H26 --> GS
    H27 --> GS
    H28 --> GS
    H22 -->|"do_send both"| GS
    H22 --> PH
    H14 --> PH
    H15 --> PH
    H17 --> PH
    H18 --> PH
    H23 --> PH
    H25 --> PH
    H19 -->|"query GetForceComputeActor, InitializeGPU"| GM
    H19 -->|"StoreGPUComputeAddress"| PH
    H1 --> SELF
    H2 --> SELF
    H3 --> SELF
    H4 --> SELF
    H5 --> SELF
    H6 --> SELF
    H8 --> SELF
    H9 -->|"self.client = msg.addr, rewire"| CL
    H20 --> SELF
    H24 --> SELF

    N1["H12/H13/H28 also call notify_graph_updated (debounced graphUpdated broadcast) before forwarding<br/>H24/H26/H27 default-value fallback when the target child is None (never propagate an error to caller)"]
    H12 --- N1
```

## VC-02.4 Generic SupervisorActor (src/actors/supervisor.rs) restart/backoff lifecycle
```mermaid
stateDiagram-v2
    [*] --> Running
    Running --> Failed: ActorFailed received (src/actors/supervisor.rs:293)
    Failed --> RestartScheduled: strategy Restart or RestartWithBackoff, should_restart true (:300-321, :326)
    Failed --> GivenUp: should_restart false — restart_count over max_restart_count within restart_window (:302-308)
    Failed --> EscalatedNoop: strategy Escalate — logs a warning only, no message sent anywhere (:344-349)
    Failed --> Stopped: strategy Stop — is_running set false (:350-358)
    RestartScheduled --> RestartAttemptFired: ctx.run_later(delay), ctx.notify RestartAttempt (:195-202)
    RestartAttemptFired --> Running: actor_factory Some, factory invoked, is_running true (:428-448)
    RestartAttemptFired --> GivenUp: actor_factory None, is_running false (:449-457)
    Running --> Draining: InitiateGracefulShutdown received, draining=true (:233)
    Draining --> Stopped: run_later(timeout_secs) elapses, ctx.stop (:235-238)
    note right of RestartScheduled
        calculate_restart_delay src/actors/supervisor.rs:159
        delay = current_delay times multiplier, capped at max_delay (RestartWithBackoff only)
        SupervisedActorTrait default src/actors/supervisor.rs:468-482
        initial_delay=1s max_delay=60s multiplier=2.0 max_restart_count=5 restart_window=300s
    end note
    note right of EscalatedNoop
        DIVERGENCE — Escalate here is a dead branch: it warn-logs and returns,
        it never calls a parent Addr (there is no parent field on SupervisorActor at all)
    end note
```

## VC-02.5 Generic SupervisorActor restart sequence — failure to give-up
```mermaid
sequenceDiagram
    autonumber
    participant C as supervised actor (via SupervisedActorTrait::report_error)<br/>src/actors/supervisor.rs:484-489
    participant S as SupervisorActor<br/>src/actors/supervisor.rs:290
    participant F as actor_factory<br/>Option Arc dyn Fn Box dyn Any

    C->>S: ActorFailed { actor_name, error }
    S->>S: state.is_running = false (:297)
    S->>S: should_restart = restart_count < max_restart_count OR restart_window elapsed (:300-319)
    alt strategy Restart or RestartWithBackoff, should_restart true
        S->>S: restart_actor(actor_name, ctx) (:326, :336)
        S->>S: delay = calculate_restart_delay(state) (:159, :177)
        loop ctx.run_later(delay) (:195)
            S->>S: ctx.notify(RestartAttempt { actor_name, supervisor_name }) (:198-201)
        end
        S->>S: RestartAttempt handler fires (:421)
        alt actor_factory Some
            S->>F: factory_clone() — construct and start a new Actix actor (:442-443)
            F-->>S: Box dyn Any (opaque) — state.is_running = true (:444)
        else actor_factory None
            S->>S: warn! no actor_factory registered, is_running = false (:449-456)
        end
    else should_restart false (restart_count over max_restart_count within restart_window)
        S->>S: error! "will not be restarted (too many failures)" (:328-331, :338-341)
    else strategy Escalate
        S->>S: warn! escalating (log only — no send, see VC-02.4 DIVERGENCE) (:344-348)
    else strategy Stop
        S->>S: is_running = false, no restart attempted (:350-357)
    end
    Note over S: ADR-01 D4 — factory call is NOT wrapped in catch_unwind on purpose (comment :429-434):<br/>a CUDA-launch panic in the factory must propagate to the actix runtime at full backtrace fidelity
```

## VC-02.6 Drain and shutdown — supervisor.rs InitiateGracefulShutdown, GraphServiceSupervisor stop, and lifecycle.rs
```mermaid
sequenceDiagram
    autonumber
    participant Caller as caller (ADR-031 item 7)
    participant SA as SupervisorActor<br/>src/actors/supervisor.rs:219
    participant Sys as actix::System
    participant S as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:816
    participant LM as ActorLifecycleManager<br/>src/actors/lifecycle.rs:17
    participant PO as PhysicsOrchestratorActor
    participant SE as SemanticProcessorActor

    Caller->>SA: InitiateGracefulShutdown { timeout_secs } (src/actors/supervisor.rs:117-122)
    SA->>SA: draining = true — RegisterActor now rejected (:233, :247-256)
    loop ctx.run_later(timeout_secs) (:235)
        SA->>SA: "Drain timeout elapsed — stopping supervisor", ctx.stop() (:236-237)
    end
    Note over Caller,SA: mirrors Multica's WaitGroup-based 30-second drain pattern (comment :115)<br/>DIVERGENCE — SupervisorActor::new is never called and InitiateGracefulShutdown is never sent<br/>anywhere in src/ outside its own test module (grep confirmed) — this whole actor is dead code<br/>in the live tree, which only runs GraphServiceSupervisor (VC-02.1, VC-02.2)
    Note over S: escalate_failure Escalate-with-no-parent path calls ctx.stop() (src/actors/graph_service_supervisor.rs:816)<br/>this is the only explicit drain/stop call found in graph_service_supervisor.rs
    Sys->>S: ctx.stop() (child restart escalation exhausted, no parent)
    S-->>Sys: Actor::stopping / stopped (Actix default lifecycle, no override found)
    Note over LM: ActorLifecycleManager::shutdown_with_timeout src/actors/lifecycle.rs:114-153 —<br/>a SEPARATE PhysicsOrchestratorActor + SemanticProcessorActor pair, distinct from the ones<br/>GraphServiceSupervisor::initialize_actors starts (VC-02.2)
    LM->>PO: check addr.connected() (:126-130)
    LM->>SE: check addr.connected() (:133-137)
    LM->>LM: tokio::time::sleep(shutdown_timeout) to drain in-flight messages (:140)
    LM->>PO: drop(addr) — last Addr clone drop triggers Actix stopping/stopped (:144-146)
    LM->>SE: drop(addr) (:149-151)
    Note over LM: shutdown() src/actors/lifecycle.rs:157-159 wraps shutdown_with_timeout with a 5s default
```

## VC-02.7 DIVERGENCE — lifecycle.rs's ActorLifecycleManager is dead code
```mermaid
flowchart TB
    LM["ActorLifecycleManager<br/>src/actors/lifecycle.rs:17-207<br/>owns its OWN PhysicsOrchestratorActor + SemanticProcessorActor"]
    ACTOR_SYS["static ACTOR_SYSTEM<br/>src/actors/lifecycle.rs:277"]
    INIT["initialize_actor_system()<br/>src/actors/lifecycle.rs:280"]
    SHUT["shutdown_actor_system()<br/>src/actors/lifecycle.rs:285"]
    REEXPORT["re-exported from src/actors/mod.rs:102-103"]
    CALLERS["callers of initialize_actor_system / shutdown_actor_system"]
    NONE["none found — grep across src/ and crates/ hits only\nthe definition and the mod.rs re-export"]

    LM --> ACTOR_SYS
    ACTOR_SYS --> INIT
    ACTOR_SYS --> SHUT
    INIT --> REEXPORT
    SHUT --> REEXPORT
    REEXPORT -.->|"searched, not invoked"| CALLERS
    CALLERS --> NONE
    N1["DIVERGENCE — this ActorLifecycleManager, its SupervisionStrategy (max_restarts=3,<br/>restart_window=60s, src/actors/lifecycle.rs:231-237) and its health-monitor loop are a complete,<br/>independent second supervision mechanism for PhysicsOrchestratorActor/SemanticProcessorActor<br/>that never runs — the live tree only ever uses GraphServiceSupervisor (VC-02.1, VC-02.2)."]
    NONE --- N1
```

## VC-02.8 event_coordination.rs — coordination event publish/consume (ADR-2007 partial)
```mermaid
sequenceDiagram
    autonumber
    participant Src as coordinating actor
    participant EC as event_coordination module<br/>src/actors/event_coordination.rs

    Src->>EC: direct Actix message send (do_send/send)
    opt optional bus publication path exists
        EC->>EC: publish onto an event bus channel
    end
    Note over Src,EC: DIVERGENCE (docs/BASELINE-architecture.md "Crate and supervision closeout — 2026-09-04" l.279):<br/>ADR-2007 is partial — four supervisors exist, but context delivery uses direct messages<br/>plus optional bus publication, with no acknowledged context generations, no<br/>responsibility/dependency acceptance and no failure/restart evidence recorded.
```

## VC-02.9 Message catalogue — client_messages, graph_messages, broadcast_messages
```mermaid
classDiagram
    class ClientCoordinatorActor {
      <<actor>>
      src/actors/client_coordinator_actor.rs
    }
    class RegisterClient {
      +ClientRecipients recipients
      result() Result~usize,String~
    }
    class ClientRecipients {
      +Recipient~SendToClientBinary~ binary
      +Recipient~SendToClientText~ text
      +Recipient~SendInitialGraphLoad~ initial_load
    }
    class UnregisterClient {
      +usize client_id
    }
    class BroadcastMessage {
      +String message
    }
    class GraphServiceSupervisor {
      <<actor>>
    }
    class GetGraphData {
      result() Result~Arc,String~
    }
    class UpdateGraphData {
      +Arc~ServiceGraphData~ graph_data
      result() Result~(),String~
    }
    class UpdateNodePositions {
      +List~u32,BinaryNodeData~ positions
      +Option~MessageId~ correlation_id
      result() Result~(),String~
    }
    class AddNodesFromMetadata {
      +MetadataStore metadata
      result() Result~(),String~
    }
    class AddEdge {
      +Edge edge
      result() Result~(),String~
    }
    class NodeInteractionMessage {
      +u32 node_id
      +NodeInteractionType interaction_type
      +Option~List~f32~~ position
      result() Result~(),VisionClawError~
    }
    RegisterClient *-- ClientRecipients
    ClientCoordinatorActor ..> RegisterClient
    ClientCoordinatorActor ..> UnregisterClient
    ClientCoordinatorActor ..> BroadcastMessage
    GraphServiceSupervisor ..> GetGraphData
    GraphServiceSupervisor ..> UpdateGraphData
    GraphServiceSupervisor ..> UpdateNodePositions
    GraphServiceSupervisor ..> AddNodesFromMetadata
    GraphServiceSupervisor ..> AddEdge
    GraphServiceSupervisor ..> NodeInteractionMessage
    note for GetGraphData "src/actors/messages/ — RegisterClient/UnregisterClient/BroadcastMessage\nowned by client_coordinator_actor.rs consumer; Result types confirmed from\nHandler impls at graph_service_supervisor.rs:1495,1694,2013,1724,2176,2040"
```

## VC-02.10 Message catalogue — supervision and heartbeat messages
```mermaid
classDiagram
    class GraphServiceSupervisor {
      <<actor>>
    }
    class SupervisorMessage {
      <<enum>>
      UpdateGraphData
      ReloadGraphFromDatabase
      StartSimulation
      StopSimulation
      SimulationStep
      UpdateSimulationParams
      UpdateNodePositions
      BroadcastMessage
      result() Result~(),VisionClawError~
    }
    class ActorHeartbeat {
      +ActorType actor_type
      +Instant timestamp
      +ActorHealth health
      +Option~ActorStats~ stats
    }
    class GetSupervisorStatus {
      result() SupervisorStatus
    }
    class RestartActor {
      +ActorType actor_type
      result() Result~(),VisionClawError~
    }
    class RestartAllActors {
      result() Result~(),VisionClawError~
    }
    class SetParentSupervisor {
      +Addr~SupervisorActor~ parent
    }
    class SetClientCoordinatorAddr {
      +Addr~ClientCoordinatorActor~ addr
    }
    class NotifyGraphUpdated {
      +str reason
    }
    class ActorFailed {
      +String actor_name
      +VisionClawError error
    }
    GraphServiceSupervisor ..> SupervisorMessage
    GraphServiceSupervisor ..> ActorHeartbeat
    GraphServiceSupervisor ..> GetSupervisorStatus
    GraphServiceSupervisor ..> RestartActor
    GraphServiceSupervisor ..> RestartAllActors
    GraphServiceSupervisor ..> SetParentSupervisor
    GraphServiceSupervisor ..> SetClientCoordinatorAddr
    GraphServiceSupervisor ..> NotifyGraphUpdated
    GraphServiceSupervisor ..> ActorFailed
    note for SetClientCoordinatorAddr "src/actors/graph_service_supervisor.rs:1550-1560 —\nfield and handler both confirmed by direct read"
```

## VC-02.11 GraphStateActor — representative read and write sequence
```mermaid
sequenceDiagram
    autonumber
    participant Caller as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:1495
    participant GS as GraphStateActor<br/>src/actors/graph_state_actor.rs:938

    Caller->>GS: GetGraphData
    GS->>GS: handler at src/actors/graph_state_actor.rs:938
    GS-->>Caller: Result~Arc~GraphData~_String~
    Note over Caller,GS: write path
    Caller->>GS: UpdateNodePositions (src/actors/graph_state_actor.rs:858)
    GS->>GS: store positions so polling GetGraphData reflects GPU layout
    GS-->>Caller: ack
    Caller->>GS: AddNode (src/actors/graph_state_actor.rs:947)
    GS-->>Caller: ack
```

## VC-02.12 PhysicsOrchestratorActor — handled messages and broadcast invariant
```mermaid
sequenceDiagram
    autonumber
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:719
    participant PO as PhysicsOrchestratorActor<br/>src/actors/physics_orchestrator_actor.rs
    participant GPU as GPU boundary<br/>see VC-10
    participant CC as ClientCoordinatorActor

    GSS->>PO: StartSimulation (src/actors/graph_service_supervisor.rs:1750)
    GSS->>PO: SimulationStep (:1767)
    GSS->>PO: UpdateSimulationParams (:1827)
    GSS->>PO: ForceResumePhysics (:1855)
    GSS->>PO: NodeInteractionMessage (:2040)
    GSS->>PO: StoreGPUComputeAddress (:1884-1911, from GPUManagerActor GetForceComputeActor reply)
    GSS->>PO: UpdateNodePositions (:2013, forwarded alongside GraphStateActor)
    Note over GPU,CC: INVARIANT (BASELINE Invariants) — position broadcast path is<br/>GPU to ForceComputeActor to PhysicsOrchestratorActor to ClientCoordinatorActor to WebSocket
    GPU->>PO: computed positions (GPU internals boundary only, see VC-10)
    PO->>CC: forward for WebSocket push
```

## VC-02.13 ClientCoordinatorActor + client_filter.rs — actor-side message surface
```mermaid
sequenceDiagram
    autonumber
    participant GSS as GraphServiceSupervisor
    participant CC as ClientCoordinatorActor<br/>src/actors/client_coordinator_actor.rs
    participant CF as client_filter<br/>src/actors/client_filter.rs

    GSS->>CC: SetClientCoordinatorAddr rebind (src/actors/graph_service_supervisor.rs:1554-1560)
    Note over CC: registers/unregisters client connections, dispatches BroadcastMessage / graphUpdated
    CC->>CF: apply per-client visibility filter before dispatch
    Note over CF: request-side visibility gate is see VC-03 — this file covers the actor side only
```

## VC-02.14 MetadataActor and OntologyActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:802
    participant MD as MetadataActor<br/>src/actors/metadata_actor.rs
    participant ONT as OntologyActor<br/>src/app_state.rs:1211-1223
    participant GPU as GPUManagerActor boundary
    participant CC as ClientCoordinatorActor

    APP->>MD: MetadataActor::new(MetadataStore::new()).start() (:802)
    APP->>ONT: OntologyActor::new()
    alt gpu_manager_addr Some
        APP->>ONT: set_gpu_manager_addr(gpu_mgr) (src/app_state.rs:1217)
        Note over ONT,GPU: wired to GPUManagerActor for constraint pipeline — GPU internals see VC-10
    end
    APP->>ONT: set_client_manager_addr(client_manager_addr) (src/app_state.rs:1220)
    Note over ONT,CC: wired for WebSocket broadcasts of ontology changes
    APP->>ONT: ontology_actor.start() (:1222) — Option~Addr~ retained
```

## VC-02.15 SemanticProcessorActor and OptimizedSettingsActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:721-726
    participant SP as SemanticProcessorActor<br/>src/actors/semantic_processor_actor.rs
    participant APP as AppState::new<br/>src/app_state.rs:1161
    participant SET as OptimizedSettingsActor<br/>src/actors/optimized_settings_actor.rs
    participant REDIS as REDIS_URL<br/>src/actors/optimized_settings_actor.rs:146

    GSS->>SP: SemanticProcessorActor::new(SemanticProcessorConfig::default()).start() (:723-726)
    APP->>SET: OptimizedSettingsActor::with_actors(sqlite_settings_repository, Some(graph_service_addr), None) (:1161-1168)
    SET->>SET: settings_actor.start() (:1170)
    alt REDIS_URL set (src/actors/optimized_settings_actor.rs:146)
        SET->>REDIS: connect for distributed settings cache
    else REDIS_URL unset
        SET->>SET: local-only settings path
    end
    Note over SET: settings internals are VC-06 — this file shows only the actor's message surface
```

## VC-02.16 ProtectedSettingsActor and WorkspaceActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:1204-1209
    participant PS as ProtectedSettingsActor<br/>src/actors/protected_settings_actor.rs
    participant WS as WorkspaceActor<br/>src/actors/workspace_actor.rs

    APP->>PS: ProtectedSettingsActor::new(ProtectedSettings::default()).start() (:1206)
    Note over PS: GetApiKeys handler referenced src/app_state.rs:1610
    APP->>WS: WorkspaceActor::new().start() (:1209)
```

## VC-02.17 PresenceActor and TaskOrchestratorActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant XR as XR client
    participant PA as PresenceActor<br/>src/actors/presence_actor.rs:46
    participant APP as AppState::new<br/>src/app_state.rs:1261-1272
    participant TO as TaskOrchestratorActor<br/>src/actors/task_orchestrator_actor.rs:68
    participant MGMT as ManagementApiClient

    XR->>PA: hand-presence update
    alt PRESENCE_HAND_REACH_M set (src/actors/presence_actor.rs:46, read again :1060)
        PA->>PA: use configured reach metres
    else unset
        PA->>PA: use built-in default reach
    end
    APP->>TO: TaskOrchestratorActor::new(mgmt_client).start() (:1272)
    alt MAX_CONCURRENT_TASKS set (src/actors/task_orchestrator_actor.rs:68)
        TO->>TO: cap concurrent task dispatch to the configured value
    else unset
        TO->>TO: use built-in default cap
    end
    TO->>MGMT: dispatch orchestrated task
```

## VC-02.18 ElevationActor (+ elevation_voice.rs) and DecisionElevationActor
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:1279-1298
    participant EL as ElevationActor<br/>src/actors/elevation_actor.rs:173
    participant EV as elevation_voice<br/>src/actors/elevation_voice.rs
    participant MAIN as main<br/>src/main.rs:541
    participant DE as DecisionElevationActor<br/>src/actors/decision_elevation_actor.rs:119

    alt ELEVATION_ACTOR_ENABLED gate passes (src/actors/elevation_actor.rs:173)
        APP->>EL: ElevationActor::new(graph_adapter, sqlite_enrichment_repository, speech_service, Some(ontology_repository)).start() (:1292)
        Note over EL,EV: voice-guided path when local speech stack (Whisper/Kokoro) is up — elevation_voice.rs
    else gate closed
        APP->>APP: log "ElevationActor disabled" (src/app_state.rs:1295-1297)
    end
    Note over MAIN,DE: DecisionElevationActor is started in main() src/main.rs:541, NOT in AppState::new —<br/>a second, separately-gated ACSP actor family alongside ElevationActor
    alt DECISION_ELEVATION_ENABLED gate passes (src/actors/decision_elevation_actor.rs:119)
        MAIN->>DE: DecisionElevationActor::new() then actix::Actor::start(actor) (src/main.rs:541-543)
        MAIN->>MAIN: wrap in ActorElevationSink, feed DecisionService.with_elevation_sink (src/main.rs:544-556)
    else gate closed
        MAIN->>MAIN: log "DecisionElevationActor disabled" (src/main.rs:551)
    end
```

## VC-02.19 VoiceInterfaceActor (+ voice_commands.rs) and MultiMcpVisualizationActor
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:1305-1313
    participant VI as VoiceInterfaceActor<br/>src/actors/voice_interface_actor.rs
    participant VC as voice_commands<br/>src/actors/voice_commands.rs
    participant MMV as MultiMcpVisualizationActor<br/>src/actors/multi_mcp_visualization_actor.rs

    alt speech_service Some
        APP->>VI: VoiceInterfaceActor::new(task_orchestrator_addr.clone(), speech_service.clone()).start() (:1308)
        VI->>VC: dispatch parsed spoken command to settings-assistant path
    else speech_service None
        APP->>APP: log "VoiceInterfaceActor disabled (no speech service)" (:1312)
    end
    Note over MMV: MultiMcpVisualizationActor has no start() call found in src/app_state.rs or src/main.rs —<br/>coverage gap, reported not diagrammed further (see report)
```

## VC-02.20 AgentMonitorActor and AgentBeamActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:1178-1193
    participant AM as AgentMonitorActor<br/>src/actors/agent_monitor_actor.rs:495
    participant CFC as ClaudeFlowClient
    participant AB as AgentBeamActor<br/>src/app_state.rs:719
    participant CC as ClientCoordinatorActor
    participant HUB as agent-events hub

    APP->>CFC: ClaudeFlowClient::new(mcp_host, mcp_port) (:1187-1189)
    APP->>AM: AgentMonitorActor::new(claude_flow_client, graph_service_addr.clone()).start() (:1193)
    alt MOCK_AGENTS set (src/actors/agent_monitor_actor.rs:495)
        AM->>AM: synthesize mock agent roster, skip live MCP poll
    else unset
        AM->>CFC: poll live MCP agent roster
    end
    APP->>AB: AgentBeamActor::new(client_manager_addr.clone()).start() (:719)
    HUB->>AB: process-global agent-event stream (subscription keeps actor alive, no Addr retained)
    AB->>CC: encoded 0x23 frames via existing binary dispatch
```
