# ADR-01 — GPU Physics & Force Engines

Status      : Proposed
Date        : 2026-05-16
Supersedes  : ADR-031 (layout mode system, original)
Related     : ADR-02 (Binary Protocol), ADR-08 (Ontology), ADR-11 (Persistence)

## Context

Baseline `41979d33e` already integrates the ForceAtlas2 LinLog kernel and the
5-engine layout mode system (per existing `ADR-031`). Between baseline and
`main`, 30+ commits accumulate around the GPU pipeline, dominantly defects
and their fixes:

- Buffer-resize race between `aabb_block_results`, `partial_kinetic_energy`,
  `zero_buffer`, and `prev_force`.
- Cross-thread supervisor context replacement clobbering a self-initialised
  GPU context (`b58e43e3b`).
- Pre-allocation magic numbers (`1000` → `8192`) (`1e46a780a`, `cb1dd3ed2`).
- Gravity inversion + degree-weighted gravity bugs (`364e650b3`, `9ffd74b99`).
- Bimodal position distribution from broadcast gating (`9ffd74b99`).
- CUDA panics tearing down the actor system (`134121964`).
- BroadcastOptimizer delta filter starving converged clients of position
  frames (the freeze bug that triggered this whole sprint).
- Cell-bounds kernel launched with wrong block count (`f19fde5da`).

This ADR documents a single set of decisions that resolve these root causes
rather than re-applying each fix individually.

## Decision

### D1. Buffer ownership consolidated

All per-node GPU buffers (`positions`, `velocities`, `forces`, `prev_force`,
`mass_weights`, `class_id`, `class_mass`, `aabb_block_results`,
`partial_kinetic_energy`, `zero_buffer`) are owned by a single
`PhysicsGpuBuffers` struct in `src/gpu/buffers.rs` with one
`resize(node_count)` method that resizes every buffer atomically. No
adjacent buffer is allowed to be resized in isolation. This eliminates the
class of bugs fixed in `db6cb9aa9`, `6364f0059`, `0d775c372`, `deeb29bde`.

### D2. GPU context owned by the physics actor, never replaced

`ForceComputeActor` initialises its own CUDA context on first message
(`InitGpu` or first `StepPhysics`). The actor's context is the canonical
context for the lifetime of the actor. The supervisor never injects a
context. This eliminates `b58e43e3b` and the entire class of cross-thread
context bugs. If the actor dies, the supervisor restarts it cold; the new
actor re-initialises its own context.

### D3. Buffer capacity policy

`PhysicsGpuBuffers::new(initial_capacity)` allocates with `initial_capacity =
ceil_to_power_of_two(min(node_count, 16384))`. Growth uses
`new_capacity = max(node_count, current_capacity * 2)`. No magic numbers;
no hand-tuned pre-allocations. The 16384 ceiling is a configured constant in
`PhysicsConfig::max_nodes_warning` (not a hard limit; it logs a warning and
proceeds).

### D4. Panic recovery via supervisor restart, not `catch_unwind`

`catch_unwind` around CUDA kernel launches is fragile and hides real bugs.
Replace with:

- Supervisor strategy `OneForOne` with restart limit 3-per-60s.
- CUDA panic logged at `error!` with full backtrace before the actor dies.
- New actor instance re-initialises GPU context (see D2) and resumes from
  the last known good position snapshot stored in `GraphStateActor`.
- If 3 restarts inside 60s, the actor stops and the supervisor surfaces a
  health alarm (the layout is broken, the system is not).

### D5. Force model registry

Five layout engines are registered as concrete implementations of a
`LayoutEngine` trait in `src/physics/engines/mod.rs`:

```rust
pub trait LayoutEngine: Send + Sync {
    fn step(&self, buffers: &mut PhysicsGpuBuffers, params: &SimParams) -> Result<()>;
    fn supports_3d(&self) -> bool;
    fn convergence_metric(&self, buffers: &PhysicsGpuBuffers) -> f32;
}
```

Engines: `ForceDirected` (ForceAtlas2 LinLog), `StressMajorization`,
`Hierarchical`, `Circular`, `Geographic`. The active engine is held by
`ForceComputeActor` and switched by `SetLayoutMode` message. No engine
owns its own buffers; all share `PhysicsGpuBuffers`.

### D6. Mass model

Node mass is derived from degree as `mass = 1.0 + log2(1 + degree)`.
Mass is uploaded into `class_mass` once at graph load and re-uploaded only
when the graph topology changes (edge add/remove). Mass is not re-derived
per frame.

### D7. Gravity policy

Gravity is a single attractive force toward graph centroid, scaled by mass.
The "degree-weighted gravity replacement" pattern is rejected as an
optimisation that masked a unit-cancellation bug. With D6's log-mass model,
gravity uniformly applied is well-behaved; degree-weighted gravity is
unnecessary and removed.

### D8. NaN / Inf gate

A single `numerical_safety` kernel runs as the last step of every physics
tick, after force integration. It:

- Clamps `|velocity|` to `MAX_VELOCITY = 100.0`.
- Replaces any non-finite position component with the centroid coordinate.
- Counts and reports clamps via a metrics atomic (visible in
  `/metrics/physics_clamp_count`).

This replaces ad-hoc per-kernel safety code from `908f7f728`.

### D9. Broadcast cadence governed by settlement, not FPS

ADR-02 owns this in detail. From the physics-side perspective: the actor
emits **state-transition events only** — `LayoutStarted`,
`LayoutSettled { iteration, rms_velocity }`, and
`LayoutDestabilised { iteration, rms_velocity }`. Settlement is detected
via RMS-velocity hysteresis across the last N ticks. The physics actor
emits **no time-based heartbeat**: wall-clock heartbeat cadence is a
broadcast concern (see ADR-02 D2) and is wholly owned by the broadcast
actor.

## Options considered

### O1. Bring each fix forward individually

Rejected. Defect-level fixes preserve the underlying fragility (separate
buffer resize, ad-hoc panic handling, context-replacement architecture).
Each fix on its own is correct; together they accumulate complexity.

### O2. Re-implement physics from scratch

Rejected. The CUDA kernels themselves (ForceAtlas2 LinLog, AABB, repulsion)
are mature and tested. The defects are in the surrounding actor /
buffer-management code, not the kernel maths.

### O3. Single buffer struct + supervisor restart (this ADR)

Adopted. Removes the entire class of buffer-race and context-replacement
bugs by design rather than by patches.

## Risks

- **R1**: The single-struct buffer refactor touches every callsite that
  reads or mutates GPU buffers. Mitigation: do this refactor in isolation
  as the first task of Phase 5 (per README phasing).
- **R2**: `OneForOne` supervisor restart with cold GPU context re-init may
  be slower than `catch_unwind`. Mitigation: measure on the test harness;
  if restart takes >500ms, add a warm GPU context pool of size 1.
- **R3**: The log-mass model (D6) is empirically chosen. Different graph
  shapes may prefer different mass functions. Mitigation: surface
  `mass_function` as a configurable enum (`log`, `linear`, `sqrt`) and
  default to `log`.

## Rejected from main as buggy / unjustified

- `aee17f6e6 fix: always randomize zero-position nodes on every GPU upload` —
  symptom-level fix. Root cause is that the upload path was running with
  zero-position nodes at all; with D1 + D2, positions are persistent on the
  GPU and uploads are additive, so the randomisation question doesn't arise.
- `cbac7532a fix: merge GPU positions with Neo4j on incremental graph
  upload` — implies positions were being clobbered by a load operation.
  D1's buffer ownership and D6's mass-only-on-topology-change policy
  eliminate the merge requirement.
- `134121964 fix: catch_unwind GPU physics panics` — replaced by D4
  supervisor restart.

## Bugs and smells at the reset point (41979d33e)

To flag for migration awareness, not for rollback baseline:

- `src/actors/force_compute_actor.rs` at baseline initialises buffers
  inside the `Started` handler. This is the source of the cross-thread
  context bug; address as part of D2's actor-owned-context discipline.
- `src/utils/visionclaw_unified_stability.cu` at baseline (50 new lines on
  main) does not yet exist. The stability checks are needed and should be
  brought forward as part of D8.
- The baseline has 5 layout engines (per the commit message) but the
  `LayoutEngine` trait abstraction is not in place yet. Engines are
  selected by an `if/else` chain in `ForceComputeActor`. D5 introduces the
  trait.
