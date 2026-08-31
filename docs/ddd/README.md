---
title: DDD Records Index
description: Curated domain-driven-design bounded-context maps for VisionClaw's subsystems.
---

# DDD Records

> [VisionClaw Docs](../README.md) · [DDD Records](README.md)

This is the curated formal domain-driven-design record for VisionClaw. Each document maps a
bounded context — its ubiquitous language, aggregates, domain events, and the seams where it
integrates with neighbours. These records turn the intent captured in the
[PRD records](../prd/README.md) into an explicit domain model that the
[ADRs](../adr/README.md) then bind to concrete crates, ports, and adapters. Process and
sprint notes have been archived; what remains is the evergreen context map.

## Bounded contexts

| Record | Context |
|--------|---------|
| [ddd-agentbox-integration-context](ddd-agentbox-integration-context.md) | Agentbox integration bounded context |
| [ddd-bead-provenance-context](ddd-bead-provenance-context.md) | Bead provenance bounded context |
| [ddd-binary-protocol-context](ddd-binary-protocol-context.md) | Binary protocol bounded context |
| [ddd-feature-engineering-context](ddd-feature-engineering-context.md) | Feature engineering pipeline context |
| [ddd-final-mile-closeout-context](ddd-final-mile-closeout-context.md) | Final-mile closeout bounded context |
| [ddd-gap-close-visionclaw-context](ddd-gap-close-visionclaw-context.md) | Gap-close VisionClaw bounded context |
| [ddd-graph-cognition-context](ddd-graph-cognition-context.md) | Graph cognition bounded context |
| [ddd-mesh-federation-context](ddd-mesh-federation-context.md) | Mesh federation bounded-context map |
| [ddd-nostr-mobile-bridge-context](ddd-nostr-mobile-bridge-context.md) | Nostr mobile agent bridge bounded context |
| [ddd-ontology-augmentation-context](ddd-ontology-augmentation-context.md) | Ontology augmentation bounded context |
| [ddd-ontology-loom-context](ddd-ontology-loom-context.md) | Ontology Loom bounded context (BC24 — corpus lifecycle + model-swappable façade) |
| [ddd-semantic-trust-layer-context](ddd-semantic-trust-layer-context.md) | Semantic trust layer bounded context |
| [ddd-xr-godot-context](ddd-xr-godot-context.md) | XR Godot bounded context (BC22) |

## Domain models

Cross-cutting domain analyses scoped to a subsystem rather than a single integration seam.

| Record | Domain |
|--------|--------|
| [clustering-analytics-domain](clustering-analytics-domain.md) | Graph analytics domain (clustering and analytics subsystem) |
| [semantic-physics-domain](semantic-physics-domain.md) | Semantic physics domain (ontology rigour and constraint-driven layout) |

## See also

- [PRD Records](../prd/README.md) — the requirements each context realises
- [ADR Records](../adr/README.md) — the decisions that bind these models to code
- [VisionClaw Docs](../README.md) — documentation home
