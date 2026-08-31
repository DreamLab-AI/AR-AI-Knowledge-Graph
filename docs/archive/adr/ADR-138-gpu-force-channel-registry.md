# ADR-138: GPU Force-Channel Registry — Mapping Layer Now, Array-Backed SimParams Later

**Status:** Accepted
**Date:** 2026-08-30
**Deciders:** VisionClaw CUDA/physics specialist, VisionClaw platform team
**Related:**
- ADR-090 (hexagonal modularisation — `visionclaw-domain` owns `SimulationParams`/`FeatureFlags`; the GPU-aligned `SimParams` stays in `visionclaw-server`)
- ADR-098 (ontology constraints drive the live `force_pass_kernel` loop — the `ENABLE_CONSTRAINTS` gate)
- ADR-070 (persona masking / sparse compute mask — the `compute_mask` kernel arguments the force channels coexist with)
- `docs/reference/binary-protocol.md` and the `/api/settings/physics` wire (the camelCase settings surface four clients speak)

## TL;DR

The layout engine's force terms are configured through a flat, `repr(C)`,
180-byte `SimParams` struct whose fields are ad-hoc scalars (`repel_k`,
`spring_k`, `center_gravity_k`, …) plus a `feature_flags` bitset. As force terms
accumulate — repulsion, springs, centering, separation, gravity, cluster
cohesion, ontology constraints, the new DAG radial bias — the flat struct offers
no enumerable "what channels exist, are they on, how strong are they" view, and
nothing forces a new kernel term to be registered anywhere.

The eventual architecture is a **named force-channel registry**: an
enum-indexed array of `{enabled, strength}` channels replacing the ad-hoc
scalars. This ADR records that turning `SimParams` into that array **in one pass
is unsafe** (three coupled surfaces move at once), and adopts a two-step plan:

1. **Now (shipped): the mapping layer.** A bounded `ForceChannel` enum
   (`src/models/force_channels.rs`) that maps each channel to the *existing*
   scalar field(s) and feature-flag bit(s), giving the codebase one enumerable
   registry with **zero** change to the struct layout, the CUDA kernels, or the
   settings wire. This is the migration seam.
2. **Later (proposed): the array-backed refactor.** A single coordinated pass
   that changes the `SimParams` layout, every kernel that reads `c_params.*`, and
   the wire→channel mapping together, with the four clients moved in lockstep.

Only the bodies of `ForceChannel::state`/`ForceChannel::apply` change when step 2
lands; every caller of the registry keeps working.

## Context

### The force terms today (audit, 2026-08-30)

Terms evaluated in the two live force kernels
(`force_pass_kernel` and `force_pass_with_stability_kernel`, selected by
`stability_threshold > 0`):

| Force term | Backing scalar(s) | Gate |
|---|---|---|
| Repulsion (inverse-square / FA2 degree-scaled) | `repel_k`, `scaling_ratio` | `ENABLE_REPULSION` |
| Separation (short-range hard push) | `separation_radius`, `max_force` | `separation_radius > 0` |
| Springs (Hooke / LinLog) | `spring_k`, `rest_length`, `spring_scale`, `sssp_alpha` | `ENABLE_SPRINGS` |
| Centering (pull to origin) | `center_gravity_k` | `ENABLE_CENTERING` |
| Constraints (ontology) | `ConstraintData`, `constraint_max_force_per_node` | `ENABLE_CONSTRAINTS` |
| DAG radial bias (ADR context below) | `dag_bias_k`, `dag_level_distance` | `dag_bias_k > 0` |

Terms in `integrate_pass_kernel`:

| Force term | Backing scalar(s) | Gate |
|---|---|---|
| Boundary (soft containment) | `viewport_bounds`, `boundary_damping` | `viewport_bounds > 0` |
| Annealing (velocity jitter) | `temperature`, `cooling_rate` | `temperature > 0` |

Terms in dedicated kernels:

| Force term | Kernel | Backing scalar |
|---|---|---|
| Gravity (degree-weighted) | `degree_weighted_gravity_kernel` | `gravity` |
| Cluster cohesion | `cluster_cohesion_kernel` | `cluster_strength` |

That is **ten** distinct, individually togglable force channels, expressed today
as loose scalars with two different gating conventions (a `FeatureFlags` bit for
four of them; a `strength > 0` test for the rest).

### Why the one-pass refactor is unsafe

`SimParams` sits on three tightly coupled surfaces:

1. **Byte-exact FFI.** `SimParams` is `repr(C)` and mirrored field-for-field by
   the CUDA `SimParams` in `visionclaw_unified.cu`. Both sides carry a size
   assertion (`static_assert(sizeof(SimParams) == 180)` in CUDA;
   `const _: () = assert!(size_of::<SimParams>() == 180)` in Rust). Reshaping the
   struct into an array changes the layout, so **every** `c_params.<scalar>` read
   across both force kernels, the integrate kernel, the gravity kernel, and the
   cohesion kernel must change in the same commit or the layout assertion (and
   the physics) breaks.
2. **The settings wire.** The scalar field names are camelCase settings keys on
   `/api/settings/physics`, spoken by **four shipping clients**. Wire
   compatibility is absolute, so an array-backed struct needs a stable
   field-name → channel-index mapping and coordinated client updates — not a
   change one engineer can land safely in isolation.
3. **Conversion fan-out.** `SimParams` is produced from `PhysicsSettings` and
   `SimulationParams` via `From` impls that also derive `feature_flags` from the
   scalars (`repel_k > 0 ⇒ ENABLE_REPULSION`, etc.). The array form must preserve
   that derivation exactly.

Moving all three at once, across a public wire, is precisely the kind of
change that wants its own risk-bounded pass with client coordination — not a
by-product of adding a force term.

## Decision

**Ship the mapping layer now; defer the array-backed struct to a coordinated
pass.**

### The mapping layer (shipped)

`src/models/force_channels.rs` introduces:

- `enum ForceChannel` — a **bounded** set of ten variants, one per force term in
  the audit (`Repulsion`, `Separation`, `Spring`, `Centering`, `Gravity`,
  `ClusterCohesion`, `Constraints`, `DagRadialBias`, `Annealing`, `Boundary`).
- `struct ForceChannelState { enabled: bool, strength: f32 }` — the per-channel
  view the future array will store natively.
- `ForceChannel::state(&SimParams) -> ForceChannelState` and
  `ForceChannel::apply(&mut SimParams, ForceChannelState)` — the *only* place
  that knows which scalar and which flag bit back each channel. `state` derives
  `enabled` from the `FeatureFlags` bit for flag-gated channels and from
  `strength > 0` for the rest; `apply` writes the scalar and, for flag-gated
  channels, sets the bit iff `enabled && strength > 0` (mirroring the existing
  `From<&SimulationParams>` flag derivation) or zeroes the scalar and clears the
  bit on disable.
- `ForceChannel::ALL`, `::key()`, `::feature_flag()`, and `snapshot(&SimParams)`
  — an enumerable registry surface for diagnostics and future consumers.

The exhaustive `match`es mean adding a kernel force term forces the author to add
a `ForceChannel` variant, so the registry cannot silently drift from the kernels.

This layer changes **nothing** on the three coupled surfaces: no struct layout
change, no kernel change, no wire change. It is additive and independently
testable.

### One noted semantic (guarded by test)

`Constraints` is flag-gated but, unlike the other gated channels, its
`ENABLE_CONSTRAINTS` bit is set at runtime (when constraints are resident,
ADR-098), not derived from its scalar. So at rest it reads `enabled = false`
while its backing scalar (`constraint_max_force_per_node`) is positive. The
mapping layer's round-trip invariant is therefore defined on **effective** state
(a disabled channel's stored strength is immaterial, and `apply` normalises it to
zero), not byte-identity. A regression test pins this behaviour.

## The coordinated-pass plan (proposed, future)

When the array-backed refactor is scheduled, it should land as one coordinated
change:

1. **Struct.** Replace the per-force scalars in `SimParams` with a fixed-size
   `channels: [GpuForceChannel; N]` (each `{f32 strength, u32 enabled}` or a
   packed equivalent), keeping any non-channel fields (`dt`, `seed`, grid/LOD
   constants) as-is. Recompute both size assertions.
2. **Kernels.** Replace every `c_params.<scalar>` read with
   `c_params.channels[CHANNEL_INDEX].strength` (and the `.enabled`/flag test),
   across both force kernels, the integrate kernel, the gravity kernel, and the
   cohesion kernel — in the same commit as the struct change.
3. **Wire mapping.** Keep the camelCase settings keys exactly as they are; map
   each key to its channel index in the `From<PhysicsSettings>`/`From<SimulationParams>`
   conversions. The mapping layer shipped here (`ForceChannel` + `state`/`apply`)
   is the seam: its bodies switch from touching scalars to touching array slots,
   and no caller changes.
4. **Clients.** Coordinate the four clients. Because the wire keys are unchanged,
   this should be a no-op for them; the pass exists to prove that and to add any
   new registry-shaped surface (e.g. a channels diagnostics endpoint) deliberately
   rather than by accident.

Sequencing steps 1–2 together is mandatory (the FFI assertion couples them);
step 3 rides the same commit; step 4 is verification.

## Consequences

**Positive**
- One enumerable, bounded source of truth for the force channels, available now,
  at zero risk to the FFI or the wire.
- New kernel force terms can no longer be added without registering a channel
  (exhaustive matches).
- The future array refactor has a defined migration seam and a written plan, so
  it can be scheduled as a bounded pass rather than an open-ended rewrite.

**Negative / trade-offs**
- Two representations coexist until the refactor: the flat scalars (authoritative,
  on the wire and in the kernels) and the `ForceChannel` view (derived). The
  mapping layer is the single reconciliation point, but it is still a layer to
  keep in sync when scalars are added.
- The array refactor's benefit is deferred; today's win is organisational
  (enumerability, drift-resistance), not a runtime or layout change.

## Context: the layout features this registry now enumerates

Two force channels in the audit above were added in the same body of work that
produced this ADR, and are the immediate motivation for wanting an enumerable
registry:

- **Pinned-node support.** A per-node pinned mask on the GPU: pinned nodes skip
  position/velocity integration (held where the client placed them) but still
  exert repulsion and spring forces on their neighbours, because the force pass
  reads every node's position regardless of pin state. This backs the VR
  grab-and-place interaction (drag-end pins in place until an explicit
  `nodeUnpin`). It is an integrator exception rather than a force term, so it is
  not itself a `ForceChannel`, but it shares the same "bounded set of GPU
  behaviours" motivation.
- **DAG radial hierarchy bias (`DagRadialBias`).** A radialout force pulling each
  ranked node onto a shell of radius `rank × dagLevelDistance` around the
  hierarchy root (approximated by the world origin, where centering already
  gathers the roots). Ranks are computed CPU-side by a cycle-safe multi-source
  BFS over edges with explicit class-subsumption provenance only (`is_subclass_of`
  / `subclass_of` / `SUBCLASS_OF`) — the symmetric relations `equivalent_class` /
  `same_as`, the separate `sub_property_of`, and the generic `hierarchical` label
  (which GitHub domain-membership edges reuse), all of which `SemanticEdgeType::
  Hierarchical` folds together, are excluded so they cannot fabricate false roots
  (roots = nodes never a child; unreachable/non-hierarchy nodes get no bias). The
  ranks are uploaded to a per-node rank buffer. Gated off by default
  (`dagBiasK = 0`). This is the tenth force channel,
  and adding it is what made the absence of a registry visible.
