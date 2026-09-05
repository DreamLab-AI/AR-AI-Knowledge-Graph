---
id: ES-04
title: did:nostr identity mesh — signing, verification, custody
area: estate
governing:
  - docs/IDENTITY-authority-chain.md
  - agentbox/docs/INGRESS-identity.md
  - agentbox/docs/BASELINE-container.md
  - docs/SECURITY-profiles.md
adrs: [ADR-2002, ADR-2009, ADR-2010, ADR-2011, ADR-2013, ADR-2026]
sources:
  - agentbox/config/nip98-proxy/proxy.mjs
  - agentbox/config/nip98-proxy/selftest.mjs
  - agentbox/management-api/lib/pod-signer.js
  - agentbox/management-api/lib/agent-identity.js
  - agentbox/management-api/lib/agent-event-auth.js
  - agentbox/management-api/lib/uris.js
  - agentbox/mcp/servers/nostr-bridge.js
  - agentbox/config/hooks/nostr-live-mirror.cjs
  - agentbox/flake.nix
  - agentbox/docker-compose.voice.yml
  - agentbox/agentbox.toml
  - src/utils/nip98.rs
  - src/services/nostr_service.rs
  - src/services/nostr_identity_verifier.rs
  - src/services/nostr_bridge.rs
  - src/services/nostr_bead_publisher.rs
  - src/services/canary_nostr_tap.rs
  - src/services/management_api_client.rs
  - src/settings/auth_extractor.rs
  - src/handlers/socket_flow_handler/http_handler.rs
  - client/src/services/nostrAuthService.ts
  - client/src/types/nip07.d.ts
verified_commit: b00c28a0d
---
## ES-04.1 verification mesh — who signs, who verifies whom
```mermaid
flowchart LR
    subgraph browser["Browser (untrusted)<br/>trust boundary"]
        BX["window.nostr<br/>NIP-07 extension"]
    end
    subgraph vc["VisionClaw process<br/>src/"]
        AE["AuthenticatedUser extractor<br/>src/settings/auth_extractor.rs:89"]
        NS["NostrService::verify_nip98_auth<br/>src/services/nostr_service.rs:579"]
        N98["validate_nip98_token<br/>src/utils/nip98.rs:330"]
        NIV["NostrIdentityVerifier<br/>src/services/nostr_identity_verifier.rs:34"]
        NB["NostrBridge (re-sign)<br/>src/services/nostr_bridge.rs:28"]
        MAC["ManagementApiClient<br/>src/services/management_api_client.rs:179"]
    end
    subgraph agentbox["agentbox host processes"]
        PROXY["nip98-proxy :9096<br/>agentbox/config/nip98-proxy/proxy.mjs:96"]
        NBB["NostrBridge.verifyNip98<br/>agentbox/mcp/servers/nostr-bridge.js:459"]
        PS["pod-signer buildPodNip98<br/>agentbox/management-api/lib/pod-signer.js:32"]
        AGID["agent-identity loadOrMint<br/>agentbox/management-api/lib/agent-identity.js:107"]
        AEA["agent-event-auth verifyAgentEventRequest<br/>agentbox/management-api/lib/agent-event-auth.js:46"]
    end
    subgraph aoe["AoE daemon :9095<br/>loopback only"]
        AOE["aoe serve --auth token --behind-proxy<br/>agentbox/flake.nix:2247"]
    end
    subgraph solid["EXTERNAL: solid-pod-rs<br/>default-deny pod"]
        POD["Solid pod HTTP endpoint"]
    end
    subgraph nonsign["Non-signing (VisionClaw canon)"]
        VF["VisionFlow canon<br/>read-only KG consumer"]
    end

    BX -- "NIP-07 signEvent()<br/>nostrAuthService.ts:275" --> AE
    AE -- "NIP-98 kind-27235" --> NS --> N98
    NS -- "Bearer + X-Nostr-Pubkey<br/>legacy fallback" --> AE
    NIV -. "XR presence Schnorr(nonce||ts)<br/>NOT NIP-98" .-> NB
    MAC -- "Authorization: Bearer api_key<br/>management_api_client.rs:286" --> PROXY
    PROXY -- "NIP-98 kind-27235<br/>proxy-verified" --> NBB
    PROXY -- "X-Agentbox-Pubkey<br/>injected, never trusted inbound" --> AOE
    PROXY -- "Authorization: Bearer <aoe token><br/>read from serve.url" --> AOE
    PS -- "NIP-98 kind-27235<br/>buildNip98Header" --> POD
    AEA -- "NIP-98 kind-27235<br/>AGENTBOX_AGENT_EVENT_AUTH=nip98" --> NBB
    AGID -. "did:nostr:&lt;hex&gt; mint<br/>consumed by" .-> PS
    NB -. "DIVERGENCE NIP-26 delegation NOT wired —<br/>nostr_bridge.rs re-signs under the BRIDGE key<br/>(service-signed, not delegation), so no request<br/>can be attributed to a user THROUGH an agent" .-> VF

    Note1["EXTERNAL: nostr-rust-forum, dreamlab-ai-website,<br/>code-as-harness sign their own events;<br/>not on disk in this repo — asserted only where cited"]
```

## ES-04.2 NIP-98 event structure (kind 27235)
```mermaid
classDiagram
    class Nip98Event {
      +String id
      +String pubkey
      +i64 created_at
      +u16 kind = 27235
      +List~List~String~~ tags
      +String content
      +String sig
    }
    class Nip98Tags {
      +u["url"] : String, no querystring
      +method["METHOD"] : String, case-insensitive
      +payload["hex(sha256(body))"] : String, optional
    }
    class Nip98ValidationResult {
      +String pubkey
      +String url
      +String method
      +i64 created_at
      +Option~String~ payload_hash
    }
    class Nip98ValidationError {
      <<enumeration>>
      InvalidBase64
      InvalidUtf8
      InvalidJson
      InvalidKind
      TokenExpired
      TokenFromFuture
      MissingTag
      UrlMismatch
      MethodMismatch
      PayloadHashMismatch
      InvalidSignature
      VerificationFailed
      TokenReplayed
      ReplayCacheFull
    }
    Nip98Event --> Nip98Tags
    Nip98Event --> Nip98ValidationResult : validate_nip98_token()
    Nip98ValidationResult --> Nip98ValidationError : Err variant

    note for Nip98Event "HTTP_AUTH_KIND = 27235 (src/utils/nip98.rs:20)<br/>TOKEN_MAX_AGE_SECONDS = 60 (src/utils/nip98.rs:169)<br/>REPLAY_CACHE_TTL = 2x window = 120s (src/utils/nip98.rs:178)<br/>REPLAY_CACHE_PRUNE_THRESHOLD = 4096 (src/utils/nip98.rs:183)<br/>REPLAY_CACHE_MAX_ENTRIES = 100000 (src/utils/nip98.rs:198)<br/>agentbox mirrors: VERIFY_NIP98_WINDOW_S=60, REPLAY_MAX_ENTRIES=20000 (nostr-bridge.js:80,96)"
    note for Nip98ValidationError "INVARIANT on a FULL cache the code REFUSES new auth<br/>(ReplayCacheFull) and must NOT evict the oldest live entry —<br/>evicting would let a flooder purge a genuine still-valid id<br/>and re-enable the very replay this layer prevents<br/>(src/utils/nip98.rs:189-197). Bounded memory beats availability."
```

## ES-04.3 validate_nip98_token — the fixed check order, replay claimed LAST
```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant V as validate_nip98_token<br/>src/utils/nip98.rs:330
    participant R as claim_event_id<br/>src/utils/nip98.rs:234
    participant CACHE as REPLAY_CACHE<br/>Mutex-guarded, src/utils/nip98.rs:200

    C->>V: base64 token, expected url, expected method, body
    alt decode fails
        V-->>C: Err InvalidBase64 / InvalidUtf8 / InvalidJson
    else decoded
        V->>V: kind check
        alt kind is not 27235
            V-->>C: Err InvalidKind — nip98.rs:349
        else kind ok
            V->>V: FRESHNESS — compare created_at to now
            alt age exceeds TOKEN_MAX_AGE_SECONDS 60
                V-->>C: Err TokenExpired — nip98.rs:364
            else created_at is in the future
                V-->>C: Err TokenFromFuture — nip98.rs:369
            else fresh
                V->>V: TAG MATCH — host-checked u tag
                alt url differs
                    V-->>C: Err UrlMismatch{expected, actual} — nip98.rs:397
                else method differs
                    V-->>C: Err MethodMismatch{expected, actual} — nip98.rs:405
                else payload hash present and differs
                    V-->>C: Err PayloadHashMismatch — nip98.rs:416
                else tags match
                    V->>V: SIGNATURE verification
                    alt signature invalid
                        V-->>C: Err InvalidSignature / VerificationFailed
                    else signature valid
                        V->>R: claim_event_id(event.id, Instant::now())
                        Note over R,CACHE: Check-and-insert is ATOMIC under the Mutex —<br/>no TOCTOU (nip98.rs:200-203). Instant is MONOTONIC, so a<br/>backward wall-clock step cannot un-spend an id (nip98.rs:432).
                        alt a still-live entry exists
                            R-->>V: Err TokenReplayed — nip98.rs:256
                            V-->>C: rejected
                        else live set is at REPLAY_CACHE_MAX_ENTRIES
                            R-->>V: Err ReplayCacheFull
                            V-->>C: rejected, fail-closed under flood
                        else recorded
                            R-->>V: Ok
                            V-->>C: Ok Nip98ValidationResult
                        end
                    end
                end
            end
        end
    end
    Note over C,CACHE: INVARIANT — the replay CLAIM is the LAST step<br/>(nip98.rs:435). It must never precede signature verification,<br/>or a forged token would burn a legitimate event id.
    Note over V,CACHE: INVARIANT — REPLAY_CACHE_TTL must stay at 2x the freshness<br/>window (nip98.rs:178). A token created at now+60 stays valid<br/>until now+120, so a shorter TTL would let it replay after<br/>its entry prunes.
```

## ES-04.4 agentbox pod-signer — an agent signing a write to a Solid pod
```mermaid
sequenceDiagram
    autonumber
    participant CALLER as management-api caller
    participant B as buildPodNip98<br/>agentbox/management-api/lib/pod-signer.js:32
    participant S as signer (lazy-loaded, cached)
    participant H as buildNip98Header<br/>deps injection, pod-signer.js:26
    participant POD as Solid pod

    CALLER->>B: buildPodNip98(manifest, deps)
    alt pod signing enabled in the manifest
        B-->>CALLER: async (method, url, body) => header<br/>pod-signer.js:81
        CALLER->>H: nip98(method, url, body)
        H->>S: resolve signer (loaded lazily on first use, then cached)
        alt signer available
            S-->>H: signer
            H->>H: buildNip98Header(s, method, url, {body})<br/>pod-signer.js:84
            H-->>CALLER: Authorization header carrying the kind-27235 event
            CALLER->>POD: LDP write with the signed header
            POD-->>CALLER: 2xx
        else signer unavailable or the key will not decrypt
            Note over S,POD: DIVERGENCE — pod signing can fall back UNSIGNED<br/>(legacy agentbox ADR-026). Combined with the did:nostr:local<br/>placeholder fallback at agent-identity.js:175, a degraded boot<br/>can silently produce a NON-SOVEREIGN identity.
            S-->>H: none
            H-->>CALLER: no header
            CALLER->>POD: UNSIGNED write
            POD-->>CALLER: accepted or refused by WAC, see ES-08
        end
    else disabled
        B-->>CALLER: null — surface stays standalone
    end
    Note over CALLER,POD: The signer lifecycle MIRRORS lib/elevation-publisher —<br/>owned here, loaded lazily on first use, cached. No<br/>in-request connect or disconnect. see ES-05.7
```

## ES-04.5 Key custody — where every signing key actually lives
```mermaid
flowchart TB
    subgraph live["Live signing keys"]
        K1["Bridge identity / unwrap key<br/>AGENTBOX_BRIDGE_SK_FILE, default /run/secrets/nostr.key<br/>legacy environment fallback REMAINS"]
        K2["Shared server publisher identity<br/>relay key list is build-projected"]
        K3["AoE daemon token<br/>~/.config/agent-of-empires/serve.url, dir chmod 0700<br/>minted at launch, NOT env-settable"]
        K4["nip98-proxy session secret<br/>NIP98_PROXY_SESSION_SECRET or per-boot crypto.randomBytes"]
        K5["Session-mirror child key<br/>~/.claude/nostr-mirror/mirror-key.txt<br/>DERIVED, re-derivable, deleted after import"]
    end
    subgraph fixtures["NOT live keys"]
        F1["test-data-idp-accounts/ at the repo root<br/>FIXTURES ONLY — test IdP accounts, never live custody"]
    end
    subgraph boundary["Egress boundary"]
        REL["cloud Nostr relay worker<br/>dreamlab-nostr-relay.solitary-paper-764d.workers.dev"]
    end

    K5 -->|"NIP-59 gift wrap, kind 1059"| REL

    D1["DIVERGENCE ADR-040 D3 — agentbox.toml:148 marks the governance<br/>publisher key-split PENDING, so governance-published events<br/>and server identity STILL SHARE a key."]
    D2["DIVERGENCE — NIP98_PROXY_SESSION_SECRET defaults to<br/>crypto.randomBytes (proxy.mjs:107), so NIP-07 sessions do<br/>NOT survive a proxy restart. Intentional, but every restart<br/>forces re-authentication."]
    D3["DIVERGENCE — a same-uid (devuser) process can still READ the<br/>AoE token file. The token raises the bar but does NOT isolate<br/>same-user peers. Per-process isolation is future work."]
    D4["DIVERGENCE — deleting the AoE state file alone does NOT<br/>rotate the token, because the proxy holds a last-good cache.<br/>Daemon and proxy must rotate COHERENTLY. see ES-10.10"]
    D5["DIVERGENCE — every custodian, deployed location, rotation<br/>cadence and incident-response window in the agentbox custody<br/>register is UNCONFIRMED (proposed governing surface)."]

    K2 --> D1
    K4 --> D2
    K3 --> D3
    K3 --> D4
    live --> D5
    F1 -.-> live
```

## ES-04.6 Session mirror — derive, gift-wrap, publish, fail open
```mermaid
sequenceDiagram
    autonumber
    participant T as turn end
    participant HK as nostr-live-mirror.cjs<br/>agentbox/config/hooks/nostr-live-mirror.cjs
    participant MK as mirror child key<br/>~/.claude/nostr-mirror/mirror-key.txt
    participant W as NIP-59 gift wrap
    participant REL as cloud relay worker
    participant AM as Amethyst on mobile

    T->>HK: per-turn hook fires
    alt AGENTBOX_LIVE_MIRROR=0
        HK-->>T: return 0 — off switch, nostr-live-mirror.cjs:355
    else no recipient pubkey configured
        HK-->>T: silent no-op
    else armed
        HK->>MK: load derived mirror child key
        HK->>HK: build kind-14 DM rumor
        Note over HK: A urn:agentbox:activity reference is placed INSIDE the<br/>already gift-wrap-sealed rumor (nostr-live-mirror.cjs:121),<br/>so the URN never appears in cleartext on the wire.
        HK->>W: seal sender identity, wrap as KIND_GIFT_WRAP 1059<br/>nostr-live-mirror.cjs:54,217
        W-->>HK: signed gift wrap
        HK->>REL: publish ONE pre-signed gift wrap, await the relay OK<br/>nostr-live-mirror.cjs:294
        alt relay accepts
            REL-->>HK: ok
            REL->>AM: readable with the derived mirror child key
        else relay rejects or is unreachable
            REL--xHK: error
            HK-->>T: FAIL-OPEN — the turn is never blocked
        end
    end
    Note over HK,REL: INVARIANT ADR-2026 — the ONLY network egress is the<br/>ENCRYPTED gift wrap. The relay accepts a kind-1059 iff its<br/>FIRST ["p"] recipient is whitelisted (nostr-live-mirror.cjs:19).
    Note over T,AM: Digest sibling — ONE curated kind-30840 summary at SessionEnd<br/>(nostr-live-mirror.cjs:8), via [sovereign_mesh.mobile_bridge].
    Note over HK: DIVERGENCE agentbox/docs/SECURITY-profiles.md — the live hook<br/>composes UNREDACTED selected text before wrapping, while the<br/>digest path sends flattened input to its summarisation<br/>provider. Their gates and encryption DIFFER. A shared<br/>off/redaction/recipient/retention contract remains OPEN.
```
