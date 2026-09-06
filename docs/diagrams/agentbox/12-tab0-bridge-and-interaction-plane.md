---
id: AB-12
title: tab0-bridge and the interaction plane
area: agentbox
governing:
  - agentbox/docs/INGRESS-identity.md
  - agentbox/docs/GOVERNANCE-capabilities.md
adrs: [ADR-2009, ADR-2010, ADR-2011, ADR-2047]
sources:
  - agentbox/config/tab0-bridge/server.mjs
  - agentbox/config/tab0-bridge/turn-sink.cjs
  - agentbox/config/tab0-bridge/start.sh
  - agentbox/config/tab0-bridge/deploy.sh
  - agentbox/config/tab0-bridge/package.json
  - agentbox/config/tab0-bridge/README.md
  - agentbox/management-api/lib/voice-intent.js
  - agentbox/management-api/routes/voice-intent.js
  - agentbox/management-api/lib/junkiejarvis-agent.js
  - agentbox/management-api/lib/agent-control-surface.js
  - agentbox/management-api/routes/approvals.js
  - agentbox/config/nip98-proxy/proxy.mjs
  - agentbox/voice/README.md
  - agentbox/docker-compose.voice.yml
  - agentbox/voice/console/Caddyfile
  - agentbox/flake.nix
  - agentbox/docker-compose.yml
  - agentbox/management-api/server.js
  - agentbox/scripts/ci/check-ports-loopback.mjs
  - agentbox/config/nostr-gateway/nostr-send.cjs
verified_commit: 7a20db228
---

## AB-12.1 Interaction-plane topology
```mermaid
flowchart TB
    Browser["Browser / mobile / voice cockpit"]
    Caddy8444["Caddy :8444 operator console<br/>published 0.0.0.0<br/>agentbox/docker-compose.voice.yml:39-40"]
    Caddy8443["Caddy :8443 stock Unmute debug<br/>published 0.0.0.0<br/>agentbox/docker-compose.voice.yml:39"]
    UnmuteFE["Unmute frontend :3000"]
    UnmuteBE["Unmute backend :80<br/>STT/TTS, /v1/realtime"]
    Bridge["tab0-bridge :8971<br/>agentbox/config/tab0-bridge/server.mjs:45<br/>never host-published"]
    Proxy["nip98-proxy :9096<br/>published 0.0.0.0<br/>agentbox/config/nip98-proxy/proxy.mjs:96-97"]
    AoE["AoE daemon :9095<br/>loopback only, never published"]
    Mgmt["management-api :9090<br/>127.0.0.1 only"]
    Tmux["tmux window agentbox:0<br/>coordinator session"]

    Browser -->|"https 8444, TLS"| Caddy8444
    Browser -->|"https 8443, debug only"| Caddy8443
    Caddy8443 -->|"handle_path /api/*"| UnmuteBE
    Caddy8443 -->|"handle default"| UnmuteFE
    Caddy8444 -->|"handle /embed*, /_next/*"| UnmuteFE
    Caddy8444 -->|"handle_path /api/*"| UnmuteBE
    UnmuteBE -->|"POST /v1/chat/completions, Bearer BRIDGE_TOKEN"| Bridge
    Caddy8444 -->|"handle /feed, /bridge/*, Authorization forwarded, ADR-069"| Proxy
    Caddy8444 -->|"handle_path /aoe/*, handle /approvals/*, /mgmt/*, /dream/*"| Proxy
    Caddy8444 -->|"handle /lo*, /docs*, direct"| Mgmt
    Proxy -->|"credential exchange, bearer_env BRIDGE_TOKEN, proxy.mjs:227-246"| Bridge
    Proxy -->|"Authorization Bearer daemon token, replaces browser credential"| AoE
    Proxy -->|"prefix /mgmt/, strip, NIP98_PROXY_MGMT_UPSTREAM"| Mgmt
    Bridge -->|"tmux send-keys -t agentbox:0, fail-open fallback"| Tmux
    Bridge -->|"POST /api/sessions/:id/send, Bearer aoe daemon token"| AoE

Divergence["RESOLVED ADR-2047: 8443 and 8444 are SANCTIONED exposures, not a breach.<br/>agentbox/scripts/ci/check-ports-loopback.mjs:92-103 lists ten sanctioned publishes with a<br/>per-entry citation at :55-73, voice 8443/8444 among them as the second LAN ingress,<br/>modelled not hidden. 9096 remains the sole IDENTITY-gated ingress to the AoE plane."]
    style Divergence fill:#ddf5dd,stroke:#2e7d32,color:#000
    Caddy8444 -.-> Divergence

Drift["RESOLVED ADR-2047: agentbox/voice/README.md now routes /feed and /bridge/* to<br/>agentbox:9096 in both its route table and its ASCII map, naming the ADR-069 server-side<br/>BRIDGE_TOKEN credential exchange. The one remaining 8971 reference is correct - it is the<br/>Unmute backend calling the bridge container-to-container, not a browser path through Caddy."]
    style Drift fill:#ddf5dd,stroke:#2e7d32,color:#000
    Proxy -.-> Drift
```

## AB-12.2 Bridge boot — reconcile, listen, coordinator resolve
```mermaid
sequenceDiagram
    autonumber
    participant Sup as Supervisor<br/>agentbox/flake.nix:2315
    participant Dep as deploy.sh<br/>agentbox/config/tab0-bridge/deploy.sh:1
    participant Node as server.mjs<br/>agentbox/config/tab0-bridge/server.mjs:45
    participant AoEd as AoE daemon :9095

    Sup->>Dep: bash deploy.sh reconcile (flake.nix:2316)
    Dep->>Dep: copy server.mjs, turn-sink.cjs, start.sh, package.json via md5 compare (deploy.sh:27-35)
    opt node_modules/ws missing
        Dep->>Dep: npm install --omit=dev (deploy.sh:37-39)
    end
    Dep-->>Sup: exit 0, reconcile-only mode, no launch (deploy.sh:44-46)
    Sup->>Node: exec node server.mjs, foreground, autorestart (flake.nix:2316)
    Node->>Node: read BRIDGE_PORT, default 8971 (server.mjs:45)
    Node->>Node: read BRIDGE_TMUX_SESSION, default agentbox (server.mjs:47)
    Node->>Node: read BRIDGE_TOKEN, default empty string (server.mjs:49)
    Node->>Node: read BRIDGE_BIND, default 0.0.0.0 (server.mjs:55)
    Node->>Node: BIND_IS_LOOPBACK check, 127.0.0.1, ::1, localhost (server.mjs:56)
    alt TOKEN empty and BIND not loopback
        Node->>Node: console.error, refusing to start (server.mjs:60-65)
        Node->>Node: process.exit(1)
        Note over Node: INVARIANT — a non-loopback bind with no BRIDGE_TOKEN is refused at startup, server.mjs:60-66
    end
    Node->>Node: delete CHILD_ENV.ANTHROPIC_API_KEY, empty key poisons OAuth chain (server.mjs:123-126)
    Node->>Node: http.createServer, WebSocketServer path /feed (server.mjs:712,797)
    Node->>Node: server.listen(PORT, BIND) (server.mjs:810)
    Node->>AoEd: GET /api/sessions?state=live, resolveCoordinatorSession (server.mjs:216-218)
    alt AoE reachable and title matches tab0
        AoEd-->>Node: 200, session list
        Node->>Node: pin aoeSessionId for process lifetime (server.mjs:227, ADR-044 D2)
    else AoE unreachable or no match
        AoEd-->>Node: error or no match
        Node->>Node: aoeSessionId stays null, fall back to tmux (server.mjs:231-234)
    end
    loop every 30000 ms while aoeSessionId is null
        Node->>AoEd: GET /api/sessions?state=live, re-resolve (server.mjs:820)
    end
```

## AB-12.3 Global HTTP auth gate
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant B as tab0-bridge<br/>server.mjs:712

    C->>B: HTTP request, any path (server.mjs:712)
    alt path is /health
        B-->>C: 200, ok true, no auth check (server.mjs:716-719)
    else path requires auth
        B->>B: authorised(req) (server.mjs:698)
        alt TOKEN is empty
            B->>B: return true, open gate, loopback dev only (server.mjs:699)
        else Authorization header equals Bearer TOKEN
            B->>B: return true (server.mjs:701)
        else Authorization starts with Nostr, verifyNip98Credential succeeds
            B->>B: return true, see AB-10.3 for NIP-98 verify internals (server.mjs:682-696,702)
        else query token equals TOKEN
            B->>B: return true (server.mjs:705)
        else query auth verifies as a Nostr credential
            B->>B: return true (server.mjs:706-707)
        else none of the above
            B->>B: return false (server.mjs:709)
        end
        alt authorised false
            B-->>C: 401, error unauthorised (server.mjs:722)
        else authorised true
            B->>B: dispatch to route table (server.mjs:723-784)
        end
    end
Note over B: INVARIANT — every surface except /health requires the bearer when BRIDGE_TOKEN is<br/>set, including /v1/chat/completions, /v1/models, /feed, /tab0/send, /nostr/*, /turns, /tabs*,<br/>/aoe/sessions — ADR-044 finding 1, server.mjs:619-634
```

## AB-12.4 WebSocket upgrade auth
```mermaid
sequenceDiagram
    autonumber
    participant Br as Browser
    participant WSS as WebSocketServer<br/>server.mjs:797, path /feed

    Br->>WSS: WS upgrade GET /feed, query token or query auth (server.mjs:799, 623-624, 630-632)
    WSS->>WSS: verifyClient(info) calls authorised(info.req) (server.mjs:800)
Note over WSS: verifyClient reuses the identical authorised() gate as HTTP requests, see<br/>AB-12.3 — browser WS clients cannot set an Authorization header, so query token / query auth is<br/>the only carrier (server.mjs:623-624,630-632)
    alt authorised
        WSS-->>Br: upgrade accepted, 101
        WSS->>WSS: on connection, send snapshot of last 50 turns (server.mjs:806-807)
    else not authorised
        WSS-->>Br: upgrade rejected, 401 (verifyClient false)
    end
```

## AB-12.5 Inject into tmux window 0 — sendToTab0
```mermaid
sequenceDiagram
    autonumber
    participant Cl as Client
    participant B as tab0-bridge<br/>server.mjs:746, POST /tab0/send
    participant S as sendToTab0()<br/>server.mjs:266
    participant A as aoeSend()<br/>server.mjs:242
    participant AoEd as AoE daemon :9095
    participant T as tmux CLI

    Cl->>B: POST /tab0/send, body text, source (server.mjs:746-748)
    B->>S: sendToTab0(text, source) (server.mjs:747)
    S->>S: clean = strip control chars, trim (server.mjs:267)
    alt clean is empty
        S-->>B: throw Error, empty text (server.mjs:268)
    end
    S->>A: aoeSend(clean) (server.mjs:271)
    A->>A: aoeSessionId unset, resolveCoordinatorSession() (server.mjs:243-245)
    A->>AoEd: POST /api/sessions/:id/send, Authorization Bearer aoe-token, body message clean (server.mjs:247)
    alt AoE responds 200
        AoEd-->>A: 200
    else AoE responds 404, session drift
        AoEd-->>A: 404
        A->>A: aoeSessionId = null, re-resolve once (server.mjs:249-251)
        A->>AoEd: retry POST /api/sessions/:id/send (server.mjs:252)
    else AoE unreachable or non-200
        AoEd-->>A: transport error or non-200 (server.mjs:254)
        A-->>S: throw Error
    end
    alt aoeSend succeeded
        S->>S: via = aoe (server.mjs:269)
    else aoeSend threw
        S->>S: via = tmux, console.error, AoE unreachable falling back (server.mjs:273-277)
        S->>T: tmux send-keys -t agentbox:0 -l clean (server.mjs:278)
        S->>T: tmux send-keys -t agentbox:0 Enter (server.mjs:279)
        Note over S: FAIL-OPEN — degrade to the byte-identical legacy tmux send-keys path, races AoE input accounting, ADR-044 D3 (server.mjs:273-277)
    end
    S->>S: pushTurn(voice-inject or nostr-inject, clean, via) (server.mjs:281)
    S-->>B: return clean text
    B-->>Cl: 200, ok true, sent clean (server.mjs:748)
```

## AB-12.6 AoE coordinator session resolution and drift retry
```mermaid
sequenceDiagram
    autonumber
    participant Ti as 30s interval<br/>server.mjs:820
    participant R as resolveCoordinatorSession()<br/>server.mjs:216
    participant AoEd as AoE daemon :9095

    Ti->>R: invoke when aoeSessionId is null (server.mjs:820)
    R->>AoEd: GET /api/sessions?state=live (server.mjs:218)
    alt status not 200
        AoEd-->>R: non-200
        R-->>Ti: return null (server.mjs:219)
    else status 200
        AoEd-->>R: 200, sessions array or wrapped data (server.mjs:204-209)
        R->>R: want = AOE_COORDINATOR_TITLE lowercased, default tab0 (server.mjs:117,220)
        R->>R: match session whose title, slug or name equals or includes want (server.mjs:221-224)
        alt match found with id or session_id
            R->>R: aoeSessionId = String(id), pin for process lifetime (server.mjs:227, ADR-044 D2)
            R-->>Ti: return aoeSessionId
        else no match
            R-->>Ti: return null (server.mjs:231)
        end
    end
    alt any error thrown, fetch abort, connection refused
        R-->>Ti: catch, return null, AoE not up yet (server.mjs:232-234)
    end
Note over R: INVARIANT — tab0-bridge targets exactly ONE pinned coordinator session<br/>(server.mjs:111-117). POST /tab0/send carries no per-request session id, so an<br/>arbitrary-session inject surface does not exist here — contrast the AoE daemon's own<br/>multi-session API reached via the proxy, see AB-10.x
```

## AB-12.7 Unmute voice loop through tab0-bridge
```mermaid
sequenceDiagram
    autonumber
    participant Mic as Browser mic, :8444 cockpit
    participant Cad as Caddy :8444<br/>voice/console/Caddyfile
    participant UFE as Unmute frontend :3000
    participant UBE as Unmute backend :80
    participant B as tab0-bridge<br/>server.mjs:726, POST /v1/chat/completions
    participant Cc as claude -p child<br/>server.mjs:300

    rect rgb(255,240,240)
    Note over Mic,UBE: trust boundary — LAN door 1, :8444 published 0.0.0.0
    Mic->>Cad: HTTPS, mic audio via /embed and /api/* (Caddyfile handle /embed*, handle_path /api/*)
    Cad->>UFE: reverse_proxy frontend:3000 (Caddyfile handle /embed*)
    Cad->>UBE: reverse_proxy backend:80, /v1/realtime (Caddyfile handle_path /api/*)
    end
    UBE->>B: POST /v1/chat/completions, Authorization Bearer BRIDGE_TOKEN aka KYUTAI_LLM_API_KEY, stream true (server.mjs:726-728, voice/README.md:27)
    B->>B: authorised(req), global gate, see AB-12.3
    alt userText equals the silence marker or is empty
        B-->>UBE: SSE, empty content, finish_reason stop, no LLM call (server.mjs:494-506)
    else userText has content
        B->>B: pushTurn(voice-user, text) (server.mjs:512)
        B->>B: metaSystemPrompt(), recent turns, tmux windows, AoE-or-tmux job instructions (server.mjs:358-426)
        B->>B: metaAllowedTools(), Bash allowlist, tmux and AOE_CURL patterns (server.mjs:434-454)
        B->>Cc: spawn claude -p --model haiku --append-system-prompt --allowedTools, stream-json (server.mjs:291-304)
        loop stream_event, content_block_delta, text_delta
            Cc-->>B: partial text chunk (server.mjs:318-321)
            B-->>UBE: SSE data chunk, delta content (server.mjs:519,536)
        end
        Cc-->>B: result event, final text or exit code (server.mjs:323-337)
        B->>B: pushTurn(voice-reply, full text) (server.mjs:545)
        B-->>UBE: SSE data DONE (server.mjs:549)
    end
    UBE-->>UFE: synthesised speech, TTS
Note over B: DIVERGENCE — :8444 and :8443 are published 0.0.0.0 by<br/>docker-compose.voice.yml:39-40, while only :9096 is the ADR-045 D2 sanctioned NIP-98-gated LAN<br/>door covered by the loopback CI gate. The Unmute voice loop itself reaches tab0-bridge only<br/>over the internal visionclaw_network hostname agentbox:8971, which is never host-published<br/>(docker-compose.yml:53-59)
```

## AB-12.8 mgmt-api voice-intent — mandate-gated ACSP dispatch
```mermaid
sequenceDiagram
    autonumber
    participant Ca as REST caller
    participant M as management-api<br/>routes/voice-intent.js:82
    participant VI as lib/voice-intent.js<br/>parseIntent:112
    participant Ma as lib/mandate.js<br/>see AB-11.10
    participant ACS as agent-control-surface.js<br/>buildActionRequest:176
    participant D as dispatchActionRequest<br/>server.js:822

Note over Ca,M: SCOPE — this route is not reached from the tab0-bridge cockpit or the Unmute<br/>voice loop, grep confirmed no reference to voice-intent.js under config/tab0-bridge. It is an<br/>independent management-api REST surface, included because the brief named it as an entry point.
    Ca->>M: POST /v1/voice-intent, transcript, actor_did, mandate (routes/voice-intent.js:82-109)
    M->>M: verifyAgentEventRequest(request), see AB-11.12 (routes/voice-intent.js:149)
    alt speaker auth not ok
        M-->>Ca: reply auth.status, error (routes/voice-intent.js:150-152)
    end
    alt mandate missing or not an object
        M-->>Ca: 403, mandate-required (routes/voice-intent.js:158-163)
    end
    M->>Ma: recordFromSignedMandate(mandate), see AB-11.10 (routes/voice-intent.js:166)
    alt recordFromSignedMandate throws
        M-->>Ca: 403, mandate-invalid (routes/voice-intent.js:168)
    end
    M->>M: verifyMandateEvent(mandate), Schnorr verifyEvent (routes/voice-intent.js:63-70,170)
    alt signature does not verify
        M-->>Ca: 403, mandate-unverified (routes/voice-intent.js:171-175)
    end
    M->>Ma: isMandateActive(mandateRecord) (routes/voice-intent.js:176)
    alt mandate revoked or expired
        M-->>Ca: 403, mandate-inactive (routes/voice-intent.js:177-181)
    end
    opt auth.did is present
        M->>Ma: reconcileSourceUrn(mandateRecord.agent, auth.did) (routes/voice-intent.js:186)
        alt grantee does not match verified speaker
            M-->>Ca: 403, mandate-speaker-mismatch (routes/voice-intent.js:188-192)
        end
    end
    M->>Ma: normalisePubkey(actor_did), validate target principal (routes/voice-intent.js:199)
    alt actor_did invalid
        M-->>Ca: 400, actor_did-invalid (routes/voice-intent.js:201-205)
    end
    M->>VI: transcriptToAction(transcript, actorRef, duration_ms) (routes/voice-intent.js:211)
    VI->>VI: parseIntent matches RULES — link, transform, delete, update, create, query, in order (lib/voice-intent.js:52-141)
    Note over VI: unrecognised utterance falls back to a read-only query, action_type QUERY, B3 fail-safe (lib/voice-intent.js:132-141)
    VI-->>M: intent — verb, action_type, subject, object, recognised
    M->>ACS: buildActionRequest(panelId, priority high, category voice-intent) (agent-control-surface.js:176-196, routes/voice-intent.js:225-246)
    ACS-->>M: unsigned kind-31402 event, d-tag panelId, p-tag actorPubkey
    alt no dispatchActionRequest wired
        M-->>Ca: 503, dispatch-unavailable (routes/voice-intent.js:218-223)
    end
    M->>D: dispatchActionRequest(unsigned), see AB-11.x for the authority gate (routes/voice-intent.js:250)
    alt dispatch throws
        M-->>Ca: 503, dispatch-failed (routes/voice-intent.js:251-257)
    end
    alt signedRequest has no id
        M-->>Ca: 503, dispatch-unsigned (routes/voice-intent.js:258-263)
    end
    D-->>M: signedRequest with id
    M->>M: emitAgentAction, beam-parity notification (routes/voice-intent.js:267-278)
    M-->>Ca: 200, dispatched true, speaker_did, actor_did, event_id, intent, dispatch (routes/voice-intent.js:286-309)
```

## AB-12.9 Inline NIP-98 approval decision
```mermaid
sequenceDiagram
    autonumber
    participant Op as Operator, :8444 cockpit
    participant Cad as Caddy :8444
    participant Pr as nip98-proxy :9096<br/>see AB-10.x
    participant M as management-api<br/>routes/approvals.js:51
    participant Az as lib/authz.js<br/>isApprover
    participant Cs as authority consumer<br/>signAndPublishDecision

Op->>Cad: GET /approvals/*, NIP-98 kind-27235 via window.nostr, or<br/>break-glass bearer (Caddyfile handle /approvals/*,<br/>voice/README.md:79-86)
Cad->>Pr: reverse_proxy agentbox:9096, Authorization forwarded<br/>(Caddyfile handle /approvals/*)
    Pr->>Pr: verify NIP-98 or session cookie, see AB-10.3, AB-10.6
Pr->>M: GET /v1/approvals, strip prefix, route table<br/>(routes/approvals.js:51)
    alt authority consumer not wired
M-->>Op: 200, approvals empty, wired false, note<br/>(routes/approvals.js:84-86)
    end
    M->>M: c.listPending() (routes/approvals.js:87)
M-->>Op: 200, approvals array, count, wired true<br/>(routes/approvals.js:88)
Op->>Op: operator reviews the list, signs a kind-27235 NIP-98 header via<br/>window.nostr for the decide POST
Op->>Cad: POST /approvals/:id/decide, body outcome or decision,<br/>reasoning
    Cad->>Pr: reverse_proxy agentbox:9096, Authorization forwarded
    Pr->>M: POST /v1/approvals/:id/decide (routes/approvals.js:92)
    alt request.auth.mode is not nip98
        M-->>Op: 401, nip98_required (routes/approvals.js:132-137)
    end
M->>Az: isApprover(request.auth.pubkey, manifest)<br/>(routes/approvals.js:143)
    alt pubkey not on the approval allowlist
        M-->>Op: 403, forbidden_not_approver (routes/approvals.js:148-152)
    end
    alt authority consumer unwired
        M-->>Op: 503, authority_consumer_unwired (routes/approvals.js:156-159)
    end
    M->>Cs: isDecided(id) (routes/approvals.js:167)
    alt already decided
        Cs-->>M: true, plus prior outcome
M-->>Op: 409, already_decided, request_event_id, outcome,<br/>response_event_id (routes/approvals.js:169-176)
    end
    M->>Cs: getPending(id) (routes/approvals.js:177)
    alt no pending request with that id
        M-->>Op: 404, unknown_request (routes/approvals.js:178-182)
    end
M->>M: normalise outcome, deny maps to reject<br/>(routes/approvals.js:186-193)
M->>Cs: signAndPublishDecision(requestId, outcome, reasoning)<br/>(routes/approvals.js:198)
    Cs-->>M: signed kind-31403 event id
M-->>Op: 200, success true, request_event_id, response_event_id,<br/>outcome, decided_by (routes/approvals.js:223-229)
Note over M: INVARIANT — the decision record is ALWAYS a Schnorr-signed<br/>kind-31403 event, never an unsigned approval, ADR-043 D4.7<br/>(routes/approvals.js:16-22)
Note over M,Cs: see AB-14.x for the governance approvals pipeline<br/>internals, signAndPublishDecision and the authority gate itself — not<br/>drawn here
```

## AB-12.10 AoE session board — cockpit proxy path and the bridge's own passthrough
```mermaid
sequenceDiagram
    autonumber
    participant Op as Operator console
    participant Cad as Caddy :8444
    participant Pr as nip98-proxy :9096
    participant AoEd as AoE daemon :9095
    participant B as tab0-bridge<br/>server.mjs:768

    rect rgb(235,245,255)
    Note over Op,AoEd: Part 1 — cockpit session board via the sole NIP-98 ingress, see AB-10.x for proxy verification internals
    Op->>Cad: GET /aoe/*, NIP-98 or nip07 session cookie (Caddyfile handle_path /aoe/*)
    Cad->>Pr: reverse_proxy agentbox:9096, Authorization forwarded
    Pr->>Pr: verifyNip98 or session cookie, X-Agentbox-Pubkey injected, see AB-10.3, AB-10.7
    Pr->>AoEd: forward, Authorization Bearer daemon token replaces browser credential, see AB-10.9 (proxy.mjs:850)
    AoEd-->>Pr: session list json
    Pr-->>Cad: response
    Cad-->>Op: session list rendered
    end
    rect rgb(255,245,235)
    Note over B,AoEd: Part 2 — tab0-bridge's own passthrough, used by the voice console feed, independent of the proxy path
    Op->>B: GET /aoe/sessions, Bearer BRIDGE_TOKEN or NIP-98, see AB-12.3 (server.mjs:768)
    B->>AoEd: aoeRequest GET /api/sessions?state=live, Authorization Bearer aoe-token from serve.url (server.mjs:772, 92-110)
    alt AoE responds non-200
        AoEd-->>B: non-200
        B-->>Op: 502, error aoe unavailable, status (server.mjs:773)
    end
    alt AoE unreachable, transport error
        B-->>Op: 502, error aoe unreachable, detail (server.mjs:775-777)
    end
    AoEd-->>B: 200, session list
    B-->>Op: 200, sessions array, coordinator aoeSessionId (server.mjs:774)
    end
```

## AB-12.11 junkiejarvis-agent.js listen and reply flow
```mermaid
sequenceDiagram
    autonumber
    participant R as Nostr relay pool, via NostrBridge
    participant Ag as JunkieJarvisAgent<br/>lib/junkiejarvis-agent.js:608
    participant Llm as callLlm()<br/>lib/junkiejarvis-agent.js:420

Note over R,Ag: SCOPE — junkiejarvis-agent.js has no reference to tab0-bridge, tmux, AoE or<br/>:8971, grep confirmed. An independent forum bot riding management-api's shared NostrBridge,<br/>included because the brief named it as an entry point.
    Ag->>R: bridge.subscribe, kinds 1059, filter p equals pubkey, gift-wrapped DMs (lib/junkiejarvis-agent.js:661-667)
    Ag->>R: bridge.subscribe, kinds 42, filter p equals pubkey, channel mentions (lib/junkiejarvis-agent.js:676-680)
    Ag->>Ag: _scheduleProfilePublish, setTimeout 2000 ms, then publish kind-0 profile (lib/junkiejarvis-agent.js:688-704)
    R-->>Ag: inbound event, kind 1059 or kind 42 (lib/junkiejarvis-agent.js:739)
    Ag->>Ag: _dedup(event.id), in-memory set capped at DEFAULT_DEDUP_CAP (lib/junkiejarvis-agent.js:646-655)
    alt already seen
        Ag->>Ag: drop event (lib/junkiejarvis-agent.js:648)
    end
    alt kind is 1059, gift wrap
        Ag->>Ag: nip59.unwrapEvent(wrap, signer.skBytes), recover rumor (lib/junkiejarvis-agent.js:757-761)
    else kind is 42, channel message
        Ag->>Ag: isChannelMention, p-tag or at-junkiejarvis text (lib/junkiejarvis-agent.js:124,805)
    end
    Ag->>Ag: _shouldIgnore(pubkey), self and configured ignore list (lib/junkiejarvis-agent.js:731-733)
    alt ignored author
        Ag->>Ag: drop, never answer self (lib/junkiejarvis-agent.js:632)
    end
    Ag->>Llm: callLlm(userText), brisk professional personality (lib/junkiejarvis-agent.js:420)
    Llm-->>Ag: reply text, or apology on outage, fail-open
    Ag->>Ag: truncateReply to maxReply, default 280 chars (lib/junkiejarvis-agent.js:245,627)
    alt source was a gift-wrapped DM
        Ag->>R: publish gift-wrapped reply, nip59, to the asker
    else source was a channel message
        Ag->>R: publish kind-42 reply, e-tag root preserved, p-tag asker (lib/junkiejarvis-agent.js:141-149)
    end
    Note over Ag: hasSchedulingIntent may buildCalendarEvent, kind-31923 NIP-52, on behalf of forum members (lib/junkiejarvis-agent.js:349,381)
```

## AB-12.12 agent-control-surface.js — ACSP panel producer
```mermaid
classDiagram
    class AgentControlSurface {
        +buildPanelDefinition(p) Event
        +buildPanelState(p) Event
        +buildActionRequest(p) Event
        +buildPanelUpdate(p) Event
        +buildPanelRetired(p) Event
        +publishPanelEvent(bridge, signer, unsignedEvent) Promise~Event~
    }
    class PanelDefinition {
        +kind 31400
        +title string
        +schema PANEL_SCHEMAS
        +layout LAYOUT_HINTS
        +fields List~Field~
        +actions List~Action~
        +capabilities List~string~
    }
    class PanelState {
        +kind 31401
        +state object
    }
    class ActionRequest {
        +kind 31402
        +fields object
        +reasoning string
        +priority ACTION_PRIORITIES
        +category string
        +subjectKind string
        +subjectId string
    }
    class PanelUpdate {
        +kind 31404
        +diff object
    }
    class PanelRetired {
        +kind 31405
    }
    AgentControlSurface --> PanelDefinition : builds kind 31400, agent-control-surface.js line 108
    AgentControlSurface --> PanelState : builds kind 31401, agent-control-surface.js line 152
    AgentControlSurface --> ActionRequest : builds kind 31402, agent-control-surface.js line 176
    AgentControlSurface --> PanelUpdate : builds kind 31404, agent-control-surface.js line 206
    AgentControlSurface --> PanelRetired : builds kind 31405, agent-control-surface.js line 220
    ActionRequest <.. VoiceIntentRoute : mints an unsigned ActionRequest, routes/voice-intent.js line 225
note for AgentControlSurface "SCOPE - this module mints NIP-33 events consumed by the EXTERNAL<br/>nostr-bbs-forum-client GovernancePage, agent-control-surface.js lines 4-19. It is not an<br/>agentbox-native operator dashboard. See AB-11.x for the authority-gate consumer side, kinds<br/>31402 and 31403"
```

## AB-12.13 turn-sink capture
```mermaid
sequenceDiagram
    autonumber
    participant Cc as Claude Code hook, Stop or UserPromptSubmit
    participant Sk as turn-sink.cjs<br/>agentbox/config/tab0-bridge/turn-sink.cjs:1
    participant B as tab0-bridge<br/>server.mjs:730, POST /hook/turn
    participant Su as summarise()<br/>server.mjs:341
    participant Fe as WebSocket /feed clients

    Cc->>Sk: hook invocation, argv[2] equals Stop or UserPromptSubmit, JSON on stdin (turn-sink.cjs:10,37-40)
    Sk->>Sk: cwd check, payload.cwd must start with /home/devuser/workspace/project (turn-sink.cjs:44-45)
    alt cwd does not match, tab0-bridge's own headless sessions excluded
        Sk-->>Cc: finish, no post (turn-sink.cjs:45,12-15)
    end
    alt event is UserPromptSubmit
        Sk->>Sk: text = payload.prompt (turn-sink.cjs:47)
    else event is Stop
        Sk->>Sk: text = lastAssistantText(transcript_path), scan transcript backwards for the last assistant text block (turn-sink.cjs:17-33,49)
    end
    alt text is null or empty
        Sk-->>Cc: finish, no post (turn-sink.cjs:53)
    end
    Sk->>B: POST /hook/turn, body event, text sliced to 20000 chars, timeout 1500 ms (turn-sink.cjs:55-60)
    B->>B: pushTurn(kind, text), kind from event name (server.mjs:730-734)
    B->>Fe: broadcast, type turn (server.mjs:151,802-805)
    alt kind is assistant and text length over 350 chars
        B->>Su: summarise(turn.text, kind), claude -p, one to three sentences (server.mjs:341-354,736)
        Su-->>B: summary text, or null on error, fail open
        B->>Fe: broadcast, type turn-update, turn with summary (server.mjs:737)
    end
    B-->>Sk: 200, ok true, id turn.id (server.mjs:740)
    Sk-->>Cc: Stop prints ok true json, UserPromptSubmit prints nothing, stdout would inject into context (turn-sink.cjs:2-3,12-15)
    Note over Sk: fail-open by design, any error still exits 0, 3000 ms safety timeout, unref'd (turn-sink.cjs:65)
    Note over B,Fe: boundary — the mobile bridge digests this same turn feed into a kind-30840 summary event, see AB-13.5, not drawn here
```

## AB-12.14 Read-only and outbound route family
```mermaid
sequenceDiagram
    autonumber
    participant Cl as Client
    participant B as tab0-bridge<br/>server.mjs:712
    participant T as tmux CLI
    participant Nf as ~/.claude/nostr-inbox files
    participant Sc as nostr-send.cjs

    Note over Cl,B: family of read-only and outbound surfaces sharing the global auth gate, see AB-12.3 — one sequence covers all members
    Cl->>B: GET /turns?n=50 (server.mjs:742-745)
    B-->>Cl: turns array, sliced to n, capped at MAX_TURNS 300 (server.mjs:743-744,57)
    Cl->>B: GET /tabs (server.mjs:765-767)
    B->>T: tmux list-windows -t agentbox -F index name active (server.mjs:163-169)
    T-->>B: window list
    B-->>Cl: tabs array
    Cl->>B: GET /tabs/:n?lines=60 (server.mjs:779-783)
    B->>T: tmux capture-pane -p -t agentbox:n -S -lines, capped at 200 (server.mjs:171-174,781)
    T-->>B: pane text, trailing whitespace stripped
    B-->>Cl: index, output
    Cl->>B: GET /nostr/status (server.mjs:750-751)
    B->>Nf: read gateway.lock pid, check pidAlive, check mirror-key.txt exists (server.mjs:567-578)
    B-->>Cl: gateway armed, stale-lock or off, mirrorKey bool, sendReady bool
    Cl->>B: GET /nostr/events?n=20 (server.mjs:753-755)
    B->>Nf: read commands.jsonl, tail n lines, capped at 100 (server.mjs:581-587,754)
    B-->>Cl: events array
    Cl->>B: POST /nostr/send, body text (server.mjs:757-763)
    alt text empty after trim and slice 3500
        B-->>Cl: 400, error empty text (server.mjs:760)
    end
    B->>Sc: spawn node nostr-send.cjs text, 12000 ms kill timer (server.mjs:592-600)
    Sc-->>B: exit code 0, success, or non-zero
    B->>B: pushTurn(nostr-out, clean) when ok (server.mjs:762)
    B-->>Cl: ok boolean
    Note over Sc: nostr-send.cjs is fail-open, exit 0 even on delivery failure — ok true means handed to the relay path, not delivered (server.mjs:590-591)
```
