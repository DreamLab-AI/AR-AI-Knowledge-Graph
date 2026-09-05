---
id: AB-05
title: Manifest gate catalogue, vault path authority and the agentbox.sh CLI
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2003, ADR-2028, ADR-2029, ADR-2036, ADR-2037, ADR-2038, ADR-2039]
sources:
  - agentbox/management-api/lib/system-manifest.js
  - agentbox/management-api/routes/system.js
  - agentbox/agentbox.sh
  - agentbox/scripts/ruvector-sidecar-update.sh
  - agentbox/config/validate-artifacts.sh
  - agentbox/config/artifact-probes.json
  - agentbox/schema/agentbox.toml.schema.json
  - agentbox/agentbox.toml
verified_commit: b00c28a0d
---

## AB-05.1 GET /v1/system — catalogue plus live introspection
```mermaid
sequenceDiagram
    autonumber
    participant C as Operator or cockpit
    participant R as routes/system.js:33<br/>fastify.get /v1/system
    participant BV as buildSystemView<br/>system-manifest.js:274
    participant CAT as CATALOGUE const<br/>system-manifest.js:40-231
    participant ST as stateOf<br/>system-manifest.js:248
    participant RG as resolveGate<br/>system-manifest.js:234
    participant AD as resolved adapters

    C->>R: GET /v1/system
    R->>BV: buildSystemView(manifest, adapters) — routes/system.js:42
    BV->>BV: core = manifest entry :277 plus identity entry :281
    loop for slot of beads pods memory events orchestrator (system-manifest.js:285-286)
        BV->>AD: read adapter.impl and adapter.CONTRACT_VERSION
        AD-->>BV: impl or 'unresolved', contract_version or null (:290-291)
        BV->>BV: push core entry adapter-<slot> (:288-293)
    end
    BV->>BV: build resolved vault block (:301-319)
    loop for entry of CATALOGUE (system-manifest.js:323)
        BV->>ST: stateOf(manifest, entry)
        ST->>RG: resolveGate(manifest, entry.gate)
        RG-->>ST: gate value walked down the dotted path (:236-239)
        ST-->>BV: on | off | available
        BV->>BV: push to surfaces or modules by entry.layer (:335)
    end
    BV-->>R: apply_classes, core, vault, surfaces, modules, counts (:338-351)
    R-->>C: live system view
    Note over BV,CAT: INVARIANT — the catalogue is documentation-as-data but STATE is always introspected from the parsed agentbox.toml at request time, never hard-coded (system-manifest.js:328)
    Note over BV: counts block emits core, surfaces_on, surfaces, modules_on, modules (:344-350)
    Note over BV,AD: DOC-DRIFT — the adapter-slot summary at system-manifest.js:292 repeats "observability → privacy → JSON-LD". Only two layers are in the wrap chain — see AB-04.4
    Note over BV,AD: RESOLVED ADR-2036: system-manifest.js:292 now reads<br/>"every dispatch wrapped by observability → privacy redaction<br/>(ADR-2036). JSON-LD encoding is a per-surface gated stage<br/>invoked by the owning route, not a dispatch layer" — see AB-04.4
    Note over R: sibling route GET /v1/system/audit-chain verifies the hash-chained events JSONL (routes/system.js:47)
```

## AB-05.2 Catalogue shape and the real surface/module census
```mermaid
flowchart TD
    CAT["CATALOGUE array<br/>system-manifest.js:40-231<br/>60 entries total"] --> SURF["layer 'surface' — 13 entries"]
    CAT --> MOD["layer 'module' — 47 entries"]
    SURF --> S1["ungated: management-api, terminal, setup-wizard,<br/>uri-resolver, agent-events-stream, metrics<br/>system-manifest.js:41-71"]
    SURF --> S2["gated: code-server, jupyter, desktop, comfyui,<br/>linked-data-viewer, interaction-plane, tab0-bridge<br/>system-manifest.js:50-79"]
    MOD --> M1["toolchain + CLI: ruflo, agentic-qe, nagual-qe, deepsec,<br/>codebase-memory, metaharness, codex, opencode, rust-toolchain"]
    MOD --> M2["GPU/media: qgis-mcp, blender-mcp, imagemagick-mcp,<br/>ffmpeg, pytorch, cuda, gaussian-splatting"]
    MOD --> M3["sidecars flagged heavy: browser-sidecar, gui-tools-sidecar,<br/>voice-console, privacy-filter"]
    MOD --> M4["sovereign: sovereign-mesh, solid-pod, linked-data,<br/>payments, llm-marketplace, project-tracking, consultants"]
    MOD --> M5["memory: ruvector-external, memory-learning,<br/>memory-hygiene, ruvnet-brain, compression"]
    MOD --> M6["corpus: vault :216, vault-tui :219, ontology"]
    CAT --> FIELDS["per-entry fields: id, name, layer, gate or gates,<br/>service, apply_class, summary, heavy"]
    FIELDS --> CORE["core layer emitted separately by buildSystemView<br/>manifest, identity, five adapter-<slot> entries<br/>system-manifest.js:275-294"]
    CAT -.-> DRIFT["DOC-DRIFT — BASELINE-container says the catalogue holds<br/>14 surfaces plus about 35 modules.<br/>Verified census is 13 surfaces and 47 modules<br/>system-manifest.js grep layer counts"]
    CAT -.-> RES["RESOLVED ADR-2039: BASELINE-container.md:109 now says<br/>60 entries = 13 surfaces + 47 modules"]
```

## AB-05.3 stateOf — how a gate value becomes a state word
```mermaid
flowchart TD
    E["catalogue entry"] --> A{"Array.isArray(entry.gates)?<br/>system-manifest.js:251"}
    A -->|yes| MG["resolve every gate path"]
    MG --> MG1{"any value === true?"}
    MG1 -->|yes| ON1["state 'on' — :253"]
    MG1 -->|no| MG2{"any value === false?"}
    MG2 -->|yes| OFF1["state 'off' — :254"]
    MG2 -->|no| AV1["state 'available' — :255"]
    A -->|no| B{"entry.gate falsy?<br/>system-manifest.js:257"}
    B -->|yes| ON2["state 'on' — ungated surface,<br/>present whenever the image is"]
    B -->|no| RG["resolveGate(manifest, entry.gate)<br/>system-manifest.js:234"]
    RG --> W["walk the dotted path key by key<br/>system-manifest.js:236-239"]
    W --> SEC{"cursor is an object?<br/>system-manifest.js:241"}
    SEC -->|yes| SECE["section gate resolves via its .enabled key — :242"]
    SEC -->|no| VAL["scalar value — :244"]
    SECE --> D
    VAL --> D{"value type"}
    D -->|"true"| ON3["state 'on' — :259"]
    D -->|"false"| OFF2["state 'off' — :260"]
    D -->|"string"| MODE{"value === 'off' or 'none'?<br/>system-manifest.js:265"}
    D -->|"undefined"| AV2["state 'available' — catalogued<br/>but unconfigured — :266"]
    MODE -->|yes| OFF3["state 'off'"]
    MODE -->|no| ON4["state 'on'"]
    MODE -.-> VT["ADR-2029 — vault.tui is a mode string naming a thing<br/>not a state, so 'none' is the conventional disabled value.<br/>Vanilla default tui = 'none' reports vault-tui as off"]
```

## AB-05.4 [vault] — the single corpus path authority, catalogued as two entries
```mermaid
flowchart TD
    TOML["agentbox.toml [vault]<br/>root required, pages, format, tui,<br/>working, transcripts"] --> SCHEMA["schema/agentbox.toml.schema.json<br/>root is required"]
    TOML --> E1["catalogue entry 'vault'<br/>gate vault.format — apply_class BOOT<br/>system-manifest.js:216-218"]
    TOML --> E2["catalogue entry 'vault-tui'<br/>gate vault.tui — apply_class REBUILD<br/>system-manifest.js:219-221"]
    E1 --> WHY1["root/pages/format are read ONCE by the entrypoint<br/>at container start — a restart picks them up"]
    E2 --> WHY2["tui decides the Nix package set (ADR-2029)<br/>none to rune needs ./agentbox.sh rebuild, not a restart"]
    E1 --> SPLIT["ADR-039 honesty rule — one entry claiming 'boot' for both<br/>would tell an operator that flipping tui and restarting<br/>gets them the Rune TUI. It does not.<br/>system-manifest.js:210-215"]
    E2 --> SPLIT
    TOML --> VB["resolved vault block from buildSystemView<br/>system-manifest.js:301-319"]
    VB --> VB1["enabled = Boolean(root) — :305"]
    VB --> VB2["pages = root minus trailing slashes + '/' + pages default 'pages' — :307"]
    VB --> VB3["format default 'obsidian' when root set — :308"]
    VB --> VB4["tui default 'none' when root set — :309"]
    VB --> VB5["ADR-2028 amendment 2026-09-02 — working_root, working_pages,<br/>transcripts sibling-vault keys — :311-313"]
    VB --> VB6["env_root, env_pages, env_working_pages, env_transcripts<br/>read from the process the container actually booted with — :314-317"]
    VB6 --> DRIFT["drift = root set AND VAULT_ROOT set AND they differ<br/>system-manifest.js:318"]
    DRIFT --> DOC["so /v1/system and the doctor can show manifest-vs-running drift"]
    TOML -.-> DIV["DIVERGENCE — BASELINE 'Vault compatibility and Notes qualification 2026-09-04'.<br/>ADR-2028 is partial for universal disablement: the no-vault resolver clears<br/>VAULT_PAGES but retains a legacy ONTOLOGY_PAGES_DIR override consumers prefer.<br/>See AB-02 for the entrypoint resolution path"]
```

## AB-05.5 agentbox.sh top-level subcommand dispatch
```mermaid
flowchart LR
    ARG["arg parse loop<br/>agentbox.sh:1760-1776"] --> AL{"subcommand in the<br/>allowlist at agentbox.sh:1770?"}
    AL -->|no| ERR["Unknown command then usage then exit 1<br/>agentbox.sh:1776-1778"]
    AL -->|yes| CASE["execute case CMD<br/>agentbox.sh:2075-2103"]
    CASE --> G1["access: ssh :2071, vnc :2072, browser :2073,<br/>code :2074, api :2075, all :2076, ip :2078"]
    CASE --> G2["lifecycle: up :2084, down :2085, build :2086,<br/>rebuild :2087, update :2088, status :2077"]
    CASE --> G3["provisioning: provision :2079, setup :2080,<br/>start-browser :2081, migrate-workspace :2100, preflight :2101"]
    CASE --> G4["data: backup :2082, restore :2083,<br/>ruvector :2089, ruvnet-brain :2090"]
    CASE --> G5["observe: logs :2091, shell :2092, health :2093"]
    CASE --> G6["sidecars: browsercontainer :2094, gui-tools :2095,<br/>openmed :2096, voice :2097, xr-runtime :2098, android :2099"]
    G2 --> RB["cmd_rebuild agentbox.sh:1042<br/>= cmd_down then cmd_build --variant runtime<br/>then cmd_up --build then post-deploy-cleanup.sh"]
    G4 --> RV["cmd_ruvector agentbox.sh:999<br/>exec bash scripts/ruvector-sidecar-update.sh — see AB-05.6"]
    G5 --> HL["cmd_health agentbox.sh:1108 — see AB-05.7"]
    G5 --> SH["cmd_shell agentbox.sh:1091"]
    SH -.-> DIV1["DIVERGENCE — cmd_shell agentbox.sh:1104 execs into<br/>cd /workspace/profiles/PROFILE. agentbox/CLAUDE.md 'Runtime model gotchas'<br/>states the literal path /workspace is retired and will break.<br/>Every other profile path in the same script uses SCRIPT_DIR/workspace/profiles<br/>agentbox.sh:348 and :511"]
    SH -.-> RES1["RESOLVED ADR-2038: cmd_shell agentbox.sh:1104 now uses<br/>cd /home/devuser/workspace/profiles/${profile} && exec fish"]
```

## AB-05.6 agentbox.sh ruvector — a dispatch table split across two files
```mermaid
sequenceDiagram
    autonumber
    participant OP as operator
    participant AB as cmd_ruvector<br/>agentbox.sh:999
    participant SC as scripts/ruvector-sidecar-update.sh<br/>dispatch :1172-1189
    participant H as scripts/ruvector-recall-harness.mjs

    OP->>AB: ./agentbox.sh ruvector <subcmd> [args]
    AB->>SC: exec bash SCRIPT_DIR/scripts/ruvector-sidecar-update.sh "$@" (agentbox.sh:1006)
    Note over AB: image pin lives in agentbox.toml [integrations.ruvector_external] and is mirrored into docker-compose.yml (agentbox.sh:1001-1004)
    alt status | check | test (ruvector-sidecar-update.sh:1173-1175)
        SC-->>OP: sidecar state, pinned-vs-Docker-Hub comparison
    else update (:1176)
        SC->>SC: dump then pg_basebackup snapshot then candidate rehearsal then swap
    else rollback (:1177)
        SC->>SC: restore previous image plus datadir from the recorded snapshot
    else migrate-trajectories | repair-namespaces | backfill-embeddings | archive-legacy | aggregate-effectiveness | build-metadata-gin (:1178-1183)
        SC-->>OP: DRY-RUN by default — each needs --yes plus its manifest flag
    else recall (:1184)
        SC->>SC: cmd_recall (:1146) — require_prod_running then node present
        SC->>SC: resolve governed MCP env via mcp_env_pairs from .mcp.json (:1156)
        SC->>H: env ENVP node ruvector-recall-harness.mjs "$@" (:1167)
        H-->>OP: harness sets its own exit code — PASS 0, FAIL non-zero (:1161)
    else anything else
        SC-->>OP: die unknown subcommand (:1188)
    end
    Note over SC,H: recall is READ-ONLY, no gate of its own — fixture scripts/recall-fixtures/recall-fixture.v1.json is frozen and checked in (:1159)
    Note over H: classes self-recall@10, true-recall@10 vs forced exact scan, exact-token — median-of-3 no-regression band (:1160-1161)
    Note over H: artefact lands in backups/ruvector-sidecar/recall-runs/<utc>.json (:1162) — retrieval-geometry gate boundary, see AB-20
    Note over AB,SC: DOC-DRIFT — the usage text at agentbox.sh:48 lists the ruvector subcommands but OMITS recall, which ruvector-sidecar-update.sh:1184 implements and :1188 advertises
    Note over AB,SC: RESOLVED ADR-2038: recall is in the ruvector subcommand list at<br/>agentbox.sh:48 and has a usage example at agentbox.sh:94
```

## AB-05.7 agentbox.sh health — exit-code contract
```mermaid
sequenceDiagram
    autonumber
    participant OP as operator
    participant SH as cmd_health<br/>agentbox.sh:1108
    participant H as GET /health<br/>server.js:564-575
    participant M as GET /v1/meta<br/>localhost:9090

    OP->>SH: ./agentbox.sh health [--json]
    SH->>H: curl -sf HEALTH_URL (agentbox.sh:605 and :1118)
    alt curl fails
        SH-->>OP: ERROR could not reach — exit 1 (agentbox.sh:1119-1121)
    else --json passed
        SH-->>OP: raw JSON then exit 0 (agentbox.sh:1125-1126)
    else jq absent
        SH-->>OP: warning plus raw response (agentbox.sh:1188-1189)
    else pretty path
        SH->>SH: degraded = jq '.adapters // {} | select(.value != healthy and != off)' (agentbox.sh:1134-1138)
        SH->>SH: degraded_count = jq '.degraded_count // 0' (agentbox.sh:1139)
        SH->>SH: print adapter/<slot> lines from .adapters (agentbox.sh:1143-1146)
        SH->>M: curl /v1/meta then read observability.metrics_endpoint (agentbox.sh:1161-1163)
        M-->>SH: metrics endpoint
        SH->>SH: print first 5 non-comment metric lines (agentbox.sh:1174)
        alt degraded non-empty OR degraded_count > 0
            SH-->>OP: exit 1 (agentbox.sh:1179)
        else
            SH-->>OP: exit 0
        end
    end
    Note over SH,H: DIVERGENCE — /health emits status, uptime, image_hash, manifest_checksum, adapters, degraded_count, note (server.js:566-574). There is NO services key, so jq '.services // {}' at agentbox.sh:1133 is always empty and the exit-1 branch at :1174 is unreachable
    Note over SH,H: RESOLVED ADR-2037: cmd_health now derives failure from .adapters<br/>(a slot fails when neither "healthy" nor "off", agentbox.sh:1134-1138)<br/>plus .degraded_count (agentbox.sh:1139) — exit 1 at agentbox.sh:1179<br/>is reachable. Same fix as AB-04.16
    Note over SH: BASELINE-container Adapter spine stage 4 claims agentbox.sh health exits non-zero if any slot gauge is 0 — it never reads the agentbox_adapter_health gauge at all. See AB-04.16
    Note over H: /health self-describes as human-inspection-only and points orchestrators at /ready (server.js:573)
```

## AB-05.8 Artifact validation gate — the last check before exec supervisord
```mermaid
sequenceDiagram
    autonumber
    participant EP as config/entrypoint-unified.sh
    participant VA as config/validate-artifacts.sh
    participant PF as config/artifact-probes.json<br/>16 probes
    participant SH as probe_command shell

    EP->>VA: invoked immediately before exec supervisord (validate-artifacts.sh:11)
    VA->>VA: set -euo pipefail (:13)
    VA->>PF: read AGENTBOX_PROBES_FILE default /opt/agentbox/config/artifact-probes.json (:15)
    alt probes file missing
        VA-->>EP: log ProbesFileMissing then exit 1 (:32-34)
    end
    alt jq absent
        VA-->>EP: log MissingDependency tool=jq then exit 1 (:37-40)
    end
    loop for each probe entry
        VA->>SH: run probe_command
        alt exit 0
            SH-->>VA: pass
        else required_for_readiness true
            SH-->>VA: fail — validate-artifacts.sh exits 1 (:5)
        else optional
            SH-->>VA: warn and continue (:6)
        end
    end
    VA-->>EP: pino-style JSON lines on stdout with agentbox.stage bootstrap (:8-9 and :26-30)
    Note over PF: probe fields capability_id, entrypoint_path, required_for_readiness, probe_command
    Note over PF: only 2 of 16 are required_for_readiness — management-api and mcp-nostr-bridge, both node --check syntax gates
    Note over PF: optional probes cover openai-codex-mcp, lazy-fetch-mcp, browser-sidecar HTTP health, ruflo-cli, claude-flow-cli, native-sqlite-backend, self-learning-hook-adapter, agentic-qe-cli, nagual-qe-cli, codebase-memory-mcp-cli, mermaid-cli, code-interpreter-mcp, code-interpreter-wheelhouse, aci-shell-mcp
    Note over VA,SH: the gate is a SYNTAX and PRESENCE check, not a behavioural one — node --check and --version dominate
```

## AB-05.9 Apply-class as an operator decision procedure
```mermaid
stateDiagram-v2
    [*] --> EditManifest
    EditManifest --> Validate: agentbox-config-validate.js plus schema/agentbox.toml.schema.json
    Validate --> Rejected: structural schema violation
    Validate --> Classify: valid manifest
    Rejected --> EditManifest

    Classify --> Live: apply_class live
    Classify --> Boot: apply_class boot
    Classify --> Rebuild: apply_class rebuild

    Live --> Effective: read at operation time, no restart
    Boot --> RestartNeeded: entrypoint reconciles every boot
    RestartNeeded --> Effective: docker restart or agentbox.sh up
    Rebuild --> RebuildNeeded: changes the Nix image composition
    RebuildNeeded --> Effective: agentbox.sh rebuild at agentbox.sh:1041

    Effective --> Verify: GET /v1/system shows state and apply_class
    Verify --> [*]

    note right of Live
        APPLY_CLASSES system-manifest.js:28
        example browser-sidecar
    end note
    note right of Boot
        APPLY_CLASSES system-manifest.js:29
        example vault gate vault.format
    end note
    note right of Rebuild
        APPLY_CLASSES system-manifest.js:30
        example vault-tui gate vault.tui ADR-2029
        also code-server, jupyter, desktop, comfyui
    end note
    note left of Classify
        INVARIANT adding a gate means gating BOTH the Nix
        package set AND the supervisor block, plus a
        catalogue entry with an honest apply class
    end note
```

## AB-05.10 Schema surfaces and the setup wizard boundary
```mermaid
flowchart TD
    S1["agentbox/schema/agentbox.toml.schema.json<br/>the only file in schema/"] --> V["scripts/agentbox-config-validate.js<br/>static stage — see AB-01.4"]
    S2["agentbox/schemas/mcp/<br/>MCP registry schemas"] --> P["skills/mcp.json projector — see AB-09"]
    V --> GATE["valid gate set feeds the flake evaluator — see AB-01.1"]
    S1 --> SW["setup-wizard catalogue entry<br/>system-manifest.js:47-49<br/>service 'setup', apply_class boot"]
    SW --> SWB["ephemeral localhost manifest editor with schema validation<br/>EXITS AFTER SAVING"]
    SWB -.-> DIV["DIVERGENCE BASELINE Known divergences — operations moved to the<br/>AoE cockpit. Legacy docs describing pseudo-user isolation<br/>gemini-user and friends are dead paths, not the runtime model"]
    S1 --> VS["[vault] keys root required, pages, format, tui<br/>plus ADR-2028 amendment working and transcripts"]
    VS --> AB4["consumed by buildSystemView vault block — see AB-05.4"]
    GATE --> MAN["/v1/system catalogue — see AB-05.1"]
```
