---
id: ADR-2036
title: Defensive OpenXR boot — query eye-gaze capability and degrade, never blind-bind
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: Godot upstream fixing hazard #113717, or eye-gaze becoming a required (not optional) interaction on the target headset
repo: visionclaw
domain: XR-client
lineage: distils legacy ADR-071 (OpenXR runtime choice) and ADR-139 (immersive interaction); guards Godot upstream hazard #113717
---

# ADR-2036 — Defensive OpenXR boot: query eye-gaze capability and degrade, never blind-bind

## Context

Binding the `XR_EXT_eye_gaze_interaction` action-map entry on a device that lacks the
extension (e.g. Quest 3) trips an action-map error at boot. Separately, the OpenXR
vendor addon adds XR nodes to the tree asynchronously; a synchronous scene swap during
that window trips "Parent node is busy adding/removing children" (Godot upstream hazard
#113717). Both make boot brittle across headsets.

## Decision

`xr_boot` only *queries* `is_eye_gaze_interaction_supported()` and degrades to
head-gaze as primary — it never blind-binds the eye-gaze action-map entry. The
graph-scene swap is deferred to idle via `change_scene_to_packed.call_deferred()` so it
cannot race the vendor addon's node insertion. This forecloses unconditional eye-gaze
binding and any synchronous scene swap during OpenXR bring-up.

## Consequences

- Boots cleanly on headsets with and without eye-gaze; head-gaze is the reliable
  baseline, eye-gaze an opt-in capability.
- Eye-gaze is left disabled unless explicitly supported — a capability surrendered by
  default rather than assumed.
- Deferring the swap adds one idle frame of latency at boot — negligible, and the price
  of dodging hazard #113717.
- See `docs/XR-client.md`.

## Verification

Re-checked at `e0f8cd896`: `xr-client/scripts/xr_boot.gd:47–50` guards on
`has_method("is_eye_gaze_interaction_supported")`, queries it, and appends a head-gaze
degrade warning without binding; `:59–61` swaps via
`get_tree().change_scene_to_packed.call_deferred(graph_scene)`.
