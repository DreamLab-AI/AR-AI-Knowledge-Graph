# ADR-136: Desktop OpenXR (VIVE Pro / SteamVR) as the Close-Out Validation Target for the Godot XR Client

**Status:** Accepted
**Date:** 2026-08-20
**Deciders:** jjohare (build authority grant, session 2026-08-20), Fable close-out mesh
**Amends:** ADR-071 (Godot 4 + godot-rust + OpenXR native APK), ADR-130 Decision 4 (sidecar-receipt discipline)
**Related:** PRD-008 §7.3 (gap-close copresence layer), PRD-023 WP-9 canaries, `docs/TODO-unified.md` L-5, `docs/gap-close-evidence/P2-{M1,M2,M3,M4,M6}.md`

## Context

PRD-008/ADR-071 target the Quest 3 APK exclusively; PRD-QE-002 lists PCVR as a
v1 non-goal. But the on-device validation those documents gate on has been
stuck: no Quest 3 self-hosted runner was ever registered, the
`aarch64-linux-android` toolchain is absent from the build container, and the
only executable validation route has been the Monado *simulated*-HMD sidecar
(VNC :5904) — a software receipt, not a headset one. Meanwhile the five
gap-close items (M1, M2/COM-18, M3, M4, M6) sit honestly tiered `standalone`
with three armed canaries (`CANARY-VC-M1-HUD`, `CANARY-VC-COM18-INTERV`,
`CANARY-VC-M4-RAY`) unfired.

A VIVE Pro on HP-Desktop (lighthouse-tracked, SteamVR OpenXR runtime,
dual Quadro RTX 6000) became available 2026-08-20.

## Decision

1. **Desktop Linux OpenXR via SteamVR on the VIVE Pro is an accepted
   validation target** for the copresence layer's live-session receipts. A
   lighthouse-tracked HMD receipt is *stronger* than the Monado simulated-HMD
   receipt ADR-130 D4 already accepts; canaries fired from this session
   legitimately promote M-items `standalone → integrated`.
2. **The Quest 3 APK remains the sole *ship* target.** Nothing here changes
   PRD-008's goals, the frozen APK cross-build (TODO §7), or the Quest perf
   gates (which stay unmeasured until a Quest runner exists). PCVR perf numbers
   collected on the VIVE are recorded as a separate baseline, never compared
   against the Quest gates.
3. **Quest-vendor extension coverage is explicitly out of the receipt's
   scope** (passthrough, hand-tracking, foveation, spatial anchors). `xr_boot.gd`'s
   graceful degradation on their absence is itself part of what the session
   validates. Controller-ray selection substitutes for pinch; the M4 receipt
   names which resolver fired.

## Consequences

- TODO L-5 ("needs a human wearing the headset") is satisfiable now, on the
  VIVE, without waiting for Quest lab hardware.
- The gap-close evidence files gain `-vive-receipt` companions; tier
  promotions cite this ADR.
- A future Quest 3 on-device pass remains desirable for the vendor-extension
  surface (M3's passthrough/hand-tracking subset stays at its current tier
  until then), but no longer blocks close-out of the copresence layer.

## Outcome (2026-08-22 — runtime bring-up)

The VIVE Pro validation target went live. **First-ever working in-headset render
of the Godot XR client was achieved** on 2026-08-22 (branch `xr-vive-runtime`) —
both eyes, live wand interaction, live physics. Details in PRD-008 §7.4; summary:

- **Working stack:** Godot 4.6.1-stable + **Compatibility (OpenGL /
  `gl_compatibility`) renderer** + NVIDIA **580.178.04** (`nvidia-580xx-open-dkms`)
  + native X11 (XFCE) + SteamVR, on HP-Desktop.
- **Hard render constraint discovered:** the RenderingDevice renderers
  (Forward+/Mobile, Vulkan) have a **broken multiview tonemapper** on this
  SteamVR/Linux/NVIDIA stack — null shader variant → **both eyes black**,
  reproducible across Godot 4.3/4.4/4.6.1, NVIDIA 610 and 580, X11 and Wayland,
  all settings. Workaround: the Compatibility renderer (inline tonemap, no RD post
  pass). Glow / advanced post-process breaks Compat XR multiview submission (→
  SteamVR home grid), so **glow OFF, Linear tonemap only**; the second-eye
  `GL_OVR_multiview` works on NVIDIA 580 but not 610. This constraint is now the
  governing fact for any XR renderer choice on this hardware.
- **Validated in-headset:** both-eye render, dual-wand interaction (right wand
  live, left pending power-on), ray/sphere node grab with keep-distance, trackpad
  locomotion, world-anchored wand-movable HUD, custom `openxr_action_map.tres`
  bound to `htc/vive_controller`, top-5% node display (640 by centrality),
  adaptive room-fit, client-side position hunting, edges (node cap 200 → 3000).

**Decision-scope correction to Decision point 1.** Decision 1 anticipated that
"canaries fired from this session legitimately promote M-items `standalone →
integrated`." That promotion has **not** happened yet: the render + interaction
bring-up is done, but the **formal copresence canary session is PENDING** (resumes
next session). `CANARY-VC-M1-HUD`, `CANARY-VC-COM18-INTERV`, and
`CANARY-VC-M4-RAY` have **not** fired on the VIVE. **M-items remain at their
pre-existing tiers** until those receipts exist. Decision 2 (Quest 3 remains the
sole ship target; APK still frozen/unbuilt) is unchanged and confirmed. Substrate
substitution per Decision 3 (controller-ray for pinch) is confirmed live.

**Status of this ADR:** Accepted — target validated as a render vehicle; the
canary-promotion clause of Decision 1 is not yet exercised.
