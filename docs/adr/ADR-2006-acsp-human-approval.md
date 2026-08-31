---
id: ADR-2006
title: Human-approval flows go through the stateless ACSP forum surface
date: 2026-08-31
decision_status: accepted
implementation_status: complete
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

- Human approval is a stateless, forum-native surface: no bespoke REST decision
  UI, no broker process to keep alive, decisions carried on signed Nostr events.
- The retained kernel stays usable but is now driven by the ACSP path rather
  than an actor mailbox — callers must not resurrect the old actor transport.
- Coupling to Neo4j is gone, consistent with ADR-2004.

## Verification

`src/services/acsp/mod.rs` documents the producer over kinds 31400-31405.
`src/domain/broker/` contains `broker_case.rs`, `broker_decision.rs`,
`precedent_registry.rs`, `mod.rs`. `src/handlers/broker_inbox_handler.rs`
exists. `src/actors/broker_actor.rs` is absent and there are no `neo4j`
adapters under `src/adapters/`. Verified at `e0f8cd896`.
