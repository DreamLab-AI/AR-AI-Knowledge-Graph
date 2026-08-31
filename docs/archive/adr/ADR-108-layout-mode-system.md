# ADR-108: Layout Mode System for Knowledge Graph Discovery

> **Renumber note (2026-06-11):** previously filed as `ADR-031-layout-mode-system`,
> which collided with `ADR-031-gpu-analytics-correctness-and-wiring`. Renumbered to
> ADR-108 to enforce the one-number-one-decision uniqueness convention (see ADR-105
> §Numbering and the ADR index in `README.md`). `ADR-031` now refers solely to
> gpu-analytics-correctness-and-wiring.

## Status
Accepted

## Context

VisionClaw renders knowledge graphs in 3D using GPU-accelerated force-directed
layout. The current implementation uses a single Fruchterman-Reingold force model
that produces a featureless ball — communities don't separate because linear springs
don't penalize long edges. Expert analysis (NPI=0.003, stress RMSE=15.05) confirms
the layout conveys near-zero topological information.

Knowledge discovery requires layout algorithms that reveal community structure,
hub importance, hierarchical relationships, and semantic groupings. Different
analytical tasks demand different spatial organizations of the same graph.

The system has 2,812 nodes, 4,744 edges on an RTX A6000 GPU with real-time
WebSocket streaming to AR clients.

## Decision

Implement a **Layout Mode System** with 6 switchable layout algorithms, a
constraint zone system, and smooth animated transitions between modes.

### Layout Modes

| Mode | Algorithm | GPU Kernel | Use Case |
|------|-----------|:----------:|----------|
| `ForceDirected` | ForceAtlas2 with LinLog | New octree kernels | Default exploration |
| `Hierarchical` | Sugiyama (DAG layers) | CPU + GPU placement | Ontology taxonomy |
| `Radial` | Centrality rings | GPU PageRank + ring | Hub analysis |
| `Spectral` | Graph Laplacian eigenvectors | cuSolver LOBPCG | Fast cold start |
| `Temporal` | Z-axis = timestamp | GPU ForceAtlas2 2D + Z offset | Knowledge evolution |
| `Clustered` | ForceAtlas2 + Louvain metanodes | GPU clustering + layout | Community overview |

### Architecture

```mermaid
graph TD
    LMM["LayoutModeManager (Rust actor)<br/>current_mode: LayoutMode enum<br/>target_mode: Option&lt;LayoutMode&gt;<br/>transition_progress: f32 (0.0-1.0)<br/>position_buffers: HashMap&lt;LayoutMode, Vec&lt;Vec3&gt;&gt;"]

    subgraph FA2["ForceAtlas2Engine (GPU)"]
        FA2_1["octree_build_kernel"]
        FA2_2["octree_summarize_kernel"]
        FA2_3["force_accumulate_kernel<br/>(LinLog + degree-scaled repulsion)"]
        FA2_4["adaptive_integrate_kernel<br/>(per-node speed)"]
    end

    subgraph SUG["SugiyamaEngine (CPU - GPU placement)"]
        SUG_1["cycle_removal"]
        SUG_2["layer_assignment"]
        SUG_3["crossing_reduction"]
        SUG_4["coordinate_assignment"]
    end

    subgraph SPEC["SpectralEngine (GPU)"]
        SPEC_1["laplacian_construction (cuSPARSE)"]
        SPEC_2["lobpcg_eigensolver (cuSolver)"]
    end

    subgraph CZS["ConstraintZoneSystem (GPU overlay)"]
        CZS_1["zone_definitions: Vec&lt;Zone&gt;"]
        CZS_2["zone_force_kernel"]
    end

    LMM --> FA2
    LMM --> SUG
    LMM --> SPEC
    LMM --> CZS
```

### Transition System

Mode switches animate over 0.5-1.0 seconds:
```rust
final_position = lerp(current_positions, target_positions, ease_in_out(t))
```

Both source and target positions are computed simultaneously on GPU using
double-buffered position arrays.

### Domain Model (DDD)

**Bounded Context**: `layout` (new module under `src/layout/`)

**Aggregates**:
- `LayoutSession` — current mode, transition state, constraint zones
- `ForceAtlas2State` — octree, per-node speeds, convergence metrics
- `SugiyamaState` — layer assignments, crossing count, virtual nodes
- `SpectralState` — eigenvectors, eigenvalues, initialization quality

**Value Objects**:
- `LayoutMode` — enum with parameters per mode
- `Zone` — center, radius, strength, target node types
- `TransitionState` — source mode, target mode, progress, easing function

**Domain Events**:
- `LayoutModeChanged { from, to, transition_duration_ms }`
- `LayoutConverged { mode, iterations, energy, duration_ms }`
- `ZoneConstraintUpdated { zone_id, center, radius, strength }`

### API Endpoints

```
GET  /api/layout/modes                     List available modes + current
POST /api/layout/mode                      Switch mode { mode, transition_ms }
GET  /api/layout/status                    Current mode, convergence, iteration
POST /api/layout/zones                     Set constraint zones
GET  /api/layout/zones                     Get current zones
POST /api/layout/reset                     Re-randomize positions + re-layout
```

### Client Integration

Settings panel adds a Layout Mode selector (Radio/select group):
- Force-Directed (default)
- Hierarchical
- Radial
- Spectral
- Temporal
- Clustered

Each mode has mode-specific parameters that appear when selected.

### ForceAtlas2 LinLog Specifics

The key algorithmic change from current FR model:

**Repulsion** (degree-scaled, Barnes-Hut):
```
F_repulsion(i,j) = scalingRatio * (deg_i + 1) * (deg_j + 1) / distance(i,j)
```

**Attraction** (LinLog):
```
F_attraction(i,j) = log(1 + distance(i,j))  // per edge
```

**Per-node adaptive speed**:
```
speed_i = global_speed / (1 + sqrt(swing_i))
swing_i = |force_i - prev_force_i|
```

This replaces the current global temperature/damping with per-node adaptation.

## Consequences

- New `src/layout/` module with ~2000 lines of Rust + ~500 lines CUDA
- ForceAtlas2 octree replaces current grid-based repulsion (better O(n log n))
- Settings API extended with layout mode endpoints
- Client adds layout mode selector and mode-specific parameter panels
- Transition animations require double-buffered position arrays on GPU
- Breaking change: SimulationParams gains `layout_mode` field
