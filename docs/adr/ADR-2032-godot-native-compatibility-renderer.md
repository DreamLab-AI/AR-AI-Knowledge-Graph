---
id: ADR-2032
title: Native Godot 4 + godot-rust + OpenXR client on a forced Compatibility renderer
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a driver/runtime change that lets Vulkan/Forward+ submit both eyes on SteamVR+Linux+NVIDIA, or an Android NDK becoming available to build the frozen Quest 3 APK
repo: visionclaw
domain: XR-client
lineage: distils legacy ADR-071 (Godot/gdext/OpenXR replacement, superseding ADR-032 RATK and ADR-033 Vircadia), ADR-136 (desktop OpenXR / VIVE Pro close-out), ADR-102 (wire = Protocol V3)
---

# ADR-2032 — Native Godot 4 + godot-rust + OpenXR client on a forced Compatibility renderer

## Context

The Three.js/R3F/Vircadia WebXR stack is deleted. Target headset is the VIVE Pro
on SteamVR + Linux + NVIDIA (580 open driver, native X11). Empirically only the
OpenGL3 Compatibility backend submits both eyes on that stack — Vulkan/Forward+
renders a single eye or fails to composite. The Rust hot path (per-frame instance
math) must live outside GDScript for frame budget. The wire is Protocol V3.

## Decision

The immersive client is a native Godot 4.x application: godot-rust (gdext) hot-path
crate + OpenXR, booting `res://scenes/XRBoot.tscn`. The desktop profile pins
`renderer/rendering_method="gl_compatibility"` (never Vulkan/Forward+) with its whole
consequence-set: `msaa_3d=0`, `hdr_2d=false`, glow off, NVIDIA 580 open driver, native
X11. This forecloses the WebXR stack (deleted, not dormant) and any Vulkan/Forward+
desktop path. Changing the renderer requires on-headset VIVE bring-up evidence.

## Consequences

- Stereo submission works on the reference headset; that is the whole point.
- Post-processing quality dials (glow, MSAA, HDR 2D) are surrendered on desktop —
  a deliberate quality-for-correctness trade.
- The Quest 3 / mobile profile (`rendering_method.mobile="mobile"`) is cross-build
  frozen and unbuilt: no Android NDK is present, so that target is untested code.
- Governing-doc Invariants 1 and 2. See `docs/XR-client.md`.

## Verification

Re-checked at `e0f8cd896`: `xr-client/project.godot:11` main_scene =
`res://scenes/XRBoot.tscn`; `:48` `rendering_method="gl_compatibility"`; `:51`
`msaa_3d=0`; `:52` `hdr_2d=false`. The gdext hot-path crate exists at
`xr-client/rust/src/render_store.rs`.
