---
id: ADR-2005
title: Hexagonal split of the webxr monolith into a thin root binary plus visionclaw crates
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: [Cargo.toml, src/actors, crates/visionclaw-actors/src]
owner: jjohare
review_trigger: completion of the actor extraction into crates/visionclaw-actors, or a new subsystem that does not map to an existing crate layer
repo: visionclaw
domain: BASELINE-architecture
lineage: Distils legacy ADR-090 (hexagonal crate modularisation, 2026-05-28) + parent PRD-016; ADR-090 amendment folded the planned visionclaw-server crate back into the thin root binary.
---

# ADR-2005 — Hexagonal split of the webxr monolith into a thin root binary plus visionclaw crates

## Context

The webxr backend was a single ~123k-line crate: a one-line change recompiled
everything and layer boundaries were unenforceable. Lineage: ADR-090 hexagonal
modularisation (2026-05-28) under PRD-016; the ADR-090 amendment dropped the
planned `visionclaw-server` crate, folding startup wiring back into the root
binary rather than adding a layer.

## Decision

New code lands in the crate matching its hexagonal layer —
`visionclaw-{contracts,domain,protocol,adapters,gpu,ontology,actors,xr-presence,analytics-oracle}`
— and the root binary is reduced to startup wiring. The original workspace declared the
root plus these nine `visionclaw-*` members; current additions are recorded below.
It excludes the gdext client
(`xr-client/rust`) and `agentbox/crates/headroom-napi`, which compile in their
own contexts.

## Consequences

- The compiler enforces declared crate dependencies. Intended layer direction
  and incremental-build savings need separate acceptance evidence.
- The migration is unfinished: the live server still runs from `src/`, and the
  actor layer is barely extracted. Two source-of-truth trees coexist until the
  extraction completes — a real navigation and drift cost.
- `contracts` is a deliberate leaf (no actix, no heavy deps) so it stays
  independently buildable.

## Verification

`Cargo.toml` `[workspace].members` lists `"."` plus the nine `crates/visionclaw-*`
members; `exclude` lists `xr-client/rust` and `agentbox/crates/headroom-napi`.
The extraction is measurably partial: `src/actors/*.rs` has 25 files against 4 in
`crates/visionclaw-actors/src/` (the mint plan recorded 11 — extraction has not
advanced, so `implementation: partial` is if anything sharpened). Verified at
`e0f8cd896`; re-verified at `542d63d1d` after the ADR-141 formatting sweep
reordered `pub use` re-exports in `src/actors/messages/mod.rs` — semantics
unchanged.

## Closeout extension — 2026-09-04

CP-01/03/06/08. Owner remains jjohare with crate/actor/build maintainers. Partial/live is retained. The current manifest has twelve members, adding vault-migrate and visionclaw-integration-tests to the historical root-plus-nine list. The actor crate documents root-internal dependencies that still block extraction. File counts do not prove independent responsibility or build-time improvement.

**Acceptance condition:** Define allowed dependency directions and module ownership, classify forwarding shims versus competing implementations, migrate callers and prove the root contains only its accepted responsibilities. Measure representative incremental changes and verify relevant feature/build combinations before retiring old modules. Reopen on new layers, dependency cycles or actor extraction completion. See [architecture review](../../../VisionFlow/docs/estate-review/vision-and-architecture.md#server-extraction-and-enforceable-boundaries) and [manifest/source receipt](../../../VisionFlow/docs/estate-review/evidence/crate-supervision-snapshot.json). No build timing or complete dependency-graph validation ran.

### Re-verification 2026-09-05 (ADR-2005)

Re-checked at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` after `Cargo.toml` changed
since the previous `verified_commit` (`9423abdb`). Both frontmatter fields are
deliberately loosened for this pass — `verified_paths` is emptied and
`verified_commit` set to the current HEAD — and **both must be restored at the
landing commit** (`verified_paths: [Cargo.toml, src/actors, crates/visionclaw-actors/src]`
plus that commit's SHA) so the staleness check regains its teeth.

Claim-by-claim:

- **Workspace membership is now twelve, not root-plus-nine.** `Cargo.toml:2-15`
  lists `"."` (`:3`) plus `crates/visionclaw-contracts` (`:4`),
  `visionclaw-domain` (`:5`), `visionclaw-protocol` (`:6`), `visionclaw-adapters`
  (`:7`), `visionclaw-gpu` (`:8`), `visionclaw-ontology` (`:9`),
  `visionclaw-actors` (`:10`), `visionclaw-xr-presence` (`:11`),
  `visionclaw-analytics-oracle` (`:12`), `vault-migrate` (`:13`) and
  `visionclaw-integration-tests` (`:14`). The two additions are the ones the
  2026-09-04 closeout extension already recorded — this re-verification confirms
  them against the manifest rather than the prose.
- **Exclusions unchanged.** `Cargo.toml:19` — `exclude = ["xr-client/rust",
  "agentbox/crates/headroom-napi"]`, matching the Decision text.
- **Actor extraction is still partial, and the two counts in the Verification
  section measure different things.** `src/actors/*.rs` is **25** files, unchanged.
  `crates/visionclaw-actors/src/` holds **4** top-level `.rs` files —
  `lib.rs`, `protected_settings_actor.rs`, `supervisor.rs`, `voice_commands.rs` —
  and **11** files counted recursively, because `messages/` contributes the
  remaining seven. The Verification section's "4" is the top-level figure and the
  mint plan's "11" is the recursive one; they were never in conflict, and the
  recursive figure has not moved. Only three of those files are actor
  implementations, so the live tree still runs its actors from `src/`.
- **`contracts` remains a deliberate leaf.** `Cargo.toml` retains the comment that
  `visionclaw-contracts` is independently buildable via
  `cargo build --manifest-path crates/visionclaw-contracts/Cargo.toml`.
- **New observation, not previously recorded.** `crates/graph-cognition-extract/`
  exists on disk but is empty (no `src`, no `Cargo.toml`) and is **not** a
  workspace member — an orphan directory that the member census should either
  adopt or delete.

`implementation_status: partial` and `activation_status: live` are retained: the
manifest grew, the extraction did not.
