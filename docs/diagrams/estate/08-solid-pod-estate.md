---
id: ES-08
title: Solid-pod estate — four deployments, write identity, access control
area: estate
governing:
  - agentbox/docs/BASELINE-container.md
  - docs/DATA-authority-erasure.md
  - agentbox/docs/INGRESS-identity.md
adrs: [ADR-2015, ADR-2016, ADR-2017, ADR-2068]
sources:
  - Cargo.toml
  - src/handlers/solid_proxy_handler.rs
  - src/handlers/mod.rs
  - src/main.rs
  - agentbox/flake.nix
  - agentbox/agentbox.toml
  - agentbox/docker-compose.solid-pods.yml
  - agentbox/docs/user/solid-pod.md
  - agentbox/docs/user/multi-user-pods.md
  - agentbox/management-api/lib/pod-signer.js
  - agentbox/management-api/lib/elevation-publisher.js
  - agentbox/management-api/lib/agent-identity.js
  - agentbox/management-api/adapters/index.js
  - agentbox/management-api/adapters/pods/_solid-http-base.js
  - agentbox/management-api/adapters/pods/local-solid-rs.js
  - agentbox/management-api/routes/admin-users.js
  - client/src/services/SolidPodService.ts
  - client/src/services/solidPod/ldpClient.ts
  - client/src/services/solidPod/agentMemory.ts
  - client/src/services/solidPod/wacManager.ts
  - client/src/services/solidPod/typeIndex.ts
verified_commit: b00c28a0d
---
## ES-08.1 Four coexisting Solid-pod deployments — topology contrast

```mermaid
flowchart TB
    subgraph VC["VisionClaw process — single binary"]
        FLAG{{"cargo feature #quot;solid-pod-embed#quot;<br/>Cargo.toml:251,285-289 (default ON)"}}
        EMB["solid_proxy_handler.rs<br/>handle_solid_proxy:307"]
        STUB["503 stub handler<br/>solid_proxy_handler.rs:379-390"]
        FLAG -->|"feature ON"| EMB
        FLAG -->|"feature OFF"| STUB
    end

    subgraph AB["agentbox container — supervised service"]
        SUP["supervisord [program:solid-pod]<br/>agentbox/flake.nix:2064-2073"]
        SRV["solid-pod-rs-server :8484<br/>agentbox/agentbox.toml:459-467"]
        HTTPS["[program:https-bridge]<br/>agentbox/flake.nix:2081-2090"]
        SUP -->|"exec solidPodRsLauncher"| SRV
        HTTPS -->|"TLS terminate to :8484"| SRV
    end

    subgraph CF["Cloudflare Tunnel overlay"]
        CFD["cloudflared-pod<br/>agentbox/docker-compose.solid-pods.yml:26-33"]
    end
    CFD -->|"pods-native.dreamlab-ai.com to agentbox:8484"| SRV

    subgraph LEGACY["JavaScriptSolidServer — DELETED by ADR-2068"]
        JSS["was bin/jss.js :3000 at the repo root —<br/>a vendored third-party copy, now removed.<br/>see ES-08.10"]
    end

    MGMT["management-api adapters/pods/local-solid-rs.js<br/>DEFAULT_BASE=http://127.0.0.1:8484"] -->|"HTTP LDP"| SRV
    CLIENTB["client SolidPodService.ts<br/>SOLID_POD_BASE_URL=/solid"] -->|"proxied #quot;/solid/*#quot;"| EMB

    NOTE1["DIVERGENCE: docker-compose.solid-pods.yml is only the Cloudflare<br/>Tunnel sidecar for THIS SAME port 8484 server (deployment 2) - solid-pod.md:253-255.<br/>It is not a separate multi-user pods deployment. Multi-user pods<br/>(agentbox/docs/user/multi-user-pods.md) is a design scaffold, defaults<br/>off, targets this same server via admin-users.js - see ES-08.5"]
    NOTE2["DIVERGENCE: sources-of-truth conflict (legacy) - Pod claims<br/>write-master for some agent state while Oxigraph, GitHub public::true<br/>and RuVector claim primacy elsewhere per docs/DATA-authority-erasure.md<br/>Known divergences and open items; legacy prose unreconciled, not authority"]
    NOTE3["_solid-http-base.js:11-12 — this base class was renamed from<br/>local-jss.js after the Python/JSS stub was retired 2026-04-25;<br/>wire contract is protocol-level Solid, not JSS-specific"]
```

## ES-08.2 Agent pod write — NIP-98 signing via pod-signer (with unsigned fallback)

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent<br/>internal caller
    participant ADP as adapters/index.js<br/>slotConfig:53-70
    participant PS as pod-signer<br/>buildPodNip98:32
    participant BR as nostr-bridge<br/>loadSigner/buildNip98Header
    participant BASE as SolidHttpPodsAdapter<br/>_solid-http-base.js:26
    participant SRV as solid-pod-rs-server<br/>port 8484

    Note over ADP,PS: INVARIANT: nip98 is null unless integrations.solid_pod_rs.sign_requests=true<br/>(pod-signer.js:35) — default keeps prior unsigned behaviour byte-identical

    ADP->>PS: buildPodNip98(manifest, opts) pod-signer.js:32
    alt sign_requests off or no stack resolved
        PS-->>ADP: return null (pod-signer.js:35,44-49)
        Note over ADP,BASE: DIVERGENCE: unsigned pod-signing fallback (legacy ADR-026) —<br/>_solid-http-base.js constructs this._fetch=this._rawFetch (line 47)<br/>when nip98 is falsy, requests go out with no Authorization header
    else sign_requests on
        PS-->>ADP: nip98 fn method,url,body returning string or null
        ADP->>BASE: withSigner(cfg) attaches opts.nip98 adapters/index.js:61
    end

    AG->>BASE: write(uri, body) _solid-http-base.js:74
    BASE->>BASE: this._fetch = this._nip98 ? _signedFetch : _rawFetch (line 47)
    alt signer configured (this._nip98 set)
        BASE->>PS: nip98(method,url,body) _signedFetch:60-65
        PS->>PS: getSigner() lazy-load, cached (pod-signer.js:69-79)
        alt key load fails
            PS-->>BASE: null (loadFailed=true, cached — never retried)
            Note over BASE,SRV: DIVERGENCE: unsigned fallback — _signedFetch:63-64<br/>"if (!header) return this._rawFetch(...)" — silent, no error surfaced
            BASE->>SRV: PUT/POST (no Authorization header)
        else key loads
            PS->>BR: buildNip98Header(signer,method,url,body) pod-signer.js:84
            BR-->>PS: base64 kind-27235 event
            PS-->>BASE: Authorization header, Nostr token
            BASE->>SRV: PUT/POST with Authorization Nostr token
        end
    else no signer (nip98 null)
        BASE->>SRV: PUT/POST (unsigned, this._rawFetch)
    end
    SRV-->>BASE: 2xx / 401 / 403 (WAC — see ES-08.4)

    Note over PS: Lifecycle mirrors lib/elevation-publisher.js bridge+signer built ONCE,<br/>loaded lazily on first use, cached — a load failure is cached and never retried
    Note over AG,SRV: A degraded boot combining this unsigned fallback with the<br/>did:nostr:local placeholder (agent-identity.js:175,184) can silently<br/>produce a non-sovereign identity writing unsigned to a default-deny pod
```

## ES-08.3 User pod write — SolidPodService through ldpClient

```mermaid
sequenceDiagram
    autonumber
    participant UI as SolidPodService<br/>SolidPodService.ts:115
    participant LDP as ldpClient<br/>ldpClient.ts:91 fetchWithAuth
    participant NA as nostrAuthService<br/>nostrAuth.signRequest
    participant PROXY as VisionClaw "/solid" proxy<br/>ES-08.1
    participant SRV as embedded solid-pod-rs<br/>solid_proxy_handler.rs:307

    UI->>UI: setPreference(key,value) SolidPodService.ts:209
    UI->>UI: getPodStructure calls initPod() ts:198,168
    UI->>LDP: putResource(path, doc) ldpClient.ts:158
    LDP->>LDP: resolvePath(path) rewrites JSS hostnames to SOLID_POD_BASE_URL ldpClient.ts:53
    LDP->>LDP: fetchWithAuth(url, PUT) ldpClient.ts:91

    alt nostrAuth.isDevMode()
        LDP->>LDP: Authorization Bearer dev-session-token<br/>X-Nostr-Pubkey header ldpClient.ts:98-101
    else NIP-98 mode
        LDP->>NA: signRequest(absoluteUrl, method, body) ldpClient.ts:109
        NA-->>LDP: base64 NIP-98 token
        LDP->>LDP: Authorization Nostr token header ldpClient.ts:110
    end
    opt signRequest throws
        LDP->>LDP: logger.warn NIP-98 signing failed ldpClient.ts:112, request proceeds unsigned
    end

    LDP->>PROXY: fetch url with method PUT, headers, credentials include ldpClient.ts:122
    PROXY->>SRV: handle_solid_proxy dispatch — see ES-08.4 for WAC evaluation
    SRV-->>LDP: 2xx / non-ok
    LDP-->>UI: true / false (logger.error on !response.ok, ldpClient.ts:172-175)

    Note over UI,SRV: Client never issues PATCH — ldpClient.ts exposes only<br/>GET/PUT/POST/DELETE/HEAD (fetchJsonLd,fetchTurtle,putResource,<br/>postResource,deleteResource,resourceExists). Every user write is a<br/>whole-resource PUT or a new-resource POST, never a partial PATCH
    Note over UI: PodStructure fields (profile, ontology_contributions,<br/>ontology_proposals, ontology_annotations, preferences, inbox)<br/>SolidPodService.ts:75-82 — see ES-08.7 classDiagram
```

## ES-08.4 WAC-denied read — embedded handle_solid_proxy

```mermaid
sequenceDiagram
    autonumber
    participant C as Caller
    participant H as handle_solid_proxy<br/>solid_proxy_handler.rs:307
    participant AUTH as authenticate_request<br/>solid_proxy_handler.rs:238
    participant ACL as load_acl_for_path<br/>solid_proxy_handler.rs:889
    participant WAC as evaluate_access<br/>solid_pod_rs::wac (imported line 55)
    participant FS as FsBackend storage

    C->>H: GET /solid/tail solid_proxy_handler.rs:1843
    H->>AUTH: authenticate_request(req) line 316
    AUTH->>AUTH: extract_user_identity, parse NIP-98 Authorization header
    alt NIP-98 present and valid
        AUTH-->>H: Ok Some did:nostr pubkey line 240
    else no identity, allow_anonymous=true
        AUTH-->>H: Ok None line 242
    else no identity, allow_anonymous=false
        AUTH-->>H: Err 401 Authentication required line 244-247
        H-->>C: 401, return resp line 320
    end

    H->>H: method_to_mode(method) line 331
    H->>ACL: load_acl_for_path(storage, storage_path) line 338
    ACL->>FS: get resource.acl then walk parents to root .acl lines 897-925
    FS-->>ACL: AclDocument or None
    ACL-->>H: acl_doc

    H->>WAC: evaluate_access(acl_doc, agent, path, access_mode, None) lines 338-344
    alt allowed
        WAC-->>H: true
        H->>H: dispatch GET/PUT/POST/DELETE/PATCH lines 362-373
        H-->>C: 200, handle_get, ES-08.6 for PATCH
    else denied, agent is None
        WAC-->>H: false
        H-->>C: 401 Authentication required lines 347-352
    else denied, agent present
        WAC-->>H: false
        H-->>C: 403 WAC denies access_mode access to path lines 353-360
    end

    Note over H: Feature-gated stub not feature solid-pod-embed returns 503<br/>Compiled without solid-pod-embed feature unconditionally<br/>solid_proxy_handler.rs:379-390, see ES-08.1
    Note over ACL,FS: WAC resource-specific ACL, containers use dir slash dot acl,<br/>non-containers use resource dot acl WAC spec section 4.1, falls back to<br/>parent-container acl then root acl, solid_proxy_handler.rs:894-926
```

## ES-08.5 Pod provisioning for a new user

```mermaid
sequenceDiagram
    autonumber
    participant CL as Client
    participant R as init_pod_nip98<br/>solid_proxy_handler.rs:1349
    participant PE as pod_exists<br/>solid_proxy_handler.rs:952
    participant CS as create_pod_with_structure<br/>solid_proxy_handler.rs:959
    participant PP as provision_pod<br/>solid_pod_rs::provision (imported line 51)
    participant FS as FsBackend storage
    participant AU as admin-users.js<br/>POST /admin/users/provision:137

    CL->>R: POST /solid/pods/init-nip98 with NIP-98 header
    R->>R: validate_nip98_token, derive did:nostr pubkey
    R->>PE: pod_exists(storage, npub) line 952
    PE->>FS: exists(/npub/) line 953-954
    alt pod already exists
        FS-->>PE: true
        PE-->>R: true
        R-->>CL: existing PodStructure, created=false
    else pod absent
        FS-->>PE: false
        PE-->>R: false
        R->>CS: create_pod_with_structure(storage,npub,pubkey,base_url) line 959
        CS->>CS: build ProvisionPlan, containers profile,ontology,<br/>ontology/contributions,proposals,annotations,preferences,inbox line 968-978
        CS->>PP: provision_pod(storage, plan) line 989
        PP->>FS: write profile card, root acl, each container
        alt provision_pod partial failure
            PP-->>CS: Err(e)
            CS->>CS: warn, continue with manual structure creation line 999
        else success
            PP-->>CS: Ok(outcome), webid, containers_created
        end
        CS-->>R: PodStructure
        R-->>CL: 201, PodStructure, created=true
    end

    Note over AU: Separate agentbox-side path (management-api), same solid-pod-rs<br/>server, different call: AU calls POST /_admin/provision/pubkey on<br/>solid-pod-rs-server, PSK-gated by SOLID_ADMIN_KEY admin-users.js:28-35
    Note over AU: DOC-DRIFT: agentbox/docs/user/multi-user-pods.md says these<br/>lifecycle endpoints return 501 Not Implemented in the scaffold release.<br/>Code lands POST /admin/users/provision fully admin-users.js:137-186,<br/>201 with pod_url/web_id/git_url. Only suspend line 229-242 and<br/>archive line 245-258 still return 501, per code, not per the doc claim
    Note over AU: sovereign_mesh.multi_user defaults off agentbox.toml comment<br/>block near line 27, git auto_init true wires a git repo per pod<br/>ensurePodGit admin-users.js when GIT_POD_ENABLED not false
```

## ES-08.6 Non-destructive PATCH — N3 Patch, SPARQL-Update, JSON Patch dialects

```mermaid
sequenceDiagram
    autonumber
    participant C as Caller
    participant H as handle_patch<br/>solid_proxy_handler.rs:749
    participant D as patch_dialect_from_mime<br/>solid_pod_rs::ldp (imported line 47)
    participant G as Graph::parse_ntriples<br/>solid_pod_rs::ldp
    participant FS as FsBackend storage

    C->>H: PATCH /solid/tail, Content-Type header, body
    H->>D: patch_dialect_from_mime(content_type) line 761
    alt unsupported content type
        D-->>H: None
        H-->>C: 415 Unsupported patch format line 764-771
    else recognised dialect
        D-->>H: N3 or SparqlUpdate or JsonPatch line 761
    end

    H->>FS: storage.get(path) fetch current resource line 785
    alt resource exists
        FS-->>H: current_body, meta
        H->>G: Graph::parse_ntriples(current_str) line 791
        alt dialect is N3
            H->>H: apply_n3_patch(graph, patch_str) line 797
        else dialect is SparqlUpdate
            H->>H: apply_sparql_patch(graph, patch_str) line 798
        else dialect is JsonPatch
            H-->>C: 415 JSON Patch not supported for RDF resources line 799-805
        end
        alt patch applies
            H->>FS: put(path, new_body ntriples, application/n-triples) line 811-817
            FS-->>H: meta with etag
            H-->>C: 200, ETag header line 819-822
        else patch fails
            H-->>C: 422 Patch failed line 833-836
        end
    else resource absent
        FS-->>H: PodError::NotFound
        H->>H: apply_patch_to_absent(dialect, patch_str) line 841
        alt create succeeds
            H->>FS: put(path, new_graph ntriples) line 849-851
            H-->>C: 201 Created, ETag header line 853-856
        else create fails
            H-->>C: 422 Patch-to-absent failed line 867-870
        end
    end

    Note over H: Non-destructive PATCH is LANDED at the embedded VisionClaw path,<br/>not an open item: N3 Patch and SPARQL-Update apply against a parsed<br/>RDF graph rather than a whole-resource overwrite<br/>solid_proxy_handler.rs:44-48,797-798
    Note over C,FS: JSON Patch dialect is accepted by patch_dialect_from_mime<br/>but rejected for existing RDF resources line 799-805, only usable<br/>via apply_patch_to_absent on a not-yet-existing resource
    Note over H: agentbox/docs/user/solid-pod.md:41 PATCH dialects table<br/>N3 Patch, SPARQL-Update, JSON Patch matches this handler's dialect set
```

## ES-08.7 Client pod resource model

```mermaid
classDiagram
    class PodStructure {
        +string profile
        +string ontology_contributions
        +string ontology_proposals
        +string ontology_annotations
        +string preferences
        +string inbox
    }
    class PodInfo {
        +bool exists
        +string podUrl
        +string webId
        +string suggestedUrl
        +PodStructure structure
    }
    class PodInitResult {
        +bool success
        +string podUrl
        +string webId
        +bool created
        +PodStructure structure
        +string npub
        +string error
    }
    class JsonLdDocument {
        +string context
        +string type
        +string id
    }
    class TypeIndexDocument {
        +string type
        +List~TypeRegistration~ typeRegistration
    }
    class TypeRegistration {
        +string type
        +string forClass
        +string instance
        +string instanceContainer
    }
    class DiscoveredView {
        +string name
        +string url
    }
    class DiscoveredAgent {
        +string id
        +List~string~ capabilities
    }
    class AclEntry {
        +string agentWebId
        +List~AclMode~ modes
    }
    class AclMode {
        <<enumeration>>
        Read
        Write
        Append
        Control
    }

    PodInfo --> PodStructure : structure
    PodInitResult --> PodStructure : structure
    JsonLdDocument <.. TypeIndexDocument : extends
    TypeIndexDocument --> TypeRegistration : typeRegistration list
    TypeRegistration ..> DiscoveredView : forClass schema colon ViewAction
    TypeRegistration ..> DiscoveredAgent : forClass vf colon Agent
    AclEntry --> AclMode : modes

    note for PodStructure "SolidPodService.ts:75-82, one container per<br/>pod facet, agent memory containers are agents<br/>slash agentId slash memory not modelled here<br/>agentMemory.ts:44-47"
    note for TypeIndexDocument "typeIndex.ts:43-45, public Type Index at<br/>settingsBase publicTypeIndex.jsonld, ensurePublicTypeIndex:65"
    note for AclEntry "wacManager.ts:15-20, buildAclTurtle grants owner<br/>Read Write Control, agent gets requested modes only<br/>wacManager.ts:29-52"
```

## ES-08.8 Supervised solid-pod service lifecycle

```mermaid
stateDiagram-v2
    [*] --> STOPPED
    STOPPED --> STARTING : autostart true, flake.nix 2064-2073
    STARTING --> RUNNING : solidPodRsLauncher exec succeeds
    STARTING --> BACKOFF : exec fails within startsecs
    BACKOFF --> STARTING : autorestart true, retry
    BACKOFF --> FATAL : retries exceed startretries
    RUNNING --> EXITED : process exits zero or signal
    RUNNING --> STOPPING : supervisorctl stop solid-pod
    EXITED --> STARTING : autorestart true, respawn
    STOPPING --> STOPPED
    FATAL --> [*]

    note right of STARTING : priority 30, user devuser<br/>environment SOLID_POD_PUBLIC_URL, SOLID_ADMIN_KEY<br/>AGENTBOX_REQUIRED_FOR_READINESS true, flake.nix 2064-2073
    note right of RUNNING : https-bridge TLS-terminates to this port when<br/>sovereignCfg.https_bridge true, flake.nix 2081-2090
    note right of FATAL : DIVERGENCE, no point-in-time backup for the pod<br/>store, scripts backup-sqlite.sh covers SQLite only<br/>docs/DATA-authority-erasure.md Known divergences,<br/>no cross-store consistent restore, no declared RPO or RTO
```

## ES-08.9 deleteAgentMemory tombstone gap

```mermaid
sequenceDiagram
    autonumber
    participant UI as SolidPodService<br/>deleteAgentMemory:363
    participant AM as agentMemory.ts<br/>deleteAgentMemory:198
    participant LDP as ldpClient<br/>deleteResource:206
    participant SRV as embedded solid-pod-rs<br/>ES-08.1
    participant RV as RuVector Postgres<br/>mcp claude-flow memory_search HNSW

    UI->>AM: deleteAgentMemory(podPath, agentId, key) SolidPodService.ts:363-366
    AM->>AM: agentMemoryContainerPath(podPath, agentId) agentMemory.ts:200
    AM->>LDP: deleteResource(containerPath plus safeKey plus .jsonld) agentMemory.ts:202-204
    LDP->>SRV: DELETE, HTTP DELETE ldpClient.ts:206-216
    SRV-->>LDP: 204 No Content, or 404 treated as success line 210
    LDP-->>AM: true
    AM-->>UI: true

    Note over AM,RV: DIVERGENCE, deleteAgentMemory tombstone gap - this call graph<br/>never reaches RuVector. agentMemory.ts:196-204 has no reverse-tombstone<br/>dispatch to mcp claude-flow memory_delete or equivalent
    Note over RV: The embedding row for this key persists in RuVector and<br/>remains semantically searchable after the pod-side delete completes.<br/>Called the single largest erasure hole in<br/>docs/DATA-authority-erasure.md Known divergences and open items
    Note over UI,RV: see ES-07.9 for the RuVector embedding-pipeline side of<br/>this gap in the memory-and-embedding-estate topic
    Note over UI,RV: Invariant 6 of docs/DATA-authority-erasure.md requires<br/>a dropped secondary write to be logged, not swallowed - this call<br/>path does not even attempt the secondary write, let alone log its absence
```

## ES-08.10 JavaScriptSolidServer — legacy JS implementation, REMOVED from the repo

```mermaid
flowchart LR
    subgraph JSS["JavaScriptSolidServer/ — DELETED by ADR-2068"]
        PKG["was a 63 MB VENDORED COPY of the third-party<br/>github.com/JavaScriptSolidServer/JavaScriptSolidServer<br/>name javascript-solid-server, port 3000 default"]
        FEAT["had implemented LDP CRUD, N3 Patch, SPARQL Update,<br/>WAC dot acl, Solid-OIDC, NIP-98 Nostr auth,<br/>Git HTTP backend — all superseded by solid-pod-rs"]
    end

    subgraph RUST["solid-pod-rs (deployments 1 and 2)"]
        EMB2["VisionClaw embedded handler<br/>src/handlers/solid_proxy_handler.rs"]
        SRV2["agentbox supervised service port 8484"]
    end

    JSS -.->|"retired from the agentbox runtime 2026-04-25,<br/>then DELETED from the repo by ADR-2068"| RUST

    NOTE1["_solid-http-base.js:11-12, this client base class was<br/>previously named local-jss.js after the legacy JSS stub,<br/>retired 2026-04-25, renamed since the wire contract is<br/>protocol-level Solid not JSS-specific"]
    NOTE2["agentbox/docs/user/solid-pod.md:58-60, the 2026-04-25<br/>cleanup removed the Python local-jss stub entirely,<br/>either local-solid-rs, external, or off - no second<br/>pod implementation ships in agentbox"]
    NOTE3["RESOLVED ADR-2068 (vc-knowledge, 2026-09-05) — the directory is<br/>GONE from the working tree, not archived. It was a vendored copy of a<br/>third-party project, wired into no supervised process in either repo,<br/>and superseded by solid-pod-rs on both deployment paths. This diagram<br/>is retained as the record of what was removed and why."]
```
