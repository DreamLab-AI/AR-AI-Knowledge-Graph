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

## Closeout extension — 2026-09-04

CP-01/06/08. Owner remains jjohare with XR/render/release maintainers. Complete/live is retained for the native-client configuration decision. Current project settings still select Compatibility, XRBoot, zero MSAA and disabled HDR 2D, with a separate mobile renderer setting. Boot requires OpenXR initialisation. These are source observations; historical VIVE bring-up does not certify the current exported artefact, driver or mobile toolchain.

**Acceptance condition:** Record the exported client, gdext library, server protocol, driver/runtime and headset revisions; verify stereo submission, frame budget, reconnect and input on that combination. Test mobile separately before changing its frozen declaration. Reopen on renderer/runtime, export, shader or target changes. See [XR coverage review](../../../VisionFlow/docs/estate-review/rendered-state.md#xr-control-coverage-and-hierarchy-semantics) and [receipt](../../../VisionFlow/docs/estate-review/evidence/xr-decision-probe.json). No Godot, Android or headset execution ran.

## Acceptance progress — 2026-09-05

**No execution possible; the receipt is now specified.** Godot is not installed in
this environment, there is no Android SDK and no headset is attached, so every
behavioural condition here is hardware-bound by construction. What was closable
without hardware is the *shape* of the evidence: the acceptance asked for
revisions to be recorded, without saying which revisions, in what combination, or
what counts as a filled cell — so the condition could only ever stay open in the
abstract rather than as a specific missing run.

`docs/estate-closeout/2026-09-05/xr-export-runtime-revision-matrix.md` is that
specification, shared with ADR-2036. It carries:

- **Column A, build identity — populated from source** at
  `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`: gdext crate and `godot` binding
  versions, crate type, renderer (`gl_compatibility`), 3D MSAA 0, HDR 2D disabled,
  `XRBoot` boot scene, the separate mobile renderer setting, and the server wire
  contract the exported client must speak (V3 52-byte record, V5 envelope,
  `0x23`/`0x43`/`0x44`).
- **Column B, execution identity — empty**: Godot editor and export-template
  versions, exported artefact and gdext library SHA-256, OpenXR runtime and
  version, GPU/driver, Android build tools.
- **Column C, behavioural receipts — empty**: stereo submission, frame budget,
  reconnect (now observable against the ADR-2018 resync), controller input.
- **Pass criteria**, including that desktop and Quest rows are filled
  independently — neither inherits from the other, since ADR-2032 keeps the mobile
  declaration separately frozen and accepted.

**Tests run.** None; none are possible here.

**Governed paths changed.** None in source.
`docs/estate-closeout/2026-09-05/xr-export-runtime-revision-matrix.md` added.

**Open — entirely.** Columns B and C are unfilled and nothing in this pass should
be read as a passing result. Complete/live is retained for the scoped native-client
*configuration* decision only; the exported artefact, driver and mobile toolchain
remain uncertified, and historical VIVE bring-up stays historical.
