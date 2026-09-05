---
id: ADR-2007
title: GPUManagerActor is a coordinator over four subsystem supervisors on a context bus
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/actors/gpu/gpu_manager_actor.rs, src/actors/gpu/mod.rs, src/actors/gpu/context_bus.rs, docs/GPU-wire-abi.md]
owner: jjohare
review_trigger: a new GPU subsystem that does not fit the four-supervisor split, or a change to SharedGPUContext distribution
repo: visionclaw
domain: BASELINE-architecture
lineage: No dedicated legacy ADR; distils the in-code 'Phase 7: God Actor Decomposition' refactor, promoted into the BASELINE actor-topology map.
---

# ADR-2007 — GPUManagerActor is a coordinator over four subsystem supervisors on a context bus

## Context

The GPU subsystem was a single God Actor: one failure took down force
computation, analytics and graph analytics together, and every actor held a
central GPU handle. Failures did not isolate and restart policy was uniform.
Lineage: the in-code "Phase 7: God Actor Decomposition" refactor, promoted here
into the BASELINE actor-topology map (no separate legacy ADR number).

## Decision

`GPUManagerActor` is a lightweight coordinator that spawns four subsystem
supervisors — **Resource, Physics, Analytics, GraphAnalytics** — each isolating
its own failures with exponential-backoff restart and reporting health
independently. `SharedGPUContext` is distributed by ResourceSupervisor through direct messages
to registered supervisors, with additional GPUContextBus publication. Shared
context ownership and recovery still couple subsystem behaviour.

## Consequences

- Restart policies are organised per supervisor. Isolation from shared-context
  failure requires runtime evidence; it is not guaranteed by topology alone.
- The analytics kernels the AnalyticsSupervisor carries are code-fixed but not
  all output-validated — legacy ADR-031's "known-broken" list is stale in the
  good direction (Louvain D1 fix, PageRank D8 fix; Landmark-APSP remains
  compile-quarantined). Per-kernel evidence: `docs/GPU-wire-abi.md`
  §"Analytics kernel trust status". Residual work is output validation
  (benchmarks), not disablement; the topology is sound independent of any
  kernel's output quality.
- One more indirection layer (bus + supervisors) to trace for a GPU call versus
  the old single actor.

## Verification

`src/actors/gpu/gpu_manager_actor.rs` spawns `ResourceSupervisor`,
`PhysicsSupervisor`, `AnalyticsSupervisor` and `GraphAnalyticsSupervisor`.
`src/actors/gpu/mod.rs` documents the "Phase 7: God Actor Decomposition"
topology and the `SharedGPUContext via GPUContextBus broadcast` distribution.
`src/actors/gpu/context_bus.rs` defines `GPUContextBus` with `publish(...)` over
`Arc<SharedGPUContext>`. Verified at `e0f8cd896`.

## Closeout extension — 2026-09-04

CP-01/06/08. Owner remains jjohare with GPU/actor/operations maintainers. Implementation is partial against broadcast-only context distribution and guaranteed isolation; historical live activation is retained. The manager creates four supervisors and registers their addresses with ResourceSupervisor. That supervisor sends context directly, then publishes to the bus. Several send results are discarded before pending graph data is cleared. Physics restarts its sibling group around shared state; address replacement alone does not prove termination or clean device recovery.

**Acceptance condition:** Bind context/graph generations to acknowledged child readiness; reconcile failed sends and late/restarted subscribers. Inject child/supervisor/mailbox/device failures and verify termination, current-state recovery, bounded backoff and the declared isolation boundary. Reopen on context ownership, restart policy or new subsystems. See [supervision review](../../../VisionFlow/docs/estate-review/rendered-state.md#gpu-supervision-and-context-delivery) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/crate-supervision-snapshot.json). No actor crash or GPU recovery was exercised.
