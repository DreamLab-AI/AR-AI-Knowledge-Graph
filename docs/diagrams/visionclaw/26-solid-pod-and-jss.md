---
id: VC-26
title: Solid Pod integration — embedded pod, proxy, client stack
area: visionclaw
governing:
  - docs/DATA-authority-erasure.md
  - docs/IDENTITY-authority-chain.md
adrs: [ADR-2067, ADR-2068, ADR-2070]
sources:
  - Cargo.toml
  - src/main.rs
  - src/handlers/solid_proxy_handler.rs
  - src/utils/nip98.rs
  - client/src/services/SolidPodService.ts
  - client/src/services/solidPod/ldpClient.ts
  - client/src/services/solidPod/agentMemory.ts
  - client/src/services/solidPod/typeIndex.ts
  - client/src/services/solidPod/wacManager.ts
  - client/src/services/solidPod/podNotifications.ts
  - client/src/services/SolidGraphViewService.ts
  - client/src/features/ontology/services/jss/contextLoader.ts
  - client/src/features/solid/hooks/useSolidPod.ts
  - client/src/features/solid/hooks/useSolidContainer.ts
  - client/src/features/solid/hooks/useSolidResource.ts
  - client/src/features/solid/components/SolidTabContent.tsx
  - client/src/features/control-center/panels/SolidPanel.tsx
  - client/src/store/websocket/solidWebSocket.ts
  - client/src/features/ontology/services/jss/schemaParser.ts
  - client/src/features/solid/components/PodBrowser.tsx
  - client/src/features/solid/components/PodSettings.tsx
  - client/src/features/solid/components/ResourceEditor.tsx
  - src/handlers/image_gen_handler.rs
verified_commit: 7a20db228
---

## VC-26.1 Deployment topology — embedded pod vs feature-off stub
```mermaid
flowchart TB
    subgraph cargo["Cargo.toml"]
        DEFAULT["default = [gpu, ontology, persistence-oxigraph, solid-pod-embed]<br/>Cargo.toml:251"]
        FEAT["solid-pod-embed feature<br/>Cargo.toml:283-285<br/>deps: solid-pod-rs, -nostr, -idp, -server"]
    end
    DEFAULT --> FEAT
    FEAT -->|"cfg(feature = solid-pod-embed)"| ON["ON: embedded solid-pod-rs"]
    FEAT -->|"cfg(not(feature = solid-pod-embed))"| OFF["OFF: stub build"]

    subgraph onpath["main.rs — feature ON"]
        INIT["init_solid_state().await<br/>main.rs:841"]
        APPDATA["app.app_data(solid_state.clone())<br/>main.rs:1016"]
        CFG["configure_solid_routes<br/>main.rs:1126"]
        FS["FsBackend::new(SOLID_DATA_ROOT)<br/>solid_proxy_handler.rs:120,133"]
        ROUTES["Full /solid scope: health, .notifications,<br/>pods*, LDP CRUD, DID<br/>solid_proxy_handler.rs:1752-1787"]
        INIT --> FS
        INIT --> APPDATA --> CFG --> ROUTES
    end

    subgraph offpath["main.rs — feature OFF"]
        NOINIT["solid_state app_data block compiled out<br/>main.rs:840-841 and :1015-1016"]
        STUBCFG["configure_routes (feature-off twin)<br/>solid_proxy_handler.rs:1799-1803"]
        STUBROUTES["RESOLVED ADR-2067 — registers nothing at all<br/>/solid/*, /.well-known/did.json and /did/* all 404<br/>in a feature-off build (was: a full table of 503 stubs)"]
        NOINIT --> STUBCFG --> STUBROUTES
    end

    ON --> onpath
    OFF --> offpath

    Note1["RESOLVED ADR-2067: with solid-pod-embed off, configure_routes now registers nothing and the eight per-handler 503 stub twins are deleted, so the /solid scope is genuinely absent rather than present-and-503. The feature-on path is unchanged and main.rs still calls configure_solid_routes unconditionally (signature preserved). get_global_storage keeps its feature-off twin - image_gen_handler.rs calls it unconditionally."]
    STUBROUTES -.-> Note1

    Legacy["/JavaScriptSolidServer (repo root)"]
    Note2["RESOLVED ADR-2068: the vendored JavaScriptSolidServer/ tree (63 MB) has been deleted. It was a third-party upstream project superseded by the embedded Rust solid-pod-rs, with no import, path or compose reference anywhere; its only mention was a doc-comment URL at src/utils/nip98.rs:5, left intact. Removed rather than archived - docs/archive/ is for our own superseded documents, and the upstream is recoverable from its own public repo."]
    Legacy -.-> Note2
```

## VC-26.2 solid_proxy_handler — route dispatch, auth, WAC
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant H as handle_solid_proxy<br/>solid_proxy_handler.rs:307
    participant AUTH as authenticate_request<br/>solid_proxy_handler.rs:242
    participant ACL as load_acl_for_path<br/>solid_proxy_handler.rs:890
    participant WAC as evaluate_access<br/>solid_pod_rs::wac (imported :55)
    participant FS as FsBackend<br/>solid_pod_rs::storage::fs (imported :53)

    C->>H: METHOD /solid/{tail} (cfg solid-pod-embed ON)
    H->>AUTH: extract_user_identity(req)
    alt Authorization: Nostr <token>
        AUTH->>AUTH: validate_nip98_token(token, url, method)<br/>nip98.rs:330 - see VC-26.4
        AUTH-->>H: Ok(Some(did:nostr:pubkey))
    else no Authorization and allow_anonymous
        AUTH-->>H: Ok(None)
    else no Authorization and NOT allow_anonymous
        AUTH-->>H: Err(401 Authentication required)
        H-->>C: 401 solid_proxy_handler.rs:248-251
    end
    H->>ACL: load_acl_for_path(storage, storage_path)
    ACL->>FS: get(/<res>.acl or /<container>/.acl)<br/>walk parents to /.acl (rs:896-927)
    FS-->>ACL: AclDocument or None
    H->>WAC: evaluate_access(acl, agent, path, method_to_mode(method))
    alt allowed
        H->>H: dispatch by method (rs:363-374)
        Note over H,FS: GET/HEAD/PUT/POST/DELETE/PATCH - see VC-26.5
    else denied, agent is None
        H-->>C: 401 solid_proxy_handler.rs:347-352
    else denied, agent present
        H-->>C: 403 WAC denies access (rs:353-359)
    end

    rect rgb(250,225,225)
    Note over C,H: cfg(not(feature = solid-pod-embed)) stub<br/>solid_proxy_handler.rs:378-390
    C->>H: METHOD /solid/{tail} (feature OFF)
    H-->>C: 503 Solid pod backend not available
    end
```

## VC-26.3 Login / pod session init — SolidPodService + useSolidPod
```mermaid
sequenceDiagram
    autonumber
    participant U as useSolidPod<br/>hooks/useSolidPod.ts:32
    participant NA as nostrAuth<br/>services/nostrAuthService
    participant S as SolidPodService<br/>services/SolidPodService.ts:115
    participant L as ldpClient.fetchWithAuth<br/>solidPod/ldpClient.ts:91
    participant B as init_pod / init_pod_nip98<br/>solid_proxy_handler.rs:1293,1349

    U->>U: useEffect - authenticated and nostrAuth.isAuthenticated()<br/>useSolidPod.ts:115-121
    U->>S: initPod() (checkPod / createPod call the same path)<br/>useSolidPod.ts:43,68 -> SolidPodService.ts:171
    S->>L: fetchWithAuth(POST /solid/pods/init, {})<br/>SolidPodService.ts:173
    L->>NA: isAuthenticated() / isDevMode()<br/>ldpClient.ts:97-98
    alt dev mode
        L->>L: headers Authorization: Bearer dev-session-token<br/>+ X-Nostr-Pubkey (ldpClient.ts:99-101)
    else NIP-98 mode
        L->>NA: signRequest(absoluteUrl, method, body)<br/>ldpClient.ts:109
        NA-->>L: NIP-98 token
        L->>L: headers Authorization: Nostr <token> (ldpClient.ts:110)
    end
    L->>B: fetch(url, {credentials: include})
    B->>B: get_user_from_request - NIP-98 or Bearer session<br/>solid_proxy_handler.rs:1416-1468
    B->>B: ensure_pod_exists(npub, pubkey, pod_base_url)<br/>solid_proxy_handler.rs:1136
    alt pod already exists
        B-->>L: 200 {pod_url, webid, created:false, structure}
    else pod missing
        B->>B: create_pod_with_structure - provision_pod + WAC root ACL<br/>solid_proxy_handler.rs:959,1081
        B-->>L: 200 {pod_url, webid, created:true, structure}
    end
    L-->>S: Response
    S-->>U: PodInitResult{success, podUrl, webId, created, structure}
    U->>U: setPodInfo(publicUrl(podUrl), publicUrl(webId))<br/>useSolidPod.ts:44-50,15-21
    Note over U,B: INVARIANT: pod check/create both funnel through initPod -<br/>there is no separate unauthenticated checkPod call path in practice
```

## VC-26.4 NIP-98 signed request reaching the pod — identity chain
```mermaid
sequenceDiagram
    autonumber
    participant NA as nostrAuth.signRequest<br/>client services/nostrAuthService
    participant EXT as NIP-07 extension / local key
    rect rgb(225,235,250)
    Note over NA,EXT: client trust boundary - key never leaves browser
    NA->>EXT: sign kind-27235 event (u, method tags)
    EXT-->>NA: signed Nostr event, base64-encoded token
    end
    NA-->>ldp: Authorization: Nostr <token>

    participant ldp as fetchWithAuth<br/>ldpClient.ts:91
    participant AUTH as extract_user_identity<br/>solid_proxy_handler.rs:184
    participant VAL as validate_nip98_token<br/>nip98.rs:330
    participant CACHE as REPLAY_CACHE<br/>nip98.rs:215 (Mutex<HashMap>)

    ldp->>AUTH: HTTP request, Authorization: Nostr <token>
    rect rgb(236,236,236)
    Note over AUTH,CACHE: server trust boundary - Schnorr verify happens here, not the proxy edge
    AUTH->>AUTH: reconstruct expected_url from X-Forwarded-Proto/Host/URI<br/>(rs:196-218)
    AUTH->>VAL: validate_nip98_token(token, expected_url, method, None)
    VAL->>VAL: base64/utf8/json decode (nip98.rs:337-343)
    VAL->>VAL: kind == 27235 check (nip98.rs:346-348)
    alt age > TOKEN_MAX_AGE_SECONDS (60s, nip98.rs:169,362)
        VAL-->>AUTH: Err(TokenExpired)
    else age < -TOKEN_MAX_AGE_SECONDS
        VAL-->>AUTH: Err(TokenFromFuture)
    else within window
        VAL->>VAL: extract u/method tags, urls_match(expected, actual)<br/>nip98.rs:524
        alt url or method mismatch
            VAL-->>AUTH: Err(UrlMismatch)
        else match
            VAL->>VAL: Schnorr signature verify (nip98.rs ~365)
            VAL->>CACHE: claim_event_id(event_id, now)<br/>nip98.rs:234
            alt event_id already claimed (within 2x window, nip98.rs:178)
                CACHE-->>VAL: Err(TokenReplayed)
            else first use
                CACHE-->>VAL: Ok(())
                VAL-->>AUTH: Ok(Nip98ValidationResult{pubkey})
            end
        end
    end
    end
    AUTH-->>ldp: Some(UserIdentity{pubkey}) or None (401)
    Note over VAL,CACHE: RESOLVED ADR-2070 - IDENTITY-authority-chain.md now cites nip98.rs:330. vc-core re-verified<br/>and corrected every citation in that section, not just this one (kind check, TOKEN_MAX_AGE_SECONDS, the<br/>past/future window, tag match, urls_match, payload hash, signature verify, replay claim and the cache TTL).
```

## VC-26.5 ldpClient CRUD — LDP resource operations
```mermaid
sequenceDiagram
    autonumber
    participant App as caller (agentMemory / typeIndex / wacManager)
    participant L as ldpClient<br/>solidPod/ldpClient.ts
    participant F as fetchWithAuth<br/>ldpClient.ts:91
    participant P as handle_solid_proxy<br/>solid_proxy_handler.rs:307

    App->>L: fetchJsonLd(path) / fetchTurtle(path)<br/>ldpClient.ts:130,144
    L->>F: GET, Accept ld+json|turtle
    F->>P: GET /solid/{tail}
    alt 2xx
        P-->>L: 200 body
        L-->>App: parsed JSON-LD / Turtle text
    else non-2xx
        L->>L: throw Error(status) (ldpClient.ts:137,151)
    end

    App->>L: putResource(path, data, contentType)<br/>ldpClient.ts:158
    L->>F: PUT body=JSON.stringify(data)|string
    F->>P: PUT /solid/{tail}
    alt response.ok
        P-->>L: 201/200 + ETag
        L-->>App: true
    else !ok
        L->>L: logger.error, return false (ldpClient.ts:172-175)
    end

    App->>L: postResource(containerPath, data, slug?)<br/>ldpClient.ts:182
    L->>F: POST, Slug header if provided
    F->>P: POST /solid/{tail}
    alt response.ok
        P-->>L: 201 + Location header
        L-->>App: Location string
    else !ok
        L-->>App: null (ldpClient.ts:197-200)
    end

    App->>L: deleteResource(path)<br/>ldpClient.ts:206
    L->>F: DELETE
    F->>P: DELETE /solid/{tail}
    alt response.ok
        L-->>App: true
    else status == 404
        Note over L: INVARIANT: deleteResource treats 404 as success -<br/>already-absent is not an error (ldpClient.ts:210,215)
        L-->>App: true
    else other non-2xx
        L-->>App: false (ldpClient.ts:211-212)
    end

    App->>L: resourceExists(path)<br/>ldpClient.ts:219
    L->>F: HEAD
    alt ok or network throw
        L-->>App: response.ok / false on catch (ldpClient.ts:222-226)
    end
```

## VC-26.6 agentMemory — read / write / delete
```mermaid
sequenceDiagram
    autonumber
    participant Caller as SolidPodService caller
    participant AM as agentMemory<br/>solidPod/agentMemory.ts
    participant ENS as ensureAgentContainer<br/>agentMemory.ts:52
    participant L as ldpClient (put/get/delete)
    participant WAC as wacManager.writeContainerAcl<br/>wacManager.ts:69

    Caller->>AM: storeAgentMemory(podPath, agentId, entry)<br/>agentMemory.ts:86
    AM->>ENS: ensureAgentContainer(podPath, agentId)
    loop for [agent/, agent/memory/] (agentMemory.ts:60-76)
        ENS->>L: resourceExists(path)
        alt !exists
            ENS->>L: PUT path, Link BasicContainer
            alt !response.ok and status != 409
                ENS-->>AM: null (agentMemory.ts:71-74)
            end
        end
    end
    AM->>L: putResource(container+safeKey.jsonld, doc)<br/>agentMemory.ts:115
    L-->>AM: success bool
    AM-->>Caller: stored (bool)

    Caller->>AM: getAgentMemory(podPath, agentId, key)<br/>agentMemory.ts:175
    AM->>L: fetchJsonLd(container+safeKey.jsonld)
    alt found
        L-->>AM: doc
        AM-->>Caller: doc
    else not found (throws)
        AM-->>Caller: null (agentMemory.ts:184-187)
    end

    Caller->>AM: deleteAgentMemory(podPath, agentId, key)<br/>agentMemory.ts:191
    AM->>L: deleteResource(container+safeKey.jsonld)<br/>agentMemory.ts:198
    L-->>AM: true (204 or already-404, see VC-26.5)
    AM-->>Caller: true
    Note over AM,L: DIVERGENCE: deleteAgentMemory has no RuVector counterpart -<br/>see VC-22.6 for the tombstone gap (DATA-authority-erasure.md "deleteAgentMemory tombstone gap")

    Caller->>AM: setAgentMemoryAccess(podPath, agentId, ownerWebId, perms)<br/>agentMemory.ts:202
    AM->>WAC: writeContainerAcl(containerPath, ownerWebId, perms)
    WAC-->>AM: ok (bool)
    AM-->>Caller: ok
```

## VC-26.7 wacManager — WAC ACL write (client) and read (server)
```mermaid
sequenceDiagram
    autonumber
    participant Caller as agentMemory / feature code
    participant WAC as writeContainerAcl<br/>wacManager.ts:69
    participant B as buildAclTurtle<br/>wacManager.ts:30
    participant F as fetchWithAuth (PUT .acl)<br/>ldpClient.ts:91
    participant H as handle_solid_proxy<br/>solid_proxy_handler.rs:307
    participant R as load_acl_for_path<br/>solid_proxy_handler.rs:890

    Caller->>WAC: writeContainerAcl(containerPath, ownerWebId, agentEntry)
    WAC->>B: buildAclTurtle(containerUrl, ownerWebId, agentEntry)<br/>emits acl:Authorization owner + agent (wacManager.ts:37-55)
    B-->>WAC: Turtle text
    WAC->>F: PUT {containerPath}.acl, Content-Type text/turtle<br/>wacManager.ts:75-82
    F->>H: PUT /solid/{container}.acl
    alt response.ok
        H-->>F: 201/200 (stored via FsBackend.put)
        WAC-->>Caller: true (wacManager.ts:89-94)
    else !ok
        WAC-->>Caller: false (wacManager.ts:84-87)
    end

    Note over H,R: A later GET/PUT/DELETE on any resource under this<br/>container re-triggers server-side ACL resolution (VC-26.2)
    H->>R: load_acl_for_path(storage, resource_path)
    R->>R: try {resource}.acl then walk parents to /.acl<br/>(solid_proxy_handler.rs:896-927)
    R-->>H: AclDocument (parsed via parse_acl_body, JSON-LD then Turtle)<br/>solid_proxy_handler.rs:932-944
```

## VC-26.8 typeIndex — registration and discovery
```mermaid
sequenceDiagram
    autonumber
    participant Caller as SolidPodService
    participant TI as typeIndex<br/>solidPod/typeIndex.ts
    participant L as ldpClient

    Caller->>TI: registerViewInTypeIndex(preferencesPath, viewName, viewUrl)<br/>typeIndex.ts:139
    TI->>TI: ensurePublicTypeIndex(preferencesPath)<br/>typeIndex.ts:69
    TI->>L: resourceExists(publicTypeIndex.jsonld)
    alt exists
        L-->>TI: true
    else missing
        TI->>L: putResource(typeIndexPath, empty solid:TypeIndex doc)<br/>typeIndex.ts:81-94
    end
    TI->>L: fetchJsonLd(typeIndexPath)
    TI->>TI: extractRegistrations(doc), dedupe by solid:instance<br/>typeIndex.ts:148-157
    alt already registered
        TI-->>Caller: true (no-op, typeIndex.ts:154-157)
    else new
        TI->>L: putResource(typeIndexPath, doc + newReg)<br/>typeIndex.ts:170
        L-->>TI: success bool
        TI-->>Caller: success
    end

    Caller->>TI: registerAgentInTypeIndex(preferencesPath, agentId, capabilities)<br/>typeIndex.ts:187
    Note over TI: Update semantics: filters out any existing vf:Agent entry<br/>for this agentId before pushing the new one (typeIndex.ts:199-201)

    Caller->>TI: discoverSharedViews(webId) / discoverAgents(webId)<br/>typeIndex.ts:228,258
    TI->>TI: resolveRemoteTypeIndex(webId)<br/>typeIndex.ts:293
    TI->>L: fetchWithAuth(webId, Accept ld+json)
    alt profile has solid:publicTypeIndex
        L-->>TI: typeIndexUrl
        TI->>L: fetchJsonLd(typeIndexUrl)
        TI-->>Caller: DiscoveredView[] / DiscoveredAgent[] filtered by forClass
    else no publicTypeIndex link or fetch fails
        TI-->>Caller: [] (typeIndex.ts:231-234,300-301)
    end
```

## VC-26.9 podNotifications subscription + solidWebSocket store
```mermaid
sequenceDiagram
    autonumber
    participant Hook as useSolidContainer/useSolidResource
    participant PN as PodNotificationManager<br/>podNotifications.ts:58
    participant WS as WebSocket (VITE_JSS_WS_URL)<br/>podNotifications.ts:18,87
    participant SW as solidWebSocket store adapter<br/>store/websocket/solidWebSocket.ts:71

    Hook->>PN: subscribe(resourceUrl, callback)<br/>podNotifications.ts:130
    alt first subscriber for this URL
        PN->>WS: send("sub " + resourceUrl") if OPEN<br/>podNotifications.ts:133-135
    end
    WS-->>PN: onmessage "protocol ..." | "ack ..." | "pub ..."<br/>podNotifications.ts:225-249
    alt msg starts with "pub "
        PN->>PN: notifySubscribers(url) + parent container (podNotifications.ts:251-259)
        PN-->>Hook: callback({type:'pub', url})
    else msg starts with "ack "
        PN-->>Hook: callback({type:'ack', url})
    end

    alt connection drops (onclose)
        PN->>PN: handleReconnect - backoff SOLID_RECONNECT_DELAY_MS * 2^attempt<br/>podNotifications.ts:275-289
        loop until SOLID_MAX_RECONNECT_ATTEMPTS (5, podNotifications.ts:32-33)
            PN->>WS: new WebSocket(JSS_WS_URL)
        end
    end

    Note over SW,WS: RESOLVED ADR-2100 (2026-09-05): there is ONE client. solidWebSocket.ts no longer opens<br/>its own socket - it is a thin store adapter over the podNotificationManager singleton<br/>(podNotifications.ts:300), registered once as solid-pod. The 10-vs-5 retry divergence is gone:<br/>both consumers share SOLID_MAX_RECONNECT_ATTEMPTS and SOLID_RECONNECT_DELAY_MS
    SW->>PN: connectSolidWebSocket(set) - delegates, no second WebSocket<br/>solidWebSocket.ts:71
    SW->>PN: subscribeSolidResource - podNotificationManager.subscribe<br/>solidWebSocket.ts:107
    PN-->>SW: onLifecycle event mirrored to store state + emit('solid-resource-changed')<br/>podNotifications.ts:186, solidWebSocket.ts:32
    Note over SW: state.solidSubscriptions is a BOOKKEEPING MIRROR only - it backs getSolidSubscriptions()<br/>and is never dispatched through, so a store callback fires exactly once
```

## VC-26.10 jss schemaParser — JSON-LD context load with cache
```mermaid
sequenceDiagram
    autonumber
    participant Caller as ontology jss consumer
    participant SP as fetchJsonLd<br/>jss/schemaParser.ts:64
    participant CL as contextLoader<br/>jss/contextLoader.ts
    participant F as fetchWithAuth<br/>contextLoader.ts:22

    Caller->>SP: fetchJsonLd(cache, metrics, {skipCache, timeout})
    alt !skipCache and isCacheValid(cache)<br/>schemaParser.ts:71 (ttlMs window)
        SP-->>Caller: cache.jsonLd (cache hit, metrics.cacheHitCount++)
    else cache miss or skipCache
        SP->>CL: getOntologyUrl() = SOLID_POD_BASE_URL + ONTOLOGY_RESOURCE_PATH<br/>contextLoader.ts:13-19
        SP->>SP: AbortController, setTimeout(timeout) default 30000ms<br/>schemaParser.ts:79-80
        SP->>F: fetchWithAuth(url, Accept ld+json, signal)
        alt response.ok
            F-->>SP: JSON body
            SP->>SP: cache.jsonLd = data - cache.timestamp = now()<br/>schemaParser.ts:93-95
            SP-->>Caller: JsonLdOntology
        else !response.ok
            SP-->>Caller: throw Error(status) (schemaParser.ts:89-90)
        else AbortError (timeout fired)
            SP-->>Caller: throw Error(ontology fetch timeout) (schemaParser.ts:109-110)
        end
    end
```

## VC-26.11 SolidGraphViewService — thin facade
```mermaid
sequenceDiagram
    autonumber
    participant Caller as graph-view UI code
    participant GV as SolidGraphViewService<br/>SolidGraphViewService.ts
    participant S as solidPodService singleton<br/>SolidPodService.ts:115

    Caller->>GV: saveGraphView(name, viewData)<br/>SolidGraphViewService.ts:17
    GV->>S: solidPodService.saveGraphView(name, viewData)
    S-->>GV: boolean
    GV-->>Caller: boolean

    Caller->>GV: loadGraphView(name) / listGraphViews()<br/>SolidGraphViewService.ts:31,37
    GV->>S: delegate to singleton method
    S-->>GV: view data | string[]
    GV-->>Caller: forwarded result

    Caller->>GV: deleteGraphView(name) / subscribeToGraphViewChanges(cb)<br/>SolidGraphViewService.ts:41,45
    GV->>S: delegate to singleton method
    Note over GV,S: INVARIANT: SolidGraphViewService holds no state of its own -<br/>every export is a 1:1 forward to the SolidPodService singleton (SolidGraphViewService.ts:12,17-49)
```

## VC-26.12 Client component/hook tree
```mermaid
flowchart TB
    Panel["SolidPanel<br/>control-center/panels/SolidPanel.tsx:10"]
    Tab["SolidTabContent<br/>solid/components/SolidTabContent.tsx:46"]
    Settings["PodSettings<br/>solid/components/PodSettings.tsx"]
    Browser["PodBrowser<br/>solid/components/PodBrowser.tsx"]
    Editor["ResourceEditor<br/>solid/components/ResourceEditor.tsx"]

    HPod["useSolidPod<br/>hooks/useSolidPod.ts:32"]
    HCont["useSolidContainer<br/>hooks/useSolidContainer.ts:31"]
    HRes["useSolidResource<br/>hooks/useSolidResource.ts:30"]

    Svc["SolidPodService singleton<br/>services/SolidPodService.ts:115"]

    Panel --> Tab
    Tab --> Settings
    Tab --> Browser
    Tab --> Editor

    Settings -->|"useSolidPod()<br/>PodSettings.tsx:29,102"| HPod
    Browser -->|"useSolidPod()<br/>PodBrowser.tsx:23,223"| HPod
    Browser -->|"useSolidContainer(containerPath)<br/>PodBrowser.tsx:24,162"| HCont
    Editor -->|"useSolidResource(resourceUrl)<br/>ResourceEditor.tsx:27,146"| HRes

    HPod --> Svc
    HCont --> Svc
    HRes --> Svc

    Svc -->|"fetchWithAuth -> /solid/*"| Proxy["solid_proxy_handler.rs<br/>see VC-26.2"]
```
