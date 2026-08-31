---
id: ADR-2007
title: GPUManagerActor is a coordinator over four subsystem supervisors on a context bus
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
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
independently. `SharedGPUContext` is distributed by a `GPUContextBus` broadcast
(publisher/subscriber), not a central handle passed by the coordinator, so
subsystems receive context without coupling to each other.

## Consequences

- A crash in one subsystem no longer downs the others; restart backoff is
  per-supervisor.
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
