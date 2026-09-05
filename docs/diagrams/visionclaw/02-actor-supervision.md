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
  - crates/visionclaw-actors/src/supervisor.rs
  - tests/orchestration_improvements_test.rs
  - src/actors/mod.rs
verified_commit: b0bc275f6
---

## VC-02.1 Supervision-tree topology — AppState::new boot order
```mermaid
flowchart TB
    APP["AppState::new<br/>src/app_state.rs:413"]
    CC["ClientCoordinatorActor<br/>src/app_state.rs:709<br/>Addr~ClientCoordinatorActor~"]
    AB["AgentBeamActor<br/>src/app_state.rs:722<br/>no Addr held — hub-subscription lifetime"]
    MD["MetadataActor<br/>src/app_state.rs:805<br/>Addr~MetadataActor~"]
    GSS["GraphServiceSupervisor<br/>src/app_state.rs:810<br/>Addr~GraphServiceSupervisor~"]
    REBIND["SetClientCoordinatorAddr<br/>src/app_state.rs:821"]
    GPU["GPUManagerActor boundary<br/>src/app_state.rs:963<br/>Note: GPU internals see VC-10"]
    ANA["analytics actors — supervised, not started here<br/>src/app_state.rs:965-971<br/>ADR-2053 removed the standalone ShortestPathActor and<br/>ConnectedComponentsActor pair, see VC-10"]
    SET["OptimizedSettingsActor<br/>src/app_state.rs:1161<br/>Addr~OptimizedSettingsActor~"]
    AM["AgentMonitorActor<br/>src/app_state.rs:1184<br/>Addr~AgentMonitorActor~"]
    PS["ProtectedSettingsActor<br/>src/app_state.rs:1197<br/>Addr~ProtectedSettingsActor~"]
    WS["WorkspaceActor<br/>src/app_state.rs:1200<br/>Addr~WorkspaceActor~"]
    ONT["OntologyActor<br/>src/app_state.rs:1213<br/>Option~Addr~OntologyActor~~"]
    TO["TaskOrchestratorActor<br/>src/app_state.rs:1263<br/>Addr~TaskOrchestratorActor~"]
    EL["ElevationActor<br/>src/app_state.rs:1271<br/>anon start — no Addr retained"]
    VI["VoiceInterfaceActor<br/>src/app_state.rs:1294<br/>anon start — no Addr retained"]
    DE["DecisionElevationActor<br/>src/main.rs:544<br/>started OUTSIDE AppState::new"]

    APP --> CC
    CC --> AB
    CC -.->|"rebind"| REBIND
    AB --> MD
    MD --> GSS
    GSS -.->|"do_send"| REBIND
    GSS --> GPU
    GPU --> ANA
    ANA --> SET
    SET --> AM
    AM --> PS
    PS --> WS
    WS --> ONT
    ONT --> TO
    TO --> EL
    EL --> VI
    VI -.->|"same process, later in main()"| DE

    N1["INVARIANT (BASELINE Invariants) — the live ClientCoordinatorActor clients register<br/>with must be the CC instance above, not GraphServiceSupervisor's own child.<br/>REBIND at :821 makes GSS forward broadcasts through CC's non-empty registry."]
    REBIND --- N1
```

## VC-02.2 GraphServiceSupervisor::initialize_actors — child start order and wiring
```mermaid
sequenceDiagram
    autonumber
    participant S as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:594
    participant CC as ClientCoordinatorActor<br/>src/actors/graph_service_supervisor.rs:723
    participant PO as PhysicsOrchestratorActor<br/>src/actors/graph_service_supervisor.rs:712
    participant SP as SemanticProcessorActor<br/>src/actors/graph_service_supervisor.rs:719
    participant GS as GraphStateActor<br/>src/actors/graph_service_supervisor.rs:702

    Note over S: initialize_actors src/actors/graph_service_supervisor.rs:594 — 4 ActorInfo rows inserted first (:597-651)
    S->>S: start_actor(ClientCoordinator) (src/actors/graph_service_supervisor.rs:653)
    S->>CC: ClientCoordinatorActor::new().start() (:723)
    S->>S: wire_physics_and_client() — fires on ClientCoordinator or PhysicsOrchestrator start (:728-733)
    S->>S: start_actor(PhysicsOrchestrator) (:654)
    S->>PO: PhysicsOrchestratorActor::new(SimulationParams::default(), None, None).start() (:712)
    S->>S: wire_physics_and_client() again
    S->>S: start_actor(SemanticProcessor) (:655)
    S->>SP: SemanticProcessorActor::new(config).start() (:719)
    S->>S: start_actor(GraphState) (:656)
    alt kg_repo present
        S->>GS: GraphStateActor::new(kg_repo.clone()).start() (:702)
    else kg_repo absent
        S->>S: error! "Cannot start GraphStateActor without kg_repo" (:706)
    end
    S->>S: ctx.run_interval(health_check_interval=30s, perform_health_check) (:659-661)
    S->>S: ctx.run_interval(15s, emit ActorHeartbeat per live child) (:667-688)
    S->>S: supervision_stats.actors_supervised = 4 (:690)
    Note over S: SetClientCoordinatorAddr src/actors/graph_service_supervisor.rs:1524-1532 —<br/>AppState sends this AFTER initialize_actors to overwrite the CC child above<br/>with the application's canonical ClientCoordinatorActor (see VC-02.1)
```

## VC-02.3 GraphServiceSupervisor message routing table
```mermaid
flowchart LR
    subgraph SUP["GraphServiceSupervisor handlers — grep impl Handler< src/actors/graph_service_supervisor.rs"]
        H1["SupervisorMessage :1411"]
        H2["ActorHeartbeat :1419"]
        H3["GetSupervisorStatus :1434"]
        H4["RestartActor :1442"]
        H5["RestartAllActors :1451"]
        H6["RESOLVED ADR-2045 (2026-09-05)<br/>SetParentSupervisor handler removed<br/>with src/actors/supervisor.rs, see VC-02.7"]
        H7["msgs::GetGraphData :1465"]
        H8["NotifyGraphUpdated :1500"]
        H9["SetClientCoordinatorAddr :1524"]
        H10["msgs::ReloadGraphFromDatabase :1534"]
        H11["msgs::ComputeShortestPaths :1638"]
        H12["msgs::UpdateGraphData :1664"]
        H13["msgs::AddNodesFromMetadata :1694"]
        H14["msgs::StartSimulation :1720"]
        H15["msgs::SimulationStep :1737"]
        H16["msgs::GetBotsGraphData :1764"]
        H17["msgs::UpdateSimulationParams :1797"]
        H18["msgs::ForceResumePhysics :1825"]
        H19["msgs::InitializeGPUConnection :1854"]
        H20["msgs::SetAppGpuComputeAddr :1952"]
        H21["msgs::UpdateBotsGraph :1966"]
        H22["msgs::UpdateNodePositions :1983"]
        H23["msgs::NodeInteractionMessage :2010"]
        H24["msgs::GetGraphStateActor :2078"]
        H25["msgs::GetPhysicsOrchestratorActor :2087"]
        H26["msgs::GetNodeTypeArrays :2102"]
        H27["msgs::GetNodeIdMapping :2124"]
        H28["msgs::AddEdge :2146"]
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
    H8 --> SELF
    H9 -->|"self.client = msg.addr, rewire"| CL
    H20 --> SELF
    H24 --> SELF

    N1["H12/H13/H28 also call notify_graph_updated (debounced graphUpdated broadcast) before forwarding<br/>H24/H26/H27 default-value fallback when the target child is None (never propagate an error to caller)"]
    H12 --- N1
```

## VC-02.4 GraphServiceSupervisor restart and backoff lifecycle — the one live supervision path
```mermaid
stateDiagram-v2
    [*] --> Unknown
    Unknown --> Healthy: start_actor completes, health Healthy and last_heartbeat set (src/actors/graph_service_supervisor.rs:735-739)
    Healthy --> Degraded: perform_health_check finds a heartbeat older than 60s (:944-950)
    Degraded --> Healthy: the 15s heartbeat emitter refreshes last_heartbeat (:667-688)
    Healthy --> Restarting: RestartActor handler calls restart_actor (:1442-1448, :742)
    Degraded --> Restarting: RestartActor handler calls restart_actor (:1442-1448, :742)
    Restarting --> Healthy: run_later(backoff) fires start_actor then replay_buffered_messages (:764-767)
    Restarting --> Escalated: restart_count exceeds restart_policy.max_restarts (:750-757)
    Escalated --> RestartingAll: strategy OneForAll, restart_all_actors clears all four Addr slots (:791-793, :818-830)
    Escalated --> Stopped: strategy Escalate, top of the tree, ctx.stop() (:795-807)
    Escalated --> Failed: any other strategy, ActorInfo health set Failed and the child stays down (:809-814)
    RestartingAll --> Healthy: each start_actor re-marks its ActorInfo Healthy (:735-739)
    note right of Restarting
        calculate_backoff src/actors/graph_service_supervisor.rs:772-785
        Exponential initial 1s doubled per restart_count, capped at 60s
        RestartPolicy::default :479-491 — max_restarts 5, within_time_period 300s
    end note
    note right of Stopped
        RESOLVED ADR-2045 (2026-09-05) — this section used to draw the generic
        SupervisorActor state machine from src/actors/supervisor.rs. Commit 346fff7af
        deleted that file with src/actors/lifecycle.rs. GraphServiceSupervisor is the
        only supervision path left in src/, and Escalate is the top of the tree.
    end note
```

## VC-02.5 GraphServiceSupervisor restart sequence — RestartActor to replay or escalate
```mermaid
sequenceDiagram
    autonumber
    participant OP as RestartActor sender<br/>src/actors/graph_service_supervisor.rs:1399-1403
    participant S as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:742
    participant AI as ActorInfo row<br/>src/actors/graph_service_supervisor.rs:321-331
    participant CH as supervised child<br/>src/actors/graph_service_supervisor.rs:694

    OP->>S: RestartActor { actor_type } (:1442-1448)
    S->>AI: health = Restarting, restart_count += 1, last_restart = now (:745-748)
    alt restart_count over restart_policy.max_restarts (:750-754)
        S->>S: escalate_failure(actor_type, ctx) (:755, :787)
        Note over S: OneForAll restarts every child (:791-793) — Escalate stops the supervisor (:795-807) — any other strategy marks the child Failed (:809-814)
    else inside the restart budget
        S->>S: backoff = calculate_backoff(actor_type) (:760, :772-785)
        S->>S: ctx.run_later(backoff) schedules the retry (:764)
        S->>CH: start_actor(actor_type) constructs and starts a fresh child (:765, :694-740)
        S->>AI: replay_buffered_messages drains message_buffer into the new Addr (:766, :847-856)
        S->>S: supervision_stats.total_restarts += 1 (:769)
    end
    Note over S,CH: the four supervised children are GraphState, PhysicsOrchestrator,<br/>SemanticProcessor and ClientCoordinator (ActorType :333-339) — agent-beam and<br/>presence are unsupervised peers started by AppState, see VC-02.17 and VC-02.20
    Note over OP,S: RESOLVED ADR-2045 (2026-09-05) — the generic SupervisorActor sequence<br/>drawn here before lived in src/actors/supervisor.rs, deleted by 346fff7af.<br/>The surviving copy is crates/visionclaw-actors/src/supervisor.rs:124, reached<br/>only from tests/orchestration_improvements_test.rs:278-280.
```

## VC-02.6 Drain and shutdown — GraphServiceSupervisor stop and the test-only crate drain
```mermaid
sequenceDiagram
    autonumber
    participant S as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:787
    participant Sys as actix runtime
    participant CH as supervised children<br/>src/actors/graph_service_supervisor.rs:694
    participant T as test caller<br/>tests/orchestration_improvements_test.rs:278-280
    participant SA as SupervisorActor crate copy<br/>crates/visionclaw-actors/src/supervisor.rs:124

    S->>S: escalate_failure with strategy Escalate — top of the tree (:795-806)
    S->>Sys: ctx.stop() (:807)
    Sys->>S: Actor stopping then stopped, logs GraphServiceSupervisor stopped (:1340-1342)
    Note over S,CH: children hold no back-reference — each stops when its last Addr clone drops
    Note over S: this ctx.stop() is the only explicit stop in graph_service_supervisor.rs —<br/>there is no drain timer and no registration gate on the live path
    T->>SA: InitiateGracefulShutdown { timeout_secs } (crates/visionclaw-actors/src/supervisor.rs:117-122, sent at :653)
    SA->>SA: draining = true — RegisterActor now rejected (:233, :245-256)
    loop ctx.run_later(timeout_secs) (:235)
        SA->>SA: drain timeout elapsed, ctx.stop() (:236-237)
    end
    Note over T,SA: RESOLVED ADR-2045 (2026-09-05) — the src/actors/supervisor.rs drain and the<br/>ActorLifecycleManager::shutdown_with_timeout path formerly drawn here are both<br/>deleted. The crate copy survives with no non-test sender of InitiateGracefulShutdown.
```

## VC-02.7 RESOLVED ADR-2045 — the removed supervision machinery and its survivor
```mermaid
flowchart TB
    ADR["ADR-2045 — remove the dead supervision machinery<br/>decision accepted, implementation complete, landed 346fff7af"]
    LIFE["DELETED src/actors/lifecycle.rs<br/>ActorLifecycleManager, static ACTOR_SYSTEM,<br/>initialize_actor_system, shutdown_actor_system"]
    SUP["DELETED src/actors/supervisor.rs<br/>generic SupervisorActor, ActorFactory,<br/>SupervisedActorTrait, ActorFailed"]
    COUP["DELETED coupling inside graph_service_supervisor.rs<br/>parent_supervisor field and the SetParentSupervisor message"]
    MOD["src/actors/mod.rs:97-116<br/>a comment block records both removals<br/>where the re-exports used to be"]
    ESC["src/actors/graph_service_supervisor.rs:795-807<br/>Escalate is now the top of the tree — log and ctx.stop()"]
    CRATE["crates/visionclaw-actors/src/supervisor.rs:124<br/>the only surviving SupervisorActor definition"]
    TEST["tests/orchestration_improvements_test.rs:278-280<br/>its only consumer, repointed at the crate copy"]
    LIVE["GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:594<br/>the one live supervision path, see VC-02.4 and VC-02.5"]

    ADR --> LIFE
    ADR --> SUP
    ADR --> COUP
    LIFE --> MOD
    SUP --> MOD
    SUP --> CRATE
    CRATE --> TEST
    COUP --> ESC
    MOD --> LIVE
    ESC --> LIVE

    N1["RESOLVED ADR-2045 (2026-09-05) — commit 346fff7af deleted both files and trimmed<br/>the actor set to graph-service, agent-beam and presence. No code in src/ constructs<br/>a generic SupervisorActor, and nothing outside tests sends InitiateGracefulShutdown."]
    LIVE --- N1
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
    note for GetGraphData "src/actors/messages/ — RegisterClient/UnregisterClient/BroadcastMessage\nowned by client_coordinator_actor.rs consumer; Result types confirmed from\nHandler impls at graph_service_supervisor.rs:1465,1664,1983,1694,2146,2010"
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
    GraphServiceSupervisor ..> SetClientCoordinatorAddr
    GraphServiceSupervisor ..> NotifyGraphUpdated
    note for SetClientCoordinatorAddr "src/actors/graph_service_supervisor.rs:1524-1532 —\nfield and handler both confirmed by direct read.\nRESOLVED ADR-2045 (2026-09-05) — SetParentSupervisor and ActorFailed\nwere removed from this actor with src/actors/supervisor.rs, see VC-02.7"
```

## VC-02.11 GraphStateActor — representative read and write sequence
```mermaid
sequenceDiagram
    autonumber
    participant Caller as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:1465
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
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:712
    participant PO as PhysicsOrchestratorActor<br/>src/actors/physics_orchestrator_actor.rs
    participant GPU as GPU boundary<br/>see VC-10
    participant CC as ClientCoordinatorActor

    GSS->>PO: StartSimulation (src/actors/graph_service_supervisor.rs:1720)
    GSS->>PO: SimulationStep (:1737)
    GSS->>PO: UpdateSimulationParams (:1797)
    GSS->>PO: ForceResumePhysics (:1825)
    GSS->>PO: NodeInteractionMessage (:2010)
    GSS->>PO: StoreGPUComputeAddress (:1854, do_send at :1886, from the GPUManagerActor GetForceComputeActor reply)
    GSS->>PO: UpdateNodePositions (:1983, forwarded alongside GraphStateActor)
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

    GSS->>CC: SetClientCoordinatorAddr rebind (src/actors/graph_service_supervisor.rs:1524-1532)
    Note over CC: registers/unregisters client connections, dispatches BroadcastMessage / graphUpdated
    CC->>CF: apply per-client visibility filter before dispatch
    Note over CF: request-side visibility gate is see VC-03 — this file covers the actor side only
```

## VC-02.14 MetadataActor and OntologyActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:805
    participant MD as MetadataActor<br/>src/actors/metadata_actor.rs
    participant ONT as OntologyActor<br/>src/app_state.rs:1202-1214
    participant GPU as GPUManagerActor boundary
    participant CC as ClientCoordinatorActor

    APP->>MD: MetadataActor::new(MetadataStore::new()).start() (:805)
    APP->>ONT: OntologyActor::new()
    alt gpu_manager_addr Some
        APP->>ONT: set_gpu_manager_addr(gpu_mgr) (src/app_state.rs:1207)
        Note over ONT,GPU: wired to GPUManagerActor for constraint pipeline — GPU internals see VC-10
    end
    APP->>ONT: set_client_manager_addr(client_manager_addr) (src/app_state.rs:1211)
    Note over ONT,CC: wired for WebSocket broadcasts of ontology changes
    APP->>ONT: ontology_actor.start() (:1213) — Option~Addr~ retained
```

## VC-02.15 SemanticProcessorActor and OptimizedSettingsActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:715-721
    participant SP as SemanticProcessorActor<br/>src/actors/semantic_processor_actor.rs
    participant APP as AppState::new<br/>src/app_state.rs:1152
    participant SET as OptimizedSettingsActor<br/>src/actors/optimized_settings_actor.rs
    participant REDIS as REDIS_URL<br/>src/actors/optimized_settings_actor.rs:146

    GSS->>SP: SemanticProcessorActor::new(SemanticProcessorConfig::default()).start() (:716-720)
    APP->>SET: OptimizedSettingsActor::with_actors(sqlite_settings_repository, Some(graph_service_addr), None) (:1152-1156)
    SET->>SET: settings_actor.start() (:1161)
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
    participant APP as AppState::new<br/>src/app_state.rs:1196-1200
    participant PS as ProtectedSettingsActor<br/>src/actors/protected_settings_actor.rs
    participant WS as WorkspaceActor<br/>src/actors/workspace_actor.rs

    APP->>PS: ProtectedSettingsActor::new(ProtectedSettings::default()).start() (:1197)
    Note over PS: GetApiKeys handler referenced src/app_state.rs:1607-1609
    APP->>WS: WorkspaceActor::new().start() (:1200)
```

## VC-02.17 PresenceActor and TaskOrchestratorActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant XR as XR client
    participant PA as PresenceActor<br/>src/actors/presence_actor.rs:46
    participant APP as AppState::new<br/>src/app_state.rs:1263
    participant TO as TaskOrchestratorActor<br/>src/actors/task_orchestrator_actor.rs:68
    participant MGMT as ManagementApiClient

    XR->>PA: hand-presence update
    alt PRESENCE_HAND_REACH_M set (src/actors/presence_actor.rs:46, asserted in test :1063)
        PA->>PA: use configured reach metres
    else unset
        PA->>PA: use built-in default reach
    end
    APP->>TO: TaskOrchestratorActor::new(mgmt_client).start() (:1263)
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
    participant APP as AppState::new<br/>src/app_state.rs:1271-1288
    participant EL as ElevationActor<br/>src/actors/elevation_actor.rs:173
    participant EV as elevation_voice<br/>src/actors/elevation_voice.rs
    participant MAIN as main<br/>src/main.rs:544
    participant DE as DecisionElevationActor<br/>src/actors/decision_elevation_actor.rs:175

    alt ELEVATION_ACTOR_ENABLED gate passes (src/actors/elevation_actor.rs:173)
        APP->>EL: ElevationActor::new(graph_adapter, sqlite_enrichment_repository, speech_service, Some(ontology_repository)).start() (:1271)
        Note over EL,EV: voice-guided path when local speech stack (Whisper/Kokoro) is up — elevation_voice.rs
    else gate closed
        APP->>APP: log "ElevationActor disabled" (src/app_state.rs:1287)
    end
    Note over MAIN,DE: DecisionElevationActor is started in main() src/main.rs:544, NOT in AppState::new —<br/>a second, separately-gated ACSP actor family alongside ElevationActor
    alt DECISION_ELEVATION_ENABLED gate passes (src/actors/decision_elevation_actor.rs:175)
        MAIN->>DE: DecisionElevationActor::new() then actix::Actor::start(actor) (src/main.rs:544-546)
        MAIN->>MAIN: wrap in ActorElevationSink, feed DecisionService.with_elevation_sink (src/main.rs:548-550)
    else gate closed
        MAIN->>MAIN: log "DecisionElevationActor disabled" (src/main.rs:553)
    end
```

## VC-02.19 VoiceInterfaceActor (+ voice_commands.rs) and MultiMcpVisualizationActor
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:1294-1305
    participant VI as VoiceInterfaceActor<br/>src/actors/voice_interface_actor.rs
    participant VC as voice_commands<br/>src/actors/voice_commands.rs
    participant MMV as MultiMcpVisualizationActor<br/>src/actors/multi_mcp_visualization_actor.rs

    alt speech_service Some
        APP->>VI: VoiceInterfaceActor::new(task_orchestrator_addr.clone(), speech_service.clone()).start() (:1294)
        VI->>VC: dispatch parsed spoken command to settings-assistant path
    else speech_service None
        APP->>APP: log "VoiceInterfaceActor disabled (no speech service)" (:1304)
    end
    Note over MMV: MultiMcpVisualizationActor has no start() call found in src/app_state.rs or src/main.rs —<br/>coverage gap, reported not diagrammed further (see report)
```

## VC-02.20 AgentMonitorActor and AgentBeamActor — message surfaces
```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:1169-1184
    participant AM as AgentMonitorActor<br/>src/actors/agent_monitor_actor.rs:574
    participant CFC as ClaudeFlowClient
    participant AB as AgentBeamActor<br/>src/app_state.rs:722
    participant CC as ClientCoordinatorActor
    participant HUB as agent-events hub

    APP->>CFC: ClaudeFlowClient::new(mcp_host, mcp_port) (:1182)
    APP->>AM: AgentMonitorActor::new(claude_flow_client, graph_service_addr.clone()).start() (:1184)
    alt MOCK_AGENTS set (src/actors/agent_monitor_actor.rs:574)
        AM->>AM: synthesize mock agent roster, skip live MCP poll
    else unset
        AM->>CFC: poll live MCP agent roster
    end
    APP->>AB: AgentBeamActor::new(client_manager_addr.clone()).start() (:722)
    HUB->>AB: process-global agent-event stream (subscription keeps actor alive, no Addr retained)
    AB->>CC: encoded 0x23 frames via existing binary dispatch
```
