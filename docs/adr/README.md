---
title: Architecture Decision Records (ADRs)
description: Themed index of every VisionClaw ADR, with title and status drawn from each record's own header.
---

# Architecture Decision Records

> [VisionClaw Docs](../README.md) · Architecture Decision Records (ADRs)

This directory holds the architecture decision records for VisionClaw. Each ADR
captures one significant choice — the context that forced it, the options weighed,
the decision taken, and its consequences. Records are append-only: a decision is
revised by writing a new ADR that supersedes the old one, never by rewriting
history in place. Superseded records move to [`superseded/`](superseded/) and keep
a forward pointer to whatever replaced them.

The numbering runs ADR-011 through ADR-129 with gaps (early consolidation records
ADR-001..010 and ADR-015..026 were folded into later ones). Sections below group the
records by theme; within a theme they are ordered by number. Status is read from
each record's own `## Status` block, so it reflects the decision's real lifecycle
state — `Proposed`, `Accepted`, `Ratified`, `Implementing`, `Implemented`, or
`Superseded`.

> Correction (2026-07-22 doc-drift audit): the range now runs **ADR-011 through
> ADR-132** — the doc-drift sweep backfilled the seven ontology siblings
> (ADR-113/115–120) from shipping code, added ADR-131 (this sweep's own record)
> and ADR-132 (the originating Neo4j-removal decision). See
> [ADR-131](ADR-131-doc-drift-reconciliation-2026-07.md).

## Numbering collisions and supersession

Two ADR numbers are reused by two distinct records each. These are genuine
collisions in the corpus, not aliases — read both files:

| Number | Files |
|--------|-------|
| ADR-074 | [Cross-System DID:Nostr Canonicalisation](ADR-074-cross-system-did-nostr-canonicalisation.md) · [§D2′ did:nostr Multikey convergence](ADR-074-D2-supersession-multikey-convergence.md) |

ADR-074 §D2/§D3/§D4/§D13 are **superseded by [ADR-125](ADR-125-did-nostr-multikey-convergence.md)**;
the remainder of ADR-074 still stands. One fully superseded record lives outside
the main set:

| Record | Superseded by |
|--------|---------------|
| [`superseded/ADR-037` — Binary Protocol Consolidation](superseded/ADR-037-binary-protocol-consolidation.md) | [ADR-061 — Binary Protocol Unification](ADR-061-binary-protocol-unification.md) |

## Folded-in early records (ADR-001..010 / ADR-015..026)

> Correction (2026-07-22 doc-drift audit): the early consolidation records were
> folded into the ADR-011+ corpus during the clean-room rebuild and have no
> standalone files, yet several are still cited by number. This table maps every
> still-referenced early number to its surviving disposition so citations resolve
> rather than dangling (mirrors the ADR-074 collision table above). Recover any
> original body via `git log --follow -- docs/adr/ADR-0NN-*.md`. See
> [ADR-131](ADR-131-doc-drift-reconciliation-2026-07.md) for the sweep that added
> this mapping.

| Folded number | Surviving record / disposition |
|---------------|--------------------------------|
| ADR-026 (3-Tier Model Routing) | No standalone file. The 3-tier routing doctrine (Agent Booster / Haiku / Sonnet-Opus) is live and canonical — cited 6× by [ADR-057](ADR-057-contributor-enablement-platform.md) and by PRD-020:193; the decision text is carried in `project/CLAUDE.md` under "3-Tier Model Routing (ADR-026)". |
| ADR-001..010, ADR-015..025 | Consolidated wholesale into the ADR-011+ corpus during the clean-room rebuild; no standalone files and no live citations. Recover an original via `git log --follow`. |

## Identity & Auth

Authentication enforcement, decentralised identity (DID:Nostr), key custody and
delegation. See [Security model](../explanation/security-model.md).

| ADR | Title | Status |
|-----|-------|--------|
| [011](ADR-011-auth-enforcement.md) | Universal Authentication Enforcement | Accepted |
| [028](ADR-028-ext-optional-auth.md) | NIP-98 as Enterprise Auth — Optional-Auth Extension | Ratified |
| [040](ADR-040-enterprise-identity-strategy.md) | Enterprise Identity Strategy | Accepted |
| [048](ADR-048-dual-tier-identity-model.md) | Dual-Tier Identity Model — KG Notes and Ontology Classes | Implemented |
| [074](ADR-074-cross-system-did-nostr-canonicalisation.md) | Cross-System DID:Nostr Canonicalisation & NIP-26 Trust Pivot | Accepted (§D2–D4/D13 superseded by 125) |
| [074-D2](ADR-074-D2-supersession-multikey-convergence.md) | did:nostr Multikey convergence (canonical DID-document form) | Accepted |
| [081](ADR-081-federation-key-custody-rotation.md) | Federation Key Custody & Rotation Protocol | Deferred (frozen 2026-07-03) |
| [088](ADR-088-auth-service-extraction.md) | Auth Service Extraction | Proposed |
| [094](ADR-094-admin-pubkey-permission-and-delegation.md) | Admin-Pubkey Permission Model and NIP-26 Phone Delegation | Deferred (frozen 2026-07-03) |
| [125](ADR-125-did-nostr-multikey-convergence.md) | DID:Nostr Multikey Convergence (supersedes ADR-074 §D2–D4/D13) | Accepted |

## Binary Protocol & Transport

The position wire format, WebSocket transport and store decomposition. See the
[Binary protocol reference](../reference/binary-protocol.md) and
[WebSocket protocol](../reference/websocket-protocol.md).

| ADR | Title | Status |
|-----|-------|--------|
| [012](ADR-012-websocket-store-decomposition.md) | WebSocket Store Decomposition | Accepted |
| [038](ADR-038-position-flow-consolidation.md) | Position Data Flow Consolidation | Implemented |
| [060](ADR-060-pubkey-filtered-binary-encoder.md) | Owner-pubkey-filtered Binary Position Encoder | Proposed |
| [061](ADR-061-binary-protocol-unification.md) | Binary Protocol Unification — Single Wire, No Versioning | Accepted |
| [`037`](superseded/ADR-037-binary-protocol-consolidation.md) | Binary Protocol Consolidation | Superseded by 061 |

## GPU, Physics & Rendering

CUDA force compute, analytics correctness, layout modes and the zero-allocation
render loop. See [Physics & GPU engine](../explanation/physics-gpu-engine.md).

| ADR | Title | Status |
|-----|-------|--------|
| [013](ADR-013-render-performance.md) | Zero-Allocation Render Loop | Accepted |
| [031](ADR-031-gpu-analytics-correctness-and-wiring.md) | GPU Analytics Correctness and Wiring | Partial |
| [039](ADR-039-settings-consolidation.md) | Settings/Physics Object Consolidation | Implemented |
| [069](ADR-069-force-preset-system.md) | Force-Preset System & Per-Edge-Category Forces | Implementing |
| [070](ADR-070-cuda-integration-hardening.md) | CUDA Integration Hardening | Implementing |
| [098](ADR-098-semantic-constraint-path-reuse.md) | Semantic Constraint Path — Reuse ConstraintData Buffer | Accepted |
| [104](ADR-104-shared-math-utilities.md) | Shared Math Utilities Extraction | Proposed |
| [108](ADR-108-layout-mode-system.md) | Layout Mode System for Knowledge Graph Discovery | Accepted |

## Ontology, Knowledge & Governance

The semantic pipeline, typed graph schema, reasoner posture, triple-store and the
governed ontology writeback loop. See [Ontology pipeline](../explanation/ontology-pipeline.md).

| ADR | Title | Status |
|-----|-------|--------|
| [014](ADR-014-semantic-pipeline-unification.md) | Semantic Pipeline Unification | Accepted |
| [036](ADR-036-node-type-consolidation.md) | Node Type System Consolidation | Accepted |
| [041](ADR-041-judgment-broker-workbench.md) | Judgment Broker Workbench Architecture | Implemented |
| [042](ADR-042-workflow-proposal-object-model.md) | Workflow Proposal Object Model | Implemented |
| [043](ADR-043-kpi-lineage-model.md) | KPI Lineage Model | Accepted |
| [045](ADR-045-policy-engine-approach.md) | Policy Engine Approach | Accepted |
| [049](ADR-049-insight-migration-broker-workflow.md) | Insight Migration Broker Workflow | Implemented |
| [064](ADR-064-typed-graph-schema.md) | Typed Graph Schema (UA-Aligned, URN-Bound) | Implementing |
| [067](ADR-067-ontobricks-mcp-bridge.md) | Ontobricks MCP Bridge & Reasoning Federation | Deferred (frozen 2026-07-03) |
| [072](ADR-072-autordf2gml-feature-engineering.md) | AutoRDF2GML-Inspired Feature Engineering Pipeline | Partial (1 of 6 components) |
| [099](ADR-099-reasoner-posture-whelk-el-primary.md) | Reasoner Posture — Whelk-rs EL Primary, DL Deep-Check Offline | Accepted |
| [100](ADR-100-canonical-iri-and-vocabulary-alignment.md) | Canonical IRI Scheme, rdf:type Classification, Vocabulary Alignment | Accepted |
| [101](ADR-101-triple-store-migration-framework.md) | Triple-Store Migration Framework for Oxigraph | Accepted |
| [106](ADR-106-sparql-patch-ontology.md) | SPARQL PATCH for Ontology Mutations | Accepted |
| [112](ADR-112-ontology-augmentation-retrieval-spine.md) | Ontology Augmentation — Shared-Library Retrieval Spine | Implemented |
| [113](ADR-113-ontology-condensation-mesh.md) | Offline Ontology Condensation Mesh + Staleness-Driven Scheduler | Accepted (retroactive 2026-07-22; scheduler shipped) |
| [114](ADR-114-ontology-class-index-memory-substrate.md) | Memory Substrate for the Ontology Class-Summary Index | Proposed |
| [115](ADR-115-turtle-serialisation.md) | Terse Turtle over SPARQL-Results JSON for Ontology Augmentation | Accepted (retroactive 2026-07-22) |
| [116](ADR-116-tiered-token-budgets.md) | Tiered Token Budgets for Ontology Retrieval | Accepted (retroactive 2026-07-22) |
| [117](ADR-117-server-side-sparql-clamp.md) | Server-Side SPARQL Clamp (LIMIT / Row / Byte Caps) | Accepted (retroactive 2026-07-22; clamp shipped) |
| [118](ADR-118-load-endpoint-hardening.md) | Ontology Load-Endpoint Hardening | Accepted (retroactive 2026-07-22) |
| [119](ADR-119-verifiable-liveness-telemetry.md) | Verifiable Liveness Telemetry for Ontology Retrieval | Accepted (retroactive 2026-07-22; sink shipped) |
| [120](ADR-120-propose-p0-auth.md) | Propose-Endpoint P0 Route Guard | Accepted (retroactive 2026-07-22; shipped + tested) |
| [134](ADR-134-voice-plane-relocated-to-agentbox.md) | Voice meta-controller relocated from `voice-stack/` into the agentbox submodule | Accepted (2026-08-04) |
| [132](ADR-132-neo4j-removal-oxigraph-adoption.md) | Neo4j Removal → Oxigraph Adoption (originating store decision) | Accepted (2026-07-22) |
| [121](ADR-121-self-improving-ontology-writeback-loop.md) | Self-Improving Ontology via Governed Writeback | Deferred (frozen 2026-07-03) |
| [122](ADR-122-two-speed-writeback-governance-routing.md) | Two-Speed Writeback — Governance Routing by Epistemic Class | Deferred (frozen 2026-07-03) |
| [123](ADR-123-voice-mediated-governance-signoff.md) | Voice-Mediated Governance — Conversational Sign-Off | Deferred (frozen 2026-07-03) |
| [127](ADR-127-semantic-trust-layer.md) | Semantic Trust Layer — SHACL in Oxigraph, PROV-O, SPARQL Federation | Accepted |

## Solid / Pod & Sovereignty

Pod-backed graph storage, WAC visibility, the embedded `solid-pod-rs` library and
per-user sovereignty. See [Solid sidecar architecture](../explanation/solid-sidecar-architecture.md).

| ADR | Title | Status |
|-----|-------|--------|
| [027](ADR-027-pod-backed-graph-views.md) | Pod-backed Graph Views | Implemented |
| [029](ADR-029-type-index-discovery.md) | Type Index for Agent and View Discovery | Implemented |
| [030](ADR-030-agent-memory-pods.md) | Agent Memory in Solid Pods | Accepted |
| [032](ADR-032-embed-solid-pod-rs-library.md) | Embed solid-pod-rs as Rust Library (replace JSS sidecar) | Implemented |
| [044](ADR-044-connector-governance-privacy.md) | Connector Governance and Privacy Boundaries | Accepted |
| [050](ADR-050-pod-backed-kgnode-schema.md) | Pod-backed KGNode Schema — Sovereign Private Nodes | Ratified |
| [051](ADR-051-visibility-transitions.md) | Visibility Transitions — Publish / Unpublish Saga | Ratified |
| [052](ADR-052-pod-default-wac-public-container.md) | Pod Default WAC + Public Container Model | Ratified |
| [053](ADR-053-solid-pod-rs-crate-extraction.md) | solid-pod-rs Crate Extraction | Implemented |
| [054](ADR-054-urn-solid-and-solid-apps-alignment.md) | URN-Solid + solid-schema + Solid-Apps Ecosystem Alignment | Ratified |
| [055](ADR-055-sovereign-debt-payoff-sprint.md) | Sovereign Debt Payoff + Phase 2 Sprint | Ratified |
| [056](ADR-056-jss-parity-migration.md) | JSS Parity Migration Architecture | Ratified |
| [066](ADR-066-pod-federated-graph-storage.md) | Pod-Federated Graph Storage with Anti-Replay Signing | Proposed |
| [096](ADR-096-solid-pod-persistence-boundary.md) | Solid Pod Persistence Boundary for the Mobile Bridge | Deferred (frozen 2026-07-03) |
| [107](ADR-107-github-creds-in-pod.md) | GitHub Credentials in Pod — Sovereign Per-User Auth | Ratified |

## XR & Client Visualisation

The Godot/OpenXR native client, WASM visualisation components, the XR transport
handshake and removal of the legacy enterprise dashboard. See
[XR architecture](../explanation/xr-architecture.md).

| ADR | Title | Status |
|-----|-------|--------|
| [046](ADR-046-enterprise-ui-architecture.md) | Enterprise UI Architecture | Accepted |
| [047](ADR-047-wasm-visualization-components.md) | WASM Visualization Components | Implemented |
| [071](ADR-071-godot-rust-xr-replacement.md) | Godot 4 + godot-rust + OpenXR Native APK as the XR Client | Accepted |
| [102](ADR-102-xr-client-backend-transport-completion.md) | XR Client ↔ Backend Transport Completion (Graph V3 + Presence) | Accepted |
| [137](ADR-137-xr-render-offload-and-runtime-quality-dials.md) | XR Render Offload, Runtime Quality Dials, and Full-3D-Default Layout | Accepted (2026-08-30) |
| [103](ADR-103-enterprise-dashboard-removal.md) | Enterprise Dashboard Removal — Migration to Nostr Forum | Accepted |
| [126](ADR-126-omb-adoption-posture.md) | XR/MR Interface — OMB Adoption Posture | Proposed |
| [129](ADR-129-control-center-reimagination.md) | Control Center Re-imagination — glass overlay replaces the docked settings panel | Accepted |

## Mesh Federation & agentbox

Bead provenance, the agent activity channel, the private Nostr relay mesh, the
mobile bridge and ACSP control surfaces. See [Subsystems](../explanation/subsystems.md)
and the [agentbox subsystem docs](../../agentbox/docs/README.md).

| ADR | Title | Status |
|-----|-------|--------|
| [033](ADR-033-git-bead-provenance.md) | Git-as-Bead-Provenance for VisionClaw Governance Events | Proposed |
| [034](ADR-034-needle-bead-provenance.md) | Adopt NEEDLE Patterns for Bead Provenance System | Accepted |
| [058](ADR-058-mad-to-agentbox-migration.md) | Deprecate multi-agent-docker in Favour of agentbox | Accepted |
| [059](ADR-059-bidirectional-agent-channel-server.md) | Bi-directional URI-keyed Agent Activity Channel | Accepted |
| [073](ADR-073-private-nostr-relay-mesh-topology.md) | Private Nostr Relay Mesh Topology & NIP-42 AUTH | Deferred (frozen 2026-07-03) |
| [075](ADR-075-is-envelope-message-contract.md) | Inter-System Message Envelope (IS-Envelope v1) | Proposed |
| [076](ADR-076-nostr-core-absorption-into-upstream.md) | Absorb Forum nostr-core into Upstream nostr Crate | Proposed |
| [092](ADR-092-android-nostr-client-and-signer.md) | Android Nostr Client and Signer for the Mobile Bridge | Deferred (frozen 2026-07-03) |
| [093](ADR-093-mobile-bridge-messaging-substrate.md) | Mobile Bridge Messaging Substrate (NIP-17 / NIP-44 / NIP-59) | Deferred (frozen 2026-07-03) |
| [095](ADR-095-session-summary-event-scheme.md) | Session-as-Summary Event Scheme (kind-30840) | Deferred (frozen 2026-07-03) |
| [097](ADR-097-mobile-bridge-relay-topology.md) | Mobile Bridge Relay Topology and Phased Federation | Deferred (frozen 2026-07-03) |
| [110](ADR-110-agentic-actors-acsp-control-surfaces.md) | Agentic Actors Project Control Surfaces (ACSP) | Accepted |

## Ecosystem, Build & Crates

Cross-substrate convergence, the hexagonal crate split, CQRS removal, URN naming
cutover, QE policy, secrets management, infographics and contracts. See
[Backend architecture](../explanation/backend-architecture.md) and
[Bounded contexts](../explanation/bounded-contexts.md).

| ADR | Title | Status |
|-----|-------|--------|
| [057](ADR-057-contributor-enablement-platform.md) | Contributor Enablement Platform | Deferred (frozen 2026-07-03) |
| [062](ADR-062-qe-prd-adr-ddd-graph-scaffolding.md) | QE Graph Scaffolding — PRD / ADR / DDD Traceability via URN | Accepted |
| [063](ADR-063-urn-traced-operations.md) | URN-Traced Operations Across All Subsystems | Accepted |
| [065](ADR-065-rust-code-analysis-pipeline.md) | Rust-Native Code Analysis Pipeline | Deferred (frozen 2026-07-03) |
| [068](ADR-068-logseq-block-level-fidelity.md) | Logseq Block-Level Fidelity (Matryca-Heritage Parser) | Implementing |
| [077](ADR-077-ecosystem-qe-policy.md) | Ecosystem Quality Engineering Policy | Proposed |
| [078](ADR-078-cross-substrate-library-convergence.md) | Cross-Substrate Library Convergence | Deferred (frozen 2026-07-03) |
| [079](ADR-079-forum-setup-skill-provider-abstraction.md) | Forum-Setup Skill Provider Abstraction | Deferred (frozen 2026-07-03) |
| [080](ADR-080-forum-kit-deployment-topology-patterns.md) | Forum Kit Deployment Topology Patterns | Deferred (frozen 2026-07-03) |
| [082](ADR-082-cross-substrate-test-fixture-sharing.md) | Cross-Substrate Test Fixture Sharing Protocol | Deferred (frozen 2026-07-03) |
| [083](ADR-083-dreamlab-ai-website-cutover-migration.md) | dreamlab-ai-website Cutover Migration Pattern | Deferred (frozen 2026-07-03) |
| [084](ADR-084-cloud-infrastructure-mapping-for-kit-consumers.md) | Cloud Infrastructure Mapping for Kit Consumers | Deferred (frozen 2026-07-03) |
| [085](ADR-085-forum-config-package-architecture.md) | forum-config/ Package Architecture & Branding Extension Points | Deferred (frozen 2026-07-03) |
| [086](ADR-086-git-over-http-ingest-unification.md) | Git-Over-HTTP Ingest Unification | Implemented |
| [087](ADR-087-rate-limit-consolidation.md) | Rate Limit Consolidation | Proposed |
| [089](ADR-089-cqrs-bus-removal.md) | CQRS Dead Bus Removal | Accepted |
| [090](ADR-090-hexagonal-crate-modularisation.md) | Hexagonal Crate Modularisation | Accepted |
| [091](ADR-091-fixture-sync-enforcement.md) | Cross-Substrate Fixture Sync Enforcement | Proposed |
| [105](ADR-105-urn-visionclaw-convergence-and-ngm-cutover.md) | urn:visionclaw Convergence and urn:ngm Cutover | Accepted |
| [109](ADR-109-sops-secrets-management.md) | SOPS + age for Ecosystem Secrets Management | Accepted |
| [111](ADR-111-ecosystem-infographic-modernisation.md) | Ecosystem Infographic Modernisation — diagram-as-code | Proposed |
| [124](ADR-124-smart-contract-features-web-contracts.md) | Smart-Contract Features — Web-Contracts on a Single-Use-Seal Through-Line | Implemented |
| [128](ADR-128-build-out-canonical-gitmark-blocktrails.md) | Build-Out — Adopt gitmark/blocktrails as the Web-Contract Substrate | Accepted |
| [131](ADR-131-doc-drift-reconciliation-2026-07.md) | Documentation-Drift Reconciliation Sweep (2026-07-22) | Accepted |

### RVF integration set

The RuVector Format integration is captured as a three-document set rather than a
numbered ADR:

| Document | Title | Status |
|----------|-------|--------|
| [PRD](rvf-integration-prd.md) | RVF (RuVector Format) Integration into VisionClaw | Draft |
| [AFD](rvf-integration-afd.md) | RVF Integration Architecture Fitness Document | Draft |
| [DDD](rvf-integration-ddd.md) | RVF Integration Domain-Driven Design | Draft |

## See also

- [PRD index](../prd/README.md) — product requirements that motivate these decisions
- [DDD index](../ddd/README.md) — domain models the decisions implement
- [Reference index](../reference/README.md) — wire formats, schemas, and config the ADRs govern
- [Explanation index](../explanation/README.md) — the conceptual architecture these records shape
- [agentbox ADRs](../../agentbox/docs/reference/adr/README.md) — decisions owned by the agentbox subsystem, federated into VisionClaw
