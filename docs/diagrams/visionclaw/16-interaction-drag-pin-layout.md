---
id: VC-16
title: Interaction — drag, pin, layout, constraints and agent beams
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/PROTOCOL-registry.md
adrs: [ADR-2020, ADR-2029, ADR-2055]
sources:
  - src/handlers/socket_flow_handler/message_routing.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - src/handlers/socket_flow_handler/types.rs
  - src/handlers/layout_handler.rs
  - src/handlers/constraints_handler.rs
  - src/actors/agent_beam_actor.rs
  - src/actors/gpu/force_compute_actor.rs
  - src/actors/gpu/constraint_actor.rs
  - Cargo.toml
verified_commit: b00c28a0d
---

## VC-16.1 Node drag start — pin acquisition

```mermaid
sequenceDiagram
    autonumber
    participant C as Browser / XR client
    participant MR as message_routing<br/>src/handlers/socket_flow_handler/message_routing.rs:80
    participant H as handle_node_drag_start<br/>position_updates.rs:954
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3367

    C->>MR: JSON text frame type nodeDragStart
    Note over C: payload shape { "type": "nodeDragStart", "data": { "nodeId": 42, "position": { "x", "y", "z" } } } documented at position_updates.rs:952
    MR->>H: dispatch :80
    alt act.pubkey is None :960
        Note over H: REJECTED - drag requires an authenticated session pubkey, anonymous clients cannot pin
    else authenticated
        alt data field missing :968
            Note over H: warn "nodeDragStart missing 'data' field"
        else position not finite or out of range :1003
            Note over H: warn "nodeDragStart - rejecting invalid position" and drop
        else valid
            H->>H: drag_last_update.insert(node_id, Instant::now()) :1028
            H->>FCA: do_send PinNodePositions{pins, unpin, reheat} :1065
            FCA->>FCA: set entries in pinned_mask :3367
            FCA->>FCA: reheat_factor = max(reheat_factor, 0.3) :3380
            H->>C: nodeDragStartAck :1077
        end
    end
    Note over FCA: pinned nodes SKIP integration but STILL exert forces on neighbours - see VC-11.6
    Note over MR,FCA: INVARIANT every drag and pin opcode requires a pubkey - the four opcodes are routed at message_routing.rs:80,83,86,89
```

## VC-16.2 Drag update and drag end

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant MR as message_routing<br/>message_routing.rs:86
    participant HU as handle_node_drag_update<br/>position_updates.rs:1105
    participant HE as handle_node_drag_end<br/>position_updates.rs:1311
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3367

    loop while the pointer is held
        C->>MR: nodeDragUpdate with nodeId, position, timestamp :1103
        MR->>HU: dispatch :86
        HU->>HU: consult drag_last_update.get(&node_id) :1131
        alt throttle window not elapsed
            Note over HU: update coalesced - the per-node timestamp map is the rate gate
        else accepted
            HU->>HU: drag_last_update.insert(node_id, now) :1190
            HU->>FCA: do_send PinNodePositions :1219 with the new position
        end
    end
    C->>MR: nodeDragEnd :83
    MR->>HE: dispatch
    HE->>HE: drag_last_update.remove(&node_id) :1343
    Note over HE,FCA: drag end releases the drag bookkeeping - an explicit nodeUnpin is what clears the GPU pin, see VC-16.3
```

## VC-16.3 Unpin and orphaned-drag cleanup on disconnect

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant MR as message_routing<br/>message_routing.rs:89
    participant HU as handle_node_unpin<br/>position_updates.rs:1462
    participant SESS as WebSocket session stopped<br/>types.rs:718
    participant FCA as ForceComputeActor

    alt explicit unpin
        C->>MR: nodeUnpin :89
        MR->>HU: dispatch
        HU->>HU: drag_last_update.remove(&node_id) :1494
        HU->>FCA: do_send PinNodePositions :1500 with the node in the unpin list
    else client vanished mid-drag
        SESS->>SESS: Actor stopped with drags still open
        SESS->>FCA: do_send PinNodePositions{pins: Vec::new(), unpin: node_ids, reheat: false} :719-723
        Note over SESS: debug "Cleaned up N orphaned drags on disconnect" :725
        Note over SESS: warn "Client disconnected with N nodes still dragged, sending release" :727-730
        SESS->>SESS: drag_last_update.clear() :732
    end
    Note over SESS,FCA: INVARIANT a dropped connection can never leave a node pinned forever - cleanup is unconditional on stop
    Note over SESS: reheat is FALSE on the cleanup path so a disconnect does not re-energise the layout
    Note over HU,FCA: a bulk release path also exists at :1535-1560 iterating drag_last_update and issuing PinNodePositions per node
```

## VC-16.4 Agent beam 0x23 fan-out

```mermaid
sequenceDiagram
    autonumber
    participant HUB as agent-action hub<br/>ingest side is ES-02
    participant ABA as AgentBeamActor<br/>src/actors/agent_beam_actor.rs:197 started
    participant BC as BeamCoalescer<br/>agent_beam_actor.rs:104
    participant ENC as encode_agent_actions<br/>src/utils/binary_protocol.rs
    participant CC as ClientCoordinatorActor
    participant WS as Subscribed clients

    HUB->>ABA: AgentActionEnvelope
    ABA->>ABA: project_action :175 maps the envelope onto the identity-blind 0x23 action
    ABA->>ABA: stamp_agent_flag(source_agent_id) :100 sets AGENT_NODE_FLAG 0x80000000 :74
    ABA->>BC: push(event) :131
    alt backlog already at MAX_PENDING_ACTIONS 256 :81
        Note over BC: push returns false - the oldest are dropped, dropped_total :152 counts them, recency is preferred over completeness :78
        Note over ABA: backpressure warned at most every BACKPRESSURE_WARN_INTERVAL 10s :96
    end
    ABA->>BC: encode_pending :158
    BC->>ENC: encode_agent_actions(&self.pending) :162
    Note over ENC: ONE multi-action 0x23 frame - up to MAX_COALESCE_PER_FLUSH 256 actions :86, see VC-14.5 for the 15-byte header layout
    ABA->>CC: try_send BroadcastAgentActionFrame(frame) :285
    alt coordinator mailbox full
        Note over ABA: backlog is NOT cleared - retried after FLUSH_RETRY_INTERVAL 20ms :91
    else accepted
        ABA->>BC: clear() :167
        Note over ABA: debug "AgentBeamActor - dispatched coalesced 0x23 frame (N action(s))" :288
    end
    CC->>WS: fan out on /wss
    Note over ABA,WS: ADR-2020 the 0x23 frame is identity-blind by design - it carries agent-id-space numeric ids only, never a pubkey or DID :46-52
```

## VC-16.5 Layout mode switch over REST — post-ADR-2055

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant LH as layout_handler<br/>src/handlers/layout_handler.rs:275
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:2641

    C->>LH: GET /modes :278
    LH->>C: get_layout_modes :7 - five modes advertised (ADR-2055)
    C->>LH: POST /mode :279
    LH->>LH: set_layout_mode :15 parses the LayoutMode
    alt parse fails
        LH->>C: RESOLVED ADR-2055 - 400 ErrorBadRequest naming the accepted values, no longer a silent coercion to ForceDirected
    else parsed
        LH->>FCA: addr.send(SetLayoutMode{mode}) :41
        FCA->>FCA: SetLayoutMode handler :2641 on the live UnifiedGPUCompute path
    end
    C->>LH: POST /radial :280
    LH->>LH: set_radial_layout :139
    LH->>FCA: SetRadialLayout :2730
    C->>LH: GET status :207 / POST zones :226 / GET zones :237 / reset :243
    LH->>FCA: reset_layout :243 triggers ResetPositions :3894 which sets reheat_factor = 1.0 :3969
    Note over LH: RESOLVED ADR-2055 - Clustered is no longer advertised. It had no dedicated arm in SetLayoutMode and was indistinguishable from ForceDirected, so advertising it promised behaviour the server could not deliver
    Note over LH,FCA: REMOVED ADR-2055 - the physics-v2 engine registry that this diagram previously showed as a compiled-out alternative path no longer exists
```

## VC-16.6 Constraint CRUD to GPU residency

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant CH as constraints_handler<br/>src/handlers/constraints_handler.rs:11
    participant CA as ConstraintActor<br/>src/actors/gpu/constraint_actor.rs:193
    participant FCA as ForceComputeActor<br/>force_compute_actor.rs:3700
    participant EX as execution.rs flag derivation

    C->>CH: POST /constraints/define :14
    CH->>CH: define_constraints :22 takes web::Json<ConstraintSystem>
    CH->>CA: UpdateConstraints :193
    C->>CH: POST /constraints/apply :15
    CH->>CH: apply_constraints :126 over an untyped Value
    alt constraintType absent
        Note over CH: ErrorBadRequest "constraintType is required" :143
    else nodeIds absent
        Note over CH: ErrorBadRequest "nodeIds array is required" :148
    else valid
        CH->>CA: UpdateConstraints
    end
    C->>CH: POST /constraints/remove :16
    CH->>CH: remove_constraints :191
    CH->>CA: ClearConstraints :256
    C->>CH: GET /constraints/list :17
    CH->>CA: GetConstraints :215 and GetConstraintStatistics :264
    C->>CH: POST /constraints/validate :18
    CH->>CH: validate_constraint_definition :282 over LegacyConstraintData - pure validation, no actor call
    CA->>FCA: UploadConstraintsToGPU :224 then :3700
    FCA->>EX: next physics step with num_constraints
    alt num_constraints > 0
        Note over EX: ENABLE_CONSTRAINTS set by derive_dispatch_feature_flags force_channels.rs:502-504
    else zero
        Note over EX: bit CLEARS - the kernel must not walk a stale buffer
    end
    Note over CH,EX: DIVERGENCE ADR-2029 enablement is RESIDENCY-owned - the force-channel registry marks Constraints read-only (is_read_only force_channels.rs:210) so any UI toggle routed through the registry is inert by design
```

## VC-16.7 Interaction surface map

```mermaid
flowchart LR
    subgraph WSOPS["WebSocket JSON opcodes - /wss, pubkey required"]
        D1["nodeDragStart<br/>message_routing.rs:80 to position_updates.rs:954"]
        D2["nodeDragEnd<br/>message_routing.rs:83 to position_updates.rs:1311"]
        D3["nodeDragUpdate<br/>message_routing.rs:86 to position_updates.rs:1105"]
        D4["nodeUnpin<br/>message_routing.rs:89 to position_updates.rs:1462"]
    end
    subgraph REST["REST surfaces"]
        L1["GET /modes :278"]
        L2["POST /mode :279"]
        L3["POST /radial :280"]
        K1["POST /constraints/define :14"]
        K2["POST /constraints/apply :15"]
        K3["POST /constraints/remove :16"]
        K4["GET /constraints/list :17"]
        K5["POST /constraints/validate :18"]
    end
    subgraph SRVPUSH["Server-initiated"]
        B1["0x23 AGENT_ACTION beam<br/>agent_beam_actor.rs:285"]
    end
    GPU["ForceComputeActor pinned_mask and SimParams<br/>force_compute_actor.rs:3367"]
    CA["ConstraintActor<br/>constraint_actor.rs:193"]
    CC["ClientCoordinatorActor fan-out"]

    D1 -->|"PinNodePositions :1065"| GPU
    D3 -->|"PinNodePositions :1219"| GPU
    D4 -->|"PinNodePositions :1500"| GPU
    D2 -->|"drag_last_update.remove :1343"| GPU
    L2 -->|"SetLayoutMode :41"| GPU
    L3 -->|"SetRadialLayout"| GPU
    K1 --> CA
    K2 --> CA
    K3 --> CA
    CA -->|"UploadConstraintsToGPU :224"| GPU
    B1 --> CC
    CC --> WSOPS

    N1["Disconnect cleanup releases every orphaned pin - types.rs:719-723"]
    N2["Ingest side of the agent beam is estate ES-02 - this file covers fan-out only"]
    GPU -.- N1
    B1 -.- N2
```
