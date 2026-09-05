---
id: VC-32
title: Client WebSocket transport and binary position protocol
area: visionclaw
governing:
  - docs/PROTOCOL-registry.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2002, ADR-2019, ADR-2020, ADR-2047, ADR-2057, ADR-2078, ADR-2080]
sources:
  - client/src/store/websocket/connectionManager.ts
  - client/src/store/websocketStore.ts
  - client/src/store/websocket/index.ts
  - client/src/store/websocket/storeState.ts
  - client/src/store/websocket/types.ts
  - client/src/store/websocket/binaryFrameDispatcher.ts
  - client/src/store/websocket/binaryProtocol.ts
  - client/src/store/websocket/filterSync.ts
  - client/src/store/websocket/textMessageHandler.ts
  - client/src/store/websocket/solidWebSocket.ts
  - client/src/services/BinaryWebSocketProtocol.ts
  - client/src/services/binaryProtocol/frameTypes.ts
  - client/src/services/binaryProtocol/agentMessages.ts
  - client/src/services/binaryProtocol/backpressure.ts
  - client/src/services/binaryProtocol/ssspVoice.ts
  - client/src/services/livenessCanary.ts
  - client/src/services/nostrAuthService.ts
  - client/src/types/binaryProtocol.ts
  - client/src/features/graph/hooks/useGraphEventHandlers.ts
  - client/src/app/AppInitializer.tsx
  - client/src/store/transientBeamStore.ts
  - xr-client/rust/src/binary_protocol.rs
  - docs/IDENTIFIER-taxonomy.md
  - client/src/utils/validation.ts
  - src/handlers/socket_flow_handler/http_handler.rs
  - src/settings/api/settings_routes.rs
verified_commit: bed6b617d
---
## VC-32.1 Connect + NIP-98 WS authenticate handshake
```mermaid
sequenceDiagram
    autonumber
    participant C as store<br/>websocket/index.ts:151
    participant WS as WebSocket
    participant AH as sendAuthOnConnect<br/>connectionManager.ts:372
    participant NA as nostrAuth<br/>services/nostrAuthService.ts:244
    participant S as Server /wss<br/>http_handler.rs:139-150

    C->>WS: new WebSocket(state.url) index.ts:162
    WS->>S: HTTP Upgrade /wss (NIP-98 Bearer or query token)
    S-->>WS: 101 Switching Protocols
    WS->>C: onopen index.ts:165
    C->>C: webSocketRegistry.register(graph,url,socket) index.ts:171
    C->>AH: sendAuthOnConnect(socket,url) index.ts:178
    alt nostrAuth.isDevMode() connectionManager.ts:375
        AH->>S: authenticate token=dev-session-token pubkey ephemeral connectionManager.ts:377-382
    else NIP-98 signed request
        AH->>NA: signRequest(httpUrl,GET) connectionManager.ts:387
        NA-->>AH: eventToken (NIP-98 kind 27235 event)
        AH->>S: authenticate event=eventToken connectionManager.ts:388
    else no current user (nostrAuth.getCurrentUser() is null)
        Note over AH: no authenticate message sent connectionManager.ts:373-394
    end
    Note over S: DIVERGENCE. BASELINE-architecture.md:217 and PROTOCOL-registry.md:198 record that a query token param is still accepted on /wss in release alongside the Authorization header, contradicting legacy ADR-011 (http_handler.rs:145-150). Client code above only ever sends the header-equivalent authenticate event, never the query form.
    opt currentFilter present index.ts:180
        C->>C: sendMessage(filter_update, ...) index.ts:182-190
    end
    C->>C: initializeBatchQueue(get) index.ts:193
    C->>C: setupFilterSubscription(get) index.ts:194
    C->>C: notifyConnectionStatusHandlers(true) index.ts:195
    C->>C: startHeartbeat(get) index.ts:196
    C->>C: processMessageQueue(get,set) index.ts:197
```
## VC-32.2 Reconnect backoff and heartbeat / binary-silence watchdog
```mermaid
sequenceDiagram
    autonumber
    participant WS as WebSocket
    participant C as store<br/>websocket/index.ts:213
    participant HB as heartbeat<br/>connectionManager.ts:226

    WS-->>C: onclose(event) index.ts:213
    alt event.code in 1000,1001 and event.wasClean index.ts:232-233
        C->>C: updateConnectionState(disconnected) index.ts:239
    else abnormal closure
        C->>C: updateConnectionState(reconnecting,event.reason) index.ts:236
        C->>C: attemptReconnect(get,set,updateConnectionState) connectionManager.ts:334
        loop while reconnectAttempts less than maxReconnectAttempts(10) connectionManager.ts:345
            Note over C: delay = min(1000 times 2 pow attempts + jitter(0-500ms), MAX_RECONNECT_DELAY=30000ms) connectionManager.ts:348-350
            C->>WS: setTimeout(get().connect, delay) connectionManager.ts:358
        end
        Note over C: reconnectAttempts exhausted -> updateConnectionState(failed) connectionManager.ts:365-366
    end
    Note over HB: HEARTBEAT_INTERVAL_MS=30000, HEARTBEAT_TIMEOUT_MS=45000, BINARY_SILENCE_PROBE_MS=60000 connectionManager.ts:25-26,216
    loop every 30000ms after startHeartbeat(get) connectionManager.ts:226-236
        HB->>WS: socket.send(ping) connectionManager.ts:271
        WS-->>HB: pong -> handleHeartbeatResponse() binaryFrameDispatcher.ts:123-126,connectionManager.ts:311
        opt no pong within 45000ms
            HB->>WS: socket.close(4000, Heartbeat timeout) connectionManager.ts:328
        end
        opt Date.now() - lastInboundBinaryAt greater than 60000ms connectionManager.ts:279
            alt unansweredProbes greater or equal 2 connectionManager.ts:280
                HB->>WS: recycle connection via handleHeartbeatTimeout connectionManager.ts:283
            else
                HB->>WS: send subscribe_position_updates interval=200 binary=true connectionManager.ts:288-291
                HB->>WS: send request_full_snapshot connectionManager.ts:299
            end
        end
    end
    Note over C: noteInboundBinary() resets lastInboundBinaryAt and unansweredProbes on every inbound binary frame connectionManager.ts:221-224
    Note over C: INVARIANT. binary silence alone never forces reconnect - only unanswered probes do (connectionManager.ts:208-215)
```
## VC-32.3 Binary frame demux: onmessage tag dispatch and single-flight discipline
```mermaid
sequenceDiagram
    autonumber
    participant WS as WebSocket
    participant MH as createMessageHandler<br/>binaryFrameDispatcher.ts:97
    participant VB as validateBinaryData<br/>binaryProtocol.ts:187
    participant SF as BinaryFrameDispatcher<br/>binaryFrameDispatcher.ts:43
    participant PB as processBinaryData<br/>binaryProtocol.ts:464

    WS->>MH: onmessage(event) binaryFrameDispatcher.ts:120
    alt event.data equals pong
        MH->>MH: handleHeartbeatResponse() connectionManager.ts:311
    else event.data instanceof Blob
        MH->>MH: noteInboundBinary() binaryFrameDispatcher.ts:133
        MH->>VB: arrayBuffer().then(validateBinaryData) binaryFrameDispatcher.ts:136-141
    else event.data instanceof ArrayBuffer
        MH->>MH: noteInboundBinary() binaryFrameDispatcher.ts:152
        MH->>VB: validateBinaryData(event.data) binaryFrameDispatcher.ts:154
    else text frame
        MH->>MH: JSON.parse then handleTextMessage(message) binaryFrameDispatcher.ts:169-176
    end
    VB->>VB: read firstByte = view.getUint8(0) binaryProtocol.ts:197
    alt firstByte is 3(V3), 5(V5) or 0x23(AGENT_ACTION) binaryProtocol.ts:201
        VB-->>SF: dispatcher.handle(buffer) binaryFrameDispatcher.ts:138,155
    else unknown lead byte
        VB-->>MH: reject, log warn, drop frame binaryProtocol.ts:202-206
    end
    alt inFlight already processing binaryFrameDispatcher.ts:52
        SF->>SF: pendingLatest = buffer, dropCount++ (newest-wins) binaryFrameDispatcher.ts:53-60
    else idle
        SF->>PB: processBinaryData(buffer,get,set) binaryFrameDispatcher.ts:64
    end
    PB->>PB: firstByte switch: PROTOCOL_V3/V5 only -> handleLegacyBinaryData binaryProtocol.ts:476-480
    Note over PB: RESOLVED ADR-2078: PROTOCOL_V2 removed from the accept list. The server<br/>rejects V2 outright (src/utils/binary_protocol.rs:565) so a 0x02 lead byte can<br/>only be a stale sender or a mis-routed payload - it is no longer routed here
    PB->>PB: firstByte MessageType.AGENT_ACTION(0x23) -> handleAgentActionTagged binaryProtocol.ts:481-484
    PB->>PB: else parseHeader(data) then switch(header.type) binaryProtocol.ts:487-513
    PB-->>SF: promise settles -> finally drains pendingLatest via queueMicrotask binaryFrameDispatcher.ts:68-76
    Note over PB: ADR-2019 tag byte is registry-governed: byte 0 selects the codec, unknown tags rejected not reinterpreted (docs/PROTOCOL-registry.md tag registry, binary_protocol.rs:521)
```
## VC-32.4 Outbound framed-message header (client-originated only)
```mermaid
classDiagram
    class FramedMessageHeader {
      +uint8 type  offset0
      +uint8 version  offset1
      +uint32 payloadLength  offset2 littleEndian
      +uint8 graphTypeFlag  offset6 GRAPH_UPDATE only
    }
    class MessageType {
      GRAPH_UPDATE 0x01
      VOICE_DATA 0x02
      POSITION_UPDATE 0x10
      AGENT_POSITIONS 0x11
      VELOCITY_UPDATE 0x12
      AGENT_STATE_FULL 0x20
      AGENT_STATE_DELTA 0x21
      AGENT_HEALTH 0x22
      AGENT_ACTION 0x23
      CONTROL_BITS 0x30
      SSSP_DATA 0x31
      HANDSHAKE 0x32
      HEARTBEAT 0x33
      BROADCAST_ACK 0x34
      VOICE_CHUNK 0x40
      SYNC_UPDATE 0x50
      ERROR 0xFF
    }
    class GraphTypeFlag {
      KNOWLEDGE_GRAPH 0x00
      ONTOLOGY 0x01
    }
    class ControlFlags {
      PAUSE_UPDATES bit0
      HIGH_FREQUENCY bit1
      LOW_BANDWIDTH bit2
      VOICE_ENABLED bit3
      DEBUG_MODE bit4
      FORCE_FULL_UPDATE bit5
      USER_INTERACTING bit6
      BACKGROUND_MODE bit7
    }
    FramedMessageHeader --> MessageType : selects codec
    FramedMessageHeader --> GraphTypeFlag : graphTypeFlag field
    note for FramedMessageHeader "createMessage/parseHeader BinaryWebSocketProtocol.ts:73-114. Header size<br/>MESSAGE_HEADER_SIZE=6, GRAPH_UPDATE_HEADER_SIZE=7 (frameTypes.ts:184-185). Used<br/>only for client-to-server encode calls: encodePositionUpdates, encodeAgentState,<br/>encodeSSSPData, encodeControlBits, encodeVoiceChunk, createBroadcastAck,<br/>encodeAgentAction (BinaryWebSocketProtocol.ts:129-290)."
    note for MessageType "DIVERGENCE. BinaryWebSocketProtocol.ts:81 comment labels this six-byte layout the<br/>V4 header, but line 83 writes PROTOCOL_VERSION which equals PROTOCOL_V3=3<br/>(frameTypes.ts:15) into the version byte - PROTOCOL_V4=4 (frameTypes.ts:14) is<br/>never actually placed on the wire by this path. frameTypes.ts:7-13 itself<br/>documents that PROTOCOL_V4 here means only this framed-header version, NOT the<br/>position/agent wire format the server ships."
    note for FramedMessageHeader "DOC-DRIFT. docs/PROTOCOL-registry.md frame tag registry lists only<br/>0x03,0x05,0x23,0x43,0x44 as live server-to-client tags; this client-only outbound<br/>envelope and its MessageType space (0x01,0x02,0x10-0x54) do not appear in that<br/>registry at all."
```
## VC-32.5 Server-to-client position wire records: V3, V4 delta, V5 envelope (V2 declined)
```mermaid
classDiagram
    class V3NodeRecord {
      +uint32 id  offset0  bits0to25 id, bits26to31 flags
      +float32x3 position  offset4
      +float32x3 velocity  offset16
      +float32 sssp_distance  offset28  default Infinity
      +int32 sssp_parent  offset32  default -1
      +uint32 cluster_id  offset36
      +float32 anomaly_score  offset40
      +uint32 community_id  offset44
      +float32 centrality  offset48
      52 bytes per record, leading tag byte 0x03
    }
    class V2NodeRecord {
      +uint32 id  offset0
      +float32x3 position  offset4
      +float32x3 velocity  offset16
      +float32 sssp_distance  offset28
      +int32 sssp_parent  offset32
      36 bytes per record, no analytics fields
    }
    class V5Envelope {
      +uint8 version  offset0  equals 5
      +uint64 broadcast_seq  offset1  8 bytes littleEndian
      +V3NodeRecord[] body  offset9 onward
    }
    class V4DeltaFrame {
      +uint8 version  offset0  equals 4
      +uint8 frame_number  offset1
      +uint16 num_changed  offset2
      +DeltaItem[] items  offset4 onward, 20 bytes each
    }
    class DeltaItem {
      +uint32 node_id  offset0
      +uint8 change_flags  offset4  bit0 pos, bit1 vel
      +int16 dx_dy_dz  offset8,10,12  divide by DELTA_SCALE_FACTOR 100
      +int16 dvx_dvy_dvz  offset14,16,18  divide by 100
    }
    V5Envelope --> V3NodeRecord : wraps
    V4DeltaFrame --> DeltaItem : contains
    note for V3NodeRecord "CROSS-CHECK OK. client/src/types/binaryProtocol.ts:95-105 offsets match<br/>xr-client/rust/src/binary_protocol.rs:8-11,681-700 exactly: id, pos, vel,<br/>sssp_distance, sssp_parent, cluster_id, anomaly, community_id, centrality. Godot<br/>NODE_RECORD_BYTES=52 (:28) equals client BINARY_NODE_SIZE_V3=52<br/>(binaryProtocol.ts:90). No DIVERGENCE."
    note for V5Envelope "types/binaryProtocol.ts:404-425 parseV5Nodes reads seqLow at offset1 and seqHigh<br/>at offset5, reconstructs a V3 buffer at offset9. Matches Godot PROTOCOL_V5=0x05<br/>(:25) and V5_SEQ_BYTES=8 (:26), and docs/PROTOCOL-registry.md 0x05 row<br/>(binary_protocol.rs:513)."
    note for V4DeltaFrame "DOC-DRIFT. docs/PROTOCOL-registry.md frame tag registry (0x03,0x05,0x23,0x43,0x44)<br/>has no 0x04 row; this delta codec exists only in client decode<br/>(types/binaryProtocol.ts:56-65,327-397) with no confirmed live sender in the<br/>registry. PROTOCOL_V4 here (delta node encoding) is a DIFFERENT meaning from the<br/>same-named PROTOCOL_V4 in services/binaryProtocol/frameTypes.ts:14 (framed-header<br/>version) - see VC-32.4."
```
## VC-32.6 Wire node-id: type flag bits over the 26-bit id
```mermaid
classDiagram
    class WireNodeId {
      +uint32 raw  the full id as sent on the wire
      bit31 AGENT_NODE_FLAG 0x80000000
      bit30 KNOWLEDGE_NODE_FLAG 0x40000000
      bits26to28 ONTOLOGY_TYPE_MASK 0x1C000000
      bits0to25 NODE_ID_MASK 0x03FFFFFF  actual id, 0 to 67108863
    }
    class OntologySubtype {
      ONTOLOGY_CLASS_FLAG 0x04000000
      ONTOLOGY_INDIVIDUAL_FLAG 0x08000000
      ONTOLOGY_PROPERTY_FLAG 0x10000000
    }
    WireNodeId --> OntologySubtype : bits26to28 when ontology
    note for WireNodeId "INVARIANT, no DIVERGENCE. Client client/src/types/binaryProtocol.ts:108-118, Godot<br/>decoder xr-client/rust/src/binary_protocol.rs:37-43, and<br/>docs/IDENTIFIER-taxonomy.md:105-110 all agree exactly on mask and flag hex values.<br/>getActualNodeId(nodeId) is nodeId AND NODE_ID_MASK<br/>(types/binaryProtocol.ts:144-146)."
    note for WireNodeId "docs/IDENTIFIER-taxonomy.md:112-117: ids are sequential u32 from a NEXT_NODE_ID<br/>atomic counter; encoders debug_assert id less-or-equal NODE_ID_MASK before OR-ing<br/>the flag, but in release the assert compiles out and an over-range id is silently<br/>truncated to its low 26 bits (binary_protocol.rs:118-136)."
```
## VC-32.7 getNodeType(): flag precedence decision order
```mermaid
flowchart TD
    A["nodeId: uint32"] --> B{"nodeId AND AGENT_NODE_FLAG != 0?<br/>binaryProtocol.ts:130"}
    B -- yes --> AG["NodeType.Agent"]
    B -- no --> C{"nodeId AND KNOWLEDGE_NODE_FLAG != 0?<br/>binaryProtocol.ts:132"}
    C -- yes --> KN["NodeType.Knowledge"]
    C -- no --> D{"nodeId AND ONTOLOGY_TYPE_MASK == CLASS_FLAG?<br/>binaryProtocol.ts:134"}
    D -- yes --> OC["NodeType.OntologyClass"]
    D -- no --> E{"nodeId AND ONTOLOGY_TYPE_MASK == INDIVIDUAL_FLAG?<br/>binaryProtocol.ts:136"}
    E -- yes --> OI["NodeType.OntologyIndividual"]
    E -- no --> F{"nodeId AND ONTOLOGY_TYPE_MASK == PROPERTY_FLAG?<br/>binaryProtocol.ts:138"}
    F -- yes --> OP["NodeType.OntologyProperty"]
    F -- no --> UN["NodeType.Unknown  binaryProtocol.ts:141"]
    AG --> G["updateNodeTypeMapFromParsed caches actualId to type<br/>evicting oldest past MAX_NODE_TYPE_ENTRIES=65536<br/>websocket/binaryProtocol.ts:155-179"]
    KN --> G
    OC --> G
    OI --> G
    OP --> G
```
## VC-32.8 Live position frame processing: parse, analytics ingest, ack
```mermaid
sequenceDiagram
    autonumber
    participant PB as processBinaryData<br/>binaryProtocol.ts:464
    participant FR as parseBinaryFrameData<br/>types/binaryProtocol.ts:434
    participant AN as nodeAnalyticsStore<br/>features/analytics/store/nodeAnalyticsStore
    participant GDM as graphDataManager<br/>features/graph/managers/graphDataManager
    participant SRV as Server /wss

    PB->>PB: handleLegacyBinaryData(data,get,set) binaryProtocol.ts:361
    PB->>FR: parseBinaryFrameData(data) binaryProtocol.ts:370
    FR-->>PB: ParsedBinaryFrame type=full|delta, nodes, broadcastSequence
    opt frame.type equals full binaryProtocol.ts:381
        alt protoByte is PROTOCOL_V3 or PROTOCOL_V5 binaryProtocol.ts:383
            PB->>AN: nodeAnalyticsStore.ingest(parsedNodes) binaryProtocol.ts:384
        else V4 delta frame
            Note over PB: V4 delta omits cluster_id/anomaly/community offsets 36/40/44 - must NOT overwrite live analytics binaryProtocol.ts:378-380
        end
    end
    PB->>PB: updateNodeTypeMapFromParsed(parsedNodes,set) binaryProtocol.ts:388
    opt hasBotsData: any node isAgentNode() binaryProtocol.ts:390
        PB->>PB: emit(bots-position-update, data) binaryProtocol.ts:392-393
    end
    PB->>GDM: graphDataManager.updateNodePositions(data) binaryProtocol.ts:407
    PB->>PB: positionUpdateSequence++ binaryProtocol.ts:415
    opt positionUpdateSequence minus lastAckSentSequence greater-or-equal ACK_BATCH_SIZE(10) binaryProtocol.ts:38,417
        PB->>SRV: sendPositionAck -> BroadcastAck message (createBroadcastAck) binaryProtocol.ts:131-151,418
    end
    Note over PB: framed-header path handlePositionUpdate (MessageType.POSITION_UPDATE/AGENT_POSITIONS, binaryProtocol.ts:307-359,502-504) mirrors this same analytics-cache/ack flow but is reached only via the 6-byte outbound envelope of VC-32.4, not the bare V3/V5 tag the live server actually sends.
```
## VC-32.9 Position subscription: idempotent one-shot subscribe, filter sync, shrink-guard
```mermaid
sequenceDiagram
    autonumber
    participant AI as AppInitializer<br/>app/AppInitializer.tsx:269
    participant C as store<br/>websocket/index.ts
    participant FS as filterSync<br/>websocket/filterSync.ts:65
    participant TH as textMessageHandler<br/>websocket/textMessageHandler.ts:131
    participant SRV as Server /wss

    C->>AI: onConnectionStatusChange(connected=true) AppInitializer.tsx:269,282
    alt websocketService.isReady() AppInitializer.tsx:284
        alt not hasSubscribedToPositions AppInitializer.tsx:290
            AI->>SRV: subscribe_position_updates binary=true interval=updateRate(default 60) AppInitializer.tsx:296-299
        else already subscribed on this connection
            Note over AI: idempotency guard skips duplicate subscribe AppInitializer.tsx:65,290
        end
    else not fully established yet
        AI->>C: onMessage waiting for connection_established AppInitializer.tsx:313
        C-->>AI: message.type equals connection_established AppInitializer.tsx:314
        alt not hasSubscribedToPositions AppInitializer.tsx:319
            AI->>SRV: subscribe_position_updates binary=true interval=updateRate2 AppInitializer.tsx:326-329
        end
    end
    Note over C: !connected clears hasSubscribedToPositions so the next connection resubscribes exactly once AppInitializer.tsx:277-278
    C->>FS: setupFilterSubscription(get) index.ts:194
    FS->>FS: subscribe to nodeFilter.* settings paths filterSync.ts:71-89
    loop on every nodeFilter change while isConnected filterSync.ts:91-118
        FS->>SRV: sendFilterUpdate(filter) -> filter_update message index.ts:362-385
        FS->>FS: expectFilterResponse() arms 15000ms acceptance window index.ts:372,filterSync.ts:29-34
    end
    SRV-->>TH: initialGraphLoad nodes,edges textMessageHandler.ts:110
    alt nodes.length less than existingNodeCount and isFilterResponseExpected() textMessageHandler.ts:138-149
        TH->>TH: accept filtered load, clearFilterResponseExpectation() textMessageHandler.ts:145-150
    else smaller unsolicited payload
        TH->>TH: skip setGraphData, positions arrive via binary stream textMessageHandler.ts:151-162
    end
```
## VC-32.10 Drag / pin send path: server-authoritative pin over JSON control frames
```mermaid
sequenceDiagram
    autonumber
    participant U as PointerEvent
    participant EH as useGraphEventHandlers<br/>features/graph/hooks/useGraphEventHandlers.ts:78
    participant WSS as webSocketService<br/>graphDataManager.webSocketService
    participant SRV as Server /wss

    U->>EH: handlePointerDown(event) useGraphEventHandlers.ts:78
    EH->>EH: dragDataRef.current = pointerDown,nodeId,startNodePos3D useGraphEventHandlers.ts:95-104
    EH->>EH: startInteraction(node.id) useGraphEventHandlers.ts:120
    U->>EH: handlePointerMove(event) useGraphEventHandlers.ts:127
    alt distance greater than DRAG_THRESHOLD(5) useGraphEventHandlers.ts:136
        EH->>EH: drag.isDragging = true useGraphEventHandlers.ts:137
        alt graphDataManager.webSocketService.isReady() useGraphEventHandlers.ts:149
            EH->>SRV: sendMessage(nodeDragStart, nodeId, startPosition) useGraphEventHandlers.ts:150-157
        else not connected or not server-ready
            Note over EH: send skipped, caught by try/catch - drag state machine still proceeds useGraphEventHandlers.ts:148,159-161
        end
    end
    opt drag.isDragging useGraphEventHandlers.ts:170
        EH->>EH: updateNodePosition(nodeId,intersection) local optimistic move useGraphEventHandlers.ts:207-211
        EH->>WSS: throttledWebSocketUpdate(nodeId,position) throttle 100ms useGraphEventHandlers.ts:17,54-76,214-218
        alt shouldSendPositionUpdates() and webSocketService.isReady() useGraphEventHandlers.ts:57-59
            WSS->>SRV: sendMessage(nodeDragUpdate, nodeId, position, timestamp) useGraphEventHandlers.ts:64-68
        end
    end
    U->>EH: handlePointerUp(event) useGraphEventHandlers.ts:223
    alt drag.isDragging useGraphEventHandlers.ts:241
        EH->>EH: flushPositionUpdates() useGraphEventHandlers.ts:251
        alt webSocketService.isReady() useGraphEventHandlers.ts:254
            EH->>SRV: sendMessage(nodeDragEnd, nodeId) useGraphEventHandlers.ts:255-257
        end
    else click, not a drag
        EH->>EH: onNodeSelect(nodeId) useGraphEventHandlers.ts:265-268
    end
    Note over SRV: ADR-03 D7 (legacy). pin state is server-authoritative - nodeDragStart pins, nodeDragEnd unpins and runs final settle. the old binary sendNodePositionUpdates second-frame path was removed 2026-06-03 useGraphEventHandlers.ts:60-63,142-144,246-247
```
## VC-32.11 Agent frame 0x23 (AGENT_ACTION): decode and beam dispatch
```mermaid
sequenceDiagram
    autonumber
    participant PB as processBinaryData<br/>binaryProtocol.ts:464
    participant DEC as decodeAgentActions<br/>binaryProtocol/agentMessages.ts:183
    participant BEAM as pushTransientBeams<br/>store/transientBeamStore

    PB->>PB: firstByte equals MessageType.AGENT_ACTION 0x23 binaryProtocol.ts:35,481
    PB->>PB: handleAgentActionTagged(data) binaryProtocol.ts:442
    Note over PB: shipping wire is a bare tag, not a 6-byte V4 header - [0x23][u16 count]([u16 len][event])... binaryProtocol.ts:436-441
    alt data.byteLength greater-or-equal 18 binaryProtocol.ts:443
        PB->>DEC: decodeAgentActions(data.slice(1)) binaryProtocol.ts:444
    else too small
        PB->>PB: actions = empty array binaryProtocol.ts:443-445
    end
    DEC->>DEC: eventCount = view.getUint16(0) agentMessages.ts:192
    loop for each event, offset advances by 2 plus eventLen agentMessages.ts:195-215
        DEC->>DEC: eventLen = view.getUint16(offset) agentMessages.ts:201
        DEC->>DEC: decodeAgentAction(eventPayload) sourceAgentId,targetNodeId,actionType,timestamp,durationMs agentMessages.ts:153-181,210
    end
    DEC-->>PB: AgentActionEvent[] binaryProtocol.ts:449
    opt actions.length greater than 0 binaryProtocol.ts:450
        PB->>PB: emit(agent-action, actions) live transcript and attention heat binaryProtocol.ts:452
        PB->>BEAM: pushTransientBeams(actions) TransientBeamsLayer sink binaryProtocol.ts:455
    end
    Note over PB: AGENT_ACTION_HEADER_SIZE=15 bytes: sourceAgentId u32@0, targetNodeId u32@4, actionType u8@8, timestamp u32@9, durationMs u16@13, optional payload from @15 frameTypes.ts:174,agentMessages.ts:161-165
```
## VC-32.12 Backpressure: broadcast acknowledgement flow control
```mermaid
classDiagram
    class BroadcastAckPayload {
      +uint32 sequenceId_low  offset0
      +uint32 sequenceId_high  offset4  combine as sequenceId
      +uint32 nodesReceived  offset8
      +uint32 timestamp_low  offset12
      +uint32 timestamp_high  offset16
      20 bytes total, BROADCAST_ACK_PAYLOAD_SIZE
    }
    class AckPolicy {
      ACK_BATCH_SIZE 10  ack sent every 10 processed position frames
      MessageType.BROADCAST_ACK 0x34  message tag when framed via createMessage
    }
    BroadcastAckPayload --> AckPolicy : governed by
    note for BroadcastAckPayload "encodeBroadcastAckPayload/decodeBroadcastAck backpressure.ts:18-69. sequenceId and<br/>timestamp each split into low/high uint32 for littleEndian 8-byte write without<br/>BigInt64 (backpressure.ts:26-29,36-39)."
    note for AckPolicy "sendPositionAck fires when positionUpdateSequence minus lastAckSentSequence is<br/>greater-or-equal ACK_BATCH_SIZE=10<br/>(websocket/binaryProtocol.ts:38,131-151,354-358,415-420). Legacy frames use<br/>frame.broadcastSequence when present, else the client-local counter<br/>(binaryProtocol.ts:416)."
```
## VC-32.13 SSSP data and voice-chunk binary sub-protocols
```mermaid
classDiagram
    class SSSPDataRecord {
      +uint32 nodeId  offset0
      +float32 distance  offset4
      +uint32 parentId  offset8
      +uint16 flags  offset12
      14 bytes per record, SSSP_DATA_SIZE_V2
    }
    class VoiceChunkHeader {
      +uint16 agentId  offset0  max 65535 agents, separate id space from node ids
      +uint16 chunkId  offset2
      +uint8 format  offset4
      +uint16 dataLength  offset5
      7 bytes header, VOICE_HEADER_SIZE, followed by raw audioData
    }
    note for SSSPDataRecord "encodeSSSPPayload/decodeSSSPData ssspVoice.ts:8-47. Wrapped via<br/>createMessage(MessageType.SSSP_DATA 0x31) for outbound use,<br/>BinaryWebSocketProtocol.ts:164-170."
    note for VoiceChunkHeader "encodeVoiceChunkPayload/decodeVoiceChunk ssspVoice.ts:49-91. Comment at<br/>ssspVoice.ts:54-56,82-84 is explicit: voice agentId is uint16, unrelated to the<br/>uint32 flag-bit node/agent id of VC-32.6 - no alignment issue between the two id<br/>spaces. Wrapped via MessageType.VOICE_CHUNK 0x40,<br/>BinaryWebSocketProtocol.ts:185-191."
```
## VC-32.14 livenessCanary: fire-and-forget observed-traffic probe
```mermaid
sequenceDiagram
    autonumber
    participant CALLER as D8 swarm dashboard mount
    participant LC as observeCanary<br/>services/livenessCanary.ts:15
    participant API as unifiedApiClient<br/>services/api/UnifiedApiClient
    participant SRV as POST /api/canary/observe/{id}

    CALLER->>LC: observeCanary(canaryId,evidence) livenessCanary.ts:15
    LC->>API: unifiedApiClient.post(/canary/observe/id, evidence) livenessCanary.ts:17
    API->>SRV: POST /api/canary/observe/{encodeURIComponent(canaryId)}
    alt request succeeds
        SRV-->>LC: 200 OK
        LC->>LC: logger.debug observed canary livenessCanary.ts:18
    else 404 unregistered canary or harness/network down
        SRV-->>LC: error
        LC->>LC: fail-open: logger.debug canary observe skipped, swallow error livenessCanary.ts:19-23
    end
    Note over LC: INVARIANT (ADR-130 Decision 3). fires from THIS observed live event, not a synthetic probe - never throws, never disrupts the mounting UI livenessCanary.ts:1-8,14
```
## VC-32.15 JSON control-frame catalogue
```mermaid
classDiagram
    class ClientToServer {
      authenticate  token/pubkey/ephemeral OR event  connectionManager.ts:377-388
      filter_update  enabled,quality_threshold,authority_threshold,filter_by_quality,filter_by_authority,filter_mode,include_linked_pages  index.ts:303-385
      subscribe_position_updates  binary,interval  AppInitializer.tsx:296-299,connectionManager.ts:288-291
      request_full_snapshot  no payload  connectionManager.ts:299
      nodeDragStart  nodeId,position  useGraphEventHandlers.ts:150-157
      nodeDragUpdate  nodeId,position,timestamp  useGraphEventHandlers.ts:64-68
      nodeDragEnd  nodeId  useGraphEventHandlers.ts:255-257
      ping  raw string, not JSON  connectionManager.ts:271
    }
    class ServerToClient {
      connection_established  sets isServerReady  textMessageHandler.ts:84-89
      error  category:validation|server|protocol|auth|rate_limit, code, retryable, retryAfter  types.ts:14-23,binaryProtocol.ts:211-261
      filter_update_success  data.visible_nodes,data.total_nodes  textMessageHandler.ts:100-108
      initialGraphLoad  nodes,edges  textMessageHandler.ts:110,131-223
      memory_flash  data forwarded to memoryFlash event  textMessageHandler.ts:115-117
      graphUpdated  revision,reason, debounced 750ms refetch  textMessageHandler.ts:28-54,122-126
      pong  raw string, not JSON  binaryFrameDispatcher.ts:123-126
      settingsUpdated  category,updatedBy,timestamp,settings? textMessageHandler.ts:138-190
    }
    ClientToServer --> ServerToClient : request/response over one socket
    note for ClientToServer "sent via store websocket/index.ts sendMessage(type,data) JSON.stringify(type,data)<br/>index.ts:303-332. Queued to messageQueue (MAX_QUEUE_SIZE=100) when not connected,<br/>connectionManager.ts:27,149-168."
    note for ServerToClient "dispatched by handleTextMessage after JSON.parse and validateMessage(type is<br/>nonempty string, length less-or-equal 100)<br/>websocket/binaryFrameDispatcher.ts:82-90,textMessageHandler.ts:74-129."
    note for ServerToClient "RESOLVED ADR-2080 settingsUpdated now HAS a consumer. The server emitted<br/>it from src/settings/api/settings_routes.rs:445,982 while the only client knowledge was<br/>validation.ts:274 case settings_update (snake_case, different shape) - every broadcast fell<br/>through to the default arm and did nothing, so two viewers saw different physics until one<br/>reloaded. nodeFilter applies the supplied settings directly. other categories re-read via<br/>getSectionPaths plus getSettingsByPaths. own-write echo and stale timestamps are dropped.<br/>Contract fixed by vc-core ADR-2047. settings_update is dead on the wire and left in place"
    note for ServerToClient "GRAPH_UPDATED shrink-guard: a smaller initialGraphLoad is discarded unless<br/>isFilterResponseExpected() is true within the 15000ms window armed by<br/>expectFilterResponse() - see VC-32.9.<br/>textMessageHandler.ts:131-163,filterSync.ts:29-38"
```
## VC-32.16 solidWebSocket: JSS resource-notification boundary
```mermaid
sequenceDiagram
    autonumber
    participant C as store<br/>websocket/index.ts:447
    participant SW as connectSolidWebSocket<br/>websocket/solidWebSocket.ts:125
    participant JSS as JSS WebSocket<br/>VITE_JSS_WS_URL

    C->>SW: connectSolid() index.ts:447-449
    alt VITE_JSS_WS_URL not configured solidWebSocket.ts:135
        SW->>SW: logger.warn, return solidWebSocket.ts:136-137
    else
        SW->>JSS: new WebSocket(wsUrl) solidWebSocket.ts:148
        JSS-->>SW: onopen -> isSolidConnected=true, webSocketRegistry.register(solid-store) solidWebSocket.ts:150-156
        JSS-->>SW: onmessage(msg) solidWebSocket.ts:159-162
        alt msg starts with protocol solidWebSocket.ts:65
            SW->>JSS: resubscribe: send(sub url) for every tracked subscription solidWebSocket.ts:70-73
        else msg starts with ack
            SW->>SW: notifySolidSubscribers(type=ack) solidWebSocket.ts:76-79
        else msg starts with pub
            SW->>SW: notifySolidSubscribers(type=pub), emit(solid-resource-changed) solidWebSocket.ts:80-84
        else msg starts with error
            SW->>SW: emit(solid-error) solidWebSocket.ts:85-88
        end
        JSS-->>SW: onclose -> isSolidConnected=false solidWebSocket.ts:170-179
        SW->>SW: attemptSolidReconnect: 1000ms times 2 pow attempts, cap 10 attempts solidWebSocket.ts:94-113
    end
    Note over SW: subscribeSolidResource sends sub url only on first callback for a URL, unsubscribeSolidResource sends unsub when the last callback is removed solidWebSocket.ts:223-276
    Note over JSS: see VC-33 for the JSS/Solid Pod ownership boundary this socket crosses into
```
