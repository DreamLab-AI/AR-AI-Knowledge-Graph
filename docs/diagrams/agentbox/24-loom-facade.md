---
id: AB-24
title: Ontology Loom facade and the model-swap seam
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
adrs: [ADR-2023, ADR-2053, ADR-2055]
sources:
  - agentbox/mcp/servers/lib/ontology-retrieval.js
  - agentbox/mcp/servers/lib/ontology-budget.js
  - agentbox/agentbox.toml
  - docker-compose.unified.yml
  - loom/README.md
  - agentbox/scripts/opf-router.py
  - agentbox/mcp/servers/lib/ontology-telemetry.js
  - agentbox/flake.nix
verified_commit: 7a20db228
---

## AB-24.1 Two deployments of one facade contract — topology

```mermaid
flowchart TB
    subgraph consumers["Consumers hold a DOOR, never a raw model port (ADR-2023)"]
        RET["ontology-retrieval brain<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:667"]
        COND["ontology condense<br/>agentbox/agentbox.toml:668"]
        DREAM["dream-engine loom_url<br/>agentbox/agentbox.toml:1648"]
        SEED["AoE session seed slug=loom<br/>agentbox/agentbox.toml:1294"]
        SEEDRAW["AoE session seed slug=loom-raw<br/>agentbox/agentbox.toml:1301"]
        EMAIL["email gateway REASONER_BASE_URL<br/>see AB-27"]
    end
    subgraph depA["Deployment A — LAN facade on machinelearn .132"]
        F84["Loom facade<br/>http://192.168.2.132:8084/v1"]
    end
    subgraph depB["Deployment B — sidecar on visionclaw_network (compose profile loom)"]
        SIDE["loom-facade (Rust)<br/>docker-compose.unified.yml:298"]
        TMPFS["tmpfs /run/loom mode=0750 uid=65532<br/>docker-compose.unified.yml:343"]
        DATA["loom-data :ro generation<br/>docker-compose.unified.yml:338"]
    end
    subgraph model["The model — an operational detail BEHIND the door"]
        M85["loom-model :8085 qwen3.8-27B<br/>DISTILL_BACKEND_URL"]
    end
    RET -->|"LOOM_FACADE_URL"| F84
    COND -->|"POST /v1/chat/completions"| F84
    DREAM -->|"llm_provider=loom only"| F84
    SEED -->|"model loom-lan/qwen3.8-27B"| F84
    SEEDRAW -->|"raw :8085 — NOT the door"| M85
    EMAIL -->|"http://loom:8080/v1"| SIDE
    F84 -->|"ml DNATs over the 25G rail 10.10.10.0/30"| M85
    SIDE -->|"DISTILL_BACKEND_URL blank = retrieval-only, /v1 returns 503"| M85
    DATA --> SIDE
    SIDE -->|"entrypoint copies .rvdb off :ro — opening redb mutates it"| TMPFS
    subgraph notes["Invariants and drift"]
        direction TB
        N1["RESOLVED ADR-2070 #40;2026-09-05#41;: not a breach. ADR-045 one-front-door is an INGRESS rule<br/>#40;:9096, NIP-98, control surfaces reaching INTO the box#41; and says nothing about EGRESS to a LAN<br/>model host. The raw :8085 door is deliberate and named #40;flake.nix LOOM_RAW_BASE_URL, the loom-raw<br/>session seed#41; — agent-choice and benchmark-only for raw coding, never a fallback and never<br/>auto-routed when the facade errors. Knowledge-work consumers hold :8084. A third door needs an ADR"]
        N2["RESOLVED ADR-2055: opf-router is the PRIVACY-FILTER redaction sidecar on OPF_PORT<br/>9092 (agentbox.toml [privacy_filter].port, scripts/opf-router.py:41, flake.nix<br/>[program:opf-router]). BASELINE-container previously described it as an<br/>OpenAI-compatible facade on :8084 — corrected. No agentbox program serves<br/>:8084 — that is the Loom facade on machinelearn"]
        N3["The loom-facade implementation lives OUTSIDE this repo at /home/devuser/workspace/loom.<br/>This repo holds the deployment contract only (loom/README.md:8-15)"]
        N1 ~~~ N2 ~~~ N3
    end
```

## AB-24.2 selectBackend — which store answers, and why

```mermaid
sequenceDiagram
    autonumber
    participant CALL as createDefaultRetrieval<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:667
    participant SEL as selectBackend<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:471
    participant ENV as process.env
    participant LF as makeLoomFetch<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:596
    participant VF as makeVcFetch<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:496
    participant TEL as createTelemetrySink<br/>agentbox/mcp/servers/lib/ontology-telemetry.js

    CALL->>SEL: selectBackend(opts, env)
    SEL->>ENV: read LOOM_FACADE_URL / VISIONCLAW_API_URL / LOOM_GENERATION
    Note over SEL,ENV: generation falls back LOOM_GENERATION then ONTOLOGY_GENERATION then null (:474-476)
    alt loomUrl set AND no injected vcFetch (:477)
        SEL-->>CALL: name=loom url=loomUrl configured=true reason=LOOM_URL_SET
        CALL->>LF: makeLoomFetch(opts)
        CALL->>TEL: canary() — startup liveness probe, loud on failure, fail-open (:672)
    else vcUrl empty (:483)
        SEL-->>CALL: name=none url=null configured=false reason=NOT_CONFIGURED
    else loomUrl set BUT vcFetch injected (:491)
        SEL-->>CALL: name=visionclaw reason=VC_FETCH_INJECTED
        CALL->>VF: makeVcFetch(opts) — tests / deliberate pinning
    else loomUrl unset (:491)
        SEL-->>CALL: name=visionclaw reason=LOOM_URL_UNSET
        CALL->>VF: makeVcFetch(opts) — DEFAULT_API http://visionclaw-server:4000 (:457)
    end
    Note over SEL: INVARIANT: an unset LOOM_FACADE_URL is the ordinary VisionClaw path and NOT a fault — a<br/>CONFIGURED Loom that is unreachable is an operational fault (:459-467)
    Note over CALL: One brain = this module plus the shared backing stores, NOT one process (:2-6)
```

## AB-24.3 Loom-backed retrieval — /loom/search seed then /loom/sparql expand

```mermaid
sequenceDiagram
    autonumber
    participant AG as ontology_ask caller<br/>see AB-25
    participant ASK as ask()<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:279
    participant SEED as loomSeedFn<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:625
    participant EXP as loomExpandFn<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:635
    participant LF as loomFetch<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:600
    participant LOOM as Loom facade<br/>LOOM_FACADE_URL
    participant BUD as clampToBudget<br/>agentbox/mcp/servers/lib/ontology-budget.js

    AG->>ASK: ask({query, mode, depth, domain, max_tokens, provenance})
    ASK->>SEED: seedFn({query, limit: 8, domain})
    SEED->>LF: POST /loom/search {q, limit}
    LF->>LOOM: fetch, AbortController timeout ONTOLOGY_TIMEOUT_MS default 10000 (:598)
    alt HTTP not ok (:607)
        LOOM-->>LF: non-2xx
        LF-->>SEED: {error: loom_http_<status>}
        SEED-->>ASK: throw res (:628)
        ASK->>ASK: classifyCause -> stages [seed] (+ backend-unavailable on availability/timeout)
        Note over ASK: NOT cached — a transport fault must not pin an empty answer for the TTL (:371-372)
        ASK-->>AG: empty, degraded=true, error=backend_configured_but_unavailable
    else AbortError (:613)
        LF-->>SEED: {error: ontology_timeout}
        ASK-->>AG: empty, degraded=true, stages [seed, backend-unavailable]
    else ok
        LOOM-->>LF: {hits:[{iri,label}]}
        SEED-->>ASK: rows mapped to {iri, label, score: 1 - i*0.01} (:630)
        ASK->>ASK: maturity + domain gate (:376-383)
        Note over ASK: unknown maturity is NOT gated out — only explicitly-low classes drop (:378-380)
        alt no seeds survive (:385)
            ASK->>ASK: cache.set(key, empty)
            ASK-->>AG: empty result
        else mode=expand AND depth>0 (:393)
            ASK->>EXP: expandFn({seedIris, depth, provenance})
            EXP->>LF: POST /loom/sparql childSparql LIMIT 60 (:643)
            Note over EXP,LF: children FIRST so the downstream budget clamp never trims them (ADR-112, :650)
            EXP->>LF: POST /loom/sparql outSparql LIMIT min(80*depth, 300) (:639-641)
            Note over EXP: Loom store merges asserted+inferred into ONE graph — no GRAPH clause needed (:640)
            alt sparql error
                LF-->>EXP: {error}
                EXP-->>ASK: throw Error with stage='sparql' (:647)
                ASK->>ASK: degradedStages.push(expansion) then push(sparql) (:405-406)
                Note over ASK: DOC-DRIFT closed 2026-09-05 — this path used to return degraded=false, making a<br/>menu-only answer indistinguishable from a full expansion (:403-404)
            else ok
                EXP-->>ASK: children then outgoing triples (:653-655)
            end
        end
        ASK->>BUD: clampToBudget(turtle, model_tier, max_tokens) (:417)
        BUD-->>ASK: {text, tokens, truncated}
        ASK-->>AG: {turtle, breadcrumb, seed_iris, tokens_used, truncated, degraded, degraded_stages,<br/>backend, generation}
    end
```

## AB-24.4 Cache key completeness and cache-hit constraint revalidation

```mermaid
sequenceDiagram
    autonumber
    participant ASK as ask()<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:279
    participant KEY as cacheKey<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:74
    participant CACHE as cache
    participant SAT as cacheEntrySatisfies<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:335
    participant TEL as telemetry

    ASK->>KEY: cacheKey(resolved req)
    KEY->>KEY: FNV-1a over field=value for every CACHE_KEY_FIELDS entry in declaration order (:74-84)
    Note over KEY: CACHE_KEY_FIELDS = query, model_tier, mode, depth, provenance, full, domain, max_tokens,<br/>budget, min_maturity, backend, generation (:46-59)
    Note over KEY: absent/null/'' collapse to one sentinel so absence never aliases a value (:61-66)
    Note over KEY: INVARIANT: an omitted field is a CORRECTNESS BUG, not a perf tweak (:32-33)
    KEY-->>ASK: 'ont:'+hash
    ASK->>CACHE: get(key)
    alt hit
        CACHE-->>ASK: cached entry
        ASK->>SAT: cacheEntrySatisfies(entry, constraints)
        alt constraints satisfied (:336)
            ASK->>TEL: record cache_hit
            Note over ASK: a hit REPLAYS the degradation state it was stored with — a partial answer stays partial<br/>however often it is served (:339-343)
            ASK-->>ASK: return entry.result with cache_hit=true
        else violation (:348)
            ASK->>TEL: record cache_constraint_miss with violations
            Note over ASK: policy is MISS and re-retrieve, NEVER truncate — the stored Turtle was already clamped<br/>once and a second cut would slice a seed mid-triple and silently change what the<br/>grounding asserts (:331-334)
        end
    else miss
        ASK->>ASK: proceed to seed (see AB-24.3)
    end
    Note over ASK,CACHE: DIVERGENCE closed 2026-09-05 — domain and max_tokens were absent from the key, so an<br/>AI-domain 830-token body was served cache_hit=true to a robotics request capped at 50<br/>tokens (:36-39)
```

## AB-24.5 Backend, stage and outcome vocabularies

```mermaid
classDiagram
    class BACKENDS {
        <<frozen enum>>
        +LOOM "loom"
        +VISIONCLAW "visionclaw"
        +INJECTED "injected"
        +NONE "none"
    }
    class DEGRADED_STAGES {
        <<frozen enum>>
        +SEED "seed"
        +EXPANSION "expansion"
        +SPARQL "sparql"
        +BACKEND_UNAVAILABLE "backend-unavailable"
    }
    class DEGRADED_OUTCOMES {
        <<frozen enum>>
        +BACKEND_CONFIGURED_UNAVAILABLE "backend_configured_but_unavailable"
        +BACKEND_NOT_CONFIGURED "backend_not_configured"
        +SEED_REJECTED "seed_rejected"
        +EXPANSION_UNAVAILABLE "expansion_unavailable"
    }
    class BackendSelection {
        +String name
        +String url
        +Boolean configured
        +String generation
        +String reason
    }
    class AskResult {
        +String turtle
        +String breadcrumb
        +List~String~ seed_iris
        +Number tokens_used
        +Boolean truncated
        +String provenance
        +Boolean cache_hit
        +Boolean degraded
        +List~String~ degraded_stages
        +Boolean full_denied
        +String domain
        +String backend
        +Boolean backend_configured
        +String generation
        +Number latency_ms
    }
    class MATURITY_RANK {
        <<frozen map>>
        +draft 0
        +developing 1
        +emerging 2
        +growing 3
        +established 4
        +mature 5
    }
    BackendSelection --> BACKENDS : name drawn from
    AskResult --> DEGRADED_STAGES : degraded_stages drawn from
    AskResult --> BACKENDS : backend drawn from
    AskResult ..> DEGRADED_OUTCOMES : error drawn from
    note for DEGRADED_OUTCOMES "BACKEND_CONFIGURED_UNAVAILABLE is deliberately distinct from BACKEND_NOT_CONFIGURED —<br/>collapsing the two hides a dead facade behind a normal fallback<br/>(ontology-retrieval.js:100-106)"
    note for MATURITY_RANK "classifyCause splits availability/timeout from auth_or_validation so a 401 is never<br/>reported as unavailability (ontology-retrieval.js:700-702)"
```

## AB-24.6 POST /v1/chat/completions — scaffold injection then delegate

```mermaid
sequenceDiagram
    autonumber
    participant C as Consumer<br/>holds the door, never the model
    participant FAC as loom-facade<br/>LOOM_FACADE_PORT 8080
    participant IDX as staged generation :ro<br/>docker-compose.unified.yml:338
    participant XI as Xinference bge-small-en-v1.5 384-dim<br/>XINFERENCE_URL
    participant M as model behind DISTILL_BACKEND_URL

    rect rgb(235,242,250)
        Note over C,FAC: retrieval tier — NO model needed (loom/README.md:21,26-32)
        C->>FAC: GET /health
        FAC-->>C: liveness, corpus generation stamp, backend/graph/index readiness, injection_policy
        C->>FAC: GET /loom/generation
        FAC-->>C: the corpus generation identity being served
        C->>FAC: POST /loom/scaffold
        FAC->>IDX: scaffold-index.json + prose-index.json lookup
        FAC-->>C: budget-clamped ontology grounding, ONTOLOGY_BUDGET default 1500
    end
    rect rgb(250,240,235)
        Note over C,M: delegation tier — REQUIRES a model
        C->>FAC: POST /v1/chat/completions
        alt DISTILL_BACKEND_URL blank (docker-compose.unified.yml:309)
            FAC-->>C: 503 — retrieval-only deployment
        else backend configured
            FAC->>IDX: scaffold-inject the LAST user message
            opt LOOM_SEMANTIC_FALLBACK=1 (default 0, gated off until the recall bench clears)
                FAC->>XI: embed query, 384-dim
                XI-->>FAC: query vector
            end
            alt LOOM_CONFIDENCE_INJECTION=1 (default 0 — master switch OFF)
                FAC->>FAC: score matches against LOOM_STRONG_MATCH_SCORE 8.0
                alt score < LOOM_MIN_INJECT_SCORE 2.0
                    FAC->>FAC: skip injection entirely
                else
                    FAC->>FAC: keep matches at or above LOOM_MIN_INJECT_FRACTION 0.4 of budget
                end
            end
            FAC->>M: delegate chat-completions
            Note over FAC,M: PROTOCOL: reasoning backends truncate to EMPTY below LOOM_MIN_MAX_TOKENS 1536 — the<br/>400-to-empty trap (docker-compose.unified.yml:316-317)
            M-->>FAC: completion
            FAC-->>C: completion
        end
        C->>FAC: GET /v1/models
        FAC-->>M: model identity passthrough
    end
    Note over C: INVARIANT: grounding stays on the LAN — the facade delegates only to a LAN/local model
```

## AB-24.7 Model swap — zero consumer change

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator
    participant CFG as deployment config<br/>DISTILL_BACKEND_URL / agentbox.toml
    participant FAC as Loom facade :8084 or loom:8080
    participant OLD as outgoing model
    participant NEW as incoming model :8085
    participant CONS as every consumer<br/>see AB-24.1

    Note over CONS: consumers hold ONLY the door URL — none names a model port for scaffolded work
    OP->>NEW: stage the new model behind the facade
    OP->>CFG: repoint DISTILL_BACKEND_URL (compose) or loom_url stays constant (toml)
    OP->>FAC: restart / reload
    FAC->>NEW: delegate subsequent /v1/chat/completions
    FAC--xOLD: no longer delegated to
    CONS->>FAC: unchanged calls
    FAC-->>CONS: unchanged contract
    Note over OP,CONS: INVARIANT ADR-2023: swapping the deployed model must NOT touch any consumer — the model<br/>is an operational detail behind :8084
    Note over CFG: history — Gemma then Muse then Qwen3.8-27B — agentbox.toml:1649 loom_model =<br/>qwen3.8-27B, :1618 loom_max_tokens = 32768
    Note over FAC: DOC-DRIFT: GOVERNANCE-capabilities cites agentbox.toml:1599-1601 with loom_max_tokens<br/>16384 — the working tree has loom_url at :1613 and loom_max_tokens = 32768 at :1618 —<br/>the cap was raised after glm-5.3 burned ~16k reasoning tokens and hit the old 16384 cap<br/>with empty content twice (agentbox.toml:1650-1652)
    Note over FAC: DOC-DRIFT: GOVERNANCE-capabilities cites session seeds at agentbox.toml:1266 and :1238 —<br/>the working tree has slug=loom at :1259 and slug=loom-raw at :1266
    Note over NEW: DIVERGENCE: HP's old 192.168.2.48 is DEAD — a stale model-backend route black-holes<br/>every synthesis while /health still answers
```

## AB-24.8 Deployment B bring-up and the staging traps

```mermaid
stateDiagram-v2
    [*] --> ImageBuilt
    ImageBuilt --> Staged : operator stages a full generation
    note right of ImageBuilt
        Image is NOT built from this repo — no Dockerfile here.
        docker build -f loom/deploy/Dockerfile -t loom-rust /home/devuser/workspace
        The build context is the workspace PARENT because loom path-depends
        on the sibling ruvector crate and COPY cannot escape its context.
        loom/README.md:36-45
    end note
    Staged --> Starting : docker compose --profile loom up -d loom
    note right of Staged
        A full generation is scaffold-index.json + prose-index.json
        + the TTLs + ontology-corpus.rvdb with its .generation.json sidecar.
        There is NO mirror-on-start step in the Rust image — ONTOLOGY_SITE and
        LOOM_MIRROR_ON_START are gone and the generation is served immutably.
        loom/README.md:56-71
    end note
    Starting --> RvdbCopied : entrypoint copies .rvdb to tmpfs /run/loom
    note right of Starting
        Opening the .rvdb mutates the redb file even for READS because the
        HNSW index is repacked on open, so it cannot be served from the
        read-only mount. tmpfs uid/gid MUST stay 65532 to match the image's
        non-root user or the copy fails EACCES.
        loom/README.md:78-81, docker-compose.unified.yml:339-343
    end note
    RvdbCopied --> Healthy : GET /health returns 200
    RvdbCopied --> EmptyFloor : source empty or mis-pointed
    note right of EmptyFloor
        THE EMPTY-FLOOR TRAP. The facade still starts and /health still
        returns 200, but the log reads "lexical index NOT loaded ... empty
        floor". That is a STAGING bug, not a dead container — check the
        mount before you check the process. loom/README.md:74-76
    end note
    EmptyFloor --> Staged : repoint LOOM_DATA_SOURCE
    Healthy --> RetrievalOnly : DISTILL_BACKEND_URL blank
    Healthy --> FullService : DISTILL_BACKEND_URL set
    RetrievalOnly --> FullService : model attached behind the seam
    FullService --> Healthy : healthcheck every 30s, 3 retries, 25s start_period
    RetrievalOnly --> [*]
    FullService --> [*]
```

## AB-24.9 Consumer register — which door each one holds

```mermaid
flowchart LR
    subgraph doors["Doors"]
        D84["LAN facade :8084/v1"]
        D80["sidecar loom:8080/v1"]
        D85["raw model :8085 — NOT a door"]
    end
    RET["ontology-retrieval brain<br/>LOOM_FACADE_URL<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:472"] --> D84
    COND["ontology condense endpoint<br/>agentbox/agentbox.toml:668<br/>model qwen3.8-27B style openai max_concurrency 2"] --> D84
    DREAM["dream_machine loom_url<br/>agentbox/agentbox.toml:1648"] --> D84
    SEEDL["session seed slug=loom<br/>agentbox/agentbox.toml:1294<br/>model loom-lan/qwen3.8-27B — scaffolded, knowledge work"] --> D84
    SEEDR["session seed slug=loom-raw<br/>agentbox/agentbox.toml:1301<br/>model loom-raw/qwen3.8-27B — no scaffold, coding"] --> D85
    EMAIL["email gateway<br/>REASONER_BASE_URL http://loom:8080/v1<br/>loom/README.md:19-21"] --> D80
    CUST["consultant custom ai_base_url<br/>agentbox/agentbox.toml:1607"] --> D80
    D84 --> M["qwen3.8-27B"]
    D80 --> M
    D85 --> M
    subgraph notes["Invariants and drift"]
        direction TB
        N1["RESOLVED ADR-2053: the dream engine's default provider is Z.AI by deliberate<br/>choice — GOVERNANCE-capabilities now states this and names the egress posture.<br/>loom_url/loom_model select the LAN-only path when llm_provider = loom. See AB-23"]
        N2["PROPOSED ADR-2074: the ADR-051 deferred-distillation tools become a discrete<br/>manifest-gated MCP server with a job URN kind and distill plus recombine beads<br/>ADR-2023 remaining is the ORIGIN of this gap, not its resolution (see AB-26)"]
        N3["PROPOSED ADR-2075: the Loom exposes a generation descriptor and the client reports the<br/>ATTESTED generation - a configured value that disagrees fails labelled instead of being<br/>served or relabelled, and the cache keys on the attested id"]
        N4["PROPOSED ADR-2076: benchmark /loom/search plus /loom/sparql on its own terms with a<br/>frozen recall band in the shape of the RuVector recall gate - scaffold and chat numbers<br/>are never cited as evidence for this path"]
        N5["app/ontology-mcp is a standalone stdio MCP server left in place with no build or run<br/>path from this repo, pending a decision on where it should live (loom/README.md:108-115)"]
        N1 ~~~ N2 ~~~ N3 ~~~ N4 ~~~ N5
    end
```
