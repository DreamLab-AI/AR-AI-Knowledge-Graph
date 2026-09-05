---
id: ADR-2081
title: Remove the dead browser XR-mode surface; the immersive client is the Godot app
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a decision to ship an in-browser immersive session (WebXR or otherwise), which would require re-introducing session lifecycle state deliberately rather than reviving this shell
repo: visionclaw
domain: XR-client
lineage: closes the browser half of the "XR + React clients" split recorded in BASELINE-architecture; the immersive client remains the Godot + OpenXR app of ADR-2032
---

# ADR-2081 — Remove the dead browser XR-mode surface; the immersive client is the Godot app

## Context

The React client carried a complete-looking XR-mode surface that could never activate.
`@react-three/xr` 6.6.29 was declared in `client/package.json` and imported nowhere in
`client/src`. `navigator.xr.requestSession` was never called — `platformManager.ts` only
probed `isSessionSupported`. `setXRMode` and `setXRSessionState` had zero callers, so the
store's `isXRMode` was permanently `false`; `ApplicationModeContext` exposed a second
`isXRMode` derived from an `'xr'` mode that `setMode` was never called with, and
`useApplicationMode` had no consumers at all. `GraphManager` threaded the constant-`false`
flag into `InstancedLabels`, where `!vrMode` was therefore always true. Babylon.js had
already been deleted (`client/vite.config.ts:47`). Exposed by diagrams VC-37.1, VC-37.2,
VC-37.3, VC-37.8 and VC-31.3.

## Decision

The browser client has no XR session state. It keeps only the **capability probe** —
`navigator.xr` presence, `isSessionSupported('immersive-vr'|'immersive-ar')`, the derived
`capabilities.xrSupported/vrSupported/arSupported`, `isXRSupported()`, `xrDeviceType` and
the user-agent `isQuest()` — because `remoteLogger` reports those as telemetry. Everything
that modelled *being in* a session is deleted: the `XRSessionState` type, `isXRMode`,
`xrSessionState`, `setXRMode`, `setXRSessionState`, the `xrmodechange` and
`xrsessionstatechange` event types and their dispatch branches, the `'xr'` member of
`ApplicationMode` and its layout branch, and the `isXRMode` prop chain through
`GraphManager` into `InstancedLabels` (including the now-constant `vrMode` parameter of
`buildLabelLines`). `@react-three/xr` is removed from `client/package.json`.

The immersive client is the Godot + OpenXR application (ADR-2032). Any future in-browser
immersive path is a new decision, not a revival of this shell.

## Consequences

- `docs/BASELINE-architecture.md` "XR + React clients" now describes the React client
  accurately: a WebGL2/WebGPU desktop client consuming the binary position stream and the
  `/api` REST surface, with no immersive mode.
- The client bundle drops `@react-three/xr` and its unique transitive dependencies.
- `InstancedLabels` metadata lines that were gated on `!vrMode` now render unconditionally
  when `showMetadata` is set. This is behaviour-preserving: `vrMode` was always `false`.
- Desktop spatial input (SpaceMouse via WebHID, MediaPipe head-tracked parallax) is
  unaffected — it never depended on XR mode. See VC-37.4 to VC-37.7.
- Follow-on: if an in-browser immersive path is ever wanted, it needs a session lifecycle,
  a render-loop switch and a camera rig — none of which this shell provided.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit.

- `cd client && ./node_modules/.bin/tsc --noEmit` → exit 0, no output.
- `cd client && npm test` (`vitest run`) → `Test Files 69 passed (69)`, `Tests 773 passed (773)`.
- `grep -rn "isXRMode\|setXRMode\|xrSessionState\|xrmodechange\|xrsessionstatechange\|@react-three/xr" client/src/ | grep -v node_modules | grep -v '\.claude-flow/'`
  → no output. (`client/src/.claude-flow/logs/` holds historical agent transcripts, not source.)
- `grep -c "isXRMode\|setXRMode\|xrSessionState" client/src/services/platformManager.ts` → `0`,
  while `isWebXRSupported`, `xrSupported`, `isXRSupported`, `xrDeviceType` and `isQuest` remain.
