---
id: AB-09
title: MCP registry, boot projector and the server catalogue
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2008, ADR-2003, ADR-2039]
sources:
  - agentbox/skills/mcp.json
  - agentbox/mcp/mcp.json
  - agentbox/scripts/project-mcp-servers.mjs
  - agentbox/config/entrypoint-unified.sh
  - agentbox/mcp/aci-shell/server.js
  - agentbox/mcp/ruvnet-brain/server.js
  - agentbox/mcp/servers/ontology-bridge.js
  - agentbox/mcp/servers/harness-bridge.js
  - agentbox/mcp/servers/precedent-bridge.js
  - agentbox/mcp/servers/governance-bridge.js
  - agentbox/mcp/servers/decision-tools.js
  - agentbox/mcp/servers/substrate-tools.js
  - agentbox/mcp/servers/ontology-propose.js
  - agentbox/mcp/servers/ruvector-mcp.cjs
verified_commit: b00c28a0d
---

## AB-09.1 Registry ownership classes — what the projector may touch
```mermaid
flowchart TD
    REG["skills/mcp.json v2.0.0<br/>CANONICAL BOOT-PROJECTION SOURCE<br/>28 servers total"] --> CLS{"x-agentbox-managed-by<br/>MANAGED_BY_VALUES project-mcp-servers.mjs:101"}
    CLS -->|projector — 9| PJ["aci-shell, code-interpreter, codebase-memory,<br/>consultant-codex, consultant-antigravity, consultant-zai,<br/>consultant-perplexity, consultant-deepseek, web-researcher"]
    CLS -->|bespoke — 3| BS["claude-flow, browser-gpu, perplexity"]
    CLS -->|reference — 16| RF["ruv-swarm, imagemagick, qgis, web-summary, comfyui, blender,<br/>gemini-url-context, notebooklm, linkedin, defense-security, reddit,<br/>meta-xr-sdk, unreal-engine, clipcannon, scrapling, context7"]
    PJ --> ACT["gate-evaluated, requires-checked, ${VAR} expanded,<br/>UPSERTED into .mcp.json — reconcile not append<br/>project-mcp-servers.mjs:13-20"]
    BS --> NEVER["hand-written entrypoint blocks with health probes,<br/>secret handling and warmup — NEVER touched here, so the<br/>live set stays byte-identical (project-mcp-servers.mjs:22-24)"]
    RF --> DOC["GPU-sidecar skill wrappers whose mcp-server lives under a skill dir,<br/>or npx/uvx network-installer servers that cannot run on the<br/>read-only rootfs — documented, not auto-projected (:25-27)"]
    REG -.-> D1["DOC-DRIFT — BASELINE-container says skills/mcp.json is a 30-server registry.<br/>The file holds 28 (9 projector + 3 bespoke + 16 reference).<br/>The 9/3/16 split the doc gives is correct"]
    REG -.-> D1R["RESOLVED ADR-2039: BASELINE-container.md:8 and :227 now say<br/>skills/mcp.json holds 28 servers, not 30"]
    REG -.-> D2["separate file agentbox/mcp/mcp.json is a DIFFERENT 17-entry map<br/>and is not the projection source"]
```

## AB-09.2 Gate and requirement grammar
```mermaid
flowchart TD
    E["projector-managed entry"] --> G["gateOpen(gate)<br/>project-mcp-servers.mjs:234"]
    G --> G0{"gate falsy or 'never'?<br/>:235"}
    G0 -->|yes| CLOSED["false — documentation-only entry"]
    G0 -->|no| G1{"gate === 'requires'?<br/>:236"}
    G1 -->|yes| OPEN["true — gate rests ENTIRELY on x-agentbox-requires.<br/>Package-gated: the nix build only bakes the binary<br/>when its manifest gate is on (:231-232)"]
    G1 -->|no| G2["split on ':' into kind and name — :237"]
    G2 --> G3{"kind === 'envset'?<br/>:239"}
    G3 -->|yes| G3A["true when env VAR is non-empty"]
    G3 -->|no| G4{"kind === 'env'?<br/>:240"}
    G4 -->|yes| G4A["true when VAR lowercased is true, 1, yes or on"]
    G4 -->|no| CLOSED
    OPEN --> R["requiresMet(reqs)<br/>project-mcp-servers.mjs:248"]
    G3A --> R
    G4A --> R
    R --> R0{"reqs undefined or null?<br/>:249"}
    R0 -->|yes| ROK["ok true, explicit false — the EMPTY requirement set"]
    R0 -->|no| R1{"reqs is an Array?<br/>:250"}
    R1 -->|no| RBAD["ok false — x-agentbox-requires is not an array"]
    R1 -->|yes| R2["for each requirement"]
    R2 --> RB{"r.bin — any of the names on PATH?<br/>:252-254"}
    RB -->|no| RF1["ok false, why missing bin A|B"]
    R2 --> RFI{"r.file — exists after ${VAR} expansion?<br/>:256"}
    RFI -->|no| RF2["ok false, why missing file PATH"]
    R2 --> RE{"r.envset — env var set and non-empty?<br/>:257"}
    RE -->|no| RF3["ok false, why unset VAR"]
    RB -->|yes| ROK2["ok true, explicit true"]
    RFI -->|yes| ROK2
    RE -->|yes| ROK2
    ROK -.-> D4["ADR-2008 closeout D4 — requiresMet is now TOTAL and always returns {ok, why}.<br/>Previously it returned a bare boolean true for a non-array, so req.ok was undefined,<br/>falsy, and the gated-ON server was reconciled OUT (project-mcp-servers.mjs:51-55)"]
    ROK2 --> WRITE["expand ${VAR} and ${VAR:-default} in command, args, env and headers (:18)"]
```

## AB-09.3 Boot projection — reconcile with an ownership ledger
```mermaid
sequenceDiagram
    autonumber
    participant EP as config/entrypoint-unified.sh
    participant P as project-mcp-servers.mjs
    participant REG as MCP_REGISTRY<br/>default SKILLS_TREE/mcp.json
    participant LED as ownership ledger<br/>MCP_PROJECTION_STATE
    participant T as MCP_JSON target<br/>default WORKSPACE/.mcp.json

    EP->>P: run the projector, piped through sed and appended with || true
    P->>REG: read registry
    alt unreadable, unparseable or schema-invalid
        P->>P: validateRegistry() enumerates every failure before anything is written (:43-44)
        P-->>EP: FAIL, exit 2 — target untouched (:73)
    end
    P->>T: read current target
    alt target unreadable or unparseable
        P-->>EP: FAIL, exit 3 (:74)
    end
    P->>LED: readLedger() (:268)
    alt ledger file missing
        LED-->>P: emptyLedger — first run or ledger removed (:271)
    else ledger unparseable
        LED-->>P: WARN then emptyLedger — a corrupt ledger must not licence deleting live entries (:275-278)
    else parsed
        LED-->>P: owned map and bounded history (:281-284)
    end
    loop for each projector-managed registry definition
        P->>P: gateOpen then requiresMet
        alt both pass
            P->>T: UPSERT the expanded definition
            P->>LED: record the name and its definition hash as owned
        else either fails
            P->>T: REMOVE the entry — closing the add-only rot of MCP-6 (:19-20)
        end
    end
    loop for each name the ledger says this projector owns
        alt no longer a projector-managed registry definition
            P->>T: REMOVE it
            P->>LED: append to the deletion history (:36-39)
        end
    end
    P->>T: atomicWriteJson — temp in the SAME directory, fsync, rename(2), preserving mode (:66-69 and :299)
    P->>LED: writeLedger with mode 0600, history capped at HISTORY_LIMIT 200 (:287-293 and :100)
    P-->>EP: exit 0 — projection applied or a clean no-op (:72)
    Note over P,T: INVARIANT — a target entry ABSENT from the ledger is treated as bespoke and is never<br/>touched, so adopting this revision cannot delete a hand-written server (:61-64)
    Note over LED: the ledger is deliberately NOT stored inside .mcp.json — that file is read by the Claude Code harness and must carry no agentbox-private keys (:59-61)
    Note over EP,P: boot is NOT blocked — a non-zero exit surfaces as a loud [mcp] FAIL line in the boot log without aborting the entrypoint (:76-79)
    Note over P: DOC-DRIFT — BASELINE "Configuration projection qualification 2026-09-04" says<br/>ADR-2008 is partial because the reconciliation loop cannot remove deleted registry<br/>definitions and unreadable input leaves stale state with exit zero. The ADR-2008<br/>closeout dated 2026-09-05 fixes both as D1 and D3 (project-mcp-servers.mjs:30-49)
    Note over P: RESOLVED ADR-2039: BASELINE-container.md:193 marks this qualification<br/>resolved with the D1/D3 evidence — ownership-ledger removal (:33-39)<br/>and non-zero exit on malformed input (:46-49, exit codes :71-74)
```

## AB-09.4 The four ADR-2008 closeout defects and their fixes
```mermaid
flowchart LR
    subgraph OLD["previous revision — reproduced by the estate review"]
        O1["D1 deleted definition leaked —<br/>the loop only iterated the REGISTRY, so a managed entry whose<br/>definition was deleted or renamed stayed in .mcp.json forever"]
        O2["D2 no schema validation —<br/>any JSON shape accepted, a mistyped args string or bogus gate<br/>silently projected a broken server"]
        O3["D3 malformed input exited ZERO —<br/>an unparseable registry logged a line and exited 0,<br/>so a truncated file looked like a successful no-op"]
        O4["D4 missing x-agentbox-requires REMOVED the entry —<br/>requiresMet returned bare true for a non-array,<br/>req.ok undefined, falsy, gated-ON server reconciled OUT"]
    end
    subgraph NEW["closeout 2026-09-05"]
        N1["ownership ledger — projection records which names it owns,<br/>any owned name no longer projector-managed is removed<br/>and recorded in the deletion history (:36-39)"]
        N2["validateRegistry() — a total explicit schema check whose<br/>failures are ENUMERATED before anything is written (:43-44 and :161)"]
        N3["malformed input exits NON-ZERO and the previous target<br/>is retained byte-for-byte (:46-49)"]
        N4["requiresMet is TOTAL, always returns {ok, explicit, why}<br/>an absent array is the empty requirement set (:51-55 and :248)"]
    end
    O1 --> N1
    O2 --> N2
    O3 --> N3
    O4 --> N4
    N2 --> V["validated keys — x-agentbox-managed-by in projector|bespoke|reference (:174-176)<br/>x-agentbox-gate matching requires|never|env:VAR|envset:VAR (:180-182)<br/>x-agentbox-requires an array of objects keyed bin, file or envset only (:185-203)"]
    N3 --> EC["EXIT CODES 0 applied or clean no-op, 2 registry bad target untouched,<br/>3 target unreadable or atomic replacement failed (:71-74)"]
```

## AB-09.5 In-image MCP servers and their tool surfaces
```mermaid
flowchart TB
    subgraph CODE["code-as-harness"]
        A["aci-shell mcp/aci-shell/server.js:561<br/>server name aci-shell v0.1.0"]
        A --> AT["aci.view_file :495 — bounded window, hard cap 150 lines<br/>aci.edit_file :508 — atomic tmp/fsync/rename, compact unified diff<br/>aci.search_repo :522 — rg preferred, grep fallback, reports total_found<br/>aci.run_tests, aci.submit — TOOL_LIST :493"]
    end
    subgraph ONT["ontology and knowledge graph"]
        B["ontology-bridge mcp/servers/ontology-bridge.js"]
        B --> BT["ontology_ask, ontology_search, ontology_validate, ontology_health,<br/>ontology_class_get, ontology_class_list, ontology_graph_query,<br/>ontology_axiom_add, kg_neighbors, kg_node_search, kg_pathfind"]
        C["ontology-propose mcp/servers/ontology-propose.js"] --> CT["ontology_propose — governed write path, see AB-25"]
    end
    subgraph GOV["governance and decisions"]
        D["governance-bridge"] --> DT["governance_list_decisions, governance_publish_panel,<br/>governance_request_action, governance_retire_panel, governance_update_panel"]
        E["decision-tools"] --> ET["record_decision, find_similar_decisions, trace_decision_chain,<br/>analyze_decision_impact, check_decision_rules"]
        F["precedent-bridge"] --> FT["precedent_list, precedent_match, precedent_promote, precedent_retire"]
    end
    subgraph HARN["harness and substrate"]
        G["harness-bridge"] --> GT["harness_audit, harness_inspect, harness_list, harness_validate"]
        H["substrate-tools"] --> HT["refine, refine_history, refine_list, refine_rollback, refine_validate,<br/>spawn_child, spawn_complete, spawn_ready,<br/>ws_drop, ws_get, ws_list, ws_note, ws_revalidate"]
    end
    subgraph KB["corpus and memory"]
        I["ruvnet-brain mcp/ruvnet-brain/server.js"] --> IT["search_ruvnet, ruvnet_brain_status"]
        J["ruvector-mcp.cjs — 26 tools"] --> JT["memory_store, memory_search, memory_retrieve, memory_list, memory_usage,<br/>memory_health, memory_orient, memory_hybrid_search, memory_sweep_episodic,<br/>memory_repair_embeddings, swarm_init, swarm_status, agent_spawn,<br/>task_orchestrate, coordination_sync, load_balance, parallel_execute,<br/>neural_patterns, sona_health, sparc_mode, performance_report,<br/>bottleneck_analyze, workflow_create, workflow_execute,<br/>github_pr_manage, github_repo_analyze"]
    end
    J -.-> RVB["memory boundary — the retrieval geometry, embedding pipeline and<br/>recall gate belong to AB-20. ruvector-mcp.cjs fails CLOSED with no<br/>sql.js fallback, so the ruvector-postgres sidecar is mandatory"]
    A -.-> AB["consultant tier consultant-codex, consultant-antigravity, consultant-zai,<br/>consultant-perplexity, consultant-deepseek all live under<br/>mcp/consultants/ and are projector-managed"]
```

## AB-09.6 A projected consultant entry, field by field
```mermaid
flowchart TD
    K["skills/mcp.json entry consultant-codex"] --> C1["command sh"]
    K --> C2["args -c with AGENTBOX_CODEX_BIN=$(command -v codex)<br/>AGENTBOX_CODEX_HOME=$HOME/.codex<br/>exec node /opt/agentbox/mcp/consultants/package/codex/server.js"]
    K --> C3["type stdio, protocol stdio, version 0.1.0"]
    K --> C4["env AGENTBOX_CODEX_MODEL = ${AGENTBOX_CODEX_MODEL:-gpt-5.4}<br/>expanded by the projector before writing"]
    K --> C5["x-agentbox-managed-by projector"]
    K --> C6["x-agentbox-gate env:AGENTBOX_CONSULTANTS_ENABLED"]
    K --> C7["x-agentbox-requires bin codex AND<br/>file /opt/agentbox/mcp/consultants/package/codex/server.js"]
    C6 --> EV["gateOpen — true only when AGENTBOX_CONSULTANTS_ENABLED<br/>is true, 1, yes or on (project-mcp-servers.mjs:240)"]
    C7 --> RQ["requiresMet — codex must be on PATH and the server file must exist,<br/>so a server whose binary was GC'd or whose key is unset is never<br/>registered as a dead entry (:15-17 and :252-257)"]
    EV --> UP{"both satisfied?"}
    RQ --> UP
    UP -->|yes| WR["UPSERT into .mcp.json with ${VAR} expanded"]
    UP -->|no| RM["REMOVE from .mcp.json if present"]
    C3 -.-> DESC["description carries the tool list — consult, health, cost_estimate —<br/>and notes it inherits OPENAI_API_KEY from session env"]
```

## AB-09.7 A tool call through the harness to a projected server
```mermaid
sequenceDiagram
    autonumber
    participant M as model
    participant H as Claude Code harness
    participant J as .mcp.json<br/>projected at boot
    participant S as MCP server process<br/>stdio transport
    participant B as backing resource

    H->>J: read server definitions at session start
    J-->>H: command, args, env, type stdio
    H->>S: spawn the server process with the expanded env
    S-->>H: initialize response
    H->>S: ListToolsRequestSchema handler (mcp/aci-shell/server.js:565)
    S-->>H: tools TOOL_LIST (mcp/aci-shell/server.js:493)
    H-->>M: tool names surfaced as mcp__<server>__<tool>
    M->>H: call mcp__aci-shell__aci_view_file with path and start_line
    H->>S: CallToolRequestSchema (mcp/aci-shell/server.js:32)
    S->>S: validate path is under ACI_WORKSPACE_ROOT (server.js:496)
    alt path escapes the workspace root
        S-->>H: error, refused
    else path accepted
        S->>B: read a bounded window, hard cap 150 lines (server.js:502)
        B-->>S: content
        S-->>H: content, total line count, truncation flag
    end
    H-->>M: tool result
    Note over S: aci-shell records agentbox_aci_calls_total and agentbox_aci_duration_ms — the server is instrumented like the adapter spine
    Note over H,J: a server whose gate or requires failed at boot is ABSENT from .mcp.json, so the harness never offers its tools rather than offering a dead entry
    Note over M,H: bespoke servers claude-flow, browser-gpu and perplexity reach the harness through hand-written entrypoint blocks, not through this projection
```
