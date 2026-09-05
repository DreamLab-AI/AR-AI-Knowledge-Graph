---
id: ADR-2006
title: Human-approval flows go through the stateless ACSP forum surface
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a human-decision flow that cannot be expressed as a Nostr control panel, or reinstatement of a stateful broker transport
repo: visionclaw
domain: BASELINE-architecture
lineage: Distils legacy ADR-110 (ACSP control surfaces, 2026-06-12) + ADR-130 Decision 2 (supersede crashbug BrokerActor, cherry-pick 936-LOC kernel); marks ADR-041 broker workbench superseded-in-part.
---

# ADR-2006 — Human-approval flows go through the stateless ACSP forum surface

## Context

Agentic actors sometimes need a human decision. The earlier design routed this
through a stateful `BrokerActor` over a Neo4j transport, which crashed and
coupled decisions to the (now removed) graph DB. Its domain kernel — the case,
orchestrator and precedent-registry logic — was sound and storage-agnostic.
Lineage: ADR-110 ACSP control surfaces (2026-06-12), ADR-130 Decision 2
(supersede the actor, keep the 936-LOC kernel); ADR-041 broker workbench is
superseded in part.

## Decision

Actors needing a human decision project Nostr control panels (kinds
**31400-31405**) via the `src/services/acsp` producer into the forum governance
page, and cases surface through `GET /api/broker/inbox`. The stateful
`BrokerActor` and its Neo4j transport are superseded and deleted; only the
storage-agnostic domain kernel (`BrokerCase`, `DecisionOrchestrator`,
`PrecedentRegistry`) is retained under `src/domain/broker`.

## Consequences

- ACSP provides a forum-native signed-event surface. Its consumers still own
  pending state and durable reconciliation; removal of BrokerActor does not make
  the entire approval workflow stateless.
- The retained domain kernel stays available. Its integration into the ACSP
  consumer must be demonstrated separately; callers must not resurrect the old
  actor transport.
- Coupling to Neo4j is gone, consistent with ADR-2004.

## Verification

`src/services/acsp/mod.rs` documents the producer over kinds 31400-31405.
`src/domain/broker/` contains `broker_case.rs`, `broker_decision.rs`,
`precedent_registry.rs`, `mod.rs`. `src/handlers/broker_inbox_handler.rs`
exists. `src/actors/broker_actor.rs` is absent and there are no `neo4j`
adapters under `src/adapters/`. Verified at `e0f8cd896`.

2026-09-05 — the durable case-state authority is now named for the decision-elevation
consumer: [ADR-2101](ADR-2101-durable-decision-elevation-case-state.md) persists `DecisionElevationActor`
cases and decisions into the same `data/enrichment.sqlite3` store via
`src/adapters/decision_elevation_store.rs`, and adds boot reconciliation that resumes or
times out every non-terminal case. This closes the "current elevation processing owns
pending state" and "restart" parts of the closeout for that consumer only; amend/delegate
semantics, supersession, reviewer authority and live relay/PR receipts remain open, so this
record stays `partial`. Verified at `b0bc275f6` (working tree); `cargo test -p visionclaw-server
--lib decision` 79 passed / 0 failed.

## Closeout extension — 2026-09-04

CP-01/03/04/05/08. Owner remains jjohare with governance/actor/storage maintainers. Implementation is partial against the stateless and kernel-driven workflow claims; historical live activation of the ACSP surface is retained. Current elevation processing owns pending state, while the inbox projects enrichment storage through a local DTO. CaseDecision does not retain signed event ID/request reference. Subscription starts at now and logs lag; the handler consumes pending state before asynchronous persistence.

**Acceptance condition:** Name the durable case-state authority and prove domain invariant integration or an explicitly equivalent consumer contract. Preserve event/request correlation, define amend/delegate and supersession, and test early response, duplicate, lag, restart and persistence failure through applied/rejected receipts. Verify reviewer authority at the chosen boundary and carry gate/PR/merge results separately from human approval. Reopen on consumer, event envelope, storage or admission-policy changes. See [consumer review](../../../VisionFlow/docs/estate-review/forum-decisions.md#visionclaw-acsp-consumption-and-recovery) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/acsp-consumer-snapshot.json). No live approval, relay, persistence failure or PR operation ran.

## Acceptance progress — 2026-09-05

**Implemented.** The reproduced defect — `CaseDecision` discarded `event.id`,
`event.sig` and `event.created_at`, so a decision could only be correlated by
re-deriving an identifier from `(case_id, action, responder_pubkey)`, which is
not unique — is closed.

* `src/services/acsp/client.rs`: `CaseDecision` gains `event_id` and
  `created_at`, populated by `decision_from_event` from the signed 31403, plus
  `correlation_id()` and `lag_seconds(observed_at)`.
* `src/actors/elevation_actor.rs`: `decision_record` correlates on the signed
  event id (`elevation-decide:<case>:<event_id>`), falling back to a
  tuple-plus-`local` form for a synthetic gate-reject that answers no signed
  event, and persists both new fields.
* `src/adapters/sqlite_enrichment_repository.rs`: `StoredDecision` gains
  `decision_event_id` / `decision_created_at_s`; the columns are added by
  `apply_additive_migrations` and guarded by a partial unique index
  (`WHERE decision_event_id IS NOT NULL`) so locally minted decisions are never
  constrained against each other. `record_decision` returns the existing row id
  for a re-delivered signed event instead of writing a duplicate.

An ordering bug the tests caught: the index initially lived in `CREATE_SCHEMA`,
which runs *before* the column migration, so opening a pre-ADR-2006 store
aborted. It is created in `apply_additive_migrations` instead.

**Tests.** `cargo test --lib --no-default-features enrichment_repository` — 6
new cases: signed-id round trip, re-delivery recorded once, two distinct signed
decisions both recorded, NULL ids not deduplicated against each other,
suppression surviving a reopen (restart), and a pre-ADR-2006 store migrated on
open. Whole-crate run: 1254 passed, 0 failed.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2006-acsp-correlation.txt`.

**Remains open.** The durable case-state authority is still not named, and
amend/delegate/supersession semantics are undefined. Reviewer authority at the
chosen boundary, early-response and persistence-failure receipts, and
gate/PR/merge results carried separately from human approval all remain. No
live approval, relay or PR operation ran.
