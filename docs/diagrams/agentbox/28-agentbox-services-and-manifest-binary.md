---
id: AB-28
title: Agentbox service crates and the manifest binary
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2030, ADR-2031, ADR-2032]
sources:
  - agentbox/services/agentbox-manifest/src/main.rs
  - agentbox/services/agentbox-manifest/src/mcp.rs
  - agentbox/services/agentbox-manifest/src/routing.rs
  - agentbox/services/agentbox-manifest/src/proxy.rs
  - agentbox/services/agentbox-manifest/src/stacks.rs
  - agentbox/services/agentbox-manifest/src/plugins.rs
  - agentbox/services/agentbox-manifest/src/tui_read.rs
  - agentbox/services/agentbox-manifest/src/tui_write.rs
  - agentbox/services/agentbox-manifest/src/tomlval.rs
  - agentbox/services/agentbox-ops/src/lib.rs
  - agentbox/services/agentbox-ops/src/process_identity.rs
  - agentbox/services/agentbox-ops/src/cost_cap/mod.rs
  - agentbox/services/agentbox-ops/src/voyager/gate.rs
  - agentbox/services/agentbox-mcp/src/main.rs
  - agentbox/services/skill-tools/src/lib.rs
  - agentbox/services/ontology-tools/src/lib.rs
  - agentbox/services/podcast-ingest/src/lib.rs
  - agentbox/mcp/servers/harness-bridge.js
  - agentbox/mcp/servers/precedent-bridge.js
  - agentbox/mcp/servers/substrate-tools.js
  - agentbox/mcp/servers/governance-bridge.js
  - agentbox/mcp/servers/decision-tools.js
  - agentbox/mcp/servers/nostr-bridge.js
  - agentbox/mcp/servers/mcp-ws-relay.js
  - agentbox/config/entrypoint-unified.sh
  - agentbox/agentbox.toml
verified_commit: b00c28a0d
---

## AB-28.1 agentbox-manifest — the boot-time projection surface

```mermaid
flowchart TB
    subgraph cli["agentbox-manifest — one clap binary, Boot-time TOML/JSON projection for agentbox<br/>(main.rs:43-51)"]
        direction TB
        subgraph mcpg["MCP projection — src/mcp.rs"]
            C1["mcp-set-server --file --name<br/>main.rs:56-63"]
            C2["mcp-reconcile-aqe --file --provider<br/>main.rs:64-72"]
            C3["mcp-protect-namespace --file --server --namespace<br/>main.rs:73-82"]
            C4["mcp-deregister-fork --file<br/>main.rs:83-87"]
        end
        subgraph plug["Plugins — src/plugins.rs"]
            C5["plugin-register --file --key --install-path --message<br/>main.rs:88-104"]
            C6["plugin-list --manifest<br/>main.rs:105-109"]
        end
        subgraph proj["Config projection"]
            C7["nip98-config --manifest --out<br/>main.rs:110-116 · src/proxy.rs"]
            C8["model-routing-project --manifest --workspace --dry-run<br/>main.rs:117-126 · src/routing.rs"]
            C9["provision-stacks<br/>main.rs:127-128 · src/stacks.rs"]
        end
        subgraph tui["TUI round-trip"]
            C10["tui-read config state<br/>main.rs:129-130 · src/tui_read.rs"]
            C11["tui-write state output existing<br/>main.rs:131-136 · src/tui_write.rs"]
            C12["state-get file key<br/>main.rs:148"]
            C13["state-set file key value<br/>main.rs:149-154"]
            C14["state-set-bool file key value<br/>main.rs:155-160"]
        end
        subgraph read["Manifest readers — src/tomlval.rs"]
            C15["toml-bool --manifest --path<br/>main.rs:137-143 · prints 1 or 0, ALWAYS exits 0"]
            C16["toml-string --manifest --path<br/>main.rs:144-147 · prints a string or empty, ALWAYS exits 0"]
            C17["embedding-dim<br/>main.rs:146-147 · reads an OpenAI-shaped response on stdin"]
        end
    end
    EP["config/entrypoint-unified.sh"] --> cli
    subgraph notes["Invariants and drift"]
        direction TB
        N1["INVARIANT: this ONE binary owns every manifest read and config projection at boot. It<br/>replaced about 377 lines of inline python3 in the entrypoint plus four scripts, so<br/>PYTHON3 IS NO LONGER A BOOT DEPENDENCY — python3 stays in the image only for the<br/>supervised Python services opf-router and code-interpreter"]
        N2["SECURITY: mcp-set-server reads the spec JSON from STDIN specifically so bearer tokens<br/>and passwords never appear in the process list (main.rs:56-57)"]
        N3["toml-bool and toml-string ALWAYS EXIT 0 — a missing key is an empty answer, not a boot<br/>failure, so the entrypoint can read an absent gate without set -e killing the boot"]
        N4["mcp-protect-namespace is APPEND-ONLY on the governed server's protected list<br/>(main.rs:73) — see AB-20.2 for the protected-namespace write guard it feeds"]
        N1 ~~~ N2 ~~~ N3 ~~~ N4
    end
```

## AB-28.2 Boot projection sequence

```mermaid
sequenceDiagram
    autonumber
    participant EP as entrypoint-unified.sh<br/>agentbox/config/entrypoint-unified.sh
    participant BIN as agentbox-manifest<br/>agentbox/services/agentbox-manifest/src/main.rs:183
    participant TOML as /etc/agentbox.toml
    participant MCPJ as .mcp.json
    participant PROXY as nip98-proxy config
    participant AQE as .agentic-qe/llm-config.json
    participant PROF as WORKSPACE/profiles

    Note over BIN: main() calls restore_default_sigpipe() FIRST (main.rs:183-184)
    Note over BIN: Rust installs SIG_IGN for SIGPIPE at startup, which turns a closed downstream pipe into<br/>a PANIC-WITH-BACKTRACE on the next println!. The entrypoint pipes this binary into sed<br/>and consumes plugin-list through command substitution, and a backtrace in the boot log<br/>would be both alarming and useless (main.rs:165-172)
    EP->>BIN: toml-bool --manifest /etc/agentbox.toml --path <dotted.gate>
    BIN->>TOML: read
    BIN-->>EP: "1" or "0", exit 0 always
    loop each enabled MCP server
        EP->>BIN: mcp-set-server --file .mcp.json --name <server> with the spec on STDIN
        BIN->>MCPJ: upsert
    end
    EP->>BIN: mcp-deregister-fork --file .mcp.json
    BIN->>MCPJ: de-register any ruvector-mcp OUTSIDE /opt/agentbox (ADR-036 D2, main.rs:83)
    Note over BIN,MCPJ: this is what keeps a stray forked memory server from shadowing the governed one — see<br/>AB-20
    EP->>BIN: mcp-reconcile-aqe --file .mcp.json --provider <p>
    Note over BIN: an EMPTY or omitted provider REMOVES AQE_LLM_PROVIDER rather than blanking it<br/>(main.rs:67-68)
    EP->>BIN: mcp-protect-namespace --file .mcp.json --server claude-flow --namespace <ns>
    EP->>BIN: nip98-config --manifest --out
    BIN->>PROXY: project [interaction_plane.proxy] (ADR-069, main.rs:110) — see AB-10
    EP->>BIN: model-routing-project --manifest --workspace --dry-run?
    BIN->>AQE: project [model_routing] into EVERY .agentic-qe/llm-config.json (ADR-041, main.rs:117)
    EP->>BIN: provision-stacks
    BIN->>PROF: provision the per-stack profile tree under WORKSPACE/profiles
    EP->>BIN: plugin-list --manifest
    BIN-->>EP: name<TAB>source for enabled, VALIDATED [[plugins.packages]] (main.rs:105)
    loop each plugin to install
        EP->>BIN: plugin-register --file installed_plugins.json --key --install-path --message
        Note over BIN: --message is printed ONLY when the plugin was actually added (main.rs:97-99). --now<br/>freezes the installedAt/lastUpdated stamp and is TEST-ONLY and hidden — without it the<br/>value is the wall clock, which no golden could pin (main.rs:100-103)
    end
    Note over EP,BIN: every failure path prints to stderr and returns ExitCode::FAILURE (main.rs:186-189)
```

## AB-28.3 Consultant model projection (ADR-2031)

```mermaid
sequenceDiagram
    autonumber
    participant EP as entrypoint-unified.sh
    participant BIN as agentbox-manifest toml-string<br/>agentbox/services/agentbox-manifest/src/tomlval.rs
    participant TOML as agentbox.toml [consultants.*]<br/>agentbox/agentbox.toml:856
    participant ENV as consultant environment
    participant TUI as TUI save path

    EP->>BIN: toml-string --manifest --path consultants.<name>.model
    BIN->>TOML: read [consultants.<name>].model
    alt a NON-EMPTY pre-boot environment override exists
        ENV-->>ENV: the override WINS — the manifest does not clobber an operator's deliberate choice
    else no override
        BIN-->>ENV: project the manifest value into the consultant's environment at boot
    end
    TUI->>TOML: an operator saves the TUI
    Note over TUI,TOML: INVARIANT ADR-2031: a TUI save NEVER resets an operator's model
    Note over TOML: consultants are codex :861, antigravity :867, zai :873, perplexity :880, deepseek :885
    Note over TOML: INVARIANT ADR-2031: cost figures are DATED API-EQUIVALENT ESTIMATES or null, NEVER a<br/>stale constant
    Note over BIN: INVARIANT: consultant models come FROM THE MANIFEST — agentbox-manifest toml-string is<br/>the single projection path
```

## AB-28.4 TUI manifest round-trip

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator TUI
    participant RD as tui-read<br/>agentbox/services/agentbox-manifest/src/tui_read.rs
    participant STATE as flat TUI state JSON
    participant GET as state-get / state-set / state-set-bool<br/>agentbox/services/agentbox-manifest/src/main.rs:148-160
    participant WR as tui-write<br/>agentbox/services/agentbox-manifest/src/tui_write.rs
    participant TOML as canonical agentbox.toml

    OP->>RD: tui-read <config> <state>
    RD->>TOML: parse agentbox.toml
    RD->>STATE: emit the FLAT TUI state document
    loop operator edits
        OP->>GET: state-get <file> <key>
        GET-->>OP: one key
        OP->>GET: state-set <file> <key> <value> or state-set-bool
        GET->>STATE: write one key
    end
    OP->>WR: tui-write <state> <output> [existing]
    WR->>STATE: read the flat state
    opt existing supplied
        WR->>TOML: preserve the existing document's shape and comments
    end
    WR->>TOML: emit CANONICAL agentbox.toml
    Note over RD,WR: the round-trip is pinned by golden tests —<br/>agentbox/services/agentbox-manifest/tests/golden.rs, golden_entrypoint.rs and<br/>consultant_model.rs
    Note over WR: a TUI save must not reset an operator's consultant model (ADR-2031) — see AB-28.3
```

## AB-28.5 agentbox-ops — the retired-Python tool suite

```mermaid
flowchart TB
    subgraph lib["agentbox-ops shared modules — agentbox/services/agentbox-ops/src/lib.rs:7-15"]
        M1["cost_cap — spend ceilings + ledger"]
        M2["distil — expel lesson extraction"]
        M3["hermes — scheduling"]
        M4["process_identity — argv daemon identification (ADR-2032)"]
        M5["procs — process enumeration"]
        M6["pyjson — Python-shaped JSON compatibility"]
        M7["solar — PV geometry and yield"]
        M8["token_audit — token accounting"]
        M9["voyager — skill-library gate"]
    end
    subgraph bins["Binaries — agentbox/services/agentbox-ops/src/bin/"]
        B1["comfyui-generate — see AB-27"]
        B2["expel-distil"]
        B3["hermes-scheduler"]
        B4["mcp-call"]
        B5["pvgis-fetch"]
        B6["report-preflight"]
        B7["ruflo-daemon-gc"]
        B8["solar-optimize"]
        B9["token-audit"]
        B10["tree-search-cap — the spend cap in AB-22"]
        B11["voyager-gate"]
        B12["yt-transcript-archive"]
    end
    M1 --> B10
    M1 --> B9
    M2 --> B2
    M3 --> B3
    M4 --> B7
    M5 --> B7
    M7 --> B8
    M7 --> B5
    M8 --> B9
    M9 --> B11
    subgraph notes["Invariants and drift"]
        direction TB
        N1["Each binary REPLACES a Python script retired by the 2026-09-02 estate legacy audit. The<br/>modules hold the behaviour worth unit-testing independently of the CLI shell around it<br/>(lib.rs:1-5)"]
        N2["ADR-2032 daemon identification argv boundaries — process_identity.rs plus<br/>process_identity_tests.rs. ruflo-daemon-gc must identify its targets by ARGV shape,<br/>never by a name substring that could match an unrelated process"]
        N3["agentbox/services/agentbox-ops/tests/reaper_default_is_read_only.rs pins the safety<br/>default: the reaper is READ-ONLY unless explicitly told otherwise"]
        N4["ADR-2030 permissive licensing for publishable service crates — these crates carry<br/>LICENSE-APACHE and LICENSE-MIT, see agentbox/services/LICENSING-NOTICE.md"]
        N1 ~~~ N2 ~~~ N3 ~~~ N4
    end
```

## AB-28.6 agentbox-mcp — one binary, three stdio MCP servers

```mermaid
sequenceDiagram
    autonumber
    participant SUP as supervisord<br/>agentbox/flake.nix:2095
    participant BIN as agentbox-mcp<br/>agentbox/services/agentbox-mcp/src/main.rs:40
    participant LOG as tracing_subscriber
    participant T as rmcp stdio transport
    participant SRV as the selected server
    participant HOST as MCP host

    SUP->>BIN: agentbox-mcp <subcommand>
    Note over BIN: "Unified agentbox MCP server (imagemagick, web-summary, gemini-url-context)"<br/>(main.rs:20)
    BIN->>LOG: fmt().with_writer(std::io::stderr)
    Note over BIN,LOG: INVARIANT: logging MUST go to STDERR — stdout is the JSON-RPC stdio transport channel<br/>and ANY STRAY BYTE ON IT CORRUPTS THE PROTOCOL STREAM (main.rs:41-42)
    Note over LOG: EnvFilter from the environment, defaulting to "info" (main.rs:45-47)
    BIN->>T: rmcp::transport::stdio() (main.rs:51)
    alt Imagemagick (main.rs:29-30)
        BIN->>SRV: ImageMagickServer::new().serve(transport) (main.rs:54-58)
        Note over SRV: image processing with format conversion, resizing, cropping and batch operations.<br/>Modules args.rs / exec.rs / types.rs — see AB-27
    else web-summary (main.rs:31-33)
        BIN->>SRV: WebSummary server
        Note over SRV: URL summarization with YouTube transcripts and topic generation. Modules fetch.rs /<br/>youtube.rs / llm.rs / types.rs
    else gemini-url-context (main.rs:34-36)
        BIN->>SRV: GeminiUrlContext server
        Note over SRV: URL expansion and analysis using Gemini's URL Context API. Modules api.rs / types.rs
    end
    SRV->>HOST: JSON-RPC over stdio
    SRV->>SRV: service.waiting().await
```

## AB-28.7 skill-tools — Rust ports backing three skills

```mermaid
flowchart LR
    subgraph mods["agentbox/services/skill-tools/src/lib.rs:12-14"]
        U["uiux — BM25 search + design-system generation<br/>backs the ui-ux-pro-max-skill skill"]
        W["wardley — map generation, heuristics, interactive D3 rendering, strategic analysis<br/>backs the wardley-maps skill"]
        D["docs_alignment — link / mermaid / ASCII validation and reporting<br/>backs the docs-alignment skill"]
    end
    subgraph ub["uiux binaries"]
        UB1["uiux_search"]
    end
    subgraph wb["wardley binaries"]
        WB1["wardley_generate"]
        WB2["wardley_mapper"]
        WB3["wardley_quick_map"]
        WB4["wardley_heuristics"]
        WB5["wardley_interactive"]
        WB6["wardley_strategic_analyzer"]
    end
    subgraph db["docs_alignment binaries"]
        DB1["docs_alignment_bin"]
        DB2["docs_check_mermaid"]
        DB3["docs_validate_links"]
        DB4["docs_detect_ascii"]
        DB5["docs_generate_report"]
    end
    U --> ub
    W --> wb
    D --> db
    subgraph dmods["docs_alignment modules"]
        DM1["links.rs · links_external.rs"]
        DM2["mermaid.rs"]
        DM3["ascii_diagrams.rs"]
        DM4["orchestrator.rs · cli.rs"]
        DM5["report.rs · report_sections.rs · models.rs"]
    end
    D --> dmods
    subgraph notes["Invariants and drift"]
        direction TB
        N1["Each module is SELF-CONTAINED and backs one or more [[bin]] targets declared in<br/>Cargo.toml (lib.rs:9-10)"]
        N2["docs_check_mermaid is the skill-side mermaid validator. This diagrams tree is validated<br/>instead by scripts/diagram-index-gen.js --render, which renders every block through mmdc<br/>— a different and stricter gate"]
        N3["skill invocation and the manifest gates that enable these skills are AB-22"]
        N1 ~~~ N2 ~~~ N3
    end
```

## AB-28.8 The MCP server fleet in mcp/servers

```mermaid
flowchart LR
    subgraph node["agentbox/mcp/servers — Node MCP servers"]
        S1["ruvector-mcp.cjs — governed memory, fails closed<br/>see AB-20"]
        S2["ontology-bridge.js · ontology-propose.js · ontology-local.cjs · ontology-workingset.cjs<br/>see AB-25"]
        S3["harness-bridge.js — harness_list/inspect/validate/audit<br/>see AB-22"]
        S4["precedent-bridge.js — precedent_list/match/promote/retire<br/>see AB-22"]
        S5["continual-harness.cjs — evidence-anchored signed refines<br/>see AB-22"]
        S6["substrate-tools.js — refine_* · ws_* · spawn_child/ready/complete<br/>see AB-26"]
        S7["governance-bridge.js"]
        S8["decision-tools.js"]
        S9["nostr-bridge.js — see AB-13"]
        S10["mcp-ws-relay.js"]
    end
    subgraph rust["Rust MCP binaries"]
        R1["agentbox-mcp imagemagick | web-summary | gemini-url-context<br/>see AB-28.6"]
    end
    subgraph proj["Projection — see AB-09 and AB-22.10"]
        P1["agentbox/mcp/mcp.json — the fleet declaration"]
        P2["agentbox/skills/mcp.json — the skills-side projection"]
        P3["agentbox-manifest mcp-set-server / mcp-reconcile-aqe / mcp-deregister-fork<br/>agentbox/services/agentbox-manifest/src/mcp.rs"]
        P4["workspace .mcp.json — what the harness actually loads"]
    end
    P1 --> P3
    P2 --> P3
    P3 --> P4
    P4 --> node
    P4 --> rust
    subgraph notes["Invariants and drift"]
        direction TB
        N1["Every server reads its gates from agentbox.toml through the entrypoint's env projection<br/>— no server parses the manifest itself, so one gate edit reaches every surface"]
        N2["INVARIANT: a gated-off server is NOT REGISTERED AT ALL rather than<br/>registered-and-disabled — byte-identical-when-off"]
        N3["DIVERGENCE carried from AB-22.13: [skills.harness] and [skills.precedent] registration<br/>blocks in config/entrypoint-unified.sh check only FILE PRESENCE, never the manifest<br/>enabled flag, unlike every other [skills.*] gate"]
        N1 ~~~ N2 ~~~ N3
    end
```

## AB-28.9 Service crate publishing posture

```mermaid
classDiagram
    class PublishableServiceCrate {
        <<ADR-2030>>
        +LICENSE-APACHE
        +LICENSE-MIT
        +README.md
        +Cargo.toml with repository metadata
        +inline rustdoc on every public item
    }
    class agentbox_manifest {
        boot-time TOML/JSON projection
        clap subcommands
        golden tests
    }
    class agentbox_ops {
        retired-Python tool suite
        12 binaries
        9 shared modules
    }
    class agentbox_mcp {
        3 stdio MCP servers
        rmcp transport
    }
    class skill_tools {
        uiux · wardley · docs_alignment
        12 binaries
    }
    class ontology_tools {
        parser · validator · writer · modifier
        enrichment · link_validator
        see AB-25
    }
    class podcast_ingest {
        corpus ingest
        see AB-27
    }
    class dream_engine {
        nightly cycle
        see AB-23
    }
    class headroom_napi {
        NAPI compression addon
        see AB-26
    }
    PublishableServiceCrate <|-- agentbox_manifest
    PublishableServiceCrate <|-- agentbox_ops
    PublishableServiceCrate <|-- agentbox_mcp
    PublishableServiceCrate <|-- skill_tools
    PublishableServiceCrate <|-- ontology_tools
    PublishableServiceCrate <|-- podcast_ingest
    PublishableServiceCrate <|-- dream_engine
    PublishableServiceCrate <|-- headroom_napi
    note for PublishableServiceCrate "ADR-2030 permissive licensing for publishable service crates. Dual LICENSE-APACHE +<br/>LICENSE-MIT are present in agentbox-manifest, agentbox-ops, ontology-tools and<br/>podcast-ingest. The estate-wide notice is agentbox/services/LICENSING-NOTICE.md"
    note for agentbox_manifest "Nix packaging for each crate lives in agentbox/lib — agentbox-manifest.nix,<br/>agentbox-ops.nix, agentbox-mcp.nix, skill-tools.nix, ontology-tools.nix,<br/>podcast-ingest.nix, dream-engine.nix, headroom-compress.nix. A crate is in the package<br/>set only when its manifest gate is on, which is what makes byte-identical-when-off real<br/>at IMAGE level, not just runtime"
```
