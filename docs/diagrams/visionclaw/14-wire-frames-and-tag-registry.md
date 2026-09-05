---
id: VC-14
title: Wire frames and tag registry
area: visionclaw
governing:
  - docs/PROTOCOL-registry.md
  - docs/GPU-wire-abi.md
adrs: [ADR-2018, ADR-2019, ADR-2020, ADR-2024, ADR-2057, ADR-2060]
sources:
  - src/utils/binary_protocol.rs
  - src/protocols/binary_settings_protocol.rs
  - crates/visionclaw-protocol/src/lib.rs
  - crates/visionclaw-protocol/src/wire_fixtures.rs
  - crates/visionclaw-xr-presence/src/wire.rs
  - crates/visionclaw-xr-presence/src/agent_presence.rs
  - xr-client/rust/src/binary_protocol.rs
  - client/src/services/binaryProtocol/frameTypes.ts
  - client/src/services/BinaryWebSocketProtocol.ts
  - src/actors/agent_beam_actor.rs
verified_commit: b00c28a0d
---

## VC-14.1 V3 position record — 52 bytes, little-endian

```mermaid
classDiagram
    class WireNodeDataItemV3 {
        +u32 id__at0_4B
        +Vec3Data position__at4_12B
        +Vec3Data velocity__at16_12B
        +f32 sssp_distance__at28_4B
        +i32 sssp_parent__at32_4B
        +u32 cluster_id__at36_4B
        +f32 anomaly_score__at40_4B
        +u32 community_id__at44_4B
        +f32 centrality__at48_4B
        +TOTAL_52_BYTES()
    }
    class WireSizeConstants {
        +WIRE_V2_ID_SIZE_4 L74
        +WIRE_VEC3_SIZE_12 L75
        +WIRE_F32_SIZE_4 L76
        +WIRE_I32_SIZE_4 L77
        +WIRE_U32_SIZE_4 L78
        +WIRE_V3_ITEM_SIZE_52 L72_L82
        +WIRE_ITEM_SIZE_alias L84
    }
    WireNodeDataItemV3 --> WireSizeConstants : size checked by

    class SizeLockEvidence {
        +struct_at_binary_protocol_rs L44
        +v5_seq_const_assert L102
        +const_assert_52 L93
        +redundant_test_asserts L1077_L1174
        +frame_is_tag_byte_then_N_records()
        +little_endian_throughout()
    }
    WireSizeConstants --> SizeLockEvidence

    note for SizeLockEvidence "INVARIANT ADR-2018 WIRE_V3_ITEM_SIZE == 52"
    note for SizeLockEvidence "RESOLVED ADR-2060: registry citations corrected"
    note for SizeLockEvidence "RESOLVED ADR-2057: now a compile-time const assert"
```

## VC-14.2 Wire node id — 26-bit id plus type flag bits

```mermaid
classDiagram
    class WireNodeId {
        +bits_0_to_25_node_id
        +bit_26_ONTOLOGY_CLASS_0x04000000 L20
        +bit_27_ONTOLOGY_INDIVIDUAL_0x08000000 L21
        +bit_28_ONTOLOGY_PROPERTY_0x10000000 L22
        +bit_30_KNOWLEDGE_0x40000000 L16
        +bit_31_AGENT_0x80000000 L15
    }
    class Masks {
        +NODE_ID_MASK_0x03FFFFFF L26
        +ONTOLOGY_TYPE_MASK_0x1C000000 L19
        +MAX_ID_67108863()
    }
    class RemapFunctions {
        +remap_wire_id() L166
        +to_wire_id() L200
        +strip_flags() L216_L228
    }
    WireNodeId --> Masks
    Masks --> RemapFunctions

    note for Masks "INVARIANT ADR-2024 NODE_ID_MASK = 0x03FF_FFFF - flag bits 26-31 are stripped before analytics and SSSP map lookups"
    note for RemapFunctions "Ids above the mask are remapped and logged NODE ID OVERFLOW at :179 - remap_wire_id returns (masked_id, overflowed)"
    note for RemapFunctions "Encoder strips flag bits before SSSP and analytics lookups - the maps are keyed by compact id, skipping this is a real bug class"
    note for WireNodeId "Ontology sub-type bits are only meaningful when the node belongs to GraphType::Ontology (comment :19)"
```

## VC-14.3 Tag byte 0 registry — two disjoint socket spaces

```mermaid
flowchart TD
    subgraph GRAPH["/wss graph socket - tag space A"]
        T03["0x03 Graph position frame V3<br/>52 B per node<br/>binary_protocol.rs:12 PROTOCOL_V3, dispatch :566"]
        T05["0x05 V5 broadcast envelope wrapping V3<br/>PROTOCOL_V5 branch binary_protocol.rs:592"]
        T23["0x23 AGENT_ACTION beam event<br/>MessageType::AgentAction :1721"]
        T34["0x34 BroadcastAck client to server<br/>MessageType::BroadcastAck :1690"]
        T02["0x02 VoiceData :1684"]
        T03c["0x03 ControlFrame in MessageType :1686"]
    end
    subgraph PRESENCE["/ws/presence socket - tag space A sibling"]
        T43["0x43 OPCODE_AVATAR_POSE<br/>crates/visionclaw-xr-presence/src/wire.rs:9"]
        T44["0x44 OPCODE_AGENT_PRESENCE<br/>crates/visionclaw-xr-presence/src/agent_presence.rs:40"]
    end
    subgraph SETTINGS["settings binary socket - tag space B, DISJOINT"]
        S05["0x05 settings batch or value-change message<br/>src/protocols/binary_settings_protocol.rs"]
        S03["0x03 BatchGet in the same space"]
    end

    N1["INVARIANT ADR-2019 tag allocation is per socket - the numeric overlap between 0x05 graph V5 and 0x05 settings is safe only because the sockets are demultiplexed independently"]
    N2["RESOLVED ADR-2060: BASELINE cited the SETTINGS 0x05 as the graph V5 envelope. Corrected, and the registry now warns that a 0x05 citation must always name its socket"]
    N3["DIVERGENCE MessageType enum :1680 reuses 0x03 for ControlFrame while PROTOCOL_V3 also uses 0x03 as the position-frame lead byte - disambiguated only by direction and call site"]
    N4["RESOLVED ADR-2057: the V5 envelope now has an owning ADR fixing its layout [0x05] u64 seq then V3 body, and the broadcast_seq contract"]
    N5["OPEN: the settings binary protocol is still not fully enumerated in the registry - unchanged by the 2026-09-05 remediation"]
    N6["RESOLVED ADR-2060: the legacy 48B/28B ADR figures are marked retired in the governing docs - the wire is 52B, now compile-time locked"]
    T05 -.- N2
    T03 -.- N3
    T05 -.- N4
    SETTINGS -.- N5
    T03 -.- N6
    SETTINGS -.- N1
```

## VC-14.4 V5 envelope and tag dispatch with unknown-tag rejection

```mermaid
sequenceDiagram
    autonumber
    participant SRV as Server encoder<br/>src/utils/binary_protocol.rs
    participant SOCK as WebSocket /wss
    participant DEC as decode dispatch<br/>binary_protocol.rs:588 match protocol_version
    participant V3 as decode_node_data_v3<br/>binary_protocol.rs:610

    SRV->>SOCK: frame bytes
    SOCK->>DEC: data[0] read as protocol_version :585, payload = data[1..] :586
    alt version == 1
        Note over DEC: Err "Protocol V1 is no longer supported. Please upgrade client." :589
    else version == 2
        Note over DEC: Err "V2 protocol no longer supported. Please upgrade client to V3+." :590
    else version == PROTOCOL_V3 (0x03)
        DEC->>V3: decode_node_data_v3(payload) :591
    else version == PROTOCOL_V5 (0x05) :592
        Note over DEC: V5 layout is [0x05] then u64 broadcast_seq LE then the V3 body
        alt payload.len() < WIRE_V5_SEQ_SIZE
            Note over DEC: Err "V5 frame too small for broadcast sequence" :594
        else
            DEC->>V3: decode_node_data_v3(&payload[WIRE_V5_SEQ_SIZE..]) :597
        end
    else any other byte
        Note over DEC: Err "Unknown protocol version" :600 - INVARIANT unknown tags are REJECTED, never reinterpreted
    end
    V3->>V3: reject when data.len() % WIRE_V3_ITEM_SIZE != 0
    V3->>V3: expected_nodes = data.len() / WIRE_V3_ITEM_SIZE then chunks_exact(52)
    Note over DEC,V3: The V5 body carries NO inner 0x03 byte - the receiver distinguishes 0x03 from 0x05 by the lead byte alone
    Note over DEC: broadcast_seq gives clients a monotonic ordering and drop-detection handle
    Note over DEC,V3: RESOLVED ADR-2060 - the V5 decode citation is corrected, and ADR-2057 replaced the bare literal 5 with a named PROTOCOL_V5 constant and WIRE_V5_SEQ_SIZE
```

## VC-14.5 0x23 AGENT_ACTION frame

```mermaid
classDiagram
    class AgentActionEvent {
        +u32 source_agent_id__at0_4B L1491
        +u32 target_node_id__at4_4B L1492
        +u8 action_type__at8_1B L1493
        +u32 timestamp_ms__at9_4B L1469
        +u16 duration_ms__at13_2B L1470
        +Vec~u8~ payload__at15_variable L1471
    }
    class AgentActionType {
        <<enum>>
        +Query_0 L1439
        +Update_1 L1440
        +Create_2 L1441
        +Delete_3 L1442
        +Link_4 L1443
        +Transform_5 L1444
    }
    class FrameLayout {
        +AGENT_ACTION_HEADER_SIZE_15 L1475
        +lead_byte_MessageType_AgentAction_0x23 L1694
        +capacity_1_plus_15_plus_payload L1500
    }
    AgentActionEvent --> AgentActionType : action_type
    AgentActionEvent --> FrameLayout : encoded by

    note for FrameLayout "Frame = [0x23] then one or more 15-byte headers each with an optional variable payload - multiple actions coalesce into ONE frame"
    note for AgentActionEvent "ADR-2020 identity-blind by design - carries agent-id-space numeric ids only, never a pubkey or DID"
    note for AgentActionEvent "RESOLVED ADR-2060: the ~+340-line 0x23 citation drift is corrected"
    note for AgentActionType "Coalescing and fan-out live in src/actors/agent_beam_actor.rs - see VC-16.4, ingest side is ES-02"
```

## VC-14.6 Encode to decode of one V5 frame, server to clients

```mermaid
sequenceDiagram
    autonumber
    participant FCA as ForceComputeActor<br/>see VC-13.3
    participant ENC as encode_node_data_extended_with_sssp<br/>src/utils/binary_protocol.rs
    participant WS as WebSocket /wss
    participant RS as XR Rust decoder<br/>xr-client/rust/src/binary_protocol.rs:400
    participant TS as Web TS decoder<br/>client/src/types/binaryProtocol.ts:410

    FCA->>ENC: node tuples plus agent and knowledge id lists
    ENC->>ENC: per node remap_wire_id :166 then stamp type flag bits
    ENC->>ENC: strip flag bits before SSSP and analytics map lookups
    ENC->>ENC: write 52-byte record - id@0 pos@4 vel@16 sssp_dist@28 sssp_parent@32 cluster@36 anomaly@40 community@44 centrality@48
    Note over ENC: sssp_distance defaults to f32::INFINITY and sssp_parent to -1 when absent
    ENC->>WS: [tag][records...]
    par XR Rust client
        WS->>RS: bytes
        RS->>RS: decode_position_frame_with_sequence :400
        alt lead byte == PROTOCOL_V5 (0x05) :415
            RS->>RS: need = HEADER_BYTES + V5_SEQ_BYTES :416 then read u64 seq :423
        end
        RS->>RS: mask each id with NODE_ID_MASK :289
        Note over RS: constants PROTOCOL_V5 :25, V5_SEQ_BYTES 8 :26, NODE_RECORD_BYTES 52 :28, NODE_ID_MASK :37
    and Web TS client
        WS->>TS: bytes
        TS->>TS: parseBinaryNodeData dispatch on lead byte :185-215
        alt lead byte == PROTOCOL_V5 (0x05) :198-201
            TS->>TS: parseV5Nodes :410 - reject if byteLength < 9 :412, read u64 seq LE :417-419, decode body from offset 9 :422
            TS->>TS: surface lastBroadcastSequence :459 - store uses it as the ack sequence binaryProtocol.ts:416
        else lead byte == 0x02 (V2)
            TS->>TS: DECLINED with a diagnostic - the server rejects V2 at binary_protocol.rs:590
        else unrecognised version
            TS->>TS: DECLINED - no size autodetection
        end
        Note over TS: CORRECTED ADR-2078. ADR-2057 Finding 1 was WRONG: the live TS path always<br/>had V5, including the short-payload guard that mirrors the server at :594.<br/>client/src/services/binaryProtocol/ is NOT the live position path - it has no<br/>52-byte decoding at all, so a V5 branch there would have been a SECOND decoder.<br/>The real defect was Finding 2 and it was worse than reported - see the next note
        Note over TS: RESOLVED ADR-2078. V2 was DECODED, not merely advertised (36-byte records at<br/>types/binaryProtocol.ts:186-189, routed in at store/websocket/binaryProtocol.ts:476),<br/>and the default arm re-read any unknown frame from offset 0 as 36-byte records<br/>whenever its length divided by 36 - fabricating nodes from arbitrary payloads.<br/>Both now decline. BINARY_NODE_SIZE_V2 and the size-swap heuristic are deleted
        Note over TS: RESOLVED ADR-2078. The TS decoder now has the fixture cross-check the two Rust<br/>decoders always had - client/src/types/__tests__/wireFixtures.test.ts, 12 tests<br/>pinning the same constants as wire_fixtures.rs plus a synthetic V5 round-trip
    end
    Note over ENC,TS: Shared fixtures crates/visionclaw-protocol/src/wire_fixtures.rs pin the format for both decoders - binary_protocol.rs:916-918 asserts fx::NODE_RECORD_BYTES == WIRE_V3_ITEM_SIZE and fx::NODE_ID_MASK == NODE_ID_MASK, and :950 guards the shared 0x23 fixture against encoder drift
```

## VC-14.7 The coexisting binary codecs

```mermaid
classDiagram
    class ServerEncoder {
        +path_src_utils_binary_protocol_rs()
        +WIRE_V3_ITEM_SIZE_52 L72
        +encode_node_data_with_types() L470
        +encode_node_data_with_flags() L511
        +encode_node_data() L526
        +decode_dispatch() L563
        +CANONICAL_source_of_truth()
    }
    class XrRustDecoder {
        +path_xr_client_rust_src_binary_protocol_rs()
        +PROTOCOL_V5_0x05 L25
        +V5_SEQ_BYTES_8 L26
        +NODE_RECORD_BYTES_52 L28
        +NODE_ID_MASK L37
        +decode_position_frame() L388
        +decode_position_frame_with_sequence() L400
    }
    class WebTsDecoder {
        +path_client_src_services_binaryProtocol()
        +PROTOCOL_V2_2 L5
        +PROTOCOL_V3_3 L6
        +PROTOCOL_V4_4 L14
        +SUPPORTED_PROTOCOLS L16
        +no_V5_support()
    }
    class SharedFixtures {
        +path_crates_visionclaw_protocol_src_wire_fixtures_rs()
        +NODE_RECORD_BYTES()
        +NODE_ID_MASK()
        +agent_action_0x23_fixture()
    }
    ServerEncoder --> SharedFixtures : asserted against L916_L918
    XrRustDecoder --> SharedFixtures : same source file included
    WebTsDecoder --> ServerEncoder : must track manually

    note for ServerEncoder "visionclaw-protocol crate's own binary_protocol module was REMOVED as a stale 48-byte copy - lib.rs:11-24 now points here as the single encoder"
    note for WebTsDecoder "ROOT CAUSE (ADR-2057): the TS decoder is the only one with no fixture cross-check"
    note for SharedFixtures "ADR-2018 consumer freshness and ADR-2019 per-opcode frame policy - dependency-free so the isolated xr-client workspace includes the same source"
```

## VC-14.8 V4 framed-message header — client-emitted only

```mermaid
classDiagram
    class V4FramedHeader {
        +u8 type__at0_1B
        +u8 version__at1_1B
        +u32 payloadLength_LE__at2_4B
        +TOTAL_6_BYTES()
    }
    class TagsDeclaredClientSide {
        +GRAPH_UPDATE_0x01 L21
        +VOICE_DATA_0x02 L24
        +POSITION_UPDATE_0x10 L27
        +AGENT_POSITIONS_0x11 L28
        +VELOCITY_UPDATE_0x12 L29
        +AGENT_STATE_FULL_0x20 L32
        +AGENT_STATE_DELTA_0x21 L33
        +AGENT_HEALTH_0x22 L34
        +AGENT_ACTION_0x23 L35
        +CONTROL_BITS_0x30 L38
        +SSSP_DATA_0x31 L39
        +HANDSHAKE_0x32 L40
        +HEARTBEAT_0x33 L41
        +BROADCAST_ACK_0x34 L49
        +VOICE_CHUNK_0x40 L44
        +VOICE_START_0x41 L45
        +VOICE_END_0x42 L46
        +SYNC_UPDATE_0x50 L52
        +ANNOTATION_UPDATE_0x51 L53
        +SELECTION_UPDATE_0x52 L54
    }
    V4FramedHeader --> TagsDeclaredClientSide : type byte drawn from

    class ServerMessageTypeEnum {
        +BinaryPositions_0 L1682
        +VoiceData_0x02 L1684
        +ControlFrame_0x03 L1686
        +AgentAction_0x23 L1694
        +BroadcastAck_0x34 L1690
        +enum_declared_at L1680_L1695
    }
    TagsDeclaredClientSide --> ServerMessageTypeEnum : only 5 of 20 have a server counterpart

    note for V4FramedHeader "Shape at BinaryWebSocketProtocol.ts:81 - 6-byte header"
    note for V4FramedHeader "By design: V4 is not the wire format - live streams are UNFRAMED"
    note for TagsDeclaredClientSide "OPEN: client declares 20 tags, server defines 5 - routed to vc-clients"
    note for ServerMessageTypeEnum "RESOLVED ADR-2057: the 48-byte comment is corrected to 52"
```

## VC-14.9 Presence sibling opcodes 0x43 and 0x44

```mermaid
classDiagram
    class AvatarPose_0x43 {
        +u8 opcode_0x43 wire_rs_L9
        +u16 frame_len_LE
        +u8_16 room_id_hash
        +u8 avatar_id_len
        +avatar_id_utf8
        +u64 timestamp_us_LE
        +u8 transform_mask
        +transforms_28B_each_head_left_right
    }
    class TransformMask {
        +head_0b001 wire_rs_L18
        +left_0b010 wire_rs_L19
        +right_0b100 wire_rs_L20
    }
    class AgentPresence_0x44 {
        +u8 opcode_0x44 agent_presence_rs_L40
        +u16 body_len_LE
        +u64 seq_LE
        +u16 agent_count_LE
        +per_agent_u32_local_id
        +per_agent_u8_field_mask
    }
    class FieldMask {
        +bit0_state_u8_idle_working_awaiting_speaking
        +bit1_gaze_i16x3_quantised_scale_32767
        +bit2_attention_u8_tag_plus_u32_node_id_when_tag_2
    }
    AvatarPose_0x43 --> TransformMask
    AgentPresence_0x44 --> FieldMask

    note for TransformMask "Presence BITMASK not a count - asymmetric hand presence round-trips correctly"
    note for AgentPresence_0x44 "ADR-2020 additive sibling of 0x43 carrying social state, not skeleton - see VC-17 for the presence pipeline"
    note for AgentPresence_0x44 "Two logical channels share one codec - reliable for state and attention, high-rate 10-20 Hz for gaze-only"
    note for AvatarPose_0x43 "Server fan-out envelope [0x43][u64 broadcast_seq LE][u32 room_id LE][u16 user_count LE] built at src/actors/presence_actor.rs:370-379"
```
