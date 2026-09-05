---
id: VC-03
title: Request lifecycle, identity and the RBAC lattice
area: visionclaw
governing:
  - docs/IDENTITY-authority-chain.md
  - docs/SECURITY-profiles.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2002, ADR-2003, ADR-2009, ADR-2010, ADR-2011, ADR-2012, ADR-2013, ADR-2026, ADR-2039, ADR-2043, ADR-2044]
sources:
  - src/main.rs
  - src/middleware/mod.rs
  - src/middleware/rbac_gate.rs
  - src/middleware/public_demo.rs
  - src/middleware/rate_limit.rs
  - src/middleware/timeout.rs
  - src/middleware/validation.rs
  - src/settings/auth_extractor.rs
  - src/utils/auth.rs
  - src/utils/nip98.rs
  - src/services/nostr_service.rs
  - src/services/nostr_identity_verifier.rs
  - src/services/nostr_bridge.rs
  - src/services/role_store.rs
  - src/models/rbac.rs
  - src/handlers/nostr_handler.rs
  - src/handlers/socket_flow_handler/http_handler.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - src/handlers/socket_flow_handler/filter_auth.rs
  - src/handlers/fastwebsockets_handler.rs
  - src/handlers/client_messages_handler.rs
  - src/handlers/mcp_relay_handler.rs
  - src/handlers/multi_mcp_websocket_handler.rs
  - src/handlers/speech_socket_handler.rs
  - src/handlers/solid_proxy_handler.rs
  - src/actors/client_filter.rs
  - src/uri/mod.rs
  - src/config/security_profile.rs
  - crates/visionclaw-domain/src/utils/visibility_filter.rs
verified_commit: bed6b617d
---

## VC-03.1 REST request end-to-end — nginx to handler, real middleware order
```mermaid
sequenceDiagram
    autonumber
    participant NG as nginx :3001
    participant AC as actix HttpServer<br/>src/main.rs:968-1027
    participant LG as Logger<br/>wrap #1 src/main.rs:969
    participant CO as cors<br/>wrap #2 src/main.rs:970
    participant CP as Compress<br/>wrap #3 src/main.rs:971
    participant TO as TimeoutMiddleware<br/>wrap #4 src/main.rs:972-975
    participant SC as scope /api<br/>src/main.rs:1044
    participant PD as PublicDemoGuard::from_env<br/>src/main.rs:1048
    participant RG as RbacGate::from_env<br/>src/main.rs:1055
    participant RL as RateLimit::per_minute 60<br/>src/main.rs:1062 (scope /api/settings only)
    participant H as route handler

    Note over LG,RG: actix applies .wrap() in REVERSE registration order at request time<br/>last .wrap() call = outermost layer that sees the request first
    Note over LG,TO: registration order in main.rs is Logger,cors,Compress,TimeoutMiddleware (969-972)<br/>so the REAL request-time order is TimeoutMiddleware,Compress,cors,Logger,then routing
    NG->>AC: HTTP request
    AC->>TO: enter (outermost of the four)
    TO->>TO: get_timeout(path) — default 30s, override 600s for "/api/admin/sync" (src/main.rs:973-974)
    TO->>CP: enter
    CP->>CO: enter
    CO->>LG: enter
    LG->>SC: enter /api scope
    rect rgb(225,225,245)
    SC->>PD: wrap #1 on scope (outermost of the two /api wraps)
    alt PUBLIC_DEMO read-only AND method not GET/HEAD/OPTIONS
        PD-->>NG: 403 read_only_demo (see VC-03.10)
    end
    PD->>RG: wrap #2 on scope
    alt required_level present (not public route)
        RG->>H: verify_access then call handler (see VC-03.6)
    else public route
        RG->>H: pass straight through
    end
    end
    alt handler exceeds timeout_duration
        TO-->>NG: 504 Gateway Timeout "Request to {path} timed out after {ms}ms" (src/main.rs:113-127 of timeout.rs)
    end
    H-->>NG: HttpResponse
```

## VC-03.2 WebSocket upgrade lifecycle — every upgrade route, the `?token=` DIVERGENCE
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant WS as socket_flow_handler<br/>src/handlers/socket_flow_handler/http_handler.rs
    participant NS as NostrService::get_session<br/>src/services/nostr_service.rs:574

    Note over WS: routes registered src/main.rs:1026-1035 — /wss, /wss/agent-events,<br/>/ws/speech, /ws/mcp-relay, /ws/client-messages, /ws/presence
    C->>WS: GET /wss (Upgrade: websocket)
    WS->>WS: require Upgrade header (http_handler.rs:100)
    alt Origin header present
        WS->>WS: check against allowed_origins / is_same_host
        alt origin not allowed
            WS-->>C: 403 Forbidden "Origin not allowed"
        end
    else Origin missing
        alt debug/dev-auth build AND ALLOW_INSECURE_DEFAULTS
            WS->>WS: allow_missing_origin = true (http_handler.rs:117-121)
        else release build
            WS-->>C: 400 BadRequest "Origin header required" (http_handler.rs:123-130)
        end
    end
    WS->>WS: token = Authorization Bearer OR query "?token=" (http_handler.rs:139-150)
    Note over WS: DEPRECATED ADR-2044 — query-param token accepted on the upgrade routes (one release):<br/>http_handler.rs:155 — fastwebsockets_handler.rs:238 — client_messages_handler.rs:127 —<br/>mcp_relay_handler.rs:461 — multi_mcp_websocket_handler.rs:798 — filter_auth.rs:138 (WS message body).<br/>Kept for XR and native clients that cannot set headers on an upgrade — the header path is<br/>preferred and the query path leaks into proxy and access logs. CORRECTION — speech_socket_handler<br/>has NO query path and does full NIP-98 verification (verify_nip98_auth, :230) — it was wrongly<br/>listed here in the Phase 1 pass.
    alt token present
        WS->>NS: get_session(token)
        alt session found
            NS-->>WS: NostrUser { pubkey, is_power_user }
            WS->>WS: ws_server.pubkey = Some(pubkey) (http_handler.rs:341-346)
        else session not found
            alt debug/dev-auth build AND insecure_allowed
                WS->>WS: warn, allow anyway (http_handler.rs:303-312)
            else release build
                WS-->>C: 401 Unauthorized "Invalid or expired authentication token"
            end
        end
    else token absent
        WS->>WS: continue unauthenticated (anonymous session — visibility filter drops private nodes, see VC-03.9)
    end
    Note over NS: RESOLVED ADR-2044 — get_session (nostr_service.rs:574) now enforces the SAME<br/>AUTH_TOKEN_EXPIRY window as validate_session (:478) through one shared rule<br/>session_is_fresh(last_seen, now, token_expiry) (:597). It previously had NO expiry check,<br/>so a WS token outlived its REST equivalent indefinitely. Empty tokens are rejected before<br/>lookup and a future last_seen (clock stepped back) is stale, not an unbounded lease.
    WS-->>C: 101 Switching Protocols
    C->>WS: WS frames (binary positions / control JSON)
```

## VC-03.3 `AuthenticatedUser` extractor — settings API dual-auth
```mermaid
sequenceDiagram
    autonumber
    participant R as request
    participant FR as AuthenticatedUser::from_request<br/>src/settings/auth_extractor.rs:93
    participant DB as try_dev_bypass<br/>src/settings/auth_extractor.rs:20 dev / :64 release stub
    participant NS as NostrService::verify_nip98_auth<br/>src/services/nostr_service.rs:579

    R->>FR: from_request(req)
    FR->>DB: try_dev_bypass(req)
    alt debug_assertions or feature dev-auth
        alt dev_full_bypass_active() (VISIONCLAW_DEV_MODE)
            DB-->>FR: Some(AuthenticatedUser{ DEV_MODE_PUBKEY, is_power_user:true })
        else Authorization == "Bearer dev-session-token"
            alt dev_bypass_permitted(req) (loopback + DEV_AUTH_LOOPBACK=1)
                DB-->>FR: Some(AuthenticatedUser{ pubkey: X-Nostr-Pubkey or "dev-user", true })
            else
                DB-->>FR: None (warn, rejected)
            end
        else
            DB-->>FR: None
        end
    else release build
        DB-->>FR: None (stub, auth_extractor.rs:62-66)
    end
    alt dev bypass returned Some
        FR-->>R: 200 Ok(user)
    end
    FR->>FR: app_data::<NostrService> lookup
    alt NostrService missing
        FR-->>R: 401 "Authentication service unavailable" (:109)
    end
    FR->>FR: Authorization header present and str-decodable?
    alt header absent
        FR-->>R: 401 "Missing authorization token" (:127)
    else header not valid UTF-8
        FR-->>R: 401 "Invalid authorization header" (:121)
    end
    alt header starts with "Nostr "
        FR->>NS: verify_nip98_auth(header, url, method, None)
        alt Ok(user)
            FR-->>R: 200 Ok(AuthenticatedUser{ pubkey, is_power_user })
        else Err(e)
            FR-->>R: 401 "NIP-98 auth failed: {e}" (:174)
        end
    else header starts with "Bearer "
        FR->>FR: pubkey = X-Nostr-Pubkey header
        alt X-Nostr-Pubkey missing
            FR-->>R: 401 "Missing pubkey" (:200)
        else header not UTF-8
            FR-->>R: 401 "Invalid pubkey header" (:195)
        end
        Note over FR: legacy Bearer fallback does NOT call validate_session here —<br/>it trusts the caller-supplied pubkey outright (settings API dual-auth path)
    else unrecognised prefix
        FR-->>R: 401 "Invalid authorization format" (:185)
    end
```

## VC-03.4 NIP-98 verification (kind 27235) — fixed order, single-use replay claim
```mermaid
sequenceDiagram
    autonumber
    participant H as caller<br/>auth.rs:142 / auth_extractor.rs:93
    participant NS as NostrService::verify_nip98_auth<br/>src/services/nostr_service.rs:579
    participant V as validate_nip98_token<br/>src/utils/nip98.rs:270
    participant RC as REPLAY_CACHE<br/>src/utils/nip98.rs:187 Mutex~HashMap~

    Note over V,RC: INVARIANT (IDENTITY-authority-chain.md #1) — order is fixed:<br/>freshness -> tag match (host-checked) -> signature -> replay claim LAST.<br/>The claim must never precede signature verification.
    H->>NS: verify_nip98_auth(auth_header, url, method, body)
    NS->>NS: reconstruct url from X-Forwarded-Proto/-Host behind TLS proxy (:57-59 doc)
    NS->>V: validate_nip98_token(token, expected_url, expected_method, body)
    V->>V: base64/UTF-8/JSON decode (nip98.rs:243-253)
    alt decode fails
        V-->>NS: InvalidBase64 / InvalidUtf8 / InvalidJson
    end
    V->>V: kind == 27235 HTTP_AUTH_KIND (:22, check :257)
    alt kind mismatch
        V-->>NS: InvalidKind(got)
    end
    V->>V: age = now - created_at, symmetric window TOKEN_MAX_AGE_SECONDS=60 (:168, checks :281-289)
    alt age > 60
        V-->>NS: TokenExpired(age)
    else age < -60
        V-->>NS: TokenFromFuture(-age)
    end
    V->>V: extract "u" and "method" tags (:293-303), urls_match (:524) host-checked
    alt "u" tag missing
        V-->>NS: MissingTag("u")
    else "method" tag missing
        V-->>NS: MissingTag("method")
    else url mismatch
        V-->>NS: UrlMismatch{expected,actual}
    else method mismatch (case-insensitive)
        V-->>NS: MethodMismatch{expected,actual}
    end
    opt request_body Some AND payload tag present
        V->>V: compute_payload_hash == SHA-256(body)
        alt hash mismatch
            V-->>NS: PayloadHashMismatch
        end
    end
    V->>V: Schnorr signature .verify() (:426-427)
    alt signature invalid
        V-->>NS: InvalidSignature
    end
    critical claim_event_id(event.id, Instant::now()) — nip98.rs:234, under one Mutex lock
        V->>RC: check-prune-cap-insert atomically (:435)
        alt id already live within REPLAY_CACHE_TTL (2x60s=120s, :177)
            RC-->>V: TokenReplayed
        else cache.len() >= REPLAY_CACHE_MAX_ENTRIES=100_000 after prune
            RC-->>V: ReplayCacheFull (maps to 503 upstream)
        else
            RC-->>V: Ok — id recorded with monotonic Instant
        end
    end
    Note over RC: INVARIANT #2 — replay-cache TTL (120s) >= 2x freshness window (60s),<br/>so no token still inside its freshness window can outlive its cache entry.<br/>Process-local scope — no cross-replica replay protection (SECURITY-profiles.md).
    V-->>NS: Ok(Nip98ValidationResult{pubkey,url,method,created_at,payload_hash})
    NS->>NS: get_or_create_user_from_pubkey, set is_power_user (nostr_service.rs:616)
    NS-->>H: NostrUser
```

## VC-03.5 Session-bearer realm (ADR-2009) — NIP-98 signing vs opaque session token
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as /api scope (RbacGate)<br/>src/middleware/rbac_gate.rs
    participant SET as /api/settings (AuthenticatedUser)<br/>src/settings/auth_extractor.rs
    participant WSy as WS upgrade routes<br/>src/handlers/socket_flow_handler/http_handler.rs
    participant NS as NostrService<br/>src/services/nostr_service.rs

    Note over API,WSy: two realms coexist by design (ADR-2009) — request-signing (NIP-98, re-verified<br/>every call) and session-bearer (opaque token minted after a NIP-98 login)
    rect rgb(225,245,225)
    Note over C,API: REALM 1 — request-signing, re-verified on EVERY call
    C->>API: Authorization: Nostr <base64 kind-27235 event>
    API->>NS: verify_nip98_auth per request (verify_access, auth.rs:142)
    C->>SET: Authorization: Nostr <event> (primary path, auth_extractor.rs:132)
    end
    rect rgb(225,225,245)
    Note over C,WSy: REALM 2 — session-bearer, minted once, presented as an opaque token
    C->>NS: POST /api/auth/nostr (login) — verify_auth_event, issue Uuid::new_v4 (nostr_service.rs:416)
    NS-->>C: session token, TTL = AUTH_TOKEN_EXPIRY (default 3600s, nostr_service.rs:131)
    C->>WSy: Authorization Bearer <token> or ?token=<token>
    WSy->>NS: get_session(token) — enforces AUTH_TOKEN_EXPIRY since ADR-2044 (see VC-03.2)
    C->>API: legacy X-Nostr-Pubkey + X-Nostr-Token headers (auth.rs:266-283)
    API->>NS: validate_session(pubkey, token) — DOES check now - last_seen <= token_expiry (:478-483)
    end
    Note over SET: /api/settings ALSO accepts legacy "Bearer <token>" + X-Nostr-Pubkey (auth_extractor.rs:181-201)<br/>WITHOUT calling validate_session — the settings extractor trusts the header pubkey outright
    Note over API,SET: /api re-verifies NIP-98 on each call rather than trusting the session token<br/>(IDENTITY-authority-chain.md line 67-68) — session tokens are the WS credential
```

## VC-03.6 `RbacGate` decision sequence — env flags, allowlist, method classification
```mermaid
sequenceDiagram
    autonumber
    participant R as request
    participant RG as RbacGateMiddleware::call<br/>src/middleware/rbac_gate.rs:238
    participant RL as required_level<br/>src/middleware/rbac_gate.rs:134
    participant VA as verify_access<br/>src/utils/auth.rs:142

    RG->>RL: required_level(method, path, public_reads)
    RL->>RL: segments(path) split on "/" (:64)
    alt has_segment_prefix ["api","auth"] or ["api","client-logs"] or ["api","health"|"healthz"|"readyz"]
        RL-->>RG: None (public — allowlisted, rbac_gate.rs:56-61)
    else has_segment_prefix ["api","admin"]
        RL-->>RG: Some(AccessLevel::Admin) for EVERY method (:147)
    else method.is_safe() (GET/HEAD/OPTIONS)
        alt RBAC_PUBLIC_READS=1 or true (public_reads_enabled, :122-128, fails closed on absence)
            RL-->>RG: None (public read)
        else
            RL-->>RG: Some(AccessLevel::ReadOnly)
        end
    else mutating method
        alt has_segment_prefix ["api","settings"]
            RL-->>RG: Some(AccessLevel::WriteSettings)
        else
            RL-->>RG: Some(AccessLevel::WriteGraph) (denies Viewer — Viewer maps to ReadOnly only)
        end
    end
    alt required is None
        RG->>R: pass through, no auth
    else required is Some(level)
        RG->>RG: app_data::<NostrService> lookup
        alt NostrService missing
            alt mode == Enforce
                RG-->>R: 401 Unauthorized
            else mode == Report
                RG->>R: pass through anyway
            end
        end
        RG->>VA: verify_access(req, nostr_service, level)
        alt Ok(pubkey)
            RG->>RG: insert AuthenticatedUser{pubkey} into request extensions (:268)
            RG->>R: call handler
        else Err(deny_response)
            alt mode == Enforce (RBAC_GATE_MODE default)
                RG-->>R: deny_response as-is (401/403)
            else mode == Report
                RG->>RG: error!() log "would DENY ... allowed because auth is NOT enforced"
                RG->>R: call handler anyway
            end
        end
    end
    Note over RG: ADR-2011 — GateMode::from_env (:83-102): report requires debug_assertions<br/>OR RBAC_REPORT_MODE_ACK==today's UTC date, else falls back to Enforce with an error! log
```

## VC-03.7 Role lattice — Owner > Admin > Editor > Viewer, default-role resolution
```mermaid
flowchart TB
    OWN["Owner (4)<br/>src/models/rbac.rs:35 — full control"]
    ADM["Admin (3)<br/>manage users/settings, cannot touch Owner grants"]
    EDI["Editor (2)<br/>default_authenticated() — read + WriteGraph"]
    VIE["Viewer (1)<br/>ReadOnly only"]
    OWN --> ADM --> EDI --> VIE
    ACC1["AccessLevel::Admin<br/>to_access_level() rbac.rs:83"]
    ACC2["AccessLevel::Authenticated"]
    ACC3["AccessLevel::ReadOnly"]
    OWN -.->|"to_access_level"| ACC1
    ADM -.->|"to_access_level"| ACC1
    EDI -.->|"to_access_level"| ACC2
    VIE -.->|"to_access_level"| ACC3
    PREC["effective_role precedence — role_store.rs:359<br/>1. explicit row (RoleStore::get)<br/>2. Admin if POWER_USER_PUBKEYS match, no row<br/>3. else configured_default (RBAC_DEFAULT_ROLE_ENV)"]
    DEF["parse_default_role — role_store.rs:195<br/>unset or #quot;#quot; -> Editor (ADR-2010 compat default)<br/>#quot;editor#quot; -> Editor, #quot;viewer#quot; -> Viewer (case-insensitive, trimmed)<br/>anything else incl. #quot;admin#quot;/#quot;owner#quot; -> FAILS CLOSED to Viewer, error! logged"]
    ERR["any lookup/parse error (RoleStoreError)<br/>-> FAILS CLOSED to Viewer, role_store.rs:381"]
    PREC --> DEF
    PREC --> ERR
    N1["INVARIANT — RBAC_DEFAULT_ROLE can only NARROW (editor/viewer), never widen<br/>to admin/owner; an env typo cannot mass-grant elevated access"]
    DEF --- N1
    N2["CAN_ASSIGN lattice, rbac.rs:120 — Owner may assign anything;<br/>Admin may assign only target.level() lt Admin.level() (Editor/Viewer);<br/>Editor/Viewer may assign nothing"]
    OWN --- N2
```

## VC-03.8 `role_store` atomic mutations (ADR-2010) — grant/revoke/transfer
```mermaid
sequenceDiagram
    autonumber
    participant C as caller (Admin/Owner)
    participant RS as RoleStore::assign_checked<br/>src/services/role_store.rs:408
    participant TX as SQLite transaction<br/>tokio_rusqlite::Connection

    C->>RS: assign_checked(target, new_role, caller: CallerAuthority)
    critical single tx.transaction() scope — role_store.rs:421-489
        RS->>TX: tx = c.transaction()
        TX->>TX: resolve_caller_role_in_tx(caller, default_role) — re-reads CALLER's own row (:429)
        Note over TX: ADR-2010 — re-resolving the caller INSIDE the same tx closes the race where<br/>a concurrent demotion of the caller is not seen before the lattice check
        TX->>TX: effective_mutation_authority(admission_role, current)
        alt caller authority changed since admission
            TX-->>RS: TxOutcome (Forbidden/other) — no write
        end
        TX->>TX: read_role(tx, target) — existing role or ExistingRole::Invalid
        alt existing role fails to parse
            TX-->>RS: TxOutcome::InvalidExisting
        end
        alt !caller.can_assign(new_role)
            TX-->>RS: TxOutcome::Forbidden("{caller} may not assign role {new_role}")
        else target has existing role AND !caller.can_assign(existing)
            TX-->>RS: TxOutcome::Forbidden("may not modify a user currently holding {existing}")
        end
        alt existing == Owner AND new_role != Owner
            TX->>TX: COUNT(*) WHERE role='owner'
            alt owners <= 1
                TX-->>RS: TxOutcome::LastOwner
            end
        end
        TX->>TX: INSERT ... ON CONFLICT UPDATE role,assigned_by,updated_at
        TX->>TX: tx.commit()
        TX-->>RS: TxOutcome::Ok
    end
    RS-->>C: Ok(new_role) or RoleStoreError
    Note over RS: remove_checked (:500) runs the SAME atomic pattern — ADR-2010 —<br/>removal is NOT revocation: deleting the row drops the target to their AMBIENT<br/>authority (Admin if power-user, else configured_default), never denies outright.<br/>Genuine revocation requires an explicit Viewer assignment.
    Note over RS: ownerless resolution — bootstrap_owner_from_env (:641) grants Owner to<br/>RBAC_OWNER_PUBKEY idempotently at boot if unassigned — see VC-03.11 for the boot-fail path
```

## VC-03.9 `PUBKEY_VISIBILITY_FILTER` — filtered vs unfiltered wire frame
```mermaid
sequenceDiagram
    autonumber
    participant WS as SocketFlowServer position tick<br/>src/handlers/socket_flow_handler/position_updates.rs
    participant EF as pubkey_visibility_filter_enabled<br/>position_updates.rs:50 (OnceLock, cached first call)
    participant VF as compute_private_opaque_ids / apply_drop_set<br/>visionclaw_domain::utils::visibility_filter

    Note over EF: parse_visibility_flag (:34-43) — default ON — only explicit<br/>"0"/"false"/"off"/"no" (case-insensitive, trimmed) disables.<br/>NOTE: env is read ONCE via OnceLock and cached for process lifetime (:50-58) —<br/>toggling PUBKEY_VISIBILITY_FILTER at runtime after first call has NO effect.
    WS->>EF: pubkey_visibility_filter_enabled()
    alt cached value is ON (default)
        alt visibility metadata non-empty
            WS->>VF: compute_private_opaque_ids(visibility, act.pubkey.as_deref())
            Note over VF: fail-closed — act.pubkey==None (unauthenticated session) drops ALL private nodes
            VF-->>WS: drop_set
            WS->>VF: apply_drop_set(&mut nodes, &drop_set) (:413)
            VF-->>WS: dropped_count
            WS->>WS: debug! "{dropped} dropped by PUBKEY_VISIBILITY_FILTER" (:751)
        end
    else disabled
        WS->>WS: no filtering — full node set (incl. private) sent to every client
    end
    WS-->>WS: single full-state frame per tick, binary positions
    Note over WS: DIVERGENCE — default flipped ON 2026-08-31 — the drop-set encoder<br/>(visibility_filter.rs) existed before this rebuild but was inert (never called<br/>from the position-tick path). See docs/IDENTITY-authority-chain.md divergence bullet.
    Note over WS: src/actors/client_filter.rs::recompute_filtered_nodes is a DIFFERENT filter —<br/>quality/authority-threshold + linked_page inclusion, NOT pubkey ownership.<br/>It does not implement PUBKEY_VISIBILITY_FILTER — position_updates.rs is the real site.
```

## VC-03.10 `PublicDemoGuard` — read-only demo mode
```mermaid
sequenceDiagram
    autonumber
    participant R as request (wrapped /api scope)
    participant PD as PublicDemoGuardService::call<br/>src/middleware/public_demo.rs:98
    participant ENV as PUBLIC_DEMO env<br/>const PUBLIC_DEMO_ENV, public_demo.rs:24

    Note over PD: read ONCE at PublicDemoGuard::from_env() (app start, :44-53) — not re-read per request
    PD->>ENV: public_demo_read_only() at construction
    Note over ENV: truthy tokens (trim+lowercase): "read-only","readonly","1","true","on" (:26-31)
    alt enabled AND method NOT IN GET/HEAD/OPTIONS
        PD-->>R: 403 {"error":"read_only_demo","message":"...mutating requests are disabled."} (:100-108)
    else enabled AND safe method, or disabled entirely
        PD->>R: pass through unchanged
    end
    Note over PD: DEFAULT OFF — with PUBLIC_DEMO unset the middleware is fully inert (:11-12)
```

## VC-03.11 Dev bypass triple gate (ADR-2012 / ADR-2039) — nested conditions
```mermaid
sequenceDiagram
    autonumber
    participant R as request
    participant A as dev_full_bypass_active<br/>src/utils/auth.rs:99 dev / :112 release stub
    participant L as dev_bypass_permitted_for_addr<br/>src/utils/auth.rs:122
    participant B as boot: enforce_release_env_hygiene<br/>src/main.rs:118 real / :169 stub

    Note over B: gate 0 (compile-time) — this whole codepath is #[cfg(any(debug_assertions,#quot —dev-auth#quot —))]<br/>compiled OUT of a release binary entirely — release stub always returns false/None
    alt release build (no debug_assertions, no dev-auth feature)
        B->>B: enforce_release_env_hygiene runs at boot (src/main.rs:195)
        alt VISIONCLAW_DEV_MODE present (any value, presence not truthiness)
            B-->>R: FATAL eprintln, std::process::exit(2) (SUSPECT_ENVS, :130-134,:156)
        end
        Note over A,L: dev_full_bypass_active and dev_bypass_permitted are unreachable stubs in this binary
    else debug or dev-auth build
        R->>A: dev_full_bypass_active() (VISIONCLAW_DEV_MODE)
        alt env == "1" or "true" (trim, case-insensitive)
            A-->>R: true — LAN-local FULL bypass, EVERY request granted DEV_MODE_PUBKEY, no signature/token/peer check (auth.rs:99-107)
        else
            A-->>R: false
            R->>L: Authorization == "Bearer dev-session-token"?
            alt header matches
                R->>L: dev_bypass_permitted_for_addr(peer)
                alt DEV_AUTH_LOOPBACK=1 or true AND peer.ip().is_loopback()
                    L-->>R: true — dev-admin as X-Nostr-Pubkey or "dev-user"
                else opt-in missing OR non-loopback peer OR no peer addr
                    L-->>R: false — falls through to real NIP-98/session auth
                end
            end
        end
    end
    Note over B: ADR-2039 — VISIONCLAW_DEV_MODE full bypass is PEER-AGNOSTIC by design (Docker SNAT<br/>hides the real LAN headset address) — the loopback+opt-in gate above applies only to the<br/>SEPARATE Bearer dev-session-token path, not to VISIONCLAW_DEV_MODE itself — three gates<br/>(compile-time, runtime opt-in, boot-refusal in release) substitute for a peer check there.
```

## VC-03.12 `RateLimit` middleware — sliding window per identifier
```mermaid
sequenceDiagram
    autonumber
    participant R as request
    participant RL as RateLimitService::call<br/>src/middleware/rate_limit.rs:274
    participant ST as RateLimitState<br/>Arc~RwLock~ HashMap~String,VecDeque~Instant~~~ (:87-94)

    RL->>RL: extract_identifier — use_user_id? AuthenticatedUser.pubkey : realip_remote_addr (:195-217)
    RL->>ST: state.write().await
    ST->>ST: cleanup(config) — periodic prune (:134)
    ST->>ST: check_and_record(identifier, config) (:105)
    ST->>ST: window_start = now - config.window — pop_front expired entries
    alt history.len() < max_requests
        ST->>ST: push_back(now)
        ST-->>RL: true
    else at limit
        ST-->>RL: false
    end
    alt not allowed
        RL-->>R: 429 ErrorTooManyRequests(message) (:322)
    else allowed
        RL->>R: service.call(req)
    end
    Note over RL: applied at src/main.rs:1062 as RateLimit::per_minute(60) on the /api/settings<br/>scope only (rate_limit.rs:169) — 60 requests / 60s window, keyed by realip (use_user_id off by default)
```

## VC-03.13 `TimeoutMiddleware` — default 30s, per-path override
```mermaid
sequenceDiagram
    autonumber
    participant R as request
    participant TO as TimeoutMiddlewareService::call<br/>src/middleware/timeout.rs:110
    participant SVC as inner service chain

    TO->>TO: timeout_duration = config.get_timeout(path) (:37-41)
    Note over TO: TimeoutConfig::new(Duration::from_secs(30)).with_override("/api/admin/sync", 600s)<br/>constructed at src/main.rs:972-975 — endpoint_overrides is an exact-path HashMap match
    TO->>SVC: tokio::time::timeout(timeout_duration, service.call(req))
    alt completes within timeout_duration
        SVC-->>TO: Ok(result)
        TO-->>R: result (success or handler error passed through)
    else exceeds timeout_duration
        TO->>TO: error! "Request to {path} timed out after {ms}ms" (:120-124)
        TO-->>R: 504 ErrorGatewayTimeout (:126-129)
    end
```

## VC-03.14 `validation` middleware — content-length and injection helpers
```mermaid
sequenceDiagram
    autonumber
    participant R as request
    participant VM as ValidationMiddleware::call<br/>src/middleware/validation.rs:124
    participant VAL as validators module<br/>src/middleware/validation.rs:166

    VM->>VM: read Content-Length header
    alt length > config.max_content_length
        VM-->>R: 413 PayloadTooLarge "Payload too large. Max size: {n} bytes" (:132-142)
    end
    opt config.validate_json AND Content-Type contains "application/json"
        VM->>VM: debug! "Validated JSON content-type" (:149-152) — no further parsing done here
    end
    VM->>R: service.call(req)
    Note over VAL: validators::{validate_iri, validate_url, validate_string_length,<br/>check_sql_injection, validate_enum, validate_range} (:183-283) — free functions<br/>callable from handlers, NOT invoked automatically by ValidationMiddleware
    Note over VM: MAX_ONTOLOGY_SIZE=10MiB (:18), MAX_REQUEST_SIZE=1MiB (:21, ValidationConfig::default),<br/>MAX_STRING_LENGTH=100KiB (:24). ValidateInput::for_ontology() sets 10MiB + check_injection=false.
    Note over VM: DOC-DRIFT / dead-code — grep across src/main.rs and every handler configure() finds<br/>no .wrap(ValidateInput::...) call anywhere — this middleware is defined and unit-tested<br/>(validation.rs:284-323) but is NOT wired into the actix App chain in this commit.
```

## VC-03.15 Identity authority chain end-to-end — Nostr key to role
```mermaid
sequenceDiagram
    autonumber
    participant K as secp256k1 keypair (client-held)
    participant U as uri::did_nostr<br/>src/uri/mod.rs:219
    participant NS as NostrService::verify_nip98_auth<br/>src/services/nostr_service.rs:579
    participant SP as init_pod_nip98<br/>src/handlers/solid_proxy_handler.rs:1349
    participant RS as RoleStore::effective_role<br/>src/services/role_store.rs:359
    participant AL as AccessLevel<br/>src/utils/auth.rs:16

    K->>U: pubkey (64-char lowercase hex, Schnorr)
    U->>U: did_nostr(pubkey) -> "did:nostr:{pubkey}" (DID_NOSTR_PREFIX, uri/mod.rs:47)
    Note over U: parse() round-trips did:nostr:* to ParsedUri::DidNostr (uri/mod.rs:491-498)<br/>cross_from_agentbox treats an already-converged did:nostr:* as identity, structural passthrough (:636-655)
    K->>NS: sign kind-27235 NIP-98 event (see VC-03.4)
    NS-->>K: NostrUser{pubkey, is_power_user}
    opt Solid pod provisioning
        K->>SP: POST /api/solid/pods/init-nip98 (Authorization: Nostr ...)
        SP->>SP: extract_user_identity(req) — re-verifies NIP-98 (this route sits under /api,<br/>so RbacGate ALSO requires WriteGraph via verify_access before the handler runs)
        SP->>SP: PublicKey::from_hex(pubkey).to_bech32() -> npub
        SP->>SP: ensure_pod_exists(npub, pubkey, pod_base_url)
        SP-->>K: { pod_url, webid: structure.profile, npub } (:1381-1387)
        Note over SP: GET /did/nostr:{pubkey} resolves a did+ld+json document via<br/>solid_pod_rs::interop::did_nostr::did_nostr_document (solid_proxy_handler.rs:1723-1730,<br/>route registered :1855). Full pod/LDP detail: see VC-26.
    end
    K->>RS: canonicalise_pubkey(pubkey) (role_store.rs:154) then effective_role(pubkey, is_power_user)
    RS-->>K: UserRole (explicit row / power-user Admin / configured_default / fail-closed Viewer)
    RS->>AL: role.to_access_level() (rbac.rs:83-89)
    Note over AL: AccessLevel drives verify_access / RbacGate for every subsequent /api call.<br/>Client-side signing/identity storage: see VC-33.
```

## VC-03.16 ADR-2013 federation-delegation boundary — NOT implemented
```mermaid
flowchart TB
    U["user's own Nostr keypair<br/>signs NIP-98 directly (VC-03.4)"]
    SS["Service-signed — nostr_bridge.rs<br/>struct field keys: Keys :29<br/>Keys::new(secret_key) :65<br/>event.sign_with_keys(&self.keys) :169"]
    DA["Delegated-agent-signed (NIP-26)<br/>agent signs ON BEHALF OF a user with a<br/>verifiable delegation tag"]
    HOOK["would-hook point: validate_nip98_token<br/>src/utils/nip98.rs:270 — tag extraction loop (:293-303)<br/>a #quot;delegation#quot; tag is never read; no NIP-26 verifier exists in this crate"]
    U -->|"authority IS the user's own key"| REAL["real, implemented (auth.rs, nip98.rs)"]
    SS -->|"re-signs under the BRIDGE's key,<br/>not the user's — original authority NOT carried"| BRIDGE_REAL["real, implemented, but NOT delegation"]
    DA -.->|"DIVERGENCE — NOT WIRED"| HOOK
    N1["DIVERGENCE (IDENTITY-authority-chain.md #154-162) — NIP-26 delegation deferred to the<br/>unbuilt Phase 5. Until it lands, no request can be attributed to a user THROUGH an agent.<br/>Key custody/rotation (legacy ADR-081) and delegated admin (legacy ADR-094) are frozen 2026-07-03.<br/>Pod signing can fall back unsigned (legacy agentbox ADR-026)."]
    HOOK --- N1
    N2["ADR-2013 scope note — the enterprise/delegation deferral governs VisionClaw's own request<br/>realm only; it does not imply a single request-credential realm across other repositories'<br/>verifiers (Request-credential review, IDENTITY-authority-chain.md 2026-09-04)."]
    DA --- N2
```
