---
id: VC-13
title: Position broadcast pipeline and WebSocket
area: visionclaw
governing:
  - docs/PROTOCOL-registry.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2003, ADR-2018, ADR-2009, ADR-2002]
sources:
  - src/gpu/broadcast_optimizer.rs
  - src/gpu/backpressure.rs
  - src/actors/gpu/force_compute_actor.rs
  - src/actors/graph_service_supervisor.rs
  - src/actors/physics_orchestrator_actor.rs
  - src/actors/client_coordinator_actor.rs
  - src/actors/graph_state_actor.rs
  - src/actors/messages/graph_messages.rs
  - src/handlers/socket_flow_handler/mod.rs
  - src/handlers/socket_flow_handler/http_handler.rs
  - src/handlers/socket_flow_handler/types.rs
  - src/handlers/socket_flow_handler/message_routing.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - src/handlers/socket_flow_handler/filter_auth.rs
  - src/utils/nip98.rs
  - src/utils/websocket_heartbeat.rs
  - src/utils/socket_flow_constants.rs
  - src/utils/binary_protocol.rs
  - src/gpu/mod.rs
  - client/src/store/websocket/binaryFrameDispatcher.ts
  - client/src/store/websocket/binaryProtocol.ts
  - client/src/features/graph/managers/graphDataManager.ts
  - client/src/features/graph/managers/dataManager/wsClient.ts
  - client/src/features/graph/managers/graphWorkerProxy.ts
  - client/src/features/graph/workers/graph.worker.ts
  - client/src/services/BinaryWebSocketProtocol.ts
  - src/handlers/socket_flow_handler/actor_messages.rs
verified_commit: bed6b617d
---

## VC-13.3 GPU broadcast frame end to end
```mermaid
sequenceDiagram
    autonumber
    participant FC as ForceComputeActor<br/>src/actors/gpu/force_compute_actor.rs:2447
    participant BO as BroadcastOptimizer<br/>src/gpu/broadcast_optimizer.rs:183
    participant BP as NetworkBackpressure<br/>src/gpu/backpressure.rs:262
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:2013
    participant GSA as GraphStateActor<br/>src/actors/graph_state_actor.rs:858
    participant PO as PhysicsOrchestratorActor<br/>src/actors/physics_orchestrator_actor.rs:1178
    participant CC as ClientCoordinatorActor<br/>src/actors/client_coordinator_actor.rs:345
    participant WS as SocketFlowServer<br/>src/handlers/socket_flow_handler/types.rs

    rect rgb(225,228,245)
    Note over FC: GPU tick (60Hz) — process_frame every physics step
    FC->>BO: process_frame(position_velocity_buffer, node_id_buffer)<br/>src/gpu/broadcast_optimizer.rs:183
    Note right of BO: BROADCAST-001: FULL SNAPSHOT ONLY<br/>no delta filter exists — visible_indices are<br/>visibility-culled only (culling disabled by default)<br/>src/gpu/broadcast_optimizer.rs:7-11
    alt rate-limit interval elapsed (target_fps=25, default)<br/>src/gpu/broadcast_optimizer.rs:33
        BO-->>FC: (should_broadcast=true, all_indices)
        FC->>BP: backpressure.try_acquire()<br/>src/gpu/backpressure.rs:262
        alt token available (max_tokens=100, cost=1)<br/>src/gpu/backpressure.rs:67-70
            BP-->>FC: Some(sequence_id)
            Note over FC: clamp NaN/Inf per-node<br/>src/actors/gpu/force_compute_actor.rs:2460
            FC->>GSS: UpdateNodePositions{positions, correlation_id}<br/>src/actors/messages/graph_messages.rs:59
            GSS->>GSA: do_send(UpdateNodePositions clone)<br/>src/actors/graph_service_supervisor.rs:2019
            GSA-->>GSA: mutate graph_data.nodes in-place<br/>src/actors/graph_state_actor.rs:861
            Note right of GSA: polling path (subscribe_position_updates)<br/>now returns GPU-computed layout, see VC-13.6
            GSS->>PO: do_send(UpdateNodePositions)<br/>src/actors/graph_service_supervisor.rs:2030
            PO->>PO: throttle to 60fps (16ms gate)<br/>src/actors/physics_orchestrator_actor.rs:1191
            opt user is dragging this node
                PO->>PO: override with pinned (x,y,z), vel=0<br/>src/actors/physics_orchestrator_actor.rs:1200
            end
            PO->>CC: do_send(BroadcastPositions{positions})<br/>src/actors/physics_orchestrator_actor.rs:1225
            CC->>CC: broadcast_with_filter(positions, node_type_arrays, seq, analytics)<br/>src/actors/client_coordinator_actor.rs:345
            CC->>CC: serialize_positions builds V5 frame (0x05 tag, u64 seq LE, V3 52B per node)<br/>src/actors/client_coordinator_actor.rs:410-446
            loop for each connected client<br/>src/actors/client_coordinator_actor.rs:368
                alt client.filter.enabled == false (default per-client node filter)
                    CC->>WS: try_send(SendToClientBinary(unfiltered_binary clone))<br/>src/actors/client_coordinator_actor.rs:371
                else client has an active node-id filter
                    CC->>CC: filter positions by filtered_node_ids<br/>src/actors/client_coordinator_actor.rs:374-378
                    CC->>WS: try_send(SendToClientBinary(filtered_binary))<br/>src/actors/client_coordinator_actor.rs:382
                end
            end
            Note over CC,WS: INVARIANT visibility filter default is fail-closed (ON)<br/>ADR-2003 — this per-client node-id filter is a<br/>SEPARATE mechanism from the pubkey visibility<br/>drop-set filter, which gates the polling path<br/>(fetch_nodes, see VC-13.6) and initial state sync,<br/>not this ClientCoordinatorActor broadcast
            Note over CC,WS: try_send Err(Full) or Err(Closed) marks the client<br/>for eviction (slow_clients), does not block other clients<br/>src/actors/client_coordinator_actor.rs:391-404
            WS->>WS: ctx.binary(payload) over the WebSocket
        else backpressure exhausted
            BP-->>FC: None
            FC->>BP: backpressure.record_skip()<br/>src/actors/gpu/force_compute_actor.rs:2484
            Note right of BP: congestion tracked, warn every<br/>log_interval_frames=60 skipped frames<br/>src/gpu/backpressure.rs:60-74,302-316
        end
    else rate-limited (inside broadcast_interval)
        BO-->>FC: (should_broadcast=false, [])
        Note right of FC: see VC-13.4 for the periodic<br/>full-broadcast escape hatch
    end
    end
```

## VC-13.1 WS connect, upgrade and authentication
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant HH as socket_flow_handler<br/>src/handlers/socket_flow_handler/http_handler.rs:39
    participant NS as NostrService<br/>src/handlers/socket_flow_handler/http_handler.rs:155
    participant N98 as nip98::validate_nip98_token<br/>src/utils/nip98.rs:325
    participant WSS as SocketFlowServer actor<br/>src/handlers/socket_flow_handler/types.rs:616
    participant FA as filter_auth::handle_authenticate<br/>src/handlers/socket_flow_handler/filter_auth.rs:10

    rect rgb(225,228,245)
    Note over C,HH: process boundary — HTTP/WS upgrade at /wss
    C->>HH: GET /wss (Upgrade: websocket)
    HH->>HH: WEBSOCKET_RATE_LIMITER.is_allowed(client_ip)<br/>src/handlers/socket_flow_handler/http_handler.rs:47
    alt rate limited
        HH-->>C: 429 create_rate_limit_response
    else Origin header check
        HH->>HH: validate Origin against CORS_ALLOWED_ORIGINS<br/>src/handlers/socket_flow_handler/http_handler.rs:58-133
        alt Origin invalid and not same-host
            HH-->>C: 403 Forbidden
        else Origin missing (release build)
            HH-->>C: 400 Origin header required<br/>src/handlers/socket_flow_handler/http_handler.rs:129-131
        else Origin OK
            HH->>HH: extract token from Authorization header<br/>OR ?token= query string (fallback)<br/>src/handlers/socket_flow_handler/http_handler.rs:139-150
            Note right of HH: RESOLVED ADR-2058 ?token= query-string auth is a live<br/>fallback alongside the Authorization header<br/>(not header-only) — tokens can land in access<br/>logs/referrers — http_handler.rs:145-150 — now header-only in release builds, query path compiled out behind the dev-auth gate
            alt token present
                HH->>NS: nostr_service.get_session(token)<br/>src/handlers/socket_flow_handler/http_handler.rs:158
                alt session valid
                    NS-->>HH: Some(session)
                else session invalid or NostrService absent
                    NS-->>HH: None
                    alt release build (or dev without ALLOW_INSECURE_DEFAULTS)
                        HH-->>C: 401 Invalid or expired authentication token<br/>src/handlers/socket_flow_handler/http_handler.rs:174-187
                    else dev build with ALLOW_INSECURE_DEFAULTS=1
                        Note right of HH: dev-only bypass, compile-gated out of release<br/>src/handlers/socket_flow_handler/http_handler.rs:19-29
                    end
                end
            else no token at all
                alt release build
                    HH-->>C: 401 Authentication required<br/>src/handlers/socket_flow_handler/http_handler.rs:236-254
                else dev build with ALLOW_INSECURE_DEFAULTS=1
                    Note right of HH: unauthenticated WS permitted, dev only
                end
            end
            HH->>WSS: ws_server = SocketFlowServer::new(...)<br/>src/handlers/socket_flow_handler/http_handler.rs:310
            HH->>WSS: set connection_url for NIP-98 WS validation<br/>src/handlers/socket_flow_handler/http_handler.rs:328-340
            opt ?token= present a second time (pre-auth of the actor)
                HH->>NS: nostr_service.get_session(token_from_qs)<br/>src/handlers/socket_flow_handler/http_handler.rs:345
                NS-->>HH: user with pubkey, is_power_user
                HH->>WSS: ws_server.pubkey = Some(user.pubkey)<br/>src/handlers/socket_flow_handler/http_handler.rs:346-347
            end
            HH->>C: 101 Switching Protocols (WsResponseBuilder, permessage-deflate)<br/>src/handlers/socket_flow_handler/http_handler.rs:360-362
        end
    end
    end

    rect rgb(225,245,230)
    Note over C,WSS: post-connect NIP-98 "authenticate" message (kind 27235, ADR-2009 primary realm)
    C->>WSS: text message type=authenticate, event=base64 kind-27235 event
    WSS->>FA: handle_authenticate(act, msg, ctx)<br/>src/handlers/socket_flow_handler/filter_auth.rs:10
    alt VISIONCLAW_DEV_MODE full bypass (dev builds only)
        FA-->>C: authenticate_success (DEV_MODE_PUBKEY, power_user)<br/>src/handlers/socket_flow_handler/filter_auth.rs:26-59
    else event field present (NIP-98 path)
        FA->>N98: verify_nip98_auth auth_header=Nostr plus event_b64, ws_url, method=GET<br/>src/handlers/socket_flow_handler/filter_auth.rs:74
        N98->>N98: check kind==27235, url/method match<br/>src/utils/nip98.rs:20,298
        N98->>N98: freshness ±TOKEN_MAX_AGE_SECONDS=60s<br/>src/utils/nip98.rs:169,362-367
        N98->>N98: claim event id in replay cache (TTL=120s,<br/>cap 100000, fail-closed ReplayCacheFull)<br/>src/utils/nip98.rs:171-196,316-319 (ADR-2002)
        alt validation and single-use claim succeed
            N98-->>FA: Ok(user)
            FA-->>C: authenticate_success{pubkey, is_power_user}<br/>src/handlers/socket_flow_handler/filter_auth.rs:104-112
            FA->>WSS: send_full_state_sync(ctx) re-push filtered by new pubkey<br/>src/handlers/socket_flow_handler/filter_auth.rs:123
        else signature/URL/method/freshness/replay fails
            N98-->>FA: Err(e)
            FA-->>C: error NIP-98 WebSocket authentication failed<br/>src/handlers/socket_flow_handler/filter_auth.rs:124-132
        end
    else legacy token+pubkey fields (ADR-2009 secondary realm)
        Note right of FA: legacy { token, pubkey, ephemeral? } path<br/>src/handlers/socket_flow_handler/filter_auth.rs:137-140
    end
    end
```

## VC-13.7 Client decode: binary frame to SharedArrayBuffer
```mermaid
sequenceDiagram
    autonumber
    participant WS as WebSocket.onmessage<br/>client/src/store/websocket/binaryFrameDispatcher.ts:97
    participant BFD as BinaryFrameDispatcher<br/>client/src/store/websocket/binaryFrameDispatcher.ts:43
    participant BP as processBinaryData<br/>client/src/store/websocket/binaryProtocol.ts:464
    participant GDM as graphDataManager<br/>client/src/features/graph/managers/graphDataManager.ts:433
    participant WSC as handleBinaryFrame<br/>client/src/features/graph/managers/dataManager/wsClient.ts:19
    participant GWP as graphWorkerProxy<br/>client/src/features/graph/managers/graphWorkerProxy.ts:205
    participant WRK as graph.worker.ts<br/>client/src/features/graph/workers/graph.worker.ts:214

    rect rgb(222,236,250)
    WS->>WS: validateBinaryData — lead byte in {3,5,AGENT_ACTION}<br/>client/src/store/websocket/binaryProtocol.ts:187-207
    WS->>BFD: dispatcher.handle(buffer)<br/>client/src/store/websocket/binaryFrameDispatcher.ts:51
    alt frame already in flight
        BFD->>BFD: pendingLatest = buffer (newest-wins, drop older)<br/>client/src/store/websocket/binaryFrameDispatcher.ts:52-60
    else no frame in flight
        BFD->>BP: processBinaryData(buffer, get, set)<br/>client/src/store/websocket/binaryFrameDispatcher.ts:64
        BP->>BP: read lead byte, V2/V3/V5 routes to legacy path<br/>client/src/store/websocket/binaryProtocol.ts:474-480
        BP->>BP: parseBinaryFrameData(data)<br/>client/src/store/websocket/binaryProtocol.ts:370
        opt full V3/V5 frame
            BP->>BP: nodeAnalyticsStore.ingest(parsedNodes)<br/>client/src/store/websocket/binaryProtocol.ts:381-386
        end
        BP->>GDM: graphDataManager.updateNodePositions(data)<br/>client/src/store/websocket/binaryProtocol.ts:407
        GDM->>WSC: handleBinaryFrame(positionData, lastUpdateTime, onUpdateTime)<br/>client/src/features/graph/managers/graphDataManager.ts:436
        alt within 16ms of last update (~60fps throttle)
            WSC-->>GDM: drop frame<br/>client/src/features/graph/managers/dataManager/wsClient.ts:27
        else
            WSC->>GWP: processBinaryFrame(frame: Uint8Array)<br/>client/src/features/graph/managers/dataManager/wsClient.ts:46
            alt frame already in flight (single-flight, D2)
                GWP->>GWP: pendingLatest = frame (newest-wins)<br/>client/src/features/graph/managers/graphWorkerProxy.ts:210-217
            else dispatch now
                GWP->>GWP: transfer(frame.buffer) — zero-copy neuter<br/>client/src/features/graph/managers/graphWorkerProxy.ts:248
                alt SAB mode (SharedArrayBuffer available, cross-origin isolated)<br/>client/src/features/graph/managers/graphWorkerProxy.ts:170-186
                    GWP->>WRK: workerApi.processBinaryFrame(transferable)<br/>client/src/features/graph/managers/graphWorkerProxy.ts:253
                    WRK->>WRK: decode V3/V5 node records into currentPositions<br/>client/src/features/graph/workers/graph.worker.ts:215
                    WRK->>WRK: syncToSharedBuffer — write positionView<br/>(the SharedArrayBuffer)<br/>client/src/features/graph/workers/graph.worker.ts:226-227
                    Note right of WRK: SAB write complete — renderer reads<br/>positionView directly, no return value needed<br/>client/src/features/graph/workers/graph.worker.ts:205-218
                else Comlink transfer mode (SAB unavailable)
                    WRK-->>GWP: transferred ArrayBuffer of stride-3 positions<br/>client/src/features/graph/workers/graph.worker.ts:219-223
                    GWP->>GWP: lastTransferredView = new Float32Array(returned)<br/>client/src/features/graph/managers/graphWorkerProxy.ts:262-264
                end
            end
        end
    end
    end
    Note over WRK: render side beyond SAB see VC-31/VC-32
```

## VC-13.4 Periodic full-broadcast escape hatch
```mermaid
sequenceDiagram
    autonumber
    participant FC as ForceComputeActor<br/>src/actors/gpu/force_compute_actor.rs:2447
    participant BO as BroadcastOptimizer<br/>src/gpu/broadcast_optimizer.rs:183
    participant BP as NetworkBackpressure<br/>src/gpu/backpressure.rs:262
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:2013

    rect rgb(225,228,245)
    Note over FC: FastSettle path — force_full_broadcast set by the settle controller
    alt actor.force_full_broadcast is true<br/>src/actors/gpu/force_compute_actor.rs:2412
        FC->>FC: force_full_broadcast=false, suppress_intermediate_broadcasts=false<br/>src/actors/gpu/force_compute_actor.rs:2414-2415
        FC->>BO: broadcast_optimizer.reset_broadcast_timer()<br/>src/actors/gpu/force_compute_actor.rs:2416
        FC->>BP: backpressure.try_acquire()<br/>src/actors/gpu/force_compute_actor.rs:2418
        FC->>GSS: UpdateNodePositions ALL nodes (final converged positions)<br/>src/actors/gpu/force_compute_actor.rs:2432-2440
        Note right of FC: FINAL full broadcast logged every settle<br/>src/actors/gpu/force_compute_actor.rs:2433-2436
    else actor.suppress_intermediate_broadcasts is true (settle burst in progress)
        FC->>BO: process_frame — advances the rate-limit timer only, no send<br/>src/actors/gpu/force_compute_actor.rs:2446
    else continuous mode, normal rate-limited path
        FC->>BO: process_frame(position_velocity_buffer, node_id_buffer)<br/>src/actors/gpu/force_compute_actor.rs:2452
        alt should_broadcast true and backpressure token acquired
            FC->>GSS: UpdateNodePositions (rate-limited full snapshot)<br/>src/actors/gpu/force_compute_actor.rs:2469-2481
            FC->>FC: last_full_broadcast_iteration = iteration_count<br/>src/actors/gpu/force_compute_actor.rs:2481
        else should_broadcast false (rate-limited this tick)
            alt iteration_count minus last_full_broadcast_iteration is at least 300<br/>src/actors/gpu/force_compute_actor.rs:2486
                Note right of FC: periodic full broadcast for late-connecting<br/>clients — N=300 iterations, independent of the<br/>25fps rate limiter above<br/>src/actors/gpu/force_compute_actor.rs:2486-2519
                FC->>BP: backpressure.try_acquire()<br/>src/actors/gpu/force_compute_actor.rs:2488
                FC->>GSS: UpdateNodePositions ALL nodes (periodic full)<br/>src/actors/gpu/force_compute_actor.rs:2504-2513
                FC->>FC: last_full_broadcast_iteration = iteration_count<br/>src/actors/gpu/force_compute_actor.rs:2516
                FC->>BO: broadcast_optimizer.reset_broadcast_timer()<br/>src/actors/gpu/force_compute_actor.rs:2518
            else below 300 iterations since last full broadcast
                Note right of FC: no broadcast this tick — waits for either<br/>the 25fps gate or the 300-iteration escape hatch
            end
        end
    end
    Note over FC,GSS: DIVERGENCE this mechanism sends ALL nodes on every<br/>emission (BROADCAST-001 full-snapshot design) — there is<br/>no delta/diff path to re-synchronise, unlike the historical<br/>delta-filter design implied by older docs<br/>src/gpu/broadcast_optimizer.rs:1-11
    end
```

## VC-13.2 Subscribe to position updates, and drag/pin control messages
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant WSS as SocketFlowServer<br/>src/handlers/socket_flow_handler/message_routing.rs:16
    participant PU as position_updates handlers<br/>src/handlers/socket_flow_handler/position_updates.rs
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:1495
    participant GPU as GpuComputeActor

    rect rgb(222,236,250)
    C->>WSS: text subscribe_position_updates {interval, binary, nodeTypes}
    WSS->>PU: handle_subscribe_position_updates(act, msg, ctx)<br/>src/handlers/socket_flow_handler/position_updates.rs:603
    alt subscribed within 2s of the last accepted subscribe
        PU-->>C: rate-limited, ignored (per-session guard)<br/>src/handlers/socket_flow_handler/position_updates.rs:618-623
    else accepted
        PU->>PU: position_sub_generation += 1 (orphans older loops)<br/>src/handlers/socket_flow_handler/position_updates.rs:632-633
        PU->>PU: min_allowed_interval = 1000 / (300/60) = 200ms<br/>EndpointRateLimits::socket_flow_updates() rpm=300<br/>src/handlers/socket_flow_handler/position_updates.rs:666-668
        PU-->>C: subscription_confirmed{interval, binary, rate_limit}<br/>src/handlers/socket_flow_handler/position_updates.rs:690-703
        loop self-perpetuating run_later every actual_interval ms<br/>src/handlers/socket_flow_handler/position_updates.rs:705
            PU->>PU: fetch_nodes(app_state, settings_addr)<br/>src/handlers/socket_flow_handler/position_updates.rs:706
            alt act.position_sub_generation != my_generation
                Note right of PU: stale loop — drop result, do not reschedule<br/>src/handlers/socket_flow_handler/position_updates.rs:712-714
            else current generation
                opt subscribed_node_types non-empty
                    PU->>PU: nodes.retain by node type flag bits<br/>src/handlers/socket_flow_handler/position_updates.rs:719-733
                end
                opt PUBKEY_VISIBILITY_FILTER on and visibility non-empty
                    PU->>PU: compute_private_opaque_ids + apply_drop_set<br/>fail-closed for anon pubkey<br/>src/handlers/socket_flow_handler/position_updates.rs:742-754 (ADR-2003)
                end
                PU->>PU: encode_node_data_extended_with_sssp<br/>src/handlers/socket_flow_handler/position_updates.rs:763-772
                PU->>C: ctx.binary(binary_data)<br/>src/handlers/socket_flow_handler/position_updates.rs:798
                PU->>PU: ctx.run_later(actual_interval, re-inject subscribe msg)<br/>src/handlers/socket_flow_handler/position_updates.rs:801-818
            end
        end
    end
    end

    rect rgb(250,228,228)
    Note over C,GPU: process boundary — drag/pin handlers ALL require an authenticated pubkey (VULN-01)
    C->>WSS: text nodeDragStart {nodeId, x, y, z}
    WSS->>PU: handle_node_drag_start(act, msg, ctx)<br/>src/handlers/socket_flow_handler/position_updates.rs:954
    alt act.pubkey.is_none()
        PU-->>C: rejected — unauthenticated<br/>src/handlers/socket_flow_handler/position_updates.rs:960-961
    else authenticated
        alt dragged_nodes.len() at least MAX_DRAGGED_NODES_PER_CLIENT (5)<br/>src/handlers/socket_flow_handler/position_updates.rs:88,1010-1015
            PU-->>C: rejected — too many simultaneous drags
        else under the cap
            PU->>GSS: UpdateNodePositions pinned_data, velocity=0<br/>src/handlers/socket_flow_handler/position_updates.rs:1044-1057
            PU->>GPU: PinNodePositions{pins:[(node_id,[x,y,z])]}<br/>src/handlers/socket_flow_handler/position_updates.rs:1060-1067
            PU->>PU: start drag timeout checker<br/>src/handlers/socket_flow_handler/position_updates.rs:1086-1091
        end
    end
    C->>WSS: text nodeDragUpdate {nodeId, x, y, z}
    WSS->>PU: handle_node_drag_update(act, msg, ctx)<br/>src/handlers/socket_flow_handler/position_updates.rs:1105
    alt act.pubkey.is_none()
        PU-->>C: rejected — unauthenticated<br/>src/handlers/socket_flow_handler/position_updates.rs:1111-1112
    else within MIN_DRAG_INTERVAL_MS=16 rate cap<br/>src/handlers/socket_flow_handler/position_updates.rs:91,1130-1136
        PU-->>C: ignored (rate limited)
    else accepted update
        PU->>GPU: PinNodePositions updated pin<br/>src/handlers/socket_flow_handler/position_updates.rs:1214-1221
        Note right of PU: settle budget DRAG_SETTLE_BUDGET_MS=50ms<br/>src/handlers/socket_flow_handler/position_updates.rs:85
    end
    C->>WSS: text nodeUnpin {nodeId}
    WSS->>PU: handle_node_unpin(act, msg, ctx)<br/>src/handlers/socket_flow_handler/position_updates.rs:1462
    alt act.pubkey.is_none()
        PU-->>C: rejected — unauthenticated<br/>src/handlers/socket_flow_handler/position_updates.rs:1468-1471
    else authenticated
        PU->>GPU: PinNodePositions{unpin:[node_id], reheat:true}<br/>src/handlers/socket_flow_handler/position_updates.rs:1498-1506
        PU-->>C: nodeUnpinAck<br/>src/handlers/socket_flow_handler/position_updates.rs:1510-1517
    end
    end
```

## VC-13.5 Heartbeat and ping/pong
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant WSS as SocketFlowServer<br/>src/handlers/socket_flow_handler/types.rs:616
    participant SH as StreamHandler dispatch<br/>src/handlers/socket_flow_handler/mod.rs:23
    participant MR as message_routing<br/>src/handlers/socket_flow_handler/message_routing.rs:16
    participant OA as other actors<br/>src/handlers/socket_flow_handler/actor_messages.rs:230-233

    rect rgb(225,228,245)
    Note over WSS: Actor::started sets up a server-driven ping timer<br/>src/handlers/socket_flow_handler/types.rs:662-669
    loop ctx.run_interval every 5s<br/>src/handlers/socket_flow_handler/types.rs:663
        WSS->>C: ctx.ping(empty payload)<br/>src/handlers/socket_flow_handler/types.rs:665
        WSS->>WSS: last_activity = Instant::now()<br/>src/handlers/socket_flow_handler/types.rs:666
    end
    C->>SH: WS Pong frame
    SH->>SH: last_activity = Instant::now()<br/>src/handlers/socket_flow_handler/mod.rs:31-33
    Note right of SH: no idle-timeout disconnect is wired to last_activity<br/>on this path — CLIENT_TIMEOUT=60s in<br/>socket_flow_constants.rs:9 is unused here
    opt client sends a standard WS Ping frame
        C->>SH: WS Ping frame
        SH->>C: ctx.pong(payload)<br/>src/handlers/socket_flow_handler/mod.rs:26-30
    end
    opt client sends plain-text "ping"
        C->>MR: text "ping"
        MR-->>C: text "pong"<br/>src/handlers/socket_flow_handler/message_routing.rs:18-22
    end
    opt client sends JSON {type: ping}
        C->>MR: text {"type":"ping",...}
        MR->>WSS: handle_ping(PingMessage)<br/>src/handlers/socket_flow_handler/message_routing.rs:109-115
        WSS-->>C: PongMessage JSON<br/>src/handlers/socket_flow_handler/types.rs:232-236
    end
    Note over WSS,OA: RESOLVED ADR-2054 PushDirective queues a HeartbeatDirective<br/>into pending_directives (ADR-031 item 4), but this actor's<br/>ping/pong path never calls WebSocketHeartbeat::send_pong<br/>or get_pending_directives — queued directives<br/>(ReloadConfig, ForceFullSync, UpdateAvailable) are never<br/>flushed to the client on this code path<br/>src/handlers/socket_flow_handler/actor_messages.rs:230-233,<br/>src/utils/websocket_heartbeat.rs:88-114 — removed, zero senders
    end
```

## VC-13.6 Polling path — GetGraphData to binary encode
```mermaid
sequenceDiagram
    autonumber
    participant PU as fetch_nodes<br/>src/handlers/socket_flow_handler/position_updates.rs:114
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs:1495
    participant GSA as GraphStateActor<br/>src/actors/graph_state_actor.rs:858
    participant BIN as binary_protocol::encode_node_data_extended_with_sssp<br/>src/utils/binary_protocol.rs:394

    rect rgb(225,228,245)
    Note over PU: invoked from the subscribe_position_updates run_later<br/>loop, see VC-13.2 — this is the per-connection<br/>poll path, independent of the ClientCoordinatorActor push<br/>path in VC-13.3
    PU->>GSS: send(GetGraphData)<br/>src/handlers/socket_flow_handler/position_updates.rs:122
    GSS->>GSA: forward GetGraphData<br/>src/actors/graph_service_supervisor.rs:1499-1506
    GSA-->>GSS: shared GraphData snapshot, GPU-computed positions<br/>updated by VC-13.3's UpdateNodePositions forward
    GSS-->>PU: shared GraphData snapshot
    alt graph_data.nodes is empty
        PU-->>PU: return None (transient) — caller reschedules<br/>src/handlers/socket_flow_handler/position_updates.rs:137-141,819-843
    else nodes present
        PU->>GSS: send(GetNodeTypeArrays)<br/>src/handlers/socket_flow_handler/position_updates.rs:144
        GSS-->>PU: NodeTypeArrays (agent/knowledge/ontology id sets)
        loop for each graph node<br/>src/handlers/socket_flow_handler/position_updates.rs:186
            PU->>PU: flag compact_id with agent/knowledge/ontology bit<br/>src/handlers/socket_flow_handler/position_updates.rs:190-202
            opt PUBKEY_VISIBILITY_FILTER on (default)<br/>src/handlers/socket_flow_handler/position_updates.rs:178
                PU->>PU: push NodeVisibility{wire_id, is_public, owner_pubkey}<br/>src/handlers/socket_flow_handler/position_updates.rs:203-204
            end
        end
        Note right of PU: INVARIANT visibility filter default is fail-closed (ON)<br/>ADR-2003 — absent env means filtered, anon caller<br/>sees public-only nodes<br/>src/handlers/socket_flow_handler/position_updates.rs:17-42
        PU-->>PU: Some((nodes, detailed_debug, visibility))<br/>src/handlers/socket_flow_handler/position_updates.rs:218
    end
    PU->>BIN: encode_node_data_extended_with_sssp(nodes, type_id_slices, analytics)<br/>src/handlers/socket_flow_handler/position_updates.rs:763-772
    BIN-->>PU: V3 binary frame (WIRE_V3_ITEM_SIZE=52 bytes per node)<br/>src/utils/binary_protocol.rs:1052,1149 (ADR-2018)
    Note right of BIN: DOC-DRIFT docs/PROTOCOL-registry.md cites the<br/>WIRE_V3_ITEM_SIZE==52 assertions at binary_protocol.rs<br/>lines 712 and 809 — the working tree has them at<br/>lines 1052 and 1149
    end
```

## VC-13.8 Client session lifecycle
```mermaid
stateDiagram-v2
    [*] --> Connecting
    Connecting --> Rejected: origin or token check fails<br/>src/handlers/socket_flow_handler/http_handler.rs:104-254
    Connecting --> Connected: 101 Switching Protocols<br/>src/handlers/socket_flow_handler/http_handler.rs:360-362
    Rejected --> [*]

    Connected --> Anonymous: SocketFlowServer started<br/>send_full_state_sync public-only view<br/>src/handlers/socket_flow_handler/types.rs:619-672
    Anonymous --> Authenticated: authenticate message<br/>NIP-98 kind-27235 or legacy token/pubkey<br/>src/handlers/socket_flow_handler/filter_auth.rs:10 (ADR-2009)
    Anonymous --> Subscribed: subscribe_position_updates<br/>(pubkey stays None, public-only stream)<br/>src/handlers/socket_flow_handler/position_updates.rs:603

    Authenticated --> Subscribed: subscribe_position_updates<br/>src/handlers/socket_flow_handler/position_updates.rs:603
    Authenticated --> Dragging: nodeDragStart requires pubkey<br/>src/handlers/socket_flow_handler/position_updates.rs:960-961

    Subscribed --> Streaming: first run_later tick delivers a<br/>binary frame via ctx.binary<br/>src/handlers/socket_flow_handler/position_updates.rs:705-798
    Streaming --> Streaming: generation-gated run_later loop<br/>re-subscribes every actual_interval ms<br/>src/handlers/socket_flow_handler/position_updates.rs:800-818
    Streaming --> Backpressured: ClientCoordinatorActor try_send<br/>hits SendError Full on the client mailbox<br/>src/actors/client_coordinator_actor.rs:392-403
    Backpressured --> Streaming: mailbox drains, next<br/>broadcast_with_filter try_send succeeds

    Dragging --> Authenticated: nodeDragEnd or timeout unpin<br/>src/handlers/socket_flow_handler/position_updates.rs:1311,1523

    Streaming --> Closed: WS Close frame or heartbeat<br/>Pong not received (server ping every 5s)<br/>src/handlers/socket_flow_handler/mod.rs:40-44
    Backpressured --> Closed: SendError Closed, client<br/>evicted as a slow client<br/>src/actors/client_coordinator_actor.rs:394-403
    Closed --> [*]
```
