---
id: ADR-2045
title: Remove the dead ActorLifecycleManager supervision machinery
date: 2026-09-05
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new supervision requirement that GraphServiceSupervisor cannot express
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2045 — Remove the dead ActorLifecycleManager supervision machinery

## Context

Diagrams VC-02.4/2.5/2.6/2.7 flag two complete supervision mechanisms alongside the live
`GraphServiceSupervisor` (`src/actors/graph_service_supervisor.rs`), neither of which runs.
(A) generic `SupervisorActor` (`src/actors/supervisor.rs`): `SupervisorActor::new` called only
from its own `#[cfg(test)]` module; `InitiateGracefulShutdown` never sent in `src/`; its
`SupervisionStrategy::Escalate` arm (`:344-349`) only `warn!`s and returns — it has no parent
field, so escalation cannot happen by construction. (B) `ActorLifecycleManager`
(`src/actors/lifecycle.rs:17-207`) plus `initialize_actor_system`/`shutdown_actor_system` (`:280`,
`:285`), re-exported at `src/actors/mod.rs:102-103`, never called anywhere: a complete parallel
`PhysicsOrchestratorActor`/`SemanticProcessorActor` pair with its own `SupervisionStrategy` and
health monitor. Verification found a third fact the diagrams did not record: the live
`GraphServiceSupervisor` imports `ActorFailed` and the `SupervisorActor` type itself from
`src/actors/supervisor.rs` (`:44`, `:442`, `:805`, `:1427`) for its own (also-unwired) escalation
plumbing, which this task's "do not touch `graph_service_supervisor.rs`" restriction blocks from
being cut.

## Decision

Both mechanisms are dead and the decision is to remove both in full. `src/actors/lifecycle.rs` is
executed now: the file, its `pub mod lifecycle;` declaration and its `pub use lifecycle::{...}`
re-export block in `src/actors/mod.rs` are deleted outright (no stub, no shim), replaced with an
ADR-2045 comment in the house style (`src/handlers/mod.rs:44-53`). `src/actors/supervisor.rs`
is **not** deleted in this change: `graph_service_supervisor.rs` — explicitly out of this task's
scope — has a real, non-test compile-time dependency on `SupervisorActor` (as the type of its
`parent_supervisor` field and `SetParentSupervisor::parent`) and on `ActorFailed` (constructed and
sent at `:805` when `parent_supervisor` is `Some`). Per the verify-before-delete rule ("if any grep
shows a real caller outside a `#[cfg(test)]` block or outside the file itself, STOP and report it
instead of deleting"), this is exactly that case, so `supervisor.rs` is left in place pending a
follow-up that touches `graph_service_supervisor.rs` (owned by vc-core, not this task) to drop the
coupling. Note for that follow-up: `parent_supervisor` is set only by `SetParentSupervisor`, which
is never sent anywhere in `src/` either — the coupling is itself provably dead at runtime, just not
at compile time, so the eventual fix is to delete `parent_supervisor`, `SetParentSupervisor`, and
the `Escalate` branch that uses them from `graph_service_supervisor.rs`, at which point
`src/actors/supervisor.rs` becomes deletable outright.

## Consequences

- `ActorLifecycleManager`, `initialize_actor_system`, `shutdown_actor_system`, the `ACTOR_SYSTEM`
  static, and `lifecycle.rs`'s own `SupervisionStrategy`/`SupervisionDecision` pair no longer exist;
  any future actor-boot code must use `GraphServiceSupervisor`, the one real supervision path.
- `src/actors/supervisor.rs` (`SupervisorActor`, `InitiateGracefulShutdown`, its
  `SupervisionStrategy`, `ActorFailed`, `ActorFactory`, `SupervisedActorInfo`,
  `SupervisedActorTrait`) remains in the tree, still unreachable at runtime outside its own tests,
  because `graph_service_supervisor.rs` references two of its types. This is recorded as
  outstanding work, not silently dropped from scope.
- Follow-on work (tracked here, actioned by vc-core or a later estate pass): remove
  `parent_supervisor`/`SetParentSupervisor`/the dead `Escalate` branch from
  `graph_service_supervisor.rs`, then delete `src/actors/supervisor.rs` entirely and its
  `src/actors/mod.rs:88-90` re-export, closing this ADR's implementation to `complete`.
- `crates/visionclaw-actors/src/supervisor.rs` is a byte-identical duplicate of
  `src/actors/supervisor.rs` in a separate, apparently-unused workspace crate. It was not touched —
  out of this task's named scope (only `src/actors/supervisor.rs` and `src/actors/lifecycle.rs`
  were named) — but is flagged here for whichever lead owns `crates/visionclaw-actors`.

## Verification

Greps run on the uncommitted working tree at `verified_commit` above; must be re-run at the landing
commit.

```
$ grep -rn "SupervisorActor" src/ crates/ --include=*.rs
```
Hits in `src/`: `src/actors/mod.rs:89` (re-export), `src/actors/supervisor.rs` (definition, impls,
and 2 `SupervisorActor::new(...)` calls both inside `#[cfg(test)] mod tests` at `:500`, `:524`),
`src/actors/graph_service_supervisor.rs:44,442` (import and field-type use — the live-code
dependency that blocks deletion), `src/services/speech_voice_integration.rs:54,117` (a `warn!` log
string only, not a type/value use). `crates/visionclaw-actors/src/supervisor.rs` carries the same
shape (definition + 6 `::new()` calls, all inside `#[cfg(test)]`) — a separate, out-of-scope crate.

```
$ grep -rn "InitiateGracefulShutdown" src/ crates/ --include=*.rs
```
`src/actors/supervisor.rs:119` (definition), `:219,222` (its own `impl Handler`) — no sender
anywhere in `src/`. `crates/visionclaw-actors/src/supervisor.rs` mirrors this plus one send at
`:653`, inside `#[cfg(test)]`.

```
$ grep -rn "ActorLifecycleManager\|initialize_actor_system\|shutdown_actor_system" src/ crates/ --include=*.rs
```
Before deletion: all 7 hits confined to `src/actors/lifecycle.rs` itself and its re-export at
`src/actors/mod.rs:102-103`. No hits under `crates/`. After deletion: 0 hits anywhere.

```
$ grep -rn "actors::lifecycle\|use crate::actors::lifecycle" src/ crates/ --include=*.rs
```
0 hits before and after — nothing imported `lifecycle` by path; all consumption was via the
`src/actors/mod.rs` re-export, which is now removed.

```
$ grep -rn "ACTOR_SYSTEM|Phase5SupervisionStrategy" src/ crates/ --include=*.rs   # (orphan check)
```
Before deletion: confined to `lifecycle.rs` (definition/use) and `mod.rs:104` (re-export alias).
After deletion: 0 hits.

Orphan check for `SupervisionStrategy` (declared in both `supervisor.rs` and the now-deleted
`lifecycle.rs`): `grep -rn "SupervisionStrategy" src/ crates/ --include=*.rs` shows the
`supervisor.rs` copy is used only within `supervisor.rs` itself and its `mod.rs:89` re-export —
distinct from `graph_service_supervisor.rs`'s own unrelated `GraphSupervisionStrategy` enum
(`:288`) — so it was kept as part of the still-live `supervisor.rs` file, not removed standalone.

```
$ cargo check -p visionclaw-server --lib
```
Before this change: 0 errors, 17 warnings (the task brief's stated 4-error
`owl_extractor_service.rs` baseline had already been fixed by its owning lead before this
verification ran). After deleting `lifecycle.rs` and its `mod.rs` re-export: 0 errors, 17 warnings
— identical warning set, `Finished` dev profile both times. No new error or warning introduced.

Working-tree caveat: verification ran on the uncommitted working tree above `verified_commit` and
must be re-run at the landing commit.

```
$ node scripts/adr-index-gen.js docs/adr --check
```
Exits 0 (see repo CI log for this change).
