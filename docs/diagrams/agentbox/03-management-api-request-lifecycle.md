---
id: AB-03
title: Management API request lifecycle and route table
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
  - agentbox/docs/INGRESS-identity.md
adrs: [ADR-2005, ADR-2013, ADR-2003]
sources:
  - agentbox/management-api/server.js
  - agentbox/management-api/lib/authz.js
  - agentbox/management-api/lib/audit-chain.js
  - agentbox/management-api/lib/failure-taxonomy.js
  - agentbox/management-api/lib/execution-journal.js
  - agentbox/management-api/observability/metrics.js
  - agentbox/scripts/agentbox-config-validate.js
  - agentbox/tests/contract/execution-journal.contract.spec.js
  - agentbox/agentbox.toml
  - agentbox/docker-compose.yml
  - agentbox/flake.nix
  - agentbox/management-api/adapters/events/local-jsonl.js
  - agentbox/management-api/middleware/consumer-payer.js
  - agentbox/management-api/middleware/spend-policy.js
  - agentbox/management-api/routes/admin-users.js
  - agentbox/management-api/routes/agent-events.js
  - agentbox/management-api/routes/approvals.js
  - agentbox/management-api/routes/beads.js
  - agentbox/management-api/routes/broker-bridge.js
  - agentbox/management-api/routes/dream.js
  - agentbox/management-api/routes/git-bridge.js
  - agentbox/management-api/routes/kg-elevation.js
  - agentbox/management-api/routes/linked-objects.js
  - agentbox/management-api/routes/memory.js
  - agentbox/management-api/routes/payments.js
  - agentbox/management-api/routes/pod-git.js
  - agentbox/management-api/routes/projects.js
  - agentbox/management-api/routes/sessions-boundary.js
  - agentbox/management-api/routes/tasks.js
  - agentbox/scripts/ci/check-ports-loopback.mjs
verified_commit: bed6b617d
---

## AB-03.1 server.js boot part 1 — Fastify construction, hooks, static route registers

```mermaid
sequenceDiagram
    autonumber
    participant Boot as server.js top-level
    participant App as fastify()<br/>server.js:81
    participant RawBody as registerRawBody<br/>middleware/auth.js:220
    participant CORS as fastify-cors<br/>server.js:183
    participant WS as fastify-websocket<br/>server.js:189
    participant RL as fastify-rate-limit<br/>server.js:192
    participant Swag as fastify-swagger(-ui)<br/>server.js:271-330
    participant Routes as route modules<br/>server.js:333-421

    Boot->>App: fastify({logger, trustProxy true, maxParamLength 512})<br/>server.js:81-92
    Note over App: INVARIANT maxParamLength 512 so urn-agentbox-bead content-addressed ids over 100 chars do not 404 in find-my-way
    Boot->>RawBody: registerRawBody(app)<br/>server.js:101
    RawBody->>App: addContentTypeParser application-json parseAs buffer<br/>middleware/auth.js:221-237
    Note over RawBody: sets req.rawBody Buffer, still delivers parsed JSON, must run before route parsers (server.js:94-101)
    Boot->>App: app.register(cors, allowedOrigins credentials true)<br/>server.js:183-186
    Boot->>App: app.register(websocket)<br/>server.js:189
    Boot->>App: app.register(rateLimit max 100 per 1 minute allowList 127.0.0.1)<br/>server.js:192-199
    Boot->>App: addHook onRequest sets request.startTime<br/>server.js:202-204
    Boot->>App: addHook onResponse metrics.recordHttpRequest<br/>server.js:206-214
    Note over App: see AB-03.9 for the http_request_duration_seconds histogram this hook feeds
    Boot->>App: addHook preValidation runs authMiddleware unless url on public allowlist<br/>server.js:227-268
    Note over App: public allowlist livez health ready metrics v1-meta lo-star docs-star well-known-did well-known-x402 (server.js:229-265)
    Boot->>App: app.register(fastify-swagger openapi 3.0.0)<br/>server.js:271-320
    Boot->>App: app.register(fastify-swagger-ui routePrefix /docs)<br/>server.js:322-330
    Boot->>Routes: app.register(routes/tasks) prefix empty<br/>server.js:333-338
    Boot->>Routes: app.register(routes/status)<br/>server.js:340-346
    Boot->>Routes: app.register(routes/comfyui)<br/>server.js:348-353
    Boot->>Routes: app.register(routes/agent-events)<br/>server.js:355-359
    Boot->>App: addHook onReady starts agent-events WS forwarder to host<br/>server.js:367-387
    Boot->>Routes: app.register(routes/memory) prefix empty<br/>server.js:391
    Boot->>Routes: app.register(routes/broker-bridge)<br/>server.js:396
    Boot->>Routes: app.register(routes/git-bridge)<br/>server.js:402
    Boot->>Routes: app.register(routes/pod-git)<br/>server.js:409
    Boot->>Routes: app.register(routes/payments) with metrics<br/>server.js:413
    Boot->>Routes: app.register(routes/llm-marketplace)<br/>server.js:417
    Boot->>Routes: app.register(routes/dream)<br/>server.js:421
    Boot->>App: app.get /livez /ready /health /health/pods /v1/meta /metrics /<br/>server.js:425-681
    Note over App: ADR-2003 server.js is baked by flake.nix from agentbox.toml at image build — not a<br/>runtime install (see AB-02.1 for the surrounding boot phases)
```

## AB-03.2 server.js boot part 2 — async start() manifest-gated route mounts

```mermaid
sequenceDiagram
    autonumber
    participant Start as start()<br/>server.js:871
    participant Manifest as loadManifest<br/>adapters/manifest-loader.js
    participant App as fastify app
    participant Adapters as resolveAdapters<br/>adapters/index.js

    Start->>Manifest: loadManifest()<br/>server.js:876
    alt manifest file missing
        Manifest-->>Start: ManifestNotFound<br/>server.js:879-881
        Start->>Start: manifest = {} all-off defaults
    end
    Start->>Adapters: resolveAdapters(manifest)<br/>server.js:902
    Start->>App: app.decorate adapters resolvedAdapters<br/>server.js:903
    Note over Start,Adapters: see AB-04.1 for slot to implementation resolution detail
    Start->>App: register middleware/linked-data createEncoder if linked_data.enabled<br/>server.js:909-931
    Start->>App: register routes/uri-resolver always mounted<br/>server.js:940
    Start->>App: register routes/system logger manifest adapters<br/>server.js:954
    Start->>App: register routes/voice-intent with dispatchActionRequest<br/>server.js:972
    Start->>App: register routes/kg-elevation self-gates on sovereign_mesh.kg_elevation<br/>server.js:986
    Start->>App: new ProjectTracker + register routes/projects<br/>server.js:1003-1016
    opt project_tracking.enabled true
        Start->>Start: tracker.scan() then tracker.startScheduler()<br/>server.js:1020-1024
    end
    alt sovereign_mesh.multi_user.enabled true
        Start->>App: register routes/admin-users<br/>server.js:1049
    else disabled default
        Start->>Start: /admin/users/star not mounted<br/>server.js:1059
    end
    Start->>App: resolveViewerImpl + register routes/linked-objects<br/>server.js:1069-1072
    Start->>App: register routes/well-known x402 discovery<br/>server.js:1092
    Start->>App: buildAuthorityConsumer + buildAuthorityGate, decorate authorityConsumer authorityGate<br/>server.js:1111-1121
    Note over Start: fallback to governance-decision-waiter.awaitDecision when the relay signer is unavailable (server.js:1115-1117)
    Start->>App: register routes/beads<br/>server.js:1135
    Start->>App: register routes/mandate<br/>server.js:1146
    Start->>App: register routes/sessions-boundary<br/>server.js:1156
    Start->>App: register routes/approvals<br/>server.js:1166
    Start->>Start: log SecurityProfileApplied resolved posture<br/>server.js:1225-1231
    Start->>Adapters: connectAdapters per-slot deadline<br/>server.js:1241
    Note over Start,Adapters: see AB-04.6 connectAdapters per-slot deadline detail, not re-drawn here
    opt AGENTBOX_RELAY_ENABLED and AGENTBOX_RELAY_POD_BRIDGE true
        Start->>Start: new RelayConsumer(npubs...).start()<br/>server.js:1284-1301
    end
    opt junkiejarvis enabled via env or manifest
        Start->>Start: startJunkieJarvis bridge logger<br/>server.js:1335-1343
    end
    Start->>Start: headroom.init(logger) decorate headroom<br/>server.js:1360-1368
    Start->>Start: initTracing, setBuildInfo, startMetricsServer<br/>server.js:1372-1374
    Start->>App: app.listen port PORT host HOST<br/>server.js:1377
    Note over Start: MANAGEMENT_API_PORT default 9090 (server.js:42) HOST default 0.0.0.0 (server.js:47)<br/>compose publishes 127.0.0.1 9090 9090 only
```

## AB-03.3 generic request lifecycle — Fastify hook phases, auth and validation failure

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant OnReq as onRequest hook<br/>server.js:202
    participant PreVal as preValidation hook<br/>server.js:227
    participant Auth as authMiddleware<br/>middleware/auth.js:167
    participant Schema as Fastify schema validation
    participant PreH as route preHandler<br/>e.g. costGate/paymentGate/authz
    participant H as route handler
    participant OnResp as onResponse hook<br/>server.js:206

    C->>OnReq: HTTP request
    OnReq->>OnReq: request.startTime = Date.now()<br/>server.js:203
    Note over OnReq: body not yet parsed here (registerRawBody parser runs after onRequest, before preValidation)
    OnReq->>PreVal: proceed
    alt url on public allowlist
        PreVal->>H: skip authMiddleware entirely<br/>server.js:229-265
    else url requires auth
        PreVal->>Auth: authMiddleware(request, reply)<br/>server.js:267
        Auth->>Auth: verifyNip98Header + verifyBearerHeader<br/>middleware/auth.js:169-170
        alt neither auth mode accepted for configured authMode
            Auth-->>C: 401 Unauthorized typed message<br/>middleware/auth.js:180-196
        else authenticated
            Auth->>Auth: request.auth = {mode, pubkey?}<br/>middleware/auth.js:198
            Auth->>Schema: continue to schema validation
        end
    end
    alt request body fails JSON schema
        Schema-->>C: 400 Bad Request fastify default error shape
    else schema ok
        Schema->>PreH: continue
    end
    opt route declares a preHandler
        PreH->>PreH: e.g. costGate routes/tasks.js:42 or paymentGate routes/comfyui.js:74 or authz.requireOperator routes/mandate.js:311
        alt preHandler rejects
            PreH-->>C: 402 or 403 typed response, handler never runs
        end
    end
    PreH->>H: route handler executes
    H-->>C: reply.send(...)
    H->>OnResp: response phase
    OnResp->>OnResp: metrics.recordHttpRequest(method, routerPath, statusCode, duration)<br/>server.js:207-213
    Note over OnResp: duration = (Date.now() minus request.startTime) divided by 1000, feeds<br/>http_request_duration_seconds (observability/metrics.js:58)
```

## AB-03.4 lib/authz.js — identity resolution predicates

```mermaid
sequenceDiagram
    autonumber
    participant Req as request.auth<br/>middleware/auth.js:198
    participant AP as authenticatedPubkey<br/>lib/authz.js:114
    participant OP as isOperator<br/>lib/authz.js:90
    participant AL as approvalAllowlist<br/>lib/authz.js:71
    participant SA as isSessionAgent<br/>lib/authz.js:160

    Note over Req: identity sources - NIP-98 pubkey header from nip98-proxy<br/>OR Bearer MANAGEMENT_API_KEY OR session-bound did in sessions-boundary registry
    Req->>AP: authenticatedPubkey(request)<br/>lib/authz.js:114-125
    alt auth.mode is nip98
        AP-->>AP: return normalised auth.pubkey if 64-hex<br/>lib/authz.js:116-119
    else auth.mode is bearer
        AP-->>AP: return operatorPubkey() — bearer IS the operator key<br/>lib/authz.js:120-123
    else no auth
        AP-->>AP: return null<br/>lib/authz.js:124
    end
    AP->>OP: isOperator(pubkey)<br/>lib/authz.js:90-94
    OP->>OP: pubkey equals operatorPubkey() from AGENTBOX_X_ONLY_PUBKEY_HEX or AGENTBOX_PUBKEY<br/>lib/authz.js:53-56
    AP->>AL: approvalAllowlist(manifest)<br/>lib/authz.js:71-87
    AL->>AL: operator plus sovereign_mesh.relay.allowed_pubkeys<br/>plus AGENTBOX_RELAY_ALLOWED_PUBKEYS or AGENTBOX_APPROVAL_ALLOWLIST
    Note over AL: isApprover(pk) is allowlist.has(pk) — lib/authz.js:97-101
    AP->>SA: isSessionAgent(pk, sessionId)<br/>lib/authz.js:160-170
    SA->>SA: reads AGENTBOX_STATE_DIR sessions JSON<br/>matches record.pubkey or hex inside record.did (lib/authz.js:136-153)
    Note over SA: default state dir var-lib-agentbox (lib/authz.js:137), independent of the on-disk filename hashing scheme
```

## AB-03.5 lib/authz.js — requireOperator and requireApprover preHandler gates

```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant RO as requireOperator<br/>lib/authz.js:184
    participant RA as requireApprover<br/>lib/authz.js:207
    participant H as route handler

    rect rgb(232,244,255)
    Note over RO: consumed by routes/mandate.js create revoke list<br/>lib/authz.js:311,389,437
    C->>RO: preHandler(request, reply)
    alt auth.mode is bearer
        RO->>H: allowed unconditionally — bearer IS the operator credential<br/>lib/authz.js:188
    else auth.mode is nip98 and isOperator(auth.pubkey)
        RO->>H: allowed<br/>lib/authz.js:189
    else neither condition holds
        RO-->>C: 403 forbidden_not_operator<br/>lib/authz.js:190-193
    end
    end

    rect rgb(255,240,230)
    Note over RA: preHandler form, plus an in-handler equivalent in routes/approvals.js:132-149
    C->>RA: preHandler(request, reply)
    alt auth.mode is bearer or pubkey missing
        RA-->>C: 403 forbidden_not_approver — bearer can never approve<br/>lib/authz.js:213-217
    else nip98 present
        RA->>RA: isApprover(auth.pubkey) checks approvalAllowlist<br/>lib/authz.js:218
        alt pubkey not on allowlist
            RA-->>C: 403 forbidden_not_approver<br/>lib/authz.js:219-224
        else on allowlist
            RA->>H: allowed
        end
    end
    end
    Note over RO,RA: ADR-2013 — 9096 is the ONE LAN-published ingress door<br/>(docker-compose.yml:54, sanctioned check-ports-loopback.mjs:77) — 9090 mgmt-api and<br/>9095 aoe serve are loopback-only (docker-compose.yml:55)
```

## AB-03.6 route table (a) — system, status, well-known, uri-resolver, meta probes

```mermaid
flowchart TD
    subgraph probes["public probes — auth-skip allowlist server.js:229-265"]
        A1["GET /livez<br/>server.js:425"] --> H1["event-loop-alive only<br/>server.js:439"]
        A2["GET /ready<br/>server.js:444"] --> H2["bootstrap+adapters+paths check<br/>server.js:475-527"]
        A3["GET /health<br/>server.js:543"] --> H3["adapterHealth snapshot<br/>server.js:564-575"]
        A4["GET /health/pods<br/>server.js:580"] --> H4["probePodHealth<br/>server.js:115-179"]
        A5["GET /v1/meta<br/>server.js:601"] --> H5["image_hash + adapter contract versions<br/>server.js:627-644"]
        A6["GET /metrics<br/>server.js:647"] --> H6["metrics.register.metrics<br/>server.js:660"]
        A7["GET /"] --> H7["endpoint index<br/>server.js:664-681"]
    end
    subgraph sysroutes["routes/system.js — authed, always mounted"]
        B1["GET /v1/system<br/>routes/system.js:33"] --> HB1["buildSystemView + buildExecutionCoverage<br/>routes/system.js:37-44"]
        B2["GET /v1/system/audit-chain<br/>routes/system.js:47"] --> HB2["auditChain.verifyFiles(?days=N)<br/>routes/system.js:56-76 see AB-03.10"]
    end
    subgraph wellknown["public discovery"]
        C1["GET /.well-known/x402.json<br/>routes/well-known.js:64"] --> HC1["cached manifest or 404<br/>routes/well-known.js:34-51,90"]
        C2["GET /.well-known/did.json<br/>auth-skip only server.js:257"] -.->|"not a fastify route here"| HC2["served by solid-pod-rs<br/>lib/uris.js:244"]
    end
    subgraph uri["routes/uri-resolver.js — authed, always mounted"]
        D1["GET /v1/uri/:urn<br/>routes/uri-resolver.js:42"] --> HD1["200+307 / 404 / 410 per kind<br/>routes/uri-resolver.js:8-16"]
        D2["GET /v1/uri<br/>routes/uri-resolver.js:154"] --> HD2["resolver capability listing"]
    end
    Note1["DOC-DRIFT candidate: BASELINE-container.md verified_commit 73540faa0<br/>is stale — every route line above re-derived against working tree (b00c28a0d)"]
```

## AB-03.7 route table (b) — tasks, beads, projects

```mermaid
flowchart TD
    subgraph tasks["routes/tasks.js"]
        T1["POST /v1/tasks<br/>tasks.js:15<br/>guard costGate tasks.js:42"] --> TH1["processManager.spawn"]
        T2["GET /v1/tasks/:taskId<br/>tasks.js:71"] --> TH2["task status lookup"]
        T3["GET /v1/tasks<br/>tasks.js:125"] --> TH3["list active tasks"]
        T4["DELETE /v1/tasks/:taskId<br/>tasks.js:163"] --> TH4["processManager.stop"]
        T5["GET /v1/tasks/:taskId/logs/stream<br/>tasks.js:233"] --> TH5["log tail stream"]
    end
    subgraph beads["routes/beads.js — self-gates 503 when beads adapter off"]
        B1["GET /v1/beads<br/>beads.js:93"] --> BH1["list beads via adapter"]
        B2["GET /v1/beads/:id<br/>beads.js:121"] --> BH2["get bead"]
        B3["POST /v1/beads/epics<br/>beads.js:140"] --> BH3["create epic — ADR-043 D4.3"]
        B4["POST /v1/beads/:id/children<br/>beads.js:168"] --> BH4["add child bead"]
        B5["POST /v1/beads/:id/deps<br/>beads.js:197"] --> BH5["addDependency — see AB-04.10"]
        B6["POST /v1/beads/:id/claim<br/>beads.js:234"] --> BH6["claim bead"]
        B7["POST /v1/beads/:id/close<br/>beads.js:258"] --> BH7["close bead"]
    end
    subgraph projects["routes/projects.js — 503 when project_tracking.enabled false"]
        P1["GET /v1/projects<br/>projects.js:106"] --> PH1["tracker.list"]
        P2["GET /v1/projects/:id<br/>projects.js:146"] --> PH2["tracker.get"]
        P3["GET /v1/projects/:id/activity<br/>projects.js:185"] --> PH3["git commit activity"]
        P4["POST /v1/projects/scan<br/>projects.js:233"] --> PH4["tracker.scan — server.js:1020"]
        P5["POST /v1/projects/:id/primer<br/>projects.js:274"] --> PH5["PrimerGenerator — server.js:1003"]
        P6["POST /v1/projects/:id/publish<br/>projects.js:327"] --> PH6["kind-30841 nostr digest publish"]
    end
```

## AB-03.8 route table (c) — memory, kg-elevation, linked-objects

```mermaid
flowchart TD
    subgraph memory["routes/memory.js — 503 when adapters.pods off, guard privacy-filter"]
        M1["POST /v1/memory<br/>memory.js:109<br/>guard middleware/privacy-filter.js:28"] --> MH1["write to Solid pod memory"]
        M2["GET /v1/memory/:key<br/>memory.js:196"] --> MH2["read memory entry"]
        M3["POST /v1/memory/search<br/>memory.js:229"] --> MH3["search memory"]
        M4["GET /v1/memory<br/>memory.js:267"] --> MH4["list memory keys"]
    end
    subgraph kg["routes/kg-elevation.js — self-gates on sovereign_mesh.kg_elevation"]
        K1["POST /v1/kg-elevation/scan<br/>kg-elevation.js:67<br/>guard verifyAgentEventRequest kg-elevation.js:105"] --> KH1["scan personal KG via memory adapter<br/>emit LINK beams + ontology-propose descriptor"]
    end
    subgraph lo["routes/linked-objects.js — viewer.mountPath, auth-skip for /lo/*"]
        L1["GET /lo/*<br/>linked-objects.js:133"] --> LH1["static viewer bundle — no auth (server.js:243-245)"]
        L2["GET viewer/manifest.json<br/>linked-objects.js:154"] --> LH2["pane manifest"]
        L3["GET viewer/panes/:file<br/>linked-objects.js:168"] --> LH3["pane asset"]
        L4["GET viewer/proxy<br/>linked-objects.js:206"] --> LH4["proxy fetch of a resource for the viewer"]
        L5["GET viewer.mountPath and viewer.mountPath/*<br/>linked-objects.js:258,261,271,275"] --> LH5["viewer shell — enabled/disabled variants"]
    end
    Note1["INVARIANT: /v1/* data endpoints stay authed even when the /lo static bundle is public (server.js:239-245)"]
```

## AB-03.9 route table (d) — sessions-boundary, agent-events, admin-users, approvals, mandate

```mermaid
flowchart TD
    subgraph sb["routes/sessions-boundary.js — ADR-043 D4.1-D4.5"]
        S1["POST /v1/sessions/boundary<br/>sessions-boundary.js:166"] --> SH1["bind did:nostr + URN + beads epic + memory namespace"]
    end
    subgraph ae["routes/agent-events.js"]
        E1["GET /v1/agent-events/stream<br/>agent-events.js:53<br/>websocket true"] --> EH1["WS push of agent action events"]
        E2["GET /v1/agent-events<br/>agent-events.js:124"] --> EH2["poll recent events"]
        E3["POST /v1/agent-events/emit<br/>agent-events.js:246<br/>guard verifyAgentEventRequest + reconcileSourceUrn"] --> EH3["emitAgentAction — failure classified via AB-03.11"]
        E4["POST /v1/agent-events/batch<br/>agent-events.js:367<br/>same guard per event"] --> EH4["batch emit"]
        E5["GET /v1/agent-events/types<br/>agent-events.js:462"] --> EH5["AgentActionType enum listing"]
        E6["POST /v1/agent-events/hook<br/>agent-events.js:496"] --> EH6["Claude Code hook ingest"]
        E7["GET /v1/agent-events/registry<br/>agent-events.js:546"] --> EH7["registry snapshot"]
        E8["GET /v1/agent-events/status<br/>agent-events.js:569"] --> EH8["forwarder status"]
    end
    subgraph au["routes/admin-users.js — mounted only if sovereign_mesh.multi_user.enabled"]
        AU1["POST /admin/users/provision<br/>admin-users.js:137"] --> AUH1["501 stub — see PRD-007 (server.js:1043)"]
        AU2["POST /admin/users/:pubkey/git-init<br/>admin-users.js:195"] --> AUH2["501 stub"]
        AU3["POST /admin/users/:pubkey/suspend<br/>admin-users.js:229"] --> AUH3["501 stub"]
        AU4["POST /admin/users/:pubkey/archive<br/>admin-users.js:245"] --> AUH4["501 stub"]
    end
    subgraph ap["routes/approvals.js — ADR-043 D4.7"]
        AP1["GET /v1/approvals<br/>approvals.js:51"] --> APH1["authorityConsumer.listPending or wired false"]
        AP2["POST /v1/approvals/:id/decide<br/>approvals.js:92<br/>guard nip98-only + isApprover approvals.js:132-149"] --> APH2["sign+publish kind-31403 — see AB-03.5"]
    end
    subgraph md["routes/mandate.js — ADR-043 D4.5, guard requireOperator"]
        MD1["POST /v1/mandate<br/>mandate.js:310<br/>guard requireOperator mandate.js:311"] --> MDH1["create scoped WAC mandate"]
        MD2["POST /v1/mandate/revoke<br/>mandate.js:388<br/>guard requireOperator mandate.js:389"] --> MDH2["revoke mandate"]
        MD3["GET /v1/mandate<br/>mandate.js:436<br/>guard requireOperator mandate.js:437"] --> MDH3["list mandates"]
    end
```

## AB-03.10 route table (e1) — payments, llm-marketplace, dream, comfyui

```mermaid
flowchart TD
    subgraph pay["routes/payments.js — x402 web ledger"]
        PY1["GET /v1/pay/info<br/>payments.js:145"] --> PYH1["ledger info"]
        PY2["GET /v1/pay/balance<br/>payments.js:203"] --> PYH2["balance via solid-pod-rs"]
        PY3["POST /v1/pay/deposit<br/>payments.js:278"] --> PYH3["deposit — consumed by middleware/consumer-payer.js"]
        PY4["POST /v1/pay/estimate<br/>payments.js:359"] --> PYH4["cost estimate"]
        PY5["POST /v1/pay/buy<br/>payments.js:446"] --> PYH5["buy compute/LLM resource"]
        PY6["POST /v1/pay/withdraw<br/>payments.js:569"] --> PYH6["withdraw"]
    end
    subgraph llm["routes/llm-marketplace.js — Nostr kinds 38300-38305"]
        LM1["POST /v1/llm/advertise<br/>llm-marketplace.js:84"] --> LMH1["advertise LLM compute"]
        LM2["DELETE /v1/llm/advertise<br/>llm-marketplace.js:145"] --> LMH2["withdraw advert"]
        LM3["GET /v1/llm/discover<br/>llm-marketplace.js:164"] --> LMH3["discover offers"]
        LM4["POST /v1/llm/request<br/>llm-marketplace.js:205"] --> LMH4["request a grant"]
        LM5["POST /v1/llm/grant<br/>llm-marketplace.js:259"] --> LMH5["grant access"]
        LM6["POST /v1/llm/deny<br/>llm-marketplace.js:325"] --> LMH6["deny request"]
        LM7["POST /v1/llm/receipt<br/>llm-marketplace.js:362"] --> LMH7["settle receipt"]
        LM8["POST /v1/llm/revoke<br/>llm-marketplace.js:422"] --> LMH8["revoke grant"]
        LM9["GET /v1/llm/grants<br/>llm-marketplace.js:512"] --> LMH9["list grants"]
        LM10["GET /v1/llm/stats<br/>llm-marketplace.js:534"] --> LMH10["marketplace stats"]
    end
    subgraph dream["routes/dream.js — operator-gated, ADR-055"]
        DR1["GET /dream/status<br/>dream.js:24"] --> DRH1["read-only per-repo dream ledgers"]
    end
    subgraph comfy["routes/comfyui.js — guard paymentGate GPU-metered"]
        CF1["POST /v1/comfyui/workflow<br/>comfyui.js:23<br/>guard paymentGate costSats 100 tier gpu comfyui.js:74"] --> CFH1["submit ComfyUI workflow"]
        CF2["GET /v1/comfyui/workflow/:workflowId<br/>comfyui.js:112"] --> CFH2["workflow status"]
        CF3["GET /v1/comfyui/models<br/>comfyui.js:163"] --> CFH3["list models"]
        CF4["GET /v1/comfyui/outputs<br/>comfyui.js:207"] --> CFH4["list outputs"]
        CF5["DELETE /v1/comfyui/workflow/:workflowId<br/>comfyui.js:251"] --> CFH5["cancel workflow"]
        CF6["GET /v1/comfyui/stream<br/>comfyui.js:319<br/>websocket true"] --> CFH6["WS progress stream"]
    end
```

## AB-03.11 route table (e2) — voice-intent, broker-bridge, git-bridge, pod-git

```mermaid
flowchart TD
    subgraph vi["routes/voice-intent.js — WS7, mandate-gated"]
        VI1["POST /v1/voice-intent<br/>voice-intent.js:82"] --> VIH1["transcript to agent intent<br/>dispatch signed 31402 or 503 when signer unavailable (server.js:966-968)"]
    end
    subgraph bb["routes/broker-bridge.js — G6"]
        BB1["GET /api/broker/bridge/inbox<br/>broker-bridge.js:252"] --> BBH1["enrichment review inbox"]
        BB2["GET /api/broker/bridge/cases/:id<br/>broker-bridge.js:330"] --> BBH2["case detail"]
        BB3["POST /api/broker/bridge/cases/:id/decide<br/>broker-bridge.js:371<br/>guard signed-31403 broker-bridge.js:443-445"] --> BBH3["zero-tolerance decision — DENY unless verified approve"]
        BB4["GET /api/broker/bridge/events<br/>broker-bridge.js:703"] --> BBH4["event feed"]
        BB5["GET /api/broker/bridge/cases/:id/history<br/>broker-bridge.js:776"] --> BBH5["case history"]
    end
    subgraph gb["routes/git-bridge.js — G5, BC20"]
        GB1["POST /v1/git/clone<br/>git-bridge.js:275"] --> GBH1["clone remote"]
        GB2["POST /v1/git/submit-enrichment<br/>git-bridge.js:396<br/>guard did match git-bridge.js:316,446"] --> GBH2["submit enrichment to judgment broker"]
        GB3["GET /v1/git/case-status/:caseId<br/>git-bridge.js:580"] --> GBH3["poll broker case status"]
        GB4["POST /v1/git/approve-callback<br/>git-bridge.js:636<br/>guard webhook sig git-bridge.js:690"] --> GBH4["broker decision callback"]
    end
    subgraph pg["routes/pod-git.js — JSS smart HTTP protocol, app.get/app.post"]
        PG1["GET /pods/:npub/.git/info/refs<br/>pod-git.js:166"] --> PGH1["git smart discovery"]
        PG2["POST /pods/:npub/.git/git-upload-pack<br/>pod-git.js:197"] --> PGH2["fetch/clone"]
        PG3["POST /pods/:npub/.git/git-receive-pack<br/>pod-git.js:221"] --> PGH3["push"]
        PG4["GET /pods/:npub/.git/HEAD<br/>pod-git.js:257"] --> PGH4["HEAD ref"]
        PG5["GET /pods/:npub/clone-url<br/>pod-git.js:279"] --> PGH5["resolve clone URL"]
    end
```

## AB-03.12 lib/audit-chain.js — hash-chained events append and verify

```mermaid
sequenceDiagram
    autonumber
    participant Adapter as LocalJsonlEventsAdapter.dispatch<br/>adapters/events/local-jsonl.js:61
    participant Chain as audit-chain.js<br/>lib/audit-chain.js
    participant Disk as YYYY-MM-DD.jsonl<br/>adapters/events/local-jsonl.js:114
    participant Sys as GET /v1/system/audit-chain<br/>routes/system.js:47

    Note over Adapter,Chain: hash = SHA256(prev_hash || canonical_json(record minus prev_hash,hash))<br/>lib/audit-chain.js:5,63-72
    Adapter->>Chain: _initChain — readTail(dir) on first dispatch<br/>local-jsonl.js:122-127, audit-chain.js:189-215
    Chain->>Disk: scan newest YYYY-MM-DD.jsonl, walk lines backward<br/>audit-chain.js:191-213
    alt a chained tail record found
        Disk-->>Chain: {prevHash record.hash, seq record.seq+1}<br/>audit-chain.js:205-208
    else legacy or empty tail
        Disk-->>Chain: {prevHash GENESIS_HASH, seq 0}<br/>audit-chain.js:36,210-214
    end
    Adapter->>Adapter: build record {ts,session_id,execution_id,kind,payload,seq}<br/>local-jsonl.js:66-73
    Adapter->>Chain: hashRecord(prevHash, record)<br/>local-jsonl.js:75, audit-chain.js:68-72
    Adapter->>Disk: append JSON line, only advance chain state on success<br/>local-jsonl.js:76-78,134-140
    Note over Adapter: session_urn threads execution-journal envelopes into this same chain — see AB-03.14

    Sys->>Chain: verifyFiles(files sorted by name)<br/>routes/system.js:67, audit-chain.js:147-183
    loop per file in order
        Chain->>Chain: verifyLines — recompute hashRecord per line, compare prev_hash and hash<br/>audit-chain.js:93-131
        alt unchained record before any chain started
            Chain-->>Chain: legacy_prefix++ tolerated<br/>audit-chain.js:112-116
        else prev_hash mismatch
            Chain-->>Sys: ok false reason splice — broken_at index<br/>audit-chain.js:117-119
        else hash mismatch
            Chain-->>Sys: ok false reason edit — content altered<br/>audit-chain.js:120-123
        end
    end
    Chain-->>Sys: {ok, checked, legacy_prefix, tail_hash, broken_at}<br/>audit-chain.js:147-183
    opt result.ok is false
        Sys->>Sys: logger.warn audit-chain.broken<br/>routes/system.js:69
    end
    Note over Chain: INVARIANT deletion at the tail is the one tamper mode a bare hash chain cannot see (lib/audit-chain.js:14-16)
```

## AB-03.13 lib/failure-taxonomy.js — MAST classifier on agent-events failure returns

```mermaid
sequenceDiagram
    autonumber
    participant R as POST /v1/agent-events/emit or /batch<br/>routes/agent-events.js:246,367
    participant V as verifyAgentEventRequest / reconcileSourceUrn<br/>agent-events.js:298,307
    participant T as taxonomy.tagFailure<br/>lib/failure-taxonomy.js:171
    participant C as taxonomy.classify<br/>lib/failure-taxonomy.js:143
    participant Pub as agentEventPublisher.emitAgentAction<br/>utils/agent-event-publisher.js:125

    R->>V: verifyAgentEventRequest(request)
    alt auth.ok is false
        V-->>R: {ok false, status, error}<br/>agent-events.js:299
        R->>T: tagFailure({error auth.error})<br/>agent-events.js:303
        T->>C: classify(ctx) — no mode, no reason, no stderr match<br/>lib/failure-taxonomy.js:146-159
        C-->>T: UNMAPPED sentinel<br/>lib/failure-taxonomy.js:30,159
        T-->>R: {failure_mode unmapped, failure_detail auth.error}<br/>lib/failure-taxonomy.js:171-183
        R-->>R: reply.code(auth.status).send success false + tag<br/>agent-events.js:304
    else auth ok, reconcile source_urn vs verified did
        R->>V: reconcileSourceUrn(claimed, auth.did)<br/>agent-events.js:307
        alt reconciliation fails — caller-claimed identity mismatch
            V-->>R: {ok false, status, error}<br/>agent-events.js:308
            R->>T: tagFailure({reason REASON.IDENTITY_MISMATCH, error})<br/>agent-events.js:312
            T->>C: classify — reason matches REASON_TO_MODE table<br/>lib/failure-taxonomy.js:148-150
            C-->>T: FM-1.2 Disobey Role Specification<br/>lib/failure-taxonomy.js:82,100
            T-->>R: {failure_mode FM-1.2, failure_detail preserved text}
            R-->>R: reply.code(rec.status).send success false + tag<br/>agent-events.js:313
        else reconciled
            R->>Pub: emitAgentAction(emitPayload)<br/>agent-events.js:353
            opt caller forwarded failure_mode
                Pub->>C: classify(ctx) — pass-through when ctx.mode is a known FM-x.y<br/>utils/agent-event-publisher.js:125, lib/failure-taxonomy.js:146
            end
        end
    end
    Note over C: 14 MAST modes across 3 categories (spec, inter-agent, verification) — Cemri et al. 2025<br/>lib/failure-taxonomy.js:33-62 — stderr heuristics are deliberately tiny, ambiguous<br/>text stays unmapped (lib/failure-taxonomy.js:119-124)
```

## AB-03.14 lib/execution-journal.js — ExecutionJournal.append envelope + D2 provenance gate

```mermaid
sequenceDiagram
    autonumber
    participant Caller as harness caller<br/>tests/contract/execution-journal.contract.spec.js
    participant J as ExecutionJournal.append<br/>lib/execution-journal.js:133
    participant Ev as injected events adapter<br/>opts.eventsAdapter (ADR-005 slot)
    participant M as assertModelRequestTraceable<br/>lib/execution-journal.js:208

    Caller->>J: append({session_urn, type, payload, ...})<br/>lib/execution-journal.js:133
    alt session_urn missing
        J-->>Caller: throw JournalError bad_event<br/>lib/execution-journal.js:135
    else type not in VOCABULARY
        J-->>Caller: throw JournalError bad_type — 15-member canonical vocabulary<br/>lib/execution-journal.js:39-53,136-138
    end
    opt event_id already committed for this session_urn
        J-->>Caller: return {envelope prior, duplicate true} — idempotent replay<br/>lib/execution-journal.js:145-149
    end
    J->>J: seq = _peekSeq(session_urn) — per-session monotonic, D1 unique key<br/>lib/execution-journal.js:108,151
    J->>J: build envelope {schema exec-event/1, event_id, seq, occurred_at, harness, agent_did, turn, type, payload, privacy_class}<br/>lib/execution-journal.js:153-167
    J->>Ev: dispatch({kind exec.<type>, session_id session_urn, execution_id eventId, payload envelope})<br/>lib/execution-journal.js:171-176
    Note over Ev: rides the ADR-005 events slot — inherits ADR-039 hash chain (see AB-03.12), ADR-008 privacy filter, ADR-012 JSON-LD
    Ev-->>J: dispatch resolves
    J->>J: commit _nextSeq and _seenIds only after successful append<br/>lib/execution-journal.js:179-182
    opt D2 strict mode — model-visible means journalled
        Caller->>M: assertModelRequestTraceable(request)<br/>lib/execution-journal.js:208
        alt any message/context item cites no journal seq
            M-->>Caller: throw UntraceableModelRequest (strict) or degraded model.requested (compatibility)<br/>lib/execution-journal.js:75-84
        end
    end
    Note over J: DOC-DRIFT candidate — no live require of lib/execution-journal.js from server.js or any routes/*.js today<br/>only lib/execution-coverage.js imports VOCABULARY/SCHEMA_ID and the contract tests instantiate ExecutionJournal directly
```

## AB-03.15 end-to-end authenticated write — nip98-proxy through audit-chain

```mermaid
sequenceDiagram
    autonumber
    participant Cli as external caller
    participant Proxy as nip98-proxy :9096<br/>config/nip98-proxy/
    participant App as fastify app :9090<br/>server.js:81
    participant PreVal as preValidation hook<br/>server.js:227
    participant Auth as authMiddleware<br/>middleware/auth.js:167
    participant Guard as authz.requireOperator<br/>lib/authz.js:184
    participant H as routes/mandate.js POST /v1/mandate<br/>mandate.js:310
    participant Ev as events adapter dispatch<br/>adapters/events/local-jsonl.js:61

    rect rgb(232,244,255)
    Note over Cli,Proxy: LAN-published trust boundary — the ONE ingress door (ADR-2013, docker-compose.yml:54)
    Cli->>Proxy: HTTPS request, NIP-98 kind-27235 Authorization header
    Proxy->>Proxy: verify Schnorr sig at the proxy, forward to upstream mgmt-api under /mgmt/<br/>agentbox/CLAUDE.md nip98-proxy entry
    end
    rect rgb(240,255,235)
    Note over App,H: loopback-only surface — 127.0.0.1 9090 9090 (docker-compose.yml:55)
    Proxy->>App: POST /mgmt/v1/mandate, Authorization Nostr base64-event, body JSON
    App->>App: onRequest sets request.startTime<br/>server.js:203
    App->>App: addContentTypeParser buffers body into request.rawBody<br/>middleware/auth.js:221-236
    App->>PreVal: preValidation phase<br/>server.js:227
    PreVal->>Auth: authMiddleware(request, reply) — url not on public allowlist<br/>server.js:267
    Auth->>Auth: verifyNip98Header — verifyNip98(header, method, url, request.rawBody)<br/>middleware/auth.js:67-97,82
    alt signature invalid or bridge absent
        Auth-->>Cli: 401 Unauthorized<br/>middleware/auth.js:192-196
    else verified
        Auth->>Auth: request.auth = {mode nip98, pubkey, event}<br/>middleware/auth.js:92-96,198
        Auth->>Guard: preHandler requireOperator({manifest})<br/>mandate.js:311, lib/authz.js:184
        alt auth.pubkey is not the operator key
            Guard-->>Cli: 403 forbidden_not_operator<br/>lib/authz.js:190-193
        else operator verified
            Guard->>H: handler runs
            H->>H: create scoped WAC mandate (ADR-043 D4.5)
            H->>Ev: emit mandate.created via events adapter
            Ev->>Ev: hashRecord(prevHash, record), append JSONL — see AB-03.12
            H-->>Cli: 200 mandate envelope
        end
    end
    end
    App->>App: onResponse records http_request_duration_seconds<br/>server.js:206-213, observability/metrics.js:58
```

## AB-03.16 middleware/*.js inventory — which routes and hooks actually consume each file

```mermaid
flowchart LR
    A["middleware/auth.js<br/>createAuthMiddleware + registerRawBody"] --> A1["global preValidation hook<br/>server.js:217,227,267"]
    B["middleware/cost-gate.js<br/>costGate"] --> B1["preHandler on POST /v1/tasks<br/>routes/tasks.js:7,42"]
    C["middleware/payment-gate.js<br/>paymentGate"] --> C1["preHandler on POST /v1/comfyui/workflow<br/>routes/comfyui.js:10,74"]
    D["middleware/privacy-filter.js<br/>ADR-008 layer 2"] --> D1["imported by routes/memory.js<br/>memory.js:28"]
    D --> D2["also wraps every adapter dispatch<br/>see AB-04.4/AB-04.5, not re-drawn here"]
    E["middleware/linked-data/*<br/>createEncoder, viewer, surfaces"] --> E1["booted in start() when linked_data.enabled<br/>server.js:909-931"]
    E --> E2["routes/linked-objects.js viewer mount<br/>server.js:1069-1072"]
    F["middleware/spend-policy.js<br/>spendPolicy factory"] -.->|"no require() from any routes/*.js or server.js"| F1["consumed only by scripts/agentbox-config-validate.js"]
    G["middleware/consumer-payer.js<br/>C2 native consumer payer"] -.->|"no require() from any routes/*.js or server.js"| G1["consumed only by scripts/agentbox-config-validate.js"]
    Note1["DOC-DRIFT candidate: spend-policy.js and consumer-payer.js are fully implemented<br/>but unwired at the HTTP route layer today (grep -rl across management-api routes/*.js server.js finds none)"]
```

## AB-03.17 DIVERGENCE check — raw-body content-type parser IS registered

```mermaid
flowchart TD
    Hyp["Hypothesis under test: registerRawBody is never called<br/>so direct body binding is inert on :9090"] --> G1["grep -n registerRawBody rawBody addContentTypeParser<br/>across management-api, config, flake.nix"]
    G1 --> F1["middleware/auth.js:220 defines registerRawBody(app)"]
    G1 --> F2["middleware/auth.js:221 addContentTypeParser application-json parseAs buffer"]
    G1 --> F3["server.js:101 calls registerRawBody(app) — before route registration, line 101"]
    F1 --> R["REFUTED: registerRawBody IS invoked at boot"]
    F2 --> R
    F3 --> R
    R --> Path["Working path: content-type parser sets request.rawBody Buffer<br/>preValidation-phase authMiddleware reads it for NIP-98 payload binding — see AB-03.15"]
    Path --> Note1["NOTE: two earlier sibling agents' unevidenced claim does not hold in this working tree (b00c28a0d)<br/>request.rawBody is populated for every application/json body before verifyNip98Header runs (middleware/auth.js:76-82, server.js:221-227)"]
```

