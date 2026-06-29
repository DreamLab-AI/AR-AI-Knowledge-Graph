# PRD-10 — External Integrations (agentbox / forum / whelk)

## 1. Capability statement

VisionClaw integrates with four external systems through narrowly-scoped,
versioned contracts:

- **agentbox** — the agent runtime. Hosts the broker, agent control panel,
  agent spawn forms, swarm topology UI. VisionClaw *consumes* agent
  telemetry from agentbox and *forwards* user click-actions on agent nodes
  back to agentbox.
- **forum** (nostr-rust-forum) — the identity, membership, governance, and
  enterprise platform plane. Hosts the dashboard, KPIs, RBAC, decision
  canvas, workflows, connectors. VisionClaw consumes (and never authors)
  enterprise events. Forum's Nostr identity bridges to VisionClaw's session.
- **whelk-rs** — the OWL EL reasoner, vendored in-tree but used through a
  pure-data contract: TBox in, inferred ABox triples out. Whelk does not
  reach into VisionClaw state and VisionClaw does not patch whelk.
- **GitHub (Logseq corpus)** — `jjohare/logseq` markdown source-of-truth
  for the knowledge graph. Read-only HTTPS via `octocrab`. Section 8 owns
  the parse; this section owns the transport. See ADR-10 §D11.

This PRD defines the contracts. It does not specify the producer
implementations (those live in agentbox / forum / whelk repositories). It
specifies precisely what VisionClaw promises to accept, what it promises to
emit, and the failure modes on both sides.

## 2. Why this exists

The codebase between baseline `41979d33e` and `main` HEAD contains a
~7,500-line enterprise platform (broker, workflows, KPIs, connectors,
policy engine, decision canvas, RBAC, OIDC) that was *removed* on `main`
in commit `c64661e97` after the strategic decision to host that platform
in the Nostr-native forum instead. Several earlier commits — `a61a9c095`
(architecture ADRs 040-045), `c3bea48b3` (the full implementation),
`15216949c` (UI), `fe5fdb184` (auth + Neo4j adapters) — collectively
represent the work that was lifted out.

The migration is therefore not "bring forward fewer features", it is
"specify the *boundary* across which those features migrated, so both
sides build compatible code". This PRD owns that boundary. ADR-10
records the design choices.

Without a written contract, the implementor on either side will invent one
ad-hoc, and the integration will drift the first time either side changes.
The freeze investigation taught us how expensive accidental cross-system
coupling becomes; this section is the prophylactic.

## 3. Users and use cases

- **Knowledge worker** opens VisionClaw. The forum has issued them a Nostr
  identity. Their session bridges to VisionClaw without a second login.
  Their authorisation level (read-only vs operator vs admin) flows from the
  forum, not from a VisionClaw setting.
- **Operator** sees agent nodes in the graph view. Clicks on an agent node.
  VisionClaw does *not* render the agent's control panel — instead it
  surfaces the click to agentbox/forum so the operator lands on the
  agentbox-hosted panel for that agent.
- **Telemetry consumer** (VisionClaw's bots subsystem) receives a stream of
  agent state updates from agentbox. The transport guarantees, the message
  shape, and the reconnection behaviour are documented here so the
  consumer implementation is not guessing.
- **Ontology curator** changes a TBox axiom in the ontology repository.
  Whelk runs as a one-shot reasoner, produces inferred triples, and those
  triples flow back into Oxigraph (per ADR-08 / ADR-11). The curator is
  unaware of whelk's existence; the contract is what makes that possible.

## 4. Acceptance criteria

A1. **Inbound telemetry contract is versioned**. Every envelope carries a
    `schema_version` field. Version skew is detected at the boundary;
    incompatible versions are logged once per connection and the
    connection refuses with a structured error rather than parsing
    silently. ADR-10 D1 defines `schema_version = 1`.

A2. **Inbound telemetry is back-pressured by drop, not queue**. If
    VisionClaw's telemetry consumer falls behind, it drops oldest frames
    and increments a counter metric. It does not grow an unbounded queue.
    The contract is explicit about this so agentbox does not implement
    reliable retry.

A3. **Outbound action contract has exactly one transport**. User click on
    an agent node produces a single deterministic action via the chosen
    transport (BroadcastChannel API for same-origin tabs, deep-link
    fallback otherwise). VisionClaw does not attempt multiple delivery
    methods or fall back silently between them within a session.

A4. **Auth bridge is signature-verified, not cookie-shared**. The forum's
    Nostr pubkey is bridged to VisionClaw through a signed challenge
    flow (ADR-06 owns the cryptography; this PRD asserts the contract).
    Cookie sharing is rejected: the forum and VisionClaw may live on
    different origins.

A5. **Enterprise events are inbound-only**. VisionClaw consumes enterprise
    events (org membership change, RBAC role change, KPI threshold
    crossed) if and only if they affect graph rendering or auth posture.
    VisionClaw never *emits* enterprise events. The contract is one-way.

A6. **Whelk runs as a pure function**. The inference contract takes
    `(TBox, ABox)` and returns inferred `ABox`. Whelk does not query
    Oxigraph, does not read settings, does not log to VisionClaw's log
    sinks. It runs in-process but with no side effects.

A7. **Disconnection has bounded recovery**. If the telemetry stream drops,
    VisionClaw's UI shows agent nodes as "stale" within 5 seconds and
    reconnects with exponential backoff (1s, 2s, 4s, 8s, 16s, cap 30s).
    On reconnect, agentbox sends a full snapshot before the delta stream
    resumes.

A8. **No enterprise control logic in VisionClaw**. The codebase must not
    contain broker, workflow, KPI, RBAC, policy-engine, connector, or
    decision-canvas modules. Any reintroduction is rejected at PR review
    against this acceptance criterion.

## 5. Non-goals

- **Hosting any agentbox/forum capability**. Broker, workflows, RBAC,
  policy engine, KPI dashboards, agent spawn forms, swarm topology UI —
  all live in agentbox or the forum. VisionClaw is a visualisation client.
- **Authenticating forum users**. The forum owns user identity, registration,
  password / passkey / PRF-derived Nostr keys, OIDC. VisionClaw accepts
  the bridge artefact (signed challenge response) and nothing more.
- **Reliable delivery of agent telemetry**. The contract is best-effort
  with bounded staleness. Agents whose state matters atomically are
  agentbox's problem; VisionClaw shows the latest received state.
- **Compiling whelk-rs into our build by hand**. Whelk is vendored as a
  workspace dependency but is logically external. PRs to whelk go
  upstream, not into our tree.
- **Bidirectional state sync**. There is no scenario where VisionClaw is
  the authoritative source for agent state, enterprise state, or forum
  state. All flow is from agentbox/forum → VisionClaw, plus the narrow
  outbound action surface in §3.
- **Hosting the agent control panel UI**. The previously-removed `enterprise`
  feature directory and its 22 component files are not reintroduced.
  Clicking an agent surfaces the click; agentbox renders the panel.

## 6. Contracts at a glance (details in ADR-10)

```text
Direction      Transport             Surface
─────────────────────────────────────────────────────────────────────
INBOUND        WebSocket             AgentTelemetryEnvelope
INBOUND        WebSocket             EnterpriseEventEnvelope
INBOUND        Signed challenge      NostrAuthBridge (one-shot)
INBOUND        HTTPS (octocrab)      ParsedMarkdown (GitHub / Logseq corpus)
OUTBOUND       BroadcastChannel API  AgentActionMessage (same-origin)
OUTBOUND       Window deep-link      AgentActionMessage (cross-origin)
INTERNAL       Function call         WhelkInferenceRequest / Response
```

## 7. Acceptance evidence to gather during implementation

- Contract-test harness for inbound telemetry: agentbox-emulator sends
  every variant of every envelope; VisionClaw's consumer either accepts
  or rejects with the contract's structured error. Pact-style is preferred
  (consumer-driven). See `/contract-testing` skill.
- Replay log of one connection lifecycle (connect → snapshot → 1000 deltas
  → forced disconnect → reconnect → snapshot → delta) verifying staleness
  flag rises and falls correctly.
- Click-to-panel round trip recorded in BroadcastChannel and deep-link
  modes; confirm exactly one transport fires per click.
- Whelk inference golden file: a fixed `(TBox, ABox)` pair with a known
  inferred-triple set. Used as a regression fixture every time whelk is
  bumped.
- A test that scans the codebase for re-introduction of any enterprise
  module name (`broker`, `workflows`, `connectors`, `policy`, `mesh_metrics`,
  `decision_canvas`, `rbac`, `kpi`) and fails the build if any reappear
  under `src/handlers/api_handler/` or `client/src/features/enterprise/`.

## 8. Historical reference (informational only)

The following commits represent the *prior* state of the enterprise
platform inside VisionClaw. They are reference for what the contracts on
the agentbox/forum side will need to support, not for what to bring
forward into VisionClaw:

| Commit       | Era      | Description                                    |
|--------------|----------|------------------------------------------------|
| `a61a9c095`  | 2026-04  | Enterprise architecture ADRs 040-045, DDD      |
| `1e1303e75`  | 2026-04  | Service scaffolding: domain, ports, REST       |
| `ed4aac368`  | 2026-04  | 60 enterprise integration tests                |
| `15216949c`  | 2026-04  | Control plane UI: 5 panels, design system      |
| `c3bea48b3`  | 2026-04  | Complete platform: OIDC, RBAC, KPIs, WCAG AA   |
| `fe5fdb184`  | 2026-04  | Auth, Neo4j adapters, policy engine            |
| `7d076d93a`  | 2026-04  | QE audit, 5 design system test files           |
| `fcfc1a166`  | 2026-04  | Live physics + enterprise drawer integration   |
| `ea0e2f50f`  | 2026-04  | Full-viewport dashboard, hash router           |
| `74d503112`  | 2026-04  | Ratify enterprise ADRs, fill how-to docs       |
| `c64661e97`  | 2026-05  | **Remove** enterprise dashboard (ADR-090)      |

ADR-090 (referenced by `c64661e97`) records the migration decision on the
VisionClaw side. ADR-10 in this sprint records the *contract* with the
new home of the platform.

## 9. Out-of-scope smells flagged for ADR review

- **Polling fallback for telemetry**. The freeze regression history
  argues that any "fallback to HTTP polling" path becomes the primary
  path under stress and accumulates its own bugs. ADR-10 must pick one
  transport and reject the fallback.
- **Cookie-shared session across origins**. Subdomain cookie sharing was
  tempting during the OIDC era. It is rejected here because forum and
  VisionClaw may legitimately live on disjoint origins. ADR-10 specifies
  the signed-challenge alternative.
- **Whelk as a long-running service**. Treating whelk as an actor with
  state would couple it to VisionClaw's lifecycle. ADR-10 specifies
  whelk as one-shot function invocations only.
- **Embedding the agent panel in VisionClaw via iframe**. Iframes leak
  click handling, CSP boundaries, and storage isolation. ADR-10 prefers
  out-of-band delivery (BroadcastChannel / deep-link) over iframe
  embedding of agentbox UI inside VisionClaw.
- **Allowing VisionClaw to publish to the forum's Nostr relays**. Once
  VisionClaw can publish, it becomes an authoritative source. ADR-10
  forbids this.

## 10. Bugs and smells at the reset point (41979d33e)

At baseline, VisionClaw had embryonic Nostr authentication
(`686ab7579 added nostr primitives`, `b8f28117b nostr api REST`,
`647e54f51 tighter nostr and edges are cylinders`) but no formalised
external contracts. The enterprise platform did not yet exist; the
removal commit `c64661e97` is future. Migration awareness:

- Baseline `BotsClient` (or its predecessor) likely speaks to agentbox
  through an ad-hoc JSON stream with no envelope versioning. ADR-10 D1
  fixes the envelope.
- Baseline Nostr login is "mostly working" per commit messages. The
  challenge-response auth bridge in ADR-10 D3 supersedes whatever
  ad-hoc state restoration exists at baseline.
- Whelk-rs is present as a vendored crate at baseline but the
  inference contract is not yet pure-functional in code; ADR-10 D5
  specifies the boundary.

## 11. Open questions deliberately left to implementation

- The exact WebSocket URL paths (`/ws/agent-telemetry` vs
  `/ws/agents/stream` etc.) are an implementation detail. The contract
  specifies the envelope shape and back-pressure policy; the URL is
  agentbox's choice.
- The forum's relay set for Nostr publishing is forum's choice.
  VisionClaw only needs to verify a signature; it does not need to
  know which relays the signature was distributed through.
- Whelk version pinning is an Oxigraph/ontology concern (Section 8 /
  Section 11). Section 10 ratifies the contract, not the version.
