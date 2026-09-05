---
id: AB-22
title: Skills estate — discovery, lint gate, routing, harness/precedent MCP bridges
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
adrs: [ADR-2020, ADR-2021, ADR-2028, ADR-2056, ADR-2057]
sources:
  - agentbox/skills/lint-skills.sh
  - agentbox/skills/lint-skills.mjs
  - agentbox/skills/SKILL-DIRECTORY.md
  - agentbox/skills/skill-router/SKILL.md
  - agentbox/skills/skill-router/references/routing-table.md
  - agentbox/.github/workflows/invariants.yml
  - agentbox/skills/tree-search-coder/SKILL.md
  - agentbox/skills/tree-search-coder/references/algorithm.md
  - agentbox/scripts/skill-count-check.js
  - agentbox/tests/fixtures/skill-router-prompts.json
  - agentbox/mcp/servers/harness-bridge.js
  - agentbox/mcp/servers/precedent-bridge.js
  - agentbox/management-api/lib/precedent-service.js
  - agentbox/mcp/servers/continual-harness.cjs
  - agentbox/mcp/servers/lib/continual-harness.js
  - agentbox/skills/mcp.json
  - agentbox/mcp/mcp.json
  - agentbox/scripts/project-mcp-servers.mjs
  - agentbox/config/entrypoint-unified.sh
  - agentbox/mcp/servers/lib/vault-frontmatter.js
  - agentbox/scripts/ci/check-no-logseq-paths.sh
  - agentbox/agentbox.toml
  - agentbox/docs/adr/ADR-2020-capability-gating.md
  - agentbox/docs/adr/ADR-2021-skills-jit-context-lint.md
  - agentbox/docs/adr/ADR-2028-vault-manifest-path-authority.md
  - agentbox/services/agentbox-ops/src/cost_cap/mod.rs
  - agentbox/services/agentbox-ops/src/bin/tree-search-cap.rs
verified_commit: b00c28a0d
---

## AB-22.1 Skill discovery and JIT load at a turn

```mermaid
sequenceDiagram
    autonumber
    participant U as User turn
    participant H as Claude Code harness
    participant FS as /opt/agentbox/skills<br/>agentbox/skills/SKILL-DIRECTORY.md
    participant SK as SKILL.md<br/>agentbox/skills/tree-search-coder/SKILL.md:1
    participant REF as references/*<br/>agentbox/skills/tree-search-coder/references/algorithm.md

    Note over H,FS: INVARIANT: skills self-trigger from description frontmatter (ADR-2021) — bake root is<br/>/opt/agentbox/skills, NEVER ~/.claude/skills
    H->>FS: scan description frontmatter of every SKILL.md
    alt manifest gate off — e.g. [skills.tree_search_coder] enabled = false
        Note right of H: DIVERGENCE: byte-identical-when-off (GOVERNANCE-capabilities.md Invariants) —<br/>instructional file still bakes with the skills tree, only the executable<br/>package/supervised process is omitted
        H-->>U: skill absent from routable set this turn
    else trigger phrase matches description
        H->>SK: load entry-context SKILL.md (<=250 lines, MAX_ENTRY_LINES)
        SK-->>H: name, description, triggers, manifest_gate, related_skills
        opt entry references deeper material
            H->>REF: load references/algorithm.md on demand
            REF-->>H: 7-step algorithm, manifest gate, URN schema
        end
        H-->>U: skill invoked in-turn
    end
    alt explicit /route dispatch
        U->>H: "/route <task>"
        Note right of H: see AB-22.2 for the full dispatch sequence
    end
```

## AB-22.2 /route dispatch — skill-router

```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant RT as skill-router<br/>agentbox/skills/skill-router/SKILL.md:1
    participant TBL as routing-table.md<br/>agentbox/skills/skill-router/references/routing-table.md
    participant FIX as skill-router-prompts.json<br/>agentbox/tests/fixtures/skill-router-prompts.json:1
    participant TGT as target skill

    U->>RT: /route describe what you need
    RT->>TBL: load generated routing table (derived from every skill description)
    Note right of TBL: INVARIANT: table is regenerated after any skill description change — not hand-edited to<br/>diverge (SKILL.md:87-90)
    alt clear match (Rule 1)
        RT-->>U: state which skill handles it, in one sentence
        RT->>TGT: invoke immediately
    else ambiguous — 2+ candidate skills (Rule 2)
        RT-->>U: ask exactly ONE clarifying question
        U-->>RT: answer
        RT->>TGT: invoke chosen skill
    else multi-skill composition (Rule 3)
        RT-->>U: state the sequence, invoke the first
        RT->>TGT: invoke first skill in the stated plan
    else no match (Rule 4) or bare /route (Rule 5)
        RT-->>U: condensed menu<br/>(Code/Research/Economics/Design/Docs/Media/DevOps/Security/Architecture/AEC)
    end
    Note over RT,FIX: fixture set distinguishes codeact (persistent-kernel, stateful) vs sparc-code<br/>(file-scaffold, no shared state) routing intent — no automated test consumer found for<br/>this fixture in agentbox/scripts or agentbox/tests
```

## AB-22.3 Lint gate (ADR-2021) — findings and the advisory divergence

```mermaid
sequenceDiagram
    autonumber
    participant CI as invariants.yml<br/>agentbox/.github/workflows/invariants.yml:68
    participant SH as lint-skills.sh<br/>agentbox/skills/lint-skills.sh:24
    participant MJS as lint-skills.mjs<br/>agentbox/skills/lint-skills.mjs:406
    participant TXT as checkTextPatterns<br/>agentbox/skills/lint-skills.mjs:308
    participant SKC as checkSkill<br/>agentbox/skills/lint-skills.mjs:341

    CI->>SH: bash skills/lint-skills.sh
    SH->>MJS: exec node ./lint-skills.mjs (skills/lint-skills.sh:24)
    MJS->>TXT: checkTextPatterns() over every *.md under skills/
    alt STALE — banned string, no suppress context
        TXT-->>MJS: fail STALE (BANNED regex agentbox/skills/lint-skills.mjs:40-41)
    else STALE — suppressed by DEAD/retired/legacy/lint-ok context
        TXT-->>MJS: suppressed STALE (agentbox/skills/lint-skills.mjs:42,326)
    else ABSPATH — literal ~/.claude/skills/ path, dir not in skip-list
        TXT-->>MJS: fail ABSPATH (agentbox/skills/lint-skills.mjs:43,45,329-332)
    else RETIRED-PATH — literal /workspace/ prefix, no lint-ok
        TXT-->>MJS: fail RETIRED-PATH (agentbox/skills/lint-skills.mjs:46,333-336)
    end
    MJS->>SKC: checkSkill(skill) for each skills/<name>/SKILL.md
    alt FRONTMATTER — missing/empty name or description, or block never opens/closes
        SKC-->>MJS: fail FRONTMATTER (agentbox/skills/lint-skills.mjs:346-364)
    else BUDGET — over MAX_ENTRY_LINES=250, no references/ or references/ has no readable file
        SKC-->>MJS: fail BUDGET (agentbox/skills/lint-skills.mjs:32,366-387)
    else RESOURCE — cited references|scripts|assets path does not resolve
        SKC-->>MJS: fail RESOURCE (agentbox/skills/lint-skills.mjs:389-399)
    end
    MJS-->>CI: exit 0 clean / exit 1 with byCode summary (agentbox/skills/lint-skills.mjs:417-429)
    Note over MJS,CI: DIVERGENCE: "Skill lint is advisory — lint-skills.sh gates estate hygiene but is not a<br/>runtime capability gate — an enabled skill with clean frontmatter is trusted"<br/>(agentbox/docs/GOVERNANCE-capabilities.md:206-207)
```

## AB-22.4 SKILL-DIRECTORY.md maintenance vs the machine-checked count

```mermaid
sequenceDiagram
    autonumber
    participant AUTHOR as Estate audit (human/Opus swarm)
    participant DIR as SKILL-DIRECTORY.md<br/>agentbox/skills/SKILL-DIRECTORY.md:1
    participant CNT as skill-count-check.js<br/>agentbox/scripts/skill-count-check.js:115
    participant DISK as skills/*/SKILL.md<br/>agentbox/skills/

    Note over AUTHOR,DIR: SKILL-DIRECTORY.md is hand-maintained prose (categorised inventory, decision tree, MCP<br/>table) — no generator script found under agentbox/scripts for its body text
    CNT->>DISK: countSkills() — one SKILL.md per top-level skills/ dir<br/>(agentbox/scripts/skill-count-check.js:52-68)
    DISK-->>CNT: count = 126 (agentbox/scripts/skill-count-check.js output, this tree)
    CNT->>DIR: scanDoc() for "N active skills" / "N+ skills" / "for N skills" claims<br/>(agentbox/scripts/skill-count-check.js:41-45,77-106)
    alt claim matches truth count
        CNT-->>AUTHOR: ok=true (SKILL-DIRECTORY.md:3,34 both state 126 active skills — matches)
    else claim diverges from truth count
        CNT-->>AUTHOR: E-SKILL1 skill-count drift, exit 1 (agentbox/scripts/skill-count-check.js:154-161)
    end
    Note over AUTHOR,DISK: DOC-DRIFT: agentbox/docs/GOVERNANCE-capabilities.md:91 says "SKILL-DIRECTORY.md (912<br/>lines)" — the working tree's SKILL-DIRECTORY.md is 915 lines (wc -l)
    Note over AUTHOR,DISK: DOC-DRIFT: agentbox/CLAUDE.md:30 says "The image bakes /opt/agentbox/skills (118<br/>skills)" — skill-count-check.js and SKILL-DIRECTORY.md:3 both agree the tree holds 126
```

## AB-22.5 harness-bridge MCP — list/inspect/validate

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant HB as harness-bridge<br/>agentbox/mcp/servers/harness-bridge.js:263
    participant LT as loadTemplates<br/>agentbox/mcp/servers/harness-bridge.js:57
    participant TD as HARNESS_TEMPLATE_DIR<br/>agentbox/mcp/servers/harness-bridge.js:25

    Note over HB: Note tools+lines — harness_list:197, harness_inspect:212, harness_validate:227,<br/>harness_audit:246 (see AB-22.5b)
    AG->>HB: CallTool harness_list {maturity_filter?}
    HB->>LT: loadTemplates() — read *.json from TEMPLATE_DIR
    LT->>TD: fs.readdirSync(TEMPLATE_DIR)
    alt TEMPLATE_DIR missing
        LT-->>HB: {templates: [], warnings: ["Template directory does not exist"]}
    else templates present, each schema-validated (Ajv or manual fallback)
        LT-->>HB: {templates, warnings}
    end
    HB-->>AG: {topology, version, maturity, substrates, guide_count, sensor_count, pairing_ratio}[]<br/>(harness-bridge.js:273-288)

    AG->>HB: CallTool harness_inspect {topology}
    HB->>LT: loadTemplates()
    alt topology not found
        HB-->>AG: {error: "not_found", available: [...]} (harness-bridge.js:298-305)
    else found
        HB->>HB: computePairingAnalysis(template) (harness-bridge.js:149-179)
        HB-->>AG: full template + computed{pairing_ratio, unpaired_guides, unpaired_sensors,<br/>coverage_summary}
    end

    AG->>HB: CallTool harness_validate {topology, output_summary}
    HB->>LT: loadTemplates()
    alt topology not found
        HB-->>AG: {error: "not_found"} (harness-bridge.js:323-329)
    else found — check required_substrates and blocked_patterns per guide, plus structure.substrates
        HB-->>AG: {compliant, violations[], template_version} (harness-bridge.js:363-369)
    end
    Note over TD: DOC-DRIFT: agentbox.toml:699 sets [skills.harness] template_dir =<br/>"/home/devuser/workspace/VisionFlow/docs/engineering/templates" but<br/>config/entrypoint-unified.sh's harness-bridge registration block (:1462-1476) never<br/>exports HARNESS_TEMPLATE_DIR — the server falls back to its hardcoded default<br/>/var/lib/agentbox/harness-templates
```

## AB-22.6 harness_audit — pairing ratio across all templates

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant HB as harness-bridge<br/>agentbox/mcp/servers/harness-bridge.js:372
    participant LT as loadTemplates<br/>agentbox/mcp/servers/harness-bridge.js:57
    participant PA as computePairingAnalysis<br/>agentbox/mcp/servers/harness-bridge.js:149

    AG->>HB: CallTool harness_audit {verbose?}
    HB->>LT: loadTemplates()
    loop for each template t
        HB->>PA: computePairingAnalysis(t) — pairedGuideIds/pairedSensorIds via pairings[] cross-ref
        PA-->>HB: {pairing_ratio, unpaired_guides, unpaired_sensors, coverage_summary}
        opt verbose = true
            HB->>HB: entry.unpaired_guides / entry.unpaired_sensors attached (harness-bridge.js:390-393)
        end
    end
    HB-->>AG: {audit: [{topology, guides_total, sensors_total, paired, ratio, maturity}], summary: "N<br/>templates, M% average pairing ratio"} (harness-bridge.js:399-408)
```

## AB-22.7 Precedent lifecycle

```mermaid
stateDiagram-v2
    [*] --> Unpromoted
    Unpromoted --> Active: precedent_promote (case_id, outcome, reason, category)
    Active --> Active: precedent_match similarity below threshold — no state change
    Active --> AutoApplied: applyPrecedent — similarity >= 0.85 threshold
    Active --> Retired: precedent_retire (case_id, reason)
    AutoApplied --> Active: still active, not retired
    Retired --> [*]
    note right of Active
        DEFAULT_SIMILARITY_THRESHOLD 0.85
        agentbox management-api lib precedent-service.js line 26
    end note
    note right of Retired
        retired precedents excluded from
        matchPrecedent and listPrecedents
        precedent-service.js line 202,356
    end note
```

## AB-22.8 precedent_match — file-store word-overlap search

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent/orchestrator
    participant PB as precedent-bridge<br/>agentbox/mcp/servers/precedent-bridge.js:188
    participant PS as PrecedentService<br/>agentbox/management-api/lib/precedent-service.js:184
    participant FS as file store<br/>agentbox/mcp/servers/precedent-bridge.js:40

    AG->>PB: CallTool precedent_match {title, description, category?}
    PB->>PS: matchPrecedent({title, description, category})
    PS->>FS: store.search(query, "governance-precedents", 5) — word-overlap similarity<br/>(precedent-bridge.js:72-82)
    FS-->>PS: results sorted by similarity desc
    loop over top-5 results
        alt record.retired = true
            PS->>PS: skip (precedent-service.js:202)
        else similarity >= similarityThreshold (0.85)
            PS-->>PB: {matched: true, precedent{key,caseId,outcome,reason,...}, similarity}
        end
    end
    alt no match found in loop
        PS-->>PB: {matched: false, precedent: null, similarity: bestSimilarity}<br/>(precedent-service.js:222-224)
    end
    PB-->>AG: JSON result
    Note over PS,FS: production wiring is RuVector MCP semantic search (384-dim, Xinference bge-small) — this<br/>file store's word-overlap similarity is documented as the local/test substitute<br/>(precedent-bridge.js:9-13)
```

## AB-22.9 continual-harness — evidence-anchored signed refine

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator/agent
    participant CLI as continual-harness.cjs<br/>agentbox/mcp/servers/continual-harness.cjs:55
    participant CH as createHarness().refine<br/>agentbox/mcp/servers/lib/continual-harness.js:153
    participant IB as immutableBase guard<br/>agentbox/mcp/servers/lib/continual-harness.js:138
    participant GIT as harnessDir git repo<br/>agentbox/mcp/servers/lib/continual-harness.js:118
    participant SIGN as defaultSign<br/>agentbox/mcp/servers/lib/continual-harness.js:87

    OP->>CLI: continual-harness refine <layer> <key> --value V --evidence E [--reason R]
    CLI->>CH: h.refine({layer, key, value, evidence, reason, actor})
    alt layer not in supplemental-prompt|memory|skill-spec|subagent-spec
        CH-->>CLI: throw "layer must be one of: ..." (continual-harness.js:154)
    else key fails [a-z0-9-]{1,128} slug
        CH-->>CLI: throw "key must match [a-z0-9-]" (continual-harness.js:155)
    else evidence missing or blank — INVARIANT: no anchor, no refine
        CH-->>CLI: throw "evidence is required — a refine must cite the transcript span / commit / test<br/>that justifies it" (continual-harness.js:157-159)
    else evidence present
        CH->>IB: targetPath(layer, key) resolves under harnessDir
        alt resolved path escapes harnessDir
            IB-->>CH: throw "refine target escapes the harness dir" (continual-harness.js:141)
        else resolved path IS one of the immutable-base files (CLAUDE.md tiers)
            IB-->>CH: throw "refuse to write an immutable base file" (continual-harness.js:144) — TWO-LAYER<br/>hard invariant, never a convention
        else path is inside the mutable layer
            CH->>GIT: write <layer>/<key>.md with layer/key/operator/actor/evidence/updated header<br/>(continual-harness.js:164-178)
            GIT-->>CH: git add, then diff --cached --name-only
            alt no staged diff — identical content
                CH-->>CLI: {changed: false, commit: HEAD, signature: null, signed: false}<br/>(continual-harness.js:184)
            else content changed
                CH->>SIGN: sign(canonical, {key, evidence}) — Schnorr via management-api Nostr key<br/>(REFINE_KIND=30841)
                alt signer/key material unavailable (dev shell) — fail-open, not signature-invalid
                    SIGN-->>CH: null
                    CH->>GIT: commit with Refine-Signature: deferred, Refine-Pubkey: (deferred)<br/>(continual-harness.js:199-200)
                    CH-->>CLI: {changed: true, signature: "deferred", signed: false}
                else signer present
                    SIGN-->>CH: {sig, pubkey}
                    CH->>GIT: commit with Refine-Evidence/Refine-Operator/Refine-Signature/Refine-Pubkey trailers
                    CH-->>CLI: {changed: true, commit, signature: sig, signed: true}
                end
            end
        end
    end
    CLI-->>OP: JSON result to stdout
    Note over CH,GIT: INVARIANT: immutable base = CLAUDE.md tiers (~/.claude/CLAUDE.md, ~/workspace/CLAUDE.md,<br/>vault CLAUDE.md via VAULT_ROOT — ADR-2028) — a refine resolving onto any of them is<br/>REJECTED, enforced as a path guard not a convention (continual-harness.js:50-65,143-144)
```

## AB-22.10 Skills-side MCP projection boundary — skills/mcp.json to workspace .mcp.json

```mermaid
flowchart TB
    REG["skills/mcp.json<br/>agentbox/skills/mcp.json:3<br/>CANONICAL BOOT-PROJECTION SOURCE"]
    MIRROR["mcp/mcp.json<br/>agentbox/mcp/mcp.json:3<br/>INFRASTRUCTURE REGISTRY — consumed by agentbox.toml, flake.nix, mcp-ws-relay.js"]
    BAKE["baked at /opt/agentbox/skills/mcp.json<br/>entrypoint _MCP_REGISTRY"]
    PROJ["project-mcp-servers.mjs<br/>agentbox/scripts/project-mcp-servers.mjs:1"]
    GATE["gateOpen(x-agentbox-gate)<br/>agentbox/scripts/project-mcp-servers.mjs:234"]
    REQ["requiresMet(x-agentbox-requires)<br/>agentbox/scripts/project-mcp-servers.mjs:248"]
    LEDGER["ownership ledger<br/>agentbox/scripts/project-mcp-servers.mjs:265"]
    WS[".mcp.json workspace target<br/>entrypoint-unified.sh:910 WORKSPACE/.mcp.json"]
    BESPOKE["bespoke entries hand-written earlier in entrypoint<br/>harness-bridge, precedent-bridge, claude-flow, browser-gpu —<br/>entrypoint-unified.sh:1445,1465"]

    REG -- "mirror shared server wiring (skills/mcp.json:3 comment)" --> MIRROR
    REG -- "nix bake" --> BAKE
    BAKE -- "MCP_REGISTRY env" --> PROJ
    PROJ --> GATE
    PROJ --> REQ
    GATE -- "x-agentbox-managed-by == projector only" --> PROJ
    REQ -- "bin on PATH / file exists / envset non-empty" --> PROJ
    PROJ --> LEDGER
    LEDGER -- "D1: owned name whose definition vanished is removed + recorded" --> WS
    BESPOKE -- "written first, entrypoint-unified.sh:1655 runs projector AFTER" --> WS
    PROJ -- "reconcile: upsert projector-managed servers, remove closed-gate ones" --> WS
```

## AB-22.11 Vault path authority for skill-side corpus readers (ADR-2028)

```mermaid
sequenceDiagram
    autonumber
    participant EP as entrypoint _ab_vault_resolve<br/>agentbox/config/entrypoint-unified.sh:81
    participant TOML as agentbox.toml [vault]<br/>agentbox/agentbox.toml:680
    participant ENV as exported VAULT_ROOT/PAGES/FORMAT/TUI<br/>agentbox/config/entrypoint-unified.sh:124
    participant SK as vault-writing skills<br/>podcast-knowledge-ingest, web-summary note-link mode
    participant VF as vault-frontmatter.js<br/>agentbox/mcp/servers/lib/vault-frontmatter.js:242
    participant CI as check-no-logseq-paths.sh<br/>agentbox/scripts/ci/check-no-logseq-paths.sh:18

    EP->>TOML: _ab_toml_val vault root (entrypoint-unified.sh:86)
    alt no [vault] in agentbox.toml — VAULT_ROOT empty
        EP->>EP: unset VAULT_ROOT/PAGES/FORMAT/TUI/WORKING_ROOT/WORKING_PAGES/TRANSCRIPTS<br/>(entrypoint-unified.sh:88)
        EP-->>ENV: echo "[vault] disabled — no [vault] in agentbox.toml" (entrypoint-unified.sh:91)
        alt AGENTBOX_VAULT_LEGACY_PATHS=1 opt-in
            EP-->>ENV: RETAIN deprecated ONTOLOGY_PAGES_DIR with a warning (entrypoint-unified.sh:96-98)
        else no opt-in
            EP-->>ENV: WARNING then clear ONTOLOGY_PAGES_DIR="" (entrypoint-unified.sh:103-107)
        end
        SK->>SK: consumer sees no VAULT_PAGES and no retained override — disables itself, one clear line<br/>(ADR-2028 D3, fail-loud)
    else [vault].root present
        EP->>EP: VAULT_PAGES = root/pages, VAULT_FORMAT default obsidian, VAULT_TUI default none<br/>(entrypoint-unified.sh:111-115)
        EP->>ENV: export VAULT_ROOT VAULT_PAGES VAULT_FORMAT VAULT_TUI ... (entrypoint-unified.sh:124)
        opt explicit ONTOLOGY_PAGES_DIR differs from VAULT_PAGES
            EP-->>ENV: note override honoured for legacy consumers, VAULT_PAGES remains the authority<br/>(entrypoint-unified.sh:130-131)
        end
        ENV-->>SK: every supervised program, tmux window, MCP server inherits<br/>VAULT_ROOT/VAULT_PAGES/VAULT_FORMAT
        SK->>VF: ensureFrontmatter(text, extraProps, opts) before writing a page<br/>(vault-frontmatter.js:242)
        VF-->>SK: V2 YAML frontmatter — public as real boolean, wikilinks quoted, legacy key:: value block<br/>converted
    end
    CI->>CI: grep -rn "workspace/logseq" outside docs/archive and docs/adr<br/>(check-no-logseq-paths.sh:18-24)
    alt hard-coded corpus literal found
        CI-->>EP: FAIL — exit 1, points at ADR-2028 remediation (check-no-logseq-paths.sh:29-40)
    else clean
        CI-->>EP: PASS (check-no-logseq-paths.sh:43)
    end
```

## AB-22.12 tree-search-coder — execution-gated branching with enforced spend cap

```mermaid
sequenceDiagram
    autonumber
    participant U as User/coordinator (explicit invocation only, never auto-routed)
    participant TSC as tree-search-coder SKILL<br/>agentbox/skills/tree-search-coder/SKILL.md:80
    participant CAP as tree-search-cap CLI<br/>agentbox/services/agentbox-ops/src/bin/tree-search-cap.rs
    participant LED as cost_cap ledger (flock)<br/>agentbox/services/agentbox-ops/src/cost_cap/mod.rs:361
    participant SC as sparc:coder
    participant KS as KernelSession (code-interpreter MCP)

    U->>TSC: /tree-search-coder <task> (manifest_gate [skills.tree_search_coder] enabled=true)
    loop candidate k = 1..max_candidates (default 5, agentbox.toml:625)
        TSC->>CAP: tree-search-cap reserve --run RUN_ID --estimate 0.13 (algorithm.md:60-63)
        CAP->>LED: reserve_at(run_id, estimate, now) under exclusive flock (cost_cap/mod.rs:361-366)
        break exit 3 REFUSED — spend_cap_exceeded, candidate_limit_exceeded, branch_timeout, or capability_disabled
            LED-->>CAP: refuse reservation
            CAP-->>TSC: exit 3 (algorithm.md:72-75)
            TSC-->>U: return best candidate found so far, annotated halted true reason spend_cap<br/>(algorithm.md:21-24)
        end
        CAP-->>TSC: exit 0 granted — reservation id, candidate_index, remaining_usd
        TSC->>SC: sparc:coder with varied temperature/framing (algorithm.md:6-8)
        SC-->>TSC: candidate program
        TSC->>KS: kernel.reset (fresh session per branch, algorithm.md:9-11)
        TSC->>KS: kernel.exec candidate's assertions/tests (algorithm.md:12-14)
        KS-->>TSC: ExecutionTrace — assertion-pass count
        alt branch succeeds within per_branch_timeout_s
            TSC->>CAP: tree-search-cap settle --run RUN_ID --reservation res-id --actual 0.11 (algorithm.md:65)
        else branch fails or is cancelled — hold must still be released
            TSC->>CAP: tree-search-cap settle --run RUN_ID --reservation res-id --actual 0.00 --failed<br/>(algorithm.md:67)
        end
        CAP->>LED: settle_at(...) — release hold, charge actual (cost_cap/mod.rs:434-444)
    end
    TSC->>TSC: score by assertion-pass count, select highest, tie-break shortest code<br/>(algorithm.md:16-18)
    TSC-->>U: chosen candidate + audit trajectory JSONL for ExpeL distillation (algorithm.md:27-28)
    Note over TSC,LED: INVARIANT (ADR-2020): no default-unlimited mode — an absent spend_cap_usd falls back to<br/>the documented 0.50 USD default, never to infinity (algorithm.md:24,<br/>agentbox.toml:625-627)
    Note over TSC,U: INVARIANT: never auto-routed — SKILL-DIRECTORY.md and skill-router's routing table<br/>exclude tree-search-coder from automatic dispatch (algorithm.md manifest gate section)
```

## AB-22.13 [skills.*] manifest gate table (ADR-2020)

```mermaid
flowchart TB
    TOML["agentbox.toml [skills.*] blocks<br/>agentbox/agentbox.toml"]
    MANI["system-manifest.js catalogue<br/>agentbox/management-api/lib/system-manifest.js:42"]

    subgraph rebuild["apply_class rebuild — Nix package set + supervisor block, image rebuild required"]
        CI2["code_interpreter :534<br/>system-manifest.js:140"]
        CODEACT["codeact :550"]
        ACI["aci_shell :578<br/>system-manifest.js:223"]
        TSCB["tree_search_coder :620<br/>system-manifest.js:226"]
        RES["research.web_researcher :522<br/>system-manifest.js:143"]
    end
    subgraph boot["apply_class boot — env/manifest re-read at container boot, no rebuild"]
        RVB["ruvnet_brain :586<br/>system-manifest.js:204"]
        ONT["ontology :629<br/>system-manifest.js:207"]
    end
    subgraph unwired["gated in TOML but NOT checked at boot — verified gap"]
        HARN["harness :694 enabled=true"]
        PREC["precedent :702 enabled=true"]
    end
    subgraph vault["[vault] — sibling top-level section, not [skills.*]"]
        VLT["vault :680 — ADR-2028 path authority"]
    end

    TOML --> MANI
    MANI --> rebuild
    MANI --> boot
    TOML -.->|"PROPOSED ADR-2057: entrypoint-unified.sh:1442,1462 register on file presence only — the enabled flag is never read, and neither gate has a system-manifest.js apply_class. Routed to the flake/manifest owner."| unwired
    TOML --> vault

    HARN -->|"file exists at /opt/agentbox/mcp/servers/harness-bridge.js"| REG2["harness-bridge registered in .mcp.json regardless of [skills.harness].enabled"]
    PREC -->|"file exists at /opt/agentbox/mcp/servers/precedent-bridge.js"| REG3["precedent-bridge registered in .mcp.json regardless of [skills.precedent].enabled"]

    TSCB -->|"enabled=false refuses every tree-search-cap reservation"| CAPGATE["cost_cap ledger — manifest gate check, algorithm.md Enforced cost cap"]
```
