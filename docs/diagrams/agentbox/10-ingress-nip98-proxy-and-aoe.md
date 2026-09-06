---
id: AB-10
title: Ingress — nip98-proxy and the AoE door
area: agentbox
governing:
  - agentbox/docs/INGRESS-identity.md
  - agentbox/docs/SECURITY-profiles.md
adrs: [ADR-2002, ADR-2009, ADR-2010, ADR-2011, ADR-2013, ADR-2047]
sources:
  - agentbox/config/nip98-proxy/proxy.mjs
  - agentbox/config/nip98-proxy/README.md
  - agentbox/config/nip98-proxy/selftest.mjs
  - agentbox/mcp/servers/nostr-bridge.js
  - agentbox/config/nostr-gateway/gateway.cjs
  - agentbox/scripts/aoe-seed-sessions.mjs
  - agentbox/management-api/server.js
  - agentbox/management-api/middleware/auth.js
  - agentbox/scripts/ci/check-ports-loopback.sh
  - agentbox/scripts/ci/check-ports-loopback.mjs
  - agentbox/flake.nix
  - agentbox/docs/adr/ADR-2002-aoe-token-auth-boundary.md
  - agentbox/docs/adr/ADR-2009-nip98-proxy-identity-boundary.md
  - agentbox/docs/adr/ADR-2010-bearer-gated-behind-nip98.md
  - agentbox/docs/adr/ADR-2011-hex-canonical-identity.md
  - agentbox/docs/adr/ADR-2013-loopback-publish-except-9096.md
  - agentbox/docker-compose.voice.yml
verified_commit: 7a20db228
---

## AB-10.1 Door inventory and port topology

```mermaid
flowchart TB
    LAN["LAN client"]
    P9096["nip98-proxy<br/>agentbox/config/nip98-proxy/proxy.mjs:1020<br/>listen 0.0.0.0:9096"]
    AOE9095["aoe serve --auth token --behind-proxy<br/>agentbox/flake.nix:2264<br/>127.0.0.1:9095"]
    MGMT9090["management-api<br/>agentbox/management-api/server.js:1423,:47<br/>HOST 0.0.0.0 PORT 9090 in-container"]
    RELAY7777["nostr-rs-relay<br/>agentbox/flake.nix:1427,:1414<br/>127.0.0.1:7777 unless sovereign_mesh.relay.expose"]
    VOICE8444["voice cockpit Caddy origin<br/>docker-compose.voice.yml:40<br/>0.0.0.0:8444"]

    LAN -->|"9096:9096 SANCTIONED agentbox/scripts/ci/check-ports-loopback.mjs:94,:76-86 ADR-2013"| P9096
    P9096 -->|"default route, Authorization UNCONDITIONALLY replaced with daemon token agentbox/config/nip98-proxy/proxy.mjs:848-863"| AOE9095
    P9096 -->|"prefix /mgmt/ NIP98_PROXY_MGMT_UPSTREAM agentbox/config/nip98-proxy/proxy.mjs:308-319"| MGMT9090
    LAN -.->|"127.0.0.1:9090:9090 host publish agentbox/flake.nix:2542 not LAN-reachable"| MGMT9090
    LAN -->|"8443/8444 SANCTIONED docker-compose.voice.yml:40 agentbox/scripts/ci/check-ports-loopback.mjs:95-96"| VOICE8444
    VOICE8444 -->|"forwards Authorization to slash mgmt slash, slash approvals slash agentbox docs INGRESS-identity.md:118-121"| MGMT9090
    VOICE8444 -->|"forwards to AoE slash aoe slash agentbox docs INGRESS-identity.md:121"| AOE9095

N1["RESOLVED ADR-2047: not a breach - a decided, enumerated exposure. The LAN surface is TEN<br/>sanctioned publishes across five compose files, matched on the normalised host_ip/published/<br/>target/protocol tuple: 9096 sovereign ingress, voice 8443 and 8444, browsercontainer 5903<br/>8931 9222-to-9223, gui-tools 5905 9876 9877, xr-runtime 5904 - each cited at<br/>agentbox/scripts/ci/check-ports-loopback.mjs:72-103. 9096 is the sole IDENTITY ingress to the<br/>AoE plane; the others carry their own auth. The old framing understated the count as well."]
N2["RESOLVED ADR-2047: the stale --auth none bullet is gone. flake.nix:2540 reads<br/>aoe serve --auth token is NEVER published, matching the live supervisor command, and<br/>INGRESS-identity now records the bullet as Resolved rather than open. The doc body's<br/>drifted verifyIdentity citation (proxy.mjs:410-450) is also corrected to proxy.mjs:527."]
N3["INVARIANT ADR-2009: aoe serve binds 127.0.0.1 plus --behind-proxy - nip98-proxy is the sole<br/>IDENTITY ingress to :9095, nothing else may open that port<br/>agentbox/config/nip98-proxy/README.md:40-50"]
N4["RESOLVED ADR-2047: the compose-exposure qualification is superseded in the governing doc.<br/>The line-walker bypass is fixed - check-ports-loopback.sh is a wrapper that execs<br/>check-ports-loopback.mjs, a strict YAML reader for the compose subset, and anything outside that<br/>subset is REJECTED with file and line, never skipped. ADR-2013 stays partial for the DEPLOYMENT<br/>half only: overlay order, interpolation, external files and active-listener evidence.<br/>Receipt: agentbox/docs/estate-closeout/2026-09-05/adr-2013-ports-gate.json."]
N5["INVARIANT ADR-2013: the wrapper FAILS LOUDLY when the .mjs gate is missing - a copy of the<br/>wrapper without its gate exits 3 with an explicit message rather than looking like a pass<br/>agentbox/scripts/ci/check-ports-loopback.sh:27-36"]
```

## AB-10.2 proxy.mjs boot sequence — config, routes, verifier load

```mermaid
sequenceDiagram
    autonumber
    participant BOOT as proxy.mjs boot<br/>agentbox/config/nip98-proxy/proxy.mjs:96
    participant FS as Filesystem<br/>CONFIG_FILE / NOSTR_BRIDGE_PATH
    participant NB as loadNostrBridge<br/>agentbox/config/nip98-proxy/proxy.mjs:377
    participant LOG as log()<br/>agentbox/config/nip98-proxy/proxy.mjs:343

    BOOT->>BOOT: PORT, BIND, AOE_UPSTREAM parsed proxy.mjs:96-98
    BOOT->>BOOT: BREAK_GLASS = NIP98_PROXY_ALLOW_BEARER proxy.mjs:99
    BOOT->>BOOT: SESSION_TTL_S = NIP98_PROXY_SESSION_TTL default 43200 proxy.mjs:109
    break SESSION_TTL_S not a positive safe integer
        BOOT-->>BOOT: throw Error proxy.mjs:111
    end
    BOOT->>BOOT: SESSION_SECRET = NIP98_PROXY_SESSION_SECRET or crypto.randomBytes(32).toString(hex) proxy.mjs:113
    Note over BOOT: DIVERGENCE session secret is per-boot - NIP-07 sessions do not survive a proxy restart, by design agentbox/docs/INGRESS-identity.md known divergences bullet 5
    Note over BOOT: SECURITY-profiles.md custody row Proxy browser-session signing secret - restart invalidation and multi-instance policy unconfirmed
    BOOT->>BOOT: NIP98_PROXY_ALLOWED_PUBKEYS split lowercased filtered proxy.mjs:119-122
    break any entry not 64-hex
        BOOT-->>BOOT: throw Error NIP98_PROXY_ALLOWED_PUBKEYS entries must be 64-character hex proxy.mjs:124
    end
    BOOT->>NB: loadNostrBridge() proxy.mjs:427
    alt NOSTR_BRIDGE_PATH explicit
        NB->>FS: existsSync(NOSTR_BRIDGE_PATH) proxy.mjs:385
        alt missing or exports no verifyNip98
            NB-->>LOG: error no fallback proxy.mjs:386,:395,:397
            NB-->>BOOT: null
        else
            NB-->>BOOT: NostrBridge proxy.mjs:393
        end
    else no explicit path
        loop source-tree then baked-image candidates proxy.mjs:402-411
            NB->>FS: existsSync(candidate)
            NB->>NB: require(candidate) proxy.mjs:415
        end
        NB-->>BOOT: NostrBridge or null proxy.mjs:416-424
    end
    alt NostrBridge is null
        BOOT-->>LOG: warn nostr-bridge unavailable NIP-98 verification DISABLED fail-closed proxy.mjs:429-434
        Note over BOOT: INVARIANT ADR-2009 - missing verifier rejects every NIP-98 token, only break-glass if configured survives
    else loaded
        BOOT-->>LOG: info nostr-bridge loaded proxy.mjs:392,:417
    end
    BOOT->>FS: readFileSync CONFIG_FILE default workspace/.agentbox/nip98-proxy-config.json proxy.mjs:253-259
    alt file absent
        FS-->>BOOT: FILE_CONFIG = routes [] allowedPubkeys [] proxy.mjs:261
    else present
        BOOT->>BOOT: JSON.parse, validate allowedPubkeys 64-hex proxy.mjs:264-271
        break malformed JSON or bad pubkey shape
            BOOT-->>BOOT: log error, process.exit(1) proxy.mjs:276-277
        end
    end
    BOOT->>BOOT: parse NIP98_PROXY_ROUTES JSON array, normalizeRoute each entry proxy.mjs:283-288
    break not an array, or normalizeRoute throws e.g. bearer_env unset proxy.mjs:236-237
        BOOT-->>BOOT: log error, process.exit(1) proxy.mjs:298-299
    end
    BOOT->>BOOT: FILE_CONFIG.routes appended, env route wins on prefix conflict proxy.mjs:290-293
    opt NIP98_PROXY_MGMT_UPSTREAM set and no existing slash mgmt slash route
        BOOT->>BOOT: push convenience route proxy.mjs:308-314
        break invalid URL
            BOOT-->>BOOT: log error, process.exit(1) proxy.mjs:316-317
        end
    end
    BOOT->>BOOT: server.listen(PORT, BIND) proxy.mjs:1020
    BOOT-->>LOG: info nip98-proxy listening - bind, upstream, routes, nip98 enabled/DISABLED, breakGlass, nip07Sessions ttl and pinned-vs-per-boot, allowedPubkeys count proxy.mjs:1021-1029
```

## AB-10.3 verifyIdentity — full auth precedence

```mermaid
sequenceDiagram
    autonumber
    participant REQ as Request<br/>HTTP forward() or WS upgrade
    participant VI as verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:527
    participant CTE as constantTimeEqual<br/>agentbox/config/nip98-proxy/proxy.mjs:441
    participant NB as NostrBridge.verifyNip98<br/>agentbox/mcp/servers/nostr-bridge.js:459
    participant CAN as canonicalPubkey<br/>agentbox/config/nip98-proxy/proxy.mjs:142
    participant PA as pubkeyAllowed<br/>agentbox/config/nip98-proxy/proxy.mjs:128
    participant VST as verifySessionToken<br/>agentbox/config/nip98-proxy/proxy.mjs:462

    REQ->>VI: verifyIdentity(req, bearerToken, rawBody) proxy.mjs:527
    VI->>VI: authHeader = req.headers.authorization proxy.mjs:528
    alt BREAK_GLASS configured and a Bearer token or query bearerToken matches
        VI->>CTE: constantTimeEqual(token, BREAK_GLASS) proxy.mjs:535
Note over VI,CTE: DIVERGENCE break-glass bearer over the LAN - a single shared secret bypasses<br/>NIP-98 entirely while NIP98_PROXY_ALLOW_BEARER is set agentbox/docs/INGRESS-identity.md known<br/>divergences bullet 4
        Note over VI: SECURITY-profiles.md custody row Proxy break-glass bearer - captured at process start, returns a sentinel identity without expiry or request-scope checks
        VI-->>REQ: ok true, pubkey NIP98_PROXY_BEARER_PUBKEY default break-glass, mode break-glass proxy.mjs:536
    else Authorization starts with Nostr
        VI->>NB: verifyNip98(authHeader, method, signedUrlFor(req), rawBody) proxy.mjs:546
        alt NostrBridge unavailable
            VI-->>REQ: ok false, reason nip98_verifier_unavailable proxy.mjs:542
        else verifyNip98 throws
            VI-->>REQ: ok false, reason nip98_verify_error proxy.mjs:548
        else result not valid
            VI-->>REQ: ok false, reason nip98_invalid proxy.mjs:564
        else result.valid true
            VI->>CAN: canonicalPubkey(result.pubkey) proxy.mjs:555
            alt not lowercase 64-hex
                VI-->>REQ: ok false, reason nip98_noncanonical_pubkey proxy.mjs:557
            else canonical
                VI->>PA: pubkeyAllowed(pubkey) proxy.mjs:559
                alt allowlist non-empty and pubkey missing
                    VI-->>REQ: ok false, reason pubkey_not_allowed proxy.mjs:560
                else
                    VI-->>REQ: ok true, pubkey, mode nip98 proxy.mjs:562
                end
            end
        end
    else Cookie carries agentbox_nip07_session
        VI->>VST: verifySessionToken(cookie) proxy.mjs:571
        VST->>CTE: constantTimeEqual(mac, expected) proxy.mjs:470
        VST->>PA: pubkeyAllowed(pubkey) proxy.mjs:471
        alt session valid, not expired, HMAC matches, and allowed
            VI-->>REQ: ok true, pubkey, mode nip07-session proxy.mjs:573
        else
            VI-->>REQ: ok false, reason session_invalid_or_expired proxy.mjs:575
        end
    else no credentials at all
        VI-->>REQ: ok false, reason no_credentials proxy.mjs:578
    end
```

## AB-10.4 verifyNip98 internals — header, tags, payload, signature, replay

```mermaid
sequenceDiagram
    autonumber
    participant CALLER as verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:546
    participant VN as verifyNip98<br/>agentbox/mcp/servers/nostr-bridge.js:459
    participant TAG as getTag<br/>agentbox/mcp/servers/nostr-bridge.js:490
    participant SHA as crypto sha256<br/>agentbox/mcp/servers/nostr-bridge.js:525,:532
    participant SIG as verifyEvent nostr-tools<br/>agentbox/mcp/servers/nostr-bridge.js:542
    participant REPLAY as replay LRU<br/>agentbox/mcp/servers/nostr-bridge.js:97

    CALLER->>VN: verifyNip98(authHeader, method, url, rawBody) nostr-bridge.js:459
    break header missing or not Nostr-prefixed
        VN-->>CALLER: valid false, error missing or malformed Nostr header nostr-bridge.js:461
    end
    VN->>VN: event = JSON.parse(Buffer.from(encoded, base64).toString(utf8)) nostr-bridge.js:471
    break not valid base64 JSON
        VN-->>CALLER: valid false, error token is not valid base64 JSON nostr-bridge.js:473
    end
    break event.kind !== kinds.AUTH (27235) nostr-bridge.js:55,:476
        VN-->>CALLER: valid false, error expected kind 27235 got X nostr-bridge.js:477
    end
    break created_at not a number
        VN-->>CALLER: valid false, error missing created_at nostr-bridge.js:481
    end
    break abs(now - created_at) greater than VERIFY_NIP98_WINDOW_S 60s nostr-bridge.js:80,:485
        VN-->>CALLER: valid false, error event timestamp out of 60-second window nostr-bridge.js:486
    end
    VN->>TAG: getTag(method), getTag(u) nostr-bridge.js:497-498
    break method tag mismatch case-insensitive
        VN-->>CALLER: valid false, error method tag mismatch nostr-bridge.js:501
    end
    break url tag mismatch - urlTag not equal url and not endsWith(url)
        VN-->>CALLER: valid false, error url tag mismatch nostr-bridge.js:505
    end
    opt rawBody supplied - Finding 4 payload binding
        VN->>TAG: getTag(payload) nostr-bridge.js:520
        VN->>SHA: hex(sha256(rawBody)) nostr-bridge.js:525,:532
        break body non-empty and payload tag missing, or computed hash mismatch either branch
            VN-->>CALLER: valid false, error missing payload tag for request body OR payload hash mismatch nostr-bridge.js:523,:527,:534
        end
    end
    VN->>SIG: verifyEvent(event) BIP-340 schnorr constant-time nostr-bridge.js:543
    break signature invalid or verifyEvent throws
        VN-->>CALLER: valid false, error invalid Schnorr signature nostr-bridge.js:545,:548
    end
    VN->>REPLAY: _recordReplayId(event.id) nostr-bridge.js:555,:111
Note over REPLAY: REPLAY_TTL_MS = VERIFY_NIP98_WINDOW_S times 1000 = 60000ms,<br/>REPLAY_MAX_ENTRIES = 20000, keyed on event id, populated ONLY after signature verify so a<br/>forged token can never poison the cache nostr-bridge.js:95-97
    break id already seen inside the freshness window - live duplicate
        VN-->>CALLER: valid false, error replayed NIP-98 event id nostr-bridge.js:556
    end
    VN-->>CALLER: valid true, pubkey event.pubkey nostr-bridge.js:559
```

## AB-10.5 Request auth state machine

```mermaid
stateDiagram-v2
    [*] --> Unauthenticated
    Unauthenticated --> BearerAccepted: break-glass bearer constant-time match proxy.mjs:531-536
    Unauthenticated --> Nip98Verified: verifyNip98 valid, canonical, allowlisted proxy.mjs:541-562
    Unauthenticated --> SessionCookieVerified: verifySessionToken valid and allowlisted proxy.mjs:567-573
    Unauthenticated --> Rejected302: browser GET, Accept text/html, auth failed proxy.mjs:785-792
    Unauthenticated --> Rejected401: auth failed, not an html GET proxy.mjs:794-799
    Unauthenticated --> FailClosed: NostrBridge failed to load at boot proxy.mjs:429-435,542
    BearerAccepted --> Routed: routeFor(req.url) proxy.mjs:803,948
    Nip98Verified --> Routed: routeFor(req.url) proxy.mjs:803,948
    SessionCookieVerified --> Routed: routeFor(req.url) proxy.mjs:803,948
    Routed --> Upstream: default AoE route, valid daemon token injected proxy.mjs:854-863
    Routed --> Upstream: named route, bearer_env or nip98 Authorization passthrough proxy.mjs:837-847
    Routed --> FailClosed: readAoeToken returns null, 503 ServiceUnavailable proxy.mjs:855-861,955-960
    FailClosed --> [*]
    Rejected401 --> [*]
    Rejected302 --> [*]
    Upstream --> [*]
```

## AB-10.6 AoE board door — default upstream, daemon-token replacement

```mermaid
sequenceDiagram
    autonumber
    participant BR as Browser or client
    participant PX as forward()<br/>agentbox/config/nip98-proxy/proxy.mjs:765
    participant VI as verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:527
    participant RT as routeFor<br/>agentbox/config/nip98-proxy/proxy.mjs:327
    participant TOK as readAoeToken<br/>agentbox/config/nip98-proxy/proxy.mjs:179
    participant FS as serve.url state file<br/>~/.config/agent-of-empires/serve.url
    participant AOE as aoe serve --auth token<br/>agentbox/flake.nix:2264 127.0.0.1:9095

    BR->>PX: GET /api/sessions (or dashboard asset) proxy.mjs:765
    PX->>PX: buffer request body, size-capped MAX_BODY_BYTES 25MiB proxy.mjs:772-909
    PX->>VI: verifyIdentity(req, undefined, rawBody) proxy.mjs:780
    alt auth not ok
        PX-->>BR: 302 to /nip07 (html GET) or 401 JSON proxy.mjs:786-799
    else auth ok
        PX->>RT: routeFor(req.url) - no prefix matches proxy.mjs:803,327-341
        RT-->>PX: upstream 127.0.0.1:9095, isAoe true proxy.mjs:340
        PX->>PX: drop hop-by-hop headers, drop inbound x-agentbox-pubkey and x-agentbox-auth-mode proxy.mjs:806-821
        PX->>PX: inject x-forwarded-for, x-forwarded-proto, x-forwarded-host proxy.mjs:823-828
        PX->>PX: headers x-agentbox-pubkey = auth.pubkey, x-agentbox-auth-mode = auth.mode proxy.mjs:829-830
        Note over PX: INVARIANT N-05 boundary - route.isAoe forces Authorization replacement for EVERY auth mode including nip98, because aoe cannot verify a Nostr header itself proxy.mjs:848-854
        PX->>TOK: readAoeToken() proxy.mjs:855,179
        TOK->>FS: statSync, readFileSync, statSync - torn-read retry once proxy.mjs:182-190
        TOK-->>PX: token or null - last-good cache on transient error, NEVER caches null on error proxy.mjs:196-206
        alt token unavailable
            PX-->>BR: 503 ServiceUnavailable AoE token unavailable proxy.mjs:857-860
            Note over PX: SECURITY-profiles.md custody row AoE daemon token - deleting the state file alone is not a revocation mechanism, last-good cache persists
        else token present
            PX->>PX: headers.authorization = Bearer aoeTok proxy.mjs:862
            PX->>AOE: forward request, path = stripUrlCreds(route.path) proxy.mjs:868-876
            AOE-->>PX: proxyRes
            PX-->>BR: relay status and body, pipe proxyRes proxy.mjs:875-876
        end
    end
```

## AB-10.7 /mgmt/* router door — bearer_env gated behind NIP-98

```mermaid
sequenceDiagram
    autonumber
    participant BR as Client
    participant PX as forward()<br/>agentbox/config/nip98-proxy/proxy.mjs:765
    participant VI as verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:527
    participant RT as routeFor<br/>agentbox/config/nip98-proxy/proxy.mjs:327
    participant MAPI as management-api server<br/>agentbox/management-api/server.js:1423
    participant AUTH as authMiddleware<br/>agentbox/management-api/middleware/auth.js:164
    participant NB2 as verifyNip98Header<br/>agentbox/management-api/middleware/auth.js:67

    BR->>PX: request /mgmt/v1/system proxy.mjs:765
    PX->>VI: verifyIdentity(req, undefined, rawBody) proxy.mjs:780
    alt not ok
        PX-->>BR: 401 or 302 proxy.mjs:786-799
    else ok
PX->>RT: routeFor(/mgmt/v1/system) matches prefix /mgmt/, strip true<br/>proxy.mjs:330-338
RT-->>PX: upstream 127.0.0.1:MANAGEMENT_API_PORT, path /v1/system, isAoe<br/>false proxy.mjs:338
        alt auth.mode is nip98
PX->>PX: headers.authorization = req.headers.authorization, unchanged<br/>proxy.mjs:837-844
Note over PX: ADR-2010 - a genuinely signed NIP-98 header always reaches<br/>a named upstream, so the governance surface re-verifies the operator<br/>signature itself
        else route declares bearer_env and auth.mode is not nip98
            PX->>PX: headers.authorization = Bearer route.bearer proxy.mjs:845-847
Note over PX: ADR-2010 - bearer_env is injected ONLY when auth.mode is<br/>not nip98, and normalizeRoute is fatal at boot if the named env var is<br/>unset proxy.mjs:233-237
        end
        PX->>MAPI: forward to 127.0.0.1:9090, path /v1/system proxy.mjs:868-876
MAPI->>AUTH: authMiddleware, authMode = MANAGEMENT_API_AUTH_MODE default<br/>hybrid agentbox/management-api/server.js:216-217
AUTH->>NB2: verifyNip98Header(authHeader, request) re-verifies the<br/>signature independently agentbox/management-api/middleware/auth.js:67-82
        alt hybrid and Bearer equals API_KEY
AUTH-->>MAPI: allow, bearer accepted<br/>agentbox/management-api/middleware/auth.js:105-110,178
        else nip98Result valid
AUTH-->>MAPI: allow, nip98 accepted<br/>agentbox/management-api/middleware/auth.js:169
        else neither
            AUTH-->>MAPI: 401 agentbox/management-api/middleware/auth.js:194
        end
        MAPI-->>PX: response
        PX-->>BR: relay status and body proxy.mjs:875-876
    end
Note over MAPI,AUTH: management-api source has zero references to<br/>X-Agentbox-Pubkey - it never<br/>reads or trusts a proxy-injected identity header, it re-verifies<br/>Authorization itself on every<br/>request
```

## AB-10.8 NIP-07 browser session handshake

```mermaid
sequenceDiagram
    autonumber
    participant BR as Browser
    participant PX as handleNip07<br/>agentbox/config/nip98-proxy/proxy.mjs:714
    participant SIGNER as window.nostr signer
    participant VI as verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:527
    participant MINT as mintSessionToken<br/>agentbox/config/nip98-proxy/proxy.mjs:456
    participant UP as default or routed upstream

    BR->>PX: GET /nip07/ or /nip07/login proxy.mjs:718
    PX-->>BR: 200 HANDSHAKE_PAGE html proxy.mjs:614-708,719-721
    BR->>SIGNER: waitForSigner(3000ms), poll every 200ms proxy.mjs:655-664
    SIGNER-->>BR: window.nostr detected
    BR->>SIGNER: signEvent(kind 27235, tags u and method POST) proxy.mjs:677-683
    SIGNER-->>BR: signed event
    BR->>PX: POST /nip07/session, Authorization Nostr base64(signed) proxy.mjs:724,686
    PX->>VI: verifyIdentity(req, undefined, rawBody) proxy.mjs:725
    alt not ok, or mode is not nip98
        PX-->>BR: 401 session minting requires a NIP-98 signature proxy.mjs:729-736
        Note over PX: an existing cookie cannot self-renew, and break-glass cannot launder its sentinel into a pubkey-bound session proxy.mjs:726-728
    else ok and mode nip98
        PX->>MINT: mintSessionToken(auth.pubkey) - v1.pubkey.expiry.hmac proxy.mjs:456-460,738
        Note over MINT: SESSION_SECRET is per-boot random unless NIP98_PROXY_SESSION_SECRET is pinned proxy.mjs:113
        Note over MINT: SECURITY-profiles.md custody row Proxy browser-session signing secret - restart invalidation and multi-instance policy unconfirmed
        PX-->>BR: 200, Set-Cookie agentbox_nip07_session HttpOnly SameSite=Lax Secure-if-https Max-Age SESSION_TTL_S proxy.mjs:499-501,742-747
    end
    BR->>PX: subsequent request, Cookie agentbox_nip07_session=token proxy.mjs:765
    PX->>VI: verifyIdentity reads cookie via parseCookies proxy.mjs:475-483,569-576
    VI-->>PX: ok true, pubkey, mode nip07-session
    PX->>PX: stripSessionCookie removes agentbox_nip07_session before forwarding proxy.mjs:490-497,815-819
    PX->>UP: forward, x-agentbox-pubkey and x-agentbox-auth-mode nip07-session injected proxy.mjs:829-830
```

## AB-10.9 WebSocket upgrade auth path

```mermaid
sequenceDiagram
    autonumber
    participant BR as WS client
    participant PX as server.on(upgrade)<br/>agentbox/config/nip98-proxy/proxy.mjs:915
    participant VI as verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:527
    participant RT as routeFor<br/>agentbox/config/nip98-proxy/proxy.mjs:327
    participant TOK as readAoeToken<br/>agentbox/config/nip98-proxy/proxy.mjs:179
    participant UP as upstream socket<br/>net.connect

    BR->>PX: Upgrade GET /sessions/id/live-ws with ?access_token= or ?auth= proxy.mjs:915
    PX->>PX: parse query access_token or bearer proxy.mjs:928-929
    opt auth query param present and no Authorization header
        PX->>PX: req.headers.authorization = Nostr plus the auth param proxy.mjs:931-933
Note over PX: DIVERGENCE query-carried credentials, WS-handshake-only - regression fix<br/>restoring the console signer-only auth carrier lost when /feed moved behind this proxy under<br/>ADR-069 proxy.mjs:917-925
    end
    PX->>VI: verifyIdentity(req, queryToken) proxy.mjs:936
    alt not ok
        PX-->>BR: HTTP/1.1 401 Unauthorized, then socket.destroy proxy.mjs:939-945
        Note over PX: DIVERGENCE break-glass bearer accepted via ?access_token= or ?bearer= on WS upgrades over the LAN agentbox/docs/INGRESS-identity.md known divergences bullet 4
    else ok
        PX->>RT: routeFor(req.url) proxy.mjs:948
        alt route.isAoe
            PX->>TOK: readAoeToken() proxy.mjs:954,179
            alt token unavailable
                PX-->>BR: HTTP/1.1 503 Service Unavailable, then socket.destroy proxy.mjs:956-960
            else token present
                PX->>UP: net.connect, rebuild request line, strip Authorization, inject x-agentbox-pubkey and x-agentbox-auth-mode proxy.mjs:962-987
                PX->>UP: Authorization Bearer aoeWsToken - unconditional for AoE proxy.mjs:998
            end
        else named route
            alt auth.mode is nip98
                PX->>UP: Authorization req.headers.authorization passthrough proxy.mjs:990-994
            else route.bearer set and auth.mode is not nip98
                PX->>UP: Authorization Bearer route.bearer proxy.mjs:995-996
            end
        end
        PX->>PX: strip session cookie before forwarding on WS too proxy.mjs:976-980
        PX->>UP: pipe socket bidirectionally proxy.mjs:1002-1003
    end
```

## AB-10.10 Direct :9090 path vs proxied :9090 path

```mermaid
sequenceDiagram
    autonumber
    participant DC as Direct caller (container-internal)
    participant MAPI as management-api server<br/>agentbox/management-api/server.js:1423, HOST 0.0.0.0 PORT 9090
    participant AUTH as authMiddleware<br/>agentbox/management-api/middleware/auth.js:164
    participant BR2 as Browser via :9096
    participant PX2 as proxy /mgmt/ route<br/>agentbox/config/nip98-proxy/proxy.mjs:803

    Note over DC,MAPI: :9090 is published to the host only as 127.0.0.1:9090:9090 agentbox/flake.nix:2542 - DC must already be container-internal or on the loopback publish
    DC->>MAPI: request with its OWN Authorization Bearer API_KEY or Nostr header, no X-Agentbox-Pubkey
    MAPI->>AUTH: authMiddleware verifies bearer or nip98 directly agentbox/management-api/server.js:216-224
    AUTH-->>MAPI: allow or 401

    BR2->>PX2: request /mgmt/... over 9096, with NIP-98 or session cookie
    PX2->>PX2: verifyIdentity, then routeFor, then inject x-agentbox-pubkey and x-agentbox-auth-mode proxy.mjs:780-830
    PX2->>MAPI: forward to 127.0.0.1:9090, Authorization is passthrough (nip98) or bearer_env (else) per ADR-2010
    MAPI->>AUTH: authMiddleware STILL independently verifies Authorization, same code path as the direct request agentbox/management-api/middleware/auth.js:164-194
Note over MAPI,AUTH: the only difference between the two paths is WHO ADDS the pubkey header -<br/>only the proxied path carries x-agentbox-pubkey and x-agentbox-auth-mode, management-api itself<br/>never consumes or requires them
```

## AB-10.11 Direct :9095 token-bearing consumer — nostr-gateway aoeRequest

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator, /spawn command
    participant GW as aoeRequest<br/>agentbox/config/nostr-gateway/gateway.cjs:431
    participant TOKFN as readAoeToken<br/>agentbox/config/nostr-gateway/gateway.cjs:127
    participant FS2 as serve.url<br/>AGENTBOX_AOE_TOKEN_FILE override, default ~/.config/agent-of-empires/serve.url
    participant AOE2 as aoe serve :9095<br/>agentbox/flake.nix:2264

    OP->>GW: /spawn dir agent text - aoeCreateSession(repoPath, tool, title) agentbox/config/nostr-gateway/gateway.cjs:320-323,452-453
    GW->>GW: POST /api/sessions?wait=ready via aoeRequest gateway.cjs:452-453
    GW->>TOKFN: readAoeToken() gateway.cjs:435
    TOKFN->>FS2: statSync then readFileSync then statSync, torn-read retry once gateway.cjs:130-138
    alt file missing or unreadable, and no last-good cache
        TOKFN-->>GW: null
        GW-->>OP: throw AoE token unavailable (N-05 fail-closed) - not sending unauthenticated request gateway.cjs:436
        Note over GW,TOKFN: ADR-2002 defence-in-depth - this is a SECOND independent enforcement point beyond the nip98-proxy, duplicated verbatim per the KEEP IN SYNC comment gateway.cjs:117-123
    else token cached or freshly read
        TOKFN-->>GW: token, last-good cache, NEVER caches null on a transient error
        GW->>AOE2: POST /api/sessions, Authorization Bearer tok gateway.cjs:437-444
        AOE2-->>GW: 200/201, session id and status
    end
    Note over GW: on any AoE failure the caller falls back to a plain tmux new-window, per the ADR-042/044 D5 comment gateway.cjs:105-109
```

## AB-10.12 selftest.mjs assertion families

```mermaid
flowchart TB
    HARNESS["selftest.mjs harness<br/>agentbox/config/nip98-proxy/selftest.mjs:271 main()"]
    PUT["proxy.mjs under test<br/>plus separate child processes for boot-config-specific cases"]
    FAKE["fake AoE, mgmt and governance upstreams"]
    A["A - no credentials, 401<br/>selftest.mjs:308"]
    B["B - break-glass bearer, 200, identity injected, Authorization stripped<br/>selftest.mjs:314"]
    C["C - valid NIP-98, 200, skips gracefully if nostr-tools unresolvable<br/>selftest.mjs:336"]
    D["D - WebSocket upgrade, forwarded with injected identity<br/>selftest.mjs:365"]
    E["E - routed prefix ADR-045, mgmt on second upstream, unrouted falls through<br/>selftest.mjs:417"]
    F["F - NIP-07 sessions, handshake page, redirect, cookie auth, strip, forged/expired reject<br/>selftest.mjs:451"]
    G["G - spoofed identity headers stripped and replaced, ADR-2009<br/>selftest.mjs:550"]
    H["H - verifier faults, absent/throwing/non-canonical, zero upstream contact, ADR-2009/2011<br/>selftest.mjs:598"]
    I["I - allowlist removal denies the next request, including a live session, ADR-2009<br/>selftest.mjs:689"]
    J["J - cookie expiry and restart under a rotated HMAC key, rejected<br/>selftest.mjs:730"]
    K["K - tokenless denial, AoE daemon token NOT injected, ADR-2002<br/>selftest.mjs:759"]
    L["L - bearer gated behind NIP-98 on a named route, ADR-2010<br/>selftest.mjs:781"]
    M["M - hex-canonical identity helper, 0600 key file, restart stability, ADR-2011 (see AB-11)<br/>selftest.mjs:870"]
    N1["45 assertions pass with no skips when NODE_PATH resolves nostr-tools - README.md:189-206"]

    HARNESS --> PUT
    HARNESS --> FAKE
    PUT --> FAKE
    HARNESS -.-> A
    HARNESS -.-> B
    HARNESS -.-> C
    HARNESS -.-> D
    HARNESS -.-> E
    HARNESS -.-> F
    HARNESS -.-> G
    HARNESS -.-> H
    HARNESS -.-> I
    HARNESS -.-> J
    HARNESS -.-> K
    HARNESS -.-> L
    HARNESS -.-> M
```

