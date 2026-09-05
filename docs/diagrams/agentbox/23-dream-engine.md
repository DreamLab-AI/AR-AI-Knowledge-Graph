---
id: AB-23
title: Dream machine — nightly cycle, gates and acceptance path
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
adrs: [ADR-2024, ADR-2053]
sources:
  - agentbox/services/dream-engine/src/engine.rs
  - agentbox/services/dream-engine/src/gate.rs
  - agentbox/services/dream-engine/src/verdict.rs
  - agentbox/services/dream-engine/src/runstate.rs
  - agentbox/services/dream-engine/src/readiness.rs
  - agentbox/services/dream-engine/src/manifest.rs
  - agentbox/services/dream-engine/src/receipts.rs
  - agentbox/services/dream-engine/src/candidate.rs
  - agentbox/services/dream-engine/src/persist.rs
  - agentbox/services/dream-engine/src/roster.rs
  - agentbox/dream.config.json
  - agentbox/agentbox.toml
  - agentbox/flake.nix
  - agentbox/config/hooks/dream-inbox-surface.cjs
  - agentbox/management-api/routes/dream.js
  - agentbox/management-api/lib/dream-ledger.js
  - agentbox/skills/dream-machine/commands/dream.md
verified_commit: b00c28a0d
---

## AB-23.1 One repo-night — run phases

```mermaid
stateDiagram-v2
    [*] --> Admission
    Admission --> Handoff : readiness refuses
    note right of Admission
        readiness::assess (readiness.rs:162) runs BEFORE scheduling —
        no clone, no build, no model call. Unusable variants (readiness.rs:25-45):
        NoEvaluators, NoEvaluatorForDeep, NoRequiredEvaluatorForDeep,
        EmptyCommand, MissingScript, NonProbativeCommand, DarwinSandboxMissing.
    end note
    Admission --> Initialised : admitted
    Initialised --> ManifestFrozen
    note right of ManifestFrozen
        manifest::freeze (manifest.rs:196) writes ATOMICALLY and BEFORE any
        model call — baseline revision AND tree hash, evaluator identities with
        sha256(command), the dream.config.json digest, the intended model
        identity, and a deterministic run_id (manifest.rs:166).
    end note
    ManifestFrozen --> BaselineEvaluated
    BaselineEvaluated --> ModelCalled
    ModelCalled --> CandidateEvaluated
    note right of CandidateEvaluated
        candidate::evaluate (candidate.rs:77) applies the emitted dream-patch on
        an ISOLATED git worktree at HEAD and re-runs the required evaluators
        against THAT tree. The operator's working tree is never touched.
    end note
    CandidateEvaluated --> Gated
    Gated --> Persisted : accepted
    Gated --> Complete : REJECT or INCONCLUSIVE or BLOCKED-ENV
    Persisted --> Complete
    Complete --> [*]
    Initialised --> Abandoned : attempt budget exhausted
    note right of Abandoned
        Phase::Abandoned (runstate.rs:31-42) is TERMINAL and never silently
        retried — the operator is alerted. Resume::AlreadyComplete means the
        caller must NOT re-run (runstate.rs:83-95).
    end note
    Abandoned --> [*]
    Handoff --> [*]
```

## AB-23.2 Nightly cycle — every gate as a branch

```mermaid
sequenceDiagram
    autonumber
    participant SUP as supervisord<br/>agentbox/flake.nix:2218
    participant ENG as Engine<br/>agentbox/services/dream-engine/src/engine.rs:59
    participant ROS as roster<br/>agentbox/services/dream-engine/src/roster.rs
    participant RS as runstate::begin<br/>agentbox/services/dream-engine/src/runstate.rs:132
    participant MAN as manifest::freeze<br/>agentbox/services/dream-engine/src/manifest.rs:196
    participant HP as HP annexe<br/>john@10.10.10.1
    participant LLM as llm provider<br/>agentbox/services/dream-engine/src/llm.rs
    participant GATE as gate::decide<br/>agentbox/services/dream-engine/src/gate.rs:187
    participant LED as ledger<br/>agentbox/services/dream-engine/src/ledger.rs

    SUP->>ENG: dream-engine --loop --agentbox-toml /etc/agentbox.toml
    Note over SUP,ENG: autostart=true autorestart=true priority=230 user=devuser (flake.nix:2218-2228)
    alt [dream_machine] enabled = false (agentbox/agentbox.toml:1610)
        ENG-->>SUP: byte-identical-when-off — no supervisor block is generated at all
    else enabled
        loop nightly window
            ENG->>ENG: UTC hour within window_start 1 .. window_end 5 (agentbox.toml:1634-1635)
            ENG->>ROS: least-recently-dreamed ordering, durable file
            Note over ROS: replaces alphabetical-sort-plus-truncate so max_repos_per_night 5 rotates the whole<br/>roster and survives a restart (agentbox.toml:1641)
            alt dry streak — last prune_dry_streak 5 ledger rows ALL INCONCLUSIVE (agentbox.toml:1645)
                ENG->>ENG: skip repo in nightly mode
                Note over ENG: REJECT counts as learning and RESETS the streak — revive via --target or a harness fix
            end
            ENG->>ENG: readiness::assess(cfg, repo, deep, repo_root) — readiness.rs:162, see AB-23.3
            alt not admitted
                ENG->>ENG: ReadinessReport refusal — HANDOFF disposition
                ENG->>LED: record HANDOFF — no clone, no build, no model call
            else admitted
                ENG->>RS: begin(dir, ...)
                alt Resume::AlreadyComplete
                    RS-->>ENG: do NOT re-run
                else Resume::Abandoned
                    RS-->>ENG: attempts exhausted, alert operator
                else Fresh or Resumed
                    ENG->>MAN: freeze(dir, manifest)
                    Note over MAN: INVARIANT: frozen BEFORE any model call. A restart recomputes the same run_id and<br/>resumes the same document — a MOVED baseline ARCHIVES the superseded manifest rather<br/>than overwriting (manifest.rs:176-220)
                    ENG->>HP: clone annexe via git archive HEAD
                    ENG->>HP: run required evaluators on the BASELINE tree
                    ENG->>LLM: call the model with the report prompt
                    Note over ENG,LLM: RESOLVED ADR-2053: the dream engine's DEFAULT reasoning provider is Z.AI<br/>(llm_provider = "zai", zai_model = "glm-5.3"), a deliberate operator choice for<br/>reasoning-token headroom. GOVERNANCE-capabilities now says so, and names the<br/>egress posture — nightly repo content leaves the LAN unless llm_provider = loom.<br/>The Loom is the opt-in LAN-only path.
                    LLM-->>ENG: report text with a VERDICT line and an optional dream-patch
                    ENG->>ENG: candidate::prepare then evaluate on an ISOLATED git worktree at HEAD<br/>(candidate.rs:41,:77) — see AB-23.5
                    ENG->>GATE: decide(manifest, strict, candidate, candidate_receipts)
                    GATE-->>ENG: GateDecision {accepted, verdict, model_verdict, vetoes, required_outcomes, summary}
                    ENG->>LED: append the row
                end
            end
        end
    end
    Note over ENG: DIVERGENCE: the nightly cycle is an acknowledged BACKGROUND-JOB bypass — legacy ADR-057<br/>replayable execution journal and ADR-059 monotonic action-policy pipeline are both<br/>status proposed with NO code, so no uniform interceptor sees this path<br/>(GOVERNANCE-capabilities divergences 1 and 6)
```

## AB-23.3 Evaluator-readiness admission — refusal before scheduling

```mermaid
sequenceDiagram
    autonumber
    participant ENG as Engine<br/>agentbox/services/dream-engine/src/engine.rs:59
    participant RDY as readiness::assess<br/>agentbox/services/dream-engine/src/readiness.rs:162
    participant CFG as dream.config.json evaluatorEntrypoints<br/>agentbox/dream.config.json:50-54
    participant TREE as checked-out annexe tree

    ENG->>RDY: assess(cfg, repo, deep, repo_root)
    RDY->>CFG: read evaluatorEntrypoints
    Note over CFG: values are bare command strings or {cmd, required, deeps, timeoutSecs}. A BARE STRING<br/>reads FAIL-CLOSED as required=true, all deeps, 1800s
    alt no evaluators declared
        RDY-->>ENG: Unusable::NoEvaluators
    else none covers tonight's deep
        RDY-->>ENG: Unusable::NoEvaluatorForDeep
    else all covering evaluators are advisory
        RDY-->>ENG: Unusable::NoRequiredEvaluatorForDeep
        Note over RDY: nothing could ever veto, so acceptance would be UNFALSIFIABLE (readiness.rs:30-32)
    else empty command
        RDY-->>ENG: Unusable::EmptyCommand
    else script absent from the tree
        RDY->>TREE: resolve the script path
        TREE-->>RDY: not present
        RDY-->>ENG: Unusable::MissingScript
        Note over RDY: the annexe clone is git archive HEAD, so an untracked script cannot run there<br/>(readiness.rs:36-38)
    else non-probative command
        RDY-->>ENG: Unusable::NonProbativeCommand
        Note over RDY: an echo, a true, a bare colon — green every night, informative never<br/>(readiness.rs:39-41)
    else darwin entrypoint without a sandbox flag
        RDY-->>ENG: Unusable::DarwinSandboxMissing
        Note over RDY: INVARIANT ADR-2024 — every @metaharness/darwin entrypoint MUST run --sandbox mock or<br/>--sandbox agent, never the no-op real default which is documented surface-INDEPENDENT<br/>and emits the same output regardless of the code under test (agentbox.toml:1628-1633)
    else usable
        RDY-->>ENG: admitted
    end
    Note over ENG,RDY: on any refusal the disposition is HANDOFF with NO clone, NO build and NO model call
    Note over RDY: config load-time validation ALSO rejects a darwin entrypoint without a sandbox flag —<br/>re-checked here so the admission report is complete on its own terms<br/>(readiness.rs:42-44)
```

## AB-23.4 The deterministic required-check gate

```mermaid
sequenceDiagram
    autonumber
    participant ENG as Engine<br/>agentbox/services/dream-engine/src/engine.rs:59
    participant VP as verdict::parse_verdict_strict<br/>agentbox/services/dream-engine/src/verdict.rs:243
    participant CR as receipts::complete_receipts<br/>agentbox/services/dream-engine/src/gate.rs:132
    participant EV as environment_vetoes<br/>agentbox/services/dream-engine/src/gate.rs:157
    participant G as gate::decide<br/>agentbox/services/dream-engine/src/gate.rs:187

    ENG->>VP: parse_verdict_strict(report)
    Note over VP: acceptance consults ONLY a bare unambiguous "VERDICT: TOKEN" line — missing, noisy,<br/>conflicting or unknown declarations are TYPED errors that can never reach ACCEPT<br/>(VerdictParseError, verdict.rs:204)
    alt parse error
        VP-->>G: Err(VerdictParseError)
        G->>G: push Veto{class: Unproven, subject: "verdict"}
    else Ok(Verdict)
        VP-->>G: Accept | Reject | Inconclusive | BlockedEnv | Handoff (verdict.rs:16-31)
    end
    G->>CR: complete_receipts(required, candidate_receipts, Phase::Candidate)
    CR-->>G: one receipt per REQUIRED evaluator, Missing where absent
    G->>EV: environment_vetoes(...)
    loop each required evaluator outcome
        alt Passed
            G->>G: no veto (gate.rs:96-99)
        else Missing
            G->>G: Veto{class: Harness}
        else Blocked or TimedOut or Silent
            G->>G: Veto{class: Harness}
            Note over G: is_harness_fault (receipts.rs:88-98) — Blocked, TimedOut, Silent, Missing. Silent is<br/>exit 0 with NO output on either stream: surface-independent, therefore unfalsifiable,<br/>the ADR-065 no-op in its most literal form (receipts.rs:59-61)
        else Failed or ExplicitFail
            G->>G: Veto{class: Evidence}
            Note over G: ExplicitFail is exit 0 whose output declares failure in so many words — FAIL:, FAILED,<br/>"test result: FAILED" (receipts.rs:52-55)
        end
    end
    alt any veto
        G-->>ENG: accepted=false
        Note over G: harness-class vetoes yield BLOCKED-ENV, evidence-class REJECT, unproven INCONCLUSIVE —<br/>regardless of the model's report text
    else claimed ACCEPT AND candidate applied AND every required evaluator Passed
        G-->>ENG: accepted=true verdict=ACCEPT
    end
    Note over G: INVARIANT: evaluator failure VETOES acceptance. Before this landed, failure text could<br/>coexist with ACCEPT (ADR-2024 closeout 2026-09-04)
    Note over VP: BLOCKED-ENV never counts toward a repo's dry streak — a broken harness must not park a<br/>healthy repo — and is raised by the engine's pre-flight probe, not parsed from an LLM<br/>report (verdict.rs:21-26)
```

## AB-23.5 Candidate re-evaluation on an isolated worktree

```mermaid
sequenceDiagram
    autonumber
    participant ENG as Engine<br/>agentbox/services/dream-engine/src/engine.rs:59
    participant PP as persist::extract_patch<br/>agentbox/services/dream-engine/src/persist.rs:38
    participant PREP as candidate::prepare<br/>agentbox/services/dream-engine/src/candidate.rs:41
    participant WT as isolated git worktree at HEAD
    participant EVAL as candidate::evaluate<br/>agentbox/services/dream-engine/src/candidate.rs:77
    participant REC as receipts::persist<br/>agentbox/services/dream-engine/src/receipts.rs:263
    participant MW as manifest::write_candidate<br/>agentbox/services/dream-engine/src/manifest.rs:329
    participant CL as candidate::cleanup<br/>agentbox/services/dream-engine/src/candidate.rs:61

    ENG->>PP: extract_patch(report)
    alt no dream-patch in the report
        PP-->>ENG: None
        Note over ENG: CandidateState records the absence — a claim of ACCEPT with no applied candidate cannot<br/>be accepted (gate.rs:171)
    else patch present
        PP-->>ENG: Some(patch)
        ENG->>PREP: prepare(...)
        PREP->>WT: create worktree at HEAD and apply the patch
        Note over PREP,WT: INVARIANT: the operator's working tree is NEVER touched
        PREP-->>ENG: PreparedCandidate with the candidate tree hash
        ENG->>EVAL: evaluate(...) — re-run the REQUIRED evaluators against THAT tree
        loop each required evaluator
            EVAL->>EVAL: run with its timeoutSecs budget
            EVAL->>EVAL: receipts::classify(exec, timeout_secs) (receipts.rs:206)
        end
        EVAL-->>ENG: Vec<EvaluatorReceipt>
        ENG->>REC: persist(dir, Phase::Candidate, receipts)
        Note over REC: raw receipts — exit code, BOTH streams verbatim, duration and a typed outcome per<br/>evaluator, per phase under the night directory
        ENG->>MW: write_candidate(dir, CandidateRecord)
        alt accepted downstream
            ENG->>CL: cleanup(repo, prepared)
        else rejected
            ENG->>CL: discard(repo, prepared) (candidate.rs:67)
        end
    end
```

## AB-23.6 Persistence and the human merge gate

```mermaid
sequenceDiagram
    autonumber
    participant G as gate::decide<br/>agentbox/services/dream-engine/src/gate.rs:187
    participant PA as persist::persist_accept<br/>agentbox/services/dream-engine/src/persist.rs:134
    participant BN as persist::branch_name<br/>agentbox/services/dream-engine/src/persist.rs:54
    participant BW as persist::build_branch_worktree<br/>agentbox/services/dream-engine/src/persist.rs:80
    participant PR as persist::push_and_open_pr<br/>agentbox/services/dream-engine/src/persist.rs:158
    participant H as Human reviewer
    participant CFG as dream.config.json<br/>agentbox/dream.config.json

    alt not accepted
        G-->>PA: skipped — verdict recorded in the ledger only
    else accepted
        G->>PA: persist_accept(...)
        PA->>BN: branch_name(deep, date)
        Note over BN: branchPrefix "dream/" (agentbox/dream.config.json:69)
        PA->>BW: build_branch_worktree(...)
        PA->>PR: push_and_open_pr(...)
        PR-->>H: PrOutcome — a PULL REQUEST, never a merge
        Note over CFG: autoMerge = false (agentbox/dream.config.json:74). labels dream-cycle, agentbox-self<br/>(:70-73)
        alt the change touches services/dream-engine — a SELF-REFERENTIAL hypothesis
            Note over PA,H: INVARIANT: agentbox contains the dream-engine that dreams it. A change to that crate<br/>changes the dreamer itself — apply EXTRA review scrutiny and NEVER let a self-modifying<br/>hypothesis bypass the human-merge gate. Still evidence-gated, still witnessed<br/>(dream.config.json:57 extraDisciplines self-referential)
        end
        H->>H: review and merge, or not
    end
    Note over CFG: extraDisciplines also fence: nix-build-out-of-annexe (the full flake.nix image build<br/>needs the host build shell, so record image/Nix findings as HANDOFF (rebuild) notes and<br/>never invent a nix-build evaluator that would silently no-op, :58) —<br/>sibling-path-deps-fenced (services/nostr-pod-bridge path-deps siblings absent from the<br/>annexe clone, so that crate cannot build there, :59, legacy ADR-060 is config-level<br/>only) — secrets-never-in-report (never quote .env or any *_KEY/*_PRIVKEY value into a<br/>report, ledger or gist, :60)
    Note over H: DIVERGENCE: the human-merge boundary is a PROCESS, not a code control — ADR-2024<br/>implementation_status stays partial for that reason
```

## AB-23.7 Run journal — restart, resume and abandonment

```mermaid
sequenceDiagram
    autonumber
    participant ENG as Engine<br/>agentbox/services/dream-engine/src/engine.rs:59
    participant RS as runstate<br/>agentbox/services/dream-engine/src/runstate.rs
    participant F as run-state.json<br/>night directory
    participant MAN as manifest<br/>agentbox/services/dream-engine/src/manifest.rs

    ENG->>RS: begin(dir, ...) (runstate.rs:132)
    RS->>F: load(dir) (runstate.rs:123)
    alt no prior record
        F-->>RS: none
        RS-->>ENG: Resume::Fresh(RunState)
    else prior attempt died part-way
        F-->>RS: RunState with phase < Complete and attempts < max_attempts
        RS-->>ENG: Resume::Resumed — continue from resumed_from
        ENG->>MAN: run_id(repo, date, deep, baseline_revision, config_digest) (manifest.rs:166)
        Note over MAN: a restart recomputes the SAME id from the same inputs, so it resumes against the same<br/>frozen document
    else already finished
        RS-->>ENG: Resume::AlreadyComplete — caller must NOT re-run
    else attempts exhausted
        RS-->>ENG: Resume::Abandoned — record the abandonment and move on rather than looping
    end
    Note over RS: should_run() is true only for Fresh or Resumed (runstate.rs:103-105)
    loop each phase transition
        ENG->>RS: advance(dir, state, phase) (runstate.rs:188)
        RS->>F: durable write
    end
    alt success
        ENG->>RS: complete(dir, state, verdict) (runstate.rs:197)
    else failure
        ENG->>RS: fail(dir, state, error) (runstate.rs:205)
    end
    Note over MAN: manifest::freeze returns Freeze (manifest.rs:176) — a diverged baseline ARCHIVES the<br/>superseded manifest instead of overwriting it. A defect caught in test: the digest<br/>originally included its own timestamp, which would have made every restart read as a<br/>diverged experiment
```

## AB-23.8 Core types

```mermaid
classDiagram
    class RunState {
        +u32 schema
        +String run_id
        +String night_id
        +String repo
        +String date
        +Phase phase
        +u32 attempts
        +u32 max_attempts
        +Option~Phase~ resumed_from
        +String first_seen
        +String updated_at
        +Option~String~ last_error
        +Option~String~ verdict
    }
    class Phase {
        <<enum>>
        Initialised
        ManifestFrozen
        BaselineEvaluated
        ModelCalled
        CandidateEvaluated
        Gated
        Persisted
        Complete
        Abandoned
    }
    class Resume {
        <<enum>>
        Fresh
        Resumed
        AlreadyComplete
        Abandoned
        +state() RunState
        +should_run() bool
    }
    class Verdict {
        <<enum>>
        Accept
        Reject
        Inconclusive
        BlockedEnv
        Handoff
        +as_str() str
        +is_significant() bool
    }
    class GateDecision {
        +bool accepted
        +String verdict
        +String model_verdict
        +Vec~Veto~ vetoes
        +Vec~Tuple~ required_outcomes
        +String summary
        +verdict_enum() Verdict
    }
    class Veto {
        +VetoClass class
        +String subject
        +String reason
    }
    class VetoClass {
        <<enum>>
        Harness
        Evidence
        Unproven
    }
    class EvaluatorOutcome {
        <<enum>>
        Passed
        Failed
        ExplicitFail
        Blocked
        TimedOut
        Silent
        Missing
        +label() str
        +is_pass() bool
        +is_harness_fault() bool
    }
    class EvaluatorReceipt {
        +String name
        +String command
    }
    class ExperimentManifest {
        +String run_id
        +String baseline_revision
        +String baseline_tree
        +Vec~EvaluatorIdentity~ evaluators
        +String config_digest
        +ModelIdentity model
    }
    class EvaluatorIdentity {
        +String name
        +String command_sha256
        +bool required
        +u64 timeout_secs
    }
    class Unusable {
        <<enum>>
        NoEvaluators
        NoEvaluatorForDeep
        NoRequiredEvaluatorForDeep
        EmptyCommand
        MissingScript
        NonProbativeCommand
        DarwinSandboxMissing
        +describe() String
    }
    RunState --> Phase
    Resume --> RunState
    GateDecision --> Veto
    Veto --> VetoClass
    GateDecision ..> Verdict : verdict_enum
    EvaluatorReceipt --> EvaluatorOutcome
    ExperimentManifest --> EvaluatorIdentity
    note for VetoClass "Harness = the evidence could not be gathered, an operational fault. Evidence = the<br/>evidence was gathered and it is against the candidate. Unproven = there was nothing to<br/>test or nothing readable to act on (gate.rs:36-44)"
```

## AB-23.9 Dream-inbox surfacing hook

```mermaid
sequenceDiagram
    autonumber
    participant U as Operator turn<br/>any Claude session
    participant CC as Claude Code UserPromptSubmit
    participant HK as dream-inbox-surface.cjs<br/>agentbox/config/hooks/dream-inbox-surface.cjs:1
    participant INBOX as dream-inbox.json<br/>/home/devuser/workspace/.agentbox/dream-inbox.json
    participant EP as entrypoint registration<br/>agentbox/config/entrypoint-unified.sh:1278

    Note over EP: the entrypoint prefers /opt/agentbox/config/hooks/dream-inbox-surface.cjs and falls back<br/>to the repo path, then dedupes on the command substring (:1279,:1291)
    U->>CC: submits a prompt
    CC->>HK: UserPromptSubmit with stdin JSON
    HK->>INBOX: readFileSync
    alt file missing or unparseable or not an array
        HK-->>CC: exit — fail-open, no injection
    else
        HK->>HK: filter status == open AND now - last_surfaced > RESURFACE_HOURS 4 * 3600
        HK->>HK: slice(0, MAX_PER_TURN 2)
        alt nothing due
            HK-->>CC: exit — no injection
        else items due
            HK->>INBOX: stamp last_surfaced = now and write back
            HK-->>CC: inject the open items as additional context with instructions to relay them and record<br/>answers via /dream answer
        end
    end
    Note over HK: the nightly engine has NO session with the operator — this hook is the bridge that makes<br/>the self-improvement loop part of working praxis instead of a log nobody reads (:4-10)
    Note over HK: rate limiting is PER ITEM via last_surfaced, and fail-open on any error (:12-15)
```

## AB-23.10 Control and reporting surfaces

```mermaid
flowchart TB
    subgraph skill["/dream control skill — agentbox/skills/dream-machine/commands/dream.md"]
        S1["/dream status (or no argument) — :7"]
        S2["/dream questions · /dream answer id text · /dream dismiss id — :17"]
        S3["/dream harvest [--days N] — :29"]
        S4["/dream off · /dream on — :39"]
        S5["/dream run [repo] — :44"]
        S6["/dream standby repo · /dream revive repo — :57"]
        S7["/dream digest [date] — :62"]
        S8["/dream nominate repo — :70"]
    end
    subgraph scripts["Scripts — agentbox/scripts/"]
        N1["dream-machine-nightly.mjs"]
        N2["dream-inbox.mjs"]
        N3["dream-harvest.mjs"]
        N4["dream-forum-suggestions.mjs"]
        N5["dream-night-digest.mjs"]
        N6["dream-hooks-syntax.sh — an evaluatorEntrypoint, not a control surface"]
    end
    subgraph api["management-api"]
        R1["GET /dream/status (fastify)<br/>agentbox/management-api/routes/dream.js:24"]
        L1["dream-ledger.js parseLedger :52 · verdictStats :80 · latestNights :91<br/>discoverNominatedRepos :117 · pendingMerges :202<br/>readRepoDreamStatus :215 · aggregateDreamStatus :264"]
    end
    subgraph out["Outputs"]
        O1["docs/dream-cycle/LEDGER.md<br/>ledgerPath, agentbox/dream.config.json:68"]
        O2["docs/dream-cycle/FORUM-SUGGESTIONS.md"]
        O3["dream-inbox.json — see AB-23.9"]
        O4["voice/console/site/dream.html"]
    end
    S1 --> R1
    R1 --> L1
    L1 --> O1
    S3 --> N3
    S2 --> N2
    N2 --> O3
    S7 --> N5
    S5 --> N1
    N4 --> O2
    L1 --> O4
    subgraph notes["Invariants and drift"]
        direction TB
        ND1["DIVERGENCE: routes/dream.js exposes exactly ONE endpoint, GET /dream/status. There is no<br/>HTTP control surface for run/nominate/answer — those are skill-plus-script paths only"]
        ND2["DIVERGENCE: legacy ADR-055 dream cockpit is PARTIAL — dream.html exists, the full<br/>cockpit is unverified. The 056/058/061/062-072 governance band is PAPER: the engine runs<br/>ahead of its decision-surface, self-GC and telemetry-contract designs<br/>(GOVERNANCE-capabilities divergence 6)"]
        ND3["Both repo roots carry a dream.config.json — agentbox/dream.config.json (the agentbox<br/>nomination) and the VisionClaw root dream.config.json. Each nominated repo declares its<br/>own evaluatorEntrypoints and extraDisciplines"]
        ND1 ~~~ ND2 ~~~ ND3
    end
```
