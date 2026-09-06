---
id: ES-06
title: Ontology Loom and the email privacy path
area: estate
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2023]
sources:
  - agentbox/skills/email-search/SKILL.md
  - agentbox/docs/adr/ADR-2023-loom-facade.md
  - loom/README.md
  - docker-compose.unified.yml
  - agentbox/flake.nix
  - agentbox/agentbox.toml
verified_commit: 7a20db228
---
## ES-06.1 Topology — the gateway holds a façade, never a model port
```mermaid
flowchart LR
    subgraph box["agentbox container"]
        AG["agent / skill<br/>mcp email-gateway"]
    end
    subgraph vcnet["visionclaw_network (docker bridge)"]
        GW["email-mcp-gateway:8765<br/>streamable-HTTP MCP, bearer auth"]
        LOOMB["loom sidecar — Deployment B, NOT RUNNING BY DEFAULT<br/>hostname loom, LOOM_FACADE_PORT 8080<br/>docker-compose.unified.yml:298-304,<br/>gated behind profiles: [loom] :360-361"]
        XI["xinference:9997/v1<br/>bge-small-en-v1.5 / 384"]
    end
    subgraph ml["machinelearn .132"]
        DNAT["hp-nat.service DNAT :8084<br/>plus MSS clamp for the 9000 to 1500 step-down"]
    end
    subgraph hp["HP-Desktop — downstream, NO LAN IP"]
        LOOMA["Loom façade :8084 — Deployment A<br/>COLOCATED with the model"]
        MODEL["loom-model container :8085<br/>Qwen3.8-27B, cutover 2026-08-14"]
    end

    AG -- "MCP tools ask_email / fetch_* / refresh_inbox" --> GW
    GW -- "REASONER_BASE_URL" --> LOOMB
    GW -- "REASONER_BASE_URL alternative<br/>http 192.168.2.132:8084/v1" --> DNAT
    DNAT -- "25G rail 10.10.10.0/30 MTU 9000" --> LOOMA
    LOOMA -- "DISTILL_BACKEND_URL" --> MODEL
    LOOMB -- "DISTILL_BACKEND_URL<br/>blank = retrieval-only" --> MODEL
    LOOMB -- "semantic fallback query embed" --> XI

    INV1["INVARIANT — consumers hold the FAÇADE, never the model port.<br/>The deployed model is a URL behind DISTILL_BACKEND_URL;<br/>swapping Muse to Gemma to Qwen3.8 to next never touches a<br/>consumer. This is the no-technical-debt-on-upgrade guarantee."]
    INV2["INVARIANT — the Loom IS the email privacy system. It<br/>delegates ONLY to a LAN/local model, never to a cloud<br/>endpoint, so mail content never leaves the LAN."]
    DIV0["DIVERGENCE — Deployment B is SPECIFIED, not running. The loom<br/>service is gated behind compose profile loom (docker-compose.unified.yml:360-361),<br/>so a default up never starts it, and its image is not built from this repo:<br/>the referenced loom/deploy/Dockerfile does not exist in this checkout<br/>(:292-296, loom/ holds README.md + app/ only). Deployment A (:8084 on HP)<br/>is the live path. see ES-01.6"]
    DIV1["DIVERGENCE — 192.168.2.48 (HP's old LAN IP) is DEAD.<br/>HP is off the Sodola with no LAN IP; ml routes and NATs it<br/>over the direct 25G rail. Never target .48. see ES-06.7"]
    DIV2["DIVERGENCE GOVERNANCE-capabilities — ADR-051 (Loom) is<br/>decision_status PROPOSED while the Loom is<br/>production-critical. Interim authority is the governing doc."]
    DIV3["DIVERGENCE ADR-045 one front door publishes TWO LAN doors —<br/>the scaffolded façade (:8084) and the raw model (:8085) are<br/>both reachable; consumers must pick correctly per task."]

    INV1 --- INV2
    LOOMA --> DIV1
    LOOMA --> DIV2
    MODEL --> DIV3
```

## ES-06.2 ask_email — tier 1, egress filter applied
```mermaid
sequenceDiagram
    autonumber
    participant A as agent
    participant G as email-mcp-gateway:8765
    participant R as retrieval (bge-m3)
    participant L as Loom façade
    participant M as LAN model
    participant F as gpt-oss-safeguard<br/>egress filter

    A->>G: ask_email{query, date_from?, date_to?, sender?, folder?, top_k?}
    Note over A,G: Transport auth — Authorization Bearer<br/>AGENTBOX_EMAIL_GATEWAY_TOKEN gates ANY call
    alt bearer missing or wrong
        G-->>A: 401 — re-provision the token
    else bearer ok
        G->>R: retrieve candidate messages
        R-->>G: top_k matches
        G->>L: POST /v1/chat/completions (REASONER_BASE_URL)
        L->>L: scaffold-inject the last user message (see ES-06.5)
        L->>M: delegate to the LAN model
        M-->>L: completion
        L-->>G: grounded answer
        G->>F: sanitize
        F-->>G: gist plus abstracted evidence — roles, date buckets, opaque ref_id
        G-->>A: sanitized answer, NEVER raw mail
    end
    Note over G,F: INVARIANT — ask_email ALWAYS sanitizes regardless of<br/>any pubkey passed to it. Raw data comes only from the<br/>raw tools. Default posture is to prefer this tool.
```

## ES-06.3 Break-glass tier — raw tools behind a second, capability gate
```mermaid
sequenceDiagram
    autonumber
    participant A as agent
    participant G as email-mcp-gateway:8765
    participant AL as PRIVILEGED_NOSTR_PUBKEYS allow-list
    participant S as message store

    A->>G: fetch_email_raw{query, nostr_pubkey} or fetch_email_by_ref{ref_id, nostr_pubkey}
    rect rgb(240,230,230)
    Note over A,G: TIER 1 — transport bearer gates any call
    alt bearer invalid
        G-->>A: 401
    else bearer valid
        Note over G,AL: TIER 2 — capability. The nostr_pubkey ARGUMENT is<br/>checked against the server allow-list. Every attempt is<br/>logged with an 8-char fingerprint.
        G->>AL: lookup pubkey
        alt pubkey on the allow-list
            AL-->>G: authorised
            G->>S: fetch raw
            S-->>G: real headers, sender, subject, date, full text
            G-->>A: RAW — egress filter BYPASSED
        else absent, empty, or npub instead of hex
            AL-->>G: not authorised
            G-->>A: {"authorized": false} and NO data
        end
    end
    end
    Note over A,AL: INVARIANT — a Nostr PUBLIC key is the publishable half,<br/>an identity/capability token, not a secret. Passing it as a<br/>tool argument is the design, not a leak. What never leaves<br/>the box is the bearer token and the Nostr PRIVATE key.
    Note over G: Read the operator pubkey from AGENTBOX_X_ONLY_PUBKEY_HEX<br/>at call time. Never hardcode the hex into skill source.
```

## ES-06.4 refresh_inbox — on-demand ingest across two Proton accounts
```mermaid
sequenceDiagram
    autonumber
    participant A as agent
    participant G as email-mcp-gateway:8765
    participant PB as Proton Bridge (in-container)
    participant P1 as primary IMAP_USER
    participant P2 as secondary IMAP_USER_2
    participant IX as index

    A->>G: refresh_inbox
    Note over G: Pull new mail NOW rather than waiting for the ~4h crawl.<br/>Used for password resets, one-time codes, verification links.
    G->>PB: open bridge
    loop within a time budget, primary first
        G->>P1: crawl new messages
        P1-->>G: messages
        G->>P2: crawl new messages
        P2-->>G: messages
    end
    G->>IX: ingest and embed
    IX-->>G: indexed
    G-->>A: newest messages plus accounts_crawled[]
    Note over G,P2: A third slot IMAP_USER_3 exists but is EMPTY.<br/>The scheduled crawl logs "IMAP ingest complete across<br/>2/2 account(s)".
    Note over A,G: DIVERGENCE — a 180 s refresh_inbox timeout is the<br/>signature of a stale reasoning route, NOT a dead<br/>container. see ES-06.7
```

## ES-06.5 Loom scaffold-inject — grounding, budget clamp and the confidence gate
```mermaid
sequenceDiagram
    autonumber
    participant C as consumer
    participant F as loom-facade
    participant IX as scaffold + prose index<br/>ontology-corpus.rvdb
    participant XI as xinference /v1/embeddings
    participant B as DISTILL_BACKEND_URL

    C->>F: POST /v1/chat/completions
    F->>IX: lexical match on the last user message
    IX-->>F: scored matches
    opt LOOM_SEMANTIC_FALLBACK=1
        F->>XI: embed query (bge-small-en-v1.5 / 384)
        XI-->>F: vector
        F->>IX: semantic fallback
        Note over F,IX: DIVERGENCE — gated OFF by default (=0) until the<br/>recall bench clears (loom RUST-ARCHITECTURE 8.4)
    end
    alt LOOM_CONFIDENCE_INJECTION=1
        F->>F: apply the gate
        Note over F: LOOM_STRONG_MATCH_SCORE 8.0 — at/above gives full budget<br/>and is the confidence denominator<br/>LOOM_MIN_INJECT_SCORE 2.0 — below skips injection entirely<br/>LOOM_MIN_INJECT_FRACTION 0.4 — weakest kept match's budget share
    else LOOM_CONFIDENCE_INJECTION=0 (default Profile B)
        F->>F: master switch OFF, thresholds pinned at loom-scaffold tuning.rs defaults
    end
    F->>F: clamp scaffold to ONTOLOGY_BUDGET (1500 tokens)
    alt DISTILL_BACKEND_URL set
        F->>B: delegate with the injected scaffold
        Note over F,B: LOOM_MIN_MAX_TOKENS = 1536 — reasoning backends<br/>truncate to EMPTY below this (the 400-to-empty trap)
        B-->>F: completion
        F-->>C: grounded completion
    else DISTILL_BACKEND_URL blank
        F-->>C: 503 — retrieval-only deployment
    end
    Note over C,B: BENCHMARK — static scaffold lifts grounded recall ~3.5x and<br/>is ~3-6x faster than cold parametric reasoning. Prose adds<br/>~nothing and was dropped as a default. Agentic tool-traversal<br/>is model-dependent (Gemma strong, Muse weak).
    Note over F: The four confidence vars are stated EXPLICITLY in compose so<br/>/health.injection_policy can be diffed against the file. The<br/>2026-09-02 dream cycle found a doc/deploy mismatch caused<br/>precisely by leaving them unstated (loom ADR-138).
```

## ES-06.6 Loom endpoint map — what needs a model and what does not
```mermaid
flowchart TB
    subgraph nomodel["Retrieval only — NO model required"]
        E1["GET /health<br/>liveness, corpus generation stamp,<br/>backend/graph/index readiness, injection_policy"]
        E2["GET /loom/generation<br/>the corpus generation identity being served"]
        E3["POST /loom/scaffold<br/>budget-clamped ontology grounding"]
        E4["POST /loom/sparql<br/>read-only clamped SPARQL over the reasoned closure"]
        E5["POST /loom/search<br/>label/substring search over the store"]
    end
    subgraph needsmodel["Delegates to DISTILL_BACKEND_URL"]
        E6["POST /v1/chat/completions<br/>scaffold-inject then delegate"]
        E7["GET /v1/models<br/>model identity passthrough"]
    end
    GEN["A full generation = scaffold-index.json + prose-index.json<br/>+ the TTLs + ontology-corpus.rvdb with its .generation.json"]
    T1["TRAP the empty floor — with an empty or mis-pointed<br/>LOOM_DATA_SOURCE the façade STILL starts and /health STILL<br/>returns 200, but the log reads lexical index NOT loaded ...<br/>empty floor. That is a staging bug, not a dead container.<br/>Check the mount before you check the process."]
    T2["The .rvdb is mounted read-only, but OPENING it mutates the<br/>redb file even for reads (the HNSW index is repacked on open),<br/>so the entrypoint copies it to a writable tmpfs at /run/loom.<br/>The tmpfs uid/gid must stay 65532 to match the image's<br/>non-root user or that copy fails EACCES."]
    D1["DIVERGENCE — the Rust image has NO mirror-on-start step.<br/>ONTOLOGY_SITE and LOOM_MIRROR_ON_START are GONE; the<br/>generation is staged by the operator and served immutably."]
    D2["DIVERGENCE — app/ontology-mcp/ (stdio MCP server, JavaScript)<br/>survived the Python deletion but now has NO build or run path<br/>from this repo. PRD-025/ADR-135 treat it as a thin client of<br/>the Loom index; placement is an open decision."]

    nomodel --> GEN
    GEN --> T1
    GEN --> T2
    GEN --> D1
    needsmodel --> D2
```

## ES-06.7 The black-hole hang — health green, every reasoning call stalls
```mermaid
sequenceDiagram
    autonumber
    participant A as agent
    participant G as email-mcp-gateway:8765
    participant H as GET /health
    participant DEAD as 192.168.2.48:8084<br/>HP's DEAD old LAN IP
    participant GOOD as 192.168.2.132:8084<br/>ml DNAT to the Loom façade

    rect rgb(240,230,230)
    Note over A,DEAD: BROKEN STATE — confirmed and fixed 10 Aug 2026
    A->>H: GET /health
    H-->>A: 200, container healthy on visionclaw_network,<br/>safeguard plus embedder ready
    A->>G: ask_email or refresh_inbox
    G->>DEAD: POST /v1/chat/completions (stale REASONER_BASE_URL)
    Note over G,DEAD: BLACK HOLE — packets are routed nowhere. Every<br/>synthesis stalls to timeout while /health still answers.
    DEAD--xG: no response
    G-->>A: 180 s timeout / whole-session tool loss
    end
    Note over A,DEAD: FINGERPRINT is exactly that split — health GREEN,<br/>all reasoning calls stall. The fix is the ROUTE, not<br/>the container. Do not restart the gateway.
    rect rgb(230,240,230)
    Note over A,GOOD: FIXED STATE
    A->>G: ask_email
    G->>GOOD: REASONER_BASE_URL=http 192.168.2.132:8084/v1
    GOOD-->>G: completion from the current loom-model
    G-->>A: answer
    end
    Note over GOOD: VERIFY — curl http 192.168.2.132:8084/v1/models returns<br/>the real model list (currently Qwen3.8-27B). Match it to the<br/>CURRENT backend rather than a fixed string, then recreate<br/>the container.
```

## ES-06.8 MCP client link loss and the mid-session JSON-RPC recovery
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code MCP client
    participant G as email-mcp-gateway:8765
    participant M as cold LAN model

    rect rgb(240,230,230)
    Note over CC,M: FAILURE — tools absent for the WHOLE session
    CC->>G: initialize handshake at session start
    G->>M: warm the backend
    M-->>G: slow (30s+ per call, SSE held open)
    Note over CC,G: The client disables an HTTP/SSE server for the ENTIRE<br/>session when the startup initialize exceeds MCP_TIMEOUT,<br/>and it does NOT retry HTTP servers after boot.
    G--xCC: handshake exceeds MCP_TIMEOUT
    CC-->>CC: server disabled for the session
    end
    Note over CC,G: DURABLE FIX (in the build, applied on next nix buildout) —<br/>MCP_TIMEOUT=60000 and MCP_TOOL_TIMEOUT=180000 set in<br/>flake.nix, tunable via [skills.email_search]<br/>.mcp_startup_timeout_ms / .mcp_tool_timeout_ms, plus a<br/>detached boot warm-up in config/entrypoint-unified.sh.
    rect rgb(230,240,230)
    Note over CC,G: MID-SESSION RECOVERY — verified 2026-07-06
    CC->>G: POST $AGENTBOX_EMAIL_GATEWAY_URL/mcp — initialize
    Note over CC,G: Authorization Bearer $AGENTBOX_EMAIL_GATEWAY_TOKEN<br/>Accept application/json, text/event-stream
    G-->>CC: response carrying the Mcp-Session-Id header
    CC->>G: notifications/initialized (with Mcp-Session-Id)
    CC->>G: tools/call
    G-->>CC: result
    end
    Note over CC,M: INVARIANT — the container itself never went down. Only<br/>the harness client link did, so driving the streamable-HTTP<br/>server directly over JSON-RPC restores the capability.
```

## ES-06.9 Backend-swap runbook — one port, stop-park-promote-start
```mermaid
stateDiagram-v2
    [*] --> Verify
    Verify --> StopCurrent
    StopCurrent --> ParkOld
    ParkOld --> PromoteNew
    PromoteNew --> StartNew
    StartNew --> Confirm
    Confirm --> [*]
    Confirm --> Rollback
    Rollback --> [*]

    note right of Verify
        Confirm the target's GGUF FIRST via
        docker inspect (MAIN_GGUF / ALIAS)
        so the right container is promoted.
        Reached over ssh john@10.10.10.1.
    end note
    note right of StopCurrent
        docker stop loom-model.
        Only ONE container can hold :8085.
        Alternates park as loom-model-NAMEbak (Exited).
    end note
    note right of PromoteNew
        docker rename loom-model loom-model-OLDbak
        then rename loom-model-NEWbak to loom-model.
    end note
    note right of Confirm
        curl :8084/v1/models shows the new alias and
        /health shows backend_reachable true, then
        smoke-test a real /v1/chat/completions.
        Reasoning models need max_tokens >= 2048
        or they return empty.
    end note
    note right of Rollback
        Reversible — the parked OLDbak restores
        the same way. Both carry
        restart-policy unless-stopped, so a
        manually-stopped alternate stays down
        and will not fight for the port.
    end note
    note right of StartNew
        GOTCHA HP's login shell is fish, so
        set -e in an SSH heredoc errors
        harmlessly. The docker lines still run.
    end note
```
