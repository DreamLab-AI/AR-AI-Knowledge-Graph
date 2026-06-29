# DDD-01 — GPU Physics Bounded Context

## Bounded context

The **Physics** bounded context owns the simulation of force-directed
layout for the knowledge graph and the agent graph. It is sovereign over:

- GPU buffer lifecycle and resizing.
- Force-model selection and per-tick integration.
- Numerical safety (NaN / Inf gating, velocity clamps).
- Settlement detection and heartbeat signalling.

It does not own:

- Graph topology (Section 8 — Ontology & Graph Data).
- Position broadcasting to clients (Section 2 — Binary Protocol).
- Rendering (Section 4).

## Ubiquitous language

| Term            | Definition                                                       |
|-----------------|------------------------------------------------------------------|
| **Tick**        | One integration step of the force model.                         |
| **Settle**      | RMS velocity falling below a configured threshold.               |
| **Heartbeat**   | Periodic event emitted regardless of settlement state.           |
| **Mass**        | Per-node scalar in `[1, ∞)` derived from degree.                 |
| **Class**       | Node category (knowledge / ontology / agent), encoded in the     |
|                 | high bits of the node id and uploaded as `class_id`.             |
| **Class mass** | Per-node mass scalar, currently derived from class but read from |
|                 | `class_mass` buffer; future-extension point.                     |
| **Layout engine** | A `LayoutEngine` trait implementation.                         |
| **Centroid**    | Mass-weighted geometric centre of all nodes; gravity target.     |
| **Sentinel**    | Kernel that clamps non-finite values before they propagate.      |

## Aggregates

### Physics aggregate root: `PhysicsSimulation`

Holds:

- `buffers: PhysicsGpuBuffers` (single owner of GPU memory).
- `engine: Box<dyn LayoutEngine>` (active force model).
- `params: SimParams` (validated; see invariants).
- `convergence_state: ConvergenceState` (last-N RMS velocities for
  hysteresis on settle detection).

Invariants:

- `buffers.capacity >= buffers.node_count`.
- `engine.supports_3d() == params.three_dimensional`.
- All `params.*_strength` values are finite and within configured ranges
  (validated at construction).

Operations:

- `step()` → executes one tick. Returns `TickOutcome { rms_velocity,
  iteration, fired_events: Vec<PhysicsEvent> }`.
- `set_engine(engine)` → atomic engine swap; preserves buffers; resets
  convergence state.
- `resize(node_count)` → atomic buffer resize across all buffers.

### Buffer aggregate: `PhysicsGpuBuffers`

The buffer struct is its own consistency boundary. Resizes are atomic:
either every buffer reaches the new capacity, or none do (the resize
returns Err and the old capacity stands).

Buffers held:

```
positions:               DeviceBuffer<Vec3>     // node_count * sizeof(Vec3)
velocities:              DeviceBuffer<Vec3>
forces:                  DeviceBuffer<Vec3>
prev_forces:             DeviceBuffer<Vec3>     // for FA2 LinLog
masses:                  DeviceBuffer<f32>
class_ids:               DeviceBuffer<u32>
class_masses:            DeviceBuffer<f32>      // pad to allocated_nodes
aabb_block_results:      DeviceBuffer<AABB>     // grid-cell partial bounds
partial_kinetic_energy:  DeviceBuffer<f32>      // reduction workspace
zero_buffer:             DeviceBuffer<u8>       // resize-on-grow scratch
```

## Domain events

Emitted by the physics aggregate, consumed by the broadcast and metrics
layers:

- `LayoutStarted { node_count, engine_name, params_hash }`
- `LayoutSettled { iteration, rms_velocity }` — fires when RMS velocity
  hysteresis crosses the configured threshold downward.
- `LayoutDestabilised { iteration, rms_velocity }` — fires when it crosses
  back upward (re-heat from settings change).
- `PhysicsClamped { kind, count }` — kind ∈ `{ NaN, Inf, VelocityCap }`.
  Emitted at most once per tick per kind.

## Commands accepted

- `StartSimulation { graph_topology, params }`
- `StepN { count }` (default 1) — used by the orchestrator's wall-clock loop.
- `SetEngine { engine }`
- `SetParams { params }` (validated; rejects invalid).
- `UpdateTopology { node_diff, edge_diff }` — additive only, never replaces
  positions for nodes already present.
- `ResetSimulation { keep_engine, keep_params }` — re-randomises positions,
  preserves engine/params if flags set.

## Anti-corruption layer to Section 8 (Ontology)

Section 8 owns the graph topology. The physics context consumes a
read-only `GraphTopology` snapshot at `StartSimulation` time and via
`UpdateTopology` deltas. The physics context never reads or writes the
ontology / KG repository directly.

## Anti-corruption layer to Section 2 (Broadcast)

The physics context emits domain events (above). The broadcast layer
subscribes to these events. The physics context never invokes broadcast
operations directly. This separation is what makes the freeze regression
preventable: the broadcast cadence is a *broadcast* concern, not a *physics*
concern.
