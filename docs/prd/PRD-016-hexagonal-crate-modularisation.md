---
title: "PRD-016: Hexagonal Crate Modularisation"
status: Proposed
date: 2026-05-23
author: jjohare
priority: P1
---

# PRD-016: Hexagonal Crate Modularisation

## 1. Problem

The VisionClaw Rust backend is a 123k-line monolith compiled as a single `webxr` crate. With `codegen-units=1` and `opt-level=3`, a single-line change triggers ~12 minutes of LLVM optimisation. This is incompatible with the current bug-squashing phase where rapid iteration is critical.

The codebase already follows hexagonal architecture conventions (`ports/`, `adapters/`, `application/`, `models/`), but these are directories inside one crate — cargo cannot skip unchanged modules.

## 2. Goals

| ID | Goal | Success Metric |
|----|------|----------------|
| G1 | Incremental build time < 2 minutes for a change in any single crate | Measured via `cargo build --release --timings` |
| G2 | No behavioural regression | All existing tests pass; WebSocket binary protocol, GPU physics, settings pipeline unchanged |
| G3 | Each crate independently testable | `cargo test -p <crate>` works without starting GPU or network |
| G4 | Dependency graph is acyclic | `cargo tree` shows no circular dependencies between workspace members |
| G5 | Existing hexagonal boundaries preserved | Ports define traits, adapters implement them, domain has zero framework deps |

## 3. Non-Goals

- No new features — this is a structural refactor only
- No changes to the binary protocol, REST API, or WebSocket behaviour
- No changes to the CUDA kernel compilation pipeline
- No splitting the client (TypeScript) codebase

## 4. Proposed Crate Structure

```
crates/
├── visionclaw-contracts/     # [EXISTING] Cross-boundary typed contracts
├── visionclaw-domain/        # Models, types, errors, events, ports (trait definitions)
│                              # ~15k lines: models/ types/ errors/ events/ ports/
├── visionclaw-gpu/           # GPU compute, CUDA kernels, force compute, broadcast
│                              # ~15k lines: gpu/ physics/ layout/ constraints/
├── visionclaw-ontology/      # Ontology reasoning, inference, validation, OWL
│                              # ~7k lines: ontology/ inference/ reasoning/ validation/
├── visionclaw-adapters/      # Oxigraph, SQLite, GitHub, Nostr adapters
│                              # ~8k lines: adapters/ repositories/ services/parsers/
├── visionclaw-protocol/      # Binary protocol, WebSocket encoding/decoding
│                              # ~4k lines: protocol/ protocols/ utils/binary_*
├── visionclaw-actors/        # Actix actors, supervisor hierarchy, CQRS
│                              # ~38k lines: actors/ cqrs/ application/
├── visionclaw-server/        # HTTP routes, middleware, config, settings, telemetry
│                              # ~40k lines: handlers/ middleware/ config/ settings/ services/
└── webxr (root)              # Thin binary: main.rs + lib.rs wiring only
                               # ~1.5k lines: bin/ + startup glue
```

### Dependency Graph (Acyclic)

```
visionclaw-contracts  (leaf — no workspace deps)
        ↑
visionclaw-domain     (depends on: contracts)
        ↑
   ┌────┼────────────────┐
   │    │                │
visionclaw-gpu    visionclaw-ontology   visionclaw-protocol
   │    │                │                    │
   └────┼────────────────┘                    │
        ↑                                     │
visionclaw-adapters  (depends on: domain, protocol)
        ↑
visionclaw-actors    (depends on: domain, gpu, ontology, adapters, protocol)
        ↑
visionclaw-server    (depends on: domain, actors, adapters, protocol)
        ↑
webxr (root binary)  (depends on: server)
```

## 5. Migration Strategy

### Phase 1: Extract `visionclaw-domain` (leaf crate)
Move models, types, errors, events, and port trait definitions. This has zero external deps and breaks the most dependency chains.

### Phase 2: Extract `visionclaw-protocol`
Binary protocol encoding/decoding is self-contained with domain type deps only.

### Phase 3: Extract `visionclaw-gpu`
GPU compute, physics, layout, constraints. Depends on domain types.

### Phase 4: Extract `visionclaw-ontology`
OWL reasoning, inference, validation. Depends on domain types.

### Phase 5: Extract `visionclaw-adapters`
Oxigraph, SQLite, GitHub sync. Depends on domain ports.

### Phase 6: Extract `visionclaw-actors`
Actix actor hierarchy. Depends on domain, gpu, ontology, adapters.

### Phase 7: Extract `visionclaw-server`
HTTP handlers, middleware. The root `webxr` becomes a thin binary.

## 6. Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Circular dependencies discovered during extraction | Use `cargo tree --invert` to map actual dep graph before each phase |
| Actor message types create coupling between crates | Move message types to `visionclaw-domain` as part of Phase 1 |
| GPU feature flag spans multiple crates | Each GPU-dependent crate re-exports its own feature; root `webxr` propagates |
| Build breaks during migration | Each phase is a separate PR; CI validates `cargo build --release` per phase |
| Test isolation broken | Phase 1 establishes test fixtures in domain crate; adapters get integration test profiles |

## 7. Acceptance Criteria

1. `cargo build --release --timings` shows no single crate taking >3 minutes
2. All existing tests pass (`cargo test --workspace`)
3. `cargo tree` shows acyclic workspace dependency graph
4. Docker wrapper builds succeed with same features (`gpu`, `ontology`)
5. Binary protocol wire format unchanged (verified by snapshot tests)
6. GPU physics behaviour unchanged (verified by convergence tests)
