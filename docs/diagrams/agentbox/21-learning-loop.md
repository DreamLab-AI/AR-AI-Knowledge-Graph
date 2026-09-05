---
id: AB-21
title: Learning loop — capture, judge, distil, consume
area: agentbox
governing:
  - agentbox/docs/LEARNING-memory.md
adrs: [ADR-2015, ADR-2016, ADR-2017, ADR-2018, ADR-2051, ADR-2052]
sources:
  - agentbox/config/hooks/trajectory-recorder.cjs
  - agentbox/config/hooks/lib/trajectory-util.cjs
  - agentbox/management-api/lib/failure-taxonomy.js
  - agentbox/scripts/ruvector-aggregate-sweep.mjs
  - agentbox/scripts/ruvector-pattern-distill.mjs
  - agentbox/mcp/servers/lib/aggregate-effectiveness.js
  - agentbox/mcp/servers/lib/memory-hybrid.js
  - agentbox/mcp/servers/lib/memory-tools.js
  - agentbox/mcp/servers/lib/ruvector-gates.js
  - agentbox/scripts/ruvector-recall-harness.mjs
  - agentbox/agentbox.toml
  - agentbox/flake.nix
verified_commit: b00c28a0d
---

## AB-21.1 Capture — transcript-driven grading

```mermaid
sequenceDiagram
    autonumber
    participant CC as Claude Code<br/>Stop / SubagentStop
    participant HK as trajectory-recorder.cjs<br/>agentbox/config/hooks/trajectory-recorder.cjs:1
    participant G as gates
    participant STASH as per-session stash<br/>line watermark
    participant TR as transcript JSONL
    participant U as trajectory-util.cjs<br/>agentbox/config/hooks/lib/trajectory-util.cjs
    participant FT as failure-taxonomy.js<br/>agentbox/management-api/lib/failure-taxonomy.js

    CC->>HK: hook fires
    HK->>G: read RUVECTOR_MEMORY_LEARNING_ENABLED and RUVECTOR_RECORD_TRAJECTORIES
    alt either off — the DEFAULT-OFF posture
        G-->>HK: off
        HK-->>CC: silent exit 0 — byte-identical to pre-learning behaviour (:28-30)
    else BOTH on ('1' or 'true')
        HK->>STASH: read the per-session line watermark
        HK->>TR: scan from the watermark — INCREMENTAL
        Note over HK,TR: WHY TRANSCRIPT-DRIVEN, not per-PostToolUse: on this Claude Code build a successful Bash<br/>tool_response carries NO exit code or error flag, and PostToolUse does NOT fire at all<br/>for non-zero-exit commands — so per-tool grading is blind to EVERY FAILURE. The<br/>transcript is the only source recording both outcomes via tool_result.is_error (:22-27)
        loop each Bash tool_use / tool_result pair
            HK->>U: gradeResult(is_error, stderr, interrupted)
            alt is_error absent (undetermined) or the call was user-interrupted
                U-->>HK: null
                HK->>HK: NOTHING is written for this step
                Note over U: INVARIANT I04 OUTCOME HONESTY — the outcome is a real graded signal or nothing is<br/>written. Never default an undetermined or interrupted call to success (:31-32)
            else graded
                U-->>HK: {success, quality, signal} — quality 1.0 clean success, 0.85 success with stderr noise,<br/>0.0 failure
            end
            HK->>U: redact(use.command)
            alt redaction returns null
                U-->>HK: null
                HK->>HK: log "redaction failed — skipping step (fail-closed, I10)" then continue (:315)
                Note over U: INVARIANT I10 PRIVACY FAIL-CLOSED — an un-redactable command is SKIPPED, never persisted<br/>raw. REDACTORS conservatively over-redact URI creds, *KEY/TOKEN/SECRET assignments,<br/>bearer tokens, 40+-char base64 and 32+-char hex runs
            else redacted
                U-->>HK: redacted string
            end
            HK->>U: commandPattern(command)
            U-->>HK: low-cardinality "<verb>[ <sub>] [args:N flags:N markers]" — NO raw args, NO secrets
            HK->>HK: durationMs from transcript timestamps (:317-319)
            Note over HK: null when either timestamp is unparseable OR t1 < t0. A ZERO duration (both rounding to<br/>the same ms) is a VALID 0 — the step is pushed regardless of durationMs
            opt graded FAILURE with stderr
                HK->>U: redact(stderr) then slice(0, 400) as failureHint (:325-329)
                HK->>FT: MAST classifier, fail-open to 'unmapped'
                Note over HK,FT: the hint is used IN-MEMORY ONLY for tagging and is NEVER persisted to the step result —<br/>I10: the durable result carries the redacted command, not stderr (:321-324)
            end
        end
    end
    Note over HK: INVARIANT FAIL-OPEN — any error exits 0 and never blocks Claude, mirroring<br/>nostr-live-mirror.cjs (ADR-2015)
```

## AB-21.2 Capture — persistence

```mermaid
sequenceDiagram
    autonumber
    participant HK as trajectory-recorder.cjs<br/>agentbox/config/hooks/trajectory-recorder.cjs
    participant PROBE as hasDurationColumn<br/>agentbox/config/hooks/trajectory-recorder.cjs:252
    participant PG as ruvector-postgres
    participant U as trajectory-util.cjs
    participant EV as agent-events

    HK->>PG: INSERT INTO trajectories (id, task, agent, status, started_at, metadata) VALUES<br/>($1,$2,$3,'recording',CURRENT_TIMESTAMP,$4::jsonb) ON CONFLICT (id) DO NOTHING<br/>(:386-391)
    Note over HK,PG: task is "claude-code-session:<first 12 of session>", agent is AGENTBOX_AGENT or<br/>'claude-code'. metadata carries session, owner_did, trajectory_urn, handoff_id<br/>(:379-383)
    HK->>PROBE: hasDurationColumn(client)
    alt cached verdict exists
        PROBE-->>HK: cached boolean (:251)
    else first call
        PROBE->>PG: SELECT 1 FROM information_schema.columns WHERE table_name='trajectory_steps' AND<br/>column_name='duration_ms' LIMIT 1 (:254-256)
        PG-->>PROBE: rowCount
        PROBE->>PROBE: cache the verdict, catch to false (:259)
    end
    loop each graded step
        HK->>HK: stepId = "<trajectoryId>:step-<sha12(toolUseId)>" (:395)
        Note over HK: DETERMINISTIC, content-addressed step ids make repeated Stop firings IDEMPOTENT (:19-20)
        HK->>PG: parameterised INSERT INTO trajectory_steps — duration_ms written ONLY when the column<br/>exists
        Note over HK,PG: result JSON carries outcome success|failure, signal, the REDACTED command, an optional<br/>MAST failure_mode on failures, and the CTC fields token_count / duration_ms
        opt CTC emit
            HK->>U: ctcEmitBodyFromStep — tokenCountOf(usage) sums the WHOLE assistant turn's token burden,<br/>handoffIdFrom resolves the chain-correlation id
            HK->>EV: best-effort emit
            Note over HK,EV: the emit is FAIL-OPEN — DB persistence is already done, so a failed emit loses<br/>telemetry, never data
        end
    end
    HK->>HK: advance the per-session line watermark in the stash
```

## AB-21.3 Trajectory schema

```mermaid
classDiagram
    class trajectories {
        +text id
        +text task
        +text agent
        +text status
        +timestamptz started_at
        +jsonb metadata
    }
    class TrajectoryMetadata {
        +String session
        +String owner_did
        +String trajectory_urn
        +String handoff_id "chain correlation"
    }
    class trajectory_steps {
        +text id
        +text trajectory_id
        +text action
        +jsonb result
        +real quality
        +int step_order
        +int duration_ms
    }
    class StepResult {
        +String outcome
        +String signal
        +String command
        +String failure_mode
        +Number token_count
        +Number duration_ms
    }
    trajectories --> TrajectoryMetadata : metadata
    trajectories "1" --> "*" trajectory_steps : has
    trajectory_steps --> StepResult : result
    note for trajectory_steps "id is the PK, content-addressed as trajId-step-sha12(toolUseId) so re-fires are<br/>idempotent. trajectory_id is the FK. action is the durable low-cardinality<br/>commandPattern — verb, optional subcommand, then args:N flags:N markers — carrying NO<br/>raw args and NO secrets, and it is the grouping key the sweep aggregates on. quality is<br/>1.0 clean success, 0.85 success with stderr noise, 0.0 failure. duration_ms is nullable,<br/>may legitimately be 0, and is written only when the column probe finds it"
    note for trajectories "id is the PK. task is claude-code-session then the first 12 chars of the session id.<br/>agent is AGENTBOX_AGENT or claude-code. status is 'recording' at insert. started_at is<br/>CURRENT_TIMESTAMP"
    note for StepResult "outcome is success or failure. command is the REDACTED command, never raw. failure_mode<br/>is the MAST tag, fail-open to 'unmapped'. token_count and duration_ms are the CTC<br/>fields. stderr is NEVER persisted here — the redacted stderr hint exists in-memory only,<br/>capped at 400 chars, purely so the MAST classifier can tag failure_mode (I10)"
```

## AB-21.4 Judge — the aggregate sweep

```mermaid
sequenceDiagram
    autonumber
    participant SUP as supervisord<br/>agentbox/flake.nix:1792
    participant SW as ruvector-aggregate-sweep.mjs<br/>agentbox/scripts/ruvector-aggregate-sweep.mjs:1
    participant G as gates
    participant MT as governed memStore/memRetrieve<br/>agentbox/mcp/servers/lib/memory-tools.js
    participant PG as trajectory_steps
    participant AE as aggregate-effectiveness.js<br/>agentbox/mcp/servers/lib/aggregate-effectiveness.js
    participant AGG as ns memory-learning-aggregates

    Note over SUP,SW: [program:ruvector-aggregate-sweep] — launched UNCONDITIONALLY and self-gating, so a<br/>default-off manifest is byte-identical to the pre-learning product
    loop every aggregate_sweep_interval_mins 30 (agentbox/agentbox.toml:429)
        SW->>G: aggregate_sweep gate (agentbox/agentbox.toml:428)
        alt off
            G-->>SW: fast exit
        else on
            SW->>MT: memRetrieve('__aggregation_sweep_cursor__', 'memory-learning-aggregates') (:105, :300)
            Note over SW,MT: the cursor is ORDINARY GOVERNED MEMORY tagged 'sweep:cursor' (:106) so consumers<br/>filtering on action:* tags never surface it as an aggregate. Written through memStore,<br/>NEVER raw SQL (I03)
            MT-->>SW: last high-water mark
            SW->>PG: AGG_SQL since the cursor, grouped by action
            Note over SW,PG: the cursor binds on max(created_at) (I21) because trajectory_steps.id is text and<br/>NON-MONOTONIC — extract(epoch FROM max(created_at)) and an ISO hwm_ts are both recorded<br/>(:286-287)
            PG-->>AE: grouped rows
            loop each action pattern
                AE->>AE: weight_i = 0.5 ^ (age_days_i / RUVECTOR_RECENCY_HALF_LIFE_DAYS) (:16)
                Note over AE: recency_half_life_days default 14 (agentbox/agentbox.toml:416). Successes are steps with<br/>quality >= 0.5
                AE->>AE: wilsonLower(wSucc, wTotal, Z) with Z = 1.96 (:47, :71, :194)
                Note over AE: the Wilson score-interval LOWER bound of the recency-weighted success proportion — NOT<br/>the raw rate. A single degenerate label cannot move the aggregate
                alt raw n < aggregate_min_samples 20 (agentbox/agentbox.toml:415)
                    AE->>AE: SKIP
                    Note over AE: INVARIANT I06 / ADR-2016 — the sample floor gates on the RAW OBSERVATION COUNT, not the<br/>recency-weighted effective size
                else survives the floor
                    AE->>MT: memStore through the GOVERNED path (:24, :294)
                    Note over AE,AGG: key effectiveness-sha256-12-<hash(pattern)> (:86), namespace memory-learning-aggregates<br/>(:45), typed metadata {importance: wilson, tags: ['action:<pattern>'], memory_type:<br/>'semantic'} (:28-29)
                    Note over AE: the tags and importance are LOAD-BEARING — feed_retrieval keys on metadata.tags and<br/>feed_routing surfaces importance — so the typed-metadata gate is FORCED ON for this<br/>process (:260-262)
                    MT->>AGG: upsert
                end
            end
            SW->>MT: writeCursor {cursor_after: hwmTs, stepsProcessed, aggregatesWritten, urn} (:307-309)
        end
    end
    Note over AE: embeddings are 384-dim and a dimension mismatch is a hard reject — "dimension mismatch:<br/>got N, expected 384" (:118-119). See AB-20
```

## AB-21.5 Distil — patterns table

```mermaid
sequenceDiagram
    autonumber
    participant SUP as supervisord<br/>agentbox/flake.nix:1821
    participant DS as ruvector-pattern-distill.mjs<br/>agentbox/scripts/ruvector-pattern-distill.mjs:1
    participant G as gates
    participant PG as trajectory_steps
    participant XI as Xinference<br/>XINFERENCE_URL /v1/embeddings
    participant PAT as patterns TABLE

    Note over SUP,DS: [program:ruvector-pattern-distill]
    DS->>G: pattern_distillation gate (agentbox/agentbox.toml:430)
    alt off
        G-->>DS: fast exit
    else on
        DS->>DS: read cursor '__pattern_distill_cursor__' (:105) tagged distill:cursor
        Note over DS: a DISTINCT key from the sweep and SONA cursors, so the three loops never trample each<br/>other
        DS->>PG: judged steps since the cursor
        loop each action pattern
            DS->>DS: build the embed text — deduped, front-loaded arg/flag/pipe descriptors, capped and<br/>per-token length-bounded so a NULL or huge blob never reaches the embedder (:277, :295)
            DS->>XI: POST /v1/embeddings (:202)
            alt embedding fails
                XI--xDS: error
                DS->>DS: SKIP THE ROW (:22)
                Note over DS: INVARIANT: EMBED BEFORE INSERT — never write a NULL-embedding, HNSW-invisible pattern.<br/>I03-faithful even though this table is not memory_entries (:18-22)
            else embedding ok
                XI-->>DS: 384-dim vector
                DS->>PAT: INSERT ... ON CONFLICT (id) DO UPDATE (:28)
                Note over DS,PAT: id = distilled-sha256-12-<hash(action)> (:260) — content-addressed, so a second tick<br/>over an unchanged action is a no-op update, not a duplicate
                Note over PAT: metadata.provenance = 'judge:trajectory' (I18, :9). A PROVENANCE FIREWALL keeps W-E<br/>legacy-mining candidates carrying proxy:legacy-mining out of this feeder's output — they<br/>share the table and are separated by the metadata stamp (:32-33)
            end
        end
        DS->>DS: advance the cursor
    end
    Note over PAT: the promoted-set consumer filters on metadata->>'provenance' = 'judge:trajectory' (:11).<br/>A memory_entries shortcut would FAIL acceptance, which is why this script owns a new<br/>embed-then-insert path (:18-19)
```

## AB-21.6 Consume — the feed_retrieval re-rank

```mermaid
sequenceDiagram
    autonumber
    participant AG as Agent
    participant HY as memory-hybrid<br/>agentbox/mcp/servers/lib/memory-hybrid.js:57
    participant ADM as consumerAdmission<br/>agentbox/mcp/servers/lib/ruvector-gates.js:89
    participant AGG as ns memory-learning-aggregates
    participant RES as result rows

    AG->>HY: memory_hybrid_search(query, namespace, limit)
    HY->>ADM: consumerAdmission('feed_retrieval', {corpusLastUpdated, corpusSize})
    alt consumer gate off
        ADM-->>HY: {admitted: false, reason: 'consumer-gate-off'} (:89)
    else master learning gate off
        ADM-->>HY: {admitted: false, reason: 'master-learning-off'} (:91-95)
        Note over ADM: the master gate is the OUTER BOUNDARY — a consumer gate left on behind an off master<br/>must not act
    else producer currently capturing
        ADM-->>HY: {admitted: true, reason: 'active-capture'} (:96)
    else producer off and no receipt
        ADM-->>HY: {admitted: false, reason: 'producer-off-and-retained-corpus-not-accepted'} (:98-103)
    else receipt present but corpus empty
        ADM-->>HY: {admitted: false, reason: 'retained-corpus-empty'} (:113)
        Note over ADM: emptiness is diagnosed FIRST — an empty corpus is also undateable, and "there is nothing<br/>here" is the more useful answer than "I cannot date it" (:111-112)
    else receipt present but corpus undateable
        ADM-->>HY: {admitted: false, reason: 'retained-corpus-freshness-unknown'} (:115-118)
        Note over ADM: an accepted corpus we CANNOT DATE is not a fresh corpus. Refuse rather than assume — an<br/>unmeasurable corpus is the same risk as a stale one
    else receipt present and corpus older than RUVECTOR_RETAINED_CORPUS_MAX_AGE_DAYS default 30
        ADM-->>HY: {admitted: false, reason: 'retained-corpus-stale'} (:119-121)
    else receipt present and fresh
        ADM-->>HY: {admitted: true, reason: 'retained-corpus-accepted', receipt, corpus_age_days} (:122)
    end
    alt admitted
        HY->>AGG: ONE bounded read, LIMIT 500 (:57-101)
        AGG-->>HY: rows
        HY->>HY: build action:<pattern> to MAX wilson map
        loop each result row
            alt row metadata.tags intersects a high-effectiveness action tag
                HY->>RES: score += 0.1 * wilson — a BOUNDED bonus
            end
        end
    else not admitted
        HY->>RES: base ranking untouched
    end
    alt any error anywhere
        HY->>RES: FAIL-OPEN — base ranking untouched
    end
    HY-->>AG: ranked results
    Note over HY: feed_retrieval = true since 2026-08-31 (agentbox/agentbox.toml:417). feed_routing =<br/>false (:418) — aggregates surface only as advisory [INTELLIGENCE] hints
```

## AB-21.7 ADR-2017 producer-before-consumer

```mermaid
stateDiagram-v2
    [*] --> ConsumerGateOff
    ConsumerGateOff --> ConsumerGateOn : operator flips feed_retrieval or feed_routing
    ConsumerGateOn --> Refused_MasterOff : master learning gate is off
    ConsumerGateOn --> Admitted_ActiveCapture : RUVECTOR_RECORD_TRAJECTORIES on
    ConsumerGateOn --> NeedsReceipt : producer off
    NeedsReceipt --> Refused_NoReceipt : RUVECTOR_RETAINED_CORPUS_ACCEPTED unset
    NeedsReceipt --> ReceiptNamed : operator names a receipt
    ReceiptNamed --> Refused_Empty : corpusSize not greater than 0
    ReceiptNamed --> Refused_Undateable : corpusLastUpdated unparseable
    ReceiptNamed --> Refused_Stale : age greater than max_age_days, default 30
    ReceiptNamed --> Admitted_RetainedCorpus : fresh
    Admitted_ActiveCapture --> [*]
    Admitted_RetainedCorpus --> [*]
    Refused_MasterOff --> [*]
    Refused_NoReceipt --> [*]
    Refused_Empty --> [*]
    Refused_Undateable --> [*]
    Refused_Stale --> [*]
    note right of NeedsReceipt
        THE INVARIANT, chosen and stated (ruvector-gates.js:44-71):
        producer-before-consumer means a QUALIFIED RETAINED CORPUS,
        not merely active capture. "Active capture" alone is too strong —
        stopping the recorder does not erase valid aggregates. "Any
        retained corpus" is too weak — that is the degenerate state the
        review found, where a consumer scores against whatever happens
        to be in the table with nobody having decided it is fit to use.
    end note
    note right of ReceiptNamed
        The provenance/freshness binding: an operator says WHICH corpus
        they accepted, and the runtime checks it is not stale before
        letting it move a score. AN ENVIRONMENT OVERRIDE CANNOT SKIP IT —
        there is no boolean that means "trust me" (ruvector-gates.js:65-68).
    end note
    note right of Admitted_ActiveCapture
        RESOLVED ADR-2051: LEARNING-memory Invariant 2 now states the
        enforced rule — a consumer is admitted only via this single
        runtime consumerAdmission decision (ruvector-gates.js:89-124),
        which the static validator (E066/W066) mirrors. The doc
        previously said the validator only diagnoses.
    end note
```

## AB-21.8 A trajectory's life

```mermaid
stateDiagram-v2
    [*] --> Observed : a Bash call appears in the transcript
    Observed --> SkippedHonesty : gradeResult returns null
    note right of SkippedHonesty
        is_error absent (undetermined) or the call was user-interrupted.
        I04 — never default to success. Nothing is written.
    end note
    Observed --> SkippedPrivacy : redact() returns null
    note right of SkippedPrivacy
        I10 fail-closed — an un-redactable command is skipped,
        never persisted raw.
    end note
    Observed --> Recorded : graded and redacted
    Recorded --> Aggregated : sweep groups it by action pattern
    Aggregated --> BelowFloor : raw n < aggregate_min_samples 20
    note right of BelowFloor
        I06 / ADR-2016 — the floor is the RAW count, not the
        recency-weighted effective size. The pattern waits for
        more observations.
    end note
    Aggregated --> Judged : Wilson lower bound computed, floor cleared
    Judged --> Distilled : embed-then-insert into the patterns table
    Distilled --> EmbedFailed : embedding transport failed
    note right of EmbedFailed
        The row is SKIPPED, not written NULL — no HNSW-invisible
        pattern is ever created.
    end note
    Judged --> Eligible : upserted into ns memory-learning-aggregates
    Eligible --> Consumed : consumerAdmission admits feed_retrieval
    Eligible --> Withheld : admission refused, see AB-21.7
    Consumed --> [*]
    Withheld --> [*]
    BelowFloor --> [*]
    EmbedFailed --> [*]
    SkippedHonesty --> [*]
    SkippedPrivacy --> [*]
```

## AB-21.9 Gate topology

```mermaid
flowchart LR
    subgraph toml["agentbox.toml [memory_learning] — block at :412"]
        direction TB
        M["enabled = true :413<br/>master gate"]
        P["record_trajectories = true :414<br/>PRODUCER"]
        F1["aggregate_min_samples = 20 :415"]
        F2["recency_half_life_days = 14 :416"]
        C1["feed_retrieval = true :417<br/>CONSUMER, enabled 2026-08-31"]
        C2["feed_routing = false :418<br/>CONSUMER, advisory only"]
        S1["aggregate_sweep = true :428"]
        S2["aggregate_sweep_interval_mins = 30 :429"]
        S3["pattern_distillation = true :430"]
    end
    subgraph env["ruvector-gates.js env resolution"]
        direction TB
        E1["RUVECTOR_MEMORY_LEARNING_ENABLED :36"]
        E2["RUVECTOR_RECORD_TRAJECTORIES :37"]
        E3["RUVECTOR_FEED_RETRIEVAL :38"]
        E4["RUVECTOR_FEED_ROUTING :39"]
        E5["RUVECTOR_RETAINED_CORPUS_ACCEPTED"]
        E6["RUVECTOR_RETAINED_CORPUS_MAX_AGE_DAYS<br/>default 30 :72-75"]
    end
    subgraph proc["Self-gating processes"]
        direction TB
        H["trajectory-recorder.cjs<br/>Stop hook"]
        A["ruvector-aggregate-sweep.mjs<br/>agentbox/flake.nix:1792"]
        D["ruvector-pattern-distill.mjs<br/>agentbox/flake.nix:1821"]
        HY["memory-hybrid re-rank"]
    end
    M --> E1
    P --> E2
    C1 --> E3
    C2 --> E4
    E1 --> H
    E2 --> H
    S1 --> A
    S3 --> D
    E3 --> HY
    E5 --> HY
    E6 --> HY
    N1["INVARIANT byte-identical-when-off: master gate off = no hook registered, no aggregation,<br/>no consumers (agentbox.toml:409). Each script is launched unconditionally and exits fast<br/>when its gate is off"]
    proc --- N1
```

## AB-21.10 The recall gate on any consumer flip

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator
    participant H as ruvector-recall-harness.mjs<br/>agentbox/scripts/ruvector-recall-harness.mjs:32
    participant TOML as agentbox/agentbox.toml [memory_learning]
    participant CONS as the consumer

    Note over OP,CONS: INVARIANT I14 / ADR-2018 — no consumer that ALTERS WHAT A QUERY RETURNS may flip its<br/>gate without a passing run. That covers SONA apply, attention re-rank, param tuning, the<br/>feed_retrieval re-rank, an embedding-model cutover and a graph-augmented orient (harness<br/>:4-9)
    OP->>H: ./agentbox.sh ruvector recall
    H-->>OP: median of 3 — see AB-20.9 for the harness internals
    alt median(self) >= 175/200 AND median(true) >= 102/120 AND exact-token delta >= 0
        H-->>OP: PASS
        OP->>TOML: flip the consumer gate
        OP->>CONS: the consumer may now take effect
        Note over OP: evidence lands under backups/ruvector-sidecar/recall-runs/<utc>.json — the merge gate<br/>wants the receipt, not the claim
    else FAIL
        H-->>OP: the gate stays shut
    end
    Note over TOML: attention_rerank stays OFF BY MEASUREMENT, not caution — attention_score = cos/sqrt(384)<br/>on an L2-normalised corpus gives a max diff of 4e-7, so the blend is a mathematical<br/>IDENTITY with zero benefit (agentbox.toml:431)
    Note over TOML: RESERVED default-off keys, each harness-gated before it may flip — sona_learn_enabled<br/>:432, sona_apply_enabled :433, param_tuning_enabled :434 (HNSW ef_search/probes<br/>auto-tuner), embedding_dual_write :400, embedding_active_column :401, graph_backbone<br/>:402. DIVERGENCE D6
    Note over TOML: RESOLVED ADR-2052: governing docs now cite agentbox.toml by [section].key, not by<br/>line number, because every line citation had drifted. The live keys are<br/>[memory_learning] feed_retrieval, feed_routing, aggregate_min_samples,<br/>recency_half_life_days, aggregate_sweep, aggregate_sweep_interval_mins,<br/>pattern_distillation.
    Note over TOML: DIVERGENCE D2: agentbox.toml:417 justifies the feed_retrieval flip with 78 aggregates<br/>>=20 samples (2026-08-31) while<br/>agentbox/docs/reference/claude-context/ruvector-memory-state.md records 12 from the<br/>2026-07-21 sweep. The toml is the running config and the more recent number
    Note over TOML: DIVERGENCE D4: agentbox/README.md still lists feed_retrieval / feed_routing as open<br/>gates (false) — the README lags the toml
    Note over CONS: the sweep and the recall harness are NOT MCP tools — they run OUT-OF-PROCESS, and the<br/>MCP server registers no tool for them so tool registration stays byte-identical<br/>(ruvector-mcp.cjs:12-18). See AB-20
    Note over OP: privacy note — redaction happens BEFORE persistence in the producer (see AB-21.1). The<br/>adapters observability then privacy-filter then JSON-LD middleware chain is AB-04
```
