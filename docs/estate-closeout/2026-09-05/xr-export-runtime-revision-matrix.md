---
title: XR export and runtime revision matrix
status: template
date: 2026-09-05
type: reference
adrs: [ADR-2032, ADR-2036]
---

# XR export and runtime revision matrix

ADR-2032 (Godot native Compatibility renderer) and ADR-2036 (defensive OpenXR
boot) both close on evidence this environment **cannot** produce: Godot is not
installed here, there is no Android SDK, and no headset is attached. Their
acceptance conditions are hardware-bound by construction.

What *can* be closed without hardware is the shape of the receipt. Both ADRs ask
for revisions to be *recorded* — and until now no one had said which revisions,
in what combination, or what counts as a filled cell. That ambiguity is why the
conditions have stayed open in the abstract rather than as a specific missing
run.

This file is that specification: the matrix to fill, the source of each value,
and the pass criteria. It is a **template with the source-derivable columns
already populated**; the execution columns stay empty until someone runs the
export on real hardware.

## Why desktop and Quest stay separate

ADR-2032 freezes a *mobile* renderer setting distinct from the desktop one, and
ADR-2036's boot path differs by runtime (SteamVR/Monado on desktop, the Quest
OpenXR runtime on device). A pass on one target is not evidence for the other:
different renderer, different driver, different OpenXR implementation, different
frame budget. Every row below is therefore per target, and a run fills exactly
one row.

## Column A — build identity (derivable from source; filled)

Recorded at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`.

| Field | Desktop | Quest 3 | Source |
|---|---|---|---|
| gdext crate | `visionclaw-xr-gdext` 0.2.1 | same | `xr-client/rust/Cargo.toml` |
| gdext binding | `godot` 0.2 | same | `xr-client/rust/Cargo.toml` |
| Rust edition | 2021 | 2021 | `xr-client/rust/Cargo.toml` |
| crate type | `cdylib` + `rlib` | same | `xr-client/rust/Cargo.toml` |
| renderer | `gl_compatibility` | mobile renderer setting (separate) | project settings, ADR-2032 |
| 3D MSAA | 0 | (record at export) | project settings |
| HDR 2D | disabled | (record at export) | project settings |
| boot scene | `XRBoot` | `XRBoot` | project settings |
| server wire | V3 52-byte record; V5 envelope; `0x23`, `0x43`, `0x44` | same | ADR-2018/2019/2020 |
| presence codec | `visionclaw-xr-presence` 0.2.1 | same | crate manifest |

The wire row matters for ADR-2032's "server protocol" requirement: an exported
client is only compatible with a server speaking these opcodes, and the
consumer-freshness gate added under ADR-2018 is part of that contract.

## Column B — execution identity (empty; requires hardware)

One row per export+runtime combination actually exercised.

| Field | How to obtain | Desktop | Quest 3 |
|---|---|---|---|
| Godot editor version | `godot --version` | — | — |
| export template version | export preset | — | — |
| exported artefact SHA-256 | `sha256sum` of the built package | — | — |
| gdext library SHA-256 | `sha256sum` of the built `.so` | — | — |
| OpenXR runtime + version | `xr_boot.gd` interface name at boot | — | — |
| GPU / driver version | `glxinfo` / device report | — | — |
| Android build tools | export preset (Quest only) | n/a | — |

## Column C — behavioural receipts (empty; requires a headset)

Each cell is pass/fail **plus** the artefact that shows it.

| Check | ADR | Evidence required | Desktop | Quest 3 |
|---|---|---|---|---|
| stereo submission | 2032 | both eyes rendered, per-eye capture | — | — |
| frame budget held | 2032 | frame-time trace over ≥60 s | — | — |
| reconnect | 2032 | socket drop → recovery, with the ADR-2018 resync observed | — | — |
| controller input | 2032 | every ray-driven HUD control dispatches | — | — |
| eye-gaze present | 2036 | boot log showing the capability found | — | — |
| eye-gaze absent | 2036 | boot log showing the warning, boot continuing | — | — |
| OpenXR runtime missing | 2036 | error surfaced, no scene transition | — | — |
| controller fallback | 2036 | hand tracking unavailable → controllers drive selection | — | — |
| scene transition | 2036 | deferred transition completes after boot checks | — | — |

## Pass criteria

1. **Every Column B field is filled for the row.** A behavioural pass with an
   unrecorded driver or artefact hash certifies nothing repeatable — the point
   of the matrix is that a later regression can be attributed.
2. **Desktop and Quest rows are filled independently.** Neither inherits from
   the other; ADR-2032 explicitly keeps the mobile declaration frozen and
   separately accepted.
3. **Both ADR-2036 eye-gaze rows are filled** — present *and* absent. The
   defensive branch is the decision; testing only the happy path leaves the
   branch that ADR-2036 is about unexercised.
4. **The artefact hash in Column B matches the build the checks ran against.**

## What ADR-2033's closeout contributes here

The Column C "controller input" cell became meaningfully testable on
2026-09-05: all eleven ray-driven HUD control construction sites in `hud.gd` now
route through the single `_press_fire` helper, so press-mode is uniform rather
than set at three sites out of eleven. Before that change the cell could only
ever have recorded "some controls dispatch" — the drag-off failure the press
mode exists to prevent was still live on Query Execute/Clear, swarm teleport,
Join Room, Mute, Reconnect and both scroll arrows.

Verifying press-to-dispatch on the target runtime remains hardware work and stays
open. The source-side inventory is closed.

## Status

Columns B and C are unfilled. **No Godot, Android or headset execution occurred
in the 2026-09-05 pass**, and nothing in this file should be read as a passing
result. ADR-2032 and ADR-2036 remain open on their hardware conditions; what has
changed is that the receipt they need is now specified rather than described.
