---
title: PRD Records Index
description: Curated Product Requirement Documents that govern VisionClaw's architecture and feature direction.
---

# PRD Records

> [VisionClaw Docs](../README.md) · [PRD Records](README.md)

This is the curated formal PRD record for VisionClaw. Each document below states the
problem, the target outcome, and the acceptance criteria for a slice of the system, and
is the source of intent that the matching [ADRs](../adr/README.md) and
[DDD bounded-context maps](../ddd/README.md) realise. Sprint and process PRDs that tracked
day-to-day delivery have been archived; what remains is the evergreen requirement record.

## Numbered PRDs

| PRD | Title |
|-----|-------|
| [PRD-002](PRD-002-enterprise-ui.md) | Enterprise Control Plane UI |
| [PRD-003](PRD-003-contributor-ai-support-stratum.md) | Contributor AI Support Stratum |
| [PRD-004](PRD-004-agentbox-visionclaw-integration.md) | Agentbox integration with VisionClaw (MAD replacement) |
| [PRD-005](PRD-005-graph-cognition-platform.md) | Graph Cognition Platform — understand-anything capabilities in the substrate |
| [PRD-006](PRD-006-visionclaw-agentbox-uri-federation.md) | VisionClaw ↔ Agentbox URI/JSON-LD federation and live agent observability |
| [PRD-007](PRD-007-binary-protocol-unification.md) | Binary Protocol Unification |
| [PRD-008](PRD-008-xr-godot-replacement.md) | XR client replacement — native Quest 3 APK via Godot 4 + godot-rust + OpenXR |
| [PRD-009](PRD-009-feature-engineering-discovery.md) | AutoRDF2GML-inspired feature engineering and discovery |
| [PRD-010](PRD-010-did-nostr-mesh-federation.md) | DID:Nostr mesh federation |
| [PRD-011](PRD-011-visionflow-forum-kit-extraction.md) | VisionClaw forum kit extraction |
| [PRD-012](PRD-012-dreamlab-ai-website-kit-adoption.md) | DreamLab website kit adoption and cloud infrastructure transition |
| [PRD-013](PRD-013-solid-git-ingest-surface.md) | Solid Pod git ingest surface — agent-mediated knowledge federation |
| [PRD-014](PRD-014-ecosystem-productionisation.md) | Ecosystem productionisation — 60% to 80% readiness |
| [PRD-016](PRD-016-hexagonal-crate-modularisation.md) | Hexagonal crate modularisation |
| [PRD-017](PRD-017-nostr-mobile-agent-bridge.md) | Nostr mobile agent bridge |
| [PRD-018](PRD-018-ontosphere-ontology-rigour.md) | Ontosphere-informed ontology rigour and exploration |
| [PRD-019](PRD-019-xr-transport-completion.md) | XR transport completion — connecting the native client to the live backend |
| [PRD-020](PRD-020-pervasive-ontology-agentbox-augmentation.md) | Pervasive ontology ↔ Agentbox augmentation |
| [PRD-022](PRD-022-semantic-trust-layer.md) | Semantic trust layer — W3C shape validation, provenance reification, relay-mediated federation |

## Subsystem PRDs

Focused requirement records scoped to a single subsystem rather than a numbered programme.

| Record | Title |
|--------|-------|
| [clustering-analytics-subsystem](clustering-analytics-subsystem.md) | GPU clustering and analytics subsystem |
| [jss-parity-migration](jss-parity-migration.md) | JSS parity migration (solid-pod-rs v0.4.0 gate) |
| [prd-bead-provenance-upgrade](prd-bead-provenance-upgrade.md) | Bead provenance system upgrade |
| [prd-insight-migration-loop](prd-insight-migration-loop.md) | Insight migration loop (MVP) |
| [prd-visual-query-builder-semantic-planes](prd-visual-query-builder-semantic-planes.md) | Visual query builder with semantic planes (Vive XR client) |
| [PRD-fold-ladder-hierarchical-density](PRD-fold-ladder-hierarchical-density.md) | Fold-level ladder — hierarchical density management (XR + desktop) |

## See also

- [ADR Records](../adr/README.md) — the architectural decisions that implement these requirements
- [DDD Records](../ddd/README.md) — the bounded-context maps that model each PRD's domain
- [VisionClaw Docs](../README.md) — documentation home
