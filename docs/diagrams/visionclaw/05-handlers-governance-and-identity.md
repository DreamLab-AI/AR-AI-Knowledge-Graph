---
id: VC-05
title: Governance and identity handler families
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/IDENTITY-authority-chain.md
adrs: [ADR-2006, ADR-2010, ADR-2011, ADR-2013, ADR-2016]
sources:
  - src/handlers/admin_rbac_handler.rs
  - src/services/role_store.rs
  - src/handlers/admin_sync_handler.rs
  - src/handlers/nostr_handler.rs
  - src/services/nostr_service.rs
  - src/handlers/presence_handler.rs
  - src/actors/presence_actor.rs
  - src/handlers/broker_inbox_handler.rs
  - src/handlers/decision_handler.rs
  - src/handlers/enrichment_proposals_handler.rs
  - src/handlers/briefing_handler.rs
  - src/services/briefing_service.rs
  - src/handlers/insight_loop_handler.rs
  - src/services/insight_loop.rs
  - src/handlers/memory_flash_handler.rs
  - src/handlers/mcp_relay_handler.rs
  - src/services/mcp_relay_manager.rs
  - src/handlers/multi_mcp_websocket_handler.rs
  - src/actors/multi_mcp_visualization_actor.rs
  - src/services/multi_mcp_agent_discovery.rs
  - src/handlers/ontology_agent_handler.rs
  - src/services/proposal_spine.rs
  - src/handlers/solid_proxy_handler.rs
  - src/handlers/speech_socket_handler.rs
  - src/actors/elevation_actor.rs
  - src/actors/decision_elevation_actor.rs
  - src/domain/broker/mod.rs
  - src/domain/broker/broker_case.rs
  - src/domain/broker/broker_decision.rs
  - src/domain/broker/precedent_registry.rs
  - src/services/acsp/mod.rs
  - src/services/acsp/client.rs
  - src/services/nostr_bridge.rs
  - src/services/decision_service.rs
  - src/handlers/ingest_writeback_handler.rs
  - src/services/ontology_mutation_service.rs
  - src/services/ontology_query_service.rs
  - src/uri/mod.rs
  - src/utils/auth.rs
  - src/utils/nip98.rs
  - src/main.rs
  - docs/adr/ADR-2006-acsp-human-approval.md
  - agentbox/management-api/routes/broker-bridge.js
  - src/config/feature_access.rs
  - src/middleware/rbac_gate.rs
  - src/services/management_api_client.rs
  - src/services/nostr_bead_publisher.rs
verified_commit: 7a20db228
---

## VC-05.1 `admin_rbac_handler` — whoami / list / assign / revoke (ADR-2010)
```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant H as admin_rbac_handler<br/>src/handlers/admin_rbac_handler.rs:284 scope /admin/rbac (registration see VC-01.13)
    participant VA as verify_access/verify_admin<br/>src/utils/auth.rs
    participant RS as RoleStore<br/>src/services/role_store.rs:408 assign_checked, :500 remove_checked

    Note over H: routes — GET /whoami :288, GET /users :289,<br/>PUT /users/{pubkey}/role :290, DELETE /users/{pubkey}/role :291
    C->>H: GET /whoami
    H->>VA: verify_access(Authenticated) :81
    alt Err
        VA-->>C: deny response
    else Ok(pubkey)
        H->>RS: effective_role(pubkey, is_power_user) :87
        RS-->>C: 200 {pubkey, role, is_power_user}
    end
    C->>H: GET /users
    H->>VA: verify_admin(req, nostr) :101
    alt not Admin/Owner
        VA-->>C: 403
    else Ok
        H->>RS: list() :108
        RS-->>C: 200 {users: [UserRoleView]}
    end
    C->>H: PUT /users/{pubkey}/role {role} :123
    H->>VA: verify_admin :129
    alt not Admin/Owner
        VA-->>C: 403
    else Ok(caller)
        H->>H: UserRole::parse(body.role) :134
        alt unknown role string
            H-->>C: 400 "unknown role '{r}'"
        else parsed
            critical RS::assign_checked single tx — role_store.rs:421-489 (full sequence VC-03.8)
                H->>RS: assign_checked(target, new_role, CallerAuthority) :157
                RS-->>H: Ok(role) or RoleStoreError
            end
            alt Ok
                H-->>C: 200 {pubkey, role, assigned_by:caller}
            else Forbidden/LastOwner/CallerAuthorityChanged/InvalidPubkey
                H-->>C: role_error_response :240 — 403/409/400/500
            end
        end
    end
    C->>H: DELETE /users/{pubkey}/role
    H->>VA: verify_admin :181
    H->>RS: remove_checked(target, target_is_power, authority) :197
    RS-->>H: RemovalOutcome{had_explicit_role, previous_role, effective_after, authority_reduced}
    Note over H: ADR-2010 — removal is NOT revocation (:202) — response carries<br/>"note" warning the target may still hold effective_after by ambient default
    H-->>C: 200 {reverted_to, access_revoked:authority_reduced, note}
```

## VC-05.2 `admin_sync_handler` — trigger_sync with the 600s timeout override
```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant TO as TimeoutMiddleware<br/>src/main.rs:975-978 override "/api/admin/sync"->600s
    participant RG as RbacGate<br/>Admin required (any /api/admin/* method, see VC-03.6)
    participant H as admin_sync_handler::trigger_sync<br/>src/handlers/admin_sync_handler.rs:115

    C->>TO: POST /api/admin/sync
    Note over TO: TimeoutConfig::with_override — this path alone gets 600s<br/>instead of the default 30s (see VC-03.13)
    TO->>RG: pass through inner service chain
    alt caller not Admin/Owner
        RG-->>C: 401/403 (RbacGate deny, ["api","admin"] prefix -> Admin for every method)
    else Ok
        RG->>H: trigger_sync(req)
        H-->>C: sync result
    end
    alt sync exceeds 600s
        TO-->>C: 504 ErrorGatewayTimeout "Request to /api/admin/sync timed out after 600000ms"
    end
    Note over RG: ADR-2011 — ONE central RbacGate covers the whole /api scope, whole-segment<br/>match on ["api","admin"] requires Admin for EVERY method, not gated per-handler
```

## VC-05.3 `nostr_handler` — session lifecycle routes (scope `/auth/nostr`)
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as nostr_handler::config<br/>src/handlers/nostr_handler.rs:53
    participant NS as NostrService<br/>src/services/nostr_service.rs
    participant FA as FeatureAccess<br/>src/config/feature_access.rs

    Note over H: this scope is on the public allowlist (api/auth prefix, RbacGate rbac_gate.rs:56-61)<br/>— no RbacGate check runs before these handlers, each handler does its own session check
    C->>H: POST empty-path login :54 -> login :125
    H->>NS: verify_auth_event(AuthEvent) :131
    alt Err(InvalidSignature)
        NS-->>C: 401 "Invalid signature"
    else Err(other)
        NS-->>C: 500 "Authentication error: {e}"
    else Ok(user)
        H->>H: expires_at = user.last_seen + AUTH_TOKEN_EXPIRY (env, :134, default 3600)
        H->>FA: get_available_features(pubkey)
        H-->>C: 200 AuthResponse{user,token:session_token,expires_at,features}
    end
    C->>H: DELETE empty-path logout :55 -> logout :161
    H->>NS: validate_session(pubkey,token) :164
    alt invalid
        NS-->>C: 401 "Invalid session"
    else valid
        H->>NS: logout(pubkey) :175
        NS-->>C: 200 "Logged out successfully"
    end
    C->>H: POST /verify :56 -> verify :182
    H->>NS: validate_session(pubkey,token) :185
    H-->>C: 200 VerifyResponse{valid, user?, features}
    C->>H: POST /refresh :57 -> refresh :216
    H->>NS: validate_session then refresh_session(pubkey) :227
    alt invalid session
        NS-->>C: 401 "Invalid session"
    else Ok(new_token)
        H-->>C: 200 AuthResponse{token:new_token, expires_at (AUTH_TOKEN_EXPIRY, :234), features}
    else Err
        H-->>C: 500 "Session refresh error: {e}"
    end
    C->>H: GET /power-user-status :60 -> check_power_user_status :66
    H->>H: pubkey = X-Nostr-Pubkey header
    alt header missing
        H-->>C: 400 "Missing Nostr pubkey"
    end
    H-->>C: 200 (feature_access power-user check)
    C->>H: GET /features :61 / GET /features/{feature} :62
    H->>FA: get_available_features(pubkey) / has_feature_access(pubkey,feature)
    H-->>C: 200 {features} / {has_access}
    C->>H: POST /api-keys :58 -> update_api_keys :259 / GET /api-keys :59 -> get_api_keys :287
    Note over H: DIVERGENCE — route "/api-keys" (:58-59) has NO {pubkey} path segment,<br/>but update_api_keys/get_api_keys both take web::Path~String~ as the pubkey<br/>parameter (:262,:290) — actix Path extraction against a pattern with zero<br/>dynamic segments fails, these two routes cannot succeed as registered
    opt get_api_keys reached
        H->>NS: validate_session(pubkey, Bearer token) :309
        alt invalid/missing token
            NS-->>C: 401
        end
    end
```

## VC-05.4 `presence_handler` — PRD-008 `/ws/presence` handshake (T-WS-1)
```mermaid
sequenceDiagram
    autonumber
    participant C as XR client<br/>Godot
    participant WS as PresenceSession<br/>src/handlers/presence_handler.rs:135 ws_presence :501 (route src/main.rs:1039)
    participant NC as SeenNonces LRU<br/>presence_handler.rs:51 cap 4096
    participant IV as IdentityVerifier<br/>visionclaw_xr_presence::ports
    participant REG as PresenceRoomRegistry<br/>DashMap~String,Addr~PresenceActor~~
    participant PA as PresenceActor<br/>src/actors/presence_actor.rs:247

    WS->>WS: Actor::started — send_challenge :162 {type:challenge,nonce,ts}<br/>run_interval HEARTBEAT_INTERVAL=15s :430-433
    C->>WS: text {type:auth, did, signature, room_id, metadata, ts?}
    WS->>WS: handle_auth :188 — must be in SessionPhase::Challenged
    alt wrong phase
        WS-->>C: close 4400 "auth in wrong phase"
    end
    opt client ts present
        WS->>WS: skew = |now_us - client_ts|
        alt skew > MAX_HANDSHAKE_SKEW_US=30_000_000us
            WS-->>C: close 4401 "stale handshake timestamp"
        end
    end
    WS->>IV: verify_signed_challenge(SignedChallenge{nonce,timestamp_us,claimed_pubkey_hex,signature_hex})
    alt Err or verified.as_str() != did
        IV-->>WS: error / mismatch
        WS-->>C: close 4401 "auth: {e}" / "did/signature mismatch"
    else Ok(verified)
        WS->>NC: seen.put(nonce,()) :253
        alt nonce already present
            NC-->>WS: Some (replay)
            WS-->>C: close 4401 "replayed challenge"
        end
        WS->>WS: RoomId::parse(room_id)
        alt invalid room_id
            WS-->>C: close 4400 "room_id: {e}"
        end
        WS->>REG: entry(room).or_insert_with(PresenceActor::new(room).start())
        Note over REG: dead-Addr guard :279-282 — if entry.connected()==false<br/>(room previously emptied), replace with a fresh actor before join
        WS->>PA: JoinRoom{did,metadata,frame_recipient,event_recipient}
        alt Ok(Err(JoinRejection::DuplicateMember)) or other rejection
            PA-->>WS: JoinRejection
            WS-->>C: close 4400 duplicate member / rejection.to_string()
        else Ok(Ok(JoinAck{avatar_id,members}))
            PA-->>WS: JoinAck
            WS-->>C: text {type:joined, room_id, avatar_id, members:[MemberDescriptor]}
            WS->>WS: phase = SessionPhase::Joined{avatar_id, room_addr}
        end
    end
    loop every 15s heartbeat
        WS->>C: ws ping
        alt no pong/ping for > 30s (2x interval)
            WS->>WS: ctx.stop()
        end
        WS->>WS: enforce_handshake_deadline — Challenged for >10s HANDSHAKE_TIMEOUT
        alt still Challenged after 10s
            WS-->>C: close 4401 "handshake timeout"
        end
    end
    Note over WS: Actor::stopping :429 — if Joined, room_addr.do_send(LeaveRoom{avatar_id})
```

## VC-05.5 `presence_handler`/`PresenceActor` — binary pose frame ingest and broadcast
```mermaid
sequenceDiagram
    autonumber
    participant C as XR client (Joined)
    participant WS as PresenceSession::handle_pose_frame<br/>presence_handler.rs:354
    participant PA as PresenceActor::handle_ingest<br/>src/actors/presence_actor.rs:751
    participant HR as configured_hand_reach_m<br/>presence_actor.rs:44 env PRESENCE_HAND_REACH_M

    Note over WS: opcode 0x43 OPCODE_AVATAR_POSE (wire::PREAMBLE_OPCODE, presence_handler.rs:5,197)<br/>envelope: [opcode u8][len u16][room_hash 16B][avatar_id_len u8][avatar_id][payload]
    C->>WS: binary frame (0x43 sibling envelope)
    alt phase not Joined
        WS-->>C: close 4400 "binary before auth"
    end
    WS->>WS: check_rate_limit — sliding 1s window, RATE_LIMIT_FRAMES_PER_SEC=120 :173-186
    alt over limit
        WS-->>C: close 4429 "rate limit exceeded"
    else within limit
        WS->>PA: IngestPose{avatar_id, frame_bytes}
        PA->>PA: not a room member? -> ValidationFailed
        PA->>PA: wire::decode(frame_bytes)
        alt decode error
            PA->>PA: record_violation — VIOLATION_KICK_THRESHOLD=10 within VIOLATION_WINDOW=1s
            alt threshold exceeded
                PA-->>WS: IngestOutcome::Kick("decode-violations exceeded: {e}")
                WS-->>C: close 4400
            else
                PA-->>WS: IngestOutcome::Decode(e)
            end
        else decoded
            alt decoded.avatar_id != msg.avatar_id
                PA-->>WS: Kick("avatar-id spoofing") after violation threshold
            else decoded.room_hash != room_id.wire_hash()
                PA-->>WS: ValidationFailed("room_hash mismatch")
            else run_validators fails (HR: hand_reach_m gate, joint_anatomy, velocity_gate)
                PA-->>WS: ValidationFailed(e) or Kick after threshold
            else Ok
                PA->>PA: room.update_pose(avatar_id, frame)
                PA->>PA: strip envelope, pending_poses.insert(avatar_id, payload)
                PA->>PA: dispatch_broadcast(avatar_id)
                PA-->>WS: IngestOutcome::Accepted
                PA->>C: BroadcastFrame{bytes} to every Subscriber.frame_recipient (fan-out)
            end
        end
    end
    Note over PA: RoomEventEnvelope (JSON channel, not 0x43) — AvatarJoined{local_id},<br/>AvatarLeft, AgentPresenceExpired{local_id} (ADR-2020 co-presence retirement,<br/>SweepStaleAgentPresence handler :819) — distinct from IngestAgentPresence 0x44 deltas
```

## VC-05.6 `decision_handler` — record (authed, append-only) / trace (anon, bounded)
```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant SC as scope /decisions<br/>src/handlers/decision_handler.rs:321-331
    participant RA as RequireAuth::authenticated<br/>wraps nested scope /record only :326
    participant H as decision_handler<br/>record_decision :145, trace_decision :231
    participant DS as DecisionService::record_decision<br/>src/services/decision_service.rs:546
    participant SP as proposal_spine::governed_commit<br/>src/services/proposal_spine.rs (Stage3 commit_quads :517)

    C->>SC: POST /decisions/record {summary,rationale,caused,precedent_for,...}
    SC->>RA: nested scope wrap (PRD-022 W-B / ADR-048)
    alt not authenticated
        RA-->>C: 401 (RequireAuth deny)
    else Ok(AuthenticatedUser)
        Note over H: INVARIANT ADR-048 Attribution — deciding principal is auth.pubkey,<br/>NEVER a body field, a caller cannot self-assert another agent's decision
        H->>DS: record_decision(&auth.pubkey, DecisionInput, idempotency_key, signature)
        DS->>SP: governed_commit — conflict gate, Whelk consistency, provenance append, single tx
        critical commit_quads — ONE oxigraph store.transaction, proposal_spine.rs:517-533
            SP->>SP: for q in provenance_quads: tx.insert(q) — INSERT-ONLY, no DELETE/DROP/CLEAR
            SP->>SP: for q in asserted_quads: tx.insert(q)
        end
        Note over SP: ADR-2016 — GRAPH_PROVENANCE append-only holds on this path too,<br/>governed_commit funnels through commit_quads which issues only tx.insert
        alt Ok(success)
            DS-->>H: DecisionSuccess{decision_urn, replayed, quads_written, receipt, gates}
            H-->>C: 200 {success:true, decision_urn, quads_written, replayed, receipt, gates}
        else Err CONFLICT_BLOCKED_PREFIX
            H-->>C: 409 {error:conflict_blocked, blockingConflicts, preExisting, conflictReport}
        else Err IDEMPOTENCY_CONFLICT_PREFIX
            H-->>C: 409 {error:idempotency_conflict, message}
        else Err ENVELOPE_REJECTED_PREFIX
            H-->>C: 403 {error:envelope_rejected, message}
        else Err other
            H-->>C: 500 "Decision write failed"
        end
    end
    C->>H: GET /decisions/{urn}/trace?direction&max_depth (anon, no RequireAuth)
    H->>H: max_depth = min(query.max_depth, MAX_DEPTH_CAP=64) :120,238
    loop up to max_depth SPARQL frontier expansions
        H->>H: direct_links_query(frontier, direction) — DIRECT dl:caused/dl:precedentFor only
        H->>H: store.query(sparql) — never a transitive property path
    end
    H->>H: bounded_bfs(root, max_depth, adjacency)
    H-->>C: 200 TraceResponse{derived:true, root, direction, max_depth, hops}
    Note over H: response is stamped derived:true — reachability, never a materialised<br/>or Whelk-classified transitive edge (ADR-048 Graph placement)
```

## VC-05.7 shared enrichment-decide core (ADR-2006 / ADR-130 Decision 2) — three callers, one path
```mermaid
sequenceDiagram
    autonumber
    participant AB as agentbox broker-bridge<br/>X-Agent-Key service caller
    participant OP as power-user operator<br/>via broker_inbox_handler scope
    participant GB as git-bridge write-back<br/>agentbox management-api
    participant D as decide / decide_as_operator<br/>src/handlers/enrichment_proposals_handler.rs:321,351
    participant WB as ingest_writeback_handler::writeback<br/>src/handlers/ingest_writeback_handler.rs:75
    participant AD as apply_decision (shared core)<br/>enrichment_proposals_handler.rs:340
    participant OK as DecisionOrchestrator<br/>src/domain/broker/broker_decision.rs (ADR-130 Decision 2 kernel)
    participant OX as OntologyRepository::append_derived_summary
    participant FR as AcspClient::publish<br/>src/services/acsp/mod.rs kind 31403

    AB->>D: POST /api/enrichment-proposals/{id}/decide (X-Agent-Key)
    D->>D: require_agent_key — header must equal VISIONCLAW_AGENT_KEY (env, :132)
    alt key invalid/missing
        D-->>AB: 401 {success:false, error:"Invalid or missing X-Agent-Key header"}
    end
    OP->>D: POST /api/broker/cases/{id}/decide (decide_as_operator, power_user() scope)
    D->>D: if body has no broker_pubkey and user.pubkey is canonical hex -> attribute to operator (HITL)
    GB->>WB: POST /api/ingest/writeback {decision:{caseId,decision,...}} (no X-Agent-Key required, GOV-4)
    WB->>AD: apply_decision(case_id, BrokerDecisionRequest, state, client_coordinator)
    D->>AD: apply_decision(case_id, body, state, client_coordinator) (both agent + operator routes)
    rect rgb(225,245,225)
    Note over AD: single decision core — validate, mint provenance, persist, write-back, broadcast, project
    AD->>AD: record_decision(case_id, req) :162 — trim outcome, classify(outcome)
    alt empty case_id or empty outcome
        AD-->>D: Err "empty case id"/"empty decision outcome" -> 400
    end
    AD->>AD: attribution — 64-hex broker_pubkey -> did_nostr+kg URN, else unattributed
    AD->>AD: activity_urn = uri::execution(content-addressed over case_id,outcome,pubkey) — idempotent
    opt repo.get(case_id) is None
        AD->>AD: is_new_case=true, create pending StoredProposal stub
    end
    AD->>AD: repo.record_decision(StoredDecision) — atomic INSERT decision + UPDATE proposal.status
    Note over AD: ADR-2006 — REST decisions carry decision_event_id:None (:398) — no signed<br/>kind-31403 correlation is stored against the persisted row
    alt persist Err
        AD-->>D: 500 "failed to persist decision: {e}"
    end
    alt writeback_triggered AND attributed
        AD->>OX: append_derived_summary(owner_did, activity_urn, summary_triples_for(record))
        alt Ok
            AD->>AD: repo.mark_writeback_committed(case_id, activity_urn, now_ms)
        else Err
            AD->>AD: writeback_committed=false, warn DEGRADED
        end
    end
    AD->>D: BroadcastMessage{enrichment_decision} to client_coordinator (all WS clients)
    AD->>OK: derive_kernel_decision — DecisionOrchestrator::decide(case, activity_urn, outcome, broker, reasoning)
    alt kernel invariant note (e.g. no-self-review)
        OK-->>AD: Err(e) -> degrade to raw outcome string, warn (record-don't-reject)
    else Ok(report)
        OK-->>AD: kernel_action, share_plan
    end
    opt is_new_case
        AD->>AD: broker_events::broadcast_new_case(client_coordinator, case_id, case_id, knowledge_enrichment)
    end
    AD->>AD: broker_events::broadcast_case_decided(client_coordinator, case_id, activity_urn, kernel_action, share_plan)
    alt state.acsp_client Some
        AD->>FR: publish(build_action_response(case_id, kernel_action, reasoning)) kind 31403
        alt Ok(event_id)
            FR-->>AD: forum_projection="published"
        else Err
            FR-->>AD: forum_projection="failed" (DEGRADED, warn)
        end
    else None configured
        AD->>AD: forum_projection="skipped" (DEGRADED — needs FORUM_RELAY_URL+ACSP_PANEL_NOSTR_PRIVKEY)
    end
    AD->>AD: state.liveness_harness.observe(CANARY_REC2_CASE, evidence) — REC-2 canary, real traffic only
    end
    AD-->>D: DecideResponse{success:true, decision, attributed, writeback_triggered, writeback_committed, activity_urn, proposal_urn}
    D-->>AB: 200 DecideResponse
    D-->>OP: 200 DecideResponse
    AD-->>WB: 200 DecideResponse
```

## VC-05.8 `broker_inbox_handler` — dedicated privileged read scope (WS-12)
```mermaid
sequenceDiagram
    autonumber
    participant C as agentbox broker-bridge<br/>management-api/routes/broker-bridge.js
    participant SC as scope /broker<br/>src/handlers/broker_inbox_handler.rs:164 wrap RequireAuth::power_user() :165
    participant H as broker_inbox_handler<br/>inbox :133, case_by_id :144
    participant ST as enrichment_proposals_handler::store<br/>same durable store as WS-9, no second store

    Note over SC: doc comment :157-161 — "Mounted as a dedicated web::scope(broker) so the<br/>privileged RequireAuth::power_user() middleware wraps exactly these read routes<br/>and nothing else, mirroring the isolated privileged scope at ontology_handler.rs:913-918"
    C->>SC: GET /api/broker/inbox (X-Agent-Key or power-user session)
    SC->>SC: RequireAuth::power_user()
    alt not power user
        SC-->>C: 401/403 deny
    else Ok
        SC->>H: inbox() :166
        H->>ST: store::all()
        ST-->>H: Vec~EnrichmentProposal~
        H->>H: project each into BrokerCase{id,category:"knowledge_enrichment",status,metadata}
        H-->>C: 200 {cases:[BrokerCase], total} (bridge shape, broker-bridge.js:233)
    end
    C->>SC: GET /api/broker/cases/{id} :167
    SC->>H: case_by_id(id) :144
    H->>ST: store::get(id)
    alt Some(p)
        ST-->>C: 200 BrokerCase::from(&p)
    else None
        ST-->>C: 404 {error:"not-found", message:"no broker case with id {id}"}
    end
    C->>SC: POST /api/broker/cases/{id}/decide :173
    SC->>H: delegates to enrichment_proposals_handler::decide_as_operator (see VC-05.7)
    Note over H: REC-2 / D3 (PRD-023 WP-4) — power-user-gated by the surrounding scope,<br/>funnels through the SAME decision core as the agentbox X-Agent-Key route
```

## VC-05.9 `briefing_handler` — brief submit / debrief, Management API bridge
```mermaid
sequenceDiagram
    autonumber
    participant C as Client (voice/UI)
    participant H as briefing_handler<br/>src/handlers/briefing_handler.rs:118 scope /briefs
    participant BS as BriefingService<br/>src/services/briefing_service.rs:15
    participant MA as ManagementApiClient<br/>src/services/management_api_client.rs
    participant NB as NostrBeadPublisher<br/>src/services/nostr_bead_publisher.rs (optional)

    rect rgb(225,225,245)
    Note over H,MA: process boundary — Management API runs in the agentbox agent container
    C->>H: POST "" {briefing:BriefingRequest, user_context} :22 -> submit_brief :22
    H->>BS: submit_brief(request, user_context) :34
    BS->>MA: create_brief(content, roles, user_context, version?, brief_type?, slug?) :37
    alt Err
        MA-->>C: 502 BadGateway {error:"Brief submission failed", message}
    else Ok(brief_result)
        MA-->>BS: {brief_id, brief_path, bead_id}
        BS->>MA: execute_brief(brief_id, brief_path, roles, user_context, bead_id) :55
        alt Err
            MA-->>C: 502 BadGateway "Failed to execute brief"
        else Ok(role_tasks)
            MA-->>BS: Vec~RoleTask~ (spawned role agents)
            BS-->>H: BriefingResponse{brief_id,brief_path,bead_id,role_tasks}
            H-->>C: 201 Created BriefingResponse
        end
    end
    end
    C->>H: POST /{brief_id}/debrief {user_context,role_tasks} :119 -> request_debrief :54
    H->>H: bead_id = first role_task.bead_id or brief_id :69-74
    H->>BS: request_debrief(brief_id, role_tasks, user_context) :77
    BS->>MA: create_debrief(brief_id, role_tasks, user_context) :93
    alt Err
        MA-->>C: 502 BadGateway "Debrief creation failed"
    else Ok(debrief_path)
        opt nostr_publisher configured
            H->>NB: tokio::spawn publish_bead_complete(bead_id, brief_id, user_pubkey, debrief_path) :88-97
            Note over NB: fire-and-forget — does not affect the HTTP response
        end
        H-->>C: 201 Created {brief_id, debrief_path}
    end
```

## VC-05.10 `insight_loop_handler` — REC-10 Mesh Velocity trace reads
```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant H as insight_loop_handler<br/>src/handlers/insight_loop_handler.rs:91 scope /insight-loop
    participant IL as insight_loop::summarise / build_trace<br/>src/services/insight_loop.rs:209 / :103
    participant DB as sqlite_enrichment_repository<br/>loop_traces(limit) / loop_trace_for(case_id)
    participant LH as liveness_harness.observe<br/>CANARY_REC10_LOOP

    Note over H: assembled from the governed write-back queue's persisted stage timestamps —<br/>propose/queued (proposal row), broker_decision/merged_enrichment (decision row).<br/>amplification stage is labelled "planned" (v1 scope, insight_loop_handler.rs:9-13)
    C->>H: GET /trace?limit= :92 -> traces :53
    H->>H: limit = query.limit.unwrap_or(DEFAULT_LIMIT=100).clamp(1,MAX_LIMIT=1000)
    H->>DB: loop_traces(limit)
    alt Err
        DB-->>C: 500 {error:"loop-trace read failed: {e}"}
    else Ok(rows)
        H->>IL: summarise(rows)
        IL-->>H: InsightLoopSummary{traces, mesh_velocity aggregate}
        opt any trace loop_closed AND monotonic
            H->>LH: observe(CANARY_REC10_LOOP, evidence) — REC-10, observed live traffic only
        end
        H-->>C: 200 InsightLoopSummary
    end
    C->>H: GET /trace/{case_id} :93 -> trace_by_case :68
    H->>DB: loop_trace_for(case_id)
    alt Ok(Some(row))
        H->>IL: build_trace(row)
        IL-->>H: InsightLoopTrace
        opt loop_closed AND monotonic
            H->>LH: observe(CANARY_REC10_LOOP, evidence)
        end
        H-->>C: 200 InsightLoopTrace
    else Ok(None)
        H-->>C: 404 {error:"not-found", message:"no insight-loop trace for case {id}"}
    else Err
        H-->>C: 500 {error:"loop-trace read failed: {e}"}
    end
```

## VC-05.11 `memory_flash_handler` — RuVector-access notification relay to WS clients
```mermaid
sequenceDiagram
    autonumber
    participant C as caller (RuVector tool wrapper)
    participant H as memory_flash_handler<br/>src/handlers/memory_flash_handler.rs:134
    participant CC as ClientCoordinatorActor<br/>BroadcastMessage

    Note over H: NOT a RuVector client — this handler only relays a notification that a<br/>RuVector memory access already happened elsewhere, so every connected WS client<br/>can animate the corresponding embedding-cloud point(s)
    C->>H: POST /api/memory-flash {key, namespace?, action?} :134 -> handle_memory_flash :41
    H->>H: namespace=body.namespace or empty, action=body.action or "access"
    H->>H: serde_json::to_string(MemoryFlashBroadcast{type:"memory_flash", data})
    H->>CC: send(BroadcastMessage{message:json})
    alt Ok(Ok(()))
        CC-->>H: broadcast delivered
        H-->>C: 200 {ok:true}
    else Ok(Err(e))
        CC-->>H: broadcast-level error
        H-->>C: 200 {ok:true, warn:e}
    else Err(mailbox error)
        CC-->>H: actor mailbox error
        H-->>C: 500 {ok:false, error:"actor error: {e}"}
    end
    C->>H: POST /api/memory-flash/batch {events:[MemoryFlashRequest]} :137 -> handle_memory_flash_batch :103
    loop for each event in body.events
        H->>CC: do_send(BroadcastMessage{message:json}) — fire-and-forget, no per-event ack
        H->>H: count += 1 on successful serialize
    end
    H-->>C: 200 {ok:true, count}
```

## VC-05.12 `mcp_relay_handler` — `/ws/mcp-relay` bidirectional bridge to the orchestrator
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as mcp_relay_handler<br/>src/handlers/mcp_relay_handler.rs:441 (route src/main.rs:1035)
    participant A as MCPRelayActor<br/>src/handlers/mcp_relay_handler.rs:38
    participant O as orchestrator WS<br/>ORCHESTRATOR_WS_URL env :77 default ws://multi-agent-container:3002/ws

    C->>H: GET /ws/mcp-relay (Authorization Bearer or ?token=)
    H->>H: token = Bearer header or query "token" (:453-462)
    alt token empty
        H-->>C: 401 {error:"Authentication required"} — logged as SECURITY reject :469-474
    else token present (ANY non-empty value)
        Note over H: DOC-DRIFT — code comment :447-449 says "currently allows but logs<br/>unauthenticated connections", but the code REJECTS an empty token with 401 (:469-475)<br/>AND never validates a present token's value against NostrService/session store —<br/>any non-empty string is accepted as authenticated
        H->>A: ws::start(MCPRelayActor::new(), req, stream)
        A->>A: connect_to_orchestrator — CircuitBreaker::execute, connect_async(url), timeout=connect_timeout
        alt Ok(ws_stream)
            A->>O: split into tx/rx, do_send(SetOrchestratorTx(tx))
            A->>A: health_manager.check_service_now("orchestrator")
            A->>C: OrchestratorText("connected")
            par client -> orchestrator
                C->>A: ws::Message::Text(json)
                alt json.type == "ping"
                    A-->>C: {type:pong, timestamp}
                else orchestrator_tx set AND healthy
                    A->>O: tokio::time::timeout(5s, tx.send(Text))
                    alt timeout or Err
                        A->>A: health_manager.check_service_now (mark degraded)
                    end
                else unhealthy
                    A-->>C: {type:error, message:"Orchestrator unhealthy"}
                else not connected
                    A-->>C: {type:error, message:"Orchestrator not connected"}
                end
                C->>A: ws::Message::Binary(bin)
                A->>O: tokio::time::timeout(5s, tx.send(Binary)) (same health/degrade branches)
            and orchestrator -> client
                O->>A: TungsteniteMessage::Text/Binary via rx.next()
                A->>A: do_send(OrchestratorText/OrchestratorBinary)
                A->>C: ctx.text(text) / ctx.binary(bin) (Handler<OrchestratorText/Binary> :261,:285)
            end
        else Err/timeout
            A->>A: circuit_breaker records failure, retry on next connect_to_orchestrator
        end
    end
    C->>A: ws::Message::Close
    A->>A: ctx.stop()
```

## VC-05.13 `multi_mcp_websocket_handler` — `/multi-mcp` scope, discovery WS and two dead-stub REST routes
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as multi_mcp_websocket_handler<br/>src/handlers/multi_mcp_websocket_handler.rs:903 scope /multi-mcp
    participant WS as MultiMcpVisualizationWs<br/>src/actors/multi_mcp_visualization_actor.rs (started :428)
    participant DS as MultiMcpAgentDiscovery<br/>src/services/multi_mcp_agent_discovery.rs:62

    C->>H: GET /ws :861 -> multi_mcp_visualization_ws :779
    H->>H: token = Bearer header or ?token= (:786-798), same presence-only check as VC-05.12
    alt token empty
        H-->>C: 401 {error:"Authentication required"}
    else token present
        H->>WS: ws::start(MultiMcpVisualizationWs::new(app_state, None), req, stream)
        WS->>WS: started :428 — register health endpoints (claude-flow, ruv-swarm, flow-nexus)
        WS->>WS: start_heartbeat, start_position_updates
        WS->>WS: run_interval 30s perform_health_checks
        WS->>WS: run_interval 60s — if no success for 300s, send_discovery_data recovery attempt
        C->>WS: text "ping" (plain, pre-JSON) -> "pong"
        C->>WS: text {action:"configure", data:ClientConfig}
        WS->>WS: handle_client_config(config)
        C->>WS: text {action:"request_discovery"}
        WS->>WS: handle_discovery_request — rate-limited to 1/sec (:349-357)
        WS->>DS: (via send_discovery_data) query configured McpServerConfig endpoints
        Note over DS: DAA_HOST/DAA_PORT :125-126, RUV_SWARM_HOST/PORT :108-109,<br/>CLAUDE_FLOW_HOST/MCP_TCP_PORT :91-92 — env-configured server list
        alt no healthy services
            WS-->>C: {type:error, message:"No healthy MCP services available"}
        else healthy
            WS->>WS: retry_with_backoff over CircuitBreaker::execute
            WS-->>C: Handler<DiscoverySuccess> -> discovery payload
        end
        C->>WS: text {action:"request_agents"} — wrapped in catch_unwind (panic containment)
    end
    C->>H: GET /status :862 -> get_mcp_server_status :819
    Note over H: DIVERGENCE / dead-code — get_mcp_server_status returns a HARDCODED<br/>static JSON literal (claude-flow is_connected:true, ruv-swarm is_connected:false,<br/>:823-841) — it never queries DS, WS, or app_state (_app_state param unused),<br/>the response never reflects real discovery state
    H-->>C: 200 {servers:[...static...], total_agents:4, timestamp}
    C->>H: POST /refresh :863 -> refresh_mcp_discovery :848
    Note over H: DIVERGENCE / dead-code — refresh_mcp_discovery is a NO-OP stub:<br/>it logs "Manual MCP discovery refresh requested" and returns success without<br/>calling DS or notifying any live WS session (_app_state param unused, :848-856)
    H-->>C: 200 {success:true, message:"Discovery refresh initiated"}
```

## VC-05.14 `ontology_agent_handler` — MCP tool surface + `/propose` governance door
```mermaid
sequenceDiagram
    autonumber
    participant AG as agent (MCP tool caller)
    participant H as ontology_agent_handler<br/>src/handlers/ontology_agent_handler.rs:434 scope /ontology-agent
    participant QS as OntologyQueryService<br/>src/services/ontology_query_service.rs
    participant PS as scope /propose<br/>RateLimit::per_minute(20) + RequireAuth::authenticated() :444-448
    participant MS as OntologyMutationService::propose_create/propose_amend<br/>src/services/ontology_mutation_service.rs
    participant SP as proposal_spine::envelope_required<br/>src/services/proposal_spine.rs:411 env ONTOLOGY_REQUIRE_SIGNED_ENVELOPE

    Note over H: each read-side route mirrors one MCP tool 1:1 (handler doc :1-10)
    AG->>H: POST /discover :435 -> discover :103
    H->>QS: discover(query, limit, domain?)
    QS-->>AG: 200 {success, results, count} / error_json "Discovery failed"
    AG->>H: POST /read :436 -> read_note :129
    H->>QS: read_note(iri)
    QS-->>AG: 200 {success, note} / "Read note failed"
    AG->>H: POST /query :437 -> query :151
    H->>QS: validate_and_execute_cypher(cypher)
    QS-->>AG: 200 {success, validation} / "Query validation failed"
    AG->>H: POST /traverse :438 -> traverse :176
    H->>H: build_traversal(query_service, start_iri, depth, relationship_types?)
    H-->>AG: 200 {success, traversal} / "Traversal failed"
    AG->>H: GET /status :440 -> status :336
    H-->>AG: 200 (service status)
    AG->>H: POST /validate :439 -> validate :300
    loop for each axiom in req.axioms
        H->>QS: validate_and_execute_cypher(subject_check Cypher-like MATCH)
        QS-->>H: extend all_errors/all_hints
    end
    H-->>AG: 200 {errors, hints}
    AG->>PS: POST /propose "" :447 -> propose :217
    PS->>PS: RateLimit::per_minute(20) then RequireAuth::authenticated()
    alt not authenticated
        PS-->>AG: 401/403 deny
    else Ok(AuthenticatedUser)
        Note over H: INVARIANT WS-1/ADR-120 — agent_context.agent_id/user_id are<br/>OVERRIDDEN with auth.pubkey (:225-226), a caller cannot self-assert another agent's identity
        alt ProposeInput::Create
            H->>MS: propose_create(proposal, agent_context, idempotency_key, signature)
        else ProposeInput::Amend
            H->>MS: propose_amend(target_iri, amendment, agent_context, idempotency_key, signature)
        end
        MS->>SP: envelope_required() — ONTOLOGY_REQUIRE_SIGNED_ENVELOPE truthy check, default false
        alt required AND signature missing/invalid
            SP-->>MS: EnvelopeError -> ENVELOPE_REJECTED_PREFIX
            MS-->>AG: 403 {error:envelope_rejected, message}
        else Ok(proposal_result)
            MS-->>AG: 200 {success:true, proposal}
        else CONFLICT_BLOCKED_PREFIX
            MS-->>AG: 409 {error:conflict_blocked, blockingConflicts, preExisting, conflictReport}
        else IDEMPOTENCY_CONFLICT_PREFIX
            MS-->>AG: 409 {error:idempotency_conflict, message}
        else other Err
            MS-->>AG: error_json "Proposal failed"
        end
    end
```

## VC-05.15 `solid_proxy_handler` — pod lifecycle, DID resolution, double-auth DIVERGENCE
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant SC as scope /solid<br/>src/handlers/solid_proxy_handler.rs:1752 configure_routes (feature solid-pod-embed)
    participant H as solid_proxy_handler
    participant RG as RbacGate<br/>/api scope wrap (WriteGraph on mutating /api/*, see VC-03.6)
    participant SD as SolidPodState::extract_user_identity<br/>:184 own NIP-98 verification

    Note over SC: env SOLID_DATA_ROOT :122, SOLID_PROXY_SECRET_KEY :125, SOLID_ALLOW_ANONYMOUS :129<br/>without solid-pod-embed feature, configure_routes :1861 registers the SAME route<br/>tree but every handler is a 503 stub
    C->>SC: GET /health :1832 -> solid_health_check :1640
    SC-->>C: 200 (or 503 when compiled without solid-pod-embed, :1658)
    C->>SC: GET /.notifications :1835 -> handle_solid_notifications_ws :1614 (solid-0.1 WS protocol)
    C->>SC: POST /pods :1838 -> create_pod :1174
    C->>SC: GET /pods/check :1839 -> check_pod_exists :1242
    C->>SC: POST /pods/init :1840 -> init_pod :1293
    rect rgb(255,235,235)
    Note over C,SD: DIVERGENCE — /api/solid/pods/init-nip98 sits inside the /api scope,<br/>so it is DOUBLE-authenticated (see VC-03.15)
    C->>RG: POST /api/solid/pods/init-nip98 :1841 (Authorization: Nostr event)
    RG->>RG: RbacGate verify_access(WriteGraph) — mutating method under /api
    alt RbacGate denies
        RG-->>C: 401/403
    else Ok
        RG->>H: init_pod_nip98(req, state) :1349
        H->>SD: extract_user_identity(req) — SECOND, fully independent NIP-98 re-verification
        alt SD returns None (bad/missing token)
            SD-->>C: 401 {error:"NIP-98 authentication required"}
        else Some(identity)
            H->>H: PublicKey::from_hex(identity.pubkey).to_bech32() -> npub
            H->>H: ensure_pod_exists(state, npub, pubkey, pod_base_url) :1384
            H-->>C: 200 {pod_url, webid:structure.profile, created, structure, npub}
        end
    end
    end
    C->>SC: any method /{tail:.*} :1843-1847,1850 -> handle_solid_proxy :307/:379 (LDP CRUD)
    C->>SC: GET /.well-known/did.json :1854 -> handle_did_wellknown :1672
    C->>SC: GET /did/{tail:.*} :1855 -> handle_did_proxy :1716 -> solid_pod_rs::interop::did_nostr::did_nostr_document
    Note over H: full pod/LDP internals (storage backend, ACL, containers) — see VC-26
```

## VC-05.16 `speech_socket_handler` — `/ws/speech` boundary only
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as speech_socket_handler<br/>src/handlers/speech_socket_handler.rs:971-972 (route src/main.rs:1034)
    participant SS as SpeechSocket actor<br/>src/handlers/speech_socket_handler.rs:106 new()

    C->>H: GET /ws/speech (Authorization Bearer or ?token=) :818
    H->>H: token = Bearer header or query "token" (same pattern as VC-05.12/.13)
    alt token empty
        H-->>C: 401 {error:"Authentication required"} (SECURITY reject log :996-999)
    else token present (presence-only, same as mcp-relay/multi-mcp)
        H->>SS: SpeechSocket::new(socket_id="speech_{uuid}", app_state, None)
        H->>SS: ws::start(socket, req, stream)
        alt Err
            SS-->>C: ws start failure logged, propagated as actix_web::Error
        else Ok(response)
            SS-->>C: 101 Switching Protocols
        end
    end
    Note over SS: voice internals (governed-voice command dispatch, clarification turns,<br/>ElevationActor VoiceTranscript hookup) — see VC-35
```

## VC-05.17 ADR-2006 human-approval journey — handler side, two disconnected producers, one store
```mermaid
sequenceDiagram
    autonumber
    participant V as voice/RunCycle trigger<br/>src/actors/elevation_actor.rs:832 Handler~RunCycle~ (VC-02 internals)
    participant EA as ElevationActor/DecisionElevationActor<br/>src/actors/elevation_actor.rs:98, decision_elevation_actor.rs:128 (VC-02)
    participant FR as forum kind-31403<br/>src/services/acsp/client.rs:22 CaseDecision{event_id,created_at}
    participant DB as SqliteEnrichmentRepository::record_decision<br/>StoredDecision table — SHARED sink
    participant D as decide/decide_as_operator -> apply_decision<br/>src/handlers/enrichment_proposals_handler.rs:341-351
    participant BI as broker_inbox_handler::inbox<br/>src/handlers/broker_inbox_handler.rs:133

    Note over V,FR: no HTTP handler in this file (decision_handler, enrichment_proposals_handler,<br/>broker_inbox_handler) ever sends a message to ElevationActor or DecisionElevationActor —<br/>grep across src/handlers finds zero references to either actor type
    V->>EA: RunCycle / VoiceTranscript (actor-internal, see VC-02)
    EA->>FR: publish ActionRequest kind-31402, poll PollPrs for the signed kind-31403 reply
    Note over FR: ADR-2013 — AcspClient signs with its OWN Keys (acsp/client.rs:67,77,<br/>Keys::new(secret_key), mirrors nostr_bridge.rs:65 sign_with_keys pattern) —<br/>the panel event carries the PANEL's authority, never the responding admin's original key
    FR-->>EA: CaseDecision{case_id,action,responder_pubkey,event_id,created_at}
    EA->>EA: decision_record(&CaseDecision) :1227 — correlation on event_id when present
    EA->>DB: repo.record_decision(StoredDecision{decision_event_id:Some(event_id), decision_created_at_s:Some(...)}) :987,1053,1060
    Note over EA,DB: this producer DOES retain signed-event correlation (elevation_actor.rs:1240-1281)
    D->>DB: repo.record_decision(StoredDecision{decision_event_id:None,...}) :383-400 — REST path, no forum event to correlate
    Note over D,DB: DIVERGENCE — the agentbox/operator/git-bridge REST path (VC-05.7) writes<br/>decision_event_id:None every time, since it never carries a signed 31403 event
    BI->>DB: store::all() / store::get(id) — reads the SAME table both producers wrote to
    BI-->>BI: projects EITHER kind of row into the SAME BrokerCase shape, indistinguishable to the bridge
    Note over V,BI: DIVERGENCE (ADR-2006 closeout, BASELINE-architecture.md l.273 2026-09-04) —<br/>"the retained domain kernel's presence does not prove integration into the elevation<br/>actor or inbox DTO. Current source review does not certify a complete human-approval journey."<br/>Verified precisely: the two producers only converge at the SQLite table, no handler route<br/>calls into either actor, and case authority/failure/restart receipts are not modelled here
```

## VC-05.18 `src/domain/broker/` verification — BrokerActor never merged (BASELINE l.224)
```mermaid
flowchart TB
    KERNEL["src/domain/broker/ — storage-agnostic kernel (936 LOC, ADR-130 Decision 2)<br/>mod.rs :1-40, broker_case.rs 490L, broker_decision.rs 437L, precedent_registry.rs 101L"]
    BC["BrokerCase aggregate<br/>CaseCategory, SubjectKind, ShareState (Private to Team to Mesh)"]
    BD["DecisionOrchestrator + DecisionOutcome<br/>six canonical outcomes, ShareTransitionPlan"]
    PR["PrecedentRegistry"]
    KERNEL --> BC
    KERNEL --> BD
    KERNEL --> PR
    ACSP["stateless ACSP producer<br/>src/services/acsp/mod.rs — kinds 31400-31405, AcspClient::publish"]
    ABSENT["src/actors/broker_actor.rs — VERIFIED ABSENT<br/>grep -r finds no file, no Neo4j adapter under src/adapters/ (listed: sqlite_*, oxigraph_*, actix_*)"]
    KERNEL -.->|"used by derive_kernel_decision (VC-05.7)"| CALLER["enrichment_proposals_handler::apply_decision"]
    KERNEL -.->|"used by decision_record (VC-05.17)"| CALLER2["elevation_actor.rs"]
    N1["DIVERGENCE (BASELINE-architecture.md l.224) — BrokerActor never merged,<br/>main uses a stateless ACSP producer + this cherry-picked storage-agnostic<br/>936-LOC domain broker kernel, confirmed against source: mod.rs doc comment<br/>:10-16 states the crashbug BrokerActor + Neo4j transport were deliberately left behind"]
    ABSENT --- N1
    ACSP --- N1
    N2["ADR-2006 Verification section (accepted) matches this source state exactly —<br/>src/services/acsp/mod.rs documents the producer, src/domain/broker/ contains<br/>the four listed files, broker_actor.rs and neo4j adapters are absent"]
    KERNEL --- N2
```
