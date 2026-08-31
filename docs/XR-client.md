---
title: XR Client Architecture
doc_id: VC-XR
version: 0.1.1
status: draft-for-ratification
verified_commit: 73540faa0
changelog:
  - "0.1.1: flag self-contradictory docstring on is_directed_hierarchy_relation (excludes vs accepts 'hierarchical')"
sources:
  - xr-client/project.godot
  - xr-client/scripts/xr_boot.gd
  - xr-client/scripts/hud.gd
  - xr-client/scripts/graph_scene.gd
  - xr-client/rust/src/render_store.rs
  - xr-client/rust/src/webrtc_audio.rs
  - xr-client/README.md
  - src/handlers/layout_handler.rs
  - src/actors/gpu/force_compute_actor.rs
  - docs/gap-close-evidence/P2-vive-closeout-2026-08-20.md
date: 2026-08-31
---

# XR Client Architecture (VC-XR)

## Purpose
The immersive client that renders the VisionClaw knowledge graph in a headset: a
Godot + OpenXR front-end whose hot path is a godot-rust (gdext) crate. This
document is the living ground truth for how it renders, connects, authenticates
and lays out — code wins over legacy ADR/PRD prose.

## Current State

### Engine, renderer, and the multiview constraint (INVARIANT)
The project is authored against Godot 4.3 (`xr-client/project.godot:12`,
`config/features=PackedStringArray("4.3", "Forward Mobile")`) but the only
build that has ever rendered on a headset is **Godot 4.6.1-stable running the
Compatibility (OpenGL 3) renderer** (README "Version note" and VIVE bring-up
2026-08-22). Do not read the 4.3 string as the runtime.

`renderer/rendering_method="gl_compatibility"` is set for desktop
(`project.godot:48`) and this is **load-bearing, not a preference**: the
RenderingDevice / Vulkan multiview tonemapper is broken on SteamVR + Linux +
NVIDIA and fails stereo submission; Compatibility is the only renderer that
submits both eyes (README render-constraints table, line 119). Consequences that
travel with it and must not be silently "upgraded":
- **Glow / bloom OFF** in `WorldEnvironment` — glow post-process blanks the
  second eye under Compat multiview (README line 120). MSAA is off
  (`project.godot:51 anti_aliasing/quality/msaa_3d=0`), `hdr_2d=false`.
- **NVIDIA 580 open driver pinned** (`nvidia-580xx-open-dkms`); the 610 driver
  fails to render the GL multiview second eye (README line 121).
- **Native X11** (`--display-driver x11`); Wayland was never brought up.
Physics tick is pinned to 90 Hz (`project.godot:43`). OpenXR is enabled with the
HTC Vive action map (`project.godot:56-57`, `openxr_action_map.tres`).

### Scene graph and boot
`XRBoot.tscn` → `xr_boot.gd` initialises the OpenXR interface, sets
`get_viewport().use_xr = true`, probes capabilities, then defers the scene swap
to idle (`xr_boot.gd:53-61`) — a synchronous `change_scene_to_packed()` trips
"Parent node is busy adding/removing children" while the OpenXR vendor addon is
still adding XR nodes. Capability probing is defensive: eye-gaze is only
*queried*, never blindly bound, because enabling the
`XR_EXT_eye_gaze_interaction` action-map binding on a device that lacks it trips
the action-map error (`xr_boot.gd:40-50`). Quest 3 returns false there;
head-gaze stays primary. `GraphScene.tscn` → `graph_scene.gd` (3326 lines) is the
runtime: it holds the `GraphRoot/NodesMulti`, `GraphRoot/EdgesMulti` and
`GraphRoot/AgentMulti` MultiMeshInstance3D nodes (`graph_scene.gd:381-385`).

### HUD structure (hud.gd)
The HUD is a tabbed panel built **programmatically** under `HudControl` into a
SubViewport shown on a world-space, wand-grabbable quad — one source of truth so
every control fits its page (`hud.gd:1-28`). Tab order:
`graph, layout, query, pins, swarm, session, help` (`hud.gd:155`). Stable node
paths are documented in-file (`hud.gd:20-28`) for rebases.

Two overflow lessons are baked in as INVARIANTS:
- **532px page host.** The constrained-layout controls were split out of the
  Graph tab onto a dedicated Layout tab (commit 2358cda4a) because the combined
  page needed ~1050px in a 532px host. The Layout page uses tightened separation
  `3` not the usual `8` (`hud.gd:340-345`): four groups land at 564px with
  default separation — 32px past the host — and the tighter separation buys
  ~35px. A dev-only overflow guard warns once per tab if a page's min-height
  exceeds the host (`hud.gd:756-766`).
- **ACTION_MODE_BUTTON_PRESS everywhere.** Every action button, tab button and
  type-toggle fires on *press*, not release (`hud.gd:252`, `637`, `647`):
  pulling the Vive trigger jolts the ray 20–30px, so a release-mode button often
  sees the release land outside itself and silently cancels the click (observed
  live 2026-08-31, commit ae9c6ac60).
The HUD owns no decision logic — it emits `control_pressed(action)` intents and
GraphScene owns every effect (`hud.gd:76-88`). Overlay panels (intervention,
document card) raise an overlay shield that hides the tab root so a stray ray
can't click a control behind them (`hud.gd:744-749`).

### Transport
Backend resolution is env-overridable (`graph_scene.gd:65-71`,
`_connect_from_env` at 1123): `XR_BACKEND_WS` (default `ws://localhost:4000`) is
the base; two well-known paths are appended — graph stream `/wss`, presence
`/ws/presence`. On the working desktop path the client runs on HP-Desktop and
points at the LAN backend `ws://192.168.2.132:4000` **directly** — nginx `:3001`
does not proxy `/ws/presence` (README run notes). "localhost:4000" is reached
over a reverse SSH tunnel from HP-Desktop to the backend host. The Rust
`BinaryProtocolClient` owns the wire; GDScript only supplies URLs/credentials and
pumps the inbox each frame (`graph_scene.gd:65-66`). It decodes **Protocol V3**
and the **V5 wrapper** (`0x05` + 8-byte broadcast seq) — see VC protocol doc and
README gdext-class table. HTTP origin for writes is derived by swapping the ws
scheme (`_http_base`, `graph_scene.gd:1141-1152`) or `XR_BACKEND_HTTP`.

### Deploy ceremony (desktop OpenXR)
The verified way onto a headset today is not the APK — it is the Compatibility
renderer on native X11 driving SteamVR (README line 81). HP-Desktop is reached
via `ssh john@10.10.10.1` (the 25G rail; gap-close evidence line 14). Launch is a
foreground process:
```
XR_BACKEND_WS=ws://192.168.2.132:4000 XR_NOSTR_SECRET=<hex> \
  godot --path xr-client --rendering-driver opengl3 --display-driver x11 \
  res://scenes/XRBoot.tscn
```
Redeploy is a **separate kill then launch** over two ssh calls (a single chained
call races the compositor), and the launch call must carry `XAUTHORITY` so Godot
can open the X11 display for the SteamVR compositor — the process has no
inherited session. SteamVR must be running with the VIVE Pro tracked and
`~/.config/openxr/1/active_runtime.json` pointed at SteamVR.

### Identity and signing (NIP-98)
`NostrAuth.create(OS.get_environment("XR_NOSTR_SECRET"))` always returns a signer
— ephemeral if the secret is empty (`graph_scene.gd:425-430`). `XR_NOSTR_SECRET`
is a hex BIP-340 key and is **required** in practice: besides the presence
challenge/response handshake it signs the NIP-98 `authenticate` on the graph
socket that gates server-authoritative node drag/pin (README, `transport.rs:75`).
Physics/layout HTTP writes attach a per-request NIP-98 `Authorization: Nostr
<b64>` header minted for the exact URL+method (`_auth_headers`,
`graph_scene.gd:1055-1073`); the HUD decide POST uses the same path
(`hud.gd:1176-1188`). A legacy dev bearer (+ `X-Nostr-Pubkey`) is the fallback
only when no real secret is present; `_nostr_secret_present` gates this and the
dev bearer 401s in release builds (`graph_scene.gd:85-90`).

### Rendering offload and edge stride (INVARIANT 16)
Rust `RenderStore` packs the whole instance buffer; GDScript does a single buffer
assignment per MultiMesh. The edge MultiMesh has `use_custom_data=true`, so the
resource stride is **16 floats/instance** — 12 transform + 4 INSTANCE_CUSTOM
(relation-style code in `.a`), `EDGE_STRIDE_TYPED = 16` in
`render_store.rs:105`. `_update_edge_multimesh` divides the packed buffer by 16
(`graph_scene.gd:1807-1811`); a `/12` divisor mis-sizes `instance_count`, so
`set_buffer` rejects every frame and edges vanish (fixed in commit 63d9bb9b8).
The work-beam MultiMesh (agent→target links, ADR-140 Pillar 2) uses the same
stride 16 (`graph_scene.gd:1818-1830`, `render_store.rs:1362`).

### Constrained layouts
The Layout tab drives the backend layout engine. Six modes cycle through the
`LAYOUT_MODES` enum, POSTed to `/api/layout/mode`
(`graph_scene.gd:213-215`; server enumerates the same list at
`layout_handler.rs:10`): `forceDirected, hierarchical, radial, spectral,
temporal, clustered`. Radial shells POST `/api/layout/radial` with a mode of
`dagRank | typeTier | ego` (`graph_scene.gd:734-742`, `_post_radial` at 983;
server at `layout_handler.rs:135-166`). The Hierarchy toggle PUTs `dagBiasK`
(0.6 on / 0.0 off) and Shells ± nudge `dagLevelDistance` (`graph_scene.gd:888-910`).

**DAG ranks are derived from "hierarchical" edge labels.** As of commit
73540faa0, `is_directed_hierarchy_relation` in `force_compute_actor.rs:580`
matches `is_subclass_of | subclass_of | SUBCLASS_OF | hierarchical |
HIERARCHICAL`. This deployment's ingest writes the collapsed label
`hierarchical`; before the fix the DAG ranks stayed all-unranked
(`compute_dag_ranks`, line 590) so `SetRadialLayout{DagRank}` and the Hierarchy
toggle were silently inert — reported in-headset as the Radial Shells buttons
"appearing disconnected". Caveat for maintainers: the function's own doc-comment
(`force_compute_actor.rs:576-578`) still asserts the generic `"hierarchical"`
string is *excluded* ("accepting it would fabricate ranks from non-subclass
structure"), directly contradicting the `matches!` set three lines below it at
line 586 which accepts it. The inline comment at 581-583 is the authoritative
intent; the stale exclusion paragraph above it should be read as superseded.

## Known divergences & open items
- **project.godot vs runtime.** File says Godot 4.3 / Forward Mobile; the working
  build is 4.6.1-stable Compatibility. The `.godot` metadata has not been
  re-pinned. Divergence is documented in README only; the config string is stale.
- **Quest 3 is unmeasured.** Quest 3 is the sole *ship* target
  (`project.godot:2`, README) but the APK is **unbuilt** and the cross-build is
  frozen — no Android NDK is provisioned in this environment (README line 6).
  The 90 fps figure (13,164 nodes / 145,692 edges) was validated only on VIVE Pro
  + dual-RTX-6000 desktop OpenXR (README line 226). No Quest performance number
  exists. Legacy PRD-008 treats Quest as the primary; that is aspirational.
- **LiveKit / spatial voice incomplete.** `SpatialVoiceRouter`
  (`webrtc_audio.rs`) owns only the routing maths and per-avatar position map;
  the livekit-android AAR media transport (PRD-008 §5.5) that would consume it is
  not wired on any built target (`webrtc_audio.rs:1-5`). Voice is design-complete,
  transport-absent.
- **Query "Execute" is a stub.** The Query tab's Execute button is gated by
  `query_builder.gd:EXECUTE_ENABLED` and renders "Execute (soon)" / disabled
  until a later phase (`hud.gd:403-414`, 716-719).
- **`?token=` / dev bearer fallback.** The dev bearer path exists and 401s in
  release, but the client still constructs it; see VC security doc for the
  broader `?token=` on `/wss` divergence from legacy ADR-011.
- **Legacy ADR status.** ADR-071 (Godot-rust replacement), ADR-136 (VIVE
  validation target), ADR-140 (swarm pillars), ADR-141 (constrained layout) are
  cited as evidence; treat this document as authority where they conflict.

## Invariants (must not silently change)
1. `gl_compatibility` / Compatibility (OpenGL 3) renderer — re-testing on Vulkan
   multiview (SteamVR/Linux/NVIDIA) is required before any change.
2. Glow/bloom stay OFF; NVIDIA 580 open pinned; native X11.
3. Edge and beam MultiMesh stride = 16 floats/instance (matches
   `EDGE_STRIDE_TYPED`); never divide packed buffers by 12.
4. HUD buttons use `ACTION_MODE_BUTTON_PRESS`.
5. Layout-tab page content must fit the 532px host (overflow guard is the tripwire).
6. `XR_NOSTR_SECRET` required for drag/pin/presence; NIP-98 header URL must be the
   exact request URL incl. query.
7. DAG-rank detection must accept the ingest's collapsed `hierarchical` label.

## Change process
Edit the affected `.gd`/`.rs` file, run `cargo test -p visionclaw-xr-gdext`
(141 headless tests, <1 s, no headset/Godot/network needed — README). Any change
to a render-constraint invariant (renderer, glow, driver, display) requires a
fresh on-headset bring-up on the VIVE Pro before merge and a note here. Bump
`version` on ratified change; record new divergences honestly rather than
deleting them.
