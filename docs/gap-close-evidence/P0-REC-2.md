# P0 — REC-2: Broker case queue on the ACSP architecture (kernel + honest docs)

- **Item:** REC-2 (PRD-023 WP-4, ADR-130 Decision 2). P0 scope: the domain
  kernel + honest docs. The control-centre UI surface is P1.
- **Canary:** `CANARY-VC-REC2-CASE` (standing, P0) — fires from the decide path
  when a queued case round-trips `broker:new_case` → `broker:case_decided` over
  live traffic.
- **Base SHA:** `4a595cc8f5ab0323dca09ea23f66bc5746bb0477` (branch `gap-close/2026-07`)
- **Commit:** the single REC-2 commit at the tip of `gap-close/2026-07`
  (`feat(gap-close): REC-2 broker kernel + ACSP case-queue events…`; SHA recorded
  in the sprint receipt — a literal SHA cannot be embedded in the commit that
  carries this file without self-reference).
- **Verified:** 2026-07-08T11:25:52Z; docs body-text fixup 2026-07-08T11:53:30Z
- **Maturity:** `scaffolded` → `integrated` for the kernel + WS-event + docs
  slice. The standing canary's live-traffic fire is `pending-live-session`
  (wired + fired-in-test; it fires for real only when a running server processes
  a live `POST /api/enrichment-proposals/:id/decide`). The control-centre queue
  UI is `planned` (P1, out of this scope).
- **Item status — `partial-by-design at P0`.** The **P0 slice is the domain
  kernel + the `broker:*` WebSocket events + the `ELEVATION_ACTOR_ENABLED` gate
  flip** — that slice is `integrated` (compiles, unit-tested, docs corrected).
  The **control-centre case-queue UI is P1 scope (REC-2/D3)** and is `planned`,
  not delivered on this branch. REC-2 is therefore **deliberately partial at P0
  and closes at P1**; the item-level status is not "done" at P0 and is not
  claimed as such.

## What was implemented

Cherry-picked the storage-agnostic broker **domain kernel** from the unmerged
`crashbug` branch onto the ACSP architecture `main` committed to (ADR-110),
superseding the `crashbug` `BrokerActor` transport and its Neo4j adapter (never
ported — no Neo4j runs in this stack).

- **Kernel** `src/domain/broker/{mod,broker_case,broker_decision,precedent_registry}.rs`
  — `BrokerCase` aggregate (append-only history, no-self-review, terminal
  idempotency, forward-only share-state), `DecisionOrchestrator`, six-variant
  `DecisionOutcome`, `PrecedentRegistry`. Ported from `git show crashbug:…`
  verbatim except the module header (records the ADR-130 D2 port + supersession)
  and an additive `DecisionOutcome::from_action` reconciliation constructor.
  Registered via `pub mod domain;` in `src/lib.rs`. All 19 original `crashbug`
  tests carried over and pass; 2 new `from_action` tests added.
- **ACSP↔kernel reconciliation** `src/services/acsp/events.rs` — a new
  `broker_kernel_reconciliation` test module proves one vocabulary flows: the
  kernel `CaseCategory`/`SubjectKind` serde forms are byte-identical to the ACSP
  producer's `as_tag_value()` strings, an ACSP kind-31403 `ActionResponse`
  parses into a kernel `DecisionOutcome`, and a `KnowledgeEnrichment` `CaseSpec`
  projects to tags matching the kernel serde.
- **Broker WS events** `src/services/broker_events.rs` — `broker:new_case` and
  `broker:case_decided` JSON text frames over the multiplexed `/wss` graph
  socket, using the same `BroadcastMessage` idiom the enrichment-decision audit
  frame uses (the `0x23` beams use a binary frame; these two carry no binary
  kind, so JSON text per the brief). Envelope shape `{type, channel, payload}`
  carried forward verbatim from the superseded `crashbug` broadcast.
- **Handler wired through the kernel** `src/handlers/enrichment_proposals_handler.rs`
  — the WS-9 `decide` path now threads the outcome through
  `DecisionOutcome::from_action` + `DecisionOrchestrator::decide`
  (`derive_kernel_decision`) to obtain the canonical action + any share plan,
  emits `broker:new_case` when a case enters the queue this call and
  `broker:case_decided` on the decision, and observes `CANARY-VC-REC2-CASE`.
  The durable `SqliteEnrichmentRepository` stays the persistence adapter and the
  handler's deliberate record-don't-reject posture is preserved (a verb the
  kernel does not recognise degrades to the raw outcome; an invariant note is
  logged, never fatal). The WS-12 read surface (`broker_inbox_handler.rs`) is a
  fixed agentbox-bridge snake_case contract; its hardcoded
  `category: "knowledge_enrichment"` already equals the kernel serde form (locked
  by the reconciliation test), so re-rooting its wire shape onto the camelCase
  kernel `BrokerCase` was intentionally *not* done — that is not a clean join.
- **`ElevationActor` default flip** `src/actors/elevation_actor.rs` — the
  `ELEVATION_ACTOR_ENABLED` gate now defaults ON in dev/staging and stays opt-in
  in production (`is_production_from` reads `APP_ENV`/`NODE_ENV`; the prod
  docker-compose profile sets `NODE_ENV=production`). An explicit env value
  always wins. The relay-URL + panel-secret requirements are unchanged, so a dev
  box without a relay stays dormant.
- **Canary** `CANARY-VC-REC2-CASE` was already seeded in
  `services::liveness_harness::P0_CANARIES`; a `CANARY_REC2_CASE` const now
  single-sources the id shared by the seed and the observe call.

## Five lying docs corrected (each names this change)

- `CHANGELOG.md` — new `[Unreleased]` section documenting the kernel port, WS
  events, reconciliation, canary, `ElevationActor` default, and the doc
  corrections.
- `docs/adr/ADR-041-judgment-broker-workbench.md` — Status → superseded-in-part
  by ADR-110 + ADR-130 D2; a dated correction banner marks the "Implemented
  2026-04-20" notes as the historical `crashbug` design (never on `main`); the
  `POST /api/broker/cases*` / `/subscribe` routes flagged as non-existent on
  `main`.
- `docs/adr/ADR-033-git-bead-provenance.md` — correction banner + point 2 fixed:
  the governance publisher is the ADR-110 ACSP producer over the ported kernel,
  not the unmerged `BrokerActor`.
- `docs/explanation/ecosystem-convergence.md` — the two lines naming "BrokerActor
  on crashbug" / "VisionClaw … + BrokerActor" now describe the ACSP producer over
  the storage-agnostic kernel.
- `docs/reference/rest-api.md` — the Broker/Governance section corrected to the
  routes that exist on `main` (`GET /api/broker/inbox`, `/cases/:id`,
  `POST /api/enrichment-proposals/:id/decide`), the Nostr/WS event table
  re-attributed to the ACSP producer + `broker_events`, and the inline line-1277
  `BrokerActor` mention fixed.

### Beyond the five: additional live-`main` `BrokerActor` assertions corrected

A whole-repo grep surfaced four further docs asserting `BrokerActor` (and the
non-existent `src/actors/broker_actor.rs`) as shipped `main` code — the same
defect the WP-4 falsification statement forbids ("*any* document"). In the first
pass each received a dated ADR-130 D2 correction banner:

- `docs/CHANGELOG.md` (distinct from the root `CHANGELOG.md`) — asserted a
  `BrokerActor` startup panel from `src/actors/broker_actor.rs`.
- `docs/prd/PRD-013-solid-git-ingest-surface.md` — "VisionClaw's BrokerActor
  (`broker:new_case`)".
- `docs/ddd/ddd-mesh-federation-context.md` — event tables naming
  `BC-MESH-VC (BrokerActor)` as the `broker:*` publisher.
- `docs/adr/ADR-086-git-over-http-ingest-unification.md` — "BrokerActor emits
  `broker:new_case`/`broker:case_decided`" / "WebSocket events unchanged".

## Gap-close fixup (2026-07-08, adversarial-verifier finding)

The adversarial verifier found the first pass had corrected **only the banner**
in four of the nine documents while their **body text still asserted the
`crashbug` `BrokerActor` as live `main` infrastructure** — which by itself trips
the WP-4 falsification statement ("*any* document"). This pass fixed the body
text **in place** so the body no longer contradicts the banner. Every corrected
assertion now describes what actually ships on this branch: the ADR-110 stateless
ACSP producer, the forum-hosted case queue, and the ported domain kernel
(`src/domain/broker/`) with the enrichment REST fallback handlers; `BrokerActor`
appears only as the superseded `crashbug`-branch design.

- `docs/adr/ADR-033-git-bead-provenance.md` — body lines ~87/165/209/221: the
  `BrokerDecisionMade` writer, the write-transaction mitigation, the
  `publish_governance_decision()` commit-step, and the ADR-041 cross-reference
  now name the broker governance publisher (ACSP producer + enrichment-decide
  handler over `src/domain/broker/`), with `BrokerActor` marked superseded.
- `docs/prd/PRD-013-solid-git-ingest-surface.md` — body lines ~124/373-375/
  419-421/532-543: the US-6 push, the G6 data-flow box, the G7 producer/consumer
  table, and the Phase-5/6 bullets now attribute the `broker:*` events to the
  enrichment-decide handler (`services::broker_events`), reattribute kinds
  30300/30301 honestly (no kind-30300 Nostr emitter on `main`; 30301 ingest not
  wired), and correct the decide route to `POST /api/enrichment-proposals/:id/
  decide`.
- `docs/ddd/ddd-mesh-federation-context.md` — body lines ~336-345/822-845/862:
  both event tables re-attributed; the **"(implemented)" tags** on the kind-30300
  and kind-30301 rows are corrected to "(superseded design; not on `main`)" while
  kinds 31400/31402 keep "(implemented)" reattributed to the ACSP producer
  (`src/services/acsp/events.rs::build_panel_definition` / `build_action_request`);
  the TR-Enrichment-Proposal-Ingest rule now names the ACSP consumer
  (`ElevationActor`).
- `docs/CHANGELOG.md` — body lines ~33-48: the `[Unreleased] - 2026-05-12` ACSP
  section now carries a per-section "superseded design" note and its bullets
  reattribute kinds 31400/31402 to the ADR-110 ACSP producer, marking the
  `crashbug` `BrokerActor` / `ServerNostrActor` (`src/actors/broker_actor.rs`,
  `src/actors/server_nostr_actor.rs` — both absent on `main`) as the superseded
  transport.

The sweep also caught two files outside the verifier's four that carried the
identical defect and were brought into line here:

- `docs/adr/ADR-086-git-over-http-ingest-unification.md` — its banner even quoted
  the still-lying body ("BrokerActor emits…" / "WebSocket events unchanged").
  Body lines ~308/322/328-329/449 corrected: the G6 data-flow, the G7 location +
  producer table, and the "events unchanged" note now name the enrichment-decide
  handler / ACSP producer.
- `docs/explanation/visionflow-coordination-platform.md` — the coordination-
  topology mermaid showed live `BrokerActor` + `ServerNostrActor` host nodes;
  relabelled to "Broker kernel + REST (`src/domain/broker/`)" and "ACSP producer
  / ElevationActor".
- `docs/diagrams/triptych-src/2-engine.md` — the engine label "ACSP /
  BrokerActor" narrowed to "ACSP producer".

**Grep receipt (this fixup pass):** `grep -rn BrokerActor docs/ --include=*.md`
excluding the evidence dir, `/archive/` and `/superseded/` went from **66 hits →
46 hits**. A targeted live-voice hunt
(`BrokerActor('?s)? (emits|owns|publishes|sends|receives|manages|handles|broadcasts|is (the )?(live|canonical|current))`)
over the same scope returns **0 hits**.

**Residual (judged legitimate, left in place):** the 46 remaining hits are all
(a) dated correction banners; (b) explicit superseded/never-merged framing;
(c) the gap-close problem/decision records that must name the defect
(`docs/prd/PRD-023-gap-close-visionclaw.md`, `docs/adr/ADR-130-*`); (d) the
bannered superseded-design ADR itself (`docs/adr/ADR-041-judgment-broker-workbench.md`,
Status *Superseded-in-part*, whose body is the historical design record the banner
points to); or (e) attributed/conceptual diagram nodes that do **not** assert a
shipped `src/actors/broker_actor.rs` — `docs/adr/ADR-111-ecosystem-infographic-modernisation.md`
(infographic flow nodes; publisher attributed to `agentbox/lib/elevation-publisher.js`)
and `docs/adr/ADR-040-enterprise-identity-strategy.md` (a passing "on behalf of the
BrokerActor" clause). None is an unqualified live-`main` code claim.

## Correction — `ServerNostrActor` sweep never run (2026-07-08, second adversarial-verifier finding)

**The "46 hits, all judged legitimate" claim above (and its restatement in the
Falsification section) was incomplete and is hereby corrected — not deleted, so
the record of the miss stays visible.** Both the first pass and the gap-close
fixup swept only `BrokerActor`. Neither ever ran a `ServerNostrActor` sweep. A
`BrokerActor`-only grep is structurally blind to the sibling phantom actor
(`src/actors/server_nostr_actor.rs`, equally `crashbug`-only, equally absent on
`main`), so "all judged legitimate" was a claim about half the surface.

Running `grep -rn "ServerNostrActor" docs/ --include="*.md"` (same exclusions)
surfaced the loci a `BrokerActor` sweep could never catch — including two the
"Residual (judged legitimate, left in place)" paragraph above had explicitly
blessed:

- **`docs/adr/ADR-040-enterprise-identity-strategy.md`** — the residual note
  above dismissed this as "a passing 'on behalf of the `BrokerActor`' clause."
  It was in fact a whole `### Agent Control Surface Protocol` subsection asserting
  in the present tense that "the `ServerNostrActor` **now publishes** governance
  events (kinds 31400, 31402) **on behalf of the `BrokerActor`**" — an unqualified
  live-`main` claim naming *both* phantom actors. Now rewritten to the ADR-110
  ACSP producer (`src/services/acsp/events.rs`) signed via
  `src/services/nostr_service.rs`, with a dated correction banner.
- **`docs/how-to/operations/bridge-audit-drift-runbook.md`** — an on-call
  diagnostic step told responders to check the "`ServerNostrActor` mailbox" and
  "restart the server-nostr sidecar." Neither exists; the kind-30100 fan-out is a
  synchronous call in `BridgeEdgeService::promote`. A real operational hazard,
  now rewritten.
- `docs/explanation/subsystems.md`, `docs/prd/PRD-013-*` (three refs: G7 row,
  relay-topology ASCII, component table), `docs/ddd/ddd-mesh-federation-context.md`
  (TR-WriteBack-Push kind-30300 step), `docs/adr/ADR-086-*` (relay-topology prose),
  `docs/adr/ADR-041-*` (the second, unbannered `## Implementation Notes (2026-05-12)`
  section — heading now qualified `— historical crashbug design, superseded` and
  its present-tense narration past-tensed), plus three more the sweep caught
  outside the seven named loci: `docs/explanation/ecosystem-convergence.md`
  (relay-mesh prose), `docs/adr/ADR-055-*` (B2 sprint row), `docs/adr/ADR-032-*`
  (env-key table row). `docs/adr/ADR-111-*` had its infographic flow/diagram/
  render-text `BrokerActor` labels relabelled to the conceptual **Broker** role so
  the phantom actor name cannot land in regenerated hero art.

**Corrected state (2026-07-08, both sweeps, exclusions = evidence-dir / `archive` / `superseded`):**

```
$ grep -rn "ServerNostrActor" docs/ --include=*.md | grep -v gap-close-evidence | grep -vi archive | grep -vi superseded | wc -l
18
$ grep -rn "BrokerActor"      docs/ --include=*.md | grep -v gap-close-evidence | grep -vi archive | grep -vi superseded | wc -l
38
```

All 18 + 38 residual hits are (a) dated correction banners, (b) explicit
negated/never-merged framing ("no `ServerNostrActor` on `main`"), (c) the
superseded-design ADR-041 body under its now-qualified historical headings, (d)
the gap-close problem/decision records (PRD-023, ADR-130) that must name the
defect, or (e) the anomaly register describing the phantom. A live-voice hunt
over **both** actor names —
`('?s)? (emits|owns|publishes|sends|receives|manages|handles|broadcasts|now |is (the )?(live|canonical|current))` —
returns **0 hits** for each.

## Falsification (PRD-023 WP-4) → how it is met

- *"a case parks in `under_review` forever with no decision path"* — the decide
  handler runs the kernel `DecisionOrchestrator` and persists a terminal
  decision; `broker:case_decided` is emitted every decide call.
- *"any document still names crashbug's `BrokerActor` as live `main`
  infrastructure"* — corrected in two passes. The first pass corrected the five
  primary docs and bannered four more; the adversarial verifier then found those
  four were **banner-only with the body still asserting `BrokerActor` as live** —
  fixed in the gap-close fixup above (body text corrected in place in ADR-033,
  PRD-013, ddd-mesh, `docs/CHANGELOG.md`, plus ADR-086 and two diagram sources).
  A repo grep for `BrokerActor` (excluding the evidence dir, `/archive/`,
  `/superseded/`) now returns **46 hits, all judged legitimate** (banners,
  superseded-framing, the gap-close problem/decision records, the bannered
  superseded-design ADR-041, and attributed diagram nodes); a live-voice
  assertion hunt over that scope returns **0 hits**.
- *"REC-2 closes with `ElevationActor` still gated off in every profile"* — the
  gate defaults ON in dev/staging (`production_gate_defaults_dev_on_prod_off`
  test); production stays opt-in.
- The control-centre pending-judgment surface is P1 and is honestly labelled
  `planned` here, not folded into this closure.

## Receipt

```
$ date -u +%Y-%m-%dT%H:%M:%SZ
2026-07-08T11:25:52Z
$ git rev-parse HEAD        # base
4a595cc8f5ab0323dca09ea23f66bc5746bb0477
$ cargo test -p visionclaw-server --lib -- domain::broker services::acsp::events \
      services::broker_events handlers::enrichment_proposals_handler \
      actors::elevation_actor::tests::production_gate
running 42 tests
...
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 710 filtered out; finished in 0.00s
```

`cargo test` compiled the whole `visionclaw-server` crate (lib + tests, default
`gpu` features) to completion, so the `AppState` `ElevationActor` boot edit, the
`app_state.rs` log, the handler wiring, and the `lib.rs` `domain` module all
compile alongside the 42 unit tests above. Breakdown: 21 kernel tests (19
ported + 2 new `from_action`), 9 ACSP/reconciliation, 3 `broker_events`
envelope, 8 enrichment-decide (6 pre-existing contract + 2 new kernel-decision),
1 production-gate.
