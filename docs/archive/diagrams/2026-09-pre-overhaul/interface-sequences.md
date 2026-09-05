# VisionClaw Interface Surface — Sequence Diagrams

Cartography audit of the actual interface surface, built from source code.
All file:line references are relative to the repo root.

---

## 1. REST Request Lifecycle

### 1a. GET /api/graph/data — Read Path

Route registered at `src/main.rs:882` under `/api` scope → `api_handler::config` →
`graph::config` (`src/handlers/api_handler/graph/mod.rs:627`) → `/graph/data GET → get_graph_data`.

Auth is **optional** on this route: `get_graph_data` does NOT extract `AuthenticatedUser`.
The handler pulls data directly from `GraphServiceSupervisor` via CQRS query handlers.

```mermaid
sequenceDiagram
    participant C as Client (browser)
    participant N as nginx
    participant A as actix-web (HttpServer)
    participant H as get_graph_data handler<br/>src/handlers/api_handler/graph/mod.rs:148
    participant GSS as GraphServiceSupervisor<br/>src/actors/graph_service_supervisor.rs
    participant GSA as GraphStateActor<br/>src/actors/graph_state_actor.rs

    C->>N: GET /api/graph/data?graph_type=knowledge
    N->>A: HTTP/1.1 proxied request
    A->>A: Route match: /api → api_handler::config<br/>→ graph::config → /graph/data
    Note over A: No auth extractor on this handler<br/>(FINDING-1: unauthenticated read path)
    A->>H: invoke handler(state, query)
    H->>GSS: GetGraphData (CQRS query)<br/>src/application/graph/queries.rs
    GSS->>GSA: GetGraphData message
    GSA-->>GSS: Ok(GraphData { nodes, edges, metadata })
    H->>GSS: GetPhysicsState
    GSS-->>H: Ok(PhysicsState { is_settled, kinetic_energy, ... })
    H->>H: filter nodes by graph_type param<br/>GraphTypeFilter::matches()<br/>src/handlers/api_handler/graph/mod.rs:110
    H->>H: build GraphResponseWithPositions<br/>{ nodes: Vec<NodeWithPosition>, edges, settlement_state }
    H-->>A: HttpResponse::Ok + JSON body
    A-->>N: 200 OK
    N-->>C: JSON { nodes, edges, metadata, settlementState }
```

### 1b. PUT /api/settings/physics — Write Path

Route: `src/main.rs:888` → `/api/settings` scope (RateLimit 60/min) →
`visionclaw_server::settings::api::configure_routes` →
`src/settings/api/settings_routes.rs:1314` → `physics PUT → update_physics_settings`.

`AuthenticatedUser` extractor is mandatory here (`src/settings/api/settings_routes.rs:303`).

```mermaid
sequenceDiagram
    participant C as Client (browser / axios)
    participant IC as axios interceptor<br/>src/client/api/settings/endpoints.ts:37
    participant N as nginx
    participant A as actix-web
    participant AE as AuthenticatedUser extractor<br/>src/settings/auth_extractor.rs:70
    participant NS as NostrService<br/>src/services/nostr_service.rs
    participant H as update_physics_settings<br/>src/settings/api/settings_routes.rs:303
    participant SA as OptimizedSettingsActor<br/>src/actors/optimized_settings_actor.rs
    participant GPU as GPUComputeActor<br/>src/actors/gpu/

    C->>IC: updatePhysics(partialSettings)
    IC->>IC: nostrAuth.signRequest(url, "PUT", body)<br/>→ window.nostr.signEvent(NIP-98 kind:27235)
    IC->>N: PUT /api/settings/physics<br/>Authorization: Nostr <base64-event>
    N->>A: proxied request
    A->>AE: FromRequest::from_request()
    AE->>AE: parse "Nostr <b64>" Authorization header
    AE->>NS: verify_nip98_auth(header, url, "PUT", None)
    NS->>NS: decode base64, verify Schnorr sig,<br/>check event.kind==27235, timestamp TTL
    NS-->>AE: Ok(AuthenticatedUser { pubkey, is_power_user })
    AE-->>A: AuthenticatedUser injected
    A->>H: invoke(state, body, auth, settings_repo)
    H->>SA: GetSettings (current snapshot)
    SA-->>H: Ok(AppFullSettings)
    H->>H: normalize_physics_keys(patch)<br/>merge patch onto current snapshot
    H->>H: validate_physics_settings(&new_physics)
    H->>SA: UpdateSettings { settings: full_settings }
    SA->>SA: update_settings(): write RwLock,<br/>clear path_cache, call settings.save()
    Note over SA: save() persists to YAML file
    SA-->>H: Ok(())
    H->>H: settings_repo.set_setting("physics", Json)<br/>(SQLite-first: SQLite wins over actor<br/>on the next GET — see 01-settings-flow.md)
    H->>GPU: UpdateSimulationParams { params }<br/>(direct GPUComputeActor addr, fallback GPUManagerActor)
    GPU-->>H: Ok(()) [fire-and-forget if no addr cached]
    H-->>A: HttpResponse::Ok + JSON { physics settings echo }
    A-->>C: 200 OK
```

---

## 2. WebSocket Binary Position Pipeline

### Connection → Position Subscription → Frame Decode → SAB Write

`/wss` route: `src/main.rs:867` → `socket_flow_handler` (actix-web-actors WebSocket upgrade).

```mermaid
sequenceDiagram
    participant C as Client JS (main thread)
    participant WS as WebSocket (browser)
    participant SFH as SocketFlowServer actor<br/>src/handlers/socket_flow_handler/
    participant CCM as ClientCoordinatorActor<br/>src/actors/client_coordinator_actor.rs
    participant GSA as GraphStateActor
    participant Enc as binary_protocol::encode<br/>src/utils/binary_protocol.rs
    participant BFD as binaryFrameDispatcher<br/>client/src/store/websocket/binaryFrameDispatcher.ts
    participant WBP as store/websocket/binaryProtocol.ts
    participant GW as graph.worker.ts (Comlink)<br/>client/src/features/graph/workers/graph.worker.ts
    participant SAB as SharedArrayBuffer<br/>(positionView Float32Array)

    C->>WS: new WebSocket("wss://host/wss")
    WS->>SFH: HTTP upgrade → actix-web-actors ws::start()
    SFH->>CCM: register client → SetClientId(N)
    CCM-->>SFH: client_id assigned

    Note over C,SFH: Client authenticates (see Diagram 4)

    C->>WS: send JSON { type: "subscribe_position_updates",<br/>data: { interval: 60, binary: true } }
    WS->>SFH: StreamHandler::handle() text message
    SFH->>SFH: handle_subscribe_position_updates()<br/>src/handlers/socket_flow_handler/position_updates.rs:433
    SFH->>SFH: bump position_sub_generation (dedup loops)
    SFH-->>WS: JSON { type: "subscription_confirmed", interval: 60 }
    WS-->>C: subscription_confirmed

    loop every 60 ms (run_later self-loop)
        SFH->>SFH: fetch_nodes(app_state)<br/>position_updates.rs:38
        SFH->>GSA: GetGraphData
        GSA-->>SFH: Ok(GraphData)
        SFH->>GSA: GetNodeTypeArrays
        GSA-->>SFH: NodeTypeArrays { agent_ids, knowledge_ids, ... }
        SFH->>SFH: stamp flagged_id bits<br/>(AGENT=0x80000000, KNOWLEDGE=0x40000000,<br/>ONTOLOGY_CLASS=0x04000000 etc)
        SFH->>Enc: encode_node_data_extended_with_sssp(&nodes, analytics)
        Note over Enc: V3 extended format: 52 bytes/node<br/>[id@0 u32][pos@4 3×f32][vel@16 3×f32]<br/>[sssp@28 f32][parent@32 i32][cluster@36 u32]<br/>[anomaly@40 f32][community@44 u32][centrality@48 f32]
        Enc-->>SFH: Vec<u8>
        SFH->>WS: ctx.binary(Vec<u8>)
        WS->>C: binary frame (ArrayBuffer)
        C->>BFD: binaryFrameDispatcher.handle(buffer)
        BFD->>BFD: newest-wins: drop if in-flight
        BFD->>WBP: processBinaryData(buffer, get, set)
        WBP->>WBP: validateBinaryData()<br/>(checks version byte == 3 or 5)
        WBP->>WBP: parseBinaryFrameData(buffer)<br/>client/src/types/binaryProtocol.ts:405
        Note over WBP: detects stride (52=V3, 36=V2) via length%nodeSize
        WBP->>GW: graphWorker.processBinaryFrame(buffer) [Comlink]
        GW->>GW: processBinaryData(data)
        GW->>GW: parseBinaryFrameData(data) → frame.nodes
        GW->>GW: processFrameUpdates() → update targetPositions[]<br/>graph.worker.ts lib/binary-processor.ts
        GW->>GW: currentPositions.set(targetPositions!)
        GW->>GW: syncToSharedBuffer()
        GW->>SAB: Float32Array(SAB).set(currentPositions)<br/>stride-3 [x,y,z] per nodeIndex
        Note over SAB: Renderer reads SAB directly on RAF<br/>(getPositionsSync — zero copy)
    end
```

---

## 3. Settings Round-Trip

### Client settingsStore → autoSaveManager → settingsApi → Server → OptimizedSettingsActor

```mermaid
sequenceDiagram
    participant UI as React UI component
    participant SS as useSettingsStore (Zustand)<br/>client/src/store/settingsStore.ts
    participant ASM as autoSaveManager<br/>client/src/store/autoSaveManager.ts
    participant API as settingsApi / endpoints.ts<br/>client/src/api/settings/endpoints.ts
    participant IC as axios NIP-98 interceptor<br/>endpoints.ts:37
    participant SRV as PUT /api/settings/physics<br/>(or /rendering, /constraints, etc.)
    participant SA as OptimizedSettingsActor<br/>src/actors/optimized_settings_actor.rs
    participant GPU as GPUComputeActor

    UI->>SS: updateSettings(draft => { draft.visualisation.graphs.knowledge.physics.springK = 0.5 })
    SS->>SS: produce(partialSettings, updater) [immer]
    SS->>SS: findChangedPaths() → ["visualisation.graphs.knowledge.physics.springK"]
    SS->>SS: Zustand set({ partialSettings, settings, loadedPaths })
    SS->>ASM: autoSaveManager.queueChanges(batchChanges)<br/>(Map<path, value>)
    ASM->>ASM: store in pendingChanges Map
    Note over ASM: 500 ms debounce timer<br/>(prevents 60fps slider flooding)

    alt batchUpdate path (persistenceSlice)
        UI->>SS: batchUpdate([{path, value}, ...])
        SS->>SS: accumulate locally in partialSettings
        SS->>API: settingsApi.updateSettingsByPaths(updates)
    end

    ASM->>ASM: setTimeout fires → flushPendingChanges()
    ASM->>API: settingsApi.updateSettingsByPaths(updates)<br/>autoSaveManager.ts:124
    API->>API: route each path by prefix:<br/>visualisation.graphs.*.physics.* → updatePhysics()<br/>visualisation.rendering.* → updateRendering()<br/>qualityGates.* → updateQualityGates()<br/>nodeFilter.* → updateNodeFilter()<br/>constraints.* → updateConstraints()
    Note over API: Paths with no server endpoint<br/>are persisted to localStorage only<br/>(FINDING-5: silent local-only persistence)
    API->>IC: axios.put("/api/settings/physics", merged)
    IC->>IC: nostrAuth.signRequest() → NIP-98 token
    IC->>SRV: PUT /api/settings/physics<br/>Authorization: Nostr <token>
    SRV->>SRV: AuthenticatedUser extraction + NIP-98 verify
    SRV->>SA: GetSettings (snapshot)
    SA-->>SRV: AppFullSettings
    SRV->>SRV: normalize + merge + validate
    SRV->>SA: UpdateSettings { settings }
    SA->>SA: RwLock write, clear cache, settings.save() → YAML
    SA-->>SRV: Ok(())
    SRV->>GPU: UpdateSimulationParams (physics paths only)
    GPU-->>SRV: Ok(())
    SRV-->>API: 200 OK { physics echo }
    API-->>ASM: resolved
    Note over UI,GPU: No WebSocket broadcast of settings<br/>back to other clients.<br/>(FINDING-6: settings changes are not<br/>pushed to other connected sessions)
```

---

## 4. Auth Interface

### 4a. Nostr NIP-07 Browser Extension Login (Client Side)

```mermaid
sequenceDiagram
    participant U as User
    participant UI as React LoginButton
    participant NAS as nostrAuthService<br/>client/src/services/nostrAuthService.ts
    participant EXT as window.nostr (NIP-07 extension)
    participant LS as localStorage

    U->>UI: click "Login with Nostr"
    UI->>NAS: nostrAuth.login()
    NAS->>EXT: window.nostr.getPublicKey()
    EXT-->>NAS: pubkey (hex)
    NAS->>NAS: hexToNpub(pubkey) → bech32 npub
    NAS->>NAS: currentUser = { pubkey, npub, isPowerUser: false }
    NAS->>LS: localStorage.setItem('nostr_user', JSON)
    NAS->>NAS: notifyListeners({ authenticated: true, user })
    NAS-->>UI: AuthState { authenticated: true }
    Note over NAS: No session token issued.<br/>isPowerUser determined per-request<br/>by server from power-user allowlist.
```

### 4b. NIP-98 Per-Request Signing (REST)

```mermaid
sequenceDiagram
    participant API as axios request
    participant IC as NIP-98 interceptor<br/>endpoints.ts:37
    participant NAS as nostrAuthService
    participant EXT as window.nostr (NIP-07)
    participant SRV as actix-web handler
    participant AE as AuthenticatedUser extractor<br/>src/settings/auth_extractor.rs
    participant NS as NostrService<br/>src/services/nostr_service.rs

    API->>IC: intercept outgoing request
    IC->>NAS: nostrAuth.isAuthenticated()
    NAS-->>IC: true
    IC->>NAS: nostrAuth.signRequest(fullUrl, method, body)
    NAS->>NAS: build NIP-98 unsigned event:<br/>{ kind: 27235, tags: [["u", url], ["method", method],<br/>  ["payload", sha256(body)]] }
    NAS->>EXT: window.nostr.signEvent(unsignedEvent)
    EXT-->>NAS: signedEvent { id, sig, pubkey, ... }
    NAS->>NAS: base64url(JSON.stringify(signedEvent))
    NAS-->>IC: token string
    IC->>IC: config.headers.Authorization = "Nostr <token>"
    IC->>SRV: HTTP request with Authorization: Nostr <token>
    SRV->>AE: FromRequest::from_request()
    AE->>AE: detect "Nostr " prefix
    AE->>AE: reconstruct request URL from X-Forwarded-* / connection_info
    AE->>NS: verify_nip98_auth(header, url, method, None)
    NS->>NS: base64-decode token → AuthEvent
    NS->>NS: verify Schnorr signature (BIP-340)
    NS->>NS: check kind==27235, timestamp within TTL
    NS->>NS: check u-tag URL matches request URL
    NS->>NS: lookup pubkey in power_user_set
    NS-->>AE: Ok(AuthenticatedUser { pubkey, is_power_user })
    AE-->>SRV: user injected into handler args
```

### 4c. WebSocket NIP-98 Authentication

```mermaid
sequenceDiagram
    participant C as Client JS
    participant WS as WebSocket (/wss)
    participant SFH as SocketFlowServer actor
    participant NS as NostrService

    C->>WS: connect ws://host/wss (no auth on HTTP upgrade)
    Note over SFH: client_id assigned, pubkey=None initially
    C->>WS: send JSON { type: "authenticate",<br/>event: "<base64-NIP98-event>" }
    WS->>SFH: StreamHandler text message
    SFH->>SFH: handle_authenticate()<br/>src/handlers/socket_flow_handler/filter_auth.rs:7
    SFH->>NS: verify_nip98_auth("Nostr <event>", ws_url, "GET", None)
    NS-->>SFH: Ok(AuthenticatedUser { pubkey, is_power_user })
    SFH->>SFH: self.pubkey = Some(pubkey)<br/>self.is_power_user = power_user
    SFH->>SFH: ClientCoordinatorActor.do_send(AuthenticateClient { client_id, pubkey, ... })
    SFH-->>WS: JSON { type: "authenticate_success", pubkey, is_power_user }
    WS-->>C: authenticate_success
    Note over C,SFH: Post-auth, the drag/pin verbs (nodeDragStart/Update/End,<br/>nodeUnpin) all re-check act.pubkey.is_none() and reject<br/>unauthenticated clients (VULN-01 guard).<br/>Full GPU-pin flow → §5a; two-hand pinch is client-local.
```

### 4d. Solid Pod Login / Init

```mermaid
sequenceDiagram
    participant C as Client JS
    participant SPS as SolidPodService<br/>client/src/services/SolidPodService.ts
    participant LDP as solidPod/ldpClient.ts
    participant NAS as nostrAuthService
    participant EXT as window.nostr
    participant SRV as /solid/pods/init-nip98<br/>src/handlers/solid_proxy_handler.rs:1347
    participant SPRS as solid-pod-rs FsBackend
    participant WAC as WAC ACL engine

    C->>SPS: solidPodService.initPod()
    SPS->>LDP: fetchWithAuth("/solid/pods/init", { method: "POST" })
    LDP->>NAS: nostrAuth.signRequest(absoluteUrl, "POST", body)
    NAS->>EXT: window.nostr.signEvent(NIP-98 kind:27235)
    EXT-->>NAS: signedEvent
    NAS-->>LDP: "Nostr <base64>"
    LDP->>LDP: headers.set("Authorization", token)
    LDP->>SRV: POST /solid/pods/init (Bearer) or /pods/init-nip98 (NIP-98)
    Note over SRV: init_pod_nip98() extracts pubkey<br/>from NIP-98 Authorization header<br/>solid_proxy_handler.rs:1347
    SRV->>SRV: extract pubkey from NIP-98 event
    SRV->>SPRS: provision_pod(pubkey, data_root)
    SPRS->>SPRS: create pod directory at<br/>$SOLID_DATA_ROOT/<npub>/
    SPRS->>WAC: write ACL resources for pod
    SPRS-->>SRV: Ok(ProvisionPlan)
    SRV-->>LDP: 200 { pod_url, webid, created, structure }
    LDP-->>SPS: Response
    SPS-->>C: PodInitResult { success, podUrl, webId, created }
    Note over SRV: /solid/{tail:.*} LDP CRUD routes also<br/>registered (GET/PUT/POST/DELETE/PATCH/HEAD)<br/>for all subsequent pod operations
```

---

## 5. Interaction & Swarm Surfaces (2026-08 wave)

Diagrams for the capabilities landed by the Graph2VR / swarm / query-builder
wave (ADR-138 pinned-node mask + force-channel registry, ADR-139 Graph2VR
two-hand manipulation, ADR-140 XR agent swarm `0x23` beams, visual query
builder `/api/graph/query/pattern`). Message names verified against
`src/handlers/socket_flow_handler/message_routing.rs` and the client transport
(`client/src/features/graph/hooks/useGraphEventHandlers.ts`).

### 5a. Node Drag → GPU Pin → Broadcast → Unpin

Grab-and-place is server-authoritative (ADR-03 D7): the client no longer pins in
the graph worker; every drag verb is a JSON WebSocket message that drives a GPU
`PinNodePositions { pins, unpin }` and (on move/end) a `BroadcastNodePositions`
re-entry so co-present clients see the held node. `nodeDragEnd` leaves the node
**pinned in place**; only an explicit `nodeUnpin` (or the drag-timeout safety
net) releases it. Two-hand pinch is a separate **client-local** workspace
scale/rotate (Graph2VR, `interactionModes/vrArMode.ts` → `scaleWorkspace`) that
sends no server message and is shown as the dashed local branch.

```mermaid
sequenceDiagram
    participant U as Pointer / XR controller
    participant EH as useGraphEventHandlers<br/>client/src/features/graph/hooks/useGraphEventHandlers.ts
    participant WS as WebSocket (/wss)
    participant SFH as SocketFlowServer actor<br/>src/handlers/socket_flow_handler/position_updates.rs
    participant GPU as GPUComputeActor
    participant CCM as ClientCoordinatorActor
    participant OC as Other clients

    Note over U,EH: Two-hand pinch → scaleWorkspace()<br/>(client-local scale/rotate, no server msg)

    U->>EH: grab node (drag start)
    EH->>WS: sendMessage("nodeDragStart",<br/>{ nodeId, position })
    WS->>SFH: handle_node_drag_start() — pubkey guard
    SFH->>GPU: PinNodePositions { pins:[(id,[x,y,z])], unpin:[] }
    SFH-->>WS: { type: "nodeDragStartAck", data:{ nodeId } }

    loop drag move (throttled ~POSITION_UPDATE_THROTTLE_MS)
        U->>EH: pointer move
        EH->>WS: sendMessage("nodeDragUpdate",<br/>{ nodeId, position, timestamp })
        WS->>SFH: handle_node_drag_update() — pubkey guard
        SFH->>GPU: PinNodePositions { pins:[(id,pos)], unpin:[] }<br/>(re-pin at new position)
        SFH->>CCM: BroadcastNodePositions { positions }
        CCM->>OC: 52-byte V3/V5 position frame
    end

    U->>EH: release (drag end)
    EH->>WS: sendMessage("nodeDragEnd", { nodeId })
    WS->>SFH: handle_node_drag_end() — pubkey guard
    Note over SFH,GPU: Node stays PINNED at drop position.<br/>One final settle cycle relaxes neighbours.
    SFH->>CCM: BroadcastNodePositions { positions }
    CCM->>OC: settled frame
    SFH-->>WS: { type: "nodeDragEndAck", data:{ nodeId } }

    opt explicit release (or drag_timeout auto-unpin safety net)
        U->>EH: unpin gesture
        EH->>WS: sendMessage("nodeUnpin", { data:{ nodeId } })
        WS->>SFH: handle_node_unpin() — pubkey guard
        SFH->>GPU: PinNodePositions { pins:[], unpin:[id] }
        SFH-->>WS: { type: "nodeUnpinAck", data:{ nodeId } }
    end
```

### 5b. Agent-Beam `0x23 AGENT_ACTION` Broadcast → XR Beam Render

Embodied agent actions (ADR-140 / ADR-059 Phase 2b). The `agent_events` ingest
publishes each `AgentActionEnvelope` to the process-global `agent_events::hub`.
`AgentBeamActor` subscribes, coalesces a burst into one `BeamCoalescer`,
identity-blindly projects each envelope onto a flag-stamped action
(`project_action` → `to_binary_event`), and encodes the whole backlog as ONE
multi-action `0x23` frame (`encode_agent_actions`). It rides the existing binary
fan-out via `BroadcastAgentActionFrame` → `ClientCoordinatorActor.broadcast_to_all`
— no new client registry. On the client, the frame's lead tag
(`MessageType.AGENT_ACTION`) routes it to `handleAgentActionTagged`, which
decodes to transient beams pushed into `transientBeamStore`; `TransientBeamsLayer`
renders and ages them out.

```mermaid
sequenceDiagram
    participant ING as agent_events ingest<br/>(publishes AgentActionEnvelope)
    participant HUB as agent_events::hub<br/>(process-global broadcast)
    participant ABA as AgentBeamActor<br/>src/actors/agent_beam_actor.rs
    participant CO as BeamCoalescer<br/>(bounded, coalescing)
    participant CCM as ClientCoordinatorActor
    participant BP as store/websocket/binaryProtocol.ts
    participant TBS as transientBeamStore
    participant TBL as TransientBeamsLayer (R3F)

    ING->>HUB: publish(AgentActionEnvelope)
    HUB->>ABA: recv envelope(s) — burst
    loop drain burst (< MAX_COALESCE_PER_FLUSH)
        ABA->>CO: push(project_action(env))<br/>flag-stamped AgentActionEvent
    end
    ABA->>CO: encode_pending()
    CO-->>ABA: one multi-action 0x23 frame<br/>[0x23][count:u16][(len:u16)(event 15B+payload)]…
    ABA->>CCM: try_send(BroadcastAgentActionFrame(frame))
    Note over ABA,CCM: On full mailbox: HOLD backlog,<br/>coalesce into next frame (bounded backpressure)
    CCM->>CCM: broadcast_to_all(frame)<br/>→ per-client SendToClientBinary
    CCM->>BP: 0x23 binary frame (ArrayBuffer)
    BP->>BP: firstByte === MessageType.AGENT_ACTION<br/>→ handleAgentActionTagged()
    BP->>BP: decodeAgentActions(data.slice(1))
    BP->>TBS: pushTransientBeams(actions)
    Note over BP: also emit('agent-action') →<br/>live transcript + attention heat
    TBS->>TBS: admit beams (FIFO cap, per-beam TTL)
    TBL->>TBS: useTransientBeams(): beams + prune
    TBL->>TBL: render beam to targetNodeId,<br/>animate opacity over durationMs, prune expired
```

### 5c. Visual Query Builder — Mark → countOnly Preview → Execute → Planes

The visual query builder (ADR-141 wave) turns a marked sub-graph into a triple
pattern and enumerates its bindings over the live in-memory typed graph via
`POST /api/graph/query/pattern` (`src/handlers/api_handler/graph/mod.rs:1451`,
read-only, 120/min). The HUD first sends `countOnly:true` for a live
binding-count preview (no bindings materialised), then re-sends with
`countOnly:false` on execute to page the bindings, which the client renders as
semantic planes / highlights over the resolved node ids.

```mermaid
sequenceDiagram
    participant U as User (HUD / query builder)
    participant QB as Query builder (client)
    participant API as POST /api/graph/query/pattern
    participant QP as query_pattern handler<br/>src/handlers/api_handler/graph/mod.rs:1451
    participant MP as match_pattern (pure core)<br/>graph/mod.rs:1333
    participant GSA as GraphStateActor (snapshot)
    participant R as Semantic planes / highlight layer

    U->>QB: mark nodes/edges, build triples<br/>[src, edgeType, tgt] (concrete ids or vars)

    loop live preview on each edit
        QB->>API: triples + countOnly true
        API->>QP: web::Json<PatternQueryRequest>
        QP->>GSA: fetch_graph_snapshot()
        GSA-->>QP: GraphData { edges }
        QP->>MP: match_pattern(edges, triples, limit, count_only=true)
        MP-->>QP: PatternQueryResponse vars, bindingCount,<br/>truncated, bindings empty
        QP-->>QB: 200 bindingCount + truncated
        QB->>U: show live count ~N matches<br/>+ truncated floor flag
    end

    U->>QB: Execute
    QB->>API: triples + limit + countOnly false
    API->>QP: same path
    QP->>MP: match_pattern(..., count_only=false)
    MP-->>QP: PatternQueryResponse vars, bindingCount,<br/>truncated, bindings var-to-nodeId
    QP-->>QB: 200 vars + bindings (first page, capped at limit)
    QB->>R: project bindings to semantic planes<br/>over resolved node ids
    R->>U: highlighted planes / result set
    Note over QB,R: 400 on empty pattern or empty var name.<br/>bindingCount is a floor when truncated (scan cap)
```

---

## 6. Audit Findings

### F-1 — Unauthenticated Read on /api/graph/data

**Severity: MEDIUM**
**Location:** `src/handlers/api_handler/graph/mod.rs:148`
**Classification:** Unauth read path

`get_graph_data` does not extract `AuthenticatedUser` or `OptionalAuth`. The entire knowledge
graph (nodes, edges, positions, metadata) is returned without any identity check. (Write paths
are protected at the route layer, not the handler signature — see the F-7 correction below.)
The settings GET routes similarly serve unauthenticated
callers (`src/settings/api/settings_routes.rs` `get_settings` has no auth param) but apply
`redact_settings_secrets()` to strip API keys — however physics, rendering and all other
sub-routes (`get_physics_settings`, `get_rendering_settings`, etc.) are also unauthenticated
reads.

---

### F-2 — Duplicate Binary Encode Implementations (Protocol Version Disagreement)

**Severity: HIGH**
**Locations:**
- Server V3 frame module: `src/protocol/v3_frame.rs` — `V3_MAGIC = 0x5633_4630`, **28 bytes/node** (header 8 + nodes×28 + trailer 4)
- Server legacy encoder: `src/utils/binary_protocol.rs` — **52 bytes/node** (V3 extended: adds sssp@28, parent@32, cluster@36, anomaly@40, community@44, centrality@48)
- Client decoder: `client/src/types/binaryProtocol.ts:76` — `BINARY_NODE_SIZE_V3 = 52`, auto-detected by `length % 52`
- Client BinaryWebSocketProtocol: `client/src/services/binaryProtocol/frameTypes.ts:8` — `PROTOCOL_VERSION = PROTOCOL_V4 = 4`
**Classification:** Duplicate encoder / protocol constant disagreement

The `src/protocol/v3_frame.rs` module documents itself as "the single source of encode/decode
logic" for the broadcast path (28-byte NodeRow). The actual position subscription loop
(`position_updates.rs:555`) calls `binary_protocol::encode_node_data_extended_with_sssp` which
produces **52-byte** records prefixed by a 1-byte protocol-version header (`buffer.push(3)`,
`src/utils/binary_protocol.rs`; V5 frames wrap the same body with `[version=5][8-byte seq]`).
The client's `validateBinaryData` checks `version byte == 3 or 5`
(`store/websocket/binaryProtocol.ts:198`) and the decoder reads the version byte first
(`types/binaryProtocol.ts:165`); the `length % nodeSize` heuristic is only a fallback for
unknown version bytes. The `BinaryWebSocketProtocol` class in
`services/BinaryWebSocketProtocol.ts` uses a different 6-byte V4 header format
(`MESSAGE_HEADER_SIZE=6`, `PROTOCOL_VERSION=V4=4`) which is never emitted by the server's
position subscription path. The V3 frame format in `v3_frame.rs` (28 bytes) is never decoded on
the client. Three incompatible wire formats coexist; the one actually flowing is the 52-byte
legacy format.

---

### F-3 — /api/settings/physics GET Has No Server-Side Route Caller

**Severity: LOW**
**Location:** `client/src/api/settings/endpoints.ts:85` calls `GET /api/settings/physics`
then `PUT /api/settings/physics`; `src/settings/api/settings_routes.rs:1314` registers both.
**Classification:** Route with no client caller (for other routes)

The client `updatePhysics()` performs a GET-then-PUT pattern (read-modify-write) as a workaround
for partial updates. This means every physics slider change issues two requests. The sub-routes
`/api/settings/node-filter`, `/api/settings/constraints`, `/api/settings/quality-gates`, and
`/api/settings/rendering` follow the same pattern. The `updateSettingsByPaths` path in
`endpoints.ts:364` also dispatches through these individual PUT endpoints, not through the
unified `POST /api/settings` bulk endpoint registered at `settings_handler/routes.rs:27`.
The unified POST endpoint is therefore a **dead server route** from the JS client's perspective:
no client caller routes through it.

---

### F-4 — Client Protocol Version Constant V4 Never Emitted by Server

**Severity: MEDIUM**
**Location:** `client/src/services/binaryProtocol/frameTypes.ts:8` (`PROTOCOL_VERSION = PROTOCOL_V4 = 4`), `SUPPORTED_PROTOCOLS = [V2, V3, V4]`
**Classification:** Protocol constant disagreement

The `BinaryWebSocketProtocol.createMessage()` stamps a V4 header (1-byte type, 1-byte version=4,
4-byte payloadLength). The server `binary_protocol.rs` encoder does not produce this header.
Server position frames begin with a 1-byte protocol version (3 or 5), then 52-byte node
records starting with `u32 node_id`. The V4 client header parser
(`parseHeader`) and the server's legacy encoder are talking different formats on the same socket.
The client's `processBinaryData` path in `store/websocket/binaryProtocol.ts` short-circuits to
`parseBinaryNodeData` / `parseBinaryFrameData` (which understands the raw 52-byte legacy format),
so the `BinaryWebSocketProtocol` class is effectively unused on the receive path — but its
`createMessage` is used when the **client sends** binary (BroadcastAck, voice). The server decode
in `binary_protocol.rs:BinaryProtocol::decode_message` uses its own message type byte scheme and
may not align with the client V4 header for the BroadcastAck path.

---

### F-5 — Settings Paths With No Server Endpoint Silently Fall Through to localStorage Only

**Severity: MEDIUM**
**Location:** `client/src/api/settings/endpoints.ts:350`
**Classification:** Client API calling absent route

The `updateSettingByPath` function has an explicit `else` branch:

```
logger.debug(`Path "${path}" persisted to localStorage only (no server endpoint)`);
```

Paths not matching `visualisation.graphs.*.physics.*`, `visualisation.rendering.*`,
`qualityGates.*`, `nodeFilter.*`, `constraints.*`, or `isVisualSettingsPath()` are never sent
to the server. This includes at minimum: `clientTweening.*`, `xr.*`, `system.*`,
`nostr.*`, and any new feature paths. The client settingsStore persists these via zustand
`persist` to `localStorage` under key `graph-viz-settings-v2`. On page reload these values are
hydrated from localStorage but never validated against the server. Server-side writes (e.g.
admin reset via `POST /api/settings/reset`) will not propagate the client-only paths back to the
browser.

---

### F-6 — Settings Updates Not Broadcast to Other Connected Sessions

**Severity: MEDIUM**
**Location:** `src/actors/optimized_settings_actor.rs:596` (`update_settings`); `src/settings/api/settings_routes.rs:364`
**Classification:** Missing cross-session sync

When `update_physics_settings` (or any write handler) calls `UpdateSettings` on
`OptimizedSettingsActor`, the actor writes the in-memory RwLock and saves to YAML. There is no
`BroadcastMessage` to `ClientCoordinatorActor` to push `settingsChanged` to all connected WebSocket
clients. Other browser tabs or co-present users will have stale physics/rendering settings until
they reload. The `BroadcastMessage` actor message exists (`src/actors/messages.rs`) and is used for
other scenarios, but not wired after settings writes. Compare: the drag path explicitly calls
`BroadcastNodePositions` after every settle cycle.

---

### F-7 — WITHDRAWN (queen verification): update_graph/refresh_graph ARE auth-gated at the route layer

**Severity: none (false positive)**
**Location:** `src/handlers/api_handler/graph/mod.rs:648–660`
**Classification:** verified-protected

Initial reading of the handler signatures suggested missing auth, but the protection lives in
the route registration, not the extractors: `/update` is wrapped with
`RequireAuth::power_user()` (bulk reload is a privileged operation) and `/refresh` with
`RequireAuth::authenticated()` (read-back). Verified 2026-06-11 against `graph/mod.rs:652,659`.
Only the unauthenticated **read** path (F-1) stands.

---

### F-8 — WebSocket Drag Handlers Require pubkey but Subscribe Does Not

**Severity: LOW**
**Location:** `src/handlers/socket_flow_handler/position_updates.rs:742` (nodeDragStart VULN-01 guard)
**Classification:** Inconsistent auth enforcement

`handle_node_drag_start/update/end` check `act.pubkey.is_none()` and reject unauthenticated
clients. `handle_subscribe_position_updates` has no such guard. An unauthenticated client can
subscribe and receive a full position stream. This is consistent with the unauthenticated REST
read path (F-1) but may be surprising: position data leaks to pre-auth connections.

---

### F-9 — Filter Persistence Phase 2 (SQLite) Not Yet Implemented

**Severity: LOW**
**Location:** `src/handlers/socket_flow_handler/filter_auth.rs:237`; `src/settings/api/settings_routes.rs:21`
**Classification:** Dead code / incomplete feature

The `filter_update` WebSocket handler stores filters in-memory on the `ClientCoordinatorActor`
only. The comment at `filter_auth.rs:241` reads: "Filter is applied in-memory only until Phase 2
SQLite migration is complete." Similarly `UserFilter` in `settings_routes.rs:21` carries a
`todo!("Phase 2: migrate UserFilter to SqliteSettingsRepository / SQLite schema")`. Filters are
lost on server restart.

---

### F-10 — Solid Routes Gated on solid-pod-embed Feature Flag; Stub Returns 503

**Severity: INFO**
**Location:** `src/handlers/solid_proxy_handler.rs:1874` (stub `configure_routes`)
**Classification:** Conditional route

Without the `solid-pod-embed` Cargo feature, all `/solid/*` routes return 503. The client
`SolidPodService.initPod()` makes no feature-flag check before calling `/solid/pods/init`.
A build without `solid-pod-embed` will surface 503 errors at runtime with no client-side guard.

---

## Wire Format Reference

### Server Position Broadcast (actual on-wire, "V3 extended")
`src/utils/binary_protocol.rs` — `encode_node_data_extended_with_sssp()`

```
Frame header: 1 byte protocol version (3 = V3; 5 = V5, followed by an 8-byte
broadcast-sequence u64 before the node records).

Per-node (52 bytes):
  Offset  Size  Field
    0      4    node_id u32 (bits 31-26 = type flags: AGENT=0x80000000, KNOWLEDGE=0x40000000,
                             OntClass=0x04000000, OntInd=0x08000000, OntProp=0x10000000)
    4     12    pos [f32; 3]  (x, y, z)
   16     12    vel [f32; 3]  (vx, vy, vz)
   28      4    sssp_distance f32
   32      4    sssp_parent   i32
   36      4    cluster_id    u32
   40      4    anomaly_score f32
   44      4    community_id  u32
   48      4    centrality    f32  (ADR-031 D2)
```

Client constants at `client/src/types/binaryProtocol.ts:72–91` match this layout exactly.

### V3 Frame Format (visionclaw-protocol crate — UNUSED on position path)
`src/protocol/v3_frame.rs` — `V3_MAGIC = 0x5633_4630`

```
Header (8 bytes):
  Offset  Size  Field
    0      4    magic u32 = 0x5633_4630 (LE: bytes [0x30,0x46,0x33,0x56])
    4      4    frame_id u32 (monotonic)

Per-node (28 bytes):
  Offset  Size  Field
    0      4    node_id u32
    4     12    pos [f32; 3]
   16     12    vel [f32; 3]

Trailer (4 bytes):
  Offset  Size  Field
    0      4    node_count u32
```

Total: `8 + 28*N + 4` bytes.  Client has no decoder for this format.

### BinaryWebSocketProtocol V4 Header (client-emitted messages only)
`client/src/services/binaryProtocol/frameTypes.ts:175`

```
Header (6 bytes for non-GRAPH_UPDATE, 7 bytes for GRAPH_UPDATE):
  Offset  Size  Field
    0      1    MessageType u8
    1      1    version u8 = 4
    2      4    payloadLength u32 (LE)
   [6]     1    graphTypeFlag u8 (GRAPH_UPDATE only)
```

Server decode in `src/utils/binary_protocol.rs:BinaryProtocol::decode_message` has its own
message-type enum that may not align with the client MessageType values.
