---
id: ADR-2033
title: HUD buttons fire on press, not release
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
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

## Closeout extension — 2026-09-04

CP-01/06/08. Owner remains jjohare with XR/interaction maintainers. Implementation is partial against the every-control rule: current hud.gd has eleven Button/CheckButton construction sites and three explicit press-mode assignments. Query Execute/Clear, swarm teleport, Join Room, Mute, Reconnect and scroll arrows omit the assignment. Existing helper/tab settings remain implemented; historical live activation is retained.

**Acceptance condition:** Inventory all dynamically and statically created ray-driven controls, establish the intended action mode and verify actual press-to-dispatch behaviour, disabled controls, drag-off, jitter and duplicate actions on the target runtime. Reopen on any control constructor or interaction-model change. See [review](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/rendered-state.md#xr-control-coverage-and-hierarchy-semantics) and [source receipt](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/evidence/xr-decision-probe.json). No missed click or controller behaviour was reproduced.

## Acceptance progress — 2026-09-05

**Implemented — the every-control rule now holds in source.** The closeout counted
eleven `Button`/`CheckButton` construction sites in `hud.gd` against three
press-mode assignments, leaving Query Execute, Query Clear, swarm teleport, Join
Room, Mute, Reconnect and both scroll arrows on Godot's default release mode —
exactly the controls where the trigger jolt cancels a click.

A single helper is now the only place the mode is set:

```gdscript
func _press_fire(b: BaseButton) -> BaseButton:
	b.action_mode = BaseButton.ACTION_MODE_BUTTON_PRESS
	return b
```

All **eleven** sites route through it, wrapping the constructor inline
(`var b := _press_fire(Button.new()) as Button`). The three sites that already
assigned the mode were converted too, so no site sets `action_mode` directly any
more — a rule stated as "every control" cannot be maintained by hand-assignment at
each site, which is how the eight omissions arose. `CheckButton` is covered by the
`BaseButton` parameter type, so the Mute toggle fires on press like the rest.

Verified by inspection of the edited file: `Button.new()`/`CheckButton.new()`
occurrences not wrapped in `_press_fire` = **0**; `action_mode` assignments outside
the helper = **0**.

**Inventory of the eleven sites.** Tab bar; Query Execute; Query Clear; swarm
roster teleport; Join Room; Mute (`CheckButton`); Reconnect; `_action_btn` helper;
`_type_toggle_btn` helper; scroll-up arrow; scroll-down arrow.

**Tests run.** None — Godot is not installed in this environment, so GDScript
cannot be parsed or executed here. Changes were kept syntactically conservative
and self-consistent: one added function, eleven single-line constructor
substitutions, and removal of the now-redundant inline note in `_action_btn` whose
assignment the helper replaced. No control flow, signal connection or layout code
was touched.

**Governed paths changed.** `xr-client/scripts/hud.gd`.

**Open — all runtime verification.** Actual press-to-dispatch behaviour, disabled
controls, drag-off, controller jitter and duplicate-action handling still require
the target runtime and a headset; none was exercised. Implementation moves from
partial to complete **against the source-inventory half** of the rule only; the
behavioural half stays open. The receipt to fill is specified in
`docs/estate-closeout/2026-09-05/xr-export-runtime-revision-matrix.md`
(Column C, "controller input").
