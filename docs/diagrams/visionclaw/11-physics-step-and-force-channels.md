---
id: VC-11
title: Physics step and force channels
area: visionclaw
governing:
  - docs/GPU-wire-abi.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2007, ADR-2028, ADR-2029, ADR-2055, ADR-2060]
sources:
  - src/actors/gpu/force_compute_actor.rs
  - src/actors/gpu/physics_supervisor.rs
  - src/actors/gpu/constraint_actor.rs
  - src/actors/gpu/ontology_constraint_actor.rs
  - src/actors/gpu/semantic_forces_actor.rs
  - src/actors/gpu/stress_majorization_actor.rs
  - src/actors/physics_orchestrator_actor.rs
  - src/models/force_channels.rs
  - src/models/simulation_params.rs
  - src/utils/unified_gpu_compute/execution.rs
  - src/handlers/layout_handler.rs
  - crates/visionclaw-domain/src/models/simulation_params.rs
  - Cargo.toml
  - src/handlers/constraints_handler.rs
  - src/utils/visionflow_unified.ptx
verified_commit: 7a20db228
---

## VC-11.1 Physics tick — phase 1, params and flag word

```mermaid
sequenceDiagram
    autonumber
    participant PO as PhysicsOrchestratorActor<br/>src/actors/physics_orchestrator_actor.rs
    participant PS as PhysicsSupervisor<br/>physics_supervisor.rs:628
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:1867
    participant EX as UnifiedGPUCompute::execute<br/>src/utils/unified_gpu_compute/execution.rs:131
    participant FC as derive_dispatch_feature_flags<br/>src/models/force_channels.rs:486

    PO->>PS: ComputeForces
    PS->>FCA: ComputeForces forwarded :628
    alt shared_context is None
        Note over FCA: no GPU context - frame skipped, logged every 300th skip :1892,:1934
    else GPU ready
        FCA->>FCA: stability_warmup_remaining -= 1 :2007
        Note over FCA: warmup reset on graph upload - 1200 frames when edge_count == 0 else 600 :1424
        FCA->>FCA: reheat_factor read :2009
        FCA->>EX: execute(sim_params) with num_constraints
        EX->>FC: derive_dispatch_feature_flags(ForceDispatchInputs) :954
        FC-->>EX: flags word
        EX->>EX: sim_params.feature_flags = feature_flags :968
        Note over EX: OVERWRITE - the converter's own flag word is discarded before every execute, so only this word reaches the device
    end
    Note over FC: repel_k > 0.0 sets ENABLE_REPULSION :488-490
    Note over FC: spring_k > 0.0 sets ENABLE_SPRINGS :491-493
    Note over FC: center_gravity_k > 0.0 sets ENABLE_CENTERING :494-496
    Note over FC: use_sssp_distances OR sssp_spring_adjust_enabled sets ENABLE_SSSP_SPRING_ADJUST :497-499
    Note over FC: num_constraints > 0 sets ENABLE_CONSTRAINTS :502-504 - KEYSTONE, never from user settings
    Note over EX,FC: INVARIANT ADR-2029 - ENABLE_CONSTRAINTS is residency-derived at step time, never a settings toggle
```

## VC-11.2 Physics tick — phase 2, kernel launch and integrate

```mermaid
sequenceDiagram
    autonumber
    participant EX as UnifiedGPUCompute::execute<br/>execution.rs
    participant K1 as force_pass_kernel<br/>src/utils/visionflow_unified.ptx
    participant K2 as integrate_pass_kernel<br/>visionflow_unified.ptx
    participant EV as cust Event<br/>execution.rs completion poll

    opt num_constraints > 0 :221
        EX->>EX: bind constraint_force_ptr and constraint_force_epsilon :221
    end
    EX->>K1: launch with pos/vel/force buffers, mass, num_nodes
    EX->>K1: class_id, class_charge, class_mass device pointers (ontology metadata)
    EX->>K1: prev_force_x/y/z for FA2 swing and traction adaptive speed
    EX->>K1: pinned_mask device pointer
    Note over K1: pinned nodes SKIP integration but still exert forces on neighbours
    K1->>K2: forces written
    Note over K2: integrate_pass adds boundary push (viewport_bounds + boundary_damping) and annealing jitter (temperature + cooling_rate) per force_channels.rs:38-40
    EX->>EV: completion_event.record(stream)
    loop poll until EventStatus::Ready
        EX->>EV: completion_event.query()
        alt poll_start.elapsed() > 10s
            Note over EX: returns Err "GPU kernel execution timed out after 10s"
        else not ready
            EX->>EX: std::thread::yield_now()
        end
    end
    EX->>EX: swap_buffers() then iteration += 1
    opt iteration % 100 == 0
        Note over EX: logs memory MB, utilisation pct, grid occupancy pct, resize count
    end
    Note over K1,K2: Gravity and cluster cohesion run as SEPARATE kernels - degree_weighted_gravity_kernel and cluster_cohesion_kernel (force_channels.rs:41-43)
```

## VC-11.3 Feature-flag derivation across the conversion paths

```mermaid
flowchart TD
    A["Path A converter<br/>src/models/simulation_params.rs:558-569"]
    B["Path B converter<br/>src/models/simulation_params.rs:641-652"]
    C["Registry mutator ForceChannel::apply<br/>src/models/force_channels.rs:183-198"]
    D["derive_dispatch_feature_flags<br/>src/models/force_channels.rs:486"]
    E["execution.rs:954 calls the helper"]
    F["execution.rs:968 sim_params.feature_flags = flags"]
    G["Device SimParams word actually read by the kernels"]

    A -->|"builds its own flag word"| X["OVERWRITTEN before execute"]
    B -->|"builds its own flag word"| X
    C -->|"sets bit per channel :198, skips read-only :191"| X
    X --> F
    D --> E --> F --> G

    N1["RESOLVED ADR-2060: the GPU-wire-abi divergence bullet claiming no single shared helper was STALE and is now marked resolved. derive_dispatch_feature_flags force_channels.rs:486 is the single helper, called from execution.rs:954."]
    N2["RESOLVED ADR-2060: Invariant 2 cited execution.rs:903-904 for the ENABLE_CONSTRAINTS rule - repointed to derive_dispatch_feature_flags in force_channels.rs, which is where the rule lives."]
    N3["ADR-2029 test module adr_2029_dispatch_authority force_channels.rs:509 states it observes 'the word that is actually uploaded - not the converter's word, which is overwritten before every execute'"]
    N4["Converter words are therefore DEAD for dispatch - a divergence between A and B cannot reach the GPU"]
    D -.- N1
    D -.- N2
    D -.- N3
    X -.- N4
```

## VC-11.4 Force-channel registry

```mermaid
classDiagram
    class ForceChannel {
        <<enum>>
        +Repulsion
        +Separation
        +Spring
        +Centering
        +Constraints
        +DagRadialBias
        +Boundary
        +Annealing
        +Gravity
        +ClusterCohesion
        +name() L118
        +flag_bit() L132
        +scalar() L165
        +constraints_scalar_is_constraint_max_force_per_node() L165_L222
        +bits_declared_in_visionclaw_domain_simulation_params() L113_L119
        +bit_names_in_src_models_simulation_params() L387_L393
        +state(SimParams) ForceChannelState
        +apply(SimParams, ForceChannelState) L183
        +is_read_only() L210
    }
    class FeatureFlags {
        +ENABLE_REPULSION bit0 L113
        +ENABLE_SPRINGS bit1 L114
        +ENABLE_CENTERING bit2 L115
        +ENABLE_TEMPORAL_COHERENCE bit3 L116
        +ENABLE_CONSTRAINTS bit4 L117
        +ENABLE_STRESS_MAJORIZATION bit5 L118
        +ENABLE_SSSP_SPRING_ADJUST bit6 L119
    }
    class ForceDispatchInputs {
        +f32 repel_k
        +f32 spring_k
        +f32 center_gravity_k
        +bool use_sssp_distances
        +bool sssp_spring_adjust_enabled
        +usize num_constraints
    }
    ForceChannel --> FeatureFlags : flag_bit() L132
    ForceDispatchInputs --> FeatureFlags : derive_dispatch_feature_flags() L486

    note for FeatureFlags "ADR-2060: bit3 and bit5 are RESERVED, not a divergence - never set"
    note for ForceChannel "ADR-2029 by design: Constraints read-only L210 - apply() early-returns L191"
    note for ForceChannel "RESOLVED ADR-2060: 180-byte header comment corrected to 212"
```

## VC-11.5 Constraint residency — upload and flag consequence

```mermaid
sequenceDiagram
    autonumber
    participant API as constraints_handler<br/>src/handlers/constraints_handler.rs:13
    participant CA as ConstraintActor<br/>constraint_actor.rs:193
    participant OCA as OntologyConstraintActor<br/>ontology_constraint_actor.rs:451
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3700
    participant EX as execution.rs:954

    API->>CA: UpdateConstraints from POST /constraints/define or /apply :193
    CA->>CA: GetConstraints :215, ClearConstraints :256, GetConstraintStatistics :264
    CA->>FCA: UploadConstraintsToGPU :224
    API->>OCA: ApplyOntologyConstraints :451
    alt GPU context absent
        Note over OCA: info "GPU not available, constraints cached for next physics step" :291 and cpu_fallback_count += 1 :286,:293
    else GPU ready
        OCA->>FCA: UpdateOntologyConstraintBuffer :3805
    end
    OCA->>OCA: ApplyMaterializedAxioms :522, AdjustConstraintWeights :723
    FCA->>EX: execute with num_constraints = resident count
    alt num_constraints > 0
        EX->>EX: ENABLE_CONSTRAINTS set :502-504 and constraint_force_ptr bound :221
    else num_constraints == 0
        Note over EX: bit CLEARS - required, or force_pass_kernel keeps walking a buffer that no longer describes anything (force_channels.rs:560-562)
    end
    Note over CA,EX: INVARIANT ADR-2029 - enablement is owned by residency, never by settings
    Note over OCA: class_id, class_charge and class_mass are uploaded as separate per-node device buffers and passed to force_pass_kernel, not carried in SimParams
```

## VC-11.6 Node pinning and the pinned mask

```mermaid
sequenceDiagram
    autonumber
    participant WS as WebSocket drag handler<br/>see VC-16.1
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3367
    participant EX as execution.rs kernel launch
    participant K as force_pass_kernel

    WS->>FCA: PinNodePositions :3367
    FCA->>FCA: set per-node entries in pinned_mask
    FCA->>FCA: reheat_factor = max(reheat_factor, 0.3) :3380
    FCA->>EX: execute
    EX->>K: pinned_mask.as_device_ptr() passed as final kernel argument
    Note over K: pinned nodes SKIP integration so they hold position, but STILL exert forces on neighbours
    WS->>FCA: UploadPositions :3303 for a dragged node's new location
    Note over FCA: ResetPositions :3894 clears the layout and sets reheat_factor = 1.0 :3969
```

## VC-11.7 Semantic forces and stress majorization

```mermaid
sequenceDiagram
    autonumber
    participant PS as PhysicsSupervisor<br/>physics_supervisor.rs:1032
    participant SFA as SemanticForcesActor<br/>semantic_forces_actor.rs:751
    participant SMA as StressMajorizationActor<br/>stress_majorization_actor.rs:315
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3724

    PS->>SFA: ConfigureDAG :751
    PS->>SFA: ConfigureTypeClustering :778
    PS->>SFA: ConfigureCollision :802
    PS->>SFA: RecalculateHierarchy :844
    SFA->>SFA: ReloadRelationshipBuffer :869
    Note over SFA: DAG radial bias is SELF-GATED on dag_bias_k > 0 with dag_level_distance - it has no feature-flag bit (force_channels.rs:34-36)
    alt GPU absent
        Note over SFA: CPU fallback implementations exist :221
    end
    PS->>SMA: TriggerStressMajorization :315
    alt no GPU context
        Note over SMA: returns Err "GPU not available for stress majorization" :95 - hard failure, no CPU degradation
    else GPU ready
        SMA->>SMA: CheckStressMajorization :369, UpdateStressMajorizationParams :347
        SMA->>FCA: TriggerStressMajorization :3724
        FCA->>FCA: GetStressMajorizationStats :3736, ResetStressMajorizationSafety :3752
    end
    Note over SMA,FCA: DIVERGENCE bit5 ENABLE_STRESS_MAJORIZATION is declared but never set by derive_dispatch_feature_flags - stress majorization is not a GPU force channel
    Note over SMA,FCA: Stress majorization params live on CPU in SemanticProcessorActor and are absent from GPU SimParams - src/models/simulation_params.rs:77
    Note over SMA,FCA: DIVERGENCE bit3 ENABLE_TEMPORAL_COHERENCE is likewise declared but never set - both are reserved bits, not wired force terms
```

## VC-11.8 Layout mode switch — post-ADR-2055

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as layout_handler<br/>src/handlers/layout_handler.rs
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:2641

    C->>API: POST /layout/mode with a mode string
    API->>API: parse into LayoutMode
    alt mode does not parse
        Note over API: RESOLVED ADR-2055 - returns 400 ErrorBadRequest naming the accepted values. It previously coerced silently to ForceDirected, turning a client error into a wrong-looking-right result
    else parsed
        API->>FCA: SetLayoutMode :2641
        FCA->>FCA: apply mode on the live UnifiedGPUCompute path
        FCA->>FCA: reheat_factor = max(reheat_factor, 1.5) :2849 on UpdateSimulationParams
    end
    C->>API: POST /layout/radial
    API->>FCA: SetRadialLayout :2730
    Note over API: RESOLVED ADR-2055 - the advertised mode list is now five (forceDirected, hierarchical, radial, spectral, temporal). Clustered is excluded because ForceComputeActor has no dedicated arm for it and it is indistinguishable from ForceDirected
    Note over FCA: Evidence for the exclusion - the CUDA layout_mode field is never read in the .cu, only dag_bias_k and layer_bias_k are, and those are primed only for Radial and Hierarchical
    Note over API,FCA: REMOVED ADR-2055 - the five-engine physics-v2 LayoutEngine registry (engine_for, src/physics/engines/) is deleted along with the Cargo feature. Its step() bodies were stubs and it was never in a shipped build, so no participant for it remains here
```

## VC-11.9 Warmup, settle and reheat cycle

```mermaid
stateDiagram-v2
    [*] --> Warmup
    Warmup: Stability warmup - stability_warmup_remaining decremented each tick at line 2007
    Active: Integrating - mean per-node KE at or above SETTLE_KE_EPSILON
    Settling: Consecutive sub-epsilon ticks accumulating in settle_stable_frames
    Settled: is_settled true - reported via GetSettlementState line 3607
    Reheated: reheat_factor greater than 0 - velocity perturbation injected

    Warmup --> Active: warmup frames exhausted
    Active --> Settling: next_stable_frames increments when mean_ke finite and below epsilon line 450-456
    Settling --> Active: KE at or above epsilon resets the run to 0 line 495
    Settling --> Settled: settle_stable_frames reaches SETTLE_FRAME_THRESHOLD
    Active --> Settled: paused latch - orchestrator declared an energy plateau line 470
    Settled --> Reheated: settings change, pin, or reset raises reheat_factor
    Reheated --> Active: decay to zero
    Reheated --> Reheated: reheat_factor multiplied by 0.997 each step line 2103

    note right of Warmup
        warmup = 1200 frames when edge_count == 0
        else 600 frames, reset after graph upload
        force_compute_actor.rs:1424
    end note
    note right of Settled
        SETTLE_KE_EPSILON = 1e-4 line 439
        SETTLE_FRAME_THRESHOLD = 180 line 444
        180 frames is about 3 s at 60 fps and mirrors
        the auto-balance stabilityFrameCount config
        so REST and auto-balance agree on settled
    end note
    note right of Reheated
        inject_velocity_perturbation(reheat_factor) line 2053 when factor > 0 line 2048
        decay 0.997 per step over about 30 steps line 2090,2103
        snapped to 0.0 once below 0.02 line 2104-2105
        seeds - UpdateSimulationParams max 1.5 line 2849
        UpdateClusteringParams assign line 3055
        PinNodePositions max 0.3 line 3380
        ResetPositions 1.0 line 3969
    end note
    note right of Active
        Periodic full broadcast and projection
        gate on iteration_count % 300 line 2204
        skipped frames logged every 300th line 1892,1934
    end note
```
