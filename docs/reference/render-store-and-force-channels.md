---
title: RenderStore and GPU Force-Channel Registry
description: Developer reference for the XR-client RenderStore (ADR-137 render offload + runtime quality dials) and the GPU force-channel registry with pinned-node mask (ADR-138)
category: reference
tags: [xr, render, gpu, physics, force-channels, pinned-nodes, adr-137, adr-138]
updated-date: 2026-08-31
---

# RenderStore and GPU Force-Channel Registry

This reference covers two subsystems that landed with the 2026-08-31 XR/layout
programme:

- **RenderStore** — the render-offload and per-frame draw path introduced by
  [ADR-137](../adr/ADR-137-xr-render-offload-and-runtime-quality-dials.md).
- **The GPU force-channel registry** and **pinned-node mask** —
  [ADR-138](../adr/ADR-138-gpu-force-channel-registry.md).

Every structural claim below cites `file:symbol` (line numbers are indicative and
drift with edits). Where a common assumption is wrong, it is flagged inline.

---

## 1. RenderStore (ADR-137)

### 1.1 Where it lives — and where it does *not*

**RenderStore is a pure-Rust struct in the native XR client, not a frontend
store.** It is defined at `xr-client/rust/src/render_store.rs:431`
(`struct RenderStore`) and has no Godot dependencies; it is exposed to the Godot
rig through `BinaryProtocolClient` in `xr-client/rust/src/binary_protocol.rs`.

There is **no** `RenderStore` symbol anywhere under `client/src` (the desktop
React/R3F web client). The desktop client's zustand stores — `settingsStore.ts`,
`websocketStore.ts`, `timelineStore.ts`, `transientBeamStore.ts`,
`workerErrorStore.ts`, etc. — are unrelated; none of them is a render/quality
store. If you are looking for the offload path, you are looking in the XR client
(`xr-client/rust/`).

The offload is specifically a **GDScript→Rust** move within the XR client, per
ADR-137 §1: the per-frame position hunt and MultiMesh instance packing that used
to run as GDScript hot loops (which collapsed past ~3k nodes) now run in Rust.
There is no separate server-side render-offload subsystem; the server only feeds
positions over the Protocol V5 wrapper (see §1.5).

### 1.2 What the store holds

`RenderStore` (`render_store.rs:431`) holds node/render state keyed by KG node id.
The load-bearing fields:

- **Topology + kinematics:** `id_index: HashMap<u32, usize>`, `ids: Vec<u32>`,
  `targets: Vec<[f32; 3]>`, `positions: Vec<[f32; 3]>` (targets are the received
  positions; `positions` are the eased/tweened render positions).
- **Visual tells:** `centrality: Vec<f32>` + `centrality_max: f32`,
  `color: Vec<[f32; 4]>`.
- **Draw subset:** `drawn: HashSet<u32>` — the node subset packed by the last
  `build_node_buffer`; the edge builder filters against it so edges to
  undrawn nodes are skipped. `render_ids` / `render_positions` cache the packed
  order.
- **Fold ladder (ADR-141 fold feature):** `fold_hidden: HashSet<u32>`,
  `fold_remap: HashMap<u32,u32>`, `fold_badge: HashMap<u32,u32>`; the raw plan
  (`raw_fold_hidden` / `raw_fold_members` / `raw_fold_reps: Vec<u32>`); and
  fold/unfold animation state (`folding: HashMap<u32,u32>`, `unfolding:
  HashSet<u32>`).
- **Query + styling:** `query_vars: HashMap<u32,u8>` (visual query builder
  variable bindings), `edge_styles: HashMap<(u32,u32),u8>`, `node_kind:
  HashMap<u32,u8>` with `type_hidden: [bool; 4]`, `degree: HashMap<u32,u32>`.
- **Agent swarm (ADR-140):** `agent_registry: HashMap<u32, AgentRec>`,
  `agent_actions_total: u64`.

Ingest is via `render_store.rs:1094` `upsert(...)` (positions, community,
anomaly, centrality).

### 1.3 The offload path — per-frame math

- `render_store.rs:1119` `hunt(&mut self, ease, grab_id, grab_pos)` — the
  per-frame position lerp toward `targets` (this is the loop that was GDScript).
- `render_store.rs:1210`
  `build_node_buffer(&mut self, ids: &[i32], scale_comp, size_lo, size_hi) ->
  Vec<f32>` — MultiMesh instance packing; honours the caller-supplied `ids`
  draw-list and force-promotes in-transit fold animations into the draw set
  (`render_store.rs:1237`).
- Companion packers: `build_edge_buffer` (`:1303`), `build_beam_buffer`
  (`:1373`, agent work beams), `build_plane_node_buffer` (`:1409`),
  `build_plane_edge_buffer` (`:1442`, stratified/semantic planes).

### 1.4 Quality dials and LOD — where they actually are

**RenderStore does not hold quality-dial fields.** There is no `quality`,
`render_scale`, `msaa`, or `foveation` field on the struct. The "runtime quality
dials" of ADR-137 are realised elsewhere:

- **LOD is caller-supplied per frame.** The draw budget is the `ids` list passed
  into `build_node_buffer` / `build_beam_buffer`; the store honours it verbatim
  (plus the fold-animation promotion above). A regression test pins this
  behaviour at `render_store.rs:1983`.
- **Instance budgets live on the Godot side.** `graph_scene.gd::
  _recompute_instance_budgets` (`xr-client/scripts/graph_scene.gd`) derives
  node/edge draw budgets from the received topology, replacing the old
  hardcoded 640/3000 quality gates (ADR-137 §2).
- **Initial-load quality is a settings dial.** `initialNodeLimit` on
  `/api/settings/physics` replaces the compiled-in `DEFAULT_INITIAL_NODE_LIMIT`
  (ADR-137 §3). The WebSocket receive cap is raised to 256 MiB so a full-graph
  initial load is not truncated mid-frame.
- **Instance sizing dials** are the `scale_comp`, `size_lo`, `size_hi` scalars
  passed into `build_node_buffer` (`render_store.rs:1210`).

### 1.5 Protocol V5

The client decodes the V5 position wrapper `[0x05][u64 broadcast_seq][V3
records]` in `binary_protocol.rs`; the wrapper is documented in
[`binary-protocol.md`](binary-protocol.md). RenderStore consumes the decoded V3
records via `upsert`.

---

## 2. GPU force-channel registry (ADR-138)

### 2.1 What it is — a mapping/view layer, not an array-backed registry

The registry is a **bounded enum that maps named channels onto the existing flat
`SimParams` scalars** — the module doc in `src/models/force_channels.rs` calls it
the "Named force-channel registry (PHASE 3, mapping layer)". It does **not**
change the 180-byte `repr(C)` `SimParams` layout, the CUDA kernels, or the
`/api/settings/physics` wire. Turning `SimParams` into an actual array of
`{enabled, strength}` channels is the explicitly-deferred step 2 (ADR-138 §TL;DR);
only the bodies of `state`/`apply` change when it lands.

### 2.2 Structure (`src/models/force_channels.rs`)

- `force_channels.rs:52` `pub enum ForceChannel` — 10 variants: `Repulsion,
  Separation, Spring, Centering, Gravity, ClusterCohesion, Constraints,
  DagRadialBias, Annealing, Boundary`.
- `force_channels.rs:83` `pub struct ForceChannelState { pub enabled: bool, pub
  strength: f32 }` — the per-channel view.
- `force_channels.rs:95` `pub const ALL: [ForceChannel; 10]` — the enumerable
  registry / stable order.
- `force_channels.rs:111` `pub const fn key(self) -> &'static str` — stable
  lowercase channel key (`"repulsion"`, `"clusterCohesion"`, …); channel identity.
- `force_channels.rs:129` `pub const fn feature_flag(self) -> Option<u32>` — maps
  flag-gated channels (`Repulsion`/`Spring`/`Centering`/`Constraints`) to their
  `FeatureFlags` bit; the rest are gated by `strength > 0`.
- `force_channels.rs:150` `pub fn state(self, p: &SimParams) -> ForceChannelState`
  — read the channel's on/off + strength out of the flat struct.
- `force_channels.rs:170` `fn strength_of(self, p: &SimParams) -> f32` — which
  scalar backs each channel (e.g. `Repulsion→repel_k`, `Spring→spring_k`,
  `Boundary→viewport_bounds`).
- `force_channels.rs:184` `pub fn apply(self, p: &mut SimParams, s:
  ForceChannelState)` — write-back mutator; zeroes the scalar when disabled and
  sets/clears the flag bit.
- `force_channels.rs:211` `pub const fn is_read_only(self)` — only `Constraints`
  is read-only (residency-driven; the registry cannot toggle it).
- `force_channels.rs:233` `pub fn snapshot(p: &SimParams) -> [(ForceChannel,
  ForceChannelState); 10]` — full-registry snapshot.

Forces are keyed by the enum variant / `key()` and mapped onto the flat
`SimParams` scalars — there is no array-of-channels struct yet.

---

## 3. Pinned-node mask (ADR-138)

Pinned nodes (client-dragged pins) keep their position but must still exert
repulsion/spring on their neighbours. This is done with a GPU mask buffer applied
in the integration kernel — **not** by removing pinned nodes from the force pass.

### 3.1 GPU buffer and application

- **Buffer:** `src/utils/unified_gpu_compute/construction.rs:74`
  `pub pinned_mask: DeviceBuffer<i32>` — `0 = free, non-0 = pinned` (comment
  `construction.rs:71-73`). Allocated zeroed at `construction.rs:358`, wired in at
  `:462`.
- **Upload:** `src/utils/unified_gpu_compute/memory.rs:151`
  `pub fn upload_pinned_mask(&mut self, flags: &[i32]) -> Result<()>`
  (size-checked at `:154`, copy at `:162`).
- **Kernel arg:** `src/utils/unified_gpu_compute/execution.rs:837` passes
  `self.pinned_mask.as_device_ptr()` as the last integration-kernel argument.
- **Kernel application:**
  `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu:933` takes
  `const int* __restrict__ pinned_mask`; at `:941`
  `if (pinned_mask != nullptr && pinned_mask[idx] != 0)` a pinned node copies
  `pos_in → pos_out`, zeroes velocity, and resets the FA2 `prev_force_*`
  accumulators (integration skipped). Crucially the **force pass still reads
  `pos_in` for all nodes**, so neighbours feel the pinned node's force — only the
  pinned node's own integration is suppressed (comment `:927-932`).

### 3.2 Mask lifecycle (`src/actors/gpu/force_compute_actor.rs`)

- `force_compute_actor.rs:278` `pinned_nodes: HashMap<u32, Vec3>` — dragged pins →
  world position (source of truth).
- `force_compute_actor.rs:283` `pinned_mask_dirty: bool` — set on any pin/unpin;
  the next physics step rebuilds and re-uploads.
- `force_compute_actor.rs:526` `fn apply_pin_ops(pinned, pins, unpin) -> bool` —
  pin/unpin bookkeeping (returns changed; marks dirty).
- `force_compute_actor.rs:549` `fn build_pinned_mask(node_ids: &[u32], pinned) ->
  Vec<i32>` — maps GPU-index node ids to the `1`/`0` mask.
- Rebuild + upload sites: `force_compute_actor.rs:669-685` and `:1394-1408`
  (`unified.upload_pinned_mask(&flags)`, clearing `pinned_mask_dirty` on success,
  re-marking dirty on failure). The actor also overwrites the
  `position_velocity_buffer` entries for pinned nodes each frame so neighbour
  springs compute against the pinned location (comment `:514`).

Pin/unpin ops are plumbed from `src/handlers/socket_flow_handler/position_updates.rs`
and `src/actors/messages/physics_messages.rs` through
`src/actors/physics_orchestrator_actor.rs`.

---

## 4. How the two subsystems relate

The force-channel registry adds a `DagRadialBias` channel and the pinned-node
mask on the **server GPU layout** side (ADR-138). Their output — positions plus a
pinned bitmask — is what the XR client's `RenderStore` receives over Protocol V5
and renders (ADR-137). ADR-139 (immersive interaction) and ADR-140 (agent-swarm
visualisation) both build on the `RenderStore` draw path; ADR-141 (constrained
layout) both consumes the force-channel mapping layer and pressures it toward the
deferred array-backed step 2.

## 5. Related ADRs

- [ADR-137](../adr/ADR-137-xr-render-offload-and-runtime-quality-dials.md) — render offload, runtime quality dials, full-3D-default layout, Protocol V5.
- [ADR-138](../adr/ADR-138-gpu-force-channel-registry.md) — force-channel registry (mapping layer now, array-backed later) and the pinned-node mask.
- [ADR-139](../adr/ADR-139-immersive-interaction-adoption-programme.md), [ADR-140](../adr/ADR-140-xr-agent-swarm-visualisation.md), [ADR-141](../adr/ADR-141-constrained-layout-engine-programme.md) — the consumers.
- [`binary-protocol.md`](binary-protocol.md) — the V5 position wrapper.
