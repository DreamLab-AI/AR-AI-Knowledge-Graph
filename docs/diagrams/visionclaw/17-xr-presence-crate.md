---
id: VC-17
title: XR presence crate and co-presence
area: visionclaw
governing:
  - docs/PROTOCOL-registry.md
  - docs/XR-client.md
adrs: [ADR-2019, ADR-2020]
sources:
  - crates/visionclaw-xr-presence/src/lib.rs
  - crates/visionclaw-xr-presence/src/wire.rs
  - crates/visionclaw-xr-presence/src/agent_presence.rs
  - crates/visionclaw-xr-presence/src/validate.rs
  - crates/visionclaw-xr-presence/src/room.rs
  - crates/visionclaw-xr-presence/src/types.rs
  - crates/visionclaw-xr-presence/src/delta.rs
  - crates/visionclaw-xr-presence/src/error.rs
  - src/handlers/presence_handler.rs
  - src/actors/presence_actor.rs
verified_commit: bed6b617d
---

## VC-17.1 Presence session handshake and authentication

```mermaid
sequenceDiagram
    autonumber
    participant C as XR client<br/>xr-client/rust
    participant PS as PresenceSession<br/>src/handlers/presence_handler.rs:434
    participant PA as PresenceActor<br/>src/actors/presence_actor.rs:652

    C->>PS: WebSocket upgrade on /ws/presence
    PS->>PS: phase = SessionPhase::Challenged{nonce, ts_us} :156
    PS->>C: send_challenge :163 - ServerHandshake::Challenge{nonce hex, ts}
    C->>PS: ws::Message::Text routed to handle_auth :461
    alt phase is not Challenged
        Note over PS: warn "auth attempted in wrong phase" then close_with CLOSE_CODE_VALIDATION 4400 :192-195
    else malformed JSON
        Note over PS: close_with_code ws::CloseCode::Unsupported "malformed json" :204
    else ClientHandshake::Auth{did, signature, room_id, metadata, ts}
        PS->>PS: verify signature over the issued nonce via the IdentityVerifier port
        alt signature invalid
            Note over PS: close with CLOSE_CODE_AUTH_FAIL 4401 :37
        else verified
            PS->>PA: JoinRoom :652
            PA->>PA: local_id_for(avatar_id) :360 assigns the compact u32 wire id
            PS->>PS: phase = SessionPhase::Joined :335
        end
    end
    opt handshake not completed in time
        PS->>PS: enforce_handshake_deadline :395 while phase is still Challenged :396
        Note over PS: HANDSHAKE_TIMEOUT = 10s :34
    end
    loop every HEARTBEAT_INTERVAL 15s :35
        PS->>C: heartbeat :403
    end
    Note over PS: INVARIANT authentication is DID-signature over a server-issued nonce - the challenge is per session, never reused
```

## VC-17.2 Headset pose to peers — 0x43 path

```mermaid
sequenceDiagram
    autonumber
    participant HMD as Headset / Godot client
    participant PS as PresenceSession<br/>presence_handler.rs:355 handle_pose_frame
    participant W as wire::decode<br/>crates/visionclaw-xr-presence/src/wire.rs
    participant V as validators<br/>crates/visionclaw-xr-presence/src/validate.rs
    participant PA as PresenceActor<br/>presence_actor.rs:739 IngestPose
    participant PEERS as Other sessions in room

    HMD->>PS: ws::Message::Binary routed to handle_pose_frame :462
    PS->>PS: check_rate_limit :175
    alt frame_window length >= RATE_LIMIT_FRAMES_PER_SEC
        Note over PS: 120 frames per RATE_LIMIT_WINDOW 1s :32-33, close with CLOSE_CODE_RATE_LIMIT 4429 :38
    else within budget
        PS->>W: decode(bytes)
        Note over W: [0x43][u16 frame_len LE][u8_16 room_id_hash][u8 avatar_id_len][avatar_id utf8][u64 timestamp_us LE][u8 transform_mask] then 28-byte transforms
        Note over W: transform_mask bits head 0b001 :18, left 0b010 :19, right 0b100 :20 - a PRESENCE bitmask not a count, so asymmetric hand presence round-trips
        PS->>PA: IngestPose :739
        PA->>V: run_validators :315
        V->>V: monotonic_timestamp :96
        V->>V: world_bounds against Aabb :77
        V->>V: velocity_gate against max_velocity_mps :10
        V->>V: hand_reach against configured_hand_reach_m :45 and DEFAULT_HAND_REACH_M
        V->>V: joint_anatomy on the two HandPoses :118
        alt any validator returns ValidationError
            PA->>PA: record_violation(avatar_id) :299
            Note over PA: frame rejected - a violating pose never reaches peers
        else all pass
            PA->>PA: build_broadcast_frame :370
            Note over PA: fan-out envelope [0x43][u64 broadcast_seq LE][u32 room_id LE][u16 user_count LE] - PREAMBLE_OPCODE :54, layout comment :53, push at :379
            PA->>PEERS: BroadcastFrame
        end
    end
    Note over PA,PEERS: Bounds and max velocity come from PresenceRoom::with_bounds :293
```

## VC-17.3 Agent co-presence — 0x44 sibling channel

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent runtime
    participant PA as PresenceActor<br/>presence_actor.rs:811 IngestAgentPresence
    participant AP as agent_presence codec<br/>crates/visionclaw-xr-presence/src/agent_presence.rs
    participant PEERS as Room members

    AG->>PA: IngestAgentPresence :811
    PA->>PA: handle_agent_presence :424
    PA->>PA: check_node_correlation :415
    alt attention target names a node id that is not correlated
        Note over PA: returns Err(u32) carrying the offending node id
    else accepted
        PA->>PA: attention_node(avatar_id) :403 - the node correlation between a 0x44 attention target and a graph node
        PA->>AP: encode_agent_presence
        Note over AP: [0x44][u16 body_len LE][u64 seq LE][u16 agent_count LE] then per agent [u32 local_id LE][u8 field_mask]
        Note over AP: field_mask bit0 state u8 - 0 idle, 1 working, 2 awaiting_approval, 3 speaking
        Note over AP: field_mask bit1 gaze i16x3 quantised unit direction, scale 32767.0 via quantise_dir and dequantise_dir
        Note over AP: field_mask bit2 attention u8 tag - 0 none, 1 user, 2 node - plus u32 node_id when tag == 2
        PA->>PEERS: broadcast_agent_presence :479
    end
    loop periodic
        PA->>PA: SweepStaleAgentPresence :819 retires agents that stopped publishing
    end
    Note over PA: A retired agent is announced on the JSON event channel rather than as a 0x44 delta :230-235
    Note over PA: The 0x44 broadcast sequence is SEPARATE from the 0x43 pose sequence :264
    Note over AP: ADR-2020 additive sibling opcode - two logical channels share one codec, reliable for state and attention, high-rate 10-20 Hz for gaze-only
    Note over PA,PEERS: Queries GetAgentPresence :828 and GetAttentionNode :836 read this state without broadcasting
```

## VC-17.4 Session phase lifecycle

```mermaid
stateDiagram-v2
    [*] --> Challenged
    Challenged: Challenged - nonce and ts_us issued, set at presence_handler.rs line 156
    Joined: Joined - room membership established, set at line 335
    Closed: Closed - CloseReason sent

    Challenged --> Joined: handle_auth verifies the DID signature over the nonce line 191
    Challenged --> Closed: auth in wrong phase - code 4400 line 192
    Challenged --> Closed: malformed json - CloseCode Unsupported line 204
    Challenged --> Closed: signature invalid - code 4401 CLOSE_CODE_AUTH_FAIL line 37
    Challenged --> Closed: HANDSHAKE_TIMEOUT 10s elapsed - enforce_handshake_deadline line 395
    Joined --> Closed: rate limit exceeded - code 4429 CLOSE_CODE_RATE_LIMIT line 38
    Joined --> Closed: client disconnect - LeaveRoom line 730
    Closed --> [*]

    note right of Challenged
        Binary frames are only accepted in Joined
        Text frames carry the handshake JSON
        StreamHandler dispatch at lines 458 to 463
        Text to handle_auth, Binary to handle_pose_frame
    end note
    note right of Joined
        HEARTBEAT_INTERVAL 15s line 35
        RATE_LIMIT_FRAMES_PER_SEC 120 line 32
        RATE_LIMIT_WINDOW 1s line 33
    end note
```

## VC-17.5 Crate structure and transport-agnostic ports

```mermaid
classDiagram
    class lib_rs {
        +single_source_of_truth_for_0x43()
        +consumed_by_server_and_godot_client()
        +no_transport_assumed()
    }
    class wire {
        +OPCODE_AVATAR_POSE_0x43 L9
        +encode()
        +decode()
    }
    class agent_presence {
        +OPCODE_AGENT_PRESENCE_0x44 L40
        +encode_agent_presence()
        +decode_agent_presence()
        +quantise_dir()
        +dequantise_dir()
        +AgentActivity()
        +AttentionTarget()
        +PresenceChannel()
        +AgentPresenceBatch()
        +AgentPresenceDelta()
    }
    class validate {
        +velocity_gate() L10
        +world_bounds() L77
        +monotonic_timestamp() L96
        +joint_anatomy() L118
        +hand_reach() L147
        +DEFAULT_HAND_REACH_M()
    }
    class room {
        +PresenceRoom()
        +AvatarState()
    }
    class types {
        +Aabb()
        +AvatarId()
        +AvatarMetadata()
        +Did()
        +HandPose()
        +PoseFrame()
        +RoomId()
        +Transform()
    }
    class delta {
        +PoseDelta()
        +TransformMask()
    }
    class error {
        +RoomError()
        +ValidationError()
        +WireError()
    }
    class ports {
        +Broadcaster()
        +IdentityVerifier()
        +SignedChallenge()
    }
    lib_rs --> wire
    lib_rs --> agent_presence
    lib_rs --> validate
    lib_rs --> room
    lib_rs --> types
    lib_rs --> delta
    lib_rs --> error
    lib_rs --> ports
    room --> validate : invariants enforced by
    ports --> lib_rs : injected by each consumer

    note for ports "No transport assumed - Actix, tokio-tungstenite and godot signal all inject these traits"
    note for lib_rs "Consumed by src/handlers/presence_handler.rs, src/actors/presence_actor.rs and xr-client/rust"
    note for wire "ADR-2019 0x43 and 0x44 are allocated on the /ws/presence socket - see VC-14.9"
```

## VC-17.6 PresenceActor message surface

```mermaid
classDiagram
    class PresenceActor {
        +JoinRoom() L652
        +LeaveRoom() L730
        +IngestPose() L739
        +IngestAgentPresence() L811
        +SweepStaleAgentPresence() L819
        +GetAgentPresence() L828
        +GetAttentionNode() L836
        +ListMembers() L844
        +RoomStats() L852
    }
    class PresenceRoomState {
        +new() L271
        +with_bounds_aabb_and_max_velocity() L293
        +local_id_for() L360
        +build_broadcast_frame() L370
        +attention_node() L403
        +check_node_correlation() L415
        +handle_agent_presence() L424
        +broadcast_agent_presence() L479
        +record_violation() L299
        +run_validators() L315
        +agent_presence_seq_separate_from_pose_seq() L264
    }
    class PresenceSession {
        +send_challenge() L163
        +check_rate_limit() L175
        +handle_auth() L191
        +handle_pose_frame() L355
        +enforce_handshake_deadline() L395
        +heartbeat() L403
        +close_with() L413
        +close_with_code() L421
    }
    PresenceSession --> PresenceActor : JoinRoom / IngestPose
    PresenceActor --> PresenceRoomState : owns

    note for PresenceSession "Actix ws actor at presence_handler.rs:434, StreamHandler at :458"
    note for PresenceRoomState "configured_hand_reach_m() L45 reads the deployment hand-reach limit"
    note for PresenceActor "Test-only CollectActor handles BroadcastFrame L879 and RoomEventEnvelope L886"
```
