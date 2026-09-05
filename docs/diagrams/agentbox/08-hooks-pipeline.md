---
id: AB-08
title: Claude Code hook pipeline and its handlers
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2015, ADR-2026, ADR-2007]
sources:
  - agentbox/config/hooks/claude-flow-hook-adapter.cjs
  - agentbox/config/hooks/trust-seed.cjs
  - agentbox/config/hooks/nostr-live-mirror.cjs
  - agentbox/config/hooks/project-tracking-publish.cjs
  - agentbox/config/hooks/ontology-monitor.cjs
  - agentbox/config/hooks/ruvnet-brain-ground.cjs
  - agentbox/config/hooks/trajectory-recorder.cjs
  - agentbox/config/hooks/dream-inbox-surface.cjs
  - agentbox/config/hooks/fleet-session-start.sh
  - agentbox/config/hooks/fleet-tab-name.sh
  - agentbox/config/hooks/lib/trajectory-util.cjs
  - agentbox/services/agentbox-manifest/src/stacks.rs
  - agentbox/services/agentbox-manifest/src/stacks_env.rs
  - agentbox/config/entrypoint-unified.sh
  - agentbox/agentbox.toml
verified_commit: b00c28a0d
---
## AB-08.1 Hook registration table part 1 — tool, prompt and session events
```mermaid
flowchart TB
    subgraph PTUG["PreToolUse"]
    direction TB
    PTU0["PreToolUse :27"]
    PTU1["Bash matcher :29<br/>hook-handler.cjs pre-bash #quot;or true#quot; :33<br/>timeout 5000"]
    PTU2["Write#124;Edit#124;MultiEdit matcher :39<br/>hook-handler.cjs pre-edit #quot;or true#quot; :43<br/>timeout 5000"]
    PTU3["no matcher :48<br/>inline sh aoe-hooks :52<br/>writes running#124;waiting status, no timeout field"]
    PTU0 --> PTU1 --> PTU2 --> PTU3
    end
    subgraph POTUG["PostToolUse"]
    direction TB
    POTU0["PostToolUse :57"]
    POTU1["Write#124;Edit#124;MultiEdit :59<br/>hook-handler.cjs post-edit :63<br/>timeout 10000"]
    POTU2["Bash :69<br/>hook-handler.cjs post-bash :73<br/>timeout 5000"]
    POTU3["AskUserQuestion matcher :79<br/>inline sh aoe-hooks :83<br/>writes running, no timeout field"]
    POTU0 --> POTU1 --> POTU2 --> POTU3
    end
    subgraph UPSG["UserPromptSubmit"]
    direction TB
    UPS0["UserPromptSubmit :88"]
    UPS1["group1 :90<br/>hook-handler.cjs route :93 t=10000<br/>nostr-live-mirror.cjs UserPromptSubmit :98 t=8000"]
    UPS2["group2 :104<br/>ruvnet-brain-ground.cjs :107 t=5000<br/>#40;baked /opt path#41;"]
    UPS3["group3 :113<br/>tab0-bridge turn-sink.cjs UserPromptSubmit :116 t=5000"]
    UPS4["group4 :122<br/>dream-inbox-surface.cjs :125 t=5000<br/>#40;live checkout path#41;"]
    UPS5["group5 :131<br/>aoe __extract-session-id :134<br/>+ inline sh aoe-hooks status=running :137"]
    UPS0 --> UPS1 --> UPS2 --> UPS3 --> UPS4 --> UPS5
    end
    subgraph SSG["SessionStart"]
    direction TB
    SS0["SessionStart :143"]
    SS1["group1 :145<br/>hook-handler.cjs session-restore :148 t=15000<br/>auto-memory-hook.mjs import :152 t=8000<br/>nostr-live-mirror.cjs SessionStart :157 t=8000<br/>hermes-scheduler start :162 t=10000"]
    SS2["group2 :169<br/>fleet-session-start.sh :171 t=8000<br/>#40;live checkout path#41;"]
    SS3["group3 :178<br/>trust-seed.cjs :180 t=8000<br/>#40;baked /opt path#41;"]
    SS4["group4 :187<br/>aoe __extract-session-id :189, no timeout field"]
    SS0 --> SS1 --> SS2 --> SS3 --> SS4
    end
    subgraph SEG["SessionEnd"]
    direction TB
    SE0["SessionEnd :195"]
    SE1["group1 :197<br/>hook-handler.cjs session-end :199 t=10000<br/>nostr-live-mirror.cjs SessionEnd :204 t=8000"]
    SE0 --> SE1
    end
    subgraph STG["Stop"]
    direction TB
    ST0["Stop :211"]
    ST1["group1 :213<br/>auto-memory-hook.mjs sync :215 t=10000<br/>nostr-live-mirror.cjs Stop :220 t=8000"]
    ST2["group2 :227<br/>tab0-bridge turn-sink.cjs Stop :229 t=5000"]
    ST3["group3 :236<br/>trajectory-recorder.cjs Stop :238 t=10000<br/>#40;baked /opt path, RUVECTOR_* env inline#41;"]
    ST4["group4 :245<br/>inline sh aoe-hooks status=idle :247"]
    ST0 --> ST1 --> ST2 --> ST3 --> ST4
    end
```

## AB-08.2 Hook registration table part 2 — lifecycle, notification and divergences
```mermaid
flowchart TB
    subgraph PCG["PreCompact"]
    direction TB
    PC0["PreCompact :253"]
    PC1["matcher=manual :255<br/>hook-handler.cjs compact-manual :258<br/>+ session-end :262 t=5000"]
    PC2["matcher=auto :269<br/>hook-handler.cjs compact-auto :272<br/>+ session-end :276 t=6000"]
    PC0 --> PC1 --> PC2
    end
    subgraph SASG["SubagentStart"]
    direction TB
    SAS0["SubagentStart :283"]
    SAS1["group1 :285<br/>hook-handler.cjs status :287 t=3000"]
    SAS0 --> SAS1
    end
    subgraph SASTG["SubagentStop"]
    direction TB
    SAST0["SubagentStop :294"]
    SAST1["group1 :296<br/>hook-handler.cjs post-task :298 t=5000"]
    SAST2["group2 :305<br/>trajectory-recorder.cjs SubagentStop :307 t=10000<br/>#40;same handleClose#40;#41; as Stop, see AB-08.14#41;"]
    SAST0 --> SAST1 --> SAST2
    end
    subgraph NG["Notification"]
    direction TB
    N0["Notification :314"]
    N1["no matcher :316<br/>hook-handler.cjs notify :318 t=3000"]
    N2["permission_prompt#124;elicitation_dialog#124;agent_needs_input :325<br/>inline sh aoe-hooks status=waiting :328"]
    N3["idle_prompt#124;agent_completed :334<br/>inline sh aoe-hooks status=idle :337"]
    N0 --> N1 --> N2 --> N3
    end
    subgraph SFG["StopFailure"]
    direction TB
    SF0["StopFailure :343"]
    SF1["group1 :345<br/>inline sh aoe-hooks status=idle :347"]
    SF0 --> SF1
    end
    subgraph ERG["ElicitationResult"]
    direction TB
    ER0["ElicitationResult :353"]
    ER1["group1 :355<br/>inline sh aoe-hooks status=running :357"]
    ER0 --> ER1
    end
    subgraph DIVG["divergences"]
    direction TB
    DIV1["DIVERGENCE: claude-flow-hook-adapter.cjs is never<br/>referenced by ~/.claude/settings.json #40;grep -n found 0 hits#41;.<br/>It is wired only into PER-PROFILE settings.json under<br/>workspace/profiles/#60;stack#62;/.claude/ by stacks.rs<br/>learning_hooks#40;#41; #40;stacks.rs:27-63#41;, default path<br/>stacks_env.rs:44-45. This tree's live session uses<br/>hook-handler.cjs instead #40;see AB-08.9#41;."]
    DIV2["DIVERGENCE: agentbox.toml :102 sets<br/>#91;ontology_monitor#93; enabled=true, and stacks.rs :45-48<br/>wires ontology-monitor.cjs into SessionEnd when<br/>gates.ontology_monitor is set — but this live root<br/>settings.json's SessionEnd :195-209 has NO<br/>ontology-monitor.cjs entry. project-tracking-publish.cjs<br/>is wired NOWHERE #40;no hook, no entrypoint, no stacks.rs<br/>reference found#41; — it is a standalone CLI shelled by the<br/>management API, not a Claude Code hook."]
    DIV3["DIVERGENCE: baked-vs-live path split.<br/>#47;opt#47;agentbox#47;... #40;image copy#41;: ruvnet-brain-ground.cjs :107,<br/>trust-seed.cjs :181, trajectory-recorder.cjs :239/:308.<br/>#47;home#47;devuser#47;workspace#47;project#47;agentbox#47;... #40;live checkout#41;:<br/>nostr-live-mirror.cjs :98/158/205/221,<br/>dream-inbox-surface.cjs :125, fleet-session-start.sh :172."]
    DIV1 --> DIV2 --> DIV3
    end
```

## AB-08.3 PreToolUse — Bash and Write/Edit/MultiEdit gates
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core
    participant HH as hook-handler.cjs<br/>~/.claude/helpers/hook-handler.cjs (out of tree)
    participant AOE as aoe-hooks inline sh<br/>settings.json:49-54

    CC->>HH: stdin JSON, matcher=Bash (settings.json:29)
    Note over HH: command node /home/devuser/.claude/helpers/hook-handler.cjs pre-bash #124;#124; true (:33)<br/>timeout 5000ms, fail-open via #124;#124; true
    HH-->>CC: exit 0 (adapter logic out of tree, not diagrammed here)

    CC->>HH: stdin JSON, matcher=Write#124;Edit#124;MultiEdit (:39)
    Note over HH: pre-edit action (:43) t=5000, fail-open #124;#124; true
    HH-->>CC: exit 0

    CC->>AOE: stdin JSON, no matcher (:48-54), every PreToolUse call
    Note over AOE: strict mode: unset IFS, set -f, umask 077 (:52)<br/>requires AOE_INSTANCE_ID set + #91;0-9a-zA-Z_-#93; only, else exit 0
    alt AOE_INSTANCE_ID unset or has bad chars
        AOE-->>CC: exit 0, no write
    else instance id valid
        AOE->>AOE: mkdir/verify /tmp/aoe-hooks-1000 mode drwx------ owned by self
        AOE->>AOE: mkdir/verify .../$AOE_INSTANCE_ID same perms+owner
        AOE->>AOE: read stdin, tool_name==AskUserQuestion -> status=waiting else running
        AOE->>AOE: printf status > $D/status
    end
    Note over AOE: INVARIANT: every ownership/mode check that fails exits 0 silently -<br/>ADR-2007 profile isolation boundary for the status file tree
```
## AB-08.4 PostToolUse — post-edit, post-bash, AskUserQuestion status flip
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core
    participant HH as hook-handler.cjs<br/>~/.claude/helpers/hook-handler.cjs (out of tree)
    participant AOE as aoe-hooks inline sh<br/>settings.json:82-86

    CC->>HH: stdin JSON, matcher=Write#124;Edit#124;MultiEdit (:59)
    Note over HH: post-edit action (:63) t=10000, fail-open #124;#124; true
    HH-->>CC: exit 0

    CC->>HH: stdin JSON, matcher=Bash (:69)
    Note over HH: post-bash action (:73) t=5000, fail-open #124;#124; true
    HH-->>CC: exit 0

    CC->>AOE: stdin JSON, matcher=AskUserQuestion (:79-86)
    Note over AOE: identical drwx------/owner guard chain as AB-08.3<br/>#40;settings.json:82-86 mirrors :49-54 verbatim#41;
    AOE->>AOE: printf running > $D/status unconditionally on this matcher
    Note over AOE: no waiting/idle branch here - PostToolUse#40;AskUserQuestion#41;<br/>always means the question tool RAN, session is running again
```
## AB-08.5 UserPromptSubmit — five handler groups, injected context
```mermaid
sequenceDiagram
    autonumber
    participant U as User turn
    participant HH as hook-handler.cjs<br/>route (out of tree)
    participant NM as nostr-live-mirror.cjs<br/>agentbox/config/hooks/nostr-live-mirror.cjs:349
    participant RBG as ruvnet-brain-ground.cjs<br/>agentbox/config/hooks/ruvnet-brain-ground.cjs:37
    participant TS as turn-sink.cjs<br/>tab0-bridge (out of tree)
    participant DI as dream-inbox-surface.cjs<br/>agentbox/config/hooks/dream-inbox-surface.cjs:24
    participant AOE as aoe __extract-session-id + aoe-hooks<br/>settings.json:133-140
    participant M as Model context

    U->>HH: stdin JSON group1 (:90-95)
    Note over HH: route --task PROMPT t=10000, stdout INJECTED into context<br/>#40;claude-flow-hook-adapter.cjs:41-46 documents the same contract#41;
    HH-->>M: [INFO]/[INTELLIGENCE]/pattern lines only (allowlist filter)
    U->>NM: stdin JSON, arg=UserPromptSubmit (:98) t=8000
    alt AGENTBOX_LIVE_MIRROR=0
        NM-->>U: return 0, silent no-op (:355)
    else no derivable child key AND no recipient pubkey
        NM-->>U: return 0, silent no-op (:356-358)
    else gated on
        NM->>NM: bodyForEvent#40;#41; = #129;[shortId] prompt text (:274-278)
        NM->>NM: mintActivityUrn#40;#41; REC-9 urn:agentbox:activity ref (:159-170)
        NM->>NM: nip59.wrapEvent#40;#41; gift-wrap kind 1059 over kind-14 rumor (:398-408)
        NM->>NM: publishWrap#40;#41; to cloud relay, DEADLINE_MS=6000 (:297-331,45,49)
        Note over NM: fail-open at every step - wrap/publish errors are logged and swallowed (:410-419)<br/>see AB-08.10 for full detail
    end
    par independent groups, unordered wrt each other
        U->>RBG: stdin JSON group2 (:104-109), baked /opt path
        RBG-->>M: JSON #123;result:continue, additionalContext#125; if RuvNet/classical-sub match (:71-78)<br/>see AB-08.12
    and
        U->>TS: stdin JSON group3 (:113-118)
        Note over TS: tab0-bridge/turn-sink.cjs is outside agentbox/ - not diagrammed here
    and
        U->>DI: stdin JSON group4 (:122-127)
        DI-->>M: JSON #123;result:continue, additionalContext#125; if dream-inbox items due<br/>see AB-08.13
    end
    U->>AOE: group5 (:131-141)
    AOE->>AOE: aoe __extract-session-id if AOE_INSTANCE_ID set and aoe binary present (:134)
    AOE->>AOE: same drwx------ guard chain as AB-08.3, status=running (:138)
```
## AB-08.6 SessionStart — restore, mirror, fleet tab, trust seed
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core
    participant HH as hook-handler.cjs<br/>session-restore (out of tree)
    participant AM as auto-memory-hook.mjs<br/>(out of tree, CLAUDE_PROJECT_DIR or ~/.claude/helpers)
    participant NM as nostr-live-mirror.cjs<br/>agentbox/config/hooks/nostr-live-mirror.cjs:349
    participant HS as hermes-scheduler CLI
    participant FS as fleet-session-start.sh<br/>agentbox/config/hooks/fleet-session-start.sh:1
    participant FTN as fleet-tab-name.sh<br/>agentbox/config/hooks/fleet-tab-name.sh:1
    participant TSD as trust-seed.cjs<br/>agentbox/config/hooks/trust-seed.cjs:70
    participant AOE as aoe __extract-session-id<br/>settings.json:189

    CC->>HH: group1 step1 (:145-149) t=15000
    Note over HH: session-restore, stdout INJECTED filtered by RESTORE_SIGNAL<br/>#40;mirrors adapter.cjs:47 allowlist pattern#41;
    HH-->>CC: exit 0
    CC->>AM: group1 step2 (:152-154) t=8000
    Note over AM: resolves CLAUDE_PROJECT_DIR/.claude/helpers/auto-memory-hook.mjs<br/>else falls back to ~/.claude/helpers/, runs import subcmd
    CC->>NM: group1 step3 (:157-159) arg=SessionStart t=8000
    NM-->>CC: bodyForEvent SessionStart = #9654; session shortId started#40;source#41; (:270-273), see AB-08.10
    CC->>HS: group1 step4 (:162-164) t=10000
    Note over HS: sh -c hermes-scheduler start #62;/dev/null 2#62;#38;1 #124;#124; true, always succeeds
    CC->>FS: group2 (:169-173) t=8000, live checkout path
    FS->>FTN: bash fleet-tab-name.sh (:17)
    Note over FTN: no TMUX or no tmux binary -#62; exit 0 (:14-15)<br/>else name = git remote basename #62; toplevel #62; cwd (:19-27)
    FTN->>FTN: tmux rename-window, automatic-rename off (:32-34)
    FTN->>FTN: write $HOME/.claude/fleet/#36;#123;win#125;.json registry entry (:37-40)
    FS->>FS: opt gateway not running, AGENTBOX_NOSTR_GATEWAY!=0 (:19-20)<br/>-#62; nohup node gateway.cjs, disown (:23-24)
    FS->>FS: opt deploy.sh exists, AGENTBOX_TAB0_BRIDGE!=0 (:34)<br/>-#62; nohup bash deploy.sh, disown (:35-36)
    CC->>TSD: group3 (:178-182) t=8000, baked /opt path
    Note over TSD: targets = WORKSPACE #43; findRepos depth 5 #43; extra argv (:72)<br/>findRepos walks git roots + Cargo.toml#124;package.json#124;flake.nix dirs (:51-68)
    TSD->>TSD: for each target: set hasTrustDialogAccepted+hasCompletedProjectOnboarding true in ~/.claude.json projects map (:80-85)
    alt any entry newly trusted
        TSD->>TSD: backup ~/.claude.json to WORKSPACE/.agentbox/claude.json.pre-trust-seed then write (:93-98)
    else already trusted / dry-run
        TSD-->>CC: log only, no write (:86-89)
    end
    Note over TSD: fail-open: any thrown error caught at top level, stderr only, exit implicit 0 (:102)
    CC->>AOE: group4 (:187-191)
    Note over AOE: aoe __extract-session-id only if AOE_INSTANCE_ID set and aoe on PATH, no status file write here
```
## AB-08.7 SessionEnd and Stop — consolidation, mirror, trajectory close
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core
    participant HH as hook-handler.cjs<br/>session-end (out of tree)
    participant AM as auto-memory-hook.mjs sync<br/>(out of tree)
    participant NM as nostr-live-mirror.cjs<br/>agentbox/config/hooks/nostr-live-mirror.cjs:349
    participant TS as turn-sink.cjs Stop<br/>tab0-bridge (out of tree)
    participant TR as trajectory-recorder.cjs<br/>agentbox/config/hooks/trajectory-recorder.cjs:507
    participant AOE as aoe-hooks status=idle<br/>settings.json:245-249 / :343-348

    rect rgb(240,240,255)
    Note over CC,NM: SessionEnd (:195-209)
    CC->>HH: group1 step1 (:199-201) t=10000
    CC->>NM: group1 step2 (:204-206) arg=SessionEnd t=8000
    NM-->>CC: bodyForEvent SessionEnd = #9632; session shortId ended#40;reason#41; (:284-287)
    end
    rect rgb(255,245,235)
    Note over CC,AOE: Stop (:211-251)
    CC->>AM: group1 step1 (:216-217) t=10000, sync
    CC->>NM: group1 step2 (:220-222) arg=Stop t=8000
    NM-->>CC: bodyForEvent Stop = last assistant text from transcript_path (:279-283,244-260)
    CC->>TS: group2 (:227-231) arg=Stop t=5000
    CC->>TR: group3 (:236-240) arg=Stop t=10000, RUVECTOR_MEMORY_LEARNING_ENABLED=1 RUVECTOR_RECORD_TRAJECTORIES=1 inline env
    alt both gates on (gateOn checks :55-58,511)
        TR->>TR: handleClose#40;payload#41; - see AB-08.14 for full detail
    else either gate off (default)
        TR-->>CC: return 0 immediately (:511-513), byte-identical to no hook present
    end
    CC->>AOE: group4 (:245-249) status=idle unconditionally on Stop
    end
    Note over CC,AOE: StopFailure (settings.json:343-348) is the SAME aoe-hooks status=idle<br/>body as Stop group4, registered on the failure event instead
```
## AB-08.8 claude-flow-hook-adapter.cjs — stdin-to-CLI translation (per-profile stacks only)
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core<br/>#40;profile stack, e.g. claude-core#41;
    participant AD as claude-flow-hook-adapter.cjs<br/>agentbox/config/hooks/claude-flow-hook-adapter.cjs:106
    participant CLI as claude-flow hooks CLI<br/>AGENTBOX_FLOW_BIN env, default claude-flow (:32)
    participant OP as ontology-push.js<br/>mcp/servers/lib #40;optional#41;
    participant M as Model context

    Note over CC,AD: registered by stacks.rs learning_hooks#40;#41; #40;stacks.rs:27-63#41;<br/>into workspace/profiles/#60;name#62;/.claude/settings.json - NOT this session's root settings.json
    CC->>AD: stdin JSON, argv[2]=action (:106-109)
    AD->>AD: parsePayload#40;readStdin#40;#41;#41; - malformed/empty JSON -#62; #123;#125; (:49-65)
    AD->>AD: extract tool_input.file_path, tool_input.command, prompt (:109-113)
    alt action=route
        AD->>CLI: spawnSync claude-flow hooks route --task PROMPT t=12000 (:117,67-73)
        CLI-->>AD: stdout #40;latency/alternatives dump, WASM-fallback banner possible#41;
        AD->>AD: filter stdout by ROUTE_SIGNAL regex #40;INFO#124;INTELLIGENCE#124;Agent:#124;Matched Pattern#41; (:46,74-81)
        AD-->>M: filtered lines only - full dump would pollute every turn (:41-46)
        opt ONTOLOGY_INJECT set and prompt non-empty
            AD->>OP: require ontology-push.js, getOntologyBreadcrumb#40;prompt#41; (:87-104)
            OP-->>AD: breadcrumb line or throws
            AD-->>M: #91;ONTOLOGY#93; breadcrumb appended, fail-open on any require/call error
        end
    else action=pre-edit #40;file present#41;
        AD->>CLI: hooks pre-edit --file FILE t=5000 (:120-122)
    else action=post-edit #40;file present#41;
        AD->>CLI: hooks post-edit --file FILE t=10000 (:123-125)
    else action=pre-command #40;command present#41;
        AD->>CLI: hooks pre-command --command CMD t=5000 (:126-128)
    else action=post-command #40;command present#41;
        AD->>CLI: hooks post-command --command CMD t=5000 (:129-131)
    else action=session-restore
        AD->>CLI: hooks session-restore t=15000 (:132-134)
        CLI-->>AD: stdout filtered by RESTORE_SIGNAL (:47,74-81)
        AD-->>M: filtered lines
    else action=session-end
        AD->>CLI: hooks session-end t=10000 (:135-137)
    else unknown action
        AD->>AD: no-op, never signals error (:138-140)
    end
    Note over AD: TRANSFORMERS_CACHE/HF_HOME forced to writable tmpfs path (:33-39,71)<br/>else @xenova/transformers ENOENT#39;s against the read-only Nix store
    Note over AD: whole main#40;#41; wrapped in try/catch, process.exit#40;0#41; unconditional (:144-149)<br/>INVARIANT: adapter holds NO learning state, intelligence lives in the CLI backend<br/>#40;header :13-17 cites stale ADR-015, pre-2026-consolidation id#41;
```
## AB-08.9 nostr-live-mirror.cjs — NIP-59 gift-wrapped turn mirror
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core
    participant NM as nostr-live-mirror.cjs<br/>agentbox/config/hooks/nostr-live-mirror.cjs:349
    participant NT as nostr-tools #40;nip59#41;<br/>management-api or mcp node_modules (:93-104)
    participant WS as ws WebSocket lib<br/>same candidate search (:106-117)
    participant R as cloud relay<br/>wss://dreamlab-nostr-relay.solitary-paper-764d.workers.dev (:45)

    CC->>NM: node nostr-live-mirror.cjs #60;event#62;, stdin JSON (:350-351)
    Note over NM: ADR-2026 session-mirror egress boundary: transport is EXCLUSIVELY<br/>this cloud relay, NEVER relay.damus.io/relay.primal.net (:18-24)
    alt AGENTBOX_LIVE_MIRROR=0
        NM-->>CC: return 0, silent no-op (:355)
    else
        NM->>NM: deriveChildKey#40;#41; HMAC-SHA256#40;operator_sk, tag#124;agentbox-mirror-v1#41; (:203-215)
        alt no child key AND no explicit recipient pubkey #40;AGENTBOX_PUBKEY etc, :70-78#41;
            NM-->>CC: return 0, silent no-op (:358)
        else gated on
            NM->>NM: readStdin#40;#41; -#62; parse JSON payload, fail-open to #123;#125; (:360-364)
            NM->>NM: bodyForEvent#40;event, payload#41; - null for e.g. empty prompt/no assistant text (:267-291)
            alt body is null or empty
                NM-->>CC: return 0 (:367)
            else
                NM->>NM: mintActivityUrn#40;loadUris#40;#41;, payload#41; - REC-9 urn:agentbox:activity ref (:159-170)
                NM->>NM: composeBody#40;text, urn#41; - cap MAX_BODY_CHARS=4000, urn NEVER truncated (:52,177-192)
                opt AGENTBOX_MIRROR_DRY_RUN=1
                    NM-->>CC: log composed body to stderr, NO network egress, return 0 (:378-382)
                end
                NM->>NT: loadNostrTools#40;#41; #43; loadWs#40;#41; - require fails -#62; null (:93-117)
                alt tools/nip59/WS unavailable
                    NM-->>CC: log skip, return 0 (:386-389)
                else
                    NM->>NT: nip59.wrapEvent#40;rumor kind=14, sk, recipient#41; - recipient FIRST in #91;#39;p#39;#93; tag for relay whitelist (:396-409)
                    NT-->>NM: gift wrap kind=1059
                    NM->>WS: publishWrap#40;WS, mirrorRelay#40;#41;, wrap, DEADLINE_MS=6000#41; (:297-331,416)
                    WS->>R: EVENT frame over wss://
                    R-->>WS: OK frame or timeout
                    Note over NM,R: guard setTimeout DEADLINE_MS#43;1500 force-exits the whole process (:436-437)<br/>INVARIANT: every wrap/publish error is caught and swallowed (:410-419)
                end
            end
        end
    end
```
## AB-08.10 trust-seed.cjs — folder-trust and worktree discovery
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code SessionStart
    participant TSD as trust-seed.cjs<br/>agentbox/config/hooks/trust-seed.cjs:70
    participant FS as filesystem
    participant CFG as ~/.claude.json<br/>trust-seed.cjs:27

    CC->>TSD: node trust-seed.cjs #91;--depth N#93; #91;--dry-run#93; #91;extra-path...#93; (:20,30-39)
    TSD->>FS: findRepos#40;WORKSPACE, depth=5, #91;#93;#41; recursive walk (:57-68)
    loop each dir entry, skip node_modules#124;target#124;.git#124;dist#124;build etc (:28,62)
        FS->>TSD: isProjectDir#40;p#41; = isGitRoot#40;p#41; OR has Cargo.toml#124;package.json#124;pyproject.toml#124;flake.nix#124;justfile (:41-55)
        opt isProjectDir true
            TSD->>TSD: acc.push#40;p#41; (:64)
        end
        TSD->>FS: recurse findRepos#40;p, depth-1, acc#41; (:65)
    end
    TSD->>TSD: targets = Set#40;WORKSPACE, ...repos, ...extra argv resolved#41; (:72)
    TSD->>CFG: JSON.parse#40;readFileSync#40;CONFIG#41;#41; - ENOENT tolerated, other errors abort (:74-76)
    loop each target dir
        alt cfg.projects#91;dir#93; already hasTrustDialogAccepted AND hasCompletedProjectOnboarding
            TSD->>TSD: skip, no change (:82)
        else
            TSD->>TSD: cfg.projects#91;dir#93; = #123;...entry, hasTrustDialogAccepted:true, hasCompletedProjectOnboarding:true#125; (:83-84)
        end
    end
    alt --dry-run OR added===0
        TSD-->>CC: stdout summary line only, CFG untouched (:86-89)
    else
        TSD->>FS: copyFileSync CONFIG to WORKSPACE/.agentbox/claude.json.pre-trust-seed backup (:93-97)
        TSD->>CFG: writeFileSync CONFIG, JSON.stringify#40;cfg, null, 2#41; #43; newline (:98)
        TSD-->>CC: stdout summary line #40;N checked, M newly trusted#41; (:99)
    end
    Note over TSD: INVARIANT: never removes/overwrites OTHER per-project keys in cfg.projects (:14)<br/>fail-open: top-level try/catch, stderr-only on error, no process.exit#40;1#41; anywhere (:102)
    Note over TSD: NOT FOUND IN CODE: no permissions.defaultMode or auto-mode<br/>opt-in dialog logic exists in this 102-line file - only trust-dialog fields are written
```
## AB-08.11 ruvnet-brain-ground.cjs and ontology-monitor.cjs
```mermaid
sequenceDiagram
    autonumber
    participant U as UserPromptSubmit
    participant RBG as ruvnet-brain-ground.cjs<br/>agentbox/config/hooks/ruvnet-brain-ground.cjs:37
    participant M as Model context
    participant SE as SessionEnd
    participant OM as ontology-monitor.cjs<br/>agentbox/config/hooks/ontology-monitor.cjs:232
    participant ONT as local ontology route<br/>mcp/servers/lib/ontology-local.js
    participant ZAI as claude-zai CLI<br/>AGENTBOX_ZAI_BIN, ZAI_URL (:149-178)
    participant FB as forum broker gate<br/>NostrBridge kind 31402

    rect rgb(235,245,255)
    U->>RBG: stdin JSON, hook.userInput#124;hook.prompt (:37-44)
    RBG->>RBG: REPO_PATTERN test against 25 RuvNet repo names (:15-21,33-35,50)
    RBG->>RBG: CLASSICAL_SUBS test - pinecone#124;pgvector#124;chromadb#124;weaviate#124;langchain#124;llamaindex#124;hnswlib (:23-31,61-69)
    alt any pattern matched
        RBG-->>M: JSON #123;result:continue, additionalContext: #91;GROUNDING#93;/#91;REDIRECT#93; lines#125; (:71-75)
    else no match
        RBG-->>M: JSON #123;result:continue#125; only (:85-87)
    end
    Note over RBG: fail-open: any JSON.parse or regex error -#62; exit#40;#41; path, no process.exit#40;1#41; anywhere (:79-81)
    end
    rect rgb(255,240,240)
    SE->>OM: stdin JSON payload, gated by AGENTBOX_ONTOLOGY_MONITOR=1 (:16,70-75)
    alt master switch off, or no ZAI key, or publish mode missing MANAGEMENT_API_KEY/NOSTR_RELAYS
        OM-->>SE: log no-op, exit 0 (:233-234)
    else gated on, BUDGET_MS=180000 wall clock (:31,34)
        OM->>OM: gatherWork#40;payload#41; - git status --porcelain + transcript tail 12k chars (:86-112)
        OM->>ONT: createLocalOntology#40;#41;.classList#40;limit:100000#41; (:118-122)
        OM->>OM: matchConcepts#40;#41; word-boundary label match, MAX_CONCEPTS=8 (:32,115-144)
        alt no concepts matched
            OM-->>SE: log no-op, exit 0 (:240)
        else
            OM->>ZAI: spawnCli claude-zai -p PROMPT, STRICT JSON proposals#91;#93; (:147-188)
            ZAI-->>OM: #123;proposals: #91;#123;iri,label,kind,title,summary,rationale#125;#93;#125;, MAX_PROPOSALS=5
            OM->>OM: fingerprint#40;p#41; = sha256#40;iri#124;kind#124;normalised summary#41;, filter against seen ledger (:50-56,245-247)
            alt fresh.length===0
                OM-->>SE: log all already seen, exit 0 (:247)
            else MODE=publish #40;AGENTBOX_ONTOLOGY_MONITOR_MODE, default dryrun#41;
                OM->>FB: buildActionRequest#40;panelId, category:ontology, kind 31402#41; per proposal (:201-228)
                FB-->>OM: published, NIP-33 d-tag = panelIdFor#40;p#41; so a repeat REPLACES the prior panel (:63-67)
            else MODE=dryrun
                OM->>OM: stageLocally#40;#41; appends $AGENTBOX_STATE/ontology-proposals.jsonl (:191-200)
            end
            OM->>OM: saveSeen#40;seen#41; ledger capped to last 5000 fingerprints (:57-62,255-256)
        end
    end
    Note over OM: human approval via 31403 happens OUTSIDE this hook - it only proposes #40;PRD-014 governed elevation#41;
    end
```
## AB-08.12 project-tracking-publish.cjs and dream-inbox-surface.cjs
```mermaid
sequenceDiagram
    autonumber
    participant CALLER as management API<br/>POST /v1/projects/:id/publish #40;NOT a Claude Code hook trigger#41;
    participant PTP as project-tracking-publish.cjs<br/>agentbox/config/hooks/project-tracking-publish.cjs:202
    participant MAPI as management-api<br/>GET /v1/projects :9090 (:133-180)
    participant NPB as nostr-pod-bridge track<br/>spawnSync binary (:184-200)
    participant U as UserPromptSubmit
    participant DI as dream-inbox-surface.cjs<br/>agentbox/config/hooks/dream-inbox-surface.cjs:24
    participant INBOX as dream-inbox.json<br/>~/workspace/.agentbox/dream-inbox.json:20
    participant M as Model context

    rect rgb(245,235,255)
    Note over CALLER,NPB: DIVERGENCE: this script is shelled directly by the management API,<br/>not registered in settings.json, entrypoint-unified.sh, or stacks.rs #40;grep -0 hits#41;
    CALLER->>PTP: node project-tracking-publish.cjs, optional ProjectTrackingDigest on stdin (:202,210-225)
    alt AGENTBOX_PROJECT_TRACKING_PUBLISH===0, or bridge secrets absent (:60-66,204-206)
        PTP-->>CALLER: return 0, silent no-op
    else
        alt stdin carries a valid digest #40;project_id present#41;
            PTP->>PTP: use verbatim (:215-216)
        else stdin is a TrackedProject or empty
            PTP->>MAPI: GET /v1/projects, Authorization Bearer MANAGEMENT_API_KEY (:135-146)
            MAPI-->>PTP: project list, mapped via toDigest#40;#41; (:92-110,228-233)
        end
        loop each digest
            PTP->>NPB: spawnSync nostr-pod-bridge track, digest JSON on stdin, t=30000 (:183-200)
            NPB-->>PTP: kind-30841 addressable event, d-tag=project slug, dual-write pod inbox
        end
        PTP-->>CALLER: log published N/total, return 0 (:241)
    end
    Note over PTP: guard setTimeout DEADLINE_MS=30000#43;1500 force-exits process (:246-247)
    end
    rect rgb(235,255,240)
    U->>DI: stdin JSON #40;payload unused - reads INBOX file directly#41; (:24-28)
    DI->>INBOX: JSON.parse#40;readFileSync#40;INBOX#41;#41; (:30)
    DI->>DI: filter status===open AND now-last_surfaced #62; RESURFACE_HOURS#42;3600, slice MAX_PER_TURN=2 (:21-22,33-36)
    alt due.length===0
        DI-->>M: JSON #123;result:continue#125; only (:37,65-67)
    else
        DI->>INBOX: stamp last_surfaced=now on due items, writeFileSync (:39-40)
        DI-->>M: JSON #123;result:continue, additionalContext: #91;DREAM INBOX#93; items #43; dream-inbox.mjs answer instructions#125; (:42-58)
    end
    Note over DI: fail-open: any error #40;missing file, bad JSON#41; -#62; exit#40;#41; path (:59-61)
    end
```
## AB-08.13 trajectory-recorder.cjs — Stop/SubagentStop boundary
```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code core<br/>Stop or SubagentStop
    participant TR as trajectory-recorder.cjs<br/>agentbox/config/hooks/trajectory-recorder.cjs:555
    participant UT as trajectory-util.cjs<br/>agentbox/config/hooks/lib/trajectory-util.cjs:1
    participant STASH as os.tmpdir stash<br/>agentbox-traj-#60;sha12#62;.json :204-221
    participant PG as ruvector-postgres<br/>trajectories / trajectory_steps
    participant MAPI as management-api<br/>/v1/agent-events/emit :9090

    CC->>TR: node trajectory-recorder.cjs #60;event#62;, stdin JSON, RUVECTOR_* env inline (:555-556)
    Note over TR: ADR-2015: transcript-driven because PostToolUse does NOT fire<br/>for non-zero-exit Bash calls on this build (:22-26)
    alt NOT #40;RUVECTOR_MEMORY_LEARNING_ENABLED AND RUVECTOR_RECORD_TRAJECTORIES#41;
        TR-->>CC: return 0, byte-identical to no hook present #40;DEFAULT-OFF, :559-561#41;
    else event in #91;Stop, SubagentStop#93; (:570-571)
        TR->>STASH: readStash#40;session#41; - processedLines watermark #43; ctcPending queue (:389,204-209)
        TR->>UT: scanTranscript#40;lines, fromLine#41; grades each Bash tool_use by tool_result.is_error (:296-368)
        loop each Bash step found
            TR->>UT: redact#40;command#41; - I10 fail-closed, null command skips the step entirely (:346)
            TR->>UT: gradeResult#40;is_error,stderr,interrupted#41; -#62; success/failure + quality score (:340)
        end
        alt pg module unavailable
            TR-->>CC: log, watermark NOT advanced, lines retried next Stop #40;ADR-2015 closeout, :413#41;
        else
            TR->>STASH: ctcEmits = stash.ctcPending FIFO-drained first, bound CTC_QUEUE_MAX=2000 (:426,163)
            TR->>PG: INSERT trajectories #40;id, task, agent, status, metadata#41; ON CONFLICT DO NOTHING (:442-448)
            loop each graded step
                TR->>PG: INSERT trajectory_steps #40;id=sha12#40;tool_use_id#41;, action, result jsonb, quality#41; ON CONFLICT DO NOTHING (:468-479)
                TR->>TR: push ctcEmitBodyFromStep#40;#41; onto ctcEmits while #60; CTC_QUEUE_MAX (:487-491)
            end
            TR->>PG: UPDATE trajectories SET ended_at, status=complete, metadata#124;#124;jsonb_build_object#40;..., ctc_emit_queued, ctc_emit_carried_in#41; (:503-517)
            TR->>STASH: watermark advances ONLY after successful persist (:522)
            TR->>MAPI: emitCtcStepsBestEffort#40;ctcEmits#41; POST /v1/agent-events/emit, CTC_EMIT_CAP=200/invocation (:169-180,533)
            MAPI-->>TR: #123;attempted, deferred#125; - surplus beyond the cap is NOT dropped here (:180)
            TR->>STASH: cur.ctcPending = deferred #40;bounded CTC_QUEUE_MAX#41;, cur.ctcPendingOverflow accumulates true drops (:536-542)
            Note over TR: overflow beyond CTC_QUEUE_MAX IS dropped and logged as INCOMPLETE (:539-540)<br/>this is the only stash write outside the persistence path (:536-537)
        end
    end
    Note over TR: guard setTimeout 8000ms force-exits process regardless of PG/HTTP state (:577)
    Note over TR: see AB-21 for the learning loop this feeds #40;ReasoningBank / SONA consumers of trajectories#41;
```
## AB-08.14 lib/ shared helpers — consumers
```mermaid
flowchart TD
    LIBDIR["agentbox/config/hooks/lib/<br/>exactly one file: trajectory-util.cjs"]
    UT["trajectory-util.cjs<br/>agentbox/config/hooks/lib/trajectory-util.cjs:1"]
    LIBDIR --> UT
    F1["sha12#40;s#41; :18<br/>content-addressed step/trajectory ids"]
    F2["commandPattern#40;command#41; :38<br/>+ hasResidualSecret#40;text#41; :180"]
    F3["redact#40;command#41; :196<br/>I10 fail-closed redaction gate"]
    F4["deriveOutcome#40;toolResponse#41; :227<br/>+ gradeResult#40;isError,stderr,interrupted#41; :285"]
    F5["tokenCountOf#40;usage#41; :309<br/>+ usageIdentityOf#40;rec#41; :341"]
    F6["handoffIdFrom#40;env,fallbackId#41; :367<br/>CTC chain-correlation id"]
    F7["ctcEmitBodyFromStep#40;step,opts#41; :392<br/>builds /v1/agent-events/emit body"]
    UT --> F1 & F2 & F3 & F4 & F5 & F6 & F7
    TR["trajectory-recorder.cjs<br/>agentbox/config/hooks/trajectory-recorder.cjs:47<br/>ONLY consumer: require#40;./lib/trajectory-util.cjs#41;"]
    F1 & F2 & F3 & F4 & F5 & F6 & F7 --> TR
    OTHER["all other hooks in this file<br/>claude-flow-hook-adapter, nostr-live-mirror, trust-seed,<br/>ruvnet-brain-ground, ontology-monitor, dream-inbox-surface,<br/>project-tracking-publish, fleet-session-start/tab-name"]
    NOTE1["no lib/ import - each hook is self-contained,<br/>duplicating small helpers #40;e.g. readStdin, envFirst#41; independently"]
    OTHER -.-> NOTE1
```





