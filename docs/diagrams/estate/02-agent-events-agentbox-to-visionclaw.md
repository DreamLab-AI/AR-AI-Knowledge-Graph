---
id: ES-02
title: Agent-event path — agentbox action to rendered beam, plus legacy paths
area: estate
governing:
  - docs/PROTOCOL-registry.md
  - docs/GPU-wire-abi.md
  - agentbox/docs/PROTOCOL-registry.md
adrs: [ADR-2020, ADR-2015, ADR-2083, ADR-2084, ADR-2085, ADR-2088, ADR-2089, ADR-2090, ADR-2091]
sources:
  - agentbox/management-api/utils/agent-event-publisher.js
  - agentbox/management-api/lib/agent-event-auth.js
  - agentbox/management-api/routes/agent-events.js
  - agentbox/management-api/utils/agent-event-ws-subscriber.js
  - src/agent_events/ingest.rs
  - src/agent_events/schema.rs
  - src/agent_events/hub.rs
  - src/agent_events/provenance.rs
  - src/agent_events/mod.rs
  - src/actors/agent_beam_actor.rs
  - src/actors/client_coordinator_actor.rs
  - src/utils/binary_protocol.rs
  - src/utils/unified_gpu_compute/construction.rs
  - src/utils/unified_gpu_compute/memory.rs
  - src/utils/unified_gpu_compute/execution.rs
  - client/src/features/visualisation/components/TransientBeamsLayer.tsx
  - agentbox/management-api/middleware/auth.js
  - agentbox/management-api/routes/briefing.js
  - agentbox/management-api/routes/status.js
  - agentbox/management-api/routes/tasks.js
  - agentbox/management-api/server.js
  - client/src/store/transientBeamStore.ts
  - client/src/store/websocket/binaryProtocol.ts
  - src/actors/graph_service_supervisor.rs
  - src/app_state.rs
  - src/handlers/mcp_relay_handler.rs
  - src/handlers/multi_mcp_websocket_handler.rs
  - src/main.rs
  - src/services/bots_client.rs
  - src/services/management_api_client.rs
  - src/services/mcp_relay_manager.rs
  - src/services/multi_mcp_agent_discovery.rs
  - src/utils/mcp_tcp_client.rs
verified_commit: 7a20db228
---
## ES-02.1 Producer — POST /v1/agent-events/emit, NIP-98 gate, local publish
```mermaid
sequenceDiagram
    autonumber
    participant CF as claude-flow hook<br/>agentbox/management-api/routes/agent-events.js:496
    participant RT as agentEventsRoutes<br/>agentbox/management-api/routes/agent-events.js:246
    participant AUTH as verifyAgentEventRequest<br/>agentbox/management-api/lib/agent-event-auth.js:46
    participant PUB as agentEventPublisher<br/>agentbox/management-api/utils/agent-event-publisher.js:22
    participant WS as wsConnections<br/>agentbox/management-api/routes/agent-events.js:22

    rect rgb(230,235,245)
    Note over CF,RT: TRUST BOUNDARY: agentbox management-api process
    CF->>RT: POST /v1/agent-events/emit {source_agent_id,target_node_id,action_type}
    RT->>AUTH: verifyAgentEventRequest(request)
    alt AGENTBOX_AGENT_EVENT_AUTH=off (default)
        AUTH-->>RT: {ok:true, did:null}
    else policy=nip98, header present, verifyNip98 valid
        AUTH-->>RT: {ok:true, did:"did:nostr:<pubkey>"}
    else policy=nip98, header missing
        AUTH-->>RT: {ok:false, status:401, error:"NIP-98 Authorization header required"}
        RT-->>CF: 401 {success:false, error, ...taxonomy.tagFailure}
    else policy=nip98, verify throws/invalid
        AUTH-->>RT: {ok:false, status:401, error:"invalid NIP-98 auth"}
        RT-->>CF: 401 {success:false, error, ...taxonomy.tagFailure}
    else unknown policy value
        AUTH-->>RT: {ok:false, status:500, error:"unknown AGENTBOX_AGENT_EVENT_AUTH policy"}
        RT-->>CF: 500 {success:false, error}
    end
    opt claimed source_urn present
        RT->>RT: reconcileSourceUrn(claimed, auth.did)<br/>agentbox/management-api/lib/agent-event-auth.js:89
        alt claimed != verified did
            RT-->>CF: 403 FM-1.2 identity-attribution failure
        end
    end
    RT->>PUB: emitAgentAction(emitPayload)
    PUB->>PUB: classify failure_mode (top-level wins, else metadata.failure_mode promoted)<br/>agent-event-publisher.js:107-132
    PUB->>PUB: eventBuffer.push (max 1000, oldest shifted)<br/>agent-event-publisher.js:146-149
    PUB->>PUB: subscribers.forEach(callback) + emit('agent_action'/'event')
    PUB-->>RT: fullEvent {id, version:3, ...}
    RT-->>CF: 200 {success:true, event_id, broadcast_count: wsConnections.size}
    PUB->>WS: subscribed callback: createMcpNotification(event) -> JSON.stringify
    WS->>WS: socket.binaryMode ? createBinaryPayload(event) : jsonMessage<br/>agent-event-publisher.js:256
    Note over WS: INVARIANT: binary header 19 bytes =<br/>version(1)+type(1)+source(4)+target(4)+action(1)+ts(4)+dur(2)+len(2), version=0x02 type=0x23
    end
```

## ES-02.2 Cross-container forward — agentbox WS subscriber to VisionClaw ingest, auth + envelope validation
```mermaid
sequenceDiagram
    autonumber
    participant PUB as agentEventPublisher<br/>agentbox/management-api/utils/agent-event-publisher.js:22
    participant SUB as AgentEventWsSubscriber<br/>agentbox/management-api/utils/agent-event-ws-subscriber.js:30
    participant VCW as agent_events_ws<br/>src/agent_events/ingest.rs:291
    participant NS as NostrService.get_session<br/>src/agent_events/ingest.rs:268
    participant PF as process_frame<br/>src/agent_events/ingest.rs:94
    participant PROV as provenance::record<br/>src/agent_events/provenance.rs:96
    participant HUB as AGENT_EVENT_HUB<br/>src/agent_events/hub.rs:24

    rect rgb(245,230,230)
    Note over SUB,VCW: TRUST BOUNDARY: agentbox container -> VisionClaw container, subprotocol vc-agent-events.v1
    PUB->>SUB: subscribe callback, direction != 'inbound'<br/>agent-event-ws-subscriber.js:46-49
    SUB->>SUB: new WebSocket(url,[SUBPROTOCOL])<br/>agent-event-ws-subscriber.js:94
    SUB->>VCW: GET /wss/agent-events upgrade, Authorization: Bearer <token> or ?token=
    VCW->>NS: authenticate(req, app_state)<br/>src/agent_events/ingest.rs:251
    alt token present, session valid
        NS-->>VCW: Some(user.pubkey)
    else token invalid/expired, ALLOW_INSECURE_DEFAULTS set (debug/dev-auth only)
        NS-->>VCW: None (warn, dev bypass)
    else token invalid/expired, insecure defaults off
        VCW-->>SUB: 401 Unauthorized "Invalid or expired authentication token"
    else no token, insecure defaults off
        VCW-->>SUB: 401 Unauthorized "Authentication required for the agent-events socket"
    end
    VCW->>VCW: WsResponseBuilder.protocols(&[SUBPROTOCOL]).start()<br/>src/agent_events/ingest.rs:305-311
    VCW-->>SUB: 101 Switching Protocols
    loop reconnect backoff on close/error
        SUB->>SUB: delay = min(30000, 1000*2^attempt)<br/>agent-event-ws-subscriber.js:27-28,133-136
    end
    PUB->>SUB: _forwardOutbound(event) -> ws.send(JSON.stringify(event))<br/>agent-event-ws-subscriber.js:144-151
    SUB->>VCW: Text frame: notifications/agent_action JSON-RPC envelope
    VCW->>PF: process_frame(&text)
    alt jsonrpc=="2.0" && method matches && kind=="agent_action" && event.version>=3
        PF->>PROV: record(&event)
        PROV-->>PF: IngestProvenance{status,source_crossing,target_crossing}
        PF->>HUB: hub::publish(event)<br/>src/agent_events/hub.rs:32
        HUB-->>PF: receivers: usize (0 until AgentBeamActor subscribes)
        PF-->>VCW: IngestOutcome::Published{action,attributed,provenance_status,crossings_recorded,ctc_present,receivers}
        opt ctc_present
            VCW->>VCW: fire_ctc_canary CANARY-VC-REC3-CTC (one-shot, AtomicBool)<br/>src/agent_events/ingest.rs:167-184
        end
    else valid JSON-RPC, wrong method or version<3
        PF-->>VCW: IngestOutcome::NonCanonical
        VCW-->>SUB: ctx.text {"error":"non_canonical_envelope"}
    else not parseable JSON
        PF-->>VCW: IngestOutcome::Malformed
        VCW-->>SUB: ctx.text {"error":"malformed_json"}
    end
    Note over HUB: INVARIANT: HUB_CAPACITY=256 tokio broadcast channel, a lagged receiver observes<br/>RecvError::Lagged(skipped) and resyncs on next frame drop-oldest no crash
    Note over VCW: RESOLVED ADR-2084: src/agent_events/mod.rs and ingest.rs now state that the beam render<br/>actor is SHIPPED and subscribes to this hub, and that the attractive gluon transient edge is a<br/>separate deferred sub-feature (agent_beam_actor.rs:327, packed-CSR reason). The stale<br/>beam-plus-gluon Phase 2b framing is gone
    end
```

## ES-02.3 Canonical wire envelope — AgentActionNotification / AgentActionEnvelope
```mermaid
classDiagram
    class AgentActionNotification {
      +String jsonrpc
      +String method
      +AgentActionParams params
      +is_canonical() bool
    }
    class AgentActionParams {
      +String kind
      +AgentActionEnvelope event
      +u8 message_type
      +u8 protocol_version
      +String timestamp
    }
    class AgentActionEnvelope {
      +u8 version
      +u64 id
      +u32 source_agent_id
      +u32 target_node_id
      +u8 action_type
      +String action_type_name
      +u64 timestamp
      +u32 duration_ms
      +Option~String~ source_urn
      +Option~String~ target_urn
      +Option~String~ pubkey
      +Option~u64~ token_count
      +Option~String~ handoff_id
      +Option~String~ verification
      +Option~String~ intent
      +Value metadata
      +action_type() AgentActionType
      +has_ctc() bool
      +declared_intent() Option~str~
      +to_binary_event() AgentActionEvent
    }
    AgentActionNotification --> AgentActionParams : params
    AgentActionParams --> AgentActionEnvelope : event
    note for AgentActionEnvelope "src/agent_events/schema.rs:61-147 - is_canonical requires jsonrpc==2.0,<br/>method==notifications/agent_action, kind==agent_action, version>=3<br/>(src/agent_events/schema.rs:36-41)"
    note for AgentActionEnvelope "token_count aliases token_burden, handoff_id aliases handoff_count, verification aliases<br/>verification_outcome (serde alias, both spellings parse) -<br/>src/agent_events/schema.rs:116-121"
```

## ES-02.4 Hub fan-out and beam coalescing — backpressure to ClientCoordinatorActor
```mermaid
sequenceDiagram
    autonumber
    participant HUB as AGENT_EVENT_HUB<br/>src/agent_events/hub.rs:24
    participant BA as AgentBeamActor<br/>src/actors/agent_beam_actor.rs:182
    participant BC as BeamCoalescer<br/>src/actors/agent_beam_actor.rs:119
    participant CC as ClientCoordinatorActor<br/>src/actors/client_coordinator_actor.rs:474

    BA->>HUB: agent_events::hub::subscribe()<br/>src/actors/agent_beam_actor.rs:199
    loop forwarding task, empty backlog blocks on rx.recv()
        HUB->>BA: rx.recv().await
        alt Ok(envelope)
            BA->>BA: project_action(&envelope): to_binary_event + stamp_agent_flag 0x80000000<br/>src/actors/agent_beam_actor.rs:175-179
            BA->>BC: push(event) [drop-oldest past MAX_PENDING_ACTIONS=256]
        else RecvError::Lagged(skipped)
            BA->>BA: warn hub lagged, skipped frame(s) - resyncing, continue loop
        else RecvError::Closed
            break hub sender dropped, forwarding task exits
                BA->>BA: info hub closed - forwarding task exiting
            end
        end
    end
    loop absorb queued burst, non-blocking, cap MAX_COALESCE_PER_FLUSH=256
        BA->>BC: try_recv -> push(event)
    end
    BA->>BC: encode_pending()
    alt backlog non-empty
        BC-->>BA: Some(0x23 multi-action frame)
        BA->>CC: coordinator.try_send(BroadcastAgentActionFrame(frame))
        alt mailbox has room
            CC-->>BA: Ok(())
            BA->>BC: clear()
        else SendError::Full(mailbox full)
            CC-->>BA: Err(Full)
            BA->>BA: rate-limited warn every BACKPRESSURE_WARN_INTERVAL=10s, backlog HELD (not cleared)
            Note over BA,BC: INVARIANT: held backlog is retried at FLUSH_RETRY_INTERVAL=20ms so the burst tail is never stranded
        else SendError::Closed
            CC-->>BA: Err(Closed)
            BA->>BA: error coordinator closed - forwarding task exiting, break
        end
    else backlog empty
        BC-->>BA: None
    end
    CC->>CC: client_manager.read().broadcast_to_all(frame)<br/>src/actors/client_coordinator_actor.rs:1462-1500
    opt slow clients detected
        CC->>CC: evict slow client via manager.unregister_client(id) (ADR-031 item 5)
    end
    Note over BC: INVARIANT: past MAX_PENDING_ACTIONS=256 the OLDEST buffered action is evicted and<br/>dropped_total incremented - never a silent drop-and-spam
```

## ES-02.5 Binary wire 0x23 AGENT_ACTION — single event and batch layout
```mermaid
classDiagram
    class MessageType {
      <<enum>>
      AgentAction = 0x23
    }
    class AgentActionEvent {
      +u32 source_agent_id
      +u32 target_node_id
      +u8 action_type
      +u32 timestamp
      +u16 duration_ms
      +List~u8~ payload
      +encode() List~u8~
      +decode(data) AgentActionEvent
      +get_action_type() AgentActionType
    }
    class AgentActionType {
      <<enum>>
      Query = 0
      Update = 1
      Create = 2
      Delete = 3
      Link = 4
      Transform = 5
    }
    AgentActionEvent --> AgentActionType : action_type
    note for AgentActionEvent "single frame src/utils/binary_protocol.rs:1499-1516: byte0=0x23 tag, then<br/>AGENT_ACTION_HEADER_SIZE=15 bytes: [0-3]source_agent_id [4-7]target_node_id [8]action_type<br/>[9-12]timestamp [13-14]duration_ms, then variable payload - all multi-byte fields<br/>little-endian"
    note for AgentActionEvent "batch frame src/utils/binary_protocol.rs:1556-1576: [0]0x23 tag [1-2]u16 event_count, then<br/>per-event: [u16 event_len][event bytes minus its own tag byte] repeated event_count times"
    note for AgentActionEvent "decode_agent_actions src/utils/binary_protocol.rs:1579-1590 rejects<br/>data.len()>MAX_PAYLOAD_SIZE=10*1024*1024 (10MB) before parsing any event"
```

## ES-02.6 Client render — decode, beam store, DIVERGENCE beam+gluon vs class_charge
```mermaid
sequenceDiagram
    autonumber
    participant WS as processBinaryData<br/>client/src/store/websocket/binaryProtocol.ts:459
    participant DEC as decodeAgentActions<br/>client/src/services/binaryProtocol
    participant DISP as dispatchAgentActions<br/>client/src/store/websocket/binaryProtocol.ts:444
    participant STORE as transientBeamStore<br/>client/src/store/transientBeamStore.ts:64
    participant LAYER as TransientBeamsLayer<br/>client/src/features/visualisation/components/TransientBeamsLayer.tsx:1

    WS->>WS: firstByte = DataView(data).getUint8(0)
    alt firstByte == MessageType.AGENT_ACTION (0x23)
        WS->>WS: handleAgentActionTagged(data): decodeAgentActions(data.slice(1)) if byteLength>=18<br/>client/src/store/websocket/binaryProtocol.ts:437-442
        Note over WS: RESOLVED ADR-2099 (2026-09-05): the 0x23 frame is a bare [tag][count]... layout, never the<br/>6-byte framed header. The unreachable handleAgentAction parseHeader branch is DELETED, not kept<br/>as a fallback - parseHeader reads type from offset 0, the same byte line 479 consumes before<br/>returning. Tests assert extractPayload is never reached for 0x23 - binaryProtocolAgentAction.test.ts
    else firstByte in {PROTOCOL_V3, PROTOCOL_V5}
        WS->>WS: handleLegacyBinaryData(data)
    else any other lead byte
        WS->>WS: parseHeader then switch on header.type - GRAPH_UPDATE / VOICE / POSITION<br/>default falls through to handleLegacyBinaryData. No AGENT_ACTION case exists here (ADR-2099)
    end
    WS->>DEC: decodeAgentActions(payload)
    DEC-->>WS: AgentActionEvent[]
    WS->>DISP: dispatchAgentActions(actions)
    DISP->>DISP: emit('agent-action', actions) [live transcript + attention heat]
    DISP->>STORE: pushTransientBeams(actions)
    STORE->>STORE: pushBeams: clampDuration (MIN_BEAM_DURATION_MS=400, DEFAULT=1500), FIFO cap MAX_TRANSIENT_BEAMS=256<br/>client/src/store/transientBeamStore.ts:57-61,25
    LAYER->>STORE: useTransientBeams() reads beams, calls pruneExpired() every frame
    LAYER->>LAYER: render coloured cylinder agent-node -> KG-node, opacity fade-in/hold/fade-out over durationMs, shape by action_type
    Note over LAYER: DIVERGENCE: render is a beam coloured cylinder only, no attractive gluon edge is wired.<br/>archived draft ADR docs/archive/adr/ADR-059-bidirectional-agent-channel-server.md rationale<br/>only specified a class_charge-modulation gluon, retracted because class_charge is bulk<br/>ontology-clustering metadata uploaded whole-array at construction<br/>src/utils/unified_gpu_compute/construction.rs:65,366 memory.rs:84 upload_class_metadata<br/>execution.rs:868 with no per-node update path
    Note over LAYER: DIVERGENCE: src/actors/agent_beam_actor.rs:327-363 documents the transient-attractive-edge<br/>mechanism gluon as DEFERRED, no UpsertTransientEdge GPU message exists, CSR edge buffers<br/>have no incremental insert path agent_beam_actor.rs:336-349, only the beam ships today
```

## ES-02.7 LEGACY — :9500 MCP-TCP relay lifecycle (docker-exec supervision)
```mermaid
sequenceDiagram
    autonumber
    participant CALL as ensure_mcp_ready<br/>src/services/mcp_relay_manager.rs:277
    participant MGR as McpRelayManager<br/>src/services/mcp_relay_manager.rs:13
    participant CB as CircuitBreaker<br/>src/services/mcp_relay_manager.rs:14
    participant DOCKER as docker exec multi-agent-container

    rect rgb(250,242,225)
    Note over CALL,DOCKER: DIVERGENCE (LEGACY): docker-exec supervision path, separate from the /wss/agent-events WS<br/>transport (ES-02.2)
    CALL->>MGR: check_mcp_container() -> docker ps -q -f name=multi-agent-container
    alt container not running
        MGR-->>CALL: Err "multi-agent-container is not running"
    end
    CALL->>MGR: ensure_relay_running()
    MGR->>CB: circuit_breaker.execute(check_relay_status_internal)<br/>failure_threshold:3 failure_rate_threshold:0.5 time_window:60s recovery_timeout:30s success_threshold:2 half_open_max_requests:3
    MGR->>DOCKER: docker exec multi-agent-container pgrep -f mcp-server
    alt pgrep succeeds (relay running)
        DOCKER-->>MGR: exit 0
        MGR-->>CALL: Ok(()) - already running, no action needed
    else pgrep fails (relay stopped)
        DOCKER-->>MGR: exit non-zero
        MGR->>DOCKER: docker exec -d multi-agent-container bash -c cd /app && npm run mcp:start
        MGR->>MGR: std::thread::sleep(2s) then re-check status
        alt restart confirmed running
            MGR-->>CALL: Ok(())
        else still not running
            MGR-->>CALL: Err "MCP relay started but not running"
        end
    else circuit breaker open (failure_rate_threshold exceeded within 60s window)
        CB-->>MGR: HealthCheckFailed
        MGR-->>CALL: Err (circuit open)
    end
    loop start_health_monitoring, interval 30s
        MGR->>DOCKER: health_manager.check_service_now("mcp-relay")
    end
    Note over MGR: INVARIANT: RetryableError classifies DockerCommandFailed/HealthCheckFailed/Timeout as<br/>retryable, ContainerNotFound as terminal
    Note over MGR: RESOLVED ADR-2090 — the /ws/mcp-relay upgrade (mcp_relay_handler.rs, route<br/>src/main.rs:1036) and /multi-mcp/ws (multi_mcp_websocket_handler.rs) previously accepted<br/>ANY non-empty string as a credential: neither referenced NostrService at all, and the sole<br/>gate was .is_empty(), so ?token=x opened the socket. Both now resolve the token through<br/>NostrService::get_session and fail closed on absent token, absent service, or a token that<br/>names no live unexpired session. Found by vc-core, fixed here as owner. see ADR-2044
    Note over MGR: RESOLVED ADR-2090 amendment / ADR-2058 — the ?token= query carrier is now<br/>DEV-ONLY on both sockets: compiled out of release behind cfg(any(debug_assertions,<br/>feature="dev-auth")), with a SECURITY warning on the dev arm and a SECURITY rejection<br/>warning on the release arm. The Authorization header is the only release carrier, so the<br/>bearer stops reaching access logs, proxy logs and Referer. Clients that cannot set headers<br/>use the post-connect NIP-98 authenticate envelope (kind 27235). Both cfg arms type-checked
    Note over MGR: RESOLVED ADR-2091 — the /multi-mcp scope also served two REST routes that<br/>returned FICTION: GET /status (get_mcp_server_status) emitted a hardcoded server list<br/>claiming claude-flow is_connected:true with agent_count:4, never querying anything, and<br/>POST /refresh (refresh_mcp_discovery) reported "Discovery refresh initiated" while doing<br/>nothing. Both took _app_state unused. Both REMOVED with their registrations — zero callers<br/>in src/, client/ or xr-client/. Real state lives in multi_mcp_agent_discovery.rs (ES-02.9)
    end
```

## ES-02.8 LEGACY — bots_client :9500 state-snapshot poll (JSON-RPC over raw TCP)
```mermaid
sequenceDiagram
    autonumber
    participant BC as BotsClient<br/>src/services/bots_client.rs:107
    participant TCP as McpTcpClient<br/>src/utils/mcp_tcp_client.rs:24
    participant SRV as MCP server<br/>multi-agent-container:9500 (env MCP_TCP_PORT, default 9500)
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs

    BC->>BC: new(): host=env CLAUDE_FLOW_HOST or MCP_HOST or multi-agent-container, port=env MCP_TCP_PORT or 9500<br/>src/services/bots_client.rs:114-123
    Note over BC: RESOLVED ADR-2088 — get_status() previously returned THREE literals<br/>(connected: true, host: agentic-workstation, port: 9090) that contradicted these<br/>resolved values. It now reports self.mcp_client.host/.port and a real<br/>AtomicBool connection state set by connect(). Routed from vc-knowledge (VC-27.1)
    BC->>TCP: connect(): test_connection() then initialize_session()
    alt server reachable
        TCP-->>BC: Ok(true)
        BC->>BC: start_polling()
    else server unreachable
        TCP-->>BC: Ok(false)
        BC-->>BC: Err "MCP server is not reachable"
    end
    loop tokio interval 2s (Duration::from_secs(2))<br/>src/services/bots_client.rs:178
        BC->>TCP: query_agent_list()
        TCP->>TCP: try_send_request(method:"agent_list", params:{filter:"all",include_metadata:true})<br/>src/utils/mcp_tcp_client.rs:291-299
        TCP->>SRV: TcpStream write_all JSON-RPC 2.0 request + newline<br/>src/utils/mcp_tcp_client.rs:242-246
        TCP->>SRV: BufReader.read_line() with self.timeout (10s)
        alt response line received before timeout
            SRV-->>TCP: {"jsonrpc":"2.0","result":[...],"id":N}
            TCP-->>BC: Ok(Vec~MultiMcpAgentStatus~)
            BC->>BC: Agent::from(mcp_agent) per entry incl did_nostr validate_did_nostr round-trip<br/>src/services/bots_client.rs:54-64,79-104
            opt graph_service_addr set
                BC->>GSS: do_send(UpdateBotsGraph{agents})
            end
        else read timeout elapses
            SRV--xTCP: (no response)
            TCP-->>BC: Err "Read timeout"
            BC->>BC: debug log, agents_lock left unchanged if not empty
        else connection closed with 0 bytes
            SRV--xTCP: EOF
            TCP-->>BC: Err "Connection closed without response"
        end
        alt MCP server returns empty agent list
            BC->>BC: clear stored agents (agents_lock.clear())
        end
    end
    Note over TCP: RESOLVED ADR-2084: ingest.rs no longer calls this deprecated. It is documented as legacy<br/>but LOAD-BEARING - the sole source of agent state snapshots (query_agent_list), constructed at<br/>app_state.rs:1231 with a boot poll, no replacement built. ADR-2084 stages the WS cutover with an<br/>acceptance test rather than implying one already exists
```

## ES-02.9 LEGACY — multi-MCP agent discovery, concurrent fan-out poll
```mermaid
sequenceDiagram
    autonumber
    participant CALLER as start_discovery caller
    participant DISC as MultiMcpAgentDiscovery<br/>src/services/multi_mcp_agent_discovery.rs:62
    participant CF as claude-flow server<br/>MCP_TCP_PORT default 9500
    participant RS as ruv-swarm server<br/>RUV_SWARM_PORT default 9501
    participant DAA as DAA server<br/>src/services/multi_mcp_agent_discovery.rs:292

    CALLER->>DISC: start_discovery()<br/>src/services/multi_mcp_agent_discovery.rs:169
    alt discovery_running already true
        DISC-->>CALLER: warn Discovery already running, return
    end
    loop while discovery_running, real sleep 1000ms (Duration::from_millis(1000))<br/>src/services/multi_mcp_agent_discovery.rs:257
        par concurrent per enabled server (futures::future::join_all)
            DISC->>CF: discover_claude_flow_agents(config)
            CF-->>DISC: Ok((server_info, agents, topology)) or Err
        and
            DISC->>RS: discover_ruv_swarm_agents(config)
            RS-->>DISC: Ok(...) or Err
        and
            DISC->>DAA: discover_daa_agents(config)
            DAA-->>DISC: Ok(...) or Err
        end
        alt server discovery Ok
            DISC->>DISC: insert into discovered_agents/server_statuses/topology_data, stats.successful_discoveries+=1, average_discovery_time_ms rolling avg<br/>src/services/multi_mcp_agent_discovery.rs:204-233
        else server discovery Err
            DISC->>DISC: stats.failed_discoveries+=1, server_info.is_connected=false<br/>src/services/multi_mcp_agent_discovery.rs:235-247
        end
    end
    Note over DISC: RESOLVED ADR-2083: discovery_interval_ms is now READ. select_discovery_interval_ms<br/>(:45) takes the MINIMUM across ENABLED servers so each is polled at least as often as it<br/>asked, floored at MIN_DISCOVERY_INTERVAL_MS 250 so a misconfigured 0 cannot busy-poll, with<br/>DEFAULT_DISCOVERY_INTERVAL_MS 5000 when no server is enabled. The hardcoded 1000ms sleep is<br/>gone - the loop now sleeps sleep_ms (:232 derived, :303 applied). Routed from vc-knowledge
```

## ES-02.10 REVERSE — VisionClaw calling agentbox: task lifecycle family
```mermaid
sequenceDiagram
    autonumber
    participant VC as ManagementApiClient<br/>src/services/management_api_client.rs:27
    participant AUTH as authMiddleware (hybrid)<br/>agentbox/management-api/middleware/auth.js:167
    participant TASKS as tasks route handlers<br/>agentbox/management-api/routes/tasks.js

    rect rgb(230,245,232)
    Note over VC,TASKS: TRUST BOUNDARY: VisionClaw container -> agentbox management-api, http://agentic-workstation:9090
    VC->>TASKS: POST /v1/tasks {agent,task,provider,claude_flow_agent_id?,user_context?,with_beads?,parent_bead_id?}<br/>Authorization: Bearer api_key<br/>src/services/management_api_client.rs:234-291 -> agentbox/management-api/routes/tasks.js:15
    TASKS->>AUTH: authMiddleware(request)
    alt Bearer token matches API_KEY (authMode allows bearer)
        AUTH-->>TASKS: request.auth = bearerResult
        TASKS-->>VC: 202/200 TaskResponse{task_id,status,message,task_dir?,log_file?,start_time?}
    else Bearer token wrong, no NIP-98 header, authMode=hybrid
        AUTH-->>TASKS: 401 {error:Unauthorized, message:"Expected Bearer token or Nostr NIP-98 authorization header"}
        TASKS-->>VC: ManagementApiError::ApiError(text,401)
    else authMode=strict-nip98, Bearer present, no Nostr header
        AUTH-->>TASKS: 401 {message:"Auth mode is strict-nip98 - Bearer tokens are not accepted"}
    end
    VC->>TASKS: GET /v1/tasks/:taskId<br/>src/services/management_api_client.rs:315-343 -> agentbox/management-api/routes/tasks.js:71
    TASKS-->>VC: 200 TaskStatus{status:Running|Completed|Failed,exit_code?,claude_flow_agent_id?} or 4xx ApiError
    VC->>TASKS: GET /v1/tasks<br/>src/services/management_api_client.rs:345-373 -> agentbox/management-api/routes/tasks.js:125
    TASKS-->>VC: 200 TaskListResponse{active_tasks:List~TaskInfo~,count}
    VC->>TASKS: DELETE /v1/tasks/:taskId<br/>src/services/management_api_client.rs:375-400 -> agentbox/management-api/routes/tasks.js:163
    TASKS-->>VC: 200 (stopped) or ApiError
    Note over VC: family: create_task/create_task_with_context, get_task_status, list_tasks, stop_task all<br/>share identical Bearer-header + StatusCode-match + ApiError(text,status) shape -<br/>src/services/management_api_client.rs:201-400
    end
```

## ES-02.11 REVERSE — VisionClaw calling agentbox: briefing workflow + status/health family
```mermaid
sequenceDiagram
    autonumber
    participant VC as ManagementApiClient<br/>src/services/management_api_client.rs:27
    participant BRF as briefs handlers<br/>agentbox/management-api/routes/briefing.js:1
    participant STA as status route<br/>agentbox/management-api/routes/status.js:12
    participant HLT as GET /health<br/>agentbox/management-api/server.js:543

    VC->>BRF: POST /v1/briefs {content,roles,user_context}<br/>src/services/management_api_client.rs:433-486
    BRF-->>VC: 201/200 BriefResponse{brief_id,brief_path,bead_id?} or ApiError
    VC->>BRF: POST /v1/briefs/:brief_id/execute {brief_path,roles,user_context,epic_bead_id?}<br/>src/services/management_api_client.rs:489-536
    BRF-->>VC: 202/200 ExecuteBriefResponse.role_tasks:List~RoleTask~ or ApiError
    VC->>BRF: POST /v1/briefs/:brief_id/debrief {role_responses,user_context}<br/>src/services/management_api_client.rs:539-584
    BRF-->>VC: 201/200 DebriefResponse{debrief_path} or ApiError
    VC->>STA: GET /v1/status (Bearer auth)<br/>src/services/management_api_client.rs:402-430
    STA-->>VC: 200 SystemStatus{api,tasks,gpu?,providers,system} or ApiError
    VC->>HLT: GET /health (no Authorization header sent)<br/>src/services/management_api_client.rs:586-597
    alt Fastify preValidation hook exempts /health from auth<br/>agentbox/management-api/server.js:231
        HLT-->>VC: 200 -> health_check() Ok(true)
    else non-200
        HLT-->>VC: Ok(false)
    end
    Note over VC,BRF: family: create_brief/execute_brief/create_debrief share identical Bearer + StatusCode-match<br/>+ ApiError(text,status) shape - src/services/management_api_client.rs:432-584
    Note over BRF: RESOLVED ADR-2085/2072 (2026-09-05): all three routes now exist in<br/>agentbox/management-api/routes/briefing.js, registered at management-api/server.js:1181.<br/>Brief documents and the durable brief record go through the pods adapter slot, the epic and<br/>role child beads through the beads slot, and every identifier is minted via lib/uris.js.<br/>The execute step is gated by the same ADR-2041 action pipeline as POST /v1/tasks and fails<br/>closed with 503 when the execution journal has no live events adapter.<br/>Activation is staged - the routes go live at the next image rebuild.
```
