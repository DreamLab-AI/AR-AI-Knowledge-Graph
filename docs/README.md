---
title: VisionClaw Documentation
description: Documentation hub for VisionClaw — a governed agentic mesh for real-time 3D knowledge-graph exploration with GPU-accelerated physics, OWL 2 EL ontology reasoning, and a multi-agent control surface.
---

# VisionClaw Documentation

> [VisionClaw](../README.md) · Documentation Hub

VisionClaw is a governed agentic mesh for real-time 3D knowledge-graph exploration: a Rust backend, CUDA GPU physics, OWL 2 EL ontology reasoning over an embedded Oxigraph triple store, and a multi-agent AI control surface.

## Quick start

```bash
git clone https://github.com/DreamLab-AI/VisionClaw.git
cd VisionClaw && cp .env.example .env
./scripts/launch.sh up dev
```

`./scripts/launch.sh up dev` is the canonical launcher. The explicit fallback is `docker compose -f docker-compose.unified.yml --profile dev up -d` — `docker-compose.unified.yml` is the only compose file shipped.

| Service | URL | Notes |
|---------|-----|-------|
| 3D graph frontend | <http://localhost:3001> | nginx |
| REST API | <http://localhost:4000/api> | HTTP + WebSocket |
| Solid pod | <http://localhost:8484> | per-user pod storage |
| Legacy MCP (TCP) | `localhost:9500` | agent orchestration channel |

The graph store is the embedded Oxigraph triple store backed by SQLite (ADR-11). Neo4j is fully removed ([ADR-132](adr/ADR-132-neo4j-removal-oxigraph-adoption.md)) and there is no separate database browser UI.

## System at a glance

| Layer | Facts |
|-------|-------|
| Backend | 428 Rust files (~178K LOC); 35 Actix actors (19 service + 16 GPU; +10 WebSocket session); 44 hexser handlers (19 directive + 25 query), no CQRS bus (ADR-089); 9 ports, 12 adapters; 8 workspace crates |
| GPU physics | 82 CUDA `__global__` kernels across 9 `.cu` files (5,854 LOC); 55× speedup — 246 ms CPU (4 FPS) → 4.5 ms GPU (222 FPS) at 100K nodes |
| Client | 465 TypeScript/TSX files (422 non-test, ~103K LOC); 16 feature modules |
| Ontology | Whelk-rs OWL 2 EL + SHACL-lite + JSON-LD validation + PROV-O provenance (PRD-022); 7 MCP ontology tools |
| Wire protocol | V4 delta is the current default (V2 = 36 B/node, V3 = 52 B/node) |
| Decision record | ~98 ADRs (ADR-011..127), plus PRDs and DDD context maps |

## Documentation map

Documentation follows the [Diátaxis](https://diataxis.fr/) framework — each quadrant serves a distinct need.

| Category | Purpose | Start here |
|----------|---------|-----------|
| Tutorials | Learning-oriented lessons that teach VisionClaw by doing | [tutorials/](tutorials/README.md) |
| How-to | Task recipes for deployment, development, operations, and features | [how-to/](how-to/README.md) |
| Explanation | Concepts and rationale — architecture, physics, ontology, security | [explanation/](explanation/system-overview.md) |
| Reference | Exhaustive specifications — REST, WebSocket, binary protocol, schema, config | [reference/](reference/README.md) |
| Decisions | Architecture Decision Records governing every major design choice | [adr/](adr/) |
| Formal record | Product Requirements, Domain-Driven Design context maps, and ADRs | [prd/](prd/) · [ddd/](ddd/) · [adr/](adr/) |

### Common entry points

- New here? Start with [What is VisionClaw?](tutorials/what-is-visionclaw.md), then [Installation](tutorials/installation.md) and [Your first graph](tutorials/first-graph.md).
- Building against the API? See [REST API](reference/rest-api.md), [WebSocket protocol](reference/websocket-protocol.md), and [Binary protocol](reference/binary-protocol.md).
- Understanding the system? Read [System overview](explanation/system-overview.md), [Backend architecture](explanation/backend-architecture.md), and [Physics & GPU engine](explanation/physics-gpu-engine.md).
- Operating it in production? See [Deployment](how-to/deployment.md) and the [operations runbooks](how-to/operations/README.md).

## Known issues

Before debugging unexpected behaviour, check [KNOWN_ISSUES.md](KNOWN_ISSUES.md) — it tracks active P1/P2 bugs and their workarounds.

## agentbox subsystem

VisionClaw runs on top of **agentbox**, the sovereign agent-runtime subsystem (skills, identity mesh, pod adapters, ACSP control surfaces). agentbox is a subsystem with its own documentation set — VisionClaw links into it rather than duplicating it.

→ [agentbox documentation](../agentbox/docs/README.md)

## Contributing and history

- [Contributing](CONTRIBUTING.md) — workflow, branching conventions, code standards
- [Changelog](CHANGELOG.md) — version history and release notes

---

*Maintained by DreamLab AI — [Issues](https://github.com/DreamLab-AI/VisionClaw/issues) · [Discussions](https://github.com/DreamLab-AI/VisionClaw/discussions)*
