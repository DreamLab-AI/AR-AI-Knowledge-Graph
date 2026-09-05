---
title: GPU & Wire ABI
doc_id: VC-GPU-ABI
version: 0.1.3
status: draft-for-ratification
verified_commit: 73540faa0
changelog:
  - "0.1.3: 2026-09-05 remediation — ADR-2053 context delivery, ADR-2054 dead-code removal, ADR-2055 physics-v2 retirement, ADR-2056 single PTX rewrite, ADR-2059 inert flag removal, ADR-2060 citation corrections (ENABLE_CONSTRAINTS repointed to force_channels.rs derive_dispatch_feature_flags; 180-byte comment corrected in code); ADR-2061 proposed for the analytics validation gap"
  - 0.1.1: correct path-3 actor call-site citation (force_compute_actor.rs 1557 → 2057); fix stale 180-byte comment line ref (:7 → :5)
  - 0.1.2: analytics kernel trust status corrected — Louvain/PageRank carry in-source fixes (D1/D8 markers), broken-list was stale-good; outputs still await reference validation
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
params, and the three host→GPU conversion paths. Also states plainly the trust
status of every analytics kernel (fixed-in-source vs output-validated).

## Current state

### SimParams — the dual struct (212 bytes, statically asserted both sides)

The GPU sees one flat `#[repr(C)]` record. It is declared twice and both declarations
are size-locked to **212 bytes**:

- Rust: `src/models/simulation_params.rs:26-115` (`struct SimParams`), asserted at
  `src/models/simulation_params.rs:228` — `assert!(size_of::<SimParams>() == 212)`.
- CUDA: `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu:21` (`struct SimParams`),
  asserted at `:117` — `static_assert(sizeof(SimParams) == 212, "SimParams size mismatch with Rust")`.

The two `assert`s guard total size: ordinary field growth without updating the
assertion fails to compile. Same-size field reorder/type drift requires separate
layout comparison; see the dated closeout review below. The `force_channels.rs` module
header comment that said "180-byte" was corrected to 212 by ADR-2060; the binding
assertion (212) remains authoritative. Legacy ADR-031 ("48B") and ADR-061 are stale on
this too, and are marked retired below.

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
constraint *residency* on every physics step: `derive_dispatch_feature_flags`
(`src/models/force_channels.rs`) sets the bit whenever `num_constraints > 0`, and
`execution.rs` assigns that helper's result over `sim_params.feature_flags` before every
execute (citation repointed by ADR-2060; it formerly read `execution.rs:903-904`, which
is no longer where the rule lives). This is the ADR-098 fix that made ontology
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

All three build the same 212-byte struct. **The flag-derivation logic is no longer
duplicated** (ADR-2060): `derive_dispatch_feature_flags` in `src/models/force_channels.rs`
is the single helper, `execution.rs` calls it, and its result is written over
`sim_params.feature_flags` before every execute — so whatever word paths 1 and 2 build is
discarded and cannot reach the device. Paths 1 and 2 remain as struct converters; only
their flag words are dead.

1. `From<&SimulationParams> for SimParams` — `simulation_params.rs:236-317`. Derives
   repulsion/spring/centering/SSSP flags from positive scalars (`:239-250`).
2. `From<&PhysicsSettings> for SimParams` — `simulation_params.rs:319-407`. Same flag
   derivation (`:322-332`) but always sets `ENABLE_SSSP_SPRING_ADJUST` and defaults
   `layout_mode`/`radial_center` to inert values so this wire path never silently
   forces a mode or resets an active radial centre.
3. `execute_physics_step` in `src/utils/unified_gpu_compute/execution.rs` — calls
   `params.to_sim_params()` (path 1) for the struct, then calls
   `derive_dispatch_feature_flags` and **overwrites** `sim_params.feature_flags` with the
   returned word. This is where the residency-driven `ENABLE_CONSTRAINTS` bit and the
   runtime `sssp_spring_adjust_enabled` toggle enter. The live actor call site (the
   `execute_physics_step_with_bypass` invocation) is `force_compute_actor.rs:2057`.

Because path 3 assigns the shared helper's word over whatever paths 1 and 2 produced,
the historical failure mode — a setting change reaching the GPU on one path and not
another, the "nothing moves when I change settings" bug — is now structurally impossible
for the flag word: there is exactly one derivation and it always wins. The ADR-2029 test
module `adr_2029_dispatch_authority` (`force_channels.rs`) asserts against the word that
is actually uploaded rather than against a converter's output.

### PTX ISA downgrade hack (`crates/visionclaw-gpu/build.rs`)

CUDA toolkit 13.x emits `.version 9.x` PTX; some host drivers only support 9.0. The
build post-processes every compiled `.ptx`: it finds `.version 9.` and rewrites it to
`.version 9.0` in place (`build.rs:162-179`). If `nvcc` runs and fails, it falls back to
pre-compiled PTX bundled in the crate / `/app` image (`build.rs:137-160`). Failure
to launch `nvcc` panics before that branch. Empty or
missing PTX panics the build (`build.rs:181-187`).

### Analytics kernel trust status

Legacy ADR-031's "known-broken" list is **stale in the good direction** — the named
kernels have since been fixed in source (re-verified 2026-08-31 during the
operative-pack mint):

- **Louvain community detection** — FIXED: fully consistent synchronous pass, "D1
  fix" marker (`crates/visionclaw-gpu/src/cuda_sources/gpu_clustering_kernels.cu:581`).
- **PageRank** — FIXED: the buggy per-block dangling-mass kernel was deleted in
  favour of the correct global two-kernel path, "D8 fix" marker
  (`crates/visionclaw-gpu/src/cuda_sources/pagerank.cu:263`).
- **DBSCAN** — no broken marker remains; border handling flows through the
  propagate/finalise phases (`gpu_clustering_kernels.cu:1079-1080`).
- **Landmark APSP** — compile-quarantined (`#if 0`-class guard), not shipped enabled.
- **Node embeddings** (legacy ADR-072) — no embedding kernels exist in the tree; the
  "random noise" hash bag-of-characters path is gone as a GPU concern.
- **LOF** (local outlier factor) — fixed and trustworthy.
- **Ontology constraints** (legacy ADR-098) — fixed and live (keystone wiring above).

No formal correctness benchmark has re-validated Louvain/PageRank/DBSCAN outputs
against reference implementations — the fixes are code-verified, not
output-verified. Treat results as probably-correct pending a validation pass.

## Known divergences & open items

- **Stale in-code comment** — *Resolved — ADR-2060 (2026-09-05)*. The `force_channels.rs`
  module header said "180-byte"; corrected to **212**, the asserted size.
- **Legacy ADR wire sizes are stale** — *Resolved — ADR-2060 (2026-09-05)*. ADR-031 ("48B")
  and ADR-061 ("28B forever") predate the 212-byte struct and the 52-byte wire record. Code
  wins: `SimParams` is 212 bytes, the V3 wire record is 52. 28 survives only as the internal
  server-side `BinaryNodeData`, never on the wire. The stale "48 bytes/node" comment on
  `MessageType::BinaryPositions` was corrected in the same change (ADR-2057).
- **Duplicated flag derivation across three paths** — *Resolved — ADR-2060 (2026-09-05)*.
  The refactor this bullet asked for has landed: `derive_dispatch_feature_flags`
  (`src/models/force_channels.rs`) is the single shared helper and the sole dispatch
  authority. `execution.rs` calls it and assigns the result over `sim_params.feature_flags`
  before every execute, so the converter paths in `simulation_params.rs` are dead for
  dispatch and cannot diverge onto the device. The ADR-2029 test module
  `adr_2029_dispatch_authority` observes the word actually uploaded.
- **`ENABLE_TEMPORAL_COHERENCE` (bit 3) and `ENABLE_STRESS_MAJORIZATION` (bit 5)** are
  **reserved bits, not a divergence** (ADR-2060). They are declared but are not live GPU
  force channels — `derive_dispatch_feature_flags` never sets either, and stress
  majorization is CPU-side (`simulation_params.rs:77`, "Stress majorization params live on
  CPU (SemanticProcessorActor); not in GPU SimParams"). The bit positions are part of the
  frozen 212-byte ABI and are deliberately not reclaimed: doing so would be a wire-visible
  change for no benefit.
- **Analytics outputs are code-fixed but not output-validated**: Louvain/PageRank/
  DBSCAN carry in-source fix markers (see trust table above) but no reference-
  implementation benchmark has confirmed their outputs; a validation pass is the
  remaining step before full trust. **Open** — carried forward as ADR-2061 (proposed),
  which specifies the per-kernel acceptance tests against
  `crates/visionclaw-analytics-oracle`.
- **Registry cannot toggle Constraints**: enablement is residency-owned. Any UI that
  exposes a "constraints on/off" switch through the force-channel registry is inert by
  design (`is_read_only`). **Working as intended** — ADR-2029; retained here as a caution
  to UI authors, not as a defect.
- **Frozen `physics-v2` layout engines** — *Resolved — ADR-2055 (2026-09-05)*. The
  five-engine `LayoutEngine` registry behind the `physics-v2` feature had stub `step()`
  bodies, was never in `default`, and was marked "must not be enabled in a shipped build",
  yet `layout_handler` advertised all six `LayoutMode` variants and silently coerced an
  unknown mode to `ForceDirected`. The stubs are removed and the handler now rejects an
  unknown mode instead of coercing it.

## Invariants (must not silently change)

1. `size_of::<SimParams>() == 212` on both sides; the two static assertions stay in
   lockstep. New fields append at the tail only.
2. `ENABLE_CONSTRAINTS` is set iff `num_constraints > 0` at physics-step time — never
   derived from user settings. The rule lives in `derive_dispatch_feature_flags`
   (`src/models/force_channels.rs`); `execution.rs` calls it and assigns the result
   (citation corrected by ADR-2060 — it previously pointed at `execution.rs:903-904`,
   which is no longer where the rule lives).
3. `derive_dispatch_feature_flags` is the single authority for the dispatched
   `feature_flags` word. It derives repulsion/spring/centering from the same
   `scalar > 0.0` rule and adds constraint residency and the runtime SSSP toggle; its
   result is written over `sim_params.feature_flags` before every execute, so any word
   built by a converter is discarded (ADR-2029, ADR-2060).
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

## Layout and force closeout — 2026-09-04

ADR-2028/2029 now distinguish host layout parity, total-size guards and final dispatch authority. The extracted Rust/C++ declarations agree at all 53 offsets; a temporary dt/damping swap preserves size and passes the original assertion. No CUDA/device path ran. Require actual toolchain/module identity, drift-negative CI, coordinated consumer upgrades and actor-to-device constraint/SSSP transition receipts. See [estate simulation review](../../VisionFlow/docs/estate-review/rendered-state.md#simulation-layout-and-force-authority).

## PTX closeout — 2026-09-04

ADR-2030 is partial after six isolated build-phase cases distinguished missing/failed nvcc, nonempty/valid output and warning/actual version changes. Runtime loading separately selects and rewrites PTX; its structural substring check is not driver or required-symbol validation. Require selected-module hashes, host ABI agreement, native/stub identity and actual driver/kernel receipts. See [estate PTX review](../../VisionFlow/docs/estate-review/rendered-state.md#ptx-build-acceptance-and-loaded-artefact-identity). No full build or GPU execution ran in this pass.

## Remediation — 2026-09-05

One line per ADR from the Phase 2 remediation of the Phase 1 diagram findings (VC-10 … VC-18).

- **ADR-2053** — Direct point-to-point delivery is the authoritative mechanism for
  `SharedGPUContext`; the `GPUContextBus` is a supplementary broadcast. Delivery failures are
  no longer fire-and-forget: they are logged, recorded, and reported as `Degraded` health.
  Every GPU analytics actor has one residency (the supervisor's instance).
- **ADR-2054** — Deleted dead code: `StreamingPipeline`, `UpdateComponentEdges` and its
  handler, `VisualAnalyticsGPU`/`TSNode`/`TSEdge`, the `PushDirective`/`HeartbeatDirective`
  queueing, and the `#if 0` APSP kernel body. `ComputeAPSP` is retained as an explicit NFR-7
  refusal.
- **ADR-2055** — Retired the frozen `physics-v2` feature and the five-engine `LayoutEngine`
  registry (stub `step()` bodies, never shipped), plus `PhysicsGpuBuffers` and its acceptance
  tests. `set_layout_mode` now rejects an unknown mode instead of silently coercing it, and
  advertises only the five modes the live handler honours.
- **ADR-2056** — One span-parsed `.version` rewrite. The runtime PTX downgrade delegates to
  `ptx_policy::rewrite_ptx_version` instead of carrying its own substring implementation; it
  retains only the runtime driver-ISA probe, which the build-time constant cannot serve.
- **ADR-2059** — Removed the seven analytics feature flags that gated nothing; kept
  `ontology_validation` (a real gate) and `sssp_integration` (documented as display-only).
- **ADR-2060** — Corrected this document's citations and marked resolved divergence bullets
  as such. Bits 3 and 5 are re-described as reserved rather than as a divergence.
- **ADR-2061** *(proposed)* — Per-kernel conformance tests against
  `crates/visionclaw-analytics-oracle`, with stated tolerances. The analytics trust gap stays
  open until it lands.
