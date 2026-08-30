# ADR-141: Constrained-Layout Engine Programme — 13-Pattern Taxonomy → Phased GPU + Estate Upgrade

**Status:** Accepted (P1–P4 complete; P5/P6 deferred)
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
  buffer; XR HUD picker + desktop option. *XR room-scale flagship. Status: **in progress** (2026-08-30).*
- **Phase 3 — Spherical shells + ego-radial (P8, P2).** Extend `dag_radial_bias`:
  type/depth-keyed shell radius; BFS-distance-from-focus mode. *Status: **complete** (2026-08-30).*
- **Phase 4 — Sugiyama layered + ontology alignment (P1 + P6).** CPU BFS/spectral rank
  + optional crossing reduction → GPU Y-by-rank SF spring. **Folded in from P5 per the
  Phase 0 steer:** WIRE the dormant `AlignmentHorizontal/Vertical/Depth` constraint
  kinds + the already-allocated `alignment_strength` `SimParams` field into a kernel
  branch — low-cost, high-leverage, no UI needed for ontology-driven alignment.
  *Desktop flagship. Status: **complete** (2026-08-30).*
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

### Phase 2 design note — plane assignment

The stratified-plane term is a soft force channel (SF): a per-node `node_plane`
buffer (centred float plane offset, `NaN` = unassigned) drives a Z-only Hooke spring
`force.z += (node_plane[idx] * plane_spacing − pos.z) * plane_bias_k`, gated on
`plane_bias_k > 0`. It mirrors the `node_rank`/`dag_radial_bias` machinery exactly
(buffer alloc/grow/upload, appended as the last kernel arg after `node_rank` in both
force kernels, added to `total_force` after `dag_radial_bias`). It **promotes the
CPU `semantic_forces` Z-stratification onto the GPU hot loop** (Phase 0 finding: that
logic existed only on the CPU path). The X/Y placement stays owned by the other forces
— only Z is touched — so planes compose with any layout mode.

Plane assignment maps node population → centred offset: **Knowledge → −1, Ontology →
0 (central schema plane), Agent → +1**. This deliberately differs from the separate
dual-disc CPU projection's Z-ordering (which centres Agents); the two are distinct
layouts and need not agree. Extending the offset to finer ontology depth/type is a
later refinement. `plane_bias_k` (default 0 = off) and `plane_spacing` (default 60)
are new `PhysicsSettings` fields on the standard `/api/settings/physics` wire; desktop
gets two sliders in the "Layout Forces" subgroup, XR gets a Planes on/off toggle +
gap ± buttons in the HUD Graph tab.

### Phase 2 status log

- 2026-08-30 — **Phase 2 in progress.** Stratified-plane SF channel implemented:
  `plane_bias_k`/`plane_spacing` appended to both `SimParams` structs (184→192, both
  static_asserts); per-node `node_plane` buffer (alloc/grow/upload mirror `node_rank`,
  NaN sentinel); `stratified_plane_bias` device fn + both kernel signatures + both
  launch sites; CPU plane assignment from `node_population` on graph upload; new
  `PhysicsSettings` wire fields; desktop sliders + XR HUD Planes toggle/gap buttons.
  Gated `cargo check --features gpu,ontology,dev-auth` (nvcc) exit 0 — the 192-byte
  struct + new kernel term + launch arg order all validated.
- 2026-08-30 — Phase 2 codex correctness pass: added a `plane_spacing <= 0 ⇒ inert`
  guard to the device fn so a zero/omitted spacing can never collapse all planes onto
  z=0 (the settings DTO defaults spacing to 0 as "absent", mirroring `dag_level_distance`;
  the kernel guard makes that self-safe regardless of wire path). Second codex note (a
  `node_population` length-mismatch leaving the buffer stale) matches `node_rank`'s
  established behaviour exactly — the buffer is NaN-reset on every grow, so a skipped
  upload leaves it inert, not stale; left at parity by design. Re-verified nvcc-green.

### Phase 3 design note — one term, three keyings, settable centre

`dag_radial_bias` is generalised rather than duplicated. The shell radius stays
`node_rank[idx] * dag_level_distance`; the two things that vary per mode are (1) what
fills the `node_rank` key buffer and (2) the shell **centre**, newly a settable
`radial_center` (`SimParams` 192→204, both structs + both static_asserts; default
`(0,0,0)` = byte-identical legacy behaviour). The kernel change is one line —
`delta = my_pos − center` — so a single term now serves DAG-depth spheres, type-tier
spheres, and focus-centred ego shells.

**Three radial modes** (`RadialMode`, actor-side; no new GPU buffer): `DagRank` (cached
subclass-BFS ranks, origin — the "concentric spheres by depth" P8 case), `TypeTier`
(population tier Agent→0 / Knowledge→1 / Ontology→2, origin — spheres by type), `Ego`
(BFS hop-distance from a focus node, centred on that node's live position — P2 ego).
Modes re-fill `node_rank` and set `radial_center` on demand; the actor caches
`dag_ranks`, an undirected `graph_adjacency`, and a node-id→index map at graph upload
to make re-keying cheap.

**Focus-setter design (justified):** a dedicated actor message `SetRadialLayout { mode,
focus_node }` + `POST /api/layout/radial`, **not** a settings field. Rationale: the
focus is ephemeral interaction state (the node the user last targeted), and applying it
requires a CPU recompute (BFS / key rebuild), a live position read, and a buffer upload
— an actor operation, not a serialisable scalar. It mirrors the `SetLayoutMode`
precedent and pairs with click-to-focus (task #21). `radial_center` is actor-authoritative
and preserved across settings PUTs (same discipline as `layout_mode`), so a physics-slider
change never resets the ego centre. A forced reheat on every re-key ensures the new
shells engage even at deep equilibrium (the key upload is invisible to the
`UpdateSimulationParams` idempotency guard).

XR HUD gains a "Radial Shells" group (DAG / Type / Ego-focus / Off; ego reuses the last
radial-targeted node `_radial_node_id`). Desktop radial is deferred as **XR-only** for
now: ego requires a focus node a static desktop `select` cannot supply, and wiring a new
endpoint into the settings-field machinery would be fragile — desktop retains the P1
`radial` layout mode + P2 plane sliders.

### Phase 4 design note — Sugiyama layering + ontology alignment

**Sugiyama (P4a)** reuses the already-computed `node_rank` (subclass-BFS depth) as the
layer index: a new SF term `sugiyama_layer_bias` springs each node's **Y** toward
`rank × layer_spacing` (SimParams `layer_bias_k`/`layer_spacing`, 204→212, both
static_asserts; Y-only, no new buffer or kernel arg — `node_rank` is already passed).
The existing repulsion/springs spread nodes within a layer and pull edge-connected
nodes together across layers, so crossing reduction is emergent rather than a separate
CPU pass — the "CPU BFS init" is the `node_rank` BFS itself (already run at graph
upload). `LayoutMode::Hierarchical` becomes GPU-resident: `SetLayoutMode(Hierarchical)`
primes `layer_bias_k`; every other mode clears it (mirrors the Radial/`dag_bias_k`
discipline). Desktop gets `layerBiasK`/`layerSpacing` sliders (the desktop flagship);
XR drives it through the existing layout-mode picker.

**Ontology alignment (P4b)** makes the dormant alignment kinds functional. A new kernel
constraint branch `ALIGNMENT = 7` (added to **both** force kernels' constraint loops)
pulls the constrained nodes toward their shared mean coordinate on one axis
(`params[0]` = 0/1/2 = X/Y/Z), scaled by the now-live `alignment_strength` (re-wired
from `PhysicsSettings.alignmentStrength`, default 0 = inert; the field was already in
both 180-byte-prefix structs, so **no size change**). The domain
`AlignmentHorizontal/Vertical/Depth` kinds map to `ALIGNMENT` with the axis in
`params[0]`. Ontology-driven emission: the live `ontology_constraint_mapper` groups
`subClassOf` children by shared parent and emits chained pairwise Y-alignment among
siblings (capped), so sibling classes settle onto the same Sugiyama layer — directly
complementing P4a. This runs under the existing `ENABLE_CONSTRAINTS` gate and self-gates
on `alignment_strength > 0`.

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
- 2026-08-30 — Phase 1 codex correctness pass (per-phase gate): fixed three defects before
  landing. (1) `layout_mode` is now **actor-authoritative** — `UpdateSimulationParams`
  preserves `self`'s mode so a physics-settings PUT (which rebuilds `SimulationParams`
  from `PhysicsSettings` with a default mode) no longer silently resets an active
  Radial/Clustered mode; only `SetLayoutMode` changes it. (2) every non-Radial mode now
  clears `dag_bias_k` so a Radial→Clustered/CPU switch stops the radial-shell term.
  (3) `POST /api/layout/mode` returns `success:false` on actor-unavailable/reject instead
  of a hollow success. Re-verified compile-green.
- 2026-08-30 — **Phase 2 complete** (stratified planes) and **Phase 3 complete** (spherical
  shells + ego-radial). See the Phase 2 / Phase 3 design notes and the Phase 2 status-log
  subsection above. P3: `dag_radial_bias` centre generalised to `radial_center` (SimParams
  192→204, both static_asserts; kernel `delta = my_pos − center`, origin default = legacy);
  three `RadialMode`s (DagRank/TypeTier/Ego-BFS) re-key `node_rank` on demand; actor caches
  `dag_ranks`/`graph_adjacency`/id-index at upload; `SetRadialLayout` message +
  `POST /api/layout/radial`; `radial_center` actor-authoritative + preserved across settings
  PUTs; forced reheat on re-key; XR HUD "Radial Shells" group (ego reuses `_radial_node_id`),
  desktop deferred XR-only (justified). Gated `cargo check --features gpu,ontology,dev-auth`
  (nvcc) exit 0 — 204-byte struct + generalised term validated. Codex per-phase pass applied.
- 2026-08-30 — **Phase 4 complete — programme build phases (P1–P4) DONE.** (A) Sugiyama
  Y-by-rank SF term `sugiyama_layer_bias` (SimParams 204→212, both static_asserts; reuses
  `node_rank`, Y-only, added to both force kernels); `LayoutMode::Hierarchical` now
  GPU-resident and auto-primes `layer_bias_k`; every other mode clears it; desktop
  `layerBiasK`/`layerSpacing` sliders, XR via the layout picker. (B) new kernel
  `ALIGNMENT = 7` constraint branch in **both** force kernels (axis in `params[0]`, pull to
  shared-mean, capped) driven by the re-wired-live `alignment_strength`
  (`PhysicsSettings.alignmentStrength`, default 0, no struct-size change); domain
  `AlignmentHorizontal/Vertical/Depth` → kind 7; `ontology_constraint_mapper` emits chained
  pairwise Y-alignment among `subClassOf` siblings (capped) with unit tests. Gated
  `cargo check --features gpu,ontology,dev-auth` (nvcc) exit 0 — 212-byte struct + layer
  term + alignment branch validated. Codex per-phase pass applied.
- 2026-08-30 — Phase 4 codex pass: two fixes. (1) added a `layer_spacing <= 0 ⇒ inert`
  guard to `sugiyama_layer_bias` (mirrors the P2 plane-spacing guard) so a zero/omitted
  spacing can't collapse all ranks onto Y=0. (2) `ontology_constraint_mapper` now
  sort+dedups siblings before pairing, so duplicate `subClassOf` axioms can't emit both
  A–B and B–A alignment (double force). Re-verified nvcc-green.

## Programme status (final)

The four build phases of ADR-141 are **complete** and each shipped compile-clean + nvcc-green
with a codex per-phase correctness pass:

| Phase | Deliverable | Status |
|---|---|---|
| P1 | Layout-mode GPU plumbing + `/api/layout/mode` unification + cleanup | ✅ complete (landed) |
| P2 | Stratified planes by type (SF channel) | ✅ complete (landed) |
| P3 | Spherical shells + ego-radial (generalised `dag_radial_bias`) | ✅ complete (landed) |
| P4 | Sugiyama Y-by-rank + ontology alignment wire | ✅ complete |
| P5 | User-drawn boundary/region zones | ⏸ **deferred** — separate programme (needs zone-draw UI both clients) |
| P6 | Ring arcs / grid / cylindrical (niche) | ⏸ **deferred** — low value for this estate |

`SimParams` grew 180 → 212 bytes across the programme (layout_mode + plane_bias_k/spacing +
radial_center xyz + layer_bias_k/spacing), both `static_assert`s moved in lockstep at every
step; `alignment_strength` (pre-allocated) went live at P4 with no size change. The estate now
covers taxonomy patterns 1 (Sugiyama), 2 (ego-radial), 6 (alignment, ontology-driven), 8
(spherical shells), 9 (stratified planes), 10 (DAG-directional) as GPU soft forces, plus the
pre-existing radial/boundary/cluster terms — leaving only the deferred zones (5) and niche
patterns (3/4/11/12).

## Status log
