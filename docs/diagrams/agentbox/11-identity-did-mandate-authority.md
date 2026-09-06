---
id: AB-11
title: Identity — DID, URN, mandate, authority
area: agentbox
governing:
  - agentbox/docs/INGRESS-identity.md
  - agentbox/docs/PROTOCOL-registry.md
adrs: [ADR-2011, ADR-2025, ADR-2027, ADR-2064]
sources:
  - agentbox/management-api/lib/agent-identity.js
  - agentbox/management-api/lib/uris.js
  - agentbox/management-api/lib/mandate.js
  - agentbox/management-api/lib/authority.js
  - agentbox/management-api/lib/authority-consumer.js
  - agentbox/management-api/lib/capability-scope.js
  - agentbox/management-api/lib/pod-signer.js
  - agentbox/management-api/adapters/pods/_solid-http-base.js
  - agentbox/management-api/adapters/index.js
  - agentbox/management-api/lib/agent-event-auth.js
  - agentbox/management-api/lib/per-user-agent.js
  - agentbox/management-api/routes/mandate.js
  - agentbox/management-api/routes/uri-resolver.js
  - agentbox/management-api/routes/well-known.js
  - agentbox/config/entrypoint-unified.sh
  - agentbox/management-api/lib/agent-control-surface.js
  - agentbox/management-api/routes/agent-events.js
  - agentbox/management-api/server.js
  - agentbox/mcp/servers/nostr-bridge.js
  - agentbox/agentbox.toml
  - agentbox/management-api/lib/bc20-provenance-bridge.js
verified_commit: 7a20db228
---

## AB-11.1 Identity, URN and mandate type model

```mermaid
classDiagram
    class AgentIdentity {
        <<returned by loadOrMint>>
        +string did
        +string pubkey
        +string multikey
        +string keyPath
        +bool minted
        +bool persisted
        -string privHex
    }
    class MULTIKEY {
        <<const agent-identity.js:45>>
        +string MULTIKEY_PREFIX
    }
    class UrnKindSpec {
        <<uris.js:87 KINDS frozen>>
        +bool ownerScope
        +bool scopeRequired
        +bool contentAddressed
        +string resolvableSurface
    }
    class MandateRecord {
        <<mandate.js:99 createMandate>>
        +string issuer
        +string agent
        +string container
        +List~string~ modes
        +int issued_at
        +int expires_at
        +bool revoked
        +string urn
    }
    class AuthorityGate {
        <<authority.js:136>>
        +object table
        +classifyAction(actionClass, opts)
        +guard(actionClass, ctx)
    }
    class CapabilityScope {
        <<capability-scope.js:320 export>>
        +List~string~ EFFECT_TYPES
        +List~string~ TRUST_CLASSES
        +int DEFAULT_DISPOSE_TIMEOUT_MS
    }
    AgentIdentity --> MULTIKEY : multikeyFromXonly
    UrnKindSpec <-- MandateRecord : kind mandate ownerScope+scopeRequired+contentAddressed
    MandateRecord --> AgentIdentity : issuer and agent are did nostr
    AuthorityGate --> CapabilityScope : narrows what a granted action may touch
    note for MULTIKEY "MULTIKEY_PREFIX = fe70102 -> f base16-lower + e701 varint multicodec + 02 compressed-point tag<br/>Fixed 71-char publicKeyMultibase. agent-identity.js:43 notes the derivation always yields even-y so 02 is invariant."
    note for AgentIdentity "INVARIANT ADR-2011 hex-canonical: lowercase 64-hex BIP-340 x-only pubkey is the single storage and URL identity<br/>did is did:nostr:hex. npub bech32 is display or symlink form only. privHex NEVER leaves agent-identity.js (agent-identity.js:141)."
    note for UrnKindSpec "19 kinds at uris.js:87 — pod envelope credential mandate receipt activity event decision mcp memory skill adr prd ddd thing dataset bead agent meta<br/>decision IS-A prov:Activity (legacy ADR-048) and mirrors activity plumbing. bead is content-addressed to match urn:visionclaw:bead so BC20 can cross it (audit 2026-06-09 A3)."
```

## AB-11.2 loadOrMint — private-key precedence and 0600 persistence

```mermaid
sequenceDiagram
    autonumber
    participant CALLER as caller<br/>agentbox/management-api/server.js
    participant AI as loadOrMint<br/>agentbox/management-api/lib/agent-identity.js:107
    participant ENV as process.env
    participant FS as profile key file<br/>profileKeyPath opts
    participant NT as nostr-tools<br/>getNostrTools

    CALLER->>AI: loadOrMint({keyPath, profile, identityDir})
    rect rgb(235,245,255)
    Note over AI,ENV: PRECEDENCE 1 — stable-identity injection
    AI->>ENV: AGENTBOX_AGENT_PRIVKEY_HEX (agent-identity.js:113)
    ENV-->>AI: value (trimmed, lowercased)
    alt HEX64.test(envHex)
        AI->>AI: privHex = envHex
    else not 64-hex or unset
        rect rgb(240,255,240)
        Note over AI,FS: PRECEDENCE 2 — persisted profile key
        AI->>FS: fs.readFileSync(keyPath, utf8).trim().toLowerCase()
        alt HEX64.test(stored)
            FS-->>AI: privHex = stored
        else read throws (no key yet)
            FS-->>AI: catch — fall through to mint
            rect rgb(255,248,235)
            Note over AI,NT: PRECEDENCE 3 — fresh mint
            AI->>NT: generateSecretKey()
            NT-->>AI: Uint8Array -> Buffer.toString('hex')
            AI->>AI: minted = true
            end
        end
        end
    end
    end
    AI->>AI: xOnly = deriveXonly(privHex)
    alt xOnly falsy or not HEX64
        AI-->>CALLER: return null
        Note over AI,CALLER: DIVERGENCE INGRESS-identity: caller then keeps did:nostr:local — a degraded boot yields a non-sovereign identity. see AB-11.3
    else derived
        critical persist so the DID survives a restart
            AI->>FS: mkdirSync(dirname, recursive) then writeFileSync(privHex, mode 0o600) then chmodSync 0o600 (agent-identity.js:141-142)
            FS-->>AI: persisted = true
        option write throws
            FS-->>AI: catch — persisted = false, NON-FATAL
            Note over AI,FS: DIVERGENCE SECURITY-profiles custody: key persistence failure yields a valid but unstable identity — the DID changes on next boot
        end
        AI-->>CALLER: {did, pubkey, multikey, keyPath, minted, persisted}
    end
    Note over AI,FS: INVARIANT ADR-2011 — privHex is never returned, logged or printed. Only did / x-only pubkey / multikey are emitted (agent-identity.js exports at :167).
```

## AB-11.3 agent-identity CLI mint — entrypoint export contract and fail-open

```mermaid
sequenceDiagram
    autonumber
    participant EP as entrypoint<br/>agentbox/config/entrypoint-unified.sh
    participant CLI as agent-identity.js main<br/>agentbox/management-api/lib/agent-identity.js:175
    participant LM as loadOrMint<br/>agentbox/management-api/lib/agent-identity.js:107
    participant SHELL as supervised programs and tmux windows

    EP->>CLI: node agent-identity.js mint
    alt argv[2] not 'mint'
        CLI-->>EP: stderr "agent-identity: unknown command" then exit 1
    else mint
        CLI->>LM: loadOrMint()
        alt identity derived
            LM-->>CLI: {did, pubkey, multikey, persisted, keyPath}
            CLI-->>EP: stdout export AGENTBOX_AGENT_DID / AGENTBOX_AGENT_PUBKEY / AGENTBOX_AGENT_DID_MULTIKEY
            CLI-->>EP: stderr "agent-identity: minted|loaded <did> (persisted=<bool>, keyfile=<path>)"
            EP->>SHELL: eval the export lines
            CLI-->>EP: exit 0
        else loadOrMint returned null
            LM-->>CLI: null
            CLI-->>EP: stderr "agent-identity: could not derive a did:nostr (fail-open — caller keeps did:nostr:local)" (agent-identity.js:184)
            CLI-->>EP: exit 1 with NO export lines
            EP->>SHELL: shell keeps its ${VAR:-did:nostr:local} fallback
            Note over EP,SHELL: DIVERGENCE INGRESS-identity "Unsigned pod-signing fallback / did:nostr:local placeholder"<br/>The fail-open is deliberate and in-code — a degraded boot runs the whole container under a non-sovereign placeholder DID with no hard stop. see AB-11.13
        end
    end
```

## AB-11.4 URN kind table — scope, content addressing and resolvable surface

```mermaid
flowchart TB
    K["uris.js:87 KINDS Object.freeze — 19 kinds"]
    K --> POD["pod / envelope / credential / mandate / receipt<br/>ownerScope=true scopeRequired=true contentAddressed=true<br/>surface: pods"]
    K --> ACT["activity / event / decision<br/>ownerScope=true scopeRequired=true contentAddressed=true<br/>surface: agent-events"]
    K --> BEAD["bead<br/>ownerScope=true scopeRequired=true contentAddressed=true<br/>surface: beads"]
    K --> DSET["dataset<br/>ownerScope=true scopeRequired=true contentAddressed=false<br/>surface: memory"]
    K --> OPT["memory / thing / agent<br/>ownerScope=true scopeRequired=FALSE contentAddressed=false<br/>surfaces: memory / things / agents"]
    K --> UNS["mcp / skill / adr / prd / ddd / meta<br/>ownerScope=false scopeRequired=false contentAddressed=false<br/>surfaces: things / skills / docs / meta"]
    POD --> R["resolveCanonical -> base + /v1/uri/{urn}?surface={resolvableSurface}<br/>uris.js:239"]
    ACT --> R
    BEAD --> R
    DSET --> R
    OPT --> R
    UNS --> R
    K -.-> RE["URN_RE = ^urn:agentbox:([a-z]+):([^:]+(?::[^:]+)?)$<br/>PUBKEY_HEX_RE = ^[0-9a-f]{64}$<br/>DID_NOSTR_RE = ^did:nostr:([0-9a-f]{64})$"]
    R -.-> N1["INVARIANT agentbox/CLAUDE.md: every durable identifier is minted through uris.js mint. Ad-hoc format!() or template-literal URNs are prohibited."]
    OPT -.-> N2["scopeRequired=false is the ADR-048 optional-scope form: thing and memory may mint unscoped (urn:agentbox:thing:mcp-foo) OR owner-scoped.<br/>The WS6 elevation path MUST supply the owner pubkey so the thing proposal can cross to urn:visionclaw:kg:<pubkey> through BC20 — bc20-provenance-bridge.js drops an unscoped thing. see AB-17"]
BEAD -.-> N3["DIVERGENCE ADR-2025 PROTOCOL-registry URN-crossing row: JS supports<br/>agent/activity/thing/bead plus option-dependent memory elevation. The Rust side carries a<br/>narrower closed map. No versioned supported-kind agreement and no explicit unmapped outcome<br/>exists yet."]
```

## AB-11.5 uris.mint — branch tree and content addressing

```mermaid
sequenceDiagram
    autonumber
    participant C as caller (any surface)
    participant M as mint<br/>agentbox/management-api/lib/uris.js:157
    participant CA as _contentAddress<br/>agentbox/management-api/lib/uris.js:281
    participant SS as _stableStringify<br/>agentbox/management-api/lib/uris.js:292
    participant NP as _normalisePubkey<br/>agentbox/management-api/lib/uris.js:203

    C->>M: mint({kind, pubkey|npub, payload, localId})
    alt kind not in KINDS
        M-->>C: throw UnknownUriKind (lists every valid kind)
    end
    alt spec.contentAddressed
        alt payload === undefined
            M-->>C: throw MalformedUri "content-addressed kind requires payload"
        else
            M->>CA: _contentAddress(payload)
            CA->>SS: _stableStringify(payload) — recursive, Object.keys sorted
            SS-->>CA: deterministic JSON string
            CA->>CA: crypto.createHash('sha256').update(canon, 'utf8').digest('hex')
            CA-->>M: local = sha256-12-<first 12 hex>
        end
    else localId supplied
        M->>M: local = _slug(localId) — [^A-Za-z0-9._-] to underscore, sliced to 96 chars (uris.js:300)
    else neither
        M-->>C: throw MalformedUri "kind requires localId"
    end
    alt spec.ownerScope
        alt no pubkey and no npub
            alt spec.scopeRequired === false
                M-->>C: urn:agentbox:<kind>:<local> — unscoped form
            else
                M-->>C: throw MalformedUri "kind requires pubkey scope"
            end
        else supplied
            M->>NP: _normalisePubkey(pubkey || npub)
            Note over NP: accepts 64-hex as-is / strips did:nostr: prefix / best-effort bech32 npub1 decode
            alt normalised
                NP-->>M: 64-char lowercase hex
                M-->>C: urn:agentbox:<kind>:<pubkeyhex>:<local>
            else unrecognised
                NP-->>M: null
                M-->>C: throw MalformedUri "bad pubkey: <supplied>"
            end
        end
    else not owner-scoped
        M-->>C: urn:agentbox:<kind>:<local>
    end
    Note over M,SS: DOC-DRIFT PROTOCOL-registry content-address row — the registry requires "same input bytes, twelve lowercase digest hex characters, explicit serialisation".<br/>_contentAddress hashes a JS stableStringify string as utf8, and its own comment at uris.js:281<br/>says "deterministic enough beats exactly RFC 8785 because we are producing a name, not a<br/>signature input". VisionClaw content_address hashes bytes. Byte-parity is asserted nowhere.
```

## AB-11.6 parse, isCanonical and resolveCanonical

```mermaid
sequenceDiagram
    autonumber
    participant C as caller
    participant P as parse<br/>agentbox/management-api/lib/uris.js:261
    participant IC as isCanonical<br/>agentbox/management-api/lib/uris.js:277
    participant RC as resolveCanonical<br/>agentbox/management-api/lib/uris.js:239

    C->>P: parse(uri)
    alt DID_NOSTR_RE matches
        P-->>C: {scheme did, method nostr, pubkey}
    else URN_RE matches
        P->>P: rest.split(':')
        alt parts.length === 1
            P-->>C: {scheme urn, kind, pubkey null, local}
        else
            P-->>C: {scheme urn, kind, pubkey parts[0], local parts.slice(1).join(':')}
        end
    else neither
        P-->>C: null
    end
    C->>IC: isCanonical(uri)
    IC-->>C: DID_NOSTR_RE.test(uri) || URN_RE.test(uri)
    C->>RC: resolveCanonical(uri, {managementApiBase, podBase})
    alt uri is a did:nostr
        alt podBase missing
            RC-->>C: null
        else
            RC-->>C: <podBase>/.well-known/did.json
        end
    else uri is a urn with a known kind
        RC-->>C: <managementApiBase>/v1/uri/<encodeURIComponent(uri)>?surface=<spec.resolvableSurface>
        Note over RC: every agentbox URI routes through management-api so auth, content negotiation and CORS live in one place (uris.js:239 comment)
    else unknown kind or non-matching
        RC-->>C: null
    end
```

## AB-11.7 POST /v1/mandate — issue a revocable delegated grant

```mermaid
sequenceDiagram
    autonumber
    participant OP as operator / caller
    participant RT as mandateRoutes<br/>agentbox/management-api/routes/mandate.js:285
    participant CS as createSignedMandate<br/>agentbox/management-api/routes/mandate.js:191
    participant CM as createMandate<br/>agentbox/management-api/lib/mandate.js:99
    participant U as uris.mint<br/>agentbox/management-api/lib/uris.js:157
    participant SM as signMandate<br/>agentbox/management-api/lib/mandate.js:163
    participant SG as _loadSigner<br/>agentbox/management-api/routes/mandate.js:97
    participant REG as registry.json<br/>agentbox/management-api/routes/mandate.js:52
    participant ACL as pod WAC .acl

OP->>RT: POST /v1/mandate {issuer, agent, container, modes,<br/>expiresAt}
    RT->>CS: createSignedMandate(args)
CS->>CM: createMandate({issuer, agent, container, modes,<br/>issuedAt, expiresAt})
CM->>CM: normalisePubkey(issuer) then normalisePubkey(agent) —<br/>did:nostr:<hex> or bare 64-hex
    alt either identity unparseable
        CM-->>RT: throw MandateError "bad issuer|agent identity"
        RT-->>OP: 400 {error mandate, message} (routes/mandate.js:380)
    end
CM->>CM: normaliseContainer(container) then<br/>normaliseModes(modes)
Note over CM: ALLOWED_MODES = Read Write Append Control<br/>(mandate.js:39). DEFAULT_MODES = Read Write Append<br/>(mandate.js:40) — Control is never a default.
    alt expiresAt not null and (not integer or <= issuedAt)
CM-->>RT: throw MandateError "expiresAt must be a Unix-seconds<br/>integer after issuedAt, or null"
    end
    CM->>U: mint({kind mandate, pubkey issuerHex, payload record})
Note over U: kind mandate is<br/>ownerScope+scopeRequired+contentAddressed — the URN is scoped to<br/>the ISSUER and content-addresses the record. see AB-11.4
    U-->>CM: urn:agentbox:mandate:<issuerHex>:sha256-12-<12hex>
    CM-->>CS: {urn, record}
    CS->>SG: _loadSigner(manifest, logger)
    alt signer available
        CS->>SM: signMandate(record, signer)
SM->>SM: build kind 30078 with tags d=<urn> p=<agentHex><br/>t=agent-mandate expiration=<expires_at>
Note over SM: MANDATE_EVENT_KIND = 30078 (mandate.js:36) —<br/>NIP-33 parameterised-replaceable. The (pubkey, kind, d-tag)<br/>triple IS the revocation mechanism. Reuses nostr-bridge<br/>AGENT_STATE kind, not a new primitive.
        SM-->>CS: signed event
    else no signer resolvable
        CS-->>CS: registry write proceeds unsigned
Note over CS,REG: routes/mandate.js:152 — the registry write is<br/>never blocked purely by a missing relay signer
    end
CS->>REG: reg[_registryKey(agent, container)] = entry then<br/>_saveRegistry (routes/mandate.js:222,65)
    RT-->>OP: 201 {urn, record, event} (routes/mandate.js:377)
    OP->>ACL: PUT mandateToAclTurtle(record) to <container>.acl
Note over ACL: mandate.js:137 emits acl:Authorization with<br/>acl:agent <did:nostr:hex> acl:accessTo + acl:default <container><br/>acl:mode acl:Read, acl:Write …<br/>Mirrors the owner-ACL shape solid-pod-rs writes at provision<br/>time.
```

## AB-11.8 Mandate revoke and list

```mermaid
sequenceDiagram
    autonumber
    participant OP as operator
    participant RV as POST /v1/mandate/revoke<br/>agentbox/management-api/routes/mandate.js:388
    participant RM as revokeMandate<br/>agentbox/management-api/routes/mandate.js:238
    participant REG as registry.json<br/>agentbox/management-api/routes/mandate.js:52
    participant RELAY as nostr relay (see AB-13)
    participant LS as GET /v1/mandate<br/>agentbox/management-api/routes/mandate.js:436

    OP->>RV: POST /v1/mandate/revoke {urn} or {agent, container}
    alt neither urn nor (agent and container)
        RV-->>OP: 400 {error validation, message "provide urn, or both agent and container"} (routes/mandate.js:420)
    end
    RV->>RM: revokeMandate(args)
    RM->>REG: _registryKey(agent, container) lookup (routes/mandate.js:249,76)
    alt no matching entry
        RM-->>RV: null
        RV-->>OP: 404 {error not-found, message "no matching mandate in the registry"} (routes/mandate.js:429)
    else found
        RM->>RM: re-sign the SAME d tag with record.revoked = true
        Note over RM,RELAY: NIP-33 replaceable semantics — kind 30078 with an identical (pubkey, kind, d=<urn>) triple REPLACES the grant at every relay. There is no delete.
        RM->>RELAY: publish replacement event
        RM->>REG: mark the registry entry revoked then _saveRegistry
        RM-->>OP: 200 result (routes/mandate.js:430)
    end
    OP->>LS: GET /v1/mandate
    Note over LS: OPERATOR-ONLY (routes/mandate.js:434 "the mandate registry enumerates every delegation") — the listing is an authority-disclosure surface, not public
    LS-->>OP: 200 {mandates, count} (routes/mandate.js:457)
```

## AB-11.9 Mandate lifecycle

```mermaid
stateDiagram-v2
    [*] --> Requested
    Requested --> Minted : createMandate mandate.js:99 mints urn:agentbox:mandate:issuerHex:sha256-12
    Minted --> SignedActive : signMandate mandate.js:163 kind 30078 d=urn p=agentHex expiration
    Minted --> UnsignedRegistered : no signer resolvable — registry write still proceeds
    UnsignedRegistered --> SignedActive : signer becomes available and the grant is re-published
    SignedActive --> Expired : isMandateActive mandate.js:191 now >= expires_at
    SignedActive --> Revoked : revokeMandate re-signs the same d tag with revoked true
    SignedActive --> Superseded : a new kind-30078 event on the same pubkey+kind+d triple replaces it
    Superseded --> SignedActive
    Expired --> [*]
    Revoked --> [*]
    note right of SignedActive
        isMandateActive checks ONLY revoked and expires_at.
        Signature authenticity is a SEPARATE concern — mandate.js:191 doc
        requires verifying the signed event with nostr-tools before trusting
        the embedded record. recordFromSignedMandate (mandate.js:209) is
        structural validation only and explicitly does NOT verify Schnorr.
    end note
    note right of UnsignedRegistered
        DIVERGENCE — an entry can sit in registry.json with no signed
        replaceable event behind it, so the local registry and the relay
        can disagree about what is granted. routes/mandate.js:152.
    end note
```

## AB-11.10 Authority gate — ACSP request and human-signed decision

```mermaid
sequenceDiagram
    autonumber
    participant SK as caller (skill or action)
    participant G as buildAuthorityGate.guard<br/>agentbox/management-api/lib/authority.js:136
    participant CL as classifyAction<br/>agentbox/management-api/lib/authority.js:101
    participant TB as loadClassificationTable<br/>agentbox/management-api/lib/authority.js:73
    participant ACS as agent-control-surface<br/>agentbox/management-api/lib/agent-control-surface.js
    participant FORUM as forum operator (owns the decision loop)
    participant VE as verifyEvent (nostr-tools)

    SK->>G: guard(actionClass, ctx)
    G->>CL: classifyAction(actionClass, {table, frontmatter})
    alt SKILL.md frontmatter carries a valid authority_class
        CL-->>G: frontmatter.authority_class
    else table has the actionClass
        CL->>TB: table.classes[actionClass]
        TB-->>CL: recoverable | zero-tolerance
        CL-->>G: that class
    else neither
        CL-->>G: 'escalation-required' (authority.js:49 ESCALATION_REQUIRED)
        Note over CL,G: INVARIANT — unknown actions default to escalation-required, never to recoverable. AUTHORITY_CLASSES = recoverable, zero-tolerance (authority.js:47).
    end
    alt class is recoverable
        G-->>SK: allow
    else zero-tolerance or escalation-required
        alt deps.awaitDecision not wired
            G-->>SK: DENY (fail-closed)
            Note over SK,G: INVARIANT authority.js:136 — without an injected<br/>consumer a zero-tolerance action has no way to receive an<br/>approval, so the gate must deny, never invent one
        else consumer wired
            G->>ACS: publishActionRequest(unsigned) -> acs.publishPanelEvent(bridge, signer, unsigned)
            Note over G,ACS: ACTION_REQUEST_KIND = 31402 — we PRODUCE the request. We NEVER build a 31403 response, that is the forum's to sign (authority.js:51-52).
            ACS-->>G: signedRequest (id used to match the response)
            G->>FORUM: kind 31402 ActionRequest over the relay
            loop awaitDecision(signedRequest, {timeoutMs}) — defaultTimeoutMs 120000 (authority.js:139)
                FORUM-->>G: signed kind 31403 ActionResponse, or null on timeout
            end
            alt no response before timeout
                G-->>SK: DENY
            else response received
                G->>VE: verifyEvent(responseEvent)
                alt nostr-tools not loadable
                    VE-->>G: false — fail-closed, treated as unverified (authority.js:136 verifier default)
                    G-->>SK: DENY
                else signature invalid
                    VE-->>G: false
                    G-->>SK: DENY
                else verified
                    G->>G: readOutcome(responseEvent, requestEvent) — must reference our request by e-tag or matching content case_id
                    alt outcome approve
                        G-->>SK: allow
                    else reject or defer or unreadable
                        G-->>SK: DENY
                    end
                end
            end
        end
    end
```

## AB-11.11 Authority consumer — response matching and decided cache

```mermaid
sequenceDiagram
    autonumber
    participant RELAY as relay subscription (see AB-13)
    participant AC as buildAuthorityConsumer<br/>agentbox/management-api/lib/authority-consumer.js:141
    participant KR as _keysForRequest / _keysForResponse<br/>agentbox/management-api/lib/authority-consumer.js:62,73
    participant PC as _parseContent / _tagVal<br/>agentbox/management-api/lib/authority-consumer.js:49,55
    participant CACHE as decided cache (bounded)
    participant GATE as authority gate awaitDecision<br/>agentbox/management-api/lib/authority.js:136
    participant BR as buildActionResponse<br/>agentbox/management-api/lib/authority-consumer.js:101

    RELAY->>AC: EVENT kind 31403 ActionResponse
    Note over RELAY,AC: ACTION_RESPONSE_KIND = 31403 — this module CONSUMES only (authority-consumer.js:45)
    AC->>PC: _parseContent(raw) then _tagVal(event, 'e') and case_id
    PC-->>AC: outcome fields plus the referenced request key
    AC->>KR: _keysForResponse(responseEvent)
    KR-->>AC: candidate match keys
    alt key matches a pending request from _keysForRequest(signedRequest)
        AC->>CACHE: record decided (cap DECIDED_CACHE_MAX = 512, authority-consumer.js:47)
        AC-->>GATE: resolve awaitDecision with the signed 31403
    else no pending request matches
        AC->>AC: drop — an unsolicited or already-decided response
    end
    alt no response within DEFAULT_TIMEOUT_MS = 120000 (authority-consumer.js:46)
        AC-->>GATE: null — the gate denies. see AB-11.10
    end
    Note over BR: buildActionResponse (authority-consumer.js:101) exists for TEST wiring and for surfaces that legitimately sign a decision. The authority GATE never calls it — see the producer/consumer split in AB-11.10.
Note over AC,CACHE: DIVERGENCE GOVERNANCE-capabilities item 1 — this ACSP loop is one of<br/>several per-path guards. There is no single policy decision point (legacy ADR-059 unbuilt) and<br/>no canonical replayable record (legacy ADR-057 unbuilt). see AB-14
```

## AB-11.12 Capability scope — effect and trust classification

```mermaid
classDiagram
    class CapabilityScope {
        <<agentbox/management-api/lib/capability-scope.js>>
        +register(capability)
        +dispose()
    }
    class ServiceRegistry {
        <<agentbox/management-api/lib/capability-scope.js:320 export>>
    }
    class EffectType {
        <<frozen list capability-scope.js:28>>
        tool
        prompt
        listener
        timer
        health
        projection
    }
    class TrustClass {
        <<frozen list capability-scope.js:33>>
        pure
        secrets
        subprocess
        writes
        network
    }
    class CapabilityError {
        <<capability-scope.js>>
        +string code
    }
    class DuplicateCapabilityIdentity {
        <<capability-scope.js>>
    }
    CapabilityScope --> EffectType : every capability declares one of six effect types
    CapabilityScope --> TrustClass : and one of five trust classes
    CapabilityScope --> ServiceRegistry : resolves shared services
    CapabilityScope --> CapabilityError : throws on malformed or unknown declarations
    CapabilityError <|-- DuplicateCapabilityIdentity
    note for CapabilityScope "DEFAULT_DISPOSE_TIMEOUT_MS = 5000 (capability-scope.js:37) bounds teardown of listeners and timers so a scope cannot leak a live effect past disposal."
    note for TrustClass "The trust class is the narrowing axis a granted mandate or authority decision applies to — secrets / subprocess / writes / network are the classes that carry real blast radius. pure carries none."
note for EffectType "DIVERGENCE GOVERNANCE-capabilities: this classification is declarative.<br/>Nothing forces a code-mode sub-call, MCP plugin tool, consultant action or background job<br/>through a scope — the acknowledged bypasses. see AB-14 and AB-15"
```

## AB-11.13 pod-signer — signing a pod write as the agent's own did:nostr

```mermaid
sequenceDiagram
    autonumber
    participant BOOT as management-api boot<br/>agentbox/management-api/server.js
    participant PS as buildPodNip98<br/>agentbox/management-api/lib/pod-signer.js:42
    participant MF as agentbox.toml<br/>[integrations.solid_pod_rs]
    participant ENV as process.env
    participant NB as nostr-bridge loadSigner / buildNip98Header
    participant ADP as pods adapter (ADR-005 pods slot)
    participant POD as solid-pod-rs

    BOOT->>PS: buildPodNip98(manifest, deps)
    PS->>MF: manifest.integrations.solid_pod_rs.sign_requests
    alt sign_requests falsy
        PS-->>BOOT: return null
        Note over PS,ADP: INVARIANT pod-signer.js:16-17 — default (unsigned)<br/>behaviour stays byte-identical. Enabling the flag is the ONLY<br/>behavioural change. Compare ADR-2020 byte-identical-when-off, see AB-15
    else enabled
        PS->>ENV: AGENTBOX_STACK then AGENTBOX_PROFILE then integ.sign_stack
        alt no stack resolves
            PS->>BOOT: deps.onError("sign_requests is on but no stack resolved")
            PS-->>BOOT: return null — but requireSigned still rides with the config<br/>(adapters/index.js:63,74-77), so the adapter FAILS CLOSED
        else stack resolved
            PS-->>BOOT: async nip98(method, url, body)
        end
    end
    ADP->>PS: nip98('PUT', url, body) on every pod request
    PS->>PS: getSigner() — lazy, cached, one-shot loadFailed latch
    alt signer not yet loaded
        PS->>NB: loadSigner(stack, signerOpts)
        alt throws (key undecryptable / stack missing)
            NB-->>PS: error
            PS->>BOOT: deps.onError(err) — fired ONCE (loadFailed latch)
            PS-->>ADP: null header
            alt sign_requests ON (requireSigned)
                ADP--xADP: throw SigningUnavailable — no bytes reach the pod<br/>_solid-http-base.js:87-90
            else flag off
                ADP->>POD: request goes out UNSIGNED — the pre-signing baseline
            end
            Note over ADP,POD: RESOLVED ADR-2064: the fail-OPEN is gone.<br/>A null header no longer means "go out unsigned":<br/>pod-signer.js:12-22 makes null mean only "no<br/>originator could be built" and hands the outcome<br/>to the adapter, which throws SigningUnavailable<br/>per request when sign_requests is on<br/>(_solid-http-base.js:84-91,104-106). Unsigned is<br/>reachable ONLY with the flag off, where it is the<br/>byte-identical pre-signing baseline.
        end
    end
    alt signer available
        PS->>NB: buildNip98Header(signer, method, url, {body})
        NB-->>PS: Authorization: Nostr base64(kind-27235) — body sha256 in the payload tag
        PS-->>ADP: header
        ADP->>POD: request signed under the agent's OWN did:nostr
        POD-->>ADP: 2xx — WAC checks acl:agent <did:nostr:hex> from the mandate. see AB-11.7
    end
    Note over PS,NB: nostr-bridge is vendored into lib/ at build time<br/>(flake buildPhaseExtra) — require('./nostr-bridge') first,<br/>falling back to '../../mcp/servers/nostr-bridge'<br/>which only resolves in a source checkout.
```

## AB-11.14 agent-event-auth — proving source_urn instead of trusting it

```mermaid
sequenceDiagram
    autonumber
    participant CALLER as any caller
    participant RT as POST /v1/agent-events/emit<br/>agentbox/management-api/routes/agent-events.js
    participant AEA as verifyAgentEventRequest<br/>agentbox/management-api/lib/agent-event-auth.js:46
    participant ENV as AGENTBOX_AGENT_EVENT_AUTH
    participant NB as NostrBridge.verifyNip98<br/>agentbox/mcp/servers/nostr-bridge.js
    participant RSU as reconcileSourceUrn<br/>agentbox/management-api/lib/agent-event-auth.js:89

    CALLER->>RT: POST /v1/agent-events/emit {source_urn, ...}
    RT->>AEA: verifyAgentEventRequest(request, deps)
    AEA->>ENV: resolvePolicy(env) (agent-event-auth.js:27)
    alt policy 'off' (DEFAULT_POLICY, agent-event-auth.js:25)
        AEA-->>RT: {ok true, did null, pubkey null}
Note over AEA,RT: DIVERGENCE — DEFAULT is off, so out of the box ANY caller may assert<br/>source_urn and attribute an action to an agent that did not perform it<br/>(agent-event-auth.js:5-12). agentbox.toml sets agent_event_auth = "nip98" for the<br/>sovereign-mesh posture, but the code default is permissive.
    else policy neither off nor nip98
        AEA-->>RT: {ok false, status 500, error "unknown AGENTBOX_AGENT_EVENT_AUTH policy"}
    else policy 'nip98'
        AEA->>AEA: authHeaderOf(request) — headers.authorization or headers.Authorization
        alt no Authorization header
            AEA-->>RT: {ok false, status 401, error "NIP-98 Authorization header required"}
        else header present
            AEA->>AEA: pathOnly = request.url.split('?')[0]
            Note over AEA,NB: the originator strips the query string from the signed u tag, so the comparison is path-only — verifyNip98 accepts urlTag.endsWith(url). see AB-10.4
            AEA->>NB: verifyNip98(authHeader, 'POST', pathOnly)
            alt verify throws
                NB-->>AEA: exception
                AEA-->>RT: {ok false, status 401, error "NIP-98 verification failed: <msg>"}
            else result.valid false
                NB-->>AEA: {valid false, error}
                AEA-->>RT: {ok false, status 401, error}
            else valid
                NB-->>AEA: {valid true, pubkey}
                AEA-->>RT: {ok true, did "did:nostr:<pubkey>", pubkey}
            end
        end
    end
    RT->>RSU: reconcileSourceUrn(claimed source_urn, verifiedDid)
    alt verifiedDid null (policy off)
        RSU-->>RT: {ok true} — no reconciliation performed
    else claimed differs from verifiedDid
        RSU-->>RT: {ok false, status 403, error "source_urn '<claimed>' does not match authenticated identity '<verified>'"}
        RT-->>CALLER: 403
    else matches or unset
        RSU-->>RT: {ok true}
        RT-->>CALLER: 202 — source_urn is provable, derived from the SIGNATURE. see AB-17
    end
```

## AB-11.15 URI resolution surface — /v1/uri and .well-known

```mermaid
sequenceDiagram
    autonumber
    participant C as client
    participant UR as uri-resolver<br/>agentbox/management-api/routes/uri-resolver.js
    participant K as uris.parse + KINDS<br/>agentbox/management-api/lib/uris.js:261,87
    participant POD as pod base (solid-pod-rs)
    participant AE as /v1/agent-events
    participant WK as well-known x402<br/>agentbox/management-api/routes/well-known.js:64

    C->>UR: GET /v1/uri/<urn>?surface=<s>
    UR->>K: parse(urn) then kind lookup
    alt urn unparseable or unknown to the registry
        UR-->>C: 404 {error, urn} (uri-resolver.js:61)
    else kind not in KINDS
        UR-->>C: 404 {error unknown-kind, kind, urn} (uri-resolver.js:76)
    else kind is an agent identity
        UR-->>C: 307 Location <podBase>/.well-known/did.json (uri-resolver.js:68)
        C->>POD: follow redirect — the DID document lives on the POD, not on management-api
    else owner-scoped kind with a pubkey
        UR-->>C: 307 Location <podBase>/agents/<pubkey>/<kind>/<local> (uri-resolver.js:93)
    else owner-scoped kind with NO pubkey
        UR-->>C: 404 {error not-resolvable, reason "kind requires owner scope", urn} (uri-resolver.js:95)
    else activity or event or decision
        UR-->>C: 307 Location /v1/agent-events?id=<urn> (uri-resolver.js:101)
        C->>AE: follow redirect
    end
Note over UR,C: contract at uri-resolver.js:12-16 — 200 or 307 means resolvable, 404 means<br/>unknown name, 410 Gone means once-resolvable and now removed. Resolvability is BEST-EFFORT<br/>(agentbox/CLAUDE.md) — a valid URN is a name first and a locator second.
    C->>WK: GET /.well-known/x402.json
    alt manifest.payments.broadcast.well_known !== true
        WK-->>C: 404 {error not-found, message "x402 well-known manifest is not enabled on this agentbox"}
    else enabled
        WK-->>C: 200 {x402Version 1, description, operator "did:nostr:"+AGENTBOX_PUBKEY, generatedAt, accepts, routes} Cache-Control public max-age=3600
Note over WK: cached once at BOOT in a closure — generatedAt is plugin-registration time, not<br/>per-request. No auth: public discovery per the x402 spec, and the auth-skip is hand-added to<br/>the server.js onRequest hook (well-known.js:16-19) rather than being declarative. see AB-15
    end
```

## AB-11.16 Key custody at boot — what exists versus what ADR-2027 proposes

```mermaid
sequenceDiagram
    autonumber
    participant BOOT as entrypoint boot
    participant AI as agent-identity loadOrMint<br/>agentbox/management-api/lib/agent-identity.js:107
    participant KF as profile key file 0600
    participant BR as nostr-pod-bridge<br/>AGENTBOX_BRIDGE_SK_FILE default /run/secrets/nostr.key
    participant PX as nip98-proxy secrets (see AB-10.3)
    participant ADR as ADR-2027 custody register<br/>agentbox/docs/SECURITY-profiles.md

    rect rgb(240,255,240)
    Note over BOOT,KF: IMPLEMENTED — what the code actually does
    BOOT->>AI: mint or load the per-profile agent key
    AI->>KF: writeFileSync mode 0o600 then chmodSync 0o600 (agent-identity.js:141-142)
    KF-->>AI: readable only by this uid
    BOOT->>BR: load bridge identity from AGENTBOX_BRIDGE_SK_FILE, legacy environment fallback remains
    end
    rect rgb(255,240,240)
    Note over ADR,PX: PROPOSED AND NOT ACTIVE — ADR-2027 requirements with no code behind them
    ADR-->>PX: expiry, request scope and auditable use for the break-glass bearer — the proxy compares a configured token and returns a sentinel identity with NO expiry and NO scope check
    ADR-->>PX: restart-invalidation and multi-instance policy for NIP98_PROXY_SESSION_SECRET — today it defaults to per-boot crypto.randomBytes
    ADR-->>BR: per-consumer key split — the governance publisher still shares the operator/server identity (legacy ADR-040 D3, relay allowlist entry agentbox.toml:148)
    ADR-->>AI: rotation cadence, revocation procedure, named custodian, maximum response window — every row is UNCONFIRMED
    end
Note over ADR: DIVERGENCE SECURITY-profiles provisional custody register 2026-09-04 — seven<br/>credential roles are identified as ROLES TO ASSIGN, not accepted custodians. "No cadence is<br/>invented here." No dated failure/recovery receipt exists for any row.
Note over BOOT,KF: DIVERGENCE — a 0600 file is a same-uid boundary only. Every agentbox process<br/>runs as devuser, so co-resident code can read the agent key, the bridge key and the AoE daemon<br/>token alike. Per-process isolation is named future work in GOVERNANCE-capabilities item 5. see<br/>AB-16
```
