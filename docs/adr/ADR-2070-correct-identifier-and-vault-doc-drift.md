---
id: ADR-2070
title: Correct identifier and vault documentation drift where the code already leads
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: the next change to src/utils/binary_protocol.rs wire-id handling, to crates/visionclaw-domain/src/vault/mod.rs gate parsing, or to src/uri/mod.rs cross-substrate mapping
repo: visionclaw
---

# ADR-2070 — Correct identifier and vault documentation drift where the code already leads

## Context
Phase 1 diagram verification (VC-23.7, VC-21.2, VC-21.12, VC-23.6, VC-26.4) found five places where
`docs/IDENTIFIER-taxonomy.md` and `docs/VAULT-corpus-format.md` describe behaviour the code has
already moved past, or cite line numbers that have shifted:
- **Wire-id overflow.** The doc says release builds silently truncate an over-range node id. The code
  now enforces the bound uniformly: `enforce_wire_id_bounds` (`src/utils/binary_protocol.rs:167-188`)
  logs `error!` and masks via `remap_wire_id` (`:199-201`) on all six branches, the untyped fallback
  included (`:445`, carrying an explicit ADR-2024 comment). `debug_assert!` is retained as a
  development aid, not as the bound.
- **owl-class typing.** The VAULT *Inclusion closeout qualification* says owl-class parsing accepts
  non-string scalars without IRI validation. The code now applies a grammar: `is_class_marker`
  (`crates/visionclaw-domain/src/vault/mod.rs:419`) accepts only an absolute `http(s)://`/`urn:` IRI
  or a `prefix:local` CURIE, and rejects values into `owl_class_rejected` (`:68-75`), so
  `owl-class: true` and `owl-class: 42` now shut the gate.
- **Three stale citations:** `cross_from_agentbox` is at `src/uri/mod.rs:650` (doc says `:514-553`);
  `page_is_kg_included` is at `src/services/github_sync_service.rs:2304` (doc says `:2256`);
  `validate_nip98_token` is at `src/utils/nip98.rs:330` (doc says `:270`, in
  `docs/IDENTITY-authority-chain.md`, owned by vc-core and routed to them).

## Decision
Where the code has already implemented the stronger behaviour, the governing document is corrected to
describe the code, and the corresponding "Known divergences" bullet is marked resolved rather than
left standing — a divergence bullet that no longer reproduces is a false positive that costs every
future reader a verification pass.

Specifically: the wire node-id is bounded in **all** builds (release logs and masks; `debug_assert!`
is a development aid only), and the vault class marker is a **typed grammar**, not any scalar that
renders to a non-empty string. Every `file:line` citation touched by this ADR is re-verified against
the working tree, and citations are treated as load-bearing: a stale one is a documentation defect.

## Consequences
- `docs/IDENTIFIER-taxonomy.md` and `docs/VAULT-corpus-format.md` stop overstating known risk; the
  remaining divergence bullets in both are ones that still reproduce.
- The release-build truncation risk is closed as a *documented* hazard, but the underlying design
  point stands: a wire id above `NODE_ID_MASK` still aliases another node after masking. The code
  now makes that loud (`error!` naming the class and both ids) instead of silent. That is the
  documented behaviour, not a silent fix.
- Citation drift will recur as code moves. `review_trigger` above names the three files whose change
  forces a re-check.
- One correction lands in another lead's file: `docs/IDENTITY-authority-chain.md`
  (`validate_nip98_token` `:270` → `:330`) was routed to vc-core, who owns that document.

## Verification
Ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; must be re-run
at the landing commit.

```
$ grep -n "fn enforce_wire_id_bounds\|fn remap_wire_id" src/utils/binary_protocol.rs
167:pub fn enforce_wire_id_bounds(node_id: u32, class: WireIdClass) -> u32 {
199:pub fn remap_wire_id(node_id: u32) -> (u32, bool) {

$ sed -n '440,446p' src/utils/binary_protocol.rs        # untyped fallback now bounded
            // ADR-2024: the untyped branch previously only debug_assert!ed and
            // then forwarded the id unchanged, so a release build shipped an
            // over-range id onto the wire with no diagnostic. It now enforces the
            // same bound as the five typed branches.
            enforce_wire_id_bounds(*node_id, WireIdClass::Untyped)

$ grep -n "pub fn is_class_marker" crates/visionclaw-domain/src/vault/mod.rs
419:pub fn is_class_marker(s: &str) -> bool {

$ grep -n "pub fn cross_from_agentbox" src/uri/mod.rs
650:pub fn cross_from_agentbox(agentbox_urn: &str) -> Option<UrnCrossing> {

$ grep -n "fn page_is_kg_included" src/services/github_sync_service.rs
2304:fn page_is_kg_included(content: &str) -> bool {

$ grep -n "fn validate_nip98_token" src/utils/nip98.rs
330:pub fn validate_nip98_token(
```

No code was changed. `docs/IDENTIFIER-taxonomy.md` and `docs/VAULT-corpus-format.md` were edited in
the same change; `docs/IDENTITY-authority-chain.md` was routed to vc-core.
