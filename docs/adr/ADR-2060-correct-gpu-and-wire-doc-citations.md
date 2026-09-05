---
id: ADR-2060
title: Correct the GPU and wire governing-doc citations and retire resolved divergence bullets
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: Any edit to GPU-wire-abi.md or PROTOCOL-registry.md that adds a file:line citation
repo: visionclaw
---

# ADR-2060 — Correct the GPU and wire governing-doc citations and retire resolved divergence bullets

## Context

The Phase 1 diagram sweep (VC-10 … VC-18) verified every governing-doc citation in the GPU
and wire domain against the working tree. Several were stale or wrong, and two "Known
divergences" bullets described problems the code had already fixed. Because these documents
are the compliance surface, a wrong `file:line` is not cosmetic: it sends the next reader to
code that does not say what the doc claims, and a stale divergence bullet invites someone to
"fix" something already correct.

Findings, all evidenced in the Phase 1 report:
`PROTOCOL-registry.md` cites the 52-byte assertions at `binary_protocol.rs:712,809` (really
`:1052,:1149`), V5 decode at `:513` (really the `PROTOCOL_V5` branch), and the `0x23` block
at `:1125-1135`/`:1354`/`:1501` (really `:1465-1472`/`:1475`/`:1694`) — a systematic drift of
about +340 lines. `BASELINE-architecture.md` cites the V5 envelope into
`binary_settings_protocol.rs`, which is the *settings-socket* `0x05`, not the graph envelope.
`GPU-wire-abi.md` still lists "no single shared helper builds `feature_flags`" as an open
divergence, but `derive_dispatch_feature_flags` exists and is the sole authority.

## Decision

Governing-doc citations are corrected to the working tree, and resolved divergence bullets
are replaced in place with `Resolved — ADR-20xx (2026-09-05)` rather than deleted, so the
history of the claim survives.

Specifically: the shared-flag-helper bullet is marked resolved, naming
`src/models/force_channels.rs` `derive_dispatch_feature_flags` as the single authority and
recording that `execution.rs` overwrites the converters' flag word before every dispatch, so
the converter paths are dead for dispatch and cannot diverge onto the device. The
`ENABLE_CONSTRAINTS` invariant citation is repointed from `execution.rs` to the helper. The
stale "180-byte" description of `SimParams` in the `force_channels.rs` module header is
corrected to 212. The `28B`/`48B` legacy-ADR sizes are recorded as retired with 52 as the
wire truth. The V5 envelope citation in BASELINE is repointed at the graph decoder, and the
registry gains an explicit warning that the two `0x05` allocations sit on disjoint sockets —
the mis-citation is exactly the confusion that hazard predicts.

Where a citation is corrected, the line number is one an editor actually opened at this SHA;
citations are not copied forward from the previous revision.

## Consequences

The documents once again match the code, so the "governing doc → file:line → ledger" lookup
order in `CLAUDE.md` terminates in real code. The cost is that these line numbers will drift
again as the tree moves — this is inherent to `file:line` citation and is why the diagram
tree records symbol names alongside lines, so a reader can re-locate a moved definition.

Two bullets in `GPU-wire-abi.md` are deliberately NOT marked resolved. The reserved-but-dead
`ENABLE_TEMPORAL_COHERENCE` (bit 3) and `ENABLE_STRESS_MAJORIZATION` (bit 5) remain declared,
because the bit positions are part of the frozen 212-byte ABI and reclaiming them would be a
wire-visible change for no benefit; they are re-described as *reserved* rather than as a
divergence. The analytics output-validation gap stays open and is carried forward as
ADR-2061.

## Verification

Every corrected citation was opened at the working tree above
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` before being written:
`sed -n '1052p;1149p' src/utils/binary_protocol.rs` shows the two
`assert_eq!(WIRE_V3_ITEM_SIZE, 52)` sites; `sed -n '710,714p;807,811p'` shows the previously
cited lines are a byte-slice expression and the `the_boundary_is_exact` test, confirming the
drift. `grep -n "struct AgentActionEvent" -A14` places the struct at `:1465-1472` and
`AGENT_ACTION_HEADER_SIZE` at `:1475`; `MessageType::AgentAction = 0x23` is at `:1694`.
`sed -n '218,226p;246,252p' crates/visionclaw-protocol/src/protocols/binary_settings_protocol.rs`
shows a `path_id`-carrying settings message at the lines BASELINE cited for the graph
envelope, establishing the mis-citation.
`grep -n "derive_dispatch_feature_flags" src/models/force_channels.rs src/utils/unified_gpu_compute/execution.rs`
shows the helper at `force_channels.rs:486` and its sole call site in `execution.rs`,
establishing that the "no shared helper" bullet is stale.

Note that the corrected line numbers refer to the tree *before* the ADR-2057 edit to
`binary_protocol.rs`, which inserts the const assertions and shifts later lines; the diagram
tree and the registry were re-verified after that edit.

Verification ran on the uncommitted working tree above the recorded SHA and must be re-run
at the landing commit; `verified_paths` is empty for that reason.
