---
id: ES-05
title: Human-approval governance loop across the estate
area: estate
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2006]
sources:
  - src/services/acsp/mod.rs
  - src/services/acsp/events.rs
  - src/services/acsp/client.rs
  - src/services/decision_elevation.rs
  - src/handlers/broker_inbox_handler.rs
  - src/domain/broker/broker_case.rs
  - src/domain/broker/precedent_registry.rs
  - docs/adr/ADR-2006-acsp-human-approval.md
  - agentbox/management-api/lib/authority.js
  - agentbox/management-api/lib/governance-decision-waiter.js
  - agentbox/management-api/lib/elevation-publisher.js
  - agentbox/management-api/lib/kg-proposal-extractor.js
  - agentbox/management-api/lib/mandate.js
  - agentbox/management-api/lib/receipt-minter.js
  - agentbox/management-api/routes/broker-bridge.js
  - agentbox/management-api/routes/kg-elevation.js
  - agentbox/mcp/nostr-bridge/relay-consumer.js
verified_commit: b00c28a0d
---
## ES-05.1 The loop — one case crossing four systems
```mermaid
flowchart LR
    subgraph ab["agentbox"]
        ACT["agent action needing approval"]
        GATE["buildAuthorityGate.guard<br/>lib/authority.js:207"]
        ELEV["elevation-publisher<br/>lib/elevation-publisher.js"]
        WAIT["GovernanceDecisionWaiter<br/>lib/governance-decision-waiter.js:45"]
        RC["relay-consumer governance branch<br/>mcp/nostr-bridge/relay-consumer.js:314-325"]
        BB["broker-bridge routes<br/>routes/broker-bridge.js:252,371"]
    end
    subgraph forum["ACSP forum relay — stateless surface"]
        K2["kind 31402 ActionRequest<br/>outbound"]
        K3["kind 31403 ActionResponse<br/>inbound, human-signed"]
    end
    subgraph vc["VisionClaw"]
        ACSP["src/services/acsp producer<br/>kinds 31400-31405"]
        INBOX["GET /api/broker/inbox<br/>src/handlers/broker_inbox_handler.rs:126"]
        KERNEL["domain kernel — BrokerCase,<br/>DecisionOrchestrator, PrecedentRegistry<br/>src/domain/broker/"]
        DEC["POST /api/enrichment-proposals/:id/decide"]
        OX["Oxigraph"]
    end
    HUM["human admin in the forum UI"]

    ACT --> GATE
    GATE -- "publish 31402" --> K2
    ELEV -- "publish 31402" --> K2
    K2 --> HUM
    HUM -- "signs 31403" --> K3
    K3 --> RC
    RC -- "notify(event)" --> WAIT
    WAIT -- "resolve awaiting gate" --> GATE
    BB -- "_vcFetch /api/broker/inbox" --> INBOX
    INBOX --> KERNEL
    BB -- "proxy decide" --> DEC
    DEC --> OX
    ACSP --> K2

    INV["INVARIANT ADR-2006 — the stateful BrokerActor and its Neo4j<br/>transport are SUPERSEDED and DELETED. Only the<br/>storage-agnostic domain kernel is retained. Callers must<br/>not resurrect the old actor transport."]
    D1["DIVERGENCE ADR-2006 implementation_status partial — ACSP<br/>gives a forum-native signed-event surface, but its consumers<br/>STILL own pending state and durable reconciliation. Removing<br/>BrokerActor did NOT make the whole workflow stateless."]
    D2["EXTERNAL — the VisionFlow Judgment Broker<br/>handleGovernanceDecision is not on disk in this repo.<br/>relay-consumer.js:316-318 hands the 31403 to the<br/>orchestrator adapter, which writes it as JSON to the pod<br/>governance decisions directory for VisionClaw to collect."]

    KERNEL --> INV
    ACSP --> D1
    RC --> D2
```

## ES-05.2 ACSP event kinds — the whole 31400-31405 block
```mermaid
classDiagram
    class AcspKinds {
        <<enumeration>>
        KIND_PANEL_DEFINITION = 31400
        KIND_PANEL_STATE = 31401
        KIND_ACTION_REQUEST = 31402
        KIND_ACTION_RESPONSE = 31403
        KIND_PANEL_UPDATE = 31404
        KIND_PANEL_RETIRED = 31405
    }
    class ActionRequestContent {
        +String case_id
        +String action
        +Option~String~ reasoning
        +Option~String~ context_url
    }
    class ActionResponseContent {
        +String case_id
        +String decision
        +String responder_pubkey
    }
    class CaseDecision {
        +String case_id
        +String admin_pubkey
    }
    AcspKinds --> ActionRequestContent : 31402 content
    AcspKinds --> ActionResponseContent : 31403 content
    ActionResponseContent --> CaseDecision : from_event when d-tag in range

    note for AcspKinds "All six defined at src/services/acsp/events.rs:18-23.<br/>Builders: kind-31400 PanelDefinition events.rs:210,<br/>kind-31401 full PanelState snapshot events.rs:219."
    note for ActionRequestContent "events.rs:127 — reasoning and context_url serialise<br/>as EXPLICIT null rather than being omitted."
    note for ActionResponseContent "events.rs:136 — published ONLY by human admins via the<br/>forum UI. The relay ENFORCES admin-only 31403<br/>(client.rs:28)."
    note for CaseDecision "client.rs:23 — case_id is the 31402 d-tag this response<br/>answers. Conversion at client.rs:139 succeeds only when<br/>the d-tag falls in range."
```

## ES-05.3 Authority gate — zero-tolerance blocks on a signed approval, else DENY
```mermaid
sequenceDiagram
    autonumber
    participant A as agent action
    participant G as buildAuthorityGate.guard<br/>lib/authority.js:207
    participant C as classifyAction<br/>lib/authority.js:101
    participant P as ACSP publish
    participant W as awaitDecision<br/>deps.awaitDecision
    participant R as readOutcome<br/>lib/authority.js:172

    A->>G: guard{action, params}
    G->>C: classifyAction(actionClass, opts)
    Note over C: AUTHORITY_CLASSES = ['recoverable', 'zero-tolerance']<br/>frozen at lib/authority.js:47. classifyAction returns one of<br/>recoverable | zero-tolerance | escalation-required.<br/>authority_class is a NEW axis, ORTHOGONAL to the old one.
    alt recoverable
        C-->>G: recoverable
        G-->>A: released without a human decision
    else zero-tolerance or escalation-required
        C-->>G: blocking class
        alt no decision consumer wired
            G-->>A: DENIED fail-closed — "no decision consumer wired"<br/>lib/authority.js:217-219
        else awaitDecision present
            G->>P: publish signed kind-31402 ActionRequest
            Note over G,P: priority = critical for zero-tolerance,<br/>else high — lib/authority.js:230
            P-->>G: signedRequest
            G->>W: awaitDecision(signedRequest, {timeoutMs})
            alt a matching signed 31403 arrives
                W-->>G: signedResponse
                G->>R: readOutcome(responseEvent, requestEvent)
                alt approving
                    R-->>G: approve
                    G-->>A: RELEASED
                else denying
                    R-->>G: deny
                    G-->>A: DENIED
                end
            else timeout or unavailable
                W-->>G: null
                G-->>A: DENY — lib/authority.js:265
            end
        end
    end
    Note over A,R: INVARIANT lib/authority.js:21-32 — a zero-tolerance action<br/>is DENIED, never released. Only a VERIFIED, signed,<br/>approving response releases it. No response is ever<br/>fabricated.
```

## ES-05.4 Decision waiter — one relay subscription, a registry of awaiters
```mermaid
sequenceDiagram
    autonumber
    participant G as authority gate
    participant W as GovernanceDecisionWaiter<br/>lib/governance-decision-waiter.js:45
    participant RC as relay-consumer<br/>mcp/nostr-bridge/relay-consumer.js:325
    participant O as orchestrator.handleGovernanceDecision

    G->>W: register awaiter for signedRequest
    W->>W: _keysForRequest(signedRequest)
    Note over W: Correlation keys, governance-decision-waiter.js:56-60 —<br/>e-REQUEST_EVENT_ID, case-CONTENT_CASE_ID and<br/>d-PANEL_D_TAG (NIP-33) as a fallback. Matching mirrors<br/>lib/authority.js readOutcome EXACTLY.
    W->>W: _pending Map key to Set of entries
    rect rgb(230,240,230)
    Note over RC,O: The SINGLE already-running relay subscription
    RC->>RC: inbound kind-31403 from a forum human
    RC->>O: handleGovernanceDecision(event)
    RC->>W: notify(event)
    end
    W->>W: match on e: / case: / d:
    alt a pending awaiter matches
        W-->>G: resolve with the signed response
    else no match
        W-->>W: drop
    end
    alt no response within DEFAULT_TIMEOUT_MS
        W-->>G: null
        Note over W,G: FAIL-CLOSED — DEFAULT_TIMEOUT_MS = 120000<br/>(governance-decision-waiter.js:31). A request whose<br/>response never arrives times out to null, which the<br/>gate treats as a DENY.
    end
    Note over W,RC: INVARIANT — there is NO second relay client. The value<br/>here is the wait registry, and the transport stays the one<br/>connected consumer (governance-decision-waiter.js:13-16).
```

## ES-05.5 Broker inbox — agentbox reads VisionClaw's case list
```mermaid
sequenceDiagram
    autonumber
    participant U as reviewer
    participant BB as fastify GET /api/broker/bridge/inbox<br/>routes/broker-bridge.js:252
    participant VF as _vcFetch<br/>routes/broker-bridge.js:281
    participant VC as GET /api/broker/inbox<br/>src/handlers/broker_inbox_handler.rs:126

    U->>BB: GET /api/broker/bridge/inbox?status=pending
    Note over BB: status enum is pending | claimed | decided | all,<br/>default pending — routes/broker-bridge.js:261
    BB->>VF: fetch /api/broker/inbox
    VF->>VC: HTTP
    alt VisionClaw reachable
        VC-->>VF: {cases: [..], total: N}
        Note over VC: broker_inbox_handler.rs:42-43 emits EXACTLY the shape<br/>broker-bridge.js destructures — cases plus total
        VF-->>BB: inbox
        BB->>BB: cases = inbox.cases || inbox.items || []<br/>routes/broker-bridge.js:291
        BB-->>U: filtered and enriched inbox
    else fetch fails
        VF--xBB: error
        BB-->>U: error "Failed to fetch broker inbox"<br/>routes/broker-bridge.js:283-286
    end
    Note over U,VC: GATING — the broker inbox is a privileged review surface.<br/>The agentbox bridge presents the same credential the<br/>decide route requires (broker_inbox_handler.rs:27-29).
    Note over BB,VC: DOC-DRIFT — broker_inbox_handler.rs:7 cites<br/>broker-bridge.js:224 for the inbox route, but the<br/>fastify registration is at broker-bridge.js:252 and the<br/>_vcFetch call at :281. Line refs in the Rust doc comment<br/>have drifted from the JS file.
```

## ES-05.6 Decide and write back — the gated mutation
```mermaid
sequenceDiagram
    autonumber
    participant U as human reviewer
    participant BB as POST /api/broker/bridge/cases/:id/decide<br/>routes/broker-bridge.js:371
    participant AG as authorityGate.guard<br/>routes/broker-bridge.js:428
    participant VC as POST /api/enrichment-proposals/:id/decide<br/>routes/broker-bridge.js:472
    participant OX as Oxigraph

    U->>BB: decide{decision, rationale}
    BB->>BB: resolve authorityGate — options.authorityGate<br/>|| fastify.authorityGate || buildAuthorityGate(manifest)<br/>routes/broker-bridge.js:236
    alt authorityEnabled — table.enabled !== false (:243)
        BB->>AG: guard(...)
        Note over AG: guard() publishes a kind-31402 ActionRequest and BLOCKS<br/>until a verified signed response arrives<br/>(routes/broker-bridge.js:222) — see ES-05.3
        alt gate approves
            AG-->>BB: released
            BB->>VC: proxy the decision to VisionClaw
            VC->>OX: governed writeback
            OX-->>VC: committed
            VC-->>BB: decisionResult
            BB-->>U: decided
        else gate denies or times out
            AG-->>BB: DENY
            BB-->>U: refused — no writeback occurs
        end
    else authority table disabled
        BB->>VC: proxy without a gate
        VC-->>BB: decisionResult
        BB-->>U: decided
        Note over BB,VC: DIVERGENCE — authorityEnabled is derived from<br/>authorityGate.table.enabled !== false<br/>(routes/broker-bridge.js:243), so an absent or disabled<br/>classification table silently removes the human gate.
    end
```

## ES-05.7 Governed ontology elevation — personal to shared, federated over Nostr
```mermaid
sequenceDiagram
    autonumber
    participant KE as kg-elevation<br/>routes/kg-elevation.js
    participant EX as kg-proposal-extractor<br/>lib/kg-proposal-extractor.js:330
    participant SC as scoreCandidate<br/>lib/kg-proposal-extractor.js:186
    participant BD as buildProposalDescriptor<br/>lib/kg-proposal-extractor.js:224
    participant EP as elevation-publisher
    participant ACS as agent-control-surface<br/>buildActionRequest / publishPanelEvent
    participant NB as NostrBridge (already connected)

    KE->>EX: extractProposals(entries, opts)
    loop each candidate entry
        EX->>EX: normaliseEntry — lib/kg-proposal-extractor.js:111
        EX->>SC: scoreCandidate(norm)
        SC-->>EX: score
        EX->>BD: buildProposalDescriptor(norm, score, opts)
        BD->>BD: buildProposeRequest — lib/kg-proposal-extractor.js:257
        Note over BD: propose_request is a GOVERNED {path, method, body} for<br/>/api/ontology-agent/propose — kg-proposal-extractor.js:214.<br/>governed_path is recorded at :286.
        BD-->>EX: descriptor plus an agent_action LINK beam
    end
    EX-->>KE: proposals
    KE->>EP: publish each proposal
    EP->>ACS: buildActionRequest — SIGNED ACSP kind 31402
    Note over EP,ACS: URN DISCIPLINE — the panel d-tag REUSES the proposal's own<br/>canonical urn-agentbox-thing-PUBKEY-proposal-SHA256_12,<br/>already minted through lib/uris.js. NIP-33 replaceability<br/>keys re-scans of the same concept to the SAME panel. No<br/>ad-hoc identifiers are invented (elevation-publisher.js:33-36).
    ACS->>NB: publishPanelEvent
    alt federation surface available
        NB-->>EP: published
        Note over NB: The relay's agent_registry gate plus broker_cases<br/>projection surface the elevation in the governance inbox.
    else nostr_bridge gate off, NOSTR_RELAYS empty, no signing<br/>stack, nostr-tools absent, or the key will not decrypt
        NB-->>EP: {published: false, reason}
        Note over EP,NB: STANDALONE-OR-FEDERATED CONTRACT ADR-005 —<br/>a no-op logged at debug. It NEVER throws into the request<br/>path: the existing beam plus propose response is returned<br/>unchanged. Federation is ADDITIVE, never load-bearing.
    end
    Note over KE,NB: INVARIANT — this is the SANCTIONED governed path.<br/>The ungoverned /api/ontology/load backdoor is never used<br/>(elevation-publisher.js:16-17).
```

## ES-05.8 One case, end to end, as a state machine
```mermaid
stateDiagram-v2
    [*] --> Proposed
    Proposed --> Classified
    Classified --> Released
    Classified --> AwaitingHuman
    AwaitingHuman --> Approved
    AwaitingHuman --> Denied
    AwaitingHuman --> TimedOut
    TimedOut --> Denied
    Approved --> WrittenBack
    Denied --> Closed
    WrittenBack --> Receipted
    Receipted --> Closed
    Closed --> [*]

    note right of Classified
        classifyAction returns recoverable,
        zero-tolerance or escalation-required
        (authority.js:101).
    end note
    note right of Released
        recoverable — no human decision needed.
    end note
    note right of AwaitingHuman
        kind-31402 published. The gate blocks.
        Correlated by e: / case: / d:.
    end note
    note right of TimedOut
        DEFAULT_TIMEOUT_MS 120000 then null,
        which the gate reads as DENY.
        Fail-closed, never fabricated.
    end note
    note right of WrittenBack
        Proxied to VisionClaw
        /api/enrichment-proposals/:id/decide
        then into Oxigraph.
    end note
    note right of Receipted
        mintSpendReceipt / mintSpendActivity
        (receipt-minter.js:45,78).
    end note
```

## ES-05.9 Mandate and receipt — the durable authority artefacts
```mermaid
sequenceDiagram
    autonumber
    participant I as issuer
    participant M as createMandate<br/>lib/mandate.js:99
    participant S as signMandate<br/>lib/mandate.js:163
    participant T as mandateToAclTurtle<br/>lib/mandate.js:137
    participant CK as isMandateActive<br/>lib/mandate.js:191
    participant RM as receipt-minter<br/>lib/receipt-minter.js

    I->>M: createMandate{issuer, agent, container, modes, issuedAt, expiresAt}
    M->>M: normalisePubkey :53, normaliseModes :60, normaliseContainer :80
    M-->>I: record
    I->>S: signMandate(record, signer)
    S-->>I: signedEvent
    opt reconstruct from the wire
        I->>M: recordFromSignedMandate(signedEvent) — lib/mandate.js:209
    end
    I->>T: mandateToAclTurtle(record)
    T-->>I: WAC turtle for the pod ACL
    loop on each authority check
        I->>CK: isMandateActive(record, nowSec)
        alt within window
            CK-->>I: true
        else expired or not yet valid
            CK-->>I: false — authority refused
        end
    end
    I->>RM: mintSpendReceipt{pubkey, origin, scheme, amountSats, outcome, idempotencyKey}
    RM-->>I: receipt — lib/receipt-minter.js:45
    I->>RM: mintSpendActivity(same shape) — lib/receipt-minter.js:78
    RM->>RM: crossActivityOutbound(activityUrn) — lib/receipt-minter.js:105
    Note over RM: crossActivityOutbound is the federation hop for the<br/>activity URN — see ES-03 for the closed kind map that<br/>governs what may cross.
    Note over I,RM: idempotencyKey makes receipt minting replay-safe, so a<br/>retried decision cannot double-spend.
```

## ES-05.10 Governance divergences — the gap the governing doc names first
```mermaid
flowchart TB
    TOP["TOP OPEN RISK — the governance gap.<br/>Autonomy (recursive spawn, code execution, nightly<br/>unattended dream cycles, background jobs) is LIVE while<br/>the two governors that would make it safe are UNBUILT<br/>PROPOSALS: execution journal ADR-057 and monotonic policy<br/>pipeline ADR-059."]
    G1["No single policy decision point"]
    G2["No canonical replayable record"]
    G3["Every side-effect path is guarded DIFFERENTLY"]
    G4["A post-hook can REWRITE an approval"]
    D2["DIVERGENCE — ADR-051 (Loom) is Proposed but the Loom is<br/>production-critical. The load-bearing external-LLM subunit<br/>runs on a decision record that has not ratified.<br/>see ES-06.1"]
    D4["DIVERGENCE — deferred-distillation MCP tools are NOT built.<br/>ADR-051 names them; only beads substrate primitives exist."]
    D6["DIVERGENCE — the dream governance band (056/058/061/<br/>062-072) is PAPER. The engine runs ahead of its<br/>decision-surface, self-GC and telemetry-contract designs."]
    D7["DIVERGENCE — skill lint is ADVISORY. lint-skills.sh gates<br/>estate hygiene but is NOT a runtime capability gate; an<br/>enabled skill with clean frontmatter is TRUSTED."]
    D8["DIVERGENCE BASELINE-architecture — BrokerActor was never<br/>merged. main uses a stateless ACSP producer plus a<br/>cherry-picked storage-agnostic domain broker kernel<br/>(~936 LOC)."]
    INV["INVARIANT — byte-identical-when-off. A disabled<br/>[skills.*] / [dream_machine] gate leaves NO runtime trace."]

    TOP --> G1
    TOP --> G2
    TOP --> G3
    TOP --> G4
    TOP --> D2
    D2 --> D4
    TOP --> D6
    TOP --> D7
    TOP --> D8
    G3 --> INV
```
