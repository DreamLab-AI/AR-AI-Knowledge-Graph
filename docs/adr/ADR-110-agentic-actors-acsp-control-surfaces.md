# ADR-110: Agentic Actors Project Control Surfaces into the Forum (ACSP)

**Status:** Accepted
**Date:** 2026-06-12
**Deciders:** jjohare, VisionClaw platform team
**Supersedes:** None
**Resolves:** PRD-014 §8 open item ("social-approval gate mechanism unresolved —
bead-provenance vs future ACSP")
**Related:**
- `docs/architecture/agent-control-surface-panels.md` (the pipeline this ADR joins)
- agentbox `docs/developer/agent-control-surface-panels.md` (canonical wire schema,
  mirrored at `docs/agentbox-docs/developer/`)
- nostr-rust-forum `crates/nostr-bbs-core/src/governance.rs` (consumer serde — the
  byte-for-byte contract)
- ADR-059 (agent events ingest), PRD-014 (sovereign mesh seams)

## TL;DR

VisionClaw agentic actors that need a **human decision** or want to expose
**live operational state** do it through the Agent Control Surface Protocol —
Nostr kinds 31400–31405 rendered as interactive panels on the forum governance
page (`/community/governance`) — and **not** through bespoke REST approval
endpoints. This ADR ships the producer (`src/services/acsp/`), the decision
return path, and the first agentic actor built on it: the **knowledge
elevation** worker (`src/actors/elevation_actor.rs`), which proposes
formalisation of frontier ontology concepts as `knowledge_enrichment` broker
cases and commits approved drafts to the corpus as PRs.

## Context

Three forces converged:

1. **The elevation backlog became concrete.** The 2026-06-12 twin-rename and
   stub-purge work left a clean split: 196 working-graph pages, ~5.9k authored
   ontology classes, and **~9.8k frontier classes** — concepts referenced by
   axioms but never authored. The owner accepted these as legitimate ontology
   population; they are also a prioritisable elevation work queue. Closing the
   informal→formal loop needs a human-approval seam.

2. **A parallel approval path was about to be duplicated.** VisionClaw already
   has an `enrichment-proposals` REST API (agent-key-gated `/decide`). Building
   elevation on it would create a second, private approval queue with a
   different identity model from the rest of the substrate.

3. **The decision-broker pipeline already exists and is better.** The forum
   relay accepts ACSP kinds from registry-listed agent pubkeys, projects 31402
   ActionRequests into the D1 `broker_cases` governance inbox, renders panels
   via the forum client's `panel_registry`, and lets admins answer with
   kind-31403 responses. Decisions are signed, attributable, durable and
   visible to the whole team. VisionClaw emitted **no** panels (its
   `nostr_bridge` is bead-provenance only) — the gap was purely a producer.

## Decision

### D1 — ACSP is the human-in-the-loop seam

Any VisionClaw agentic actor needing human sign-off opens a **31402 broker
case**; any actor with operational state worth watching publishes a **31400
panel definition + 31401/31404 state**. New REST approval endpoints for
agent-initiated decisions are not added; the existing `enrichment-proposals`
REST path is retained for API-driven integrations but is not the pattern for
agentic actors (candidate for later bridging into case types).

### D2 — The producer (`src/services/acsp/`)

- `events.rs` — serde-exact mirrors of the consumer structs
  (`PanelDefinition`, `FieldDef`/`ActionDef` vocabularies, `ActionRequest`,
  `ActionResponse`) plus unsigned-event builders enforcing the protocol
  invariants: `["d", id]` first tag, snake_case content keys, kebab-case panel
  enums, priority/category/subject/title travelling as **tags** (the relay
  projects them into `broker_cases` without parsing content). Round-trip
  tests lock the wire shapes against the doc examples.
- `client.rs` — built on `nostr_sdk::Client` (relay pool, OK handling,
  automatic reconnection; no hand-rolled relay sockets). Signs with a
  dedicated panel keypair (`ACSP_PANEL_NOSTR_PRIVKEY`, falling back to
  `VISIONCLAW_NOSTR_PRIVKEY`), publishes to `FORUM_RELAY_URL`, and runs a
  long-lived kind-31403 subscription delivering `CaseDecision`s to the owning
  actor, filtered by per-actor case-id namespace (`vc-elev-…`).
- **Registration is operational, not code:** the panel pubkey must be added
  to the relay's `agent_registry` (admin `POST /api/governance/agents/register`).
  The client logs the pubkey at startup; until registered the relay answers
  `blocked: pubkey not in agent registry`.

### D3 — The agentic-actor pattern

An agentic actor owns: a panel id (NIP-33 `d` tag), a case-id prefix, its
panel definition, and its decision handler. The ACSP client is shared
infrastructure; actors compose it rather than re-implementing transport.
`ElevationActor` is the reference implementation and the template for the
queued candidates: sync governance (force-resync action), physics health,
agent telemetry.

### D4 — Elevation, the flagship case

`ElevationActor` (env-gated: `ELEVATION_ACTOR_ENABLED=1`):

- **Select:** load the graph, rank unauthored `owl_class` frontier stubs by
  degree; cap open cases at 5; session skip-list for rejections.
- **Draft:** canonical Title Case page name (corpus convention), JSON-LD
  Class block (`urn:ngm:class:<slug>`, inferred majority domain from
  referencing neighbours, `maturity: draft`, honest draft definition citing
  its references).
- **Case:** `knowledge_enrichment` category, `automation_proposal` subject,
  fields carrying name/domain/definition/referenced-by/path — the custom
  control surface a reviewer needs.
- **Decide:** `approve` → `GitHubPRService::create_ontology_pr` commits the
  draft to the corpus repo as a reviewable PR (the existing sync ingests it
  on merge); `reject` → skip. Panel state (frontier size, open cases,
  elevated/rejected counts, last PR) updates after every transition.

## Considered Options

- **(chosen) ACSP forum panels.** One approval surface for the whole mesh;
  signed, attributable decisions; panels are custom per-actor control
  surfaces; the relay/consumer pipeline already exists and is contract-tested.
- **Rejected: extend the `enrichment-proposals` REST path.** Duplicates the
  broker; private queue invisible to the team; separate identity model;
  every new actor needs new endpoints + UI.
- **Rejected: bead-provenance (kind 30001→9) as the approval signal.** It is
  an audit trail, not a decision mechanism — no actions, no inbox, no
  response kind. (This closes the PRD-014 §8 open question in ACSP's favour.)

## Consequences

**Positive.** Human decisions for agent work happen where the team already
governs; new agentic actors get panels and approvals from shared
infrastructure; the elevation loop is live end-to-end (frontier → case →
PR); wire contract locked by tests on both ends.

**Negative.** A relay-admin registration step gates first use. Panel
identity is another secret to manage. Session-scoped rejection memory means
a restart may re-propose rejected candidates (durable skip-list is future
work).

**Neutral.** Voice/desktop surfaces are unaffected; the producer is
publish-only plus one subscription — no new inbound HTTP surface.

## References

- Producer: `src/services/acsp/{events,client}.rs`
- Flagship actor: `src/actors/elevation_actor.rs` (boot: `src/app_state.rs`)
- Consumer contract: nostr-rust-forum `nostr-bbs-core/src/governance.rs`;
  relay projection `nostr-bbs-relay-worker/src/relay_do/nip_handlers.rs`
  (`project_action_request` / `project_action_response`)
- Schema doc: `docs/agentbox-docs/developer/agent-control-surface-panels.md`
