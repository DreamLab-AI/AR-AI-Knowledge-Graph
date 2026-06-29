# DDD-16 — Domain-Driven Design: Hexagonal Crate Boundaries

Date        : 2026-05-23
Related     : PRD-016, ADR-090

## 1. Bounded Contexts

The VisionClaw backend has six bounded contexts, each mapping to a workspace crate:

### 1.1 Graph Domain (`visionclaw-domain`)

**Ubiquitous Language**: Node, Edge, GraphData, MetadataStore, BinaryNodeData, SimParams, SettleMode

**Aggregate Roots**:
- `GraphData` — owns nodes, edges, metadata, id_to_metadata map
- `SimParams` — owns all physics simulation parameters including SettleMode

**Value Objects**: `Vec3Data`, `BinaryNodeDataClient`, `NodeId` (u32), `EdgeId` (String)

**Port Traits** (defined here, implemented by adapters):
- `GraphRepository` — load/save graph data
- `OntologyRepository` — load/save ontology classes, axioms, properties
- `SettingsRepository` — load/save application settings

**Domain Events**:
- `GraphLoaded`, `GraphUpdated`, `NodePositionsChanged`
- `SettingsChanged`, `PhysicsParametersUpdated`
- `OntologyValidated`, `OntologyConstraintViolation`

### 1.2 GPU Physics (`visionclaw-gpu`)

**Ubiquitous Language**: ForceCompute, BroadcastOptimizer, FastSettle, Continuous, CUDA kernel, PTX module, spatial hash, Barnes-Hut

**Aggregate Root**: `UnifiedGPUCompute` — owns GPU buffers, kernel launches, position/velocity state

**Invariants**:
- Broadcasts are ALWAYS full position snapshots (BROADCAST-001)
- No delta/diff encoding in the broadcast pipeline
- Energy threshold must be reachable for the current node count
- NaN/Inf positions are filtered before broadcast

**Anti-Corruption Layer**: `GpuBuffers` translates between domain `SimParams` and GPU-native `CudaSimParams`

### 1.3 Ontology Reasoning (`visionclaw-ontology`)

**Ubiquitous Language**: OwlClass, OwlProperty, OwlAxiom, Whelk, SubClassOf, DisjointWith, FunctionalProperty

**Aggregate Root**: `OntologyState` — owns class hierarchy, property graph, axiom set

**Invariants**:
- Ontology nodes are spatially separated from knowledge nodes (graphSeparationX)
- Ontology mass is 10× knowledge mass for visual stability
- Whelk inference results are cached with TTL

### 1.4 Infrastructure Adapters (`visionclaw-adapters`)

**Ubiquitous Language**: Oxigraph, SQLite, SPARQL, GitHub sync, Nostr bridge

**Repository Implementations**:
- `OxigraphGraphRepository` — implements `GraphRepository` port
- `OxigraphOntologyRepository` — implements `OntologyRepository` port
- `SqliteSettingsRepository` — implements `SettingsRepository` port

**Anti-Corruption Layer**: SPARQL query builders translate domain queries to Oxigraph-specific SPARQL

### 1.5 Actor Orchestration (`visionclaw-actors`)

**Ubiquitous Language**: Supervisor, ForceComputeActor, PhysicsOrchestratorActor, GraphStateActor, ClientCoordinatorActor

**Aggregate Root**: `GraphServiceSupervisor` — owns actor lifecycle, message routing

**Invariants**:
- Single `ClientCoordinatorActor` instance shared between supervisor and socket handler
- FastSettle → Continuous fallback on energy threshold exhaustion
- Settings changes trigger reheat with gradual decay

### 1.6 HTTP Surface (`visionclaw-server`)

**Ubiquitous Language**: Route, Handler, Middleware, SocketFlowServer, Settings endpoint

**Context Map**:
- **Conformist** to `visionclaw-domain` (uses domain types directly in JSON responses)
- **Customer-Supplier** with `visionclaw-actors` (sends messages, receives results)
- **Published Language** for REST API (camelCase JSON via serde aliases)

## 2. Context Map

```
┌─────────────────────────────────────────────────────┐
│                  visionclaw-server                   │
│  (HTTP handlers, WebSocket, middleware)              │
│  Conformist to domain types                         │
└──────────┬──────────────────────────┬───────────────┘
           │ messages                 │ port calls
           ▼                         ▼
┌──────────────────┐    ┌─────────────────────────┐
│ visionclaw-actors│    │ visionclaw-adapters      │
│ (orchestration)  │    │ (Oxigraph, SQLite, etc.) │
└──────┬───────────┘    └──────────┬──────────────┘
       │                           │
       ▼                           │
┌──────────────┐ ┌──────────────┐  │
│visionclaw-gpu│ │visionclaw-   │  │
│(CUDA physics)│ │ontology      │  │
└──────┬───────┘ └──────┬───────┘  │
       │                │          │
       ▼                ▼          ▼
┌──────────────────────────────────────┐
│         visionclaw-domain            │
│  (models, ports, events, types)      │
└──────────────────┬───────────────────┘
                   │
                   ▼
┌──────────────────────────────────────┐
│      visionclaw-contracts            │
│  (cross-boundary typed contracts)    │
└──────────────────────────────────────┘
```

## 3. Shared Kernel

The following types are shared across ALL bounded contexts via `visionclaw-domain`:

- `NodeId` (u32), `EdgeId` (String)
- `Vec3Data` { x, y, z }
- `BinaryNodeData` { node_id, x, y, z, vx, vy, vz }
- `SimParams` (physics parameters)
- `SettleMode` (FastSettle / Continuous)
- `GraphData` { nodes, edges, metadata }
- Domain events (trait + enum)

## 4. Module-to-Crate Migration Map

| Source Module | Target Crate | Lines | Phase |
|--------------|-------------|-------|-------|
| `src/models/` | `visionclaw-domain` | 3,256 | 1 |
| `src/types/` | `visionclaw-domain` | 1,407 | 1 |
| `src/errors/` | `visionclaw-domain` | 989 | 1 |
| `src/events/` | `visionclaw-domain` | 3,150 | 1 |
| `src/ports/` | `visionclaw-domain` | 1,323 | 1 |
| `src/protocol/` | `visionclaw-protocol` | 303 | 2 |
| `src/protocols/` | `visionclaw-protocol` | 590 | 2 |
| `src/gpu/` | `visionclaw-gpu` | 9,036 | 3 |
| `src/physics/` | `visionclaw-gpu` | 5,678 | 3 |
| `src/layout/` | `visionclaw-gpu` | 605 | 3 |
| `src/constraints/` | `visionclaw-gpu` | 4,804 | 3 |
| `src/ontology/` | `visionclaw-ontology` | 1,371 | 4 |
| `src/inference/` | `visionclaw-ontology` | 1,394 | 4 |
| `src/reasoning/` | `visionclaw-ontology` | 480 | 4 |
| `src/validation/` | `visionclaw-ontology` | 205 | 4 |
| `src/adapters/` | `visionclaw-adapters` | 7,636 | 5 |
| `src/repositories/` | `visionclaw-adapters` | 10 | 5 |
| `src/actors/` | `visionclaw-actors` | 34,515 | 6 |
| `src/cqrs/` | `visionclaw-actors` | 3,959 | 6 |
| `src/application/` | `visionclaw-actors` | 4,912 | 6 |
| `src/handlers/` | `visionclaw-server` | 35,145 | 7 |
| `src/middleware/` | `visionclaw-server` | 1,149 | 7 |
| `src/config/` | `visionclaw-server` | 4,001 | 7 |
| `src/settings/` | `visionclaw-server` | 2,195 | 7 |
| `src/telemetry/` | `visionclaw-server` | 735 | 7 |
| `src/services/` | `visionclaw-server` | 29,217 | 7 |
| `src/utils/` | Split across crates | 25,264 | 1-7 |
| `src/client/` | `visionclaw-server` | 373 | 7 |
