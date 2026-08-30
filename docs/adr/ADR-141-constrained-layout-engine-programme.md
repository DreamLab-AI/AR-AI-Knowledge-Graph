# ADR-141: Constrained-Layout Engine Programme — 13-Pattern Taxonomy → Phased GPU + Estate Upgrade

**Status:** Proposed
**Date:** 2026-08-30
**Deciders:** VisionClaw layout-engine lead (Opus), VisionClaw CUDA/physics specialist, XR + desktop client owners
**Related:**
- ADR-138 (GPU Force-Channel Registry — the mapping-layer-now/array-backed-later seam this programme both consumes and pressures toward step 2)
- ADR-098 (ontology constraints drive the live `force_pass_kernel` loop — `ENABLE_CONSTRAINTS`)
- ADR-090 (hexagonal modularisation — `visionclaw-domain` owns `SimulationParams`/`FeatureFlags`; GPU-aligned `SimParams` stays in `visionclaw-server`)
- ADR-031 (`LayoutMode` enum — exists in `SimulationParams`, currently dropped at the GPU boundary)
- ADR-137 (XR render offload + runtime quality dials), ADR-139 (immersive interaction adoption programme)
- `docs/reference/binary-protocol.md` and the `/api/settings/physics` + `/api/layout/mode` wire

## TL;DR

The user asked for the GPU settling engine to be audited against a 13-pattern
constrained-layout taxonomy and then upgraded across the estate (CUDA + settings
API + desktop menus + XR HUD). This ADR records the Phase 0 audit, maps each
taxonomy pattern to an implementation **method** (soft force channel vs hard
projection vs CPU one-shot placement), and adopts a **six-phase, each-phase-shippable**
programme. Each implementation phase updates this ADR's status line for its phase.

The audit's headline finding: the engine is **less capable than its code surface
suggests**. There is no force-channel registry in CUDA (forces are hard-coded and
duplicated across two kernels); the advertised "dual-disc projection" does not
exist in the `.cu`; and the full constraint taxonomy already exists as **dead
data structures** (`ConstraintKind` enumerates alignment/radial/layer/boundary/
clustering, but the live mapper only ever emits DISTANCE + SEPARATION). The
richest real layout code — per-type Z-stratification, DAG vertical levels — lives
in `semantic_forces.rs` on the **CPU path**, off the GPU hot loop.

## Context — Phase 0 audit (2026-08-30)

Deep-read: `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu` (2485 L),
`src/models/force_channels.rs` + ADR-138, `src/models/constraints.rs` +
`crates/visionclaw-domain/src/models/constraints.rs`, `src/physics/ontology_constraint_mapper.rs`,
`src/constraints/*`, `src/gpu/semantic_forces.rs` (1705 L),
`src/handlers/api_handler/ontology_physics/*`, `src/services/ontology_pipeline_service.rs`,
`src/services/inferred_edge_materialiser.rs`, both `SimParams` structs,
`crates/visionclaw-domain/src/types/physics_config.rs`, the desktop
`client/src/features/control-center/registry/groups/motion.ts`, and the XR
`xr-client/scripts/{plane_manager,hud,graph_scene,radial_menu,query_builder}.gd`.

### Three structural facts that shape the programme

1. **No force-channel registry in CUDA.** Forces accumulate into a single
   `total_force` inline in `force_pass_kernel` (`visionclaw_unified.cu:301-693`),
   gated by a `FeatureFlags` bitmask, and the block is **duplicated** in
   `force_pass_with_stability_kernel` (`:2069-2353`). Post-passes:
   `cluster_cohesion_kernel` (`:741`), `degree_weighted_gravity_kernel` (`:2436`),
   `blend_semantic_physics_forces` (`:1809`). ADR-138's `ForceChannel` registry is
   a **mapping layer onto existing scalars only** — it adds nothing to `SimParams`,
   the GPU buffer, or the kernels. Consequence: every new force term = edit **both**
   kernels + append `SimParams` fields + bump **both** 180-byte `static_assert`s.

2. **"Dual-disc projection" is not in the kernel.** grep for disc/plane/projection
   returns nothing. `PhysicsSettings.enable_dual_disc_layout`, `graph_separation_x`,
   `axis_compression_z` are CPU-side/aux params with no GPU `SimParams` slot. The
   only hard-position op in the kernel is the pinned-node **hold** (`:809-830`) — a
   copy-through, not a projection. There is **no general hard-projection primitive
   on the GPU**; the taxonomy's "hard projection" method is effectively absent.

3. **The taxonomy already exists as dead data.** Domain `ConstraintKind`
   (`constraints.rs:16-46`) enumerates FixedPosition, Separation, AlignmentH/V/Depth,
   Clustering, Boundary, DirectionalFlow, RadialDistance, LayerDepth, Semantic — but
   the **only live mapper** (`physics/ontology_constraint_mapper.rs`) emits DISTANCE(0)
   + SEPARATION(6) exclusively. `src/constraints/` (rich `PhysicsConstraintType` with
   HierarchicalLayer/Containment/priority bands) is **unwired dead code**;
   `src/handlers/api_handler/constraints/mod.rs` is **orphaned** (imports a
   `ConstraintType` enum that no longer exists). Three divergent axiom→constraint
   mappers exist; only one reaches the kernel. `semantic_forces.rs` holds the richest
   real layout logic (DAG vertical levels `:834`, type-clustering `:847`,
   physicality/role/maturity→Z `:1288`, cross-domain) but **mostly on the CPU path**.

### Audit table — taxonomy pattern → existing support

| # | Pattern | Support | Evidence (file:line) |
|---|---------|---------|----------------------|
| 1 | Hierarchical/layered Sugiyama (2D) | Partial | `semantic_forces` DAG vertical levels (`:834`, CPU); desktop `dag-topdown` option (`motion.ts:60`). No GPU Y-by-rank, no crossing reduction |
| 2 | Radial/concentric ego (2D) | Partial | `dag_radial_bias` shells by rank (`unified.cu:176-201`). No ego-distance-from-focus variant |
| 3 | Circular/ring — communities as arcs (2D) | None | community label-prop exists (`:1580`) but no angular/azimuthal placement force |
| 4 | Grid/matrix (2D) | None | grid exists only for spatial hashing (`build_grid_kernel:219`), never a layout target |
| 5 | Boundary/region — user zones (2D) | Partial | soft global box boundary (`:922-960`); `Boundary` kind declared but unproduced. No multi-zone / user-drawn |
| 6 | Alignment/relative placement (2D) | None live | `AlignmentH/V/Depth` in enum + `alignment_strength` `SimParams` field (offset 72) **both unused** by any kernel/mapper |
| 7 | Compound/nested groups (2D) | Partial | `cluster_cohesion_kernel` centroid pull (`:741`); `Containment`/`GROUP` inert/dead |
| 8 | Spherical/globe concentric (3D) | Partial | `dag_radial_bias` = spherical shells by rank; isolated-node peripheral shell (`:2464`). Not type/depth-selectable |
| 9 | Multi-layered stratified planes by type (3D) | Partial | `semantic_forces` maturity→Z / physicality / role (CPU, `:1288`); `plane_manager.gd` Y-strata but query-result presentation only. No GPU per-type Z-plane force |
| 10 | DAG-directional axis (3D) | Partial | `dag_radial_bias` radial in/out; desktop topdown/leftright options but backend axis variants thin; `axis_compression_z` CPU |
| 11 | Geo-constrained (3D) | None | — |
| 12 | Cylindrical/tubular (3D) | None | no angle+axis force term |
| 13 | Methods present | — | soft forces **dominant**; hard projection **≈none** (only pinned-hold); spectral-init **none** |

### Architecture rule — CPU projections vs GPU soft forces (recorded per Phase 0 steer)

The engine deliberately keeps **two** placement mechanisms and this ADR fixes the
boundary between them:

- **GPU soft forces** (`SimParams` scalars + the two force kernels) provide
  *continuous settling*: repulsion, springs, centering, `dag_radial_bias`, cluster
  cohesion, and the new per-mode terms P2–P4 add. These are the default method (SF).
- **CPU projections** (`force_compute_actor`, applied per broadcast frame) provide
  *deterministic post-integration reshaping*: the `enableDualDiscLayout` /
  `graph_separation_x` / `axis_compression_z` gates are computed host-side against
  the position buffer before broadcast — they are **not** in the `.cu`. The
  projection-free kernel is therefore correct: the "dual-disc projection" lives on
  the CPU, not the GPU. Any future **hard projection** (HP) primitive (crisp planes,
  shells, region walls) follows this same rule — implemented as a CPU per-broadcast
  position clamp in `force_compute_actor`, never as a kernel position write —
  because the kernel integrates velocities and must not fight a hard clamp mid-step.

So the method axis maps to a code location: **SF → CUDA kernel + `SimParams`;
HP → `force_compute_actor` per-broadcast projection; CPU one-shot → the layout
handler's `compute_layout` (returns positions the client applies once).**

### The three constraint mappers — decision (recorded per Phase 0 steer)

Phase 0 found three axiom→constraint mappers; only `physics/ontology_constraint_mapper.rs`
(the lean DISTANCE+SEPARATION mapper) reaches the kernel. Decision: **keep the lean
mapper as the live path.** Mark `src/constraints/` (the rich
`PhysicsConstraintType`/`OWLAxiomMapper` model) as the **target shape for the
ADR-138 array-backed `SimParams` refactor** — it is where HierarchicalLayer,
Containment, priority bands and the full alignment/radial/layer kinds already live,
so it becomes the design reference when P4/P5 land those as real force channels —
**unless** the P1 implementation finds it cheaper to revive the rich model directly
than to extend the lean one (it did not, for P1). The orphaned
`src/handlers/api_handler/constraints/mod.rs` (never mod-declared, referenced a
non-existent `ConstraintType`) is **deleted** as part of P1's cleanup lane.

## Decision

Adopt a phased constrained-layout programme. For each taxonomy pattern, bind an
**implementation method** — the three methods from the taxonomy's "application
methods" axis:

- **SF — soft constraint force channel**: a new per-node force term summed into
  `total_force`, following the `dag_radial_bias` pattern. Requires appending
  `SimParams` field(s) + bumping both `static_assert`s + editing both force kernels.
  Preferred default (continuous, composable, GPU-resident, damping-friendly).
- **HP — hard projection**: post-integration position clamp onto a target manifold
  (plane/disc/shell/box). New primitive — none exists today. Use only where crisp,
  non-negotiable placement is wanted (e.g. exact concentric spheres, region walls).
- **CPU — CPU one-shot placement / spectral-or-BFS init**: compute target positions
  or ranks once on the host (BFS ranks, spectral embedding, ring ordering, grid
  assignment), upload as a per-node target buffer that an SF term springs toward.
  Preferred for anything needing a global combinatorial pass (Sugiyama ranks,
  crossing reduction, ring angular order) that does not belong in a per-node kernel.

### Taxonomy → method mapping

| # | Pattern | Method | Notes |
|---|---------|--------|-------|
| 9 | Stratified planes by type (3D) | **SF** + per-type target-Z buffer | promote `semantic_forces` Z-logic onto the GPU hot loop |
| 8 | Spherical shells by depth/type (3D) | **SF (EXT)**; optional **HP** for crisp shells | generalise `dag_radial_bias` shell radius to key on type/ontology-depth |
| 1 | Sugiyama layered (2D) | **CPU** (BFS/spectral rank + crossing reduction) + **SF** (Y-by-rank spring) | ranks computed host-side, sprung on GPU |
| 2 | Radial/concentric ego (2D) | **SF (EXT)** | `dag_radial_bias` extended to BFS-distance from a focus node; pairs with click-to-focus (task #21) |
| 6 | Alignment/relative placement (2D) | **SF (WIRE)** | wire dormant `AlignmentH/V/Depth` kinds + existing unused `alignment_strength` field into a kernel branch |
| 5 | Boundary/region zones (2D) | **SF** (multi-zone soft walls) or **HP** (hard walls) | extend the soft box; add user-drawn zone geometry both clients |
| 7 | Compound/nested groups (2D) | **SF (EXT)** | extend `cluster_cohesion` toward a containment radius; revive `Containment` from `src/constraints/` |
| 10 | DAG-directional axis (3D) | **SF (EXT)** | make topdown/leftright real axis variants of `dag_radial_bias` |
| 3 | Circular/ring arcs (2D) | **CPU** (angular order) + **SF** (ring spring) | lower priority |
| 4 | Grid/matrix (2D) | **CPU** (cell assignment) + **SF** (snap spring) or **HP** | lower priority |
| 12 | Cylindrical/tubular (3D) | **SF** | angle+axis encode two dims; lower priority |
| 11 | Geo-constrained (3D) | **CPU** (lat/long → position) + **HP** | niche for this estate |

### Estate surfaces (all phases)

- **SimParams**: dual 180-byte `repr(C)` structs, **zero free space** (Rust
  `src/models/simulation_params.rs:26-91`, assert `:195`; CUDA `unified.cu:21-84`,
  `static_assert :86`). New params append to **both**, bump **both** asserts,
  byte-identical. `LayoutMode` (ADR-031) exists in `SimulationParams` but is dropped
  at the GPU boundary (`simulation_params.rs:165`) — the anchor to make layout-mode
  GPU-visible.
- **Wire**: settings ride `/api/settings/physics` (camelCase `PhysicsSettings`,
  `physics_config.rs:215-376`, generated TS `generate_types.rs:132`). Live
  layout-mode rides `POST /api/layout/mode` (`layoutApi.ts:52`). New per-pattern
  params extend `PhysicsSettings` + `PhysicsUpdate`; new modes extend the
  `LayoutMode` enum and `LAYOUT_SETTING_PATHS`.
- **Desktop menu**: picker already at `motion.ts:60` (`Semantic & Layout Forces`
  subgroup: `force-directed, dag-topdown, dag-radial, dag-leftright, type-clustering`)
  → extend `options` + add companion fields; add live paths to `LAYOUT_SETTING_PATHS`
  (`useSettingField.ts:27-30`).
- **XR HUD**: no layout-mode concept today. Add a cycling `_action_btn` (or radial
  sub-menu) in `_build_graph_page` (`hud.gd:284-330`), routed through
  `_on_hud_control` (`graph_scene.gd:678-716`) to `POST /api/layout/mode`. **Clients
  currently diverge** on the layout endpoint (desktop uses `/api/layout/mode`, XR
  only `/api/settings/physics` PUTs) — Phase 1 unifies XR onto `/api/layout/mode`.

## Phased plan (each phase shippable; EWQ + codex review per phase)

- **Phase 1 — Layout-mode plumbing + cleanup lane.** Make `LayoutMode` GPU-visible
  (append `layout_mode: u32` to both `SimParams`, bump both asserts 180→184), wire the
  existing dag-radial mode end-to-end (Radial ⇒ `dag_radial_bias` shell primed), unify
  XR + desktop on `POST /api/layout/mode`, and delete the orphaned constraints handler.
  Ships: layout modes reach the kernel; both clients share one endpoint. *Status: **in progress** (2026-08-30).*
- **Phase 2 — Stratified planes by type (P9).** New SF channel + per-type target-Z
  buffer; XR HUD picker + desktop option. *XR room-scale flagship. Status: not started.*
- **Phase 3 — Spherical shells + ego-radial (P8, P2).** Extend `dag_radial_bias`:
  type/depth-keyed shell radius; BFS-distance-from-focus mode. *Status: not started.*
- **Phase 4 — Sugiyama layered + ontology alignment (P1 + P6).** CPU BFS/spectral rank
  + optional crossing reduction → GPU Y-by-rank SF spring. **Folded in from P5 per the
  Phase 0 steer:** WIRE the dormant `AlignmentHorizontal/Vertical/Depth` constraint
  kinds + the already-allocated `alignment_strength` `SimParams` field (offset 72) into
  a kernel branch — low-cost, high-leverage, no UI needed for ontology-driven alignment.
  *Desktop flagship. Status: not started.*
- **Phase 5 (deferred) — User-drawn boundary/region zones (P5).** Multi-zone soft walls
  / hard projection + zone-draw UI on both clients. **Deferred to a separate programme**
  per the Phase 0 steer (needs new UI surfaces on both clients). *Status: deferred.*
- **Phase 6 (deferred) — Ring arcs / grid / cylindrical (P3, P4, P12).** Lower-value
  niche patterns. *Status: deferred.*

This programme is the natural forcing function for **ADR-138 step 2** (array-backed
`SimParams`): each SF phase appends per-channel scalars and grows the struct, so the
array-backed refactor should be bundled once the field count makes the flat struct
unwieldy (P2–P4 will pressure this).

## Consequences

- **Positive**: a coherent, enumerable layout-mode surface across both clients;
  the CPU-only `semantic_forces` layout logic finally reaches the GPU hot loop;
  dormant constraint kinds become live; the estate gains a documented method-per-pattern
  contract for future patterns.
- **Negative / risks**: each SF term grows the 180-byte `SimParams` and touches two
  duplicated kernels — high-friction until ADR-138 step 2 lands (this programme should
  bundle that refactor rather than fight it). The PTX-downgrade in `build.rs` and the
  dual `static_assert`s are the two tripwires per phase (per the CUDA skill).
- **Agentbox contract check**: this programme is host-project (VisionClaw) layout
  engine + clients only. It does **not** touch agent/runtime contracts owned by
  agentbox (adapters, URN grammar, interaction plane, memory). **No agentbox ADR
  counterpart is required.** If a later phase surfaces agent-swarm layout that binds
  to agentbox session/identity contracts (cf. task #22), flag it then for an agentbox
  ADR-suite counterpart.

## Status log

- 2026-08-30 — Proposed. Phase 0 audit complete; programme scoped. Awaiting phase-order approval.
- 2026-08-30 — Phase 0 approved. Scope set to P1→P4; P5 (user-drawn zones) + P6 deferred;
  ontology-driven alignment (P6/alignment) folded into P4. Architecture rule (CPU
  projection vs GPU soft force) and the lean-mapper decision recorded above.
- 2026-08-30 — **Phase 1 in progress.** `SimParams.layout_mode: u32` appended to both the
  Rust (`src/models/simulation_params.rs`) and CUDA (`visionclaw_unified.cu`) structs,
  both static_asserts bumped 180→184; `LayoutMode::{as_gpu_u32,from_gpu_u32,is_gpu_resident}`
  added (`crates/visionclaw-domain/src/types/layout.rs`); all three `From→SimParams`
  conversions + the reverse conversion carry the mode; `SetLayoutMode` given a real
  handler (`force_compute_actor.rs`) that reuses the `UpdateSimulationParams` resync/reheat
  path and primes `dag_bias_k` for Radial; `POST /api/layout/mode` now persists the mode
  GPU-side (`layout_handler.rs`); orphaned `api_handler/constraints/mod.rs` deleted; both
  clients unified on `/api/layout/mode` (desktop option strings corrected, XR HUD picker
  added).
