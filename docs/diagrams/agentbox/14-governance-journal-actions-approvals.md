---
id: AB-14
title: Governance — journal, action pipeline, approvals
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
  - agentbox/docs/SECURITY-profiles.md
adrs: [ADR-2022, ADR-2027]
sources:
  - agentbox/management-api/lib/execution-journal.js
  - agentbox/management-api/lib/execution-projections.js
  - agentbox/management-api/lib/execution-coverage.js
  - agentbox/management-api/lib/agent-action-pipeline.js
  - agentbox/management-api/routes/approvals.js
  - agentbox/management-api/lib/governance-decision-waiter.js
  - agentbox/management-api/lib/receipt-minter.js
  - agentbox/management-api/lib/audit-chain.js
  - agentbox/management-api/lib/failure-taxonomy.js
  - agentbox/management-api/lib/precedent-service.js
  - agentbox/mcp/servers/precedent-bridge.js
  - agentbox/management-api/lib/elevation-publisher.js
  - agentbox/management-api/lib/project-tracker.js
  - agentbox/management-api/lib/project-primer.js
  - agentbox/management-api/routes/projects.js
  - agentbox/management-api/routes/tasks.js
  - agentbox/management-api/server.js
  - agentbox/docs/adr/ADR-2022-governed-ontology-writes.md
  - agentbox/docs/adr/ADR-2027-secret-custody-rotation-break-glass.md
  - agentbox/docs/archive/adr/ADR-057-replayable-agent-execution-journal.md
  - agentbox/docs/archive/adr/ADR-059-monotonic-agent-action-policy-pipeline.md
verified_commit: b00c28a0d
---

## AB-14.1 Governance plane — surfaces that reach the decision point vs surfaces that miss it

```mermaid
flowchart TB
    subgraph SURF["Agent-initiated side-effect surfaces GOVERNANCE-capabilities.md:39-63"]
        DTC["Direct tool call<br/>MCP fleet mcp/mcp.json"]
        CMS["Code-mode sub-call<br/>codeact / code-interpreter agentbox.toml:539,554"]
        ACISHELL["ACI shell<br/>test allowlist agentbox.toml:585<br/>raw Bash still reachable outside it"]
        CONSULT["Consultant / subagent action<br/>tree-search-coder agentbox.toml:621"]
        DREAM["Background job dream-engine<br/>01:00-05:00 UTC unattended"]
        BEADS["Background job beads work-DAG<br/>spawn_child mcp/mcp.json:197"]
        ALTHARNESS["Alternate harness path<br/>non-Claude harness"]
        TASKAPI["POST /v1/tasks<br/>routes/tasks.js:15 spawnTask"]
    end

    PIPE{{"AgentActionPipeline.dispatch<br/>agent-action-pipeline.js:110<br/>9-stage legacy-ADR-059 pipeline"}}
    JOURNAL[["ExecutionJournal.append<br/>execution-journal.js:133"]]
    COSTGATE["costGate middleware<br/>middleware/cost-gate.js, see AB-15.x"]
    ACSPGATE["ACSP authority gate<br/>authority.js buildAuthorityGate.guard, see AB-11.10"]
    AXIOMGUARD["direct_axiom_load=false guard<br/>ADR-2022, see AB-25"]
    EXEC(["side effect executes"])

    DTC -.->|"designed target, never instantiated in production"| PIPE
    CMS -->|"own guard only, no interceptor"| EXEC
    ACISHELL -->|"allowlist only, no interceptor"| EXEC
    CONSULT -->|"spend cap only, see AB-15.x"| EXEC
    DREAM -->|"own evidence/merge gate, see AB-23"| EXEC
    BEADS -->|"own work-DAG guard, see AB-26"| EXEC
    ALTHARNESS -->|"no interceptor at all"| EXEC
    TASKAPI --> COSTGATE --> EXEC
    COSTGATE -.->|"never calls AgentActionPipeline.dispatch"| PIPE
    PIPE --> JOURNAL
    PIPE -.->|"0 production instantiations found, tests only"| EXEC
    ACSPGATE --> EXEC
    AXIOMGUARD --> EXEC

    DRIFT["DOC-DRIFT GOVERNANCE-capabilities.md:72-73 claims a repo-wide search of src/, services/, mcp/ for SessionEvent / execution-journal code returns nothing<br/>execution-journal.js:85 class ExecutionJournal and agent-action-pipeline.js:58 class AgentActionPipeline fully implement legacy-ADR-057/059 D1-D5<br/>under management-api/lib/, a path outside the doc's stated search scope"]
    DIVERGE1["DIVERGENCE TOP OPEN RISK GOVERNANCE-capabilities.md:183-188 no single policy decision point<br/>grep -rn confirms the ONLY non-test requires of agent-action-pipeline.js / execution-journal.js are execution-coverage.js:20,22 reading static constants for a declared-not-live /v1/system block<br/>no route, MCP server or capability call site instantiates AgentActionPipeline or ExecutionJournal"]
    DIVERGE7["DIVERGENCE GOVERNANCE-capabilities.md:206-207 skill lint is advisory, not a runtime capability gate<br/>lint-skills.sh gates estate hygiene only; an enabled skill with clean frontmatter is trusted at runtime with no further check"]
```

## AB-14.2 Action lifecycle — intent through policy, approval, execution, receipt, journal

```mermaid
stateDiagram-v2
    [*] --> Normalise
    Normalise --> Enrich : stage 2, agent-action-pipeline.js:238, must not change operation/target
    Enrich --> Classify : stage 3, assigns side_effect_class privacy_class estimated_cost
    Classify --> FastPath : side_effect_class in read,local FAST_PATH :38
    Classify --> ApprovalGate : side_effect_class in mutate,egress,secret,spend APPROVAL_REQUIRED :34
    FastPath --> Guard : approval-free, still journalled
    ApprovalGate --> Approve : _obtainApproval :244
    Approve --> Guard : receipt valid, identity+cost ceiling frozen :143-144
    Approve --> Denied : approval_missing, approval_expired, approval_replayed, approval_mismatch or cost_exceeds_ceiling :250-273
    Guard --> Execute : every guard abstains :148-159
    Guard --> Denied : any guard denies, errors or returns a non-monotonic verdict :153-158
    Guard --> Denied : mutation_after_approval, identity changed post-approval :162-164
    Execute --> PostProcess : _protectedExecute returns within deadline :281-296
    Execute --> Denied : no_capability_token or timeout :283,289
    PostProcess --> Finalise : redaction ok, or degraded output for public/internal only :173-183
    PostProcess --> Denied : redaction_failed on mutate,egress,secret,spend or sensitive privacy_class :178-180
    Finalise --> Record : stage 8, definition-owned invariants :186
    Record --> Allowed : stage 9, journal.append tool.completed decision=allow :188-190
    Denied --> RecordDeny : _record still called with decision=deny :193
    RecordDeny --> [*]
    Allowed --> [*]

    note right of Guard
        INVARIANT legacy-ADR-059 D3 mutate, egress, secret and spend
        NEVER fail open. NEVER_FAIL_OPEN set agent-action-pipeline.js:36
        matches APPROVAL_REQUIRED exactly.
    end note
    note right of Record
        INVARIANT legacy-ADR-059 D2 after Approve begins, the tuple
        capability+operation+target+canonical_args_hash+cost_ceiling
        is frozen; re-checked at :162 before Execute. No later stage
        can rewrite an earlier approval.
    end note
    note right of RecordDeny
        DOC-DRIFT this state machine is fully implemented and
        contract-tested (tests/contract/agent-action-pipeline.contract.spec.js)
        but is never reached by real traffic — see AB-14.1 DIVERGENCE.
    end note
```

## AB-14.3 AgentActionPipeline.dispatch — intentSpec construction and policy evaluation

```mermaid
sequenceDiagram
    autonumber
    participant CALLER as caller<br/>agent-action-pipeline.js:110
    participant AAP as AgentActionPipeline<br/>agent-action-pipeline.js:58
    participant CLS as classifier callback<br/>opts.classifier :78
    participant APP as approver callback<br/>opts.approver :244
    participant GRD as guards array<br/>opts.guards :76

    CALLER->>AAP: dispatch(raw, opts) agent-action-pipeline.js:110
    AAP->>AAP: _normalise(raw, opts.parentToken) :201
    Note over AAP: session_urn and capability+operation required, else ActionDenied bad_action :203-204
    opt parentToken supplied
        AAP->>AAP: _verifyToken(parentToken) :227, HMAC-SHA256 sig check :375-377
        Note over AAP: D4 child inherits parent.authority into _delegatedAuthority :230
    end
    AAP->>AAP: action_id = hash(coreIdentity + dispatchSeq) :233
    AAP->>AAP: _enrich(action) :238, freezes operation/target
    alt enrich mutated operation or target
        AAP-->>CALLER: ActionDenied enrich_mutated_identity :120
    end
    AAP->>CLS: classifier(action) :124
    CLS-->>AAP: side_effect_class, privacy_class, estimated_cost
    Note over AAP: unknown side_effect_class defaults to mutate, the strictest class :125
    alt _delegatedAuthority set and class exceeds it
        AAP-->>CALLER: ActionDenied authority_exceeded :131-134
    end
    AAP->>AAP: identityHash = _identityHash(action) :138, capability+operation+target+canonical_args_hash
    alt side_effect_class in APPROVAL_REQUIRED and not FAST_PATH
        AAP->>APP: _obtainApproval(action, opts.approval) :244
        APP-->>AAP: approval receipt or opts.approval supplied
        AAP->>AAP: _validateReceipt(receipt, action, identityHash) :250
    end
    loop for each guard in this._guards :148-159
        AAP->>GRD: guard(action)
        GRD-->>AAP: deny or abstain
        alt verdict is deny
            AAP-->>CALLER: ActionDenied guard_denied :155
        else verdict not abstain
            AAP-->>CALLER: ActionDenied bad_verdict, non-monotonic :157
        end
    end
    Note over AAP: D2 re-check — _approvedIdentityHash must still equal _identityHash(action) :162-164
```

## AB-14.4 F2 operator-gated approval — pending, decide, 409 already-decided

```mermaid
sequenceDiagram
    autonumber
    participant AG as authority gate<br/>see AB-11.10
    participant AC as authority consumer<br/>fastify.authorityConsumer, approvals.js:47
    participant OP as operator<br/>NIP-98 authed caller
    participant RT as POST /v1/approvals/:id/decide<br/>approvals.js:92
    participant AUTHZ as authz.isApprover<br/>approvals.js:143

    AG->>AC: publish kind-31402 ActionRequest, awaiting decision
    Note over AC: request now pending — c.getPending(id) can find it
    OP->>RT: POST /v1/approvals/:id/decide {outcome or decision, reasoning}
    alt request.auth.mode is not nip98
        RT-->>OP: 401 nip98_required approvals.js:132-137
    end
    RT->>AUTHZ: isApprover(request.auth.pubkey, manifest) :143
    alt pubkey not on approval allowlist
        RT-->>OP: 403 forbidden_not_approver approvals.js:148-152
    end
    alt c.isDecided(id) true
        RT->>AC: getDecision(id) :168
        RT-->>OP: 409 already_decided {request_event_id, outcome, response_event_id} :169-175
    end
    alt c.getPending(id) is falsy
        RT-->>OP: 404 unknown_request approvals.js:177-182
    end
    RT->>RT: normalise decision=deny to outcome=reject :186-187
    alt outcome not in approve,reject,defer
        RT-->>OP: 400 invalid_outcome approvals.js:188-193
    end
    RT->>AC: signAndPublishDecision({requestId, outcome, reasoning}) :198
    AC-->>RT: signed kind-31403 event, race-safe errors ALREADY_DECIDED/NOT_PENDING/DECISION_IN_FLIGHT
    alt err.code is ALREADY_DECIDED
        RT-->>OP: 409 already_decided approvals.js:203-204
    else err.code is NOT_PENDING
        RT-->>OP: 404 unknown_request approvals.js:206-207
    else err.code is DECISION_IN_FLIGHT
        RT-->>OP: 409 decision_in_flight approvals.js:209-212, concurrent decider already claimed it
    else sign/publish throws for another reason
        RT-->>OP: 502 sign_publish_failed approvals.js:214-215
    end
    RT-->>OP: 200 {success, request_event_id, response_event_id, outcome, decided_by} :223-229
    Note over AG,AC: governance-decision-waiter.js notify() then resolves any gate awaiting this request, see AB-14.6
```

## AB-14.5 routes/approvals.js — every route and status code

```mermaid
sequenceDiagram
    autonumber
    participant C as client
    participant FA as fastify<br/>approvals.js:33 approvalsRoutes
    participant CON as consumer()<br/>approvals.js:46-48

    C->>FA: GET /v1/approvals approvals.js:51
    FA->>CON: options.authorityConsumer or fastify.authorityConsumer
    alt no consumer or listPending not a function
        FA-->>C: 200 {approvals:[], count:0, wired:false, note} :85
    else consumer wired
        FA-->>C: 200 {approvals, count, wired:true} :87-88
    end

    C->>FA: POST /v1/approvals/:id/decide approvals.js:92
    Note over FA: body accepts outcome approve/reject/defer OR decision approve/deny/reject/defer :101-107
    alt not NIP-98 authed
        FA-->>C: 401 nip98_required :132-137
    else pubkey not on approval allowlist
        FA-->>C: 403 forbidden_not_approver :148-152
    else no consumer or signAndPublishDecision not a function
        FA-->>C: 503 authority_consumer_unwired :155-160
    else c.isDecided(id)
        FA-->>C: 409 already_decided :167-176
    else not c.getPending(id)
        FA-->>C: 404 unknown_request :177-182
    else outcome missing or invalid
        FA-->>C: 400 invalid_outcome :188-193
    else signAndPublishDecision throws ALREADY_DECIDED
        FA-->>C: 409 already_decided :203-204
    else signAndPublishDecision throws NOT_PENDING
        FA-->>C: 404 unknown_request :206-207
    else signAndPublishDecision throws DECISION_IN_FLIGHT
        FA-->>C: 409 decision_in_flight :209-212
    else signAndPublishDecision throws other
        FA-->>C: 502 sign_publish_failed :214-215
    else signed ok
        FA-->>C: 200 {success:true, request_event_id, response_event_id, outcome, decided_by} :223-229
    end
    Note over FA: legacy-ADR-043 D4.7 hard rule — the route never writes an unsigned approval, approvals.js:16-22
```

## AB-14.6 governance-decision-waiter.js — awaiting an external decision, fail-closed timeout

```mermaid
sequenceDiagram
    autonumber
    participant GATE as authority gate<br/>lib/authority.js awaitDecision dep, see AB-11.10
    participant GDW as GovernanceDecisionWaiter singleton<br/>governance-decision-waiter.js:155
    participant RC as relay consumer<br/>mcp/nostr-bridge/relay-consumer.js governance branch

    GATE->>GDW: awaitDecision(signedRequest, {timeoutMs}) governance-decision-waiter.js:98
    GDW->>GDW: _keysForRequest(signedRequest) :54, keys e:id, case:case_id, d:tag
    alt keys.length is 0
        GDW-->>GATE: resolve(null) immediately :104, un-addressable wait never hangs
    else keys present
        GDW->>GDW: register entry in this._pending per key :115-118
        GDW->>GDW: setTimeout DEFAULT_TIMEOUT_MS=120000 :31, not unref'd :110
        Note over GDW: fail-closed — a response that never arrives resolves null, the gate treats null as DENY :24
        par relay delivers a matching kind-31403
            RC->>GDW: notify(responseEvent) :130
            GDW->>GDW: _keysForResponse(responseEvent) :67, match by e tag, case_id or d tag
            GDW->>GDW: clearTimeout(entry.timer), _remove(entry) :139-140
            GDW-->>GATE: resolve(responseEvent) :141
        and timeout fires first
            GDW->>GDW: _remove(entry) :113
            GDW-->>GATE: resolve(null) :114
        end
    end
    Note over GATE,GDW: INVARIANT there is NO second relay subscription — the ALREADY-CONNECTED relay-consumer calls notify(), this module is only the wait registry :10-16
```

## AB-14.7 ExecutionJournal.append — what is written, where, and the ordering guarantee

```mermaid
sequenceDiagram
    autonumber
    participant CALLER as caller<br/>e.g. AgentActionPipeline._record :299
    participant EJ as ExecutionJournal<br/>execution-journal.js:85
    participant EVT as events adapter<br/>opts.eventsAdapter.dispatch, ADR-005 events slot

    CALLER->>EJ: append(event) execution-journal.js:133
    alt session_urn missing
        EJ-->>CALLER: JournalError bad_event :135
    else event.type not in VOCABULARY
        EJ-->>CALLER: JournalError bad_type :136-138, VOCABULARY has 14 entries :40-56
    else privacy_class invalid
        EJ-->>CALLER: JournalError bad_privacy_class :140-142, must be public/internal/sensitive/secret
    end
    opt event.event_id supplied and already seen for this session_urn
        EJ-->>CALLER: {envelope: prior, duplicate:true} :146-150, no re-append
    end
    EJ->>EJ: seq = this._nextSeq.get(session_urn) or 0 :152, _peekSeq
    EJ->>EJ: mint event_id via uris.js if absent :153, _mintEventId :331
    EJ->>EJ: build envelope {schema, event_id, session_urn, seq, occurred_at, harness, agent_did, turn, type, payload, privacy_class} :155-167
    EJ->>EVT: dispatch({kind: exec.<type>, session_id, execution_id, payload: envelope}) :174-179
    Note over EJ: session_urn + seq is the journal key, contiguous from 0, assigned HERE, never by the caller :16-18
    EJ->>EJ: commit _nextSeq.set(session_urn, seq+1) and _seenIds AFTER successful dispatch only :182-185
    EJ-->>CALLER: {envelope, duplicate:false} :187
    Note over EJ,EVT: INVARIANT the journal rides the ADR-005 events adapter — no sixth adapter slot, no new database :9,20-24
    Note over EJ: hydrate(envelopes) rebuilds _nextSeq and _seenIds from persisted envelopes after a restart, idempotent replay :289-300
    Note over EJ: DOC-DRIFT this append path, its 14-event VOCABULARY and its D1-D5 invariants are fully coded — see AB-14.1 for why it is never called in production
```

## AB-14.8 execution-projections and execution-coverage over the journal

```mermaid
flowchart LR
    ENV["AgentExecutionEvent envelope<br/>{session_urn, seq, type, payload}"]
    APPLY["Projection.apply(envelope)<br/>execution-projections.js:45"]
    WM{"envelope.seq <= watermark for session_urn ?<br/>:47-48"}
    NOOP["idempotent no-op<br/>return false"]
    REDUCE["_reduce(envelope)<br/>subclass-defined"]
    TRANS["TranscriptProjection<br/>execution-projections.js:77<br/>input.claimed, assistant.completed, tool.completed only"]
    COST["CostLedgerProjection<br/>execution-projections.js:115<br/>sums usage from assistant.completed only"]
    REBUILD["Projection.rebuild(envelopes)<br/>:55, sorts by session then seq, replays apply()"]
    COV["buildExecutionCoverage(live)<br/>execution-coverage.js:31"]
    SYS["/v1/system execution block"]

    ENV --> APPLY --> WM
    WM -->|"yes, already folded in"| NOOP
    WM -->|"no"| REDUCE
    REDUCE --> TRANS
    REDUCE --> COST
    REBUILD --> APPLY

    COV --> SYS
    COV -.->|"journal.status = live if live.journal passed, else declared"| SYS
    COV -.->|"action_pipeline.status = live if live.pipeline passed, else declared execution-coverage.js:54"| SYS

    DRIFTNOTE["DOC-DRIFT execution-coverage.js:22 imports STAGES, SIDE_EFFECT_CLASSES, APPROVAL_REQUIRED from agent-action-pipeline.js as its ONLY non-test caller<br/>no live instance is ever constructed and passed as live.pipeline or live.journal, so /v1/system reports status: declared always, honestly agreeing with GOVERNANCE-capabilities.md despite the doc's literal no-code claim being false"]
    COV --- DRIFTNOTE
```

## AB-14.9 receipt-minter then audit-chain — the hash chain link and break detection

```mermaid
sequenceDiagram
    autonumber
    participant SPEND as spend attempt<br/>paid, denied, failed or pending-approval
    participant RM as receipt-minter<br/>lib/receipt-minter.js
    participant URIS as uris.mint<br/>lib/uris.js
    participant BC20 as bc20-provenance-bridge<br/>crossActivityOutbound :105
    participant WRITER as events writer<br/>daily-rotated JSONL, ADR-039
    participant AC as audit-chain<br/>lib/audit-chain.js

    SPEND->>RM: mintSpendReceipt({pubkey, origin, scheme, amountSats, outcome, idempotencyKey}) :45
    RM->>URIS: mint({kind:'receipt', pubkey, payload}) :47-57
    URIS-->>RM: urn:agentbox:receipt:pubkey:sha256-12-hash, content-addressed
    alt mint throws
        RM-->>SPEND: urn:agentbox:receipt:error:mint-failed :58-60, never throws
    end
    SPEND->>RM: mintSpendActivity({...}) :78
    RM->>URIS: mint({kind:'activity', payload:{type:'pay-'+scheme, ...}}) :81-91
    RM->>BC20: crossActivityOutbound(urn) :95,105
    Note over RM,BC20: fail-open — a crossOutbound failure is logged to stderr and never blocks the caller :101-112

    Note over WRITER,AC: separately, each JSONL record written gets hash = SHA256(prev_hash || canonical_json(record minus prev_hash,hash)) :7,68-72
    WRITER->>AC: verifyLines(lines, {expectedPrev}) audit-chain.js:89
    loop each JSONL line :102-132
        alt record has neither hash nor prev_hash and chain not yet started
            AC->>AC: legacyPrefix += 1, tolerate as pre-ADR-039 prefix :113-118
        else record.prev_hash != prevHash
            AC-->>WRITER: fail(i, 'prev_hash mismatch') :121, splice detected
        else record.hash != hashRecord(prevHash, record)
            AC-->>WRITER: fail(i, 'hash mismatch (record content altered)') :124-125, edit detected
        else ok
            AC->>AC: prevHash = record.hash, chainStarted = true :128-129
        end
    end
    AC-->>WRITER: {ok, checked, legacy_prefix, broken_at, tail_hash, tail_seq} :134-137
    Note over AC: reorder is equivalent to a splice at the first moved record — deletion at the tail is the ONE mode a bare chain cannot see :13-16
    AC->>AC: readTail(dir) :195, walks newest file backward to resume prevHash and seq across restarts and daily rotation
```

## AB-14.10 failure-taxonomy — the 14 MAST failure modes

```mermaid
classDiagram
    class FailureTaxonomy {
        <<module lib/failure-taxonomy.js>>
        +UNMAPPED : string = "unmapped"
        +classify(context) string
        +tagFailure(context) List~string~
        +isMode(id) bool
        +isTag(tag) bool
    }
    class SpecCategory {
        <<category 1, code 1>>
        FM-1.1 Disobey Task Specification
        FM-1.2 Disobey Role Specification
        FM-1.3 Step Repetition
        FM-1.4 Loss of Conversation History
        FM-1.5 Unaware of Termination Conditions
    }
    class InterAgentCategory {
        <<category 2, code 2>>
        FM-2.1 Conversation Reset
        FM-2.2 Fail to Ask for Clarification
        FM-2.3 Task Derailment
        FM-2.4 Information Withholding
        FM-2.5 Ignored Other Agent's Input
        FM-2.6 Reasoning-Action Mismatch
    }
    class VerificationCategory {
        <<category 3, code 3>>
        FM-3.1 Premature Termination
        FM-3.2 No or Incomplete Verification
        FM-3.3 Incorrect Verification
    }
    FailureTaxonomy --> SpecCategory : MODES lines 41-45
    FailureTaxonomy --> InterAgentCategory : MODES lines 47-52
    FailureTaxonomy --> VerificationCategory : MODES lines 54-56
    note for FailureTaxonomy "classify() priority failure-taxonomy.js:126-159 — 1 context.mode passthrough :146, 2 REASON_TO_MODE symbolic reason :148-150, 3 two high-precision STDERR_HEURISTICS regexes :119-124, 4 else UNMAPPED :159. Attribution — Cemri et al. Why Do Multi-Agent LLM Systems Fail arXiv:2503.13657 2025, PRD-019/ADR-037 D1"
```

## AB-14.11 Precedent match, promote and retire — PrecedentService and the MCP bridge

```mermaid
sequenceDiagram
    autonumber
    participant AGENT as agent<br/>MCP client
    participant PB as precedent-bridge<br/>mcp/servers/precedent-bridge.js:186 handleTool
    participant PS as PrecedentService<br/>lib/precedent-service.js:109
    participant FS as file store<br/>createFileStore, $AGENTBOX_POD_ROOT/precedents/*.json :40

    AGENT->>PB: precedent_match {title, description, category} :128,188
    PB->>PS: matchPrecedent({title, description, category}) :184
    PS->>FS: search(query, namespace, 5) :191
    FS-->>PS: results ranked by word-overlap similarity :72-82
    loop each result, skip retired :193-202
        alt result.similarity >= similarityThreshold 0.85 DEFAULT_SIMILARITY_THRESHOLD :26
            PS-->>PB: {matched:true, precedent, similarity} :205-219
        end
    end
    PS-->>PB: {matched:false, precedent:null, similarity:bestSimilarity} :223-224, no match above threshold
    PB-->>AGENT: JSON result

    AGENT->>PB: precedent_promote {case_id, outcome, reason, category, decided_by, event_id} :153,209
    PB->>PS: storePrecedent({caseId, outcome, reason, category, decidedBy, eventId}) :147
    PS->>PS: build record with promotedAt, retired:false, _searchText = category+outcome+reason :154-169
    PS->>FS: store(precedent-<caseId>, JSON, namespace) :171
    PS-->>PB: {stored:true, key} :172
    PB-->>AGENT: JSON result

    AGENT->>PB: precedent_retire {case_id, reason} :170,233
    PB->>PS: retirePrecedent({caseId, reason}) :311
    PS->>FS: retrieve(key, namespace) :317
    alt not found
        PS-->>PB: throw PrecedentError Precedent not found :319
        PB-->>AGENT: {error:'not_found', message} :244-245
    else found
        PS->>PS: record.retired = true, retiredAt, retireReason :329-331
        PS->>FS: store(key, JSON, namespace) :333
        PS-->>PB: {retired:true, key} :334
        PB-->>AGENT: JSON result
    end
    Note over PS,FS: applyPrecedent (lib/precedent-service.js:239) builds a synthetic kind-31403 ActionResponse plus a PROV-O activity URN via uris.mint — not exposed as an MCP tool, called by the orchestrator directly :279-300
    Note over PS: production wires RuVector semantic search bge-small 384-dim — this file-based store uses deterministic word-overlap, suitable for local/test :10-13
```

## AB-14.12 project-tracker.js publish path and /v1/projects

```mermaid
sequenceDiagram
    autonumber
    participant SCHED as scan trigger<br/>startScheduler :604 or POST /v1/projects/scan
    participant PT as ProjectTracker<br/>lib/project-tracker.js:230
    participant GIT as git via execFileSync<br/>SAFE_ENV, no shell :121-134
    participant URIS as uris.mint<br/>lib/uris.js
    participant PRIMER as PrimerGenerator<br/>lib/project-primer.js:71
    participant HOOK as project-tracking-publish.cjs<br/>config/hooks/, spawned

    SCHED->>PT: scan({dirs, githubEnrichment}) :379
    loop each repo under scanDirs :391-405
        PT->>GIT: gitMetadata(repoPath) :193, branch/lastCommit/commits30d/commitDays/remote/language
        PT->>URIS: mint({kind:'thing', localId:'project-'+sha256_12}) :354, content-addressed on remote or path
        PT->>URIS: mint({kind:'dataset', localId:'commits-'+sha+'-30d'}) :363, requires pubkey scope
        opt githubEnrichment and GITHUB_TOKEN set and remote is github :456
            PT->>PT: _enrichFromGithub(remote) via gh api :329, openIssues/stars, fail-open on error :344-346
        end
        Note over PT: fail-open per repo — one bad repo is logged, scan continues :397-404
    end
    PT->>URIS: mint({kind:'activity', payload:{type:'projscan', dirs, count, outcome}}) :422, one PROV-O scan receipt
    PT-->>SCHED: {projects, scanUrn, durationMs} :438

    Note over PT: GET /v1/projects routes/projects.js:106, GET /v1/projects/:id :146, GET /v1/projects/:id/activity :185
    Note over PT: POST /v1/projects/scan :233, POST /v1/projects/:id/primer :274 delegates to PRIMER.primer() :552-553

    Note right of PT: POST /v1/projects/:id/publish routes/projects.js:327
    alt manifest.project_tracking.nostr_publish is not true
        PT-->>SCHED: {published:false, note:'nostr_publish disabled', urn} :360-367, fail-open, no spawn
    else nostr_publish true
        PT->>HOOK: spawn('node', [PUBLISH_HOOK]) stdin = projectDigest(project) :369-403, slug-addressed, no absolute host paths :31-33
        HOOK-->>PT: exit code, 0 means published
        PT-->>SCHED: {published, urn} :405
    end
    Note over PT: kind-30841 digest publish happens INSIDE the hook (nostr-pod-bridge track subcommand) — project-tracker.js itself never signs or connects to a relay :18-21
```

## AB-14.13 elevation-publisher.js — outbound elevation boundary to VisionClaw

```mermaid
sequenceDiagram
    autonumber
    participant KGE as kg-elevation candidate builder<br/>lib/kg-proposal-extractor.js, see AB-17
    participant EP as buildElevationPublisher<br/>lib/elevation-publisher.js:79
    participant BC20 as bc20-provenance-bridge<br/>durableStore :166
    participant BRIDGE as NostrBridge<br/>mcp/servers/nostr-bridge.js
    participant ACS as agent-control-surface<br/>buildActionRequest, publishPanelEvent

    KGE->>EP: publish(proposal) :151
    alt nostrBridgeEnabled(manifest) is false and no deps.bridge
        EP-->>KGE: {published:false, reason:'nostr-bridge-gate-off'} :86-87,102-103
    else resolveRelays(env) empty and no deps.bridge
        EP-->>KGE: {published:false, reason:'no-relays'} :88-89
    else no signing stack (AGENTBOX_STACK/AGENTBOX_PROFILE unset) and no deps.signer
        EP-->>KGE: {published:false, reason:'no-signing-stack'} :91-93
    end
    Note over EP: static eligibility decided ONCE at boot — reason is cached, logged at debug, never thrown into the request path :84-103
    EP->>EP: ensureConnected() :122, lazy cache bridge+signer, mirrors lib/pod-signer
    alt bridge/signer load throws
        EP-->>KGE: {published:false, reason: loadFailReason} :152-153
    end
    opt proposal.proposal_foreign_urn present
        EP->>BC20: durableStore().put({agentbox_urn, visionclaw_urn, owner_did}) :162-171
        Note over EP,BC20: additive — a storage failure never blocks the publish :159,172
    end
    EP->>ACS: buildActionRequest({panelId: proposal_urn, category:'ontology-elevation', subjectKind:'concept', fields:{propose_request, ...}}) :179-198
    EP->>ACS: publishPanelEvent(bridge, signer, unsigned) :200
    ACS->>BRIDGE: publish signed kind-31402 ActionRequest
    alt publish throws
        EP-->>KGE: {published:false, reason: err.message} :206-213, federation is additive
    else published
        EP-->>KGE: {published:true, event_id, kind} :205
    end
    Note over EP,ACS: the relay agent_registry gate plus broker_cases projection then surface the elevation in the governance inbox a human approves from — never the ungoverned /api/ontology/load path, see ADR-2022 and AB-25
```
