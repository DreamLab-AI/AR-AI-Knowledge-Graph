---
id: VC-23
title: Identifier taxonomy — typed URN, did:nostr, sha256-12, federation crossing, wire node-id
area: visionclaw
governing:
  - docs/IDENTIFIER-taxonomy.md
adrs: [ADR-2021, ADR-2022, ADR-2023, ADR-2024, ADR-2025, ADR-2070, ADR-2072]
sources:
  - src/uri/mod.rs
  - src/utils/binary_protocol.rs
  - src/types/user_context.rs
  - src/services/nostr_identity_verifier.rs
  - src/services/ontology_mutation_service.rs
  - src/handlers/enrichment_proposals_handler.rs
  - src/domain/broker/precedent_registry.rs
  - src/actors/presence_actor.rs
  - src/actors/elevation_actor.rs
  - src/actors/client_filter.rs
  - src/adapters/oxigraph_graph_repository.rs
  - agentbox/management-api/lib/bc20-provenance-bridge.js
  - agentbox/management-api/middleware/linked-data/surfaces/s04-did.js
  - agentbox/mcp/servers/lib/memory-tools.js
verified_commit: 7a20db228
---

## VC-23.1 Typed URN kind taxonomy — src/uri/mod.rs
```mermaid
classDiagram
    class Kind {
      <<enum>>
      Concept
      Kg
      Bead
      Execution
      Group
      Room
      Avatar
    }
    class ParsedUri {
      <<enum>>
    }
    class DidNostr {
      +String pubkey
    }
    class Concept {
      +String domain
      +String slug
    }
    class Kg {
      +String pubkey
      +String address
    }
    class Bead {
      +String pubkey
      +String address
    }
    class Execution {
      +String address
    }
    class Group {
      +String team
    }
    class Room {
      +String address
    }
    class Avatar {
      +String pubkey
    }
    class LegacyNgm {
      +String sub
    }
    class UrnCrossing {
      +String agentbox_urn
      +String visionclaw_id
      +Option~String~ owner_did
    }
    ParsedUri <|-- DidNostr
    ParsedUri <|-- Concept
    ParsedUri <|-- Kg
    ParsedUri <|-- Bead
    ParsedUri <|-- Execution
    ParsedUri <|-- Group
    ParsedUri <|-- Room
    ParsedUri <|-- Avatar
    ParsedUri <|-- LegacyNgm
    Kind ..> ParsedUri : mint and parse
    UrnCrossing ..> ParsedUri : visionclaw_id parses as

    note for Kind "src/uri/mod.rs:53-69 -- no Agent variant (ADR-2022), identity is did:nostr. Grammar<br/>and mint/parse citations per kind: see VC-23.10"
```

## VC-23.2 Fail-closed mint path — kg() (ADR-2021)
```mermaid
sequenceDiagram
    autonumber
    participant Caller
    participant U as uri::kg<br/>src/uri/mod.rs:242
    participant PK as is_pubkey_hex<br/>src/uri/mod.rs:136
    participant CA as content_address<br/>src/uri/mod.rs:179
    Caller->>U: kg(owner_pubkey, content)
    U->>PK: is_pubkey_hex(owner_pubkey)
    alt owner_pubkey is not 64-char lowercase hex
        PK-->>U: false
        U-->>Caller: Err(UriError::InvalidPubkey) src/uri/mod.rs:244
    else owner_pubkey is valid hex
        PK-->>U: true
        U->>CA: content_address(content)
        CA->>CA: Sha256 digest, first 6 bytes to 12 hex chars, src/uri/mod.rs:180-186
        CA-->>U: sha256-12-HEX
        U-->>Caller: Ok urn:visionclaw:kg:PUBKEY:sha256-12-HEX, src/uri/mod.rs:246-250
    end
    Note over Caller,U: INVARIANT mint rejects what parse tolerates -- ADR-2021
    rect rgb(255,230,204)
    Note over U: DIVERGENCE ADR-2021 closeout -- emit_proposal_provenance formats urn:visionclaw:execution ad hoc, bypassing<br/>uri::execution and sha256-12 addressing, src/services/ontology_mutation_service.rs:181-193
    Note over U: DIVERGENCE oxigraph_graph_repository.rs:88,97 format! urn:ngm:node / urn:ngm:edge inline instead of calling ngm::node_iri / ngm::edge_iri, src/uri/mod.rs:328,343
    end
```

## VC-23.3 parse (strict) vs parse_dual (lenient legacy resolve)
```mermaid
sequenceDiagram
    autonumber
    participant MintOrValidate as mint / strict resolve
    participant P as parse<br/>src/uri/mod.rs:520
    participant ResolveSurface as resolve / lookup surface
    participant PD as parse_dual<br/>src/uri/mod.rs:630
    MintOrValidate->>P: parse(input)
    alt input has did:nostr: prefix
        P->>P: is_pubkey_hex check on the did:nostr tail, src/uri/mod.rs:521-527
        alt pubkey invalid
            P-->>MintOrValidate: Err(InvalidPubkey)
        else pubkey valid
            P-->>MintOrValidate: Ok(ParsedUri::DidNostr)
        end
    else input has urn:visionclaw: prefix
        P->>P: split kind token, src/uri/mod.rs:535-537
        alt kind token unrecognised
            P-->>MintOrValidate: Err(UnknownKind) src/uri/mod.rs:538-539
        else kind recognised
            P->>P: per-kind structural validation, src/uri/mod.rs:541-614
            P-->>MintOrValidate: Ok(ParsedUri kind variant)
        end
    else any other namespace, including urn:ngm
        P-->>MintOrValidate: Err(NotVisionclaw) src/uri/mod.rs:530-532
    end
    ResolveSurface->>PD: parse_dual(input)
    PD->>P: parse(input)
    alt parse succeeds
        P-->>PD: Ok(ParsedUri)
        PD-->>ResolveSurface: Ok(ParsedUri) unchanged, src/uri/mod.rs:631-632
    else parse returns NotVisionclaw and input has urn:ngm: prefix
        PD-->>ResolveSurface: Ok(ParsedUri::LegacyNgm) src/uri/mod.rs:634-641
    else parse returns NotVisionclaw and no urn:ngm prefix
        PD-->>ResolveSurface: Err(NotVisionclaw) src/uri/mod.rs:642
    else parse returns any other UriError
        PD-->>ResolveSurface: Err propagated unchanged, src/uri/mod.rs:644
    end
    Note over MintOrValidate,PD: INVARIANT parse rejects urn:ngm, parse_dual accepts it -- mint calls parse, resolve calls parse_dual, ADR-2021
    Note over PD: DIVERGENCE open governance item -- legacy ADR-100/105/050/053/063 identifier prose is superseded by this code<br/>where it diverges, per IDENTIFIER-taxonomy.md Known divergences
```

## VC-23.4 did:nostr identity — hex canonical, npub display-only (ADR-2022)
```mermaid
sequenceDiagram
    autonumber
    participant Caller
    participant DN as did_nostr<br/>src/uri/mod.rs:220
    participant VDC as verify_did_matches_challenge<br/>src/services/nostr_identity_verifier.rs:84
    participant NIV as NostrIdentityVerifier<br/>src/services/nostr_identity_verifier.rs:34
    participant UC as UserContext<br/>src/types/user_context.rs:15
    Caller->>DN: did_nostr(pubkey)
    alt pubkey is not 64-char lowercase hex
        DN-->>Caller: Err(InvalidPubkey) src/uri/mod.rs:221-223
    else pubkey valid
        DN-->>Caller: Ok did:nostr:PUBKEY src/uri/mod.rs:224
    end
    Caller->>VDC: verify_did_matches_challenge(payload_did, challenge, verifier)
    VDC->>VDC: uri::parse(payload_did) -- Gate 1, src/services/nostr_identity_verifier.rs:91-92
    alt payload_did is not a canonical ParsedUri::DidNostr
        VDC-->>Caller: Err(RoomError::InvalidDid) src/services/nostr_identity_verifier.rs:93-97
    else payload_did parses cleanly
        VDC->>NIV: verify_signed_challenge(challenge) -- Gate 2, src/services/nostr_identity_verifier.rs:100-102
        NIV->>NIV: schnorr verify (nonce||timestamp_us) against event pubkey, src/services/nostr_identity_verifier.rs:52-58
        alt schnorr verify fails
            NIV-->>VDC: Err(RoomError::InvalidDid) src/services/nostr_identity_verifier.rs:58-64
        else schnorr verify succeeds
            NIV-->>VDC: Ok(Did::parse(did:nostr:event.pubkey)) src/services/nostr_identity_verifier.rs:65
        end
        VDC->>VDC: compare verified.pubkey_hex to payload_pubkey, src/services/nostr_identity_verifier.rs:105
        alt hex mismatch
            VDC-->>Caller: Err(RoomError::InvalidDid) src/services/nostr_identity_verifier.rs:106-112
        else hex matches
            VDC-->>Caller: Ok(verified Did) src/services/nostr_identity_verifier.rs:114
        end
    end
    Note over UC: display encoding -- npub1... is UI-only, user_id field src/types/user_context.rs:16-17 - hex kept separately in pubkey field src/types/user_context.rs:20
    Note over UC: DIVERGENCE npub-vs-hex mixing risk -- user_context.rs:16 documents npub as the primary user identifier, no automatic hex conversion enforced at that boundary
    Note over NIV: DOC-DRIFT docs/IDENTIFIER-taxonomy.md says agentbox s04-did.js still emits the invalid 2019<br/>SchnorrSecp256k1VerificationKey2019 shape - code emits the DIDNostr/Multikey form,<br/>agentbox/management-api/middleware/linked-data/surfaces/s04-did.js:79-91
```

## VC-23.5 sha256-12 content addressing — VisionClaw and agentbox (ADR-2023)
```mermaid
sequenceDiagram
    autonumber
    participant Caller
    participant VC as content_address<br/>src/uri/mod.rs:179
    participant AB as sha12 (bc20-provenance-bridge)<br/>agentbox/management-api/lib/bc20-provenance-bridge.js:132
    participant MT as sha12 (memory-tools)<br/>agentbox/mcp/servers/lib/memory-tools.js:60
    Caller->>VC: content_address(bytes)
    VC->>VC: Sha256::digest(input), take 6 bytes, 2 hex chars each, src/uri/mod.rs:180-186
    VC-->>Caller: sha256-12-HEX (CONTENT_ADDR_PREFIX + 12 lowercase hex), src/uri/mod.rs:187
    par agentbox BC20 side
        Caller->>AB: sha12(input)
        AB->>AB: sha256 hex digest, slice(0,12), agentbox/management-api/lib/bc20-provenance-bridge.js:132-135
        AB-->>Caller: sha256-12-HEX
    and agentbox memory-tools side
        Caller->>MT: sha12(v)
        MT->>MT: sha256 hex digest, slice(0,12), agentbox/mcp/servers/lib/memory-tools.js:60
        MT-->>Caller: bare 12 hex chars, no sha256-12- prefix added by this helper
    end
    Note over VC,AB: INVARIANT byte-identical -- both truncate to the first 6 bytes (12 hex chars) of the same SHA-256 digest, ADR-2023
    Note over VC: verified fixture sha256-12-b94d27b9934d for input hello world, src/uri/mod.rs:883-894
    Note over MT: DIVERGENCE memory-tools.js:60 returns the bare 12-hex digest with no sha256-12- prefix, unlike CONTENT_ADDR_PREFIX-enforced VisionClaw and bc20-provenance-bridge sides
```

## VC-23.6 Federation crossing — toVisionclaw / cross_from_agentbox closed map (ADR-2025)
```mermaid
sequenceDiagram
    autonumber
    participant CallerAB as agentbox caller
    participant JS as toVisionclaw<br/>agentbox/management-api/lib/bc20-provenance-bridge.js:158
    participant CallerVC as VisionClaw ingest caller
    participant RS as cross_from_agentbox<br/>src/uri/mod.rs:809
    rect rgb(230,230,250)
    Note over CallerAB,JS: federation trust boundary -- agentbox to VisionClaw
    CallerAB->>JS: toVisionclaw(agentboxUrn, opts)
    alt kind is agent
        JS-->>CallerAB: did:nostr:PUBKEY, bc20-provenance-bridge.js:169-176
    else kind is execution
        JS-->>CallerAB: urn:visionclaw:execution:sha256-12-HEX, bc20-provenance-bridge.js:188-191
    else kind is bead, structural passthrough
        JS-->>CallerAB: urn:visionclaw:bead:PUBKEY:sha256-12-LOCAL, bc20-provenance-bridge.js:191-207
    else kind is kg
        JS-->>CallerAB: urn:visionclaw:kg:PUBKEY:sha256-12-HEX, bc20-provenance-bridge.js:207-214
    else kind is memory/concept and opts.domain+opts.slug present
        JS-->>CallerAB: urn:visionclaw:concept:DOMAIN:SLUG, bc20-provenance-bridge.js:214-221
    else kind is memory/concept but elevation target missing, or kind unmapped
        JS-->>CallerAB: null, onDrop logged (never silent), bc20-provenance-bridge.js:180-185,192-196
    end
    end
    rect rgb(220,245,220)
    Note over CallerVC,RS: VisionClaw side closed map -- narrower than agentbox
    CallerVC->>RS: cross_from_agentbox(agentbox_urn)
    alt already-converged did:nostr passthrough
        RS-->>CallerVC: Some(UrnCrossing), src/uri/mod.rs:811-819
    else kind is agent
        RS-->>CallerVC: Some(did:nostr:PUBKEY), src/uri/mod.rs:843
    else kind is activity
        RS-->>CallerVC: Some(urn:visionclaw:execution:sha256-12-HEX), src/uri/mod.rs:844
    else kind is thing
        RS-->>CallerVC: Some(urn:visionclaw:kg:PUBKEY:sha256-12-HEX), src/uri/mod.rs:845
    else kind is bead
        RS-->>CallerVC: Some(urn:visionclaw:bead:PUBKEY:sha256-12-HEX), src/uri/mod.rs:846-856
    else kind is memory (crosses false, deliberate) or any not-federated kind
        RS-->>CallerVC: None -- artefact refusal at src/uri/mod.rs:832-835, closed-map default arm :857
    end
    end
    Note over JS,RS: RESOLVED ADR-2072 -- cross_from_agentbox now has a bead arm (src/uri/mod.rs:846-856) that crosses<br/>structurally via bead_with_address (:284), PRESERVING the existing sha256-12 address rather than re-hashing,<br/>matching bc20-provenance-bridge.js:191-206. agentbox ADR-2061 holds the cross-repo parity test. The kind map<br/>itself is DERIVED from agentbox/schema/federation-kinds.json, never transcribed (src/uri/mod.rs:828-832).
    Note over CallerVC,RS: INVARIANT callers record the raw string plus an unmapped marker, never a synthetic ID, on None -- ADR-2025, src/uri/mod.rs:828-831
```

## VC-23.7 Wire node-id u32 bit layout and overflow policy (ADR-2024)
```mermaid
flowchart TD
    subgraph WireU32["u32 wire node id -- src/utils/binary_protocol.rs:17-29"]
        B31["bit 31 -- AGENT_NODE_FLAG -- 0x80000000 -- line 15"]
        B30["bit 30 -- KNOWLEDGE_NODE_FLAG -- 0x40000000 -- line 16"]
        BOnt["bits 26-28 -- ONTOLOGY_TYPE_MASK -- 0x1C000000 -- line 19"]
        BId["bits 0-25 -- NODE_ID_MASK -- 0x03FFFFFF -- line 26 -- range 0 to 67108863"]
    end
    BOnt --> OC["0x04000000 Class -- line 20"]
    BOnt --> OI["0x08000000 Individual -- line 21"]
    BOnt --> OP["0x10000000 Property -- line 22"]

    Enc["enforce_wire_id_bounds -- binary_protocol.rs:167-188"]
    Enc --> Assert["debug_assert node_id within NODE_ID_MASK -- fails fast in debug -- lines 168-175"]
    Enc --> Remap["remap_wire_id masks to 26 bits -- lines 199-201"]
    Remap --> BId
    Enc --> LogCheck{"overflowed?"}
    LogCheck -->|yes| Log["error! NODE ID OVERFLOW, class and both ids logged -- lines 176-186, ADR-2024"]
    LogCheck -->|no| NoLog["masked id returned unchanged"]

    Setters["all six wire-id branches call enforce_wire_id_bounds -- agent line 204, knowledge line 208, ontology-class line 258, ontology-individual line 262, ontology-property line 266, untyped fallback line 445"] --> Enc

    Drift["RESOLVED ADR-2070: IDENTIFIER-taxonomy.md now describes the code - every encoder branch routes through enforce_wire_id_bounds (binary_protocol.rs:167-188), masking via remap_wire_id (:199-201) and logging error! on overflow in ALL builds, untyped fallback included (:445). debug_assert! (:168-175) is a development aid, not the bound. An over-range id is still masked and therefore aliases, but never silently."]
```

## VC-23.8 Durable urn:visionclaw vs ephemeral wire node-id
```mermaid
flowchart LR
    Counter["NEXT_NODE_ID atomic counter"] --> BareId["bare u32 id, 0..67108863 -- NODE_ID_MASK, binary_protocol.rs:26"]
    BareId --> Flagged["flagged wire id -- set_agent_flag / set_knowledge_flag / set_ontology_*_flag, binary_protocol.rs:203-267"]
    Flagged --> Wire["Protocol V3 wire frame, 52 bytes per node, binary_protocol.rs:37-51"]
    Wire --> Decode["decode_node_data_v3 -- get_actual_node_id strips flags, binary_protocol.rs:227-229 and :730"]
    Decode --> Durable["urn:visionclaw:kg:PUBKEY:sha256-12-HEX -- graph store, src/uri/mod.rs:242-251"]
    Durable -.-> NoReverse["no code path derives a wire id FROM a urn -- render-plane id is allocated independently per session"]
    Inv["INVARIANT: wire id maps TO a durable urn:visionclaw:kg:*, never the reverse"]
    SeeNote["Note: see VC-14 for the wire protocol"]
```

## VC-23.9 vc: CURIE, unminted legacy forms, and the ADR-050 owner-scoped path
```mermaid
flowchart TD
    Concept["urn:visionclaw:concept:DOMAIN:SLUG -- durable subject, src/uri/mod.rs:229-239"] --> Curie["vc:referencedBy -- RDF predicate CURIE only, src/actors/elevation_actor.rs:510"]
    Curie -.-> NotSubject["vc:DOMAIN/SLUG subject form -- NOT independently minted anywhere in src/ or crates/"]
    QS["vc:qualityScore -- provenance noted in a source comment only, src/actors/client_filter.rs:61-62 -- expanded qualityScore key is consumed, the CURIE itself is not emitted as a literal there"]
    LegacyClass["urn:ngm:class:SLUG -- typed legacy mint ngm::class_iri, defined crates/visionclaw-domain/src/uri.rs:31 and re-exported src/uri/mod.rs:351 -- legacy scheme, not urn:visionclaw:concept"]
    OwnerScoped["visionclaw:owner:NPUB/kg/... -- legacy ADR-050 form"] -.-> Absent["grep across src/ and crates/ -- zero occurrences -- superseded by the hex-scoped urn:visionclaw:kg:PUBKEY:ADDRESS grammar"]

    DivA["DOC-CORRECTED 2026-09-05: vc:{domain}/{slug} is only an RDF predicate CURIE, never an independently minted subject<br/>IDENTIFIER-taxonomy.md now reconciles its section-1 kind table with this - no row mints a vc: CURIE and the durable<br/>concept subject is urn:visionclaw:concept:domain:slug"]
    DivB["DIVERGENCE: owner-scoped visionclaw:owner:{npub}/kg/... (legacy ADR-050) is not emitted anywhere in src/ or crates/"]
    DriftX["RESOLVED ADR-2095 (2026-09-05): the class scheme is now a typed constructor paired with a parser<br/>CLASS_PREFIX and class_iri and parse_class_iri live in crates/visionclaw-domain/src/uri.rs:15-56 -- the domain crate, because<br/>visionclaw-adapters mints class IRIs and is upstream of the server crate -- and re-export from ngm at src/uri/mod.rs:351<br/>All five raw format! mints are routed through it: elevation_actor.rs:329 and :514, oxigraph_ontology_repository.rs:174 and :1598 and :1619<br/>Emitted strings are byte-identical -- the pre-existing literal assertion at elevation_actor.rs:1409 still passes"]
```

## VC-23.10 Per-kind URN grammar, mint/parse sites and live emission
```mermaid
flowchart TD
    subgraph ID["Identity kinds"]
        direction TB
        K1["DidNostr<br/>did:nostr:HEXPUBKEY<br/>mint src/uri/mod.rs:220-225<br/>parse :521-527<br/>kind() returns None :467-470"]
        K9["Avatar<br/>urn:visionclaw:avatar:HEXPUBKEY<br/>1:1 with the avatar DID<br/>mint :297-302 / parse :579-586"]
    end
    subgraph OWNED["Owner-scoped content-addressed kinds"]
        direction TB
        K3["Kg<br/>urn:visionclaw:kg:HEXPUBKEY:sha256-12-HEX<br/>mint :242-251<br/>kg_with_address :255-262<br/>parse :530-551"]
        K4["Bead<br/>urn:visionclaw:bead:HEXPUBKEY:sha256-12-HEX<br/>mint :265-274 / parse :530-551"]
    end
    subgraph UNSCOPED["Unscoped content-addressed kinds"]
        direction TB
        K5["Execution<br/>urn:visionclaw:execution:sha256-12-HEX<br/>owner travels in owner_did, not the URN<br/>mint :277-279 / parse :552-559<br/>live emission enrichment_proposals_handler.rs:189"]
        K7["Room<br/>urn:visionclaw:room:sha256-12-HEX<br/>unscoped XR presence room<br/>mint :292-294 / parse :571-578"]
    end
    subgraph SHARED["Shared and team-scoped kinds"]
        direction TB
        K2["Concept<br/>urn:visionclaw:concept:DOMAIN:SLUG<br/>mint :229-239 / parse :515-529<br/>live emission precedent_registry.rs:88"]
        K6["Group<br/>urn:visionclaw:group:TEAM-hash-members<br/>mint :282-288 / parse :560-570"]
    end
    subgraph LEGACY["Legacy exception"]
        direction TB
        K8["LegacyNgm<br/>urn:ngm:SUB opaque<br/>accepted only by parse_dual :603-619<br/>rejected by parse :503-505<br/>typed ngm module :320-404 -- ADR-2021"]
    end
    XR["Room has no production emission site -- only RoomId::parse under cfg(test), presence_actor.rs:920-921"]
    K7 -.-> XR
    CROSS["UrnCrossing -- federation boundary record, src/uri/mod.rs:772-779 -- see VC-23.6"]
    ID --> CROSS
    OWNED --> CROSS
    UNSCOPED --> CROSS
```
