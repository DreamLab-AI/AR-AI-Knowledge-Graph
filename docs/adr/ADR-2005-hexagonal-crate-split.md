---
id: ADR-2005
title: Hexagonal split of the webxr monolith into a thin root binary plus visionclaw crates
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 542d63d1d8e28fb2af1dce420635e7a1cee165f6
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
— and the root binary is reduced to startup wiring. The workspace declares the
root plus these nine `visionclaw-*` members and excludes the gdext client
(`xr-client/rust`) and `agentbox/crates/headroom-napi`, which compile in their
own contexts.

## Consequences

- Layer boundaries are compiler-enforced; incremental builds no longer
  recompile the whole tree for a leaf change.
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
