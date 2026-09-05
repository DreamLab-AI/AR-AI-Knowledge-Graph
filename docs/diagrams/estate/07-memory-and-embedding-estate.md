---
id: ES-07
title: RuVector memory and embedding estate
area: estate
governing:
  - agentbox/docs/LEARNING-memory.md
  - docs/DATA-authority-erasure.md
adrs: [ADR-2014, ADR-2015, ADR-2016]
sources:
  - agentbox/mcp/servers/ruvector-mcp.cjs
  - agentbox/scripts/ruvector-recall-harness.mjs
  - agentbox/scripts/ruvector-sona-feeder.mjs
  - agentbox/scripts/ruvector-aggregate-sweep.mjs
  - agentbox/scripts/ruvector-pattern-distill.mjs
  - agentbox/agentbox.toml
  - agentbox/tests/contract/ruvector-gates.contract.spec.js
  - agentbox/docs/adr/ADR-2014-memory-mcp-only-fail-closed.md
  - src/handlers/memory_flash_handler.rs
  - src/main.rs
  - src/actors/agent_monitor_actor.rs
verified_commit: b00c28a0d
---
## ES-07.1 Every RuVector client and the one shared embedder
```mermaid
flowchart TB
    subgraph clients["Clients — all writes MUST go through the MCP surface"]
        MCP["agentbox/mcp/servers/ruvector-mcp.cjs<br/>memory_store / memory_retrieve / memory_list / memory_search<br/>lines 241,255,267,278"]
        HOOKS["agentbox hooks + skills<br/>route via the same MCP server"]
        SWEEP["ruvector-aggregate-sweep.mjs"]
        DISTILL["ruvector-pattern-distill.mjs"]
        SONA["ruvector-sona-feeder.mjs"]
        HARNESS["ruvector-recall-harness.mjs"]
        VCMF["VisionClaw memory_flash_handler<br/>src/handlers/memory_flash_handler.rs:41<br/>OBSERVER ONLY — broadcasts access events"]
        AMA["agent_monitor_actor.rs:417-421<br/>narrates RuVector Memory Specialist activity"]
    end
    subgraph store["ruvector-postgres"]
        PG["host=ruvector-postgres port=5432<br/>dbname=ruvector user=ruvector<br/>$RUVECTOR_PG_CONNINFO — ruvector-mcp.cjs:48-57"]
        HNSW["HNSW index over 384-dim vectors"]
    end
    subgraph embed["Shared embedder"]
        XI["xinference /v1/embeddings<br/>$XINFERENCE_ENDPOINT default http://xinference:9997<br/>ruvector-mcp.cjs:91"]
        MODEL["bge-small-en-v1.5<br/>EMBEDDING_DIM = 384 — ruvector-mcp.cjs:92-93"]
    end

    MCP --> PG
    HOOKS --> MCP
    SWEEP --> PG
    DISTILL --> PG
    SONA --> PG
    HARNESS --> PG
    PG --> HNSW
    MCP -- "client-side embed before write" --> XI
    XI --> MODEL

    INV1["INVARIANT — access is mcp__claude-flow__memory_* ONLY.<br/>The claude-flow CLI and raw SQL INSERT bypass the embedding<br/>pipeline, so rows written that way are INVISIBLE to HNSW search."]
    INV2["INVARIANT — ruvector-mcp.cjs FAILS CLOSED with no sql.js<br/>fallback: cannot reach ruvector-postgres is FATAL<br/>(ruvector-mcp.cjs:157)"]
    DIV1["DIVERGENCE — VisionClaw has NO RuVector write client. Despite<br/>agent-facing narration, the only Rust touchpoint is the<br/>memory-flash WS broadcast (see ES-07.6). There is no<br/>RuVectorAdapter type in src/ or crates/ (verified by grep)."]

    MCP --> INV1
    PG --> INV2
    VCMF --> DIV1
```

## ES-07.2 memory_store — embed-then-write, fail-closed when the embedder is down
```mermaid
sequenceDiagram
    autonumber
    participant A as agent
    participant M as ruvector-mcp.cjs<br/>memory_store:241
    participant X as xinference :9997<br/>bge-small-en-v1.5
    participant P as ruvector-postgres :5432
    participant H as HNSW index

    A->>M: memory_store{namespace, key, value, ttl}
    Note over M: entryId = `${WRITE_SOURCE_TYPE}:${namespace}:${key}`<br/>ruvector-mcp.cjs:129 — namespace defaults to "default"
    M->>X: POST /v1/embeddings (client-side embed)
    alt xinference reachable
        X-->>M: 384-dim vector
        M->>P: upsert row + vector
        P->>H: index insert
        H-->>P: ok
        P-->>M: stored
        M-->>A: ok
    else xinference unavailable
        X--xM: connect error
        Note over M,P: ADR-2014 FAIL-CLOSED — the store is REJECTED.<br/>A row without an embedding would be permanently<br/>invisible to semantic search (ruvector-mcp.cjs:165)
        alt RUVECTOR_EMBED_REPAIR=true
            M->>P: accept as a repairable PENDING write
            P-->>M: pending, awaiting later repair
            M-->>A: accepted pending
        else default
            M-->>A: REJECT — store refused until xinference returns
        end
    end
    Note over A,H: EMBED CAP — bge-small embeds only the first ~512 tokens<br/>(~2,500 chars) of a value. The tail is invisible to search.<br/>Keep values under ~2,000 chars and front-load the facts.<br/>Retrieve-by-key still returns the WHOLE value.
```

## ES-07.3 memory_search — HNSW semantic path with an ILIKE degradation
```mermaid
sequenceDiagram
    autonumber
    participant A as agent
    participant M as ruvector-mcp.cjs<br/>memory_search:278
    participant X as xinference :9997
    participant P as ruvector-postgres
    participant H as HNSW index

    A->>M: memory_search{query, namespace, limit}
    M->>X: embed(query)
    alt embedder healthy
        X-->>M: 384-dim query vector
        M->>P: namespace-scoped pgvector search
        P->>H: HNSW top-k
        H-->>P: candidates
        P-->>M: ranked rows
        M-->>A: semantic results (~100ms typical)
    else embedder down
        X--xM: error
        Note over M,P: DEGRADED — search falls back to ILIKE<br/>(substring match, no semantics) — ruvector-mcp.cjs:165
        M->>P: ILIKE scan
        P-->>M: literal matches only
        M-->>A: degraded results
    end
    opt reconnect probe
        M->>X: getEmbedding("reconnect probe")
        Note over M,X: recall-harness mirrors this probe at<br/>ruvector-recall-harness.mjs:173-174 — one probe<br/>flips xinferenceOk back to true
    end
    Note over A,H: namespace "*" performs a global cross-namespace search.<br/>AVOID memory_hybrid_search on large namespaces — it<br/>materialises the whole namespace (~72s on ruvnet-kb).
```

## ES-07.4 Cross-agent pattern propagation — store in one session, search in another
```mermaid
sequenceDiagram
    autonumber
    participant S1 as session A agent
    participant M1 as ruvector-mcp (session A)
    participant P as ruvector-postgres<br/>shared sidecar
    participant M2 as ruvector-mcp (session B)
    participant S2 as session B agent

    rect rgb(230,240,230)
    Note over S1,P: WRITE — after a successful task
    S1->>M1: memory_store{namespace "patterns", key, value}
    M1->>P: embed + upsert (see ES-07.2)
    P-->>M1: stored
    end
    rect rgb(230,235,245)
    Note over S2,P: READ — a DIFFERENT session, later
    S2->>M2: memory_search{query "task keywords", namespace "patterns"}
    M2->>P: HNSW top-k
    P-->>M2: session A's row ranked by semantic similarity
    M2-->>S2: the pattern propagates with no direct agent-to-agent link
    end
    Note over S1,S2: INVARIANT — the shared Postgres sidecar IS the<br/>coordination channel. This is why CLI/raw-SQL writes are<br/>prohibited: an unembedded row never reaches session B.
    Note over P: DIVERGENCE — file-based auto-memory<br/>(~/.claude/projects/.../memory/, MEMORY.md) is INVISIBLE<br/>to this path and to every other agent in the mesh.
```

## ES-07.5 Namespace map and the protected reference corpus
```mermaid
flowchart LR
    subgraph ctx["Context namespaces — search before priority-aware work"]
        N1["personal-context<br/>identity, team, goals, comms style<br/>index key personal-context-portfolio-index"]
        N2["project-state<br/>current focus, priority order, decisions<br/>index key project-state-current-focus"]
        N3["patterns<br/>what worked — written after success"]
    end
    subgraph learn["Learning-loop namespaces"]
        N4["memory-learning-aggregates<br/>mapped onto the memory slot, no new slot<br/>agentbox.toml:408"]
        N5["code-harness-lessons<br/>trajectory sink — agentbox.toml:557"]
    end
    subgraph prot["Protected"]
        N6["ruvnet-kb — reference corpus, INGEST-ONLY writes<br/>appended to RUVECTOR_PROTECTED_NAMESPACES<br/>so agents cannot mutate it (agentbox.toml:610)"]
    end
    SCOPE["Pubkey scoping — NIP-98 callers are scoped to their own<br/>pubkey namespace (AGENTBOX_X_ONLY_PUBKEY_HEX /<br/>AGENTBOX_PUBKEY). A session cannot read another session's<br/>per-project namespace. agentbox.toml:333-346"]
    LOW["DIVERGENCE — ruvnet-kb and knowledge-* namespaces have<br/>LOW scoped recall (R@10 ~9-11%) until candidate-bounded<br/>hybrid search lands."]

    ctx --> SCOPE
    learn --> SCOPE
    prot --> SCOPE
    N6 --> LOW
```

## ES-07.6 VisionClaw memory-flash — RuVector access events on the WebSocket
```mermaid
sequenceDiagram
    autonumber
    participant AG as agent or tool
    participant H as handle_memory_flash<br/>src/handlers/memory_flash_handler.rs:41
    participant R as route table<br/>configure_routes:133
    participant WS as all WebSocket clients

    Note over R: POST /api/memory-flash and the batch sibling are wired<br/>at src/main.rs:1128 via configure_memory_flash_routes
    AG->>H: POST /api/memory-flash {key, namespace, action}
    H->>H: namespace = body.namespace.unwrap_or_default()<br/>memory_flash_handler.rs:45
    H->>WS: broadcast MemoryFlashEvent{key, namespace, action}
    WS-->>H: fan-out complete
    H-->>AG: 200
    opt batch
        AG->>H: handle_memory_flash_batch — memory_flash_handler.rs:103
        loop each event in the batch
            H->>WS: broadcast with per-event namespace unwrap_or_default<br/>memory_flash_handler.rs:118
        end
    end
    Note over AG,WS: INVARIANT — this path is PURELY OBSERVATIONAL. It<br/>renders memory activity in the graph. It never reads or<br/>writes RuVector itself, so it cannot be a coordination<br/>channel and carries no embedding.
```

## ES-07.7 Recall gate — the band that must hold before and after any retrieval change
```mermaid
flowchart TB
    RUN["./agentbox.sh ruvector recall<br/>ruvector-recall-harness.mjs"]
    SELF["self-recall@10 — 200 rows<br/>the row's OWN stored embedding is the query<br/>SELF_NS_MIN_ROWS = 50 eligible rows per namespace<br/>harness:15-17,80"]
    TRUE["true-recall@10 — 120 rows vs a forced exact<br/>brute-force scan as ground truth<br/>TRUE_TOTAL = 120, TRUE_NS_MIN_ROWS = 20<br/>harness:18-20,82-83"]
    GATE["PASS iff median(self) >= 175/200<br/>AND median(true) >= 102/120 AND exactOk<br/>harness:32, evaluated at :228-236"]
    NSB["Per-namespace self-recall breakdown is surfaced<br/>but NOT gated — harness:34"]
    D3["DIVERGENCE D3 (LEARNING-memory.md) — the harness gates<br/>true at >= 102/120 (harness:32) while agentbox/CLAUDE.md<br/>and the reference doc quote >= 107/120.<br/>CODE IS AUTHORITATIVE for the gate; the prose band is a<br/>tighter operational target. self >= 175/200 agrees in both."]

    RUN --> SELF
    RUN --> TRUE
    SELF --> GATE
    TRUE --> GATE
    GATE --> NSB
    GATE --> D3
```

## ES-07.8 Index law — the rebuild that must never be concurrent
```mermaid
stateDiagram-v2
    [*] --> Steady
    Steady --> BulkChurn
    BulkChurn --> Degraded
    Degraded --> RebuildNonConcurrent
    RebuildNonConcurrent --> Steady
    Degraded --> ForbiddenPath
    ForbiddenPath --> Corrupt
    Corrupt --> [*]

    note right of BulkChurn
        Any bulk ingest or delete.
    end note
    note right of Degraded
        HNSW degrades SILENTLY under bulk churn.
        Recall drops with no error surfaced.
    end note
    note right of RebuildNonConcurrent
        REQUIRED — non-concurrent rebuild,
        m=16, ef_construction=128, ~5 min.
        Then re-run the ES-07.7 recall gate.
    end note
    note right of ForbiddenPath
        FORBIDDEN — CREATE INDEX CONCURRENTLY
        on the ruvector HNSW access method.
        Verified double-insertion.
    end note
```

## ES-07.9 Learning loop — the gates that are off, and why
```mermaid
flowchart TB
    subgraph feeder["ruvector-sona-feeder.mjs"]
        F1["streams judged trajectories into<br/>ruvector_sona_learn under fixed 384-dim<br/>scope agentbox_memory"]
    end
    subgraph gates["agentbox.toml gates"]
        G1["sona_learn = OFF<br/>sona_apply = OFF<br/>agentbox.toml:429-431"]
        G2["attention_rerank = OFF<br/>agentbox.toml:430"]
        G3["pattern_distillation = true<br/>ENABLED 2026-07-21, 13 patterns live<br/>provenance judge:trajectory<br/>agentbox.toml:430"]
        G4["allow_namespace_repair = false<br/>agentbox.toml:442"]
        G5["allow_pattern_graduation = false RESERVED<br/>agentbox.toml:448"]
    end
    D1["DIVERGENCE D1 — SONA is INERT. The prebuilt<br/>@ruvector/sona@0.1.5 NAPI binary hardcodes<br/>embedding_dim = 256, so 384-dim learns return<br/>status:learned but accumulate NOTHING (verified live).<br/>Both gates stay off until a 384-dim-capable binary."]
    D1B["attention_rerank is OFF BY MEASUREMENT, not caution —<br/>on an L2-normalised corpus the attention blend is a<br/>mathematical identity (max diff 4e-7)."]
    D2["DIVERGENCE D2 — aggregate-count drift. agentbox.toml:415<br/>cites 78 aggregates >=20 samples (2026-08-31); the<br/>reference doc records 12 from the 2026-07-21 sweep.<br/>The toml is the running config and the newer number."]
    D4["DIVERGENCE D4 — agentbox/README.md:314 still lists<br/>feed_retrieval / feed_routing as open gates (false);<br/>the running toml has feed_retrieval = true since 2026-08-31."]
    D5["DIVERGENCE D5 — pod-sync deletion has NO reverse<br/>tombstone. deleteAgentMemory() in the Pod does not revoke<br/>the RuVector-held agent memory: the embedding row persists<br/>and stays semantically searchable. Largest erasure hole.<br/>No point-in-time RuVector backup exists, so there is no<br/>cross-store consistent restore, RPO or RTO."]

    F1 --> G1
    G1 --> D1
    G2 --> D1B
    G3 --> D2
    G3 --> D4
    G4 --> D5
    G5 --> D5
```
