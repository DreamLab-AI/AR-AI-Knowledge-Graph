---
id: VC-33
title: Browser-client identity — NIP-07, NIP-98, passkeys, RBAC gating
area: visionclaw
governing:
  - docs/IDENTITY-authority-chain.md
  - docs/SECURITY-profiles.md
adrs: [ADR-2002, ADR-2009, ADR-2011, ADR-2012, ADR-2074, ADR-2075]
sources:
  - client/src/services/nostrAuthService.ts
  - client/src/hooks/useNostrAuth.ts
  - client/src/services/api/authInterceptor.ts
  - client/src/api/settings/endpoints.ts
  - client/src/services/passkeyService.ts
  - client/src/services/SolidPodService.ts
  - client/src/services/solidPod/ldpClient.ts
  - client/src/app/App.tsx
  - client/src/store/settings/coreSlice.ts
  - client/src/features/control-center/primitives/SettingRow.tsx
  - client/src/features/control-center/primitives/NostrAuthControl.tsx
  - client/src/store/websocket/connectionManager.ts
  - client/src/__tests__/agent-pod/pod-provisioning.test.ts
  - src/middleware/rbac_gate.rs
  - src/services/role_store.rs
  - src/models/rbac.rs
  - docker-compose.unified.yml
  - docs/BASELINE-architecture.md
  - docs/IDENTIFIER-taxonomy.md
  - client/src/api/analyticsApi.ts
  - client/src/app/main.tsx
  - client/src/features/graph/managers/dataManager/restClient.ts
  - client/src/features/ontology/services/jss/contextLoader.ts
  - client/src/services/VoiceWebSocketService.ts
  - client/src/services/api/UnifiedApiClient.ts
  - src/services/nostr_service.rs
  - src/utils/auth.rs
  - src/utils/nip98.rs
verified_commit: 7a20db228
---
## VC-33.1 NIP-07 extension login (client-asserted, no server verify round-trip)
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant H as useNostrAuth<br/>client/src/hooks/useNostrAuth.ts:7
    participant NA as NostrAuthService<br/>client/src/services/nostrAuthService.ts:197
    participant EXT as window.nostr<br/>NIP-07 extension
    participant LS as localStorage

    H->>NA: initialize() useNostrAuth.ts:22
    NA->>NA: isDevMode() check nostrAuthService.ts:235-237
    alt dev mode VITE_DEV_MODE_AUTH=true
        NA->>NA: ephemeral pubkey sessionStorage ephemeral_session_pubkey nostrAuthService.ts:323-333
        Note over NA: auto-authenticated as isPowerUser true. no extension needed
    else normal mode
        NA->>LS: getItem nostr_user nostrAuthService.ts:341
        LS-->>NA: cached SimpleNostrUser or null
        NA->>NA: restorePasskeySession() nostrAuthService.ts:355,581
        opt user cached but no signer available
            NA->>NA: waitForNip07Provider 5000ms nostrAuthService.ts:227,365
            NA->>EXT: poll and Object.defineProperty hook waitForNip07 nostrAuthService.ts:70-144
            alt extension appears within 5s
                EXT-->>NA: window.nostr set
                Note over NA: session considered valid again
            else timeout
                NA->>NA: clear stale session nostrAuthService.ts:373-378
                NA->>LS: removeItem nostr_user
            end
        end
    end
    NA-->>H: notifyListeners AuthState nostrAuthService.ts:477-485

    U->>H: login() useNostrAuth.ts:66
    H->>NA: login() nostrAuthService.ts:388
    NA->>NA: hasNip07Provider() nostrAuthService.ts:213-215
    alt no NIP-07 provider installed
        NA-->>H: throw Nostr NIP-07 provider not found nostrAuthService.ts:391-395
    else provider present
        NA->>EXT: getPublicKey() nostrAuthService.ts:398
        alt user rejects in extension popup
            EXT-->>NA: reject User rejected
            NA-->>H: errorMessage Login request rejected in Nostr extension nostrAuthService.ts:416-417
        else success
            EXT-->>NA: pubkey hex
            NA->>NA: hexToNpub(pubkey) nip19.npubEncode nostrAuthService.ts:406,503-511
            NA->>NA: currentUser = pubkey npub isPowerUser false nostrAuthService.ts:404-408
            Note over NA: isPowerUser is a client-side placeholder. server determines it per-request from<br/>POWER_USER_PUBKEYS. no verify endpoint exists in this file
            NA->>LS: setItem nostr_user JSON nostrAuthService.ts:438-444
            NA-->>H: AuthState authenticated true user
        end
    end
    H-->>U: authState re-render
    Note over NA: getSessionToken() is deprecated. always returns null nostrAuthService.ts:433-436.<br/>NIP-98 is per-request, no bearer token is minted here
```

## VC-33.2 NIP-98 per-request signing — two independent signing sites (DIVERGENCE)
```mermaid
sequenceDiagram
    autonumber
    participant C as Component
    participant UAC as UnifiedApiClient<br/>client/src/services/api/UnifiedApiClient.ts:116
    participant AI as authRequestInterceptor<br/>client/src/services/api/authInterceptor.ts:41
    participant AX as axios.interceptors.request<br/>client/src/api/settings/endpoints.ts:37
    participant NA as NostrAuthService<br/>nostrAuthService.ts:197
    participant EXT as window.nostr or local passkey key

    rect rgb(255,235,235)
    Note over UAC,AI: SITE A. UnifiedApiClient path, wired at client/src/app/main.tsx:28<br/>initializeAuthInterceptor. Keeps X-Request-ID and the release-mode 401 warning<br/>locally, delegates the auth branch to computeAuthHeaders authInterceptor.ts:52
    C->>UAC: request GET or PUT api-path
    UAC->>AI: onRequest(config,url) authInterceptor.ts:41
    AI->>NA: isAuthenticated() authInterceptor.ts:53
    alt isDevMode
        AI->>AI: headers Authorization Bearer dev-session-token authInterceptor.ts:60
        AI->>AI: headers X-Nostr-Pubkey user.pubkey authInterceptor.ts:61-63
    else authenticated
        AI->>AI: fullUrl = new URL(url, origin).href authInterceptor.ts:70
        AI->>NA: signRequest(fullUrl, method, body) authInterceptor.ts:73
    end
    end

    rect rgb(235,235,255)
    Note over AX,NA: SITE B. Global axios interceptor, installed at module load. Keeps its own URL and<br/>body derivation, then calls the SAME helper endpoints.ts:38-48
    C->>AX: axios.get or axios.put api-path via settingsApi wrappers
    AX->>NA: isAuthenticated() endpoints.ts:38
    alt isDevMode
        AX->>AX: config.headers Authorization Bearer dev-session-token endpoints.ts:44
        AX->>AX: config.headers X-Nostr-Pubkey user.pubkey endpoints.ts:45-47
    else authenticated
        AX->>AX: fullUrl = new URL(config.url, config.baseURL or origin).href endpoints.ts:50
        AX->>NA: signRequest(fullUrl, method, body) endpoints.ts:55
    end
    end

    Note over AI,AX: RESOLVED ADR-2074. FOUR transports carried this branch - authInterceptor, the<br/>axios global, contextLoader.fetchWithAuth and ldpClient.fetchWithAuth. All four<br/>now call one exported computeAuthHeaders(fullUrl, method, body) at<br/>authInterceptor.ts:52. The literal Bearer dev-session-token is constructed in<br/>exactly one place, authInterceptor.ts:65. Transport concerns (X-Request-ID, URL<br/>derivation, Headers copy, warn-on-failure) stay with each caller

    NA->>NA: build kind 27235 tags u=fullUrl method=METHOD nostrAuthService.ts:254-257,288-291
    Note over NA: INVARIANT. the u tag must be the exact request URL including query string. constructed via new URL(url, origin).href in both sites
    opt body present
        NA->>NA: sha256 digest hex append payload tag nostrAuthService.ts:259-266,293-300
    end
    NA->>NA: unsignedEvent kind 27235 created_at tags content empty nostrAuthService.ts:268-273,302-307
    alt localPrivateKey set (passkey-derived)
        NA->>NA: finalizeEvent(eventTemplate, localPrivateKey) nostrAuthService.ts:309
    else NIP-07 extension
        NA->>EXT: window.nostr.signEvent(unsignedEvent) nostrAuthService.ts:275
        alt no signer available
            NA-->>AI: throw No signing method available nostrAuthService.ts:250-251
        end
    end
    NA->>NA: btoa(JSON.stringify(signedEvent)) nostrAuthService.ts:276-277,310-311
    NA-->>AI: base64 token
    AI->>AI: headers Authorization = Nostr token authInterceptor.ts:74
    Note right of AI: server-side single-use replay cache and freshness window validate this token — ADR-2002 src/utils/nip98.rs
```

## VC-33.3 Session bearer / dev-token realm — the non-NIP-98 auth path
```mermaid
sequenceDiagram
    autonumber
    participant NA as NostrAuthService<br/>nostrAuthService.ts:236
    participant AI as authRequestInterceptor<br/>authInterceptor.ts:56-64
    participant LDP as fetchWithAuth<br/>client/src/services/solidPod/ldpClient.ts:91
    participant S as Server verify_access<br/>src/utils/auth.rs (ADR-2009)

    Note over NA: isDevMode() true requires BOTH import.meta.env.DEV and<br/>VITE_DEV_MODE_AUTH=true nostrAuthService.ts:235-237. Build-time gate, not a runtime<br/>toggle
    AI->>AI: computeAuthHeaders(fullUrl, method, body) authInterceptor.ts:52
    AI->>AI: Authorization Bearer dev-session-token authInterceptor.ts:65
    AI->>AI: X-Nostr-Pubkey user.pubkey authInterceptor.ts:68-70
    Note over AI: RESOLVED ADR-2074. The dev-token pair is constructed in ONE place.<br/>The other three HTTP transports call the same helper and add nothing of<br/>their own - endpoints.ts:48, contextLoader.ts:31, ldpClient.ts:105.<br/>An earlier reading called two of them ungated. that was WRONG - all four<br/>always branched on isDevMode. the defect was duplication, now closed
    par WebSocket auth frames stay separate - browsers cannot set WS headers
        Note over AI: client/src/api/analyticsApi.ts:450 sends JSON type auth token dev-session-token
    and
        Note over AI: client/src/services/VoiceWebSocketService.ts:144 sends the same shape - ADR-2075, see VC-35.4
    and
        Note over AI: client/src/store/websocket/connectionManager.ts:379 sends its own - see VC-32.1
    end
    Note over AI: A further reference at<br/>client/src/features/graph/managers/dataManager/restClient.ts:215 is a COMMENT<br/>documenting the pattern, not a call site
    AI->>S: request with Bearer dev-session-token
    alt server built with --features dev-auth AND DEV_AUTH_LOOPBACK=1 AND loopback peer<br/>(ADR-2012 triple gate)
        S-->>AI: 200 dev-admin principal granted
    else release build or gate not satisfied
        S-->>AI: 401 Unauthorized
        AI->>AI: warnIfServerInReleaseMode(401, sentDevToken=true) authInterceptor.ts:27-39,93-100
        Note over AI: one-shot console.warn. skipAuth is informational only client-side. the<br/>server alone decides ADR-06 section D1 comment authInterceptor.ts:12-25
    end

    Note over NA,S: NON-DEV realm. ADR-2009 legacy session-bearer fallback (UUID minted at<br/>login, plain-equality check against token_expiry) is a SERVER-side acceptance path in<br/>src/utils/auth.rs and src/services/nostr_service.rs:478. No client code in this tree<br/>ever constructs an X-Nostr-Token session header — nostrAuthService.getSessionToken()<br/>always returns null nostrAuthService.ts:433-436. The browser client's only two realms<br/>actually exercised are NIP-98 (VC-33.2) and the dev-session-token bearer above
```

## VC-33.4 Passkey / WebAuthn registration and authentication ceremonies
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant UI as Passkey UI component
    participant PS as passkeyService<br/>client/src/services/passkeyService.ts
    participant WA as navigator.credentials<br/>WebAuthn API
    participant S as /idp/passkey/* routes

    rect rgb(235,255,235)
    Note over UI,S: Registration
    UI->>PS: startRegistration(username) passkeyService.ts:81
    PS->>S: POST /idp/passkey/register-new/options passkeyService.ts:82-86
    alt username taken
        S-->>PS: 409
        PS-->>UI: throw Username already taken passkeyService.ts:88-90
    else route disabled
        S-->>PS: 405
        PS-->>UI: throw Passkey registration is not enabled on this server passkeyService.ts:95-97
    else ok
        S-->>PS: RegistrationOptionsResponse challenge rp user challengeKey passkeyService.ts:18-26
        PS->>PS: createPasskeyCredential(options) passkeyService.ts:132
        PS->>PS: publicKeyOptions extensions prf eval first=PRF_SALT passkeyService.ts:150-153
        Note over PS: PRF_SALT fixed 32-byte value from TextEncoder solid-nostr-prf-v1 must match<br/>server-side registration views passkeyService.ts:9-11
        PS->>WA: navigator.credentials.create({publicKey}) passkeyService.ts:155-157
        alt user cancels
            WA-->>PS: null
            PS-->>UI: throw Passkey creation was cancelled passkeyService.ts:159-161
        else success
            WA-->>PS: PublicKeyCredential + clientExtensionResults.prf passkeyService.ts:164-166
            PS->>PS: deriveNostrKey(prfOutput) HKDF SHA-256 info nostr-secp256k1-v1 to 256-bit key passkeyService.ts:60-73
            PS->>S: POST /idp/passkey/register-new/verify challengeKey pubkey prfEnabled credential passkeyService.ts:184-203
            S-->>PS: RegistrationVerifyResponse accountId webId completionToken passkeyService.ts:36-41
        end
    end
    end

    rect rgb(235,245,255)
    Note over UI,S: Authentication
    UI->>PS: startLogin(username?) passkeyService.ts:219
    PS->>S: POST /idp/passkey/login/options passkeyService.ts:220-224
    alt route disabled
        S-->>PS: 405
        PS-->>UI: throw Passkey login is not enabled on this server passkeyService.ts:226-228
    else ok
        S-->>PS: AuthenticationOptionsResponse challenge rpId allowCredentials passkeyService.ts:28-34
        PS->>WA: navigator.credentials.get({publicKey extensions.prf}) passkeyService.ts:256-258
        alt user cancels
            WA-->>PS: null
            PS-->>UI: throw Passkey authentication was cancelled passkeyService.ts:260-262
        else success
            WA-->>PS: assertion + prfOutput passkeyService.ts:264-266
            PS->>S: POST /idp/passkey/login/verify challengeKey credential passkeyService.ts:282-301
            S-->>PS: AuthenticationVerifyResponse accountId webId passkeyService.ts:43-47
            PS->>PS: deriveNostrKey(prfOutput) same HKDF as registration
            Note over PS: caller then invokes nostrAuth.loginWithPasskey(pubkey, privateKey) — see VC-33.9 for storage lifetime of the derived key
        end
    end
    end
    Note over PS: fallback for authenticators without PRF — downloadKeyBackup() generates a random<br/>key and forces a file download, marking it the ONLY copy passkeyService.ts:349-377
```

## VC-33.5 Logout / session teardown — what survives
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant H as useNostrAuth<br/>useNostrAuth.ts:81
    participant NA as NostrAuthService<br/>nostrAuthService.ts:427
    participant SPS as SolidPodService<br/>client/src/services/SolidPodService.ts:426
    participant LS as localStorage
    participant SS as sessionStorage

    U->>H: logout()
    H->>NA: logout() nostrAuthService.ts:427
    NA->>NA: clearSession() nostrAuthService.ts:446-465
    NA->>NA: currentUser = null
    critical wipe key material
        NA->>NA: localPrivateKey.fill(0) then null nostrAuthService.ts:448-451
        NA->>NA: module-scoped _localKeyHex cleared nostrAuthService.ts:452-455
    end
    NA->>LS: removeItem nostr_user nostrAuthService.ts:456
    NA->>LS: removeItem nostr_session_token legacy nostrAuthService.ts:458
    NA->>SS: removeItem nostr_privkey, nostr_passkey_pubkey, nostr_prf nostrAuthService.ts:460-464
    NA-->>H: notifyListeners authenticated false nostrAuthService.ts:430
    H-->>U: authState cleared useNostrAuth.ts:84-86

    Note over NA,SS: NOT cleared by logout(). sessionStorage ephemeral_session_pubkey (dev mode tab<br/>identity, nostrAuthService.ts:323-327) survives — it is only ever read in<br/>isDevMode(), so it has no effect on a normal logged-out session
    par app-level teardown wired at client/src/app/App.tsx:60,65
        Note over SPS: App.tsx effect on authenticated=false calls solidPodService.disconnect()
        SPS->>SPS: notifications.disconnect() SolidPodService.ts:426
    end
    Note right of SS: window beforeunload listener also wipes _localKeyHex on tab close nostrAuthService.ts:40-47, independent of explicit logout
```

## VC-33.6 RBAC-driven UI gating — client has no role model, only isPowerUser (DIVERGENCE)
```mermaid
sequenceDiagram
    autonumber
    participant NA as NostrAuthService<br/>nostrAuthService.ts:404-408
    participant App as App.tsx effect<br/>client/src/app/App.tsx:43-51
    participant Store as settingsStore coreSlice<br/>client/src/store/settings/coreSlice.ts:176-179
    participant Row as SettingRow<br/>client/src/features/control-center/primitives/SettingRow.tsx:50,64
    participant Ctl as NostrAuthControl<br/>client/src/features/control-center/primitives/NostrAuthControl.tsx:20,56

    NA->>App: AuthState authenticated user.isPowerUser App.tsx:36
    App->>Store: setUser({isPowerUser, pubkey}) App.tsx:47-50, coreSlice.ts:176-179
    Note over Store: isPowerUser is a single boolean, always false from<br/>login()/loginWithPasskey() and true only from devLogin()/dev-mode auto-login. NEVER the<br/>four-tier Owner/Admin/Editor/Viewer role — nostrAuthService.ts:407,542,630 vs 332,674
    Store->>Row: isPowerUser via useSettingsStore selector SettingRow.tsx:50
    Row->>Row: isWritable = !disabled && (!field.isPowerUserOnly || isPowerUser) SettingRow.tsx:64
    alt field.isPowerUserOnly and not isPowerUser
        Row->>Row: render disabled with Lock icon SettingRow.tsx:81
    else writable
        Row->>Row: render editable control
    end
    Store->>Ctl: isPowerUser via selector NostrAuthControl.tsx:20
    alt isPowerUser
        Ctl->>Ctl: show "Power User - Full access" badge NostrAuthControl.tsx:56-60
    end

    Note over Row,Ctl: DIVERGENCE. this is the ENTIRE client-side gating surface — a binary<br/>power-user flag, not the server's UserRole lattice (Owner greater than Admin greater<br/>than Editor greater than Viewer, docs/BASELINE-architecture.md:203). The client never<br/>fetches its own resolved role. All real enforcement happens server-side per-request in<br/>RbacGate and a rejected write surfaces only as a 401/403 after the fact, not as<br/>pre-emptive UI disablement of non-power-user fields tied to WriteGraph/Admin
    Note over Row: DOC-DRIFT. docs/BASELINE-architecture.md:222 cites the structural default at<br/>rbac_gate.rs:122-128 - public_reads_enabled is at :126-133 with its doc at :121-125.<br/>DOC-DRIFT. docs/BASELINE-architecture.md:218-228 documents server RBAC<br/>posture only. It makes no claim the client UI reflects role — none of the governing docs<br/>claim client-side role gating exists, matching what was found here

    Note over Row,Ctl: server posture (not client-enforced). structural default<br/>RBAC_PUBLIC_READS=false src/middleware/rbac_gate.rs:126-132 unwrap_or(false), but<br/>docker-compose.unified.yml:93 sets RBAC_PUBLIC_READS=1 in the dev/unified compose<br/>service. An unassigned authenticated pubkey resolves to Editor via<br/>UserRole::default_authenticated() src/models/rbac.rs:68-70,<br/>src/services/role_store.rs:188,196-198 — see docs/BASELINE-architecture.md:203-211
    Note over Row,Ctl: DIVERGENCE. compose ships public reads open and<br/>unassigned-pubkey-is-Editor by default, while the Rust struct-level default is<br/>fail-closed (no public reads) — two different postures depending whether you read the<br/>binary default or the shipped compose env
```

## VC-33.7 WS authenticate boundary — link to VC-32.1
```mermaid
sequenceDiagram
    autonumber
    participant NA as NostrAuthService<br/>nostrAuthService.ts:197
    participant CM as connectionManager<br/>client/src/store/websocket/connectionManager.ts

    Note over NA,CM: The browser client's WebSocket authenticate frame (dev-session-token / NIP-98<br/>event, header-equivalent) is a SEPARATE handshake from every REST NIP-98 signing<br/>site drawn in VC-33.2/VC-33.3. Full sequence, backoff and DIVERGENCE note on the<br/>accepted query-param form already drawn — see VC-32.1 Connect + NIP-98 WS<br/>authenticate handshake
    NA->>CM: same isDevMode()/isAuthenticated() state consulted by REST interceptors feeds the WS authenticate payload
    Note right of CM: not redrawn here per brief instruction — this file only marks the connection point into VC-32.1
```

## VC-33.8 Solid login boundary — OIDC-shaped WebID, session restore
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant SPS as SolidPodService<br/>client/src/services/SolidPodService.ts:115
    participant LDP as ldpClient fetchWithAuth<br/>client/src/services/solidPod/ldpClient.ts:91
    participant NA as NostrAuthService (delegate)
    participant S as Solid/JSS server via /solid proxy

    U->>SPS: connectToPod(npub) SolidPodService.ts:384
    SPS->>SPS: validate npub startsWith npub1 SolidPodService.ts:386
    alt invalid format
        SPS-->>U: throw Invalid npub format SolidPodService.ts:386
    else
        SPS->>LDP: fetchWithAuth POST pods/connect body npub SolidPodService.ts:389-392
        LDP->>NA: isAuthenticated() / isDevMode() / signRequest(url,method,body) ldpClient.ts:97-113
        Note over LDP: SAME dual-branch NIP-98/dev-token pattern as VC-33.2/VC-33.3 — no<br/>separate Solid-specific OIDC login exists in this client. WebID identity is bootstrapped<br/>FROM the Nostr pubkey (solid:oidcIssuer = did:nostr:hex-pubkey, per<br/>client/src/__tests__/agent-pod/pod-provisioning.test.ts:104), not via a browser OIDC<br/>redirect flow
        LDP->>S: fetch credentials include ldpClient.ts:122
        alt ok
            S-->>LDP: {podUrl, webId} pods/connect response
            SPS->>SPS: notifications.connect() if JSS_WS_URL and not connected SolidPodService.ts:400
        else error
            S-->>LDP: non-2xx
            SPS-->>U: throw Pod connection failed SolidPodService.ts:394-397
        end
    end

    Note over SPS: Session restore. getPodStructure()/initPod() is called on every operation<br/>(setPreference, saveGraphView, etc) — POST pods/init is idempotent and re-derives<br/>podUrl/webId/structure from the current NIP-98 identity each time. No separate WebID<br/>session token is cached client-side SolidPodService.ts:171-206
    Note over SPS,S: Auth boundary only. Full Solid data path (LDP CRUD, WAC ACLs, Type<br/>Index discovery, agent memory) — see VC-26
```

## VC-33.9 Identity-bearing types held by the browser client
```mermaid
classDiagram
    class SimpleNostrUser {
        +string pubkey
        +string npub optional
        +bool isPowerUser
        storage localStorage key nostr_user JSON
        lifetime survives reload until logout or stale-session detect
        definedAt nostrAuthService.ts:149-153
    }
    class LocalPrivateKeyMaterial {
        +Uint8Array localPrivateKey
        +string _localKeyHex module closure
        storage in-memory only, never sessionStorage
        lifetime cleared on logout, tab close beforeunload
        definedAt nostrAuthService.ts:14,200
    }
    class PasskeyPubkeyMarker {
        +string pubkey hex
        storage sessionStorage key nostr_passkey_pubkey
        lifetime tab session, verified against derived pubkey on restore
        definedAt nostrAuthService.ts:549,602
    }
    class DevSessionToken {
        +string literal Bearer dev-session-token
        storage not stored, recomputed per-request from isDevMode
        lifetime request-scoped, no expiry field
        definedAt authInterceptor.ts:60, endpoints.ts:44, ldpClient.ts:99
    }
    class Nip98EventToken {
        +string base64 signed kind 27235 event
        storage not stored, minted per-request in signRequest/signWithLocalKey
        lifetime single-use, server-enforced via replay cache ADR-2002
        definedAt nostrAuthService.ts:244,283
    }
    class WebIdUrl {
        +string webId URL
        storage returned by pods/connect and pods/init, not independently cached
        lifetime re-derived from current Nostr identity on every pod operation
        definedAt SolidPodService.ts:88,104,108
    }
    class PasskeyCredentialId {
        +string id base64url
        +ArrayBuffer rawId
        storage held only in the WebAuthn PublicKeyCredential object during ceremony
        lifetime ceremony-scoped, not persisted by this client
        definedAt passkeyService.ts:49-52
    }
    class LegacySessionBearer {
        +string X-Nostr-Token header value
        +UUID minted at login, plain-equality check vs token_expiry
        storage NEVER constructed by this client
        lifetime N per ADR-2009 server accepts it but no client code path exists
        definedAt ADR-2009 verified_paths list, nostr_service.rs:478
    }

    SimpleNostrUser --> LocalPrivateKeyMaterial : signs with when passkey path chosen
    SimpleNostrUser --> PasskeyPubkeyMarker : cross-checked on restore
    LocalPrivateKeyMaterial --> Nip98EventToken : produces via signWithLocalKey
    SimpleNostrUser --> Nip98EventToken : produces via window.nostr signEvent
    SimpleNostrUser --> DevSessionToken : substitutes in dev builds
    SimpleNostrUser --> WebIdUrl : bootstraps did-nostr issuer for
    PasskeyCredentialId --> LocalPrivateKeyMaterial : PRF output HKDF-derives

    note for SimpleNostrUser "DOC-DRIFT-adjacent co-storage risk. taxonomy line 57 makes pubkey 64-char<br/>lowercase hex and line 92 makes npub UI-display-only, yet nostr_user persists BOTH<br/>together. Not a breach - taxonomy line 178-179 scopes hex-canonical to persisted<br/>URN, DID and ACL entries, not a browser-local UI cache - but it is exactly the<br/>mixing pattern taxonomy line 169-171 warns about."
    note for LegacySessionBearer "DOC-DRIFT-adjacent. ADR-2009 lists authInterceptor, restClient, endpoints,<br/>ldpClient and contextLoader as client dependents of this realm, but none of them<br/>builds an X-Nostr-Token header. They emit only Bearer dev-session-token or a<br/>NIP-98 Nostr token. Verified by reading every cited client file in this diagram<br/>set."
```




