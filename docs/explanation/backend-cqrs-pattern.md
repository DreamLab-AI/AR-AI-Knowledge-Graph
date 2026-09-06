---
title: Backend Architecture (moved)
description: This page has moved. The canonical backend architecture explanation now lives at backend-architecture.md.
---

# Backend Architecture (moved)

> [VisionClaw Docs](../README.md) · [Explanation](README.md)

This page has moved. The backend is documented as **hexagonal ports and adapters with direct hexser dispatch** — there is no CQRS bus (the dead `src/cqrs/` scaffold was removed in ADR-089). The canonical, current explanation is:

**→ [Backend Architecture](backend-architecture.md)**

That page covers the 9 ports, 12 adapters, the 44 hexser handlers (19 `DirectiveHandler` + 25 `QueryHandler`), the eight-crate workspace dependency DAG, and where the actor system fits.

## See also

- [Backend Architecture](backend-architecture.md) — the canonical page
- [Actor Hierarchy](actor-hierarchy.md)
- Governing decisions: [ADR-089 — CQRS Dead Bus Removal](../archive/adr/ADR-089-cqrs-bus-removal.md) · [ADR-090 — Hexagonal Crate Modularisation](../archive/adr/ADR-090-hexagonal-crate-modularisation.md)
