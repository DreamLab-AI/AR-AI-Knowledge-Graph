---
title: GPU & Wire ABI
doc_id: VC-GPU-ABI
version: 0.1.1
status: draft-for-ratification
verified_commit: 73540faa0
changelog:
  - 0.1.1: correct path-3 actor call-site citation (force_compute_actor.rs 1557 → 2057); fix stale 180-byte comment line ref (:7 → :5)
sources:
  - src/models/simulation_params.rs
  - src/models/force_channels.rs
  - crates/visionclaw-domain/src/models/simulation_params.rs
  - crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu
  - crates/visionclaw-gpu/build.rs
  - src/utils/unified_gpu_compute/execution.rs
  - src/handlers/layout_handler.rs
  - src/actors/gpu/force_compute_actor.rs
date: 2026-08-31
---

# GPU & Wire ABI

## Purpose

Defines the binary contract between the Rust host and the CUDA layout engine: the
`SimParams` struct, its feature-flag word, the force-channel registry, the layout
params, and the three host→GPU conversion paths. Also states plainly which analytics
kernels are known-broken so nobody trusts their output.

## Current state

### SimParams — the dual struct (212 bytes, statically asserted both sides)

The GPU sees one flat `#[repr(C)]` record. It is declared twice and both declarations
are size-locked to **212 bytes**:

- Rust: `src/models/simulation_params.rs:26-115` (`struct SimParams`), asserted at
  `src/models/simulation_params.rs:228` — `assert!(size_of::<SimParams>() == 212)`.
- CUDA: `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu:21` (`struct SimParams`),
  asserted at `:117` — `static_assert(sizeof(SimParams) == 212, "SimParams size mismatch with Rust")`.

The two `assert`s are the ABI guard: a field added on one side without the other
fails to compile. The Rust module header comment still says "180-byte" — that comment
is stale; the binding assertion (212) is authoritative. Legacy ADR-031 ("48B") and
ADR-061 are stale on this too.

Layout discipline: every field added since the original prefix is appended at the
**tail** to preserve the `repr(C)` prefix that four shipping clients already speak
(see field comments at `simulation_params.rs:78-114`). The tail additions are the
FA2/LinLog block (`lin_log_mode`, `scaling_ratio`, `adaptive_speed`, `global_speed`),
the DAG block (`dag_bias_k`, `dag_level_distance`), and the ADR-141 constrained-layout
block (`layout_mode`, `plane_bias_k`, `plane_spacing`, `radial_center_{x,y,z}`,
`layer_bias_k`, `layer_spacing`). The CUDA struct mirrors this field-for-field
(`visionclaw_unified.cu:2-79+`). `bytemuck::Pod`/`Zeroable`, `cudarc::DeviceRepr`,
and `cust_core::DeviceCopy` are all derived/impl'd on the Rust side
(`simulation_params.rs:9-11, 118-119`) so the struct transfers to device memory by
raw copy — there is no serialisation layer, so byte layout *is* the contract.

### Feature flags (bitfield in `feature_flags: u32`)

Defined in `crates/visionclaw-domain/src/models/simulation_params.rs:113-119`:

| Bit | Flag | Gates |
|-----|------|-------|
| `1<<0` | `ENABLE_REPULSION` | pairwise repulsion / FA2 |
| `1<<1` | `ENABLE_SPRINGS` | edge springs |
| `1<<2` | `ENABLE_CENTERING` | centre gravity |
| `1<<3` | `ENABLE_TEMPORAL_COHERENCE` | (declared; not a live force channel) |
| `1<<4` | `ENABLE_CONSTRAINTS` | **keystone** — ontology constraint loop |
| `1<<5` | `ENABLE_STRESS_MAJORIZATION` | (CPU-side; not a GPU force term) |
| `1<<6` | `ENABLE_SSSP_SPRING_ADJUST` | SSSP-weighted rest lengths |

**ENABLE_CONSTRAINTS is the keystone.** Without bit 4 set, the constraint loop at
`visionclaw_unified.cu:475` never runs and the uploaded ontology `ConstraintData`
buffer has zero effect. It is **not** derived from user settings — it is rebuilt from
constraint *residency* on every physics step: `execution.rs:903-904` sets the bit
whenever `self.num_constraints > 0`. This is the ADR-098 fix that made ontology
constraints live (audit: constraints FIXED and live). The force-channel registry
treats `Constraints` as read-only for exactly this reason (see below).

### Force-channel registry (`src/models/force_channels.rs`)

A bounded enum mapping each live kernel force term to its backing scalar(s) and flag
bit. It is a **mapping layer over the flat struct**, not a new representation — it
changes no struct layout, no kernel, no wire (`force_channels.rs:14-22`). Ten channels
(`ForceChannel::ALL`, `:94-105`): Repulsion, Separation, Spring, Centering, Gravity,
ClusterCohesion, Constraints, DagRadialBias, Annealing, Boundary.

- Flag-gated channels (`feature_flag()`, `:127-140`): Repulsion, Spring, Centering,
  Constraints. The rest are gated purely by `strength > 0` — the same test the kernels
  apply (`state()`, `:147-154`).
- Backing scalars (`strength_of`, `:157-170`): e.g. Spring→`spring_k`,
  Separation→`separation_radius`, DagRadialBias→`dag_bias_k`,
  Constraints→`constraint_max_force_per_node` (a per-node ramp CAP, not an on/off strength).
- `Constraints` is the only read-only channel (`is_read_only`, `:210-212`): `apply()`
  is a no-op for it (`:191-193`) because its flag is residency-owned and rebuilt every
  step, and zeroing its ramp cap would silently change force semantics. This is
  enforced by test (`constraints_apply_is_a_noop`, `:380`).

### Layout params — DAG-rank / radial

`layout_mode: u32` is the GPU-visible discriminant of `LayoutMode`
(`0=ForceDirected … 5=Clustered`, `simulation_params.rs:92-96`). The radial/DAG term
springs each ranked node onto a shell of radius `rank * dag_level_distance` centred on
`radial_center_{x,y,z}` (default origin = legacy DAG behaviour); `dag_bias_k = 0`
disables it. Sugiyama Y-by-rank uses `layer_bias_k`/`layer_spacing`; stratified planes
use `plane_bias_k`/`plane_spacing` (all ADR-141 tail fields). The HTTP surface is
`POST /api/layout/radial` with `{mode: dagRank|typeTier|ego, focusNode, transitionMs}`
(`layout_handler.rs:135-189`), dispatched to the GPU actor via `SetRadialLayout`. The
`hierarchical` edge label maps to DAG-rank detection (commit 73540faa0;
`layout_handler.rs:10,217`).

### The three host→GPU conversion paths (must stay consistent)

All three build the same 212-byte struct; the flag-derivation logic is duplicated and
**must not diverge**:

1. `From<&SimulationParams> for SimParams` — `simulation_params.rs:236-317`. Derives
   repulsion/spring/centering/SSSP flags from positive scalars (`:239-250`).
2. `From<&PhysicsSettings> for SimParams` — `simulation_params.rs:319-407`. Same flag
   derivation (`:322-332`) but always sets `ENABLE_SSSP_SPRING_ADJUST` and defaults
   `layout_mode`/`radial_center` to inert values so this wire path never silently
   forces a mode or resets an active radial centre.
3. `execution.rs:882-913` (`execute_physics_step`) — rebuilds `feature_flags` from
   scratch "mirroring `to_sim_params()`", then calls `params.to_sim_params()` (path 1)
   and **overwrites** `sim_params.feature_flags` with the freshly built word
   (`:912-913`). This is the path that adds the residency-driven `ENABLE_CONSTRAINTS`
   bit (`:903-904`) and honours the runtime `sssp_spring_adjust_enabled` toggle
   (`:895`). The live actor call site (the path-3 `execute_physics_step_with_bypass`
   invocation) is `force_compute_actor.rs:2057`.

Because path 3 recomputes the flags itself rather than trusting paths 1/2, the three
must agree on the derivation rule (`scalar > 0.0`) or a setting change will reach the
GPU on one path and not another (the historical "nothing moves when I change settings"
bug, noted at `execution.rs:907-911`).

### PTX ISA downgrade hack (`crates/visionclaw-gpu/build.rs`)

CUDA toolkit 13.x emits `.version 9.x` PTX; some host drivers only support 9.0. The
build post-processes every compiled `.ptx`: it finds `.version 9.` and rewrites it to
`.version 9.0` in place (`build.rs:162-179`). If `nvcc` is unavailable it falls back to
pre-compiled PTX bundled in the crate / `/app` image (`build.rs:137-160`). Empty or
missing PTX panics the build (`build.rs:181-187`).

### Known-BROKEN kernels — do not trust their output

Per legacy ADR-031 (GPU analytics correctness) and ADR-072, verified still-Partial in
this audit. These kernels compile and run but produce wrong results:

- **Louvain community detection** — `sigma_tot` race; modularity converges to ~0.
- **DBSCAN** — all border points misclassified as noise.
- **PageRank** (`crates/visionclaw-gpu/src/cuda_sources/pagerank.cu`) — per-block
  dangling-mass kernel is bound wrong; scores are unreliable.
- **Node embeddings** (ADR-072) — "effectively random noise": a hash bag-of-characters,
  not learned features. Do not feed downstream similarity/clustering.
- **LOF** (local outlier factor) — fixed and trustworthy.
- **Ontology constraints** (ADR-098) — fixed and live (keystone wiring above).

## Known divergences & open items

- **Stale in-code comment**: `force_channels.rs:5` and the header say `SimParams` is
  "180-byte"; the real, asserted size is **212**. Comment lags the struct.
- **Legacy ADR wire sizes are stale**: ADR-031 ("48B"), ADR-061 ("28B forever") predate
  the 212-byte struct. Code wins.
- **Duplicated flag derivation across three paths** is a latent divergence risk: no
  single shared helper builds `feature_flags`; `execution.rs` re-implements it. A future
  refactor should collapse the rule into one function.
- **`ENABLE_TEMPORAL_COHERENCE` (bit 3) and `ENABLE_STRESS_MAJORIZATION` (bit 5)** are
  declared but are not live GPU force channels (stress majorization is CPU-side,
  `simulation_params.rs:77`). Reserved bits, not wired terms.
- **Broken analytics kernels ship enabled**: Louvain/DBSCAN/PageRank/embeddings emit
  values that look valid. No runtime gate marks them untrusted; consumers must know.
- **Registry cannot toggle Constraints**: enablement is residency-owned. Any UI that
  exposes a "constraints on/off" switch through the force-channel registry is inert by
  design (`is_read_only`).

## Invariants (must not silently change)

1. `size_of::<SimParams>() == 212` on both sides; the two static assertions stay in
   lockstep. New fields append at the tail only.
2. `ENABLE_CONSTRAINTS` is set iff `num_constraints > 0` at physics-step time — never
   derived from user settings (`execution.rs:903-904`).
3. All three conversion paths derive repulsion/spring/centering flags from the same
   `scalar > 0.0` rule; `execution.rs` remains the authority that adds constraint
   residency and runtime SSSP toggle.
4. `Constraints` is the only read-only force channel; `apply()` never mutates its scalar
   or flag.
5. PTX is downgraded to `.version 9.0` before load; the fallback-PTX path exists for
   `nvcc`-less builds.
6. Field order and `repr(C)` prefix are the wire contract for existing clients — do not
   reorder.

## Change process

This is a living document. Any change to the `SimParams` layout, the feature-flag word,
the force-channel set, or a conversion path is an ABI change: update both struct
declarations, bump both size assertions if the size moves, update this doc's Current
State and Invariants, and bump `version`. Fixing a broken analytics kernel moves it out
of the Known-BROKEN list with a code citation. Cite legacy ADRs (e.g. legacy ADR-031,
ADR-098, ADR-141) as evidence only; live code is ground truth.
