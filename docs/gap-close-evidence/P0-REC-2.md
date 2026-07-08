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
- **Verified:** 2026-07-08T11:25:52Z
- **Maturity:** `scaffolded` → `integrated` for the kernel + WS-event + docs
  slice. The standing canary's live-traffic fire is `pending-live-session`
  (wired + fired-in-test; it fires for real only when a running server processes
  a live `POST /api/enrichment-proposals/:id/decide`). The control-centre queue
  UI is `planned` (P1, out of this scope).

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
defect the WP-4 falsification statement forbids ("*any* document"). Each now
carries a dated ADR-130 D2 correction banner:

- `docs/CHANGELOG.md` (distinct from the root `CHANGELOG.md`) — asserted a
  `BrokerActor` startup panel from `src/actors/broker_actor.rs`.
- `docs/prd/PRD-013-solid-git-ingest-surface.md` — "VisionClaw's BrokerActor
  (`broker:new_case`)".
- `docs/ddd/ddd-mesh-federation-context.md` — event tables naming
  `BC-MESH-VC (BrokerActor)` as the `broker:*` publisher.
- `docs/adr/ADR-086-git-over-http-ingest-unification.md` — "BrokerActor emits
  `broker:new_case`/`broker:case_decided`" / "WebSocket events unchanged".

**Residual (low-risk, not corrected):** three files retain a conceptual/diagram
mention of `BrokerActor` that does not assert a shipped `src/actors/broker_actor.rs`
— `docs/explanation/visionflow-coordination-platform.md` (a mermaid node label),
`docs/adr/ADR-111-ecosystem-infographic-modernisation.md` (an infographic flow
node, publisher already attributed to `agentbox/lib/elevation-publisher.js`), and
`docs/adr/ADR-040-enterprise-identity-strategy.md` (a passing "on behalf of the
BrokerActor" clause). These are diagram/conceptual references, not live-`main`
code claims; flagged here for a later doc-sweep rather than folded into closure.

## Falsification (PRD-023 WP-4) → how it is met

- *"a case parks in `under_review` forever with no decision path"* — the decide
  handler runs the kernel `DecisionOrchestrator` and persists a terminal
  decision; `broker:case_decided` is emitted every decide call.
- *"any document still names crashbug's `BrokerActor` as live `main`
  infrastructure"* — the five documents above are corrected; a repo grep for
  `BrokerActor` now returns only historical/corrected references
  (ADR-041/ADR-033 correction banners, the crashbug branch itself).
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
