---
id: ADR-2033
title: HUD buttons fire on press, not release
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a controller/runtime whose trigger pull no longer jolts the pointer ray, or a switch to a non-ray HUD interaction model
repo: visionclaw
domain: XR-client
lineage: distils legacy ADR-139 (immersive interaction programme), hardened by live VIVE bring-up (commit ae9c6ac60)
---

# ADR-2033 — HUD buttons fire on press, not release

## Context

The HUD is driven by a laser pointer ray from the VIVE controller. Pulling the
trigger to click jolts the ray 20–30px off target. Godot's `BaseButton` default is
`ACTION_MODE_BUTTON_RELEASE`: the click resolves on release, by which point the ray
has often drifted outside the control, so the press is silently cancelled and the
button appears dead. Discovered and fixed during live on-headset bring-up.

## Decision

Every HUD action button, tab, and type-toggle sets
`BaseButton.ACTION_MODE_BUTTON_PRESS`, overriding Godot's release default. The click
commits on the press edge, before the trigger jolt moves the ray. This forecloses the
release-mode default for all ray-driven HUD controls; any new HUD button must set
press mode.

## Consequences

- HUD clicks register reliably on the VIVE despite trigger jolt.
- Accidental brushes over a control now fire immediately (no release-time cancel as a
  safety net) — acceptable for this HUD, but a real behavioural change.
- The rule is per-control, not global, so a future button that omits the setting
  silently regresses; the constructors `_action_btn`/`_type_toggle_btn` carry it by
  default to contain that risk.
- Governing-doc Invariant 4. See `docs/XR-client.md`.

## Verification

Re-checked at `e0f8cd896`: `xr-client/scripts/hud.gd:252` (tab buttons), `:637`
(`_action_btn`), `:647` (`_type_toggle_btn`) all set
`BaseButton.ACTION_MODE_BUTTON_PRESS`. Landed in commit ae9c6ac60.
