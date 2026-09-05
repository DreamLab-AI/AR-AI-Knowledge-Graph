---
id: AB-20
title: RuVector memory path — every MCP memory tool end to end
area: agentbox
governing:
  - agentbox/docs/LEARNING-memory.md
adrs: [ADR-2014, ADR-2018, ADR-2019, ADR-2051]
sources:
  - agentbox/mcp/servers/ruvector-mcp.cjs
  - agentbox/mcp/servers/lib/memory-tools.js
  - agentbox/mcp/servers/lib/memory-hybrid.js
  - agentbox/mcp/servers/lib/memory-health.js
  - agentbox/mcp/servers/lib/memory-metadata.js
  - agentbox/mcp/servers/lib/embedding-identity.js
  - agentbox/mcp/servers/lib/ruvector-gates.js
  - agentbox/scripts/ruvector-recall-harness.mjs
  - agentbox/scripts/ruvector-sona-feeder.mjs
  - agentbox/scripts/recall-fixtures/recall-fixture.v1.json
  - agentbox/agentbox.sh
  - agentbox/agentbox.toml
  - agentbox/docs/reference/claude-context/ruvector-memory-state.md
verified_commit: bed6b617d
---

## AB-20.1 Server boot — fail-closed on Postgres, advisory on Xinference

```mermaid
sequenceDiagram
    autonumber
    participant SUP as supervisord / MCP host
    participant SRV as ruvector-mcp.cjs<br/>agentbox/mcp/servers/ruvector-mcp.cjs:1
    participant PG as ruvector-postgres<br/>RUVECTOR_PG_CONNINFO
    participant XI as Xinference bge-small-en-v1.5<br/>XINFERENCE_URL
    participant EI as verifyEmbeddingIdentity<br/>agentbox/mcp/servers/lib/embedding-identity.js
    participant MT as createMemoryTools<br/>agentbox/mcp/servers/lib/memory-tools.js:204

    SUP->>SRV: start
    SRV->>PG: SELECT 1 (:154)
    alt unreachable
        PG--xSRV: error
        SRV-->>SUP: [FATAL] cannot reach ruvector-postgres then process.exit(1) (:158-159)
        Note over SRV: INVARIANT ADR-2014: FAIL-CLOSED. There is NO sql.js fallback — the server replaces<br/>`claude-flow mcp start` precisely so memory routes to ruvector-postgres instead of the<br/>bundled sql.js store (:5-7)
    else connected
        PG-->>SRV: ok
        SRV->>XI: getEmbedding("startup probe") (:161)
        alt unavailable
            XI--xSRV: error
            SRV->>SRV: log WARN — search will use ILIKE fallback, and ADR-2014 fail-closed will REJECT stores<br/>until it returns (:165)
            Note over SRV: set RUVECTOR_EMBED_REPAIR=true to accept repairable PENDING writes instead
        else connected
            XI-->>SRV: 384-dim vector
            SRV->>EI: verifyEmbeddingIdentity(getEmbedding) (:174)
            Note over EI: ADR-2019 closeout — DIMENSION AGREEMENT IS NOT COMPATIBILITY. Probe the live transport,<br/>compute the effective identity fingerprint, compare with the checked-in pin (:167-173)
            alt verdict not ok
                EI-->>SRV: incompatible same-dimension swap
                SRV-->>SUP: [FATAL] then process.exit(1) (:178-179)
                Note over EI: continuing would write vectors into a corpus whose GEOMETRY they do not share, producing<br/>confidently wrong recall with NO error anywhere
            else unpinned or override
                EI-->>SRV: advisory WARN — we refuse a KNOWN-bad identity, we do not invent a pin (:180-181)
            else matches the pin
                EI-->>SRV: INFO fingerprint matches
            end
        end
    end
    SRV->>MT: createMemoryTools({backend: 'external-pg', deps: {pool, getEmbedding, xinfEnsure,<br/>vecToSql, entryId, ...}}) (:204-205)
    Note over MT: the ADR-015 mandated external-pg path — this server injects its pool, embedding<br/>transport, notifier and helpers so the extracted logic behaves byte-for-byte as before<br/>(:201-203)
    Note over SRV: serverInfo is name "claude-flow" (:643) — the server impersonates the claude-flow MCP<br/>identity so tool names stay byte-identical
```

## AB-20.2 memory_store — the write path

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant SRV as ruvector-mcp memory_store<br/>agentbox/mcp/servers/ruvector-mcp.cjs:241
    participant MS as memStore<br/>agentbox/mcp/servers/lib/memory-tools.js:155
    participant PROT as checkProtectedNamespace<br/>agentbox/mcp/servers/lib/memory-tools.js:121
    participant XI as Xinference bge-small-en-v1.5 384-dim
    participant MD as memory-metadata<br/>agentbox/mcp/servers/lib/memory-metadata.js
    participant PG as memory_entries + HNSW
    participant NOT as memory-flash-notifier

    AG->>SRV: memory_store(key, value, namespace)
    SRV->>MS: memStore(key, value, namespace, options)
    MS->>PROT: checkProtectedNamespace(namespace)
    alt namespace is protected and RUVECTOR_ADMIN_WRITE is not "true"
        PROT-->>AG: write-protected (IR2 mandate-at-grant) storage none (:124)
        Note over PROT: RUVECTOR_PROTECTED_NAMESPACES default "governance-precedents" — prevents agents<br/>injecting synthetic records into governance-critical stores, e.g. precedent namespace<br/>poisoning via memory_store (:113-118)
    else allowed
        MS->>MS: id = entryId(namespace, key) (:159)
        MS->>XI: embed the value
        alt embedding succeeds
            XI-->>MS: 384-dim vector
            MS->>MS: embeddingClause = $6::ruvector(384) (:173)
        else embedding fails or xinference unavailable
            XI--xMS: error
            MS->>MS: embedFailure set (:177-180)
            alt RUVECTOR_EMBED_REPAIR not true — the DEFAULT
                MS-->>AG: REJECTED reason embedding-unavailable, remedy "restore the embedding service, or set<br/>RUVECTOR_EMBED_REPAIR=true" (:187-198)
                Note over MS: INVARIANT ADR-2014 FAIL-CLOSED: the store no longer degrades silently. Nothing is<br/>written — NO UNSEARCHABLE ROW is created (:36-45)
            else repair mode on
                MS->>MS: metadata.embedding_state = 'pending' plus embedding_pending_since / _reason (:224-226)
                Note over MS: an explicit, REPAIRABLE pending write — recoverable later by memory_repair_embeddings
            end
        end
        MS->>MD: type the metadata when the gate is on — importance, tags, memory_type (:204)
        MS->>PG: INSERT ... ON CONFLICT DO UPDATE
        Note over MS,PG: the ON CONFLICT clause assigns EXCLUDED.embedding rather than<br/>COALESCE(EXCLUDED.embedding, memory_entries.embedding), so the stored vector always<br/>tracks the stored value (:53-55)
        MS->>MD: metadata.embedding_state = 'embedded' (:220)
        MS->>NOT: notifyMemoryFlash
        MS-->>AG: success
    end
    Note over XI: INVARIANT (measured 2026-09-03): bge-small embeds only the first ~512 tokens, about<br/>2,500 chars, of a value — THE TAIL IS INVISIBLE TO SEARCH. Keep values under ~2,000<br/>chars, front-load the searchable facts, split long detail into linked entries.<br/>Retrieve-by-key still returns the whole value
    Note over MS: RESOLVED ADR-2051: LEARNING-memory Invariant 1 now states the enforced rule —<br/>embedding failure REJECTS the write by default (ADR-2014 fail-closed,<br/>memory-tools.js:36-45,:187-198), with RUVECTOR_EMBED_REPAIR=true the only route to<br/>an explicit repairable pending row.
```

## AB-20.3 memory_search — vector ANN, namespace scope and the degraded fallback

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant SRV as ruvector-mcp memory_search<br/>agentbox/mcp/servers/ruvector-mcp.cjs:278
    participant MS as memSearch<br/>agentbox/mcp/servers/lib/memory-tools.js:380
    participant XI as Xinference
    participant PG as memory_entries HNSW

    AG->>SRV: memory_search(query, namespace, limit, sourceType)
    SRV->>MS: memSearch(query, namespace, limit, sourceType)
    MS->>MS: sourceType "*" collapses to null — no filter (:382)
    MS->>XI: embed the query
    alt embedding available
        XI-->>MS: 384-dim query vector
        alt namespace is "*"
            MS->>PG: no namespace clause — GLOBAL CROSS-NAMESPACE search (:412)
            Note over MS,PG: namespace "*" = global cross-namespace (verified, undocumented in the tool schema)
        else scoped
            MS->>PG: AND namespace = $n (:412)
        end
        PG->>PG: ORDER BY embedding <=> $1::ruvector(384) (:432, :438)
        Note over PG: score = 1.0 - (embedding <=> query) — cosine distance operator on the RuVector HNSW<br/>access method (:430, :435)
        PG-->>MS: top-k rows, expired rows excluded by NOT_EXPIRED (:65)
        MS-->>AG: ranked results
    else vector search unavailable or failed
        MS->>MS: log WARN "DEGRADED: falling back to ILIKE text search — xinference unavailable or vector<br/>search failed. Semantic search is disabled." (:548-549)
        MS->>PG: WHERE (namespace = $1 OR $1 = '*') AND (key ILIKE $2 OR value::text ILIKE $2) (:552-556)
        PG-->>MS: literal matches only
        MS-->>AG: DEGRADED results
        Note over MS: this is DEGRADED, NOT NORMAL (:548) — check the xinference container and<br/>XINFERENCE_ENDPOINT
    end
    Note over MS,PG: every read path honours the `expires_at` guard — retrieve, list, vector search, the<br/>ILIKE fallback and the sweep all share NOT_EXPIRED (:65)
```

## AB-20.4 memory_retrieve and memory_list

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant SRV as ruvector-mcp<br/>agentbox/mcp/servers/ruvector-mcp.cjs
    participant MT as memory-tools<br/>agentbox/mcp/servers/lib/memory-tools.js
    participant PG as memory_entries

    alt memory_retrieve (declared :255)
        AG->>SRV: memory_retrieve(key, namespace)
        SRV->>MT: memRetrieve(key, namespace) (:355)
        MT->>PG: SELECT key, value, source_type WHERE namespace = $1 AND key = $2 AND NOT_EXPIRED ORDER<br/>BY updated_at DESC LIMIT 1 (:358-360)
        PG-->>AG: the newest non-expired row for that exact key
        Note over MT: retrieve-by-key is EXACT, not semantic — it returns the WHOLE value, so the ~512-token<br/>embed cap does not apply on this path
    else memory_list (declared :267)
        AG->>SRV: memory_list(namespace, limit)
        SRV->>MT: memList(namespace, limit) (:368)
        MT->>PG: SELECT key, value, source_type WHERE namespace = $1 AND NOT_EXPIRED ORDER BY created_at<br/>DESC LIMIT $2 (:371-373)
        PG-->>AG: newest-first page, default limit 100
        Note over MT: memList takes a LITERAL namespace — unlike memSearch it has no "*" global branch
    end
    Note over SRV: the same server also registers the non-memory claude-flow surface — swarm_init :298,<br/>agent_spawn :303, task_orchestrate :308, swarm_status :313, neural_patterns :318,<br/>coordination_sync :337, load_balance :342, performance_report :347, bottleneck_analyze<br/>:352, github_repo_analyze :357, github_pr_manage :362, workflow_create :367,<br/>workflow_execute :372, parallel_execute :377, sparc_mode :382
```

## AB-20.5 memory_hybrid_search

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant SRV as ruvector-mcp memory_hybrid_search<br/>agentbox/mcp/servers/ruvector-mcp.cjs:426
    participant HY as createHybridTools<br/>agentbox/mcp/servers/lib/memory-hybrid.js
    participant XI as Xinference
    participant PG as memory_entries
    participant AGG as memory-learning-aggregates

    AG->>SRV: memory_hybrid_search(query, namespace, limit)
    SRV->>HY: hybrid search
    par vector leg
        HY->>XI: embed the query
        HY->>PG: kNN over the HNSW index
    and lexical leg
        HY->>PG: literal token match
    end
    HY->>HY: blend the two rankings
    opt feed_retrieval gate on
        HY->>AGG: ONE bounded read, LIMIT 500 (memory-hybrid.js:57-101)
        AGG-->>HY: action:<pattern> to max wilson map
        HY->>HY: add a bounded bonus of 0.1 * wilson to rows whose metadata.tags intersect
        Note over HY: fail-open — any error leaves the base ranking untouched. Full producer chain in AB-21
    end
    HY-->>AG: blended, optionally re-ranked results
    Note over HY,PG: PERF WARNING (measured): memory_hybrid_search MATERIALISES THE WHOLE NAMESPACE — about<br/>72 s on ruvnet-kb. Avoid it on large namespaces until candidate-bounded hybrid lands.<br/>Plain memory_search is about 100 ms everywhere
    Note over PG: the recall harness exact-token class exists to prove hybrid never trades exact-token<br/>recall for semantic gains — see AB-20.9
```

## AB-20.6 memory_orient — the OODA cold-start bundle

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant SRV as ruvector-mcp memory_orient<br/>agentbox/mcp/servers/ruvector-mcp.cjs:444
    participant G as ruvector-gates<br/>agentbox/mcp/servers/lib/ruvector-gates.js
    participant OR as memOrient
    participant PG as memory_entries
    participant AGG as memory-learning-aggregates

    AG->>SRV: memory_orient {task, namespace, semantic_limit, aggregate_limit, episodic_limit}
    Note over SRV: defaults namespace "default", semantic_limit 8, aggregate_limit 10, episodic_limit 10<br/>(:450-453)
    SRV->>G: gates.memoryOrient()
    alt gate off
        G-->>AG: unknownTool — the tool is not merely disabled, it is INVISIBLE (:547)
    else gate on
        SRV->>OR: memOrient(task, namespace, {semanticLimit, aggregateLimit, episodicLimit}) (:548-550)
        par
            OR->>PG: top-k SEMANTIC memories for the task
        and
            OR->>AGG: effectiveness AGGREGATES — see AB-21
        and
            OR->>PG: recent EPISODIC entries for the session namespace
        end
        OR-->>AG: one cold-start bundle
    end
    Note over OR: read-only and FAIL-OPEN (:445)
    Note over G: every gated tool follows this shape — a gate-off tool returns unknownTool rather than an<br/>error, so a disabled feature leaves no runtime trace (byte-identical-when-off)
```

## AB-20.7 memory_sweep_episodic and memory_repair_embeddings

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator or scheduler
    participant SRV as ruvector-mcp<br/>agentbox/mcp/servers/ruvector-mcp.cjs
    participant SW as memSweepEpisodic<br/>agentbox/mcp/servers/lib/memory-tools.js:590
    participant RP as memRepairEmbeddings<br/>agentbox/mcp/servers/lib/memory-tools.js:269
    participant XI as Xinference
    participant PG as memory_entries

    alt memory_sweep_episodic (declared :479)
        OP->>SRV: memory_sweep_episodic(namespace, {types})
        SRV->>SW: memSweepEpisodic(namespace, opts)
        alt pg unavailable
            SW-->>OP: pg unavailable (:591)
        else namespace protected and not admin
            SW-->>OP: write-protected, swept 0 (:593-595)
        else types contains an unknown memory_type
            SW-->>OP: error naming the valid set from VALID_TYPES (:598-604)
            Note over SW: the type filter is validated against VALID_TYPES BEFORE any delete — an unknown type<br/>never silently sweeps everything
        else valid
            SW->>PG: delete expired rows matching the type clause
            PG-->>OP: swept count
            Note over PG: INDEX LAW consequence — a bulk delete degrades the HNSW graph silently. See AB-20.10
        end
    else memory_repair_embeddings (declared :411)
        OP->>SRV: memory_repair_embeddings(namespace)
        SRV->>RP: memRepairEmbeddings(opts)
        RP->>RP: namespace "*" collapses to null = all namespaces (:269)
        RP->>PG: SELECT count(*) WHERE embedding IS NULL (:278)
        alt none pending
            RP-->>OP: pending 0, repaired 0 (:288)
        else pending rows
            RP->>PG: SELECT id, namespace, key, value WHERE embedding IS NULL ORDER BY updated_at ASC<br/>(:304-306)
            loop each pending row
                RP->>XI: embed the value
                RP->>PG: UPDATE the embedding
            end
            RP-->>OP: repaired count
            Note over RP: this is the recovery path for rows admitted under RUVECTOR_EMBED_REPAIR — it is how a<br/>pending write becomes searchable
        end
    end
    Note over PG: DIVERGENCE D5 — the VisionClaw Solid Pod deleteAgentMemory() has NO REVERSE TOMBSTONE<br/>into RuVector, so deleting the pod copy does not revoke the RuVector-held agent memory.<br/>No point-in-time RuVector backup exists (SQLite-only backup-sqlite.sh), so there is no<br/>cross-store consistent restore, RPO or RTO for memory today. Cross-reference<br/>docs/DATA-authority-erasure.md before designing any right-to-erasure flow
```

## AB-20.8 The FORBIDDEN write paths

```mermaid
flowchart TB
    subgraph ok["THE ONLY SANCTIONED PATH"]
        A["Agent"] --> B["mcp__claude-flow__memory_* MCP tools"]
        B --> C["createMemoryTools backend external-pg<br/>agentbox/mcp/servers/lib/memory-tools.js:204"]
        C --> D["Xinference bge-small-en-v1.5 384-dim"]
        D --> E["INSERT with a real ruvector(384) vector"]
        E --> F["row is VISIBLE to HNSW search"]
    end
    subgraph bad["FORBIDDEN — bypasses the embedding pipeline"]
        G["claude-flow memory * CLI"] --> I["INSERT with NULL embedding"]
        H["raw SQL INSERT INTO memory_entries"] --> I
        I --> J["row is INVISIBLE to HNSW search"]
        J --> K["the write appears to succeed and the data is unfindable"]
    end
    subgraph idx["FORBIDDEN — index maintenance"]
        L["CREATE INDEX CONCURRENTLY on the RuVector HNSW AM"] --> M["VERIFIED DOUBLE-INSERTION —<br/>every tuple indexed twice"]
    end
    subgraph notes["Invariants and drift"]
        direction TB
        N1["INVARIANT ADR-2014 / DDD-016 I03: memory is written and read ONLY through the<br/>mcp__claude-flow__memory_* tools. Every learning component honours this — aggregates and<br/>cursors upsert through the governed memStore path, never raw SQL (see AB-21)"]
        N2["The governed server FAILS CLOSED on an unreachable Postgres (process.exit(1),<br/>ruvector-mcp.cjs:158-159) and there is NO sql.js fallback — so the CLI path is not a<br/>degraded mode of the same store, it is a DIFFERENT and broken store"]
        N3["Recovery from a NULL-embedding row is memory_repair_embeddings (see AB-20.7). Recovery<br/>from a degraded HNSW graph is a NON-CONCURRENT rebuild (see AB-20.10)"]
        N1 ~~~ N2 ~~~ N3
    end
```

## AB-20.9 The recall harness — the geometry merge gate

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator
    participant SH as agentbox.sh ruvector recall<br/>agentbox/agentbox.sh
    participant H as ruvector-recall-harness.mjs<br/>agentbox/scripts/ruvector-recall-harness.mjs:1
    participant FIX as recall-fixture.v1.json<br/>agentbox/scripts/recall-fixtures/recall-fixture.v1.json
    participant PG as live HNSW index
    participant EX as forced exact brute-force scan
    participant ART as backups/ruvector-sidecar/recall-runs/

    OP->>SH: ./agentbox.sh ruvector recall
    Note over SH: the lifecycle surface is ./agentbox.sh ruvector<br/><status|check|test|update|rollback|recall>
    SH->>H: run the frozen fixture
    H->>FIX: load the checked-in QuerySetFixture
    loop 3 runs — median of 3 absorbs HNSW ef_search entry-point jitter (:30-31)
        par self-recall@10 — 200 rows (:15-17)
            H->>PG: the row's OWN stored embedding is the query
            PG-->>H: pass iff the row's own id survives its own top-10
            Note over H: stratified across the >=50-row namespaces, ruvnet-kb capped at about 40 percent
        and true-recall@10 — 120 rows (:18-22)
            H->>EX: ground truth
            H->>PG: HNSW top-10
            PG-->>H: gated score counts queries whose own row survives the top-10 (the 119/120 framing)
            Note over H: the intersection recall |HNSW n exact| / min(10,|exact|) is SURFACED ALONGSIDE but is<br/>not the gated number. Restricted to >=20-row namespaces
        and exact-token — about 20-30 literal tokens (:23-28)
            H->>PG: pure-vector then hybrid
            PG-->>H: literal tokens known verbatim in a bounded namespace — error codes, CUDA_ARCH, HNSW,<br/>filenames, function names
            Note over H: requirement hybrid recall >= pure-vector recall (delta >= 0) — hybrid must NEVER trade<br/>exact-token recall for semantic gains
        end
    end
    H->>H: take the MEDIAN of the 3 runs
    alt median(self) >= 175/200 AND median(true) >= 102/120 AND median(exact-token hybrid delta) >= 0 (:32-33)
        H-->>OP: PASS — the gate opens
    else
        H-->>OP: FAIL — the consumer may not flip its gate
    end
    H->>ART: write the per-run evidence artifact <utc>.json (:40-42)
    Note over H,PG: INVARIANT: the harness is READ-ONLY against the DB — no memory_store, no schema change.<br/>Classes 1 and 2 issue only kNN SELECTs, class 3 calls the governed memSearch /<br/>memHybridSearch read paths. It NEVER writes an aggregate or a fixture row (:37-39)
    Note over H: INVARIANT I14 / ADR-2018: no consumer that ALTERS WHAT A QUERY RETURNS may flip its gate<br/>without a passing run here — SONA apply, attention re-rank, param tuning, feed_retrieval<br/>re-rank, an embedding-model cutover, a graph-augmented orient (:4-9)
    Note over H: a per-namespace self-recall breakdown is surfaced but NOT gated — it catches a<br/>regression localised to one namespace that a corpus-wide average would hide (:34-35)
    Note over OP: DOC-DRIFT D3: agentbox/CLAUDE.md quotes the frozen band as true >= 107/120 (live<br/>post-rebuild 109/120). The harness code gates at >= 102/120 (:32-33). CODE IS<br/>AUTHORITATIVE for the gate — the prose band is a tighter operational target. self >=<br/>175/200 agrees across both
```

## AB-20.10 The index law

```mermaid
stateDiagram-v2
    [*] --> Healthy
    Healthy --> Degraded : bulk ingest
    Healthy --> Degraded : bulk deletion, e.g. memory_sweep_episodic
    note right of Degraded
        HNSW graphs degrade SILENTLY under write churn. Recall drops with
        no error anywhere — nothing in the query path reports it, which is
        why the harness is the only detector.
    end note
    Degraded --> Rebuilding : NON-CONCURRENT rebuild, m=16, ef_construction=128
    note right of Rebuilding
        Takes about 5 minutes. This is the ONLY sanctioned recovery.
    end note
    Rebuilding --> Verifying : rebuild complete
    Verifying --> Healthy : recall harness PASSES the band (see AB-20.9)
    Verifying --> Degraded : harness FAILS
    Corrupted --> [*]
    Degraded --> Corrupted : CREATE INDEX CONCURRENTLY
    note right of Corrupted
        FORBIDDEN on the RuVector HNSW access method — VERIFIED
        DOUBLE-INSERTION, every tuple indexed twice. Never do this.
        agentbox/docs/reference/claude-context/ruvector-memory-state.md
    end note
    Healthy --> [*]
```

## AB-20.11 384-dim freeze and the inert SONA branch

```mermaid
sequenceDiagram
    autonumber
    participant SW as ruvector-sona-feeder.mjs<br/>agentbox/scripts/ruvector-sona-feeder.mjs
    participant G as gates<br/>agentbox/mcp/servers/lib/ruvector-gates.js
    participant TR as judged trajectories<br/>see AB-21
    participant SONA as ruvector_sona_learn scope agentbox_memory
    participant BIN as "@ruvector/sona@0.1.5 NAPI binary"
    participant SH as sona_health<br/>agentbox/mcp/servers/ruvector-mcp.cjs:471

    SW->>G: read sona_learn / sona_apply
    alt both OFF — the shipped state
        G-->>SW: gates off, fast exit
        Note over G: byte-identical-when-off — a default-off manifest is indistinguishable from the<br/>pre-learning product
    else hypothetically on
        SW->>TR: stream judged trajectories
        SW->>SONA: learn under a FIXED 384-dim scope
        SONA->>BIN: forward
        BIN-->>SONA: status "learned"
        Note over BIN: DIVERGENCE D1: the prebuilt binary HARDCODES embedding_dim = 256. A 384-dim learn<br/>returns status "learned" but ACCUMULATES NOTHING — verified live. Both gates stay off<br/>until a 384-dim-capable binary ships (agentbox.toml sona keys)
    end
    SH-->>SH: surfaces the SONA verdict for an operator
    Note over G: attention_rerank is OFF BY MEASUREMENT, not caution — on an L2-normalised corpus the<br/>attention blend is a MATHEMATICAL IDENTITY, max diff 4e-7. att = cos/sqrt(dim) on<br/>L2-normalised bge embeddings (memory-tools.js:74-80)
    Note over SONA: INVARIANT ADR-2019: one FIXED GLOBAL SONA scope, never per-namespace, dimension-tagged<br/>(D4 / I22, memory-tools.js:74). A dimension migration mints a FRESH scope and never<br/>reuses agentbox_memory
    Note over SW: DIVERGENCE D6: the v2 model-lifecycle keys embedding_dual_write,<br/>embedding_active_column, graph_backbone, param_tuning_enabled and the m3 / legacy-mining<br/>hygiene ops are DECLARED AND DEFAULT-OFF, gated on a passing recall harness run before<br/>any may flip
    Note over BIN: DIVERGENCE D2: agentbox.toml justifies the feed_retrieval flip with "78 aggregates >=20<br/>samples (2026-08-31)" while<br/>agentbox/docs/reference/claude-context/ruvector-memory-state.md records 12 from the<br/>2026-07-21 sweep. The toml is the running config and the more recent number
```

## AB-20.12 memory_health and the store schema

```mermaid
erDiagram
    memory_entries {
        text id PK "entryId(namespace, key)"
        text namespace "'*' means global at search time"
        text key
        jsonb value
        ruvector_384 embedding "NULL = invisible to HNSW"
        text source_type
        jsonb metadata "importance, tags, memory_type, embedding_state, embedding_pending_since, embedding_pending_reason"
        timestamptz created_at "memList orders by this"
        timestamptz updated_at "memRetrieve and repair order by this"
        timestamptz expires_at "NOT_EXPIRED guards every read path"
    }
    patterns {
        text id PK "distilled-sha256-12-<hash(action)>"
        text action
        ruvector_384 embedding "embedded BEFORE insert, never NULL"
        jsonb metadata "provenance judge:trajectory"
    }
    trajectories {
        text id PK
        text task
        text agent
        text status
        timestamptz started_at
        jsonb metadata
    }
    trajectory_steps {
        text id PK
        text trajectory_id FK
        text action "low-cardinality command pattern"
        jsonb result "outcome, signal, failure_mode, token_count, duration_ms"
        real quality "1.0 clean, 0.85 stderr noise, 0.0 failure"
        int step_order
        int duration_ms "may legitimately be 0 or NULL"
    }
    trajectories ||--o{ trajectory_steps : "has"
    trajectory_steps ||--o{ patterns : "distilled into"
    trajectory_steps ||--o{ memory_entries : "aggregated into ns memory-learning-aggregates"
```
