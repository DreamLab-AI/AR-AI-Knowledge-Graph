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
