---
id: VC-10
title: GPU supervision and context bus
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/GPU-wire-abi.md
adrs: [ADR-2007, ADR-2053]
sources:
  - src/actors/gpu/gpu_manager_actor.rs
  - src/actors/gpu/resource_supervisor.rs
  - src/actors/gpu/physics_supervisor.rs
  - src/actors/gpu/analytics_supervisor.rs
  - src/actors/gpu/graph_analytics_supervisor.rs
  - src/actors/gpu/context_bus.rs
  - src/actors/gpu/gpu_resource_actor.rs
  - src/actors/gpu/force_compute_actor.rs
  - src/actors/gpu/supervisor_messages.rs
  - src/actors/gpu/constraint_actor.rs
  - src/actors/gpu/ontology_constraint_actor.rs
  - src/actors/gpu/semantic_forces_actor.rs
  - src/actors/gpu/stress_majorization_actor.rs
  - src/actors/gpu/clustering_actor.rs
  - src/actors/gpu/anomaly_detection_actor.rs
  - src/actors/gpu/pagerank_actor.rs
  - src/actors/gpu/shortest_path_actor.rs
  - src/actors/gpu/connected_components_actor.rs
  - src/actors/physics_orchestrator_actor.rs
  - src/app_state.rs
verified_commit: bed6b617d
---

## VC-10.1 GPU supervision tree

```mermaid
flowchart TD
    APP["AppState::new<br/>src/app_state.rs:965 starts GPUManagerActor when GPU enabled"]
    GM["GPUManagerActor<br/>gpu_manager_actor.rs:57 struct, :140 impl Actor"]
    RS["ResourceSupervisor<br/>resource_supervisor.rs:40 struct, :263 impl Actor"]
    PS["PhysicsSupervisor<br/>physics_supervisor.rs"]
    AS["AnalyticsSupervisor<br/>analytics_supervisor.rs"]
    GAS["GraphAnalyticsSupervisor<br/>graph_analytics_supervisor.rs"]
    GRA["GPUResourceActor<br/>gpu_resource_actor.rs:451 InitializeGPU"]
    FCA["ForceComputeActor<br/>force_compute_actor.rs:1867 ComputeForces"]
    SMA["StressMajorizationActor<br/>stress_majorization_actor.rs:315"]
    CA["ConstraintActor<br/>constraint_actor.rs:193"]
    OCA["OntologyConstraintActor<br/>ontology_constraint_actor.rs:451"]
    SFA["SemanticForcesActor<br/>semantic_forces_actor.rs:751"]
    CLA["ClusteringActor<br/>clustering_actor.rs:1172 RunKMeans"]
    ADA["AnomalyDetectionActor<br/>anomaly_detection_actor.rs:112"]
    PRA["PageRankActor<br/>pagerank_actor.rs:433"]
    SPA["ShortestPathActor<br/>shortest_path_actor.rs:214 ComputeSSP"]
    CCA["ConnectedComponentsActor<br/>connected_components_actor.rs:233"]

    APP -->|"InitializeGPU"| GM
    GM -->|"ResourceSupervisor::new().start() :101"| RS
    GM -->|"PhysicsSupervisor::new().start() :91"| PS
    GM -->|"AnalyticsSupervisor::new().start() :94"| AS
    GM -->|"GraphAnalyticsSupervisor::new().start() :97"| GAS
    GM -->|"SetSubsystemSupervisors try_send :105"| RS
    RS -->|"spawn_resource_actor :266 in Actor::started"| GRA
    RS -->|"SetSharedGPUContext try_send :138 then UpdateGPUGraphData :151"| PS
    RS -->|"SetSharedGPUContext try_send :163"| AS
    RS -->|"SetSharedGPUContext try_send :173"| GAS
    PS --> FCA
    PS --> SMA
    PS --> CA
    PS --> OCA
    PS --> SFA
    AS --> CLA
    AS --> ADA
    AS --> PRA
    GAS --> SPA
    GAS --> CCA

    N1["LAZY: Actor::started :143 spawns nothing - spawn_supervisors :82 runs on the first message via get_supervisors :126, guarded by supervisors_spawned :68"]
    N3["ORDER: ResourceSupervisor is spawned LAST :101 so the other three addresses already exist for SetSubsystemSupervisors :105"]
    GM -.- N3
    N2["ADR-2007 GPUManagerActor is a coordinator not a God Actor - it owns no CUDA handle, only supervisor addresses :113-118"]
    GM -.- N1
    GM -.- N2
```

## VC-10.2 Boot — GPU initialisation with total timeout

```mermaid
sequenceDiagram
    autonumber
    participant APP as AppState::new<br/>src/app_state.rs:965
    participant GM as GPUManagerActor<br/>gpu_manager_actor.rs:253
    participant RS as ResourceSupervisor<br/>resource_supervisor.rs:298
    participant GRA as GPUResourceActor<br/>gpu_resource_actor.rs:451
    participant PS as PhysicsSupervisor<br/>physics_supervisor.rs:768

    Note over GM: Actor::started :143 spawns NO supervisors - they are created on first message via get_supervisors :126
    APP->>GM: InitializeGPU with graph + graph_service_addr
    GM->>GM: get_supervisors :130 then spawn_supervisors :82
    GM->>RS: InitializeGPU forwarded
    RS->>RS: pending_graph_data = Some(graph) :311
    alt resource_actor is None
        RS->>RS: spawn_resource_actor :318
    end
    RS->>RS: init_state = InProgress :331
    Note over RS: timeout = self.timeouts.total = 60s<br/>supervisor_messages.rs:222 (device_init 10s :218, ptx_load 5s :219, graph_upload 30s :220, context_distribution 5s :221)
    RS->>GRA: tokio::time::timeout(60s, resource_addr.send(InitializeGPU)) :340
    alt Ok(Ok(_)) device + PTX ready
        GRA-->>RS: Ok
        RS->>RS: init_state = Completed :356, failure_count = 0 :357, current_delay = 1s :358
        opt shared_context.is_some() :361
            RS->>RS: distribute_context_to_supervisors :362
        end
    else Err(_) elapsed past 60s
        RS->>RS: init_state = TimedOut :367
        Note over RS: warn "GPU initialization timed out, system will continue in degraded mode" :368
        RS->>RS: handle_init_failure :370
    else Ok(Err(e)) mailbox error
        RS->>RS: handle_init_failure with "Mailbox error" :347
    end
    GRA->>RS: SetSharedGPUContext :380 (context flows back up, not down)
    RS->>RS: init_state = Completed :388 then distribute_context_to_supervisors :391
    RS->>PS: SetSharedGPUContext try_send :138
```

## VC-10.3 SharedGPUContext distribution — direct messages, bus is additive

```mermaid
sequenceDiagram
    autonumber
    participant GRA as GPUResourceActor<br/>gpu_resource_actor.rs:451
    participant RS as ResourceSupervisor<br/>resource_supervisor.rs:123 distribute_context_to_supervisors
    participant PS as PhysicsSupervisor<br/>physics_supervisor.rs:768
    participant AS as AnalyticsSupervisor<br/>analytics_supervisor.rs:409
    participant GAS as GraphAnalyticsSupervisor<br/>graph_analytics_supervisor.rs:336
    participant BUS as GPUContextBus<br/>context_bus.rs:66 tokio broadcast::Sender

    GRA->>RS: SetSharedGPUContext :380
    RS->>RS: shared_context = Some(msg.context) :386
    alt shared_context is None
        Note over RS: warn "No context to distribute" and return :127-128
    else context present :124
        rect rgb(220,235,250)
            Note over RS,GAS: PRIMARY PATH - central point-to-point try_send from ResourceSupervisor
            opt physics_supervisor is Some :137
                RS->>PS: SetSharedGPUContext try_send :138
                opt pending_graph_data is Some :146
                    RS->>PS: UpdateGPUGraphData try_send :151 (relays graph to ForceComputeActor)
                end
            end
            opt analytics_supervisor is Some :162
                RS->>AS: SetSharedGPUContext try_send :163
            end
            opt graph_analytics_supervisor is Some :172
                RS->>GAS: SetSharedGPUContext try_send :173
            end
        end
        rect rgb(250,235,220)
            Note over RS,BUS: SECONDARY PATH - comment reads "Also publish to event bus for any additional subscribers" :181
            RS->>BUS: context_bus.publish(context) :182
            BUS-->>RS: receiver_count :92 (Err(_) maps to 0 when nobody subscribed :105)
        end
        RS->>RS: pending_graph_data = None :189
    end
    Note over RS,BUS: RESOLVED ADR-2053: direct point-to-point delivery is now the DECLARED authoritative mechanism - the bus is a supplementary broadcast for non-supervisor observers and a zero receiver count is normal
    Note over RS,BUS: RESOLVED ADR-2053 + ADR-2060: BASELINE said "not a central handle" - corrected to describe the code, which delivers direct-first and publishes to the bus as a supplement
    Note over RS: RESOLVED ADR-2053: try_send results are now inspected - a failure logs error!, is recorded in context_delivery_failures, and forces get_health to report Degraded instead of Healthy
```

## VC-10.4 GPU readiness lifecycle

```mermaid
stateDiagram-v2
    [*] --> NotStarted
    NotStarted: InitializationState NotStarted - resource_supervisor.rs line 30
    InProgress: InitializationState InProgress - set at line 331
    Completed: InitializationState Completed - set at lines 356 and 388
    Failed: InitializationState Failed(String) - set at line 196
    TimedOut: InitializationState TimedOut - set at line 367

    NotStarted --> InProgress: InitializeGPU handler :298
    InProgress --> Completed: Ok(Ok(_)) within timeouts.total 60s
    InProgress --> TimedOut: tokio timeout elapsed :348
    InProgress --> Failed: Ok(Err(mailbox)) :347
    TimedOut --> Failed: handle_init_failure :370
    Failed --> NotStarted: ctx.run_later(delay) re-spawns GPUResourceActor :223-227
    Failed --> [*]: failure_count > policy.max_restarts - give up :202-208
    Completed --> Completed: SetSharedGPUContext re-distributes :391

    note right of Failed
        Backoff current_delay starts 1s :95
        multiplied by policy.backoff_multiplier :213
        clamped to policy.max_delay :215
        reset to 1s on success :358
    end note
    note right of Completed
        get_health :231 maps state to SubsystemStatus
        Completed AND has_context to Healthy :236
        InProgress or NotStarted to Initializing :237-238
        Failed or TimedOut to Degraded :239-241
    end note
```

## VC-10.5 Initialisation failure, backoff and manual restart

```mermaid
sequenceDiagram
    autonumber
    participant CH as Child actor / init future
    participant RS as ResourceSupervisor<br/>resource_supervisor.rs:193 handle_init_failure
    participant CTX as Actix Context<br/>ctx.run_later
    participant GRA as GPUResourceActor<br/>spawn_resource_actor :118

    CH->>RS: ActorFailure{actor_name, error} :436
    alt actor_name == "GPUResourceActor" :440
        RS->>RS: handle_init_failure :441
    else other name
        Note over RS: ignored - ResourceSupervisor supervises exactly one child
    end
    RS->>RS: init_state = Failed(error) :196, failure_count += 1 :197, last_error :198, last_attempt :199
    alt failure_count > policy.max_restarts :202
        Note over RS: error "Exceeded max initialization attempts, giving up" :203-206 then return - NO further retry is scheduled
    else within budget
        RS->>RS: delay = current_delay :211 then current_delay = min(current_delay * backoff_multiplier, max_delay) :212-215
        RS->>CTX: ctx.run_later(delay, ...) :223
        CTX->>RS: closure fires
        RS->>GRA: spawn_resource_actor :225
        RS->>RS: init_state = NotStarted :226
    end
    Note over RS,GRA: MANUAL override - RestartActor :446 re-spawns and resets state to NotStarted :452-453 WITHOUT consuming the backoff budget, and returns Err for any other actor_name :456
    Note over RS: GetSubsystemHealth :289 reports restart_count = failure_count :258 so backoff pressure is observable
```

## VC-10.6 ForceComputeActor self-initialisation and supersession

```mermaid
sequenceDiagram
    autonumber
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:755 self-init guard
    participant RS as ResourceSupervisor<br/>resource_supervisor.rs:138
    participant PS as PhysicsSupervisor<br/>physics_supervisor.rs:768

    Note over FCA: Fields gpu_self_init_attempts :315, gpu_self_init_max_retries = 3 :425, gpu_self_init_last_attempt :319
    alt shared_context already present :755
        Note over FCA: trace "GPU context already present, skipping self-init" - supervisor-supplied context wins
    else attempts >= max_retries :760
        Note over FCA: warn "GPU self-init exhausted all 3 retries, skipping" :762 - actor stays without a context permanently
    else backoff not elapsed :770-777
        Note over FCA: backoff_secs = 1u64 << (attempts - 1) :772 giving 1s then 2s then 4s
    else proceed
        FCA->>FCA: gpu_self_init_attempts += 1 :783, gpu_self_init_last_attempt = now :784
        FCA->>FCA: create its own CUDA context
    end
    RS->>PS: SetSharedGPUContext :138
    PS->>FCA: SetSharedGPUContext :3825
    alt had_context true :3829
        Note over FCA: info "Received SharedGPUContext from supervisor chain (replacing self-initialized context)" :3831
    else first context
        Note over FCA: info "Received SharedGPUContext from supervisor chain" :3833
    end
    FCA->>FCA: shared_context = Some(msg.context) :3840 then gpu_state.is_initialized = true :3853
    opt pending_graph_data is Some :3856
        FCA->>FCA: try_upload_pending_graph_data :3858
    end
    Note over FCA,PS: INVARIANT: the externally supplied context always replaces a self-created one so every GPU actor shares one CUDA device and stream :3836-3839
    Note over FCA: DIVERGENCE: self-init is a second, unsupervised path to a CUDA context that bypasses ResourceSupervisor timeouts and backoff entirely
    Note over FCA: InitializeGPU :3394 deliberately does NOT set gpu_state.num_nodes - that happens only after a successful upload, preventing ComputeForces on uninitialised buffers and CUDA mutex poisoning :3406-3408
```

## VC-10.7 Message surface — coordinator and supervisors

```mermaid
classDiagram
    class GPUManagerActor {
        +GetGPUSystemHealth() L181
        +InitializeGPU() L253
        +UpdateGPUGraphData() L294
        +ComputeForces() L329
        +RunKMeans() L346
        +RunCommunityDetection() L369
        +RunDBSCAN() L392
        +RunAnomalyDetection() L415
        +PerformGPUClustering() L438
        +TriggerStressMajorization() L469
        +UpdateConstraints() L483
        +GetGPUStatus() L497
        +GetForceComputeActor() L511
        +UploadConstraintsToGPU() L534
        +GetNodeData() L548
        +UpdateSimulationParams() L571
        +UpdateAdvancedParams() L585
        +SetSharedGPUContext() L599
        +ApplyOntologyConstraints() L628
        +ApplyMaterializedAxioms() L645
    }
    class ResourceSupervisor {
        +GetSubsystemHealth() L289
        +InitializeGPU() L298
        +SetSharedGPUContext() L380
        +SetSubsystemSupervisors() L413
        +ActorFailure() L436
        +RestartActor() L446
        +GetContextBus() L462
        +UpdateGPUGraphData() L476
    }
    class PhysicsSupervisor {
        +GetSubsystemHealth() L557
        +InitializeSubsystem() L586
        +ActorFailure() L603
        +RestartActor() L611
        +ComputeForces() L628
        +TriggerStressMajorization() L658
        +UpdateConstraints() L683
        +ApplyOntologyConstraints() L707
        +ApplyMaterializedAxioms() L732
        +GetForceComputeActor() L758
        +SetSharedGPUContext() L768
        +UpdateSimulationParams() L784
        +GetPhysicsStats() L808
        +UpdateGPUGraphData() L832
        +UpdateAdvancedParams() L846
        +UploadConstraintsToGPU() L870
        +GetNodeData() L894
        +GetOntologyConstraintStats() L921
        +GetSemanticConfig() L954
        +GetHierarchyLevels() L981
        +RecalculateHierarchy() L1008
        +ConfigureDAG() L1032
        +ConfigureTypeClustering() L1056
        +ConfigureCollision() L1080
        +AdjustConstraintWeights() L1104
    }
    class AnalyticsSupervisor {
        +GetSubsystemHealth() L366
        +InitializeSubsystem() L393
        +SetSharedGPUContext() L409
        +SetNodeAnalytics() L424
        +WriteClusterAnalytics() L434
        +ActorFailure() L459
        +RestartActor() L467
        +RunKMeans() L484
        +RunCommunityDetection() L514
        +RunDBSCAN() L538
        +RunAnomalyDetection() L568
        +ComputePageRank() L593
        +UpdateGPUGraphData() L617
        +PerformGPUClustering() L642
    }
    class GraphAnalyticsSupervisor {
        +GetSubsystemHealth() L294
        +InitializeSubsystem() L320
        +SetSharedGPUContext() L336
        +ActorFailure() L351
        +RestartActor() L359
        +ComputeShortestPaths() L376
        +ComputeConnectedComponents() L435
    }
    GPUManagerActor --> ResourceSupervisor : spawns L101
    GPUManagerActor --> PhysicsSupervisor : spawns L91
    GPUManagerActor --> AnalyticsSupervisor : spawns L94
    GPUManagerActor --> GraphAnalyticsSupervisor : spawns L97
    ResourceSupervisor --> PhysicsSupervisor : SetSharedGPUContext L138
    ResourceSupervisor --> AnalyticsSupervisor : SetSharedGPUContext L163
    ResourceSupervisor --> GraphAnalyticsSupervisor : SetSharedGPUContext L173
```

## VC-10.8 Message surface — physics leaf actors

```mermaid
classDiagram
    class ForceComputeActor {
        +ComputeForces() L1867
        +SetLayoutMode() L2641
        +SetRadialLayout() L2730
        +UpdateSimulationParams() L2858
        +UpdateClusteringParams() L3081
        +ForceFullBroadcast() L3108
        +SetComputeMode() L3238
        +GetPhysicsStats() L3255
        +UpdateAdvancedParams() L3263
        +UploadPositions() L3303
        +PinNodePositions() L3367
        +InitializeGPU() L3394
        +UpdateGPUGraphData() L3478
        +GetNodeData() L3507
        +GetGPUStatus() L3515
        +GetCurrentPositions() L3528
        +GetSettlementState() L3607
        +SetPhysicsSettled() L3621
        +GetGPUMetrics() L3633
        +RunCommunityDetection() L3651
        +UpdateVisualAnalyticsParams() L3659
        +GetConstraints() L3672
        +UpdateConstraints() L3680
        +UploadConstraintsToGPU() L3700
        +TriggerStressMajorization() L3724
        +GetStressMajorizationStats() L3736
        +ResetStressMajorizationSafety() L3752
        +UpdateStressMajorizationParams() L3767
        +PerformGPUClustering() L3780
        +GetClusteringResults() L3790
        +UpdateOntologyConstraintBuffer() L3805
        +SetSharedGPUContext() L3825
        +SetPhysicsOrchestratorAddr() L3879
        +ResetPositions() L3894
        +ConfigureStressMajorization() L3985
        +GetStressMajorizationConfig() L4026
        +ConfigureBroadcastOptimization() L4059
        +UpdateCameraFrustum() L4107
        +GetBroadcastStats() L4128
        +RunAnomalyDetection() L4159
        +PositionBroadcastAck() L4371
    }
    class StressMajorizationActor {
        +TriggerStressMajorization() L315
        +ResetStressMajorizationSafety() L334
        +UpdateStressMajorizationParams() L347
        +CheckStressMajorization() L369
        +SetSharedGPUContext() L397
        +ConfigureStressMajorization() L410
        +GetStressMajorizationConfig() L475
    }
    class ConstraintActor {
        +UpdateConstraints() L193
        +GetConstraints() L215
        +UploadConstraintsToGPU() L224
        +ClearConstraints() L256
        +GetConstraintStatistics() L264
        +SetSharedGPUContext() L281
    }
    class OntologyConstraintActor {
        +ApplyOntologyConstraints() L451
        +ApplyMaterializedAxioms() L522
        +UpdateOntologyConstraints() L545
        +GetOntologyStats() L557
        +GetOntologyConstraintStats() L565
        +SetForceComputeAddr() L589
        +SetSharedGPUContext() L603
        +GetConstraintStats() L625
        +GetConstraintBuffer() L649
        +UpdateConstraints() L668
        +InitializeGPU() L705
        +AdjustConstraintWeights() L723
    }
    class SemanticForcesActor {
        +ConfigureDAG() L751
        +ConfigureTypeClustering() L778
        +ConfigureCollision() L802
        +GetSemanticConfig() L826
        +GetHierarchyLevels() L834
        +RecalculateHierarchy() L844
        +SetSharedGPUContext() L859
        +ReloadRelationshipBuffer() L869
    }
    PhysicsSupervisor --> ForceComputeActor
    PhysicsSupervisor --> StressMajorizationActor
    PhysicsSupervisor --> ConstraintActor
    PhysicsSupervisor --> OntologyConstraintActor
    PhysicsSupervisor --> SemanticForcesActor
```

## VC-10.9 Message surface — analytics and graph-analytics leaf actors

```mermaid
classDiagram
    class ClusteringActor {
        +SetSharedGPUContext() L1159
        +RunKMeans() L1172
        +RunCommunityDetection() L1192
        +RunDBSCAN() L1209
        +SetNodeAnalytics() L1229
        +WriteClusterAnalytics() L1238
        +UpdateGPUGraphData() L1286
        +PerformGPUClustering() L1304
    }
    class AnomalyDetectionActor {
        +RunAnomalyDetection() L112
        +SetSharedGPUContext() L423
        +SetNodeAnalytics() L435
    }
    class PageRankActor {
        +ComputePageRank() L433
        +GetPageRankResult() L561
        +ClearPageRankCache() L570
        +SetSharedGPUContext() L580
        +SetNodeAnalytics() L592
        +InitializeActor() L602
    }
    class ShortestPathActor {
        +InitializeActor() L185
        +SetSharedGPUContext() L194
        +SetNodeSSSP() L205
        +ComputeSSP() L214
        +ComputeAPSP() L349
        +GetShortestPathStats() L373
    }
    class ConnectedComponentsActor {
        +InitializeActor() L213
        +SetSharedGPUContext() L222
        +ComputeConnectedComponents() L233
        +GetConnectedComponentsStats() L327
        +UpdateComponentEdges() L339
    }
    class GPUResourceActor {
        +InitializeGPU() L451
        +UpdateGPUGraphData() L567
        +GetNodeData() L580
    }
    AnalyticsSupervisor --> ClusteringActor
    AnalyticsSupervisor --> AnomalyDetectionActor
    AnalyticsSupervisor --> PageRankActor
    GraphAnalyticsSupervisor --> ShortestPathActor
    GraphAnalyticsSupervisor --> ConnectedComponentsActor
    ResourceSupervisor --> GPUResourceActor
```

## VC-10.10 GPU-absent and CPU-fallback behaviour per actor

```mermaid
sequenceDiagram
    autonumber
    participant PO as PhysicsOrchestratorActor<br/>src/actors/physics_orchestrator_actor.rs:379
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3122
    participant SMA as StressMajorizationActor<br/>stress_majorization_actor.rs:95
    participant CCA as ConnectedComponentsActor<br/>connected_components_actor.rs:82
    participant OCA as OntologyConstraintActor<br/>ontology_constraint_actor.rs:291
    participant GRA as GPUResourceActor<br/>gpu_resource_actor.rs:142

    Note over PO,GRA: There is NO single system-wide CPU fallback - each actor degrades differently, so "GPU absent" is not one branch
    PO->>PO: physics step with no GPU
    alt cpu_fallback_warned is false :602
        Note over PO: warn once then cpu_fallback_warned = true :604 (field declared :123-124, initialised false :241)
    end
    Note over PO: on the CPU path no PhysicsStepCompleted message comes back :379 so the orchestrator must not await one
    FCA->>FCA: ForceFullBroadcast with no context
    Note over FCA: warn "ForceFullBroadcast - no GPU context, skipping" :3122 - the frame is DROPPED, not computed on CPU
    FCA->>FCA: recover_from_divergence with no context
    Note over FCA: warn "recover_from_divergence called with no GPU context" :1695
    SMA->>SMA: stress majorization requested
    Note over SMA: returns Err "GPU not available for stress majorization" :95 - hard failure, no CPU path
    CCA->>CCA: GPU kernel failed
    Note over CCA: falls back to a CPU implementation, recorded as ExecutionPath::CpuFallback :279 (doc comment :53)
    OCA->>OCA: constraints arrive before GPU
    Note over OCA: info "GPU not available, constraints cached for next physics step" :291 and cpu_fallback_count += 1 :286,:293 surfaced via GetConstraintStats :581
    GRA->>GRA: APSP PTX load fails
    Note over GRA: warn "Failed to load APSP PTX (will use CPU fallback)" :142
    Note over CCA: SemanticForcesActor carries CPU fallback implementations :221 in semantic_forces_actor.rs
    Note over CCA: DIVERGENCE: ShortestPathActor's CPU fallback was REMOVED - shortest_path_actor.rs:356 refers to "the former CPU fallback", so SSSP has no degraded path
    Note over PO,GRA: DIVERGENCE: coverage is uneven - ConnectedComponents and SemanticForces degrade, StressMajorization and ForceFullBroadcast hard-fail or drop, SSSP lost its fallback
```
