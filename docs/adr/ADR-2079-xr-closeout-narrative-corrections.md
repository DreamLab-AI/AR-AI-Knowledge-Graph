---
id: ADR-2079
title: Correct the XR closeout narrative — the press-mode and DAG-label items are closed in code
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a headset session that finally exercises ADR-2033's behavioural half, a new HUD control added outside the _press_fire helper, or an ingest change that stops collapsing subclass provenance to the 'hierarchical' label
repo: visionclaw
domain: XR-client
lineage: corrects the 2026-09-04 closeout narrative in XR-client against ADR-2033's own 2026-09-05 acceptance-progress section and ADR-2035's reconciled predicate test; neither ADR's decision changes and ADR-2033 stays partial
---

# ADR-2079 — Correct the XR closeout narrative: the press-mode and DAG-label items are closed in code

## Context

Two XR items were recorded as outstanding but are satisfied in the working tree.

**ADR-2033 (HUD press-mode).** The 2026-09-04 closeout in `docs/XR-client.md` says "several HUD
constructors omit the press-mode setting". That was true when written — eleven
`Button`/`CheckButton` sites, three assignments — and is **no longer true of the source**:
`hud.gd` routes every control through a single `_press_fire` helper (`hud.gd:262-264`) with zero
raw constructors remaining. ADR-2033's own "Acceptance progress — 2026-09-05" section already
records this and draws the right distinction: the source-inventory half of the rule holds, the
behavioural half (press-to-dispatch, disabled controls, drag-off, jitter, duplicate actions on
the target runtime) has never been exercised because Godot is not installed here.

**ADR-2035 (collapsed `hierarchical` label).** The same closeout said its "existing predicate
test fails against that implementation when extracted unchanged". That test was reconciled on
2026-09-05: `directed_hierarchy_accepts_subsumption_and_the_collapsed_label`
(`force_compute_actor.rs:4562`) now asserts the accept, and its comment records that the earlier
version contradicted both the implementation and the ratified decision.

Exposed by diagrams VC-36.8 and VC-36.12.

## Decision

ADR-2033 **stays `implementation_status: partial`**. Its acceptance condition includes runtime
verification on the target headset, and none has been run — claiming `complete` on source
inspection alone would overstate it. Only `verified_commit` advances, to the full 40-char SHA.

What is corrected is the *narrative*: no HUD constructor omits the press-mode setting any more.
The rule is enforced structurally rather than by convention — `_press_fire(BaseButton) ->
BaseButton` is the single place `ACTION_MODE_BUTTON_PRESS` is set, and controls are constructed
through it (`var b := _press_fire(Button.new())`). A raw `Button.new()` in `hud.gd` is now the
defect to grep for, not a missing `action_mode` assignment. The governing doc says that instead
of asserting omissions that no longer exist.

ADR-2035's decision, status and verified paths are unchanged; only the closeout narrative that
described its test as failing is corrected. The residual cost the accept carries is restated
rather than dropped: the collapsed label is lossy, so a producer that reuses it for domain
membership contributes edges ranked as if they were subsumption. That is a producer-provenance
question and is exactly ADR-2035's existing `review_trigger`.

## Consequences

- `docs/XR-client.md`'s "Renderer, HUD and hierarchy closeout — 2026-09-04" no longer asserts a
  constructor-coverage gap that the code does not exhibit, while still recording that the
  behavioural half of ADR-2033 and the ADR-2032 receipts are open. A reader auditing against it
  chases the open items and not the closed ones.
- The press-mode invariant is now cheap to audit: one grep for `Button.new()` not wrapped in
  `_press_fire`.
- ADR-2032's scoped-desktop qualification in that same closeout is **not** corrected — it
  remains true. Headset, export and mobile acceptance receipts are still outstanding, and the
  Quest APK is still unbuilt (no Android NDK in this environment).
- ADR-2033 remains open, so its `review_trigger` still stands and the receipt named in
  `docs/estate-closeout/2026-09-05/xr-export-runtime-revision-matrix.md` (Column C, "controller
  input") is still to be filled on a headset.
- No code changes. This ADR is a documentation correction plus one `verified_commit` amendment.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit.

- `grep -c "_press_fire(" xr-client/scripts/hud.gd` → `13` = 1 definition (`:262`) + 1
  doc-comment example (`:261`) + **11 call sites** (`:273, 425, 436, 503, 546, 552, 560, 650,
  663, 699, 704`).
- `grep -n "Button.new()" xr-client/scripts/hud.gd | grep -v "_press_fire"` → no output; no
  control is constructed outside the helper.
- `sed -n '4556,4595p' src/actors/gpu/force_compute_actor.rs` → the test at `:4562` asserts
  `is_directed_hierarchy_relation` accepts `is_subclass_of`, `subclass_of`, `SUBCLASS_OF`,
  `hierarchical` and `HIERARCHICAL`, and its comment states the earlier assertion "previously
  asserted that 'hierarchical' must be REJECTED, contradicting the implementation and the
  accepted decision".
- `cargo test --manifest-path xr-client/rust/Cargo.toml --lib` →
  `test result: ok. 226 passed; 0 failed` (the gdext crate; the predicate test above lives in
  the server crate, which has pre-existing unrelated compile errors in other domains' files).
