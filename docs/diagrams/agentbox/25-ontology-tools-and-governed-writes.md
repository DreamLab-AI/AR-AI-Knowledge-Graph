---
id: AB-25
title: Ontology tools and governed writes
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
adrs: [ADR-2022, ADR-2023, ADR-2028, ADR-2054]
sources:
  - agentbox/mcp/servers/ontology-bridge.js
  - agentbox/mcp/servers/ontology-propose.js
  - agentbox/mcp/servers/ontology-local.cjs
  - agentbox/mcp/servers/ontology-workingset.cjs
  - agentbox/mcp/servers/lib/ontology-authoring-authority.js
  - agentbox/mcp/servers/lib/ontology-budget.js
  - agentbox/mcp/servers/lib/ontology-retrieval.js
  - agentbox/mcp/servers/lib/ontology-local.js
  - agentbox/mcp/servers/lib/ontology-condense.js
  - agentbox/mcp/servers/lib/ontology-index-build.js
  - agentbox/mcp/servers/lib/vault-frontmatter.js
  - agentbox/scripts/ontology-condense-scheduler.mjs
  - agentbox/scripts/ontology-condense-refresh.sh
  - agentbox/config/hooks/ontology-monitor.cjs
  - agentbox/services/ontology-tools/src/lib.rs
  - agentbox/agentbox.toml
verified_commit: b00c28a0d
---

## AB-25.1 ontology-bridge tool surface and dispatch

```mermaid
flowchart TB
    A["MCP call<br/>agentbox/mcp/servers/ontology-bridge.js"] --> B{"LOCAL_WRITE_TOOLS?<br/>ontology-bridge.js:47"}
    B -->|"ontology_axiom_add or ontology_propose"| W["handleLocalWrite<br/>ontology-bridge.js:87"]
    B -->|"every read tool"| C{"FORCE_LOCAL or remote error?"}
    C -->|"remote"| R["VisionClaw REST<br/>VISIONCLAW_API_URL default http://visionclaw-server:4000<br/>ontology-bridge.js:99"]
    C -->|"local"| L["handleLocal switch<br/>ontology-bridge.js:62-79"]
    R --> D{"isNetErr(result)?<br/>ontology-bridge.js:53"}
    D -->|"ontology_unavailable or ontology_timeout"| L
    D -->|"substantive 400/403 — a REAL answer"| OUT["surface unchanged"]
    subgraph tools["Read tools — declaration line in ontology-bridge.js"]
        T1["ontology_health :145 → local :65 returns _route local-fallback"]
        T2["ontology_search :150 → L.search :66"]
        T3["ontology_class_get :164 → L.classGet :67"]
        T4["ontology_class_list :176 → L.classList :68"]
        T5["ontology_validate :210 → L.validate :69"]
        T6["ontology_graph_query :221 → L.graphQuery :70 — read-only SPARQL"]
        T7["kg_node_search :233 → L.nodeSearch :71"]
        T8["kg_neighbors :246 → L.neighbors :72"]
        T9["kg_pathfind :259 → L.pathfind :73"]
        T10["ontology_ask :272 → L.ask :74"]
    end
    subgraph writes["Write tools — gated, never a direct helper call"]
        T11["ontology_axiom_add :188 handler :395"]
        T12["ontology_propose :407"]
    end
    L --> tools
    W --> writes
    subgraph notes["Invariants and drift"]
        direction TB
        N1["INVARIANT: a VisionClaw result counts as a network failure ONLY for ontology_unavailable<br/>/ ontology_timeout. Substantive HTTP errors are real answers and are NOT masked by the<br/>local route (ontology-bridge.js:49-52)"]
        N2["INVARIANT: no direct L.axiomAdd / L.propose call remains in the bridge — both write<br/>tools route through handleLocalWrite to the gated writer (ontology-bridge.js:75-77)"]
        N3["TIMEOUT_MS = ONTOLOGY_TIMEOUT_MS default 10000 (ontology-bridge.js:100). Server identity<br/>is ontology-bridge v0.1.0 (:522)"]
        N1 ~~~ N2 ~~~ N3
    end
```

## AB-25.2 ontology_ask — budget-bounded provenance-scoped subgraph

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant BR as ontology-bridge<br/>agentbox/mcp/servers/ontology-bridge.js:272
    participant RET as retrieval brain<br/>agentbox/mcp/servers/lib/ontology-retrieval.js:279
    participant BUD as ontology-budget<br/>agentbox/mcp/servers/lib/ontology-budget.js:66
    participant BE as backend<br/>Loom or VisionClaw — see AB-24

    AG->>BR: ontology_ask {query, model_tier, mode, depth, provenance, full, domain, max_tokens}
    Note over BR: model_tier enum booster | haiku | sonnet | opus default sonnet. mode enum menu | expand.<br/>provenance enum asserted | inferred default asserted (:280-289)
    BR->>RET: ask(req)
    RET->>BUD: resolveBudget(tier, max_tokens)
    Note over BUD: TIERS — booster maxTokens 80 depth 0 mode menu allowFull false — haiku 500 / 0 / menu /<br/>false — sonnet 2000 / 1 / expand / true — opus 6000 / 2 / expand / true (:14-19)
    Note over BUD: INVARIANT: an override may only LOWER the tier ceiling, never raise it — closes the<br/>discretionary-budget adversarial finding (:8, :66-77)
    alt full=true below sonnet
        BUD-->>RET: isFullAllowed false (:55-57)
        RET->>RET: full_denied = true
        Note over RET: page bodies are FORBIDDEN below sonnet by construction — and when allowed the body is<br/>chunked to at most the tier budget, closing the 93k-token leak (:11)
    end
    RET->>BE: seed then optional k-hop expand — see AB-24.3
    BE-->>RET: seeds and triples
    RET->>BUD: clampToBudget(turtle, tier, max_tokens)
    Note over BUD: estimateTokens rounds UP at ~4 chars/token so the governor errs toward UNDER-filling,<br/>never over (:31-35). Truncation appends the TRUNCATION_MARK (:22)
    BUD-->>RET: {text, tokens, truncated, budget}
    RET-->>BR: terse Turtle + breadcrumb + seed_iris + provenance
    BR-->>AG: budget-bounded provenance-scoped subgraph
    Note over AG,BR: read-pervasive and FAIL-OPEN — the tool never blocks a turn and never bloats the context<br/>window
    Note over RET: DIVERGENCE: the Loom expand helper queries ONE merged graph and does not isolate<br/>asserted from inferred, so provenance remains a REQUESTED scope, not a backend-enforced<br/>one (ADR-2023 remaining)
```

## AB-25.3 The authoring-authority gate — four modes, deny by default

```mermaid
sequenceDiagram
    autonumber
    participant BR as handleLocalWrite<br/>agentbox/mcp/servers/ontology-bridge.js:87
    participant WR as createAuthoredCorpusWriter<br/>agentbox/mcp/servers/lib/ontology-authoring-authority.js
    participant GATE as assertAuthoringAuthority<br/>agentbox/mcp/servers/lib/ontology-authoring-authority.js:255
    participant ENV as env
    participant MAN as agentbox.toml [skills.ontology]
    participant HELP as Markdown helper<br/>agentbox/mcp/servers/lib/ontology-local.js
    participant FM as vault-frontmatter<br/>agentbox/mcp/servers/lib/vault-frontmatter.js

    BR->>WR: axiomAdd(args, ctx) or propose(args, ctx)
    WR->>GATE: assertAuthoringAuthority({mode, manifest, env, target, operation})
    alt unknown or absent mode (:269)
        GATE-->>WR: throw OntologyAuthorityError code ontology_authoring_unknown_mode
        Note over GATE: DENY BY DEFAULT — an unknown or absent mode is denied and an absent manifest key reads<br/>false
    else target = shared-ontology AND mode is not governed-proposal (:277)
        GATE-->>WR: throw code ontology_shared_store_forbidden_in_local_mode
        Note over GATE: INVARIANT: the shared ontology is reachable SOLELY through governed-proposal —<br/>ontology_propose then Whelk then PR then human review
    else operation = direct-axiom-load AND mode is not governed-proposal (:285)
        GATE-->>WR: throw code ontology_direct_axiom_load_disabled
        Note over GATE: direct_axiom_load = false (agentbox.toml:637) now ACTUALLY blocks a direct axiom load in<br/>forced-local, remote-disabled and bootstrap. In governed-proposal the request is<br/>CONVERTED into a proposal rather than executed
    else mode = forced-local
        GATE->>ENV: require AGENTBOX_ONTOLOGY_LOCAL
        GATE->>ENV: require ONTOLOGY_LOCAL_AUTHORING=1
        GATE->>MAN: require skills.ontology.local_authoring = true
        alt any missing
            GATE-->>WR: throw code ontology_local_authoring_not_authorised with missing_authority[]
        end
    else mode = remote-disabled
        GATE->>ENV: require ONTOLOGY_LOCAL_AUTHORING=1 and the manifest key, MINUS the selector
        Note over GATE: INVARIANT: AN OUTAGE IS NOT AN AUTHORITY — a network failure withdraws the governed<br/>write path, it does not grant a local one (:43-45)
    else mode = bootstrap
        GATE->>ENV: require AGENTBOX_ONTOLOGY_BOOTSTRAP=1
        GATE->>ENV: require AGENTBOX_ONTOLOGY_BOOTSTRAP_AUTHORISATION reference
        GATE->>MAN: require direct_axiom_load = true
        alt any missing
            GATE-->>WR: throw code ontology_bootstrap_not_authorised
        end
        Note over GATE: ZERO-TOLERANCE class — ontology_axiom_load = "zero-tolerance" in<br/>[skills.authority.classes] (agentbox.toml:752)
    end
    GATE->>GATE: mint correlation id ont-auth-<ts>-<12 hex> at stage validation
    GATE-->>WR: authorised {mode, target, operation, correlation_id}
    WR->>HELP: write the authored page
    WR->>FM: stamp ontology-authoring-correlation / -mode / -stage into V2 frontmatter (:96-100)
    WR-->>BR: {authorised, mode, governed, route, correlation_id}
    Note over BR: local authoring reports governed=false so a caller can recognise the authority change<br/>before acting on the result
    Note over GATE: chain stages validation then proposal then approval then merge then served-corpus — ONE<br/>linked record rather than five unrelated events (:72-79)
```

## AB-25.4 Authority vocabularies

```mermaid
classDiagram
    class AUTHORING_MODES {
        <<frozen enum>>
        +FORCED_LOCAL "forced-local"
        +REMOTE_DISABLED "remote-disabled"
        +BOOTSTRAP "bootstrap"
        +GOVERNED_PROPOSAL "governed-proposal"
    }
    class AUTHORING_TARGETS {
        <<frozen enum>>
        +LOCAL_AUTHORED_CORPUS "local-authored-corpus"
        +SHARED_ONTOLOGY "shared-ontology"
    }
    class AUTHORING_OPERATIONS {
        <<frozen enum>>
        +LOCAL_AUTHORING_WRITE "local-authoring-write"
        +DIRECT_AXIOM_LOAD "direct-axiom-load"
        +PROPOSAL "proposal"
    }
    class DENIAL_CODES {
        <<frozen enum>>
        +UNKNOWN_MODE "ontology_authoring_unknown_mode"
        +LOCAL_AUTHORING_NOT_AUTHORISED "ontology_local_authoring_not_authorised"
        +SHARED_STORE_FORBIDDEN "ontology_shared_store_forbidden_in_local_mode"
        +DIRECT_AXIOM_LOAD_DISABLED "ontology_direct_axiom_load_disabled"
        +BOOTSTRAP_NOT_AUTHORISED "ontology_bootstrap_not_authorised"
        +GOVERNED_ROUTE_REQUIRED "ontology_governed_route_required"
    }
    class ENV_KEYS {
        <<frozen map>>
        +FORCE_LOCAL "AGENTBOX_ONTOLOGY_LOCAL"
        +LOCAL_AUTHORING "ONTOLOGY_LOCAL_AUTHORING"
        +BOOTSTRAP "AGENTBOX_ONTOLOGY_BOOTSTRAP"
        +BOOTSTRAP_AUTHORISATION "AGENTBOX_ONTOLOGY_BOOTSTRAP_AUTHORISATION"
    }
    class MANIFEST_KEYS {
        <<frozen map>>
        +LOCAL_AUTHORING "skills.ontology.local_authoring"
        +DIRECT_AXIOM_LOAD "skills.ontology.direct_axiom_load"
    }
    class CORRELATION_FRONTMATTER_KEYS {
        <<frozen map>>
        +ID "ontology-authoring-correlation"
        +MODE "ontology-authoring-mode"
        +STAGE "ontology-authoring-stage"
    }
    class OntologyAuthorityError {
        +String code
        +String message
        +String mode
        +String target
        +String operation
        +List~String~ missing_authority
        +toResult()
    }
    class AUTHORING_CHAIN_STAGES {
        <<frozen list>>
        validation
        proposal
        approval
        merge
        served-corpus
    }
    OntologyAuthorityError --> DENIAL_CODES
    OntologyAuthorityError --> AUTHORING_MODES
    OntologyAuthorityError --> AUTHORING_TARGETS
    note for OntologyAuthorityError "A denial is an ERROR, not a return value — a caller that forgets to check cannot<br/>silently proceed. missing_authority names the EXACT authorities that were absent: never<br/>a silent no-op and never a silent write (ontology-authoring-authority.js:106-131)"
    note for MANIFEST_KEYS "PROPOSED ADR-2054: skills.ontology.local_authoring is undeclared in agentbox.toml,<br/>so local authoring is DENIED IN EVERY MODE as shipped — correct-by-default but<br/>implicit. Declaring it explicitly as false is routed to the manifest owner so the<br/>deny is reviewable rather than an absence."
```

## AB-25.5 The governed promotion route

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant BR as ontology_propose<br/>agentbox/mcp/servers/ontology-bridge.js:407
    participant GUARD as ontology-propose guard<br/>agentbox/mcp/servers/ontology-propose.js:140
    participant VC as VisionClaw<br/>POST /api/ontology-agent/propose
    participant WHELK as Whelk consistency check<br/>see VC-20
    participant PR as Pull request
    participant H as Human reviewer
    participant CORP as served corpus

    AG->>BR: ontology_propose(body)
    BR->>GUARD: guard the write
    Note over GUARD: AGENTBOX_ONTOLOGY_DIRECT_LOAD default OFF so ontology_axiom_add refuses and REDIRECTS to<br/>the governed path (ontology-propose.js:16, :140)
    BR->>VC: governed proposal
    VC->>WHELK: consistency check
    alt inconsistent
        WHELK-->>VC: reject
        VC-->>AG: staged result with the inconsistency named
    else consistent
        WHELK-->>VC: ok
        VC->>PR: open a PR
        PR->>H: review
        alt approved
            H->>CORP: merge, then the generation is served
        else refused
            H-->>AG: no change
        end
    end
    Note over AG,CORP: INVARIANT ADR-2022: personal-KG concepts reach the shared ontology ONLY through<br/>ontology_propose then Whelk then PR then human review/merge (agentbox.toml:632-635)
    Note over GUARD: DIVERGENCE: ADR-2022 implementation_status is PARTIAL. FORCE_LOCAL dispatch precedes the<br/>remote axiom descriptor and reaches a Markdown-writing helper, so the remote default<br/>prevents an ungoverned REMOTE load but does not by itself enforce every LOCAL authoring<br/>path — that is what the AB-25.3 gate now covers
    Note over CORP: DIVERGENCE: carrying one correlation id through a real Whelk validation then PR then<br/>approval then merge then served corpus still requires the VisionClaw-side stages to<br/>accept and echo it — the end-to-end correlated promotion chain is not demonstrated
    Note over BR: RESOLVED ADR-2054: the standalone CLI now routes through createAuthoredCorpusWriter<br/>with mode forced-local, so EVERY authoring caller crosses assertAuthoringAuthority.<br/>A denial returns a typed OntologyAuthorityError result and exits non-zero — being a<br/>CLI is not an authority. The static-guard test pins the receiver as the gated writer.
```

## AB-25.6 ontology_axiom_add — the disabled backdoor

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant BR as ontology_axiom_add<br/>agentbox/mcp/servers/ontology-bridge.js:395
    participant HW as handleLocalWrite<br/>agentbox/mcp/servers/ontology-bridge.js:87
    participant GATE as assertAuthoringAuthority<br/>agentbox/mcp/servers/lib/ontology-authoring-authority.js:255
    participant MAN as agentbox.toml
    participant LOAD as POST /api/ontology/load

    AG->>BR: ontology_axiom_add(args)
    BR->>HW: route through the gated writer — never a direct helper call
    HW->>GATE: operation = direct-axiom-load
    GATE->>MAN: read skills.ontology.direct_axiom_load
    alt direct_axiom_load = false (agentbox.toml:637 — the DEFAULT)
        GATE-->>HW: throw ontology_direct_axiom_load_disabled
        HW-->>AG: typed denial with missing_authority, redirected to ontology_propose
        BR--xLOAD: the ungoverned POST /api/ontology/load is NEVER issued
    else bootstrap mode with every authority present
        GATE-->>HW: authorised — explicit, signed, auditable, deliberately slow
        HW->>LOAD: bulk load
    end
    Note over MAN: ontology_axiom_load = "zero-tolerance" in [skills.authority.classes]<br/>(agentbox.toml:752), commented "ungoverned KG write backdoor"
    Note over BR: INVARIANT: default off = ontology_axiom_add refuses and redirects (agentbox.toml:636).<br/>Set true only for admin/bootstrap, where a signed authorisation is required
    Note over GATE: the bootstrap authorisation reference is RECORDED, not verified, here — signature<br/>verification remains the management-api authority consumer's job (ADR-2022 remaining)
    Note over BR: routine enrichment is gated behind a PR round-trip and there is NO fast path for<br/>high-volume trusted writes, by design (ADR-2022 consequences)
```

## AB-25.7 Condensation index — the staleness scheduler

```mermaid
sequenceDiagram
    autonumber
    participant SCH as ontology-condense-scheduler.mjs<br/>agentbox/scripts/ontology-condense-scheduler.mjs:1
    participant GATE as manifest gates
    participant CORP as authored corpus<br/>VAULT_PAGES
    participant OUT as condense output
    participant REF as ontology-condense-refresh.sh<br/>agentbox/scripts/ontology-condense-refresh.sh
    participant IDX as ontology-index-build.js
    participant LLM as Loom facade<br/>agentbox/agentbox.toml:649 — see AB-24
    participant RV as RuVector ns ontology-classes<br/>see AB-20

    Note over SCH: supervised as [program:ontology-condense-scheduler] (agentbox/flake.nix:1839) — launched<br/>unconditionally, exits fast when its gate is off
    loop tick — schedule_interval_mins 60, jittered plus or minus 20 percent (agentbox.toml:665)
        SCH->>GATE: require BOTH ONTOLOGY_CONDENSE_ENABLED and ONTOLOGY_CONDENSE_SCHEDULE
        alt either off
            SCH-->>SCH: no-op — byte-identical-when-off until an operator opts in and the container reboots
        else both on
            SCH->>CORP: newest page mtime
            SCH->>OUT: last condense output mtime
            alt corpus newer OR output missing OR older than schedule_max_age_hours 24 (agentbox.toml:666)
                SCH->>REF: exec the refresh
                Note over REF: flock-serialised — SKIPS if a refresh already holds the lock. Stages overwrite/resume<br/>deterministically, so the scheduler is idempotent
                REF->>IDX: parse the corpus into classes
                loop each KG class, max_concurrency 2 (agentbox.toml:653)
                    REF->>LLM: POST /v1/chat/completions — one retrieval sentence + a synonym list
                    Note over LLM: model qwen3.8-27B style openai (agentbox.toml:650-652). The model runs BEHIND the Loom<br/>facade so it is swappable with zero change here
                end
                REF->>OUT: PUSH Class-Summary cache
                REF->>RV: condensed store ns ontology-classes
            else fresh
                SCH-->>SCH: writes NOTHING — no LLM load, no cache churn
            end
        end
    end
    Note over SCH: the refresh had NO natural trigger — nothing re-ran it when GitHubSync/elevation rewrote<br/>the corpus, so the per-turn [ONTOLOGY] breadcrumb cache silently went stale. The<br/>"triggered incrementally on sync" claim was UNWIRED (:7-13)
    Note over SCH: fail-open, and a THIN wrapper — parse, cheap-LLM pass and cache fold all stay in the<br/>refresh script (:15-16). Mirrors the ruvector-aggregate-sweep house pattern, see AB-21
```

## AB-25.8 ontology-monitor — SessionEnd governed elevation

```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code SessionEnd
    participant MON as ontology-monitor.cjs<br/>agentbox/config/hooks/ontology-monitor.cjs:1
    participant LOC as local ontology route<br/>VisionClaw-free
    participant ZAI as Z.AI GLM worker
    participant JSONL as ontology-proposals.jsonl<br/>AGENTBOX_STATE
    participant RELAY as forum broker gate<br/>Nostr relay
    participant H as Human approver

    CC->>MON: SessionEnd
    alt AGENTBOX_ONTOLOGY_MONITOR not 1
        MON-->>CC: silent no-op — fail-open on any miss (:14)
    else master switch on
        alt ZAI_ANTHROPIC_API_KEY and ZAI_API_KEY both absent
            MON-->>CC: silent no-op — no GLM worker (:15)
        else
            MON->>MON: gather the session's concept-bearing work
            MON->>LOC: match ontology concepts via the local route
            MON->>ZAI: ONE review call
            ZAI-->>MON: proposals where the corpus looks stale, wrong or missing
            loop each proposal
                MON->>MON: build an ACSP ActionRequest kind 31402
                alt AGENTBOX_ONTOLOGY_MONITOR_MODE = dryrun (the DEFAULT)
                    MON->>JSONL: append — NO relay egress (:18)
                else mode = publish
                    alt MANAGEMENT_API_KEY and NOSTR_RELAYS present
                        MON->>RELAY: publish the signed 31402 live
                        RELAY->>H: forum approval
                        H-->>RELAY: kind 31403 approval — see AB-14
                    else
                        MON-->>CC: silent no-op — cannot sign and publish (:16)
                    end
                end
            end
        end
    end
    Note over MON: INVARIANT: nothing here signs the DECISION — a human approves via the forum.<br/>Read-pervasive, write-governed (:9-11)
    Note over MON: never throws and never blocks the session — a hard wall-clock budget aborts cleanly<br/>(:23)
```

## AB-25.9 Session working set and the drift guard

```mermaid
stateDiagram-v2
    [*] --> Empty
    Empty --> Held : note iri-or-slug
    note right of Empty
        ontology-workingset CLI — a session-scoped, IRI-keyed working set of
        compacted ontology-class digests carried ACROSS TURNS.
        Session defaults to AGENTBOX_SESSION_ID, bound to a beads epic or a
        session URN. agentbox/mcp/servers/ontology-workingset.cjs:1-18
    end note
    Held --> Held : get iri-or-slug
    Held --> Held : list
    Held --> Held : note another iri
    Held --> Revalidating : revalidate
    Revalidating --> Held : digests still match the live corpus
    Revalidating --> Drifted : corpus moved beneath the digest
    note right of Drifted
        The DRIFT GUARD. A digest compacted in an earlier turn can silently
        disagree with the corpus a later turn reasons over, so revalidate
        diffs the held digests against the live corpus rather than trusting
        carried state.
    end note
    Drifted --> Held : re-note the changed IRIs
    Held --> Empty : drop iri-or-slug
    Held --> Empty : clear
    Empty --> [*]
    note right of Held
        Resolution order is the repo lib first, then the baked /opt copy —
        the same pattern as ontology-local.cjs and continual-harness.cjs.
    end note
```

## AB-25.10 Vault path authority for ontology writes

```mermaid
sequenceDiagram
    autonumber
    participant EP as entrypoint<br/>agentbox/config/entrypoint-unified.sh
    participant MAN as agentbox.toml [vault]<br/>agentbox/agentbox.toml:680
    participant SK as ontology write path
    participant FM as vault-frontmatter<br/>agentbox/mcp/servers/lib/vault-frontmatter.js
    participant CI as check-no-logseq-paths.sh<br/>agentbox/scripts/ci/check-no-logseq-paths.sh

    EP->>MAN: read [vault]
    alt no [vault] configured
        EP-->>EP: FAIL-LOUD — boot prints "[vault] disabled"
        SK-->>SK: the corpus skill disables itself with one clear line rather than writing into a stale<br/>tree
    else configured
        EP->>SK: export VAULT_ROOT / VAULT_PAGES / VAULT_FORMAT / VAULT_TUI to every supervised program,<br/>tmux window and MCP server
        Note over EP,SK: ONTOLOGY_PAGES_DIR is DERIVED from [vault] — no consumer hard-codes a corpus path, so<br/>one edit relocates the corpus for every agent surface
        SK->>SK: expand ${VAULT_ROOT} / ${VAULT_PAGES} placeholders in its own config
        SK->>FM: write the page
        FM->>FM: emit V2 YAML frontmatter — public is a real YAML boolean, wikilink values are quoted
        opt legacy leading block present
            FM->>FM: convert a legacy key-colon-colon-value block on write
        end
        FM->>FM: stamp the AB-25.3 correlation keys
    end
    CI->>CI: gate the tree against literal corpus paths
    Note over FM: INVARIANT ADR-2028: emitting a key-colon-colon-value line is a VIOLATION<br/>(VAULT-corpus-format Invariant 1). format "obsidian" is the one format writers emit —<br/>"logseq-legacy" is READ-tolerance only for an unconverted graph
    Note over SK: see AB-22 for the full skills-side vault contract
```
