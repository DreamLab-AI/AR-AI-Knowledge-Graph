# ADR-137: XR Render Offload, Runtime Quality Dials, and Full-3D-Default Layout

**Status:** Accepted
**Date:** 2026-08-30
**Deciders:** jjohare, VisionClaw XR/platform team (branch `xr-vive-hardening`)
**Amends:** ADR-071 (Godot + gdext + OpenXR runtime — render pipeline and budget model), ADR-102 (transport — adds the Protocol V5 wrapper the client now decodes)
**Related:**
- ADR-136 (desktop OpenXR / VIVE Pro validation target — the vehicle these changes were brought up on)
- PRD-008 §7.4/§7.5 (XR client native replacement — runtime record)
- PRD-019 (XR transport completion — the transport this extends)
- `docs/reference/binary-protocol.md` (V5 position-stream wrapper)

## TL;DR

The 2026-08-22 VIVE bring-up (ADR-136) rendered the graph but only at a
capped subset and with per-frame GDScript hot loops that collapsed at density.
The `xr-vive-hardening` branch closes that gap with five load-bearing decisions:

1. **Render offload to Rust.** The per-frame position-hunt and MultiMesh buffer
   packing move from GDScript into a pure Rust `RenderStore`
   (`xr-client/rust/src/render_store.rs`), exposed through `BinaryProtocolClient`.
   Full density (**13,164 nodes / 145,692 edges**) now renders at **90 fps**
   (was ~11 fps in GDScript at that density; GDScript held 90 fps only to ~3k).
2. **Runtime-derived instance budgets.** Node/edge draw budgets are derived from
   the received topology (bounded by an absolute safety ceiling), replacing the
   hardcoded 640/3000 quality gates.
3. **Initial-load quality is a settings dial.** `initialNodeLimit`
   (`/api/settings/physics`) replaces the compiled-in `DEFAULT_INITIAL_NODE_LIMIT`;
   the WebSocket receive cap is raised to **256 MiB** so a full-graph initial load
   is not truncated mid-frame.
4. **Full-3D layout is the default.** `axisCompressionZ` is removed; the dual-disc
   flatten is now opt-in (`enableDualDiscLayout`, default **OFF**). The natural 3D
   force layout is what ships.
5. **Protocol V5 + XR-safe eye candy.** The client decodes the V5 position wrapper
   (`[0x05][u64 broadcast_seq][V3 records]`), and adds fresnel-halo / edge-flow /
   centrality-size visual tells that work under the Compatibility renderer's
   no-post-process constraint (ADR-136).

The ADR-071 decision (Godot + gdext + OpenXR, Rust-for-substrate /
GDScript-for-rig) is unchanged; this ADR records how the render path and quality
model were made production-shaped on top of it.

## Context

ADR-136's bring-up validated stereo render and interaction but ran a deliberately
small graph (top-5% / 640 nodes, edges capped at a 200→3000 initial load) and did
its per-frame work in GDScript: `_hunt_positions` over a ~13k `Dictionary` plus
`_update_multimesh` / `_update_edge_multimesh` issuing ~100k `set_instance_*` calls
per frame, measured at **~90 ms/frame** at full density. That is an order of
magnitude over the 11.1 ms 90 Hz budget. Three structural problems followed:

- **Quality was hardcoded, not observed.** The 640/3000 caps were compile-time
  constants unrelated to the actual topology size, so a small graph over-capped and
  a large one still blew the frame budget.
- **The wire truncated large loads.** tungstenite's default 16 MiB message cap
  silently killed a full-graph initial frame → connect→sync→die loop.
- **The layout was artificially flat.** `axisCompressionZ` and the dual-disc
  flatten squashed Z by default, which fought the 3D affordance XR exists to
  provide and (via the old clamping sanitiser) fanned outlier edges onto the
  bounds-cube faces.

## Decision

### 1. Rust `RenderStore` render offload

`xr-client/rust/src/render_store.rs` (pure, no Godot deps) owns node state, the
optimistic position hunt, and MultiMesh buffer packing. It is wrapped by
`BinaryProtocolClient` (`binary_protocol.rs:451`, `GodotClass`) with thin `#[func]`
adapters — GDScript calls `build_node_buffer(ids, comp, 0.7, 1.9)`,
`build_edge_buffer(pairs, radius_comp)`, `hunt(ease, grab_id, grab_pos)`, and
`nodes_near(center, radius, max)`, receiving flat `PackedFloat32Array`s it hands
straight to `MultiMesh`.

- **Buffer layout:** 20 floats/node instance — 12 transform (Godot row-major 3×4)
  + 4 colour + 4 custom — matching `GraphScene.tscn`'s `use_colors=true` /
  `use_custom_data=true` MultiMesh format flags.
- **Colour Rust-side:** `community_color()` ports `graph_scene.gd::_community_color`
  (Louvain hue with anomaly red-blend), computed inside `build_node_buffer`; GDScript
  no longer loops nodes for colour.
- **Edge basis:** `build_edge_buffer` ports the `scaled_local` cylinder basis and
  the (anti)parallel-to-UP degenerate handling from the old
  `_update_edge_multimesh`, returning `None` for near-zero-length edges.

**Perf ladder (measured):** GDScript 3k nodes = 90 fps, 13k = ~11 fps → Rust
offload 13k nodes / 145k edges = 90 fps.

### 2. Topology-derived instance budgets

Instance budgets are recomputed from the received topology
(`graph_scene.gd::_recompute_instance_budgets`): node budget = topology node count,
edge budget = topology edge count, each bounded by an absolute safety ceiling so a
runaway payload cannot overflow the Quest instance buffers. The hardcoded 640/3000
gates are gone. LOD selection is topology-biased (by centrality/weight) and only
recomputed when the selection domain (visuals/topology/budget) changes.

### 3. `initialNodeLimit` settings dial + 256 MiB receive cap

- `initialNodeLimit` lives on `visualisation.graphs.logseq.physics.initial_node_limit`
  and is applied server-side in `socket_flow_handler/types.rs`
  (`resolve_initial_node_limit`, `.take(initial_node_limit)`). Unset (`0`) falls back
  to `INITIAL_NODE_LIMIT_DEFAULT = 3000`; hard ceiling
  `INITIAL_NODE_LIMIT_CEILING = 100_000`. Live-tunable via
  `PUT /api/settings/physics` (dev-auth `Authorization: Bearer dev-session-token`).
- WebSocket receive limit raised to **256 MiB** (`transport.rs:29-30`:
  `max_message_size` / `max_frame_size = 256 * 1024 * 1024`; tungstenite default is
  16 MiB) so a full-graph initial frame is not dropped.

### 4. Full-3D-default layout

- `axisCompressionZ` / per-user `axis_compression_z` **removed**
  (`force_compute_actor.rs:102`, `physics_config.rs:299`,
  `crates/visionclaw-domain/.../simulation_params.rs:170`).
- Dual-disc flatten is now opt-in: `enable_dual_disc_layout` default **false**
  (`settings_handler/types.rs:380`), with `graph_separation_x = 100` as the canonical
  default (`settings_routes.rs:1617`). Natural 3D is the shipped layout; the flatten
  is a CPU-side projection selected only when explicitly enabled.
- The client sanitiser **hard-rejects** non-finite / out-of-`WORLD_LIMIT_M`
  records rather than clamping survivors (`binary_protocol.rs:334-359`): the live
  layout legitimately overshoots the physics volume (observed r_max ~3200), and
  clamping collapsed distinct outliers onto the bounds-cube faces (the edge-fan
  artefact).

### 5. Protocol V5 + XR-safe eye candy

- **V5 wrapper:** `PROTOCOL_V5 = 0x05` wraps a V3 body with an 8-byte broadcast
  sequence: `[0x05][u64 broadcast_seq][V3 node records]`. The client skips the seq
  and decodes the V3 body unchanged (`binary_protocol.rs:23-26,292-309`). See
  `docs/reference/binary-protocol.md`.
- **Eye candy under the Compat no-post-process constraint (ADR-136):**
  - `materials/node_halo.gdshader` — a `next_pass` fresnel halo (fake bloom):
    second pass over the node mesh, shell pushed out along the normal, `cull_front`
    + `blend_add` + `depth_draw_never`, fresnel rim. No WorldEnvironment glow.
  - `materials/edge_flow.gdshader` — additive cyan pulse travelling along the
    cylinder (UV.y, `TIME`-driven), zero per-frame GDScript/material touches, faint
    base alpha so dense bundles don't blow out.
  - **Centrality size tells:** `build_node_buffer(..., 0.7, 1.9)` scales node size
    between those bounds by normalised centrality.

## Consequences

### Positive
- Full-density graph renders at 90 fps on the VIVE Compat path; the 11 ms budget is
  met at 13k/145k, not just 3k.
- Quality scales with the actual graph, tunable live without a rebuild.
- Large graphs load intact (256 MiB cap); the connect→sync→die loop is gone.
- XR gets its native 3D affordance back; the edge-fan clamping artefact is removed.
- Additive eye candy adds depth/salience cues without any post-process pass,
  staying inside the Compat/multiview constraint.

### Negative / open
- `RenderStore` duplicates the colour/basis maths that GDScript once owned; the two
  must stay in sync (mitigated: `render_store.rs` documents each as a direct port and
  carries unit tests).
- Perf numbers are the desktop VIVE (dual Quadro RTX 6000) baseline, **not** Quest 3
  — the Quest perf gates stay unmeasured until a Quest runner exists (ADR-136 D2).
- 256 MiB is a generous cap, not a back-pressure strategy; revisit if a pathological
  payload is ever seen.

### Neutral
- ADR-071's runtime decision is unchanged. This ADR refines the render pipeline and
  budget model inside it.
- V5 is additive over V3 (ADR-102 §2); a V3-only decoder still works against a V3
  stream. ADR-061/`binary-protocol.md` remain the wire references.

## References
- Client: `xr-client/rust/src/{render_store,binary_protocol,transport}.rs`,
  `xr-client/scripts/{graph_scene,hud}.gd`, `xr-client/materials/{node_halo,edge_flow}.{gdshader,tres}`,
  `xr-client/scenes/{GraphScene,HUD}.tscn`
- Backend: `src/handlers/socket_flow_handler/types.rs`,
  `src/handlers/settings_handler/types.rs`, `src/actors/gpu/force_compute_actor.rs`,
  `crates/visionclaw-domain/src/types/physics_config.rs`
- ADR-071, ADR-102, ADR-136; PRD-008 §7.4/§7.5; PRD-019; `docs/reference/binary-protocol.md`
