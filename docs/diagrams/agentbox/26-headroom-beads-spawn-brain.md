---
id: AB-26
title: Headroom compression, the beads work-DAG, typed spawn and RuvNet grounding
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
  - agentbox/docs/LEARNING-memory.md
adrs: [ADR-2004, ADR-2005, ADR-2020]
sources:
  - agentbox/crates/headroom-napi/src/lib.rs
  - agentbox/crates/headroom-napi/src/types.rs
  - agentbox/crates/headroom-napi/src/smart_crusher.rs
  - agentbox/crates/headroom-napi/src/log_compressor.rs
  - agentbox/crates/headroom-napi/src/diff_compressor.rs
  - agentbox/crates/headroom-napi/src/content_router.rs
  - agentbox/crates/headroom-napi/src/ccr_store.rs
  - agentbox/management-api/lib/headroom.js
  - agentbox/management-api/lib/headroom-mcp-tools.js
  - agentbox/management-api/adapters/beads/local-sqlite.js
  - agentbox/management-api/adapters/beads/external.js
  - agentbox/management-api/adapters/beads/off.js
  - agentbox/mcp/servers/lib/typed-spawn.js
  - agentbox/mcp/servers/substrate-tools.js
  - agentbox/mcp/ruvnet-brain/server.js
  - agentbox/config/hooks/ruvnet-brain-ground.cjs
  - agentbox/tests/contract/beads.contract.spec.js
  - agentbox/agentbox.toml
  - agentbox/mcp/servers/lib/ontology-local.js
verified_commit: 7a20db228
---

## AB-26.1 headroom — lazy native load and slot gating

```mermaid
sequenceDiagram
    autonumber
    participant MW as adapter dispatch middleware<br/>see AB-04
    participant H as headroom.compress<br/>agentbox/management-api/lib/headroom.js:193
    participant LN as _loadNative<br/>agentbox/management-api/lib/headroom.js:40
    participant CFG as _readManifestConfig<br/>agentbox/management-api/lib/headroom.js:56
    participant ADDON as headroom_napi.node<br/>/opt/agentbox/lib/headroom/headroom_napi.node
    participant CR as content_router::detect<br/>agentbox/crates/headroom-napi/src/content_router.rs

    MW->>H: compress(content, slot, opts)
    alt slot == events
        H-->>MW: input UNCHANGED
        Note over H: HARD-CODED regardless of manifest config — the AUDIT TRAIL MUST NEVER BE COMPRESSED<br/>(headroom.js:15-17). agentbox.toml:1560 events = false agrees, but the code does not<br/>trust it
    else slot not enabled in [compression.slots]
        H->>CFG: read the manifest
        Note over CFG: agentbox.toml:1557-1562 — memory true, pods true, events false, beads true, orchestrator<br/>false
        H-->>MW: input UNCHANGED — fail-open
    else compression.enabled false (agentbox.toml:1551)
        H-->>MW: input UNCHANGED — fail-open
    else enabled and slot on
        H->>LN: load the addon on FIRST CALL, not at require() time
        Note over LN: lazy so management-api startup is unaffected when the addon is not installed. Sentinels<br/>— null not yet attempted, false unavailable, object loaded (headroom.js:30-33)
        alt addon absent
            LN-->>H: false
            H-->>MW: input UNCHANGED — fail-open (headroom.js:12-14)
        else loaded
            LN->>ADDON: bind the N-API surface
            H->>CR: detect_content_type(input)
            CR-->>H: json_array | log_output | unified_diff | unknown
            H->>ADDON: the matching compressor — see AB-26.2
            ADDON-->>H: CompressResult with CCR sentinels
            H-->>MW: compressed payload
        end
    end
    Note over H: INVARIANT: fail-open everywhere — when the native addon is absent or compression is off,<br/>compress() returns the input unchanged (headroom.js:12-14)
```

## AB-26.2 The three content-aware compressors and the CCR store

```mermaid
sequenceDiagram
    autonumber
    participant JS as headroom.js<br/>agentbox/management-api/lib/headroom.js
    participant ROUTE as detect_content_type<br/>agentbox/crates/headroom-napi/src/lib.rs:54
    participant SC as smart_crush<br/>agentbox/crates/headroom-napi/src/lib.rs:22
    participant LC as compress_log<br/>agentbox/crates/headroom-napi/src/lib.rs:33
    participant DC as compress_diff<br/>agentbox/crates/headroom-napi/src/lib.rs:44
    participant CCR as ccr_store<br/>agentbox/crates/headroom-napi/src/ccr_store.rs
    participant DB as SQLite CCR store

    JS->>ROUTE: detect_content_type(input)
    Note over ROUTE: regex probes — ISO-8601, syslog, CLF and kernel dmesg timestamp shapes identify<br/>log_output (content_router.rs:6-13)
    alt json_array
        JS->>SC: smart_crush(input, SmartCrushOptions)
        SC->>SC: analyse the schema, preserve ANCHORS and OUTLIERS, sample the rest
        Note over SC: target_ratio default 0.3 in-crate (smart_crusher.rs:11), min_items default 2.<br/>agentbox.toml:1555 sets an aggressive target_ratio = 0.15 — keep about 15 percent
        SC->>CCR: emit a CCR sentinel per DROPPED row
    else log_output
        JS->>LC: compress_log(input, LogCompressOptions)
        LC->>LC: normalise variable runs — NUMBER_RE 2+ digits, UUID_RE — then fold repeats
    else unified_diff
        JS->>DC: compress_diff(input, DiffCompressOptions)
        DC->>DC: sample context lines when the context-to-changes ratio exceeds context_ratio default 3.0<br/>(diff_compressor.rs:6)
        alt input empty
            DC-->>JS: empty CompressResult, original_bytes 0
        end
    end
    CCR->>DB: ccr_store_entry(hash, original) — lib.rs:61
    Note over CCR,DB: BLAKE3 hash prefix, 24 hex chars, identifies the stored content (types.rs:5-12).<br/>Process-global singleton via OnceLock (ccr_store.rs:11-12), DashMap over a rusqlite<br/>Connection
    Note over DB: backend sqlite (memory also supported, redis DEFERRED), ttl_minutes 30, max_entries 1000<br/>with LRU eviction (agentbox.toml:1552-1554)
    JS-->>JS: CompressResult {compressed, original_bytes, ...}
```

## AB-26.3 headroom MCP tools

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant T as headroom-mcp-tools<br/>agentbox/management-api/lib/headroom-mcp-tools.js
    participant H as headroom.js
    participant N as headroom-napi
    participant DB as CCR store

    alt headroom_compress (headroom-mcp-tools.js:65)
        AG->>T: headroom_compress {content, content_type}
        Note over T: content_type enum json_array | log_output | unified_diff | auto
        T->>H: compress / smartCrush / compressLog / compressDiff (headroom.js:355,365,375)
        H->>N: the matching compressor
        N->>DB: originals stored in the CCR cache for later retrieval
        N-->>AG: compressed payload with CCR sentinels
    else headroom_retrieve (headroom-mcp-tools.js:34)
        AG->>T: headroom_retrieve {hash}
        Note over T: hash is the BLAKE3 prefix, 24 hex chars
        T->>H: retrieve(hash) (headroom.js:286)
        H->>N: ccr_retrieve(hash) (lib.rs:70)
        N->>DB: lookup
        alt entry expired or evicted
            DB-->>N: None
            N-->>AG: error — the entry has expired or was evicted
        else present
            DB-->>N: Some(Buffer)
            N-->>AG: decompressed content and size
        end
    else headroom_stats (headroom-mcp-tools.js:53)
        AG->>T: headroom_stats {}
        T->>H: stats() (headroom.js:305)
        H->>N: ccr_stats() (lib.rs:78)
        N-->>AG: CcrStoreStats — entry count, bytes stored, hit and miss counts, hit rate
    end
    Note over H,N: init_compression(CompressionConfig) (lib.rs:87) is called from headroom.js init<br/>(headroom.js:118) with the manifest overrides
```

## AB-26.4 The beads adapter contract

```mermaid
classDiagram
    class BeadsAdapter {
        <<interface>>
        +CONTRACT_VERSION
        +createEpic(opts) Epic
        +createChild(opts) Child
        +claim(id, actor) Bead
        +close(id, outcome) Bead
        +addDependency(childId, blockerId, type) void
        +getReady(filter) List~Bead~
        +show(id) Bead
    }
    class LocalSqliteBeadsAdapter {
        createEpic :78
        createChild :113
        claim :145
        close :168
        addDependency :195
        getReady :236
        show :259
    }
    class ExternalBeadsAdapter {
        addDependency :48
        getReady :52
    }
    class OffBeadsAdapter {
        addDependency :23 throws AdapterDisabled
        getReady :24 throws AdapterDisabled
    }
    class PlaceholderBeadsAdapter {
        getReady :18 throws AdapterDisabled
    }
    BeadsAdapter <|.. LocalSqliteBeadsAdapter
    BeadsAdapter <|.. ExternalBeadsAdapter
    BeadsAdapter <|.. OffBeadsAdapter
    BeadsAdapter <|.. PlaceholderBeadsAdapter
    note for BeadsAdapter "One of the FIVE adapter slots (beads, pods, memory, events, orchestrator — legacy<br/>ADR-005). Every slot resolves to local-*, external or off, and tests/contract must pass<br/>for ALL THREE implementation classes per slot. Never a client-only or standalone-only<br/>feature"
    note for LocalSqliteBeadsAdapter "CONTRACT_VERSIONS.beads is asserted valid semver AND matched against a canonical fixture<br/>value by the contract spec (beads.contract.spec.js:70-79). addDependency/dep-aware<br/>getReady were resurrected in beads 1.1.0"
    note for OffBeadsAdapter "the off class raises AdapterDisabled on EVERY method — asserted by<br/>beads.contract.spec.js:81"
```

## AB-26.5 Work-DAG readiness semantics

```mermaid
stateDiagram-v2
    [*] --> Open : createEpic or createChild
    Open --> Blocked : addDependency(childId, blockerId, "blocks")
    note right of Blocked
        getReady WITHHOLDS childId until EVERY blocker is closed
        (local-sqlite.js:181, :195, :236). Asserted by
        beads.contract.spec.js:141 — "addDependency gates readiness:
        a blocked bead is withheld until its blocker closes".
    end note
    Blocked --> Ready : every blocker closed
    Open --> Ready : no dependencies
    note right of Ready
        getReady returns only UNCLAIMED children (actor IS NULL) —
        beads.contract.spec.js:128. With no filter it returns all
        unclaimed open beads (:205).
    end note
    Ready --> Claimed : claim(id, actor)
    Claimed --> Claimed : re-claim by the SAME actor is a no-op
    note right of Claimed
        claim is IDEMPOTENT for the same actor
        (beads.contract.spec.js:110) and throws a typed AlreadyClaimed
        when ANOTHER actor holds the bead (:183).
    end note
    Claimed --> Closed : close(id, outcome)
    Ready --> Closed : close(id, outcome)
    Closed --> Closed : close is idempotent, returns closed status
    note right of Closed
        close sets status=closed and records the outcome
        (beads.contract.spec.js:120). Closing an already-closed bead
        returns closed status rather than throwing (:198).
        claim then close preserves the ORIGINAL actor (:219).
    end note
    Closed --> [*]
    note right of Open
        Typed failures: show throws NotFound for an unknown id
        (beads.contract.spec.js:176) and createChild throws NotFound
        when parent_id is unknown (:192). createEpic assigns unique
        ids across calls (:213). SLO — createEpic p95 under 200 ms,
        the in-process local-sqlite floor for the 50 req/s SLO (:235).
        Dispatch middleware for this slot is AB-04.
    end note
```

## AB-26.6 typed-spawn — typed, DID-owned recursive spawn

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant ST as substrate-tools spawn_child<br/>agentbox/mcp/servers/substrate-tools.js:108
    participant CTX as createSpawnContext<br/>agentbox/mcp/servers/lib/typed-spawn.js:62
    participant VAL as validateIris<br/>agentbox/mcp/servers/lib/typed-spawn.js:40
    participant ONT as local ontology<br/>agentbox/mcp/servers/lib/ontology-local.js — see AB-25
    participant BEADS as beads adapter<br/>agentbox/management-api/adapters/beads/local-sqlite.js

    AG->>ST: spawn_child {title, skill, input_iris, blocked_by, epic_id, epic_title, owner}
    Note over ST: only `title` is required (substrate-tools.js:120)
    ST->>CTX: createSpawnContext(opts)
    CTX->>CTX: owner = opts.owner or AGENTBOX_REFINE_OPERATOR or did:nostr:jjohare (:64)
    alt no epicId supplied
        CTX->>BEADS: createEpic({title, tags: ["owner:<did>"]})
        Note over CTX,BEADS: OWNERSHIP is an axis distinct from `actor`, which is the CLAIM — who WORKS the bead. The<br/>owner DID rides in tags so getReady (actor IS NULL) still treats children as runnable,<br/>and a worker can claim-and-run WITHOUT taking ownership. This is the<br/>sovereign-ownership-vs-claim separation (:67-71)
    end
    CTX->>VAL: validateIris(ontology, inputIris, "input")
    loop each IRI
        VAL->>ONT: classGet({iri})
        alt unknown
            ONT-->>VAL: error
            VAL-->>AG: throw "input references unknown ontology IRIs: ..."
            Note over VAL: INVARIANT: an unknown IRI is REJECTED BEFORE ANY WORK IS SPAWNED (:15-16, :81)
        else known
            ONT-->>VAL: canonical IRI
        end
    end
    CTX->>BEADS: createChild({title, parent_id: epicId, tags: [owner:<did>, skill:<skill>, in:<iri>...]})
    Note over CTX,BEADS: actor is left NULL so the child stays runnable in getReady until a worker claims it<br/>(:88-89)
    loop each blocker in blockedBy
        CTX->>BEADS: addDependency(child.id, blocker) — a work-DAG edge (:92)
    end
    CTX-->>AG: {beadId, epicId, owner, skill, typedInput, status}
    Note over AG,CTX: prime-agent's rlm() spawns ANONYMOUS, UNTYPED children — the adoptable here is the<br/>CONTRACT prime lacks: a child bead under the parent epic, typed IRIs, and inherited DID<br/>ownership (:7-20)
    Note over CTX: the heavy lifting — retrieval, reasoning, actual agent execution — stays with the<br/>ORCHESTRATOR. This is a thin typing+attribution+ownership skin over a spawn (:21-22).<br/>Dispatch is AB-04
```

## AB-26.7 spawn_ready and spawn_complete

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant ST as substrate-tools<br/>agentbox/mcp/servers/substrate-tools.js
    participant CTX as spawn context<br/>agentbox/mcp/servers/lib/typed-spawn.js
    participant VAL as validateIris<br/>agentbox/mcp/servers/lib/typed-spawn.js:40
    participant BEADS as beads adapter

    alt spawn_ready (substrate-tools.js:125)
        AG->>ST: spawn_ready {epic_id, owner}
        Note over ST: epic_id is REQUIRED
        ST->>CTX: ready() (typed-spawn.js:108)
        CTX->>BEADS: getReady({parent_id: epicId})
        BEADS-->>CTX: only children whose blockers are ALL closed and whose actor IS NULL
        CTX-->>AG: [{beadId, title, actor}]
    else spawn_complete (substrate-tools.js:130)
        AG->>ST: spawn_complete {bead_id, output_iris, outcome}
        ST->>CTX: completeChild(beadId, {outputIris, outcome})
        CTX->>VAL: validateIris(ontology, outputIris, "output")
        alt any unknown output IRI
            VAL-->>AG: throw "output references unknown ontology IRIs: ..."
            Note over VAL: the bead is NOT closed — typing is enforced on the way OUT as well as in (:101-102)
        else all known
            CTX->>BEADS: close(beadId, outcome or "done")
            BEADS-->>CTX: closed
            CTX-->>AG: {beadId, owner, typedOutput, status}
        end
    end
    Note over ST: the substrate-tools server also exposes refine :34, refine_validate :51, refine_rollback<br/>:56, refine_history :61, refine_list :70 and the working-set family ws_note :77, ws_get<br/>:86, ws_list :91, ws_drop :96, ws_revalidate :101 — see AB-22 and AB-25
    Note over ST: PROPOSED ADR-2074: build the ADR-051 distillation tools as a discrete manifest-gated MCP server that holds the<br/>harness signing key, rather than as tools on the fail-open ontology-bridge - none of D2 or D3 exists today, so there is<br/>no distill tool, no job URN kind and no job_urn field. See AB-24
```

## AB-26.8 RuvNet Brain grounding

```mermaid
sequenceDiagram
    autonumber
    participant U as User prompt
    participant CC as Claude Code UserPromptSubmit
    participant HK as ruvnet-brain-ground.cjs<br/>agentbox/config/hooks/ruvnet-brain-ground.cjs:1
    participant MCP as ruvnet-brain server<br/>agentbox/mcp/ruvnet-brain/server.js:198
    participant KB as ruvnet-kb namespace<br/>see AB-20

    U->>CC: prompt text
    CC->>HK: hook JSON on stdin
    HK->>HK: scan for RUVNET_REPOS references (:15-22)
    Note over HK: ruflo, claude-flow, ruvector, rvf, safla, agentdb, agentic-flow, agentic-qe, rulake,<br/>agenticow, sparc, agent-harness-generator, qudag, rvm, ruv-fann, rupixel, synthlang,<br/>dspy.ts, fact, ruview, daa, metaharness, redblue, cve-bench, ruvnet
    HK->>HK: scan for CLASSICAL_SUBS anti-patterns (:24-32)
    Note over HK: pinecone / pgvector / chromadb / weaviate map to ruvector-agentdb — langchain /<br/>llamaindex map to ruflo-agentic-flow — hnswlib maps to @ruvector/rvf
    alt no match
        HK-->>CC: exit 0, NO injection
    else match
        HK-->>CC: exit 0 with additionalContext — a grounding directive to call search_ruvnet BEFORE<br/>asserting, and to redirect a classical substitute to its RuvNet equivalent
        CC->>MCP: search_ruvnet (server.js:205)
        MCP->>KB: semantic search over the reference corpus
        KB-->>MCP: hits
        MCP-->>CC: grounded evidence
        opt status probe
            CC->>MCP: ruvnet_brain_status (server.js:221)
        end
    end
    Note over HK: FAIL-OPEN: any error exits 0 with no injection (:11). Protocol is hook JSON in on stdin,<br/>JSON out on stdout (:9-10)
    Note over KB: DIVERGENCE: ruvnet-kb and the knowledge-* namespaces are LOW-RECALL — scoped R@10 about<br/>9-11 percent — until candidate-bounded hybrid lands. ruvnet-kb is also a PROTECTED<br/>reference corpus with ingest-only writes, and it is capped at about 40 percent of the<br/>recall harness self-recall stratification (see AB-20)
    Note over KB: DIVERGENCE: memory_hybrid_search materialises the whole namespace and measured about 72<br/>s on ruvnet-kb — avoid it on this namespace
```

## AB-26.9 Compression slot topology

```mermaid
flowchart TB
    subgraph man["agentbox.toml [compression] — line 1515"]
        E["enabled = true :1516<br/>headroom-napi crate built, compression active"]
        B["backend = sqlite :1517<br/>sqlite or memory, redis DEFERRED"]
        T["ttl_minutes = 30 :1518"]
        M["max_entries = 1000 :1519<br/>LRU eviction"]
        R["target_ratio = 0.15 :1520<br/>aggressive default, keep about 15 percent"]
    end
    subgraph slots["[compression.slots] — line 1522"]
        S1["memory = true :1523<br/>compress memory search results"]
        S2["pods = true :1524<br/>compress pod writes"]
        S3["events = false :1525<br/>NEVER compress audit trail"]
        S4["beads = true :1526<br/>compress bead payloads"]
        S5["orchestrator = false :1527<br/>skip orchestrator coordination"]
    end
    subgraph adapters["The five adapter slots (legacy ADR-005) — dispatch in AB-04"]
        A1["memory → RuVector, see AB-20"]
        A2["pods → Solid Pod"]
        A3["events → audit trail"]
        A4["beads → work-DAG, see AB-26.4"]
        A5["orchestrator"]
    end
    E --> slots
    S1 --> A1
    S2 --> A2
    S3 --> A3
    S4 --> A4
    S5 --> A5
    B --> CCR["CCR store<br/>agentbox/crates/headroom-napi/src/ccr_store.rs"]
    T --> CCR
    M --> CCR
    R --> SC["smart_crush<br/>agentbox/crates/headroom-napi/src/smart_crusher.rs"]
    subgraph notes["Invariants and drift"]
        direction TB
        N1["INVARIANT: the events slot ALWAYS returns input unchanged regardless of manifest config<br/>— hard-coded in agentbox/management-api/lib/headroom.js:15-17. The manifest value is<br/>belt-and-braces, not the control"]
        N2["The addon is loaded from /opt/agentbox/lib/headroom/headroom_napi.node (headroom.js:28),<br/>built from agentbox/crates/headroom-napi and packaged by<br/>agentbox/lib/headroom-compress.nix. Absent addon = fail-open passthrough, so a disabled<br/>build leaves no runtime trace"]
        N3["Three middleware layers wrap EVERY adapter dispatch in order: observability (ADR-005)<br/>then privacy filter (ADR-008) then JSON-LD encoder (ADR-012). Compression rides inside<br/>that chain — see AB-04"]
        N1 ~~~ N2 ~~~ N3
    end
```
