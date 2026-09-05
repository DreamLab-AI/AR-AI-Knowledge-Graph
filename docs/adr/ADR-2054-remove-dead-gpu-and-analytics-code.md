---
id: ADR-2054
title: Delete the dead GPU, analytics and WebSocket code paths
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: Any reintroduction of a message type, module or kernel with no caller at merge time
repo: visionclaw
---

# ADR-2054 — Delete the dead GPU, analytics and WebSocket code paths

## Context

The Phase 1 diagram sweep verified reachability for every actor, message and module in the
GPU and wire domain, and found several that could not execute. Dead code in this subsystem is
worse than clutter: it appears in the supervision diagrams, it is cited by governing docs, and
it makes a reader believe a capability exists. Each item below was confirmed unreachable by
grep before deletion, not merely assumed.

- `StreamingPipeline` (`src/gpu/streaming_pipeline.rs`): exported from `gpu/mod.rs`, never
  instantiated anywhere (VC-13).
- `UpdateComponentEdges`: zero senders tree-wide — only the type definition, its re-export and
  the handler existed. It was the sole writer of `ConnectedComponentsActor::cached_edges`, so
  the CPU fallback that reads that field could only ever return all-singleton components
  (VC-15.8).
- `VisualAnalyticsGPU`, `VisualAnalyticsEngine`, `TSNode`, `TSEdge`: re-exported from
  `gpu/mod.rs`, no caller of `new()` anywhere in `src` (VC-15.12).
- `PushDirective` / `HeartbeatDirective` queueing (ADR-031 item 4): directives were queued into
  `pending_directives`, but `send_pong` / `get_pending_directives` were never called on the live
  ping/pong path — dead on arrival (VC-13.5).
- The `approximate_apsp_kernel` body in `gpu_landmark_apsp.cu`: `#if 0`-guarded and explicitly
  labelled "quarantined (NFR-7)" (VC-15.6).
- `UnifiedGPUCompute::apsp_module`: declared, built from PTX at construction and stored, with
  **zero read sites** — no kernel was ever resolved from it. Found while removing the kernel
  above, not by the Phase 1 sweep.

## Decision

Dead code is deleted, not stubbed, not feature-gated, not left with a "future use" comment.
Each deletion site carries a one-line `REMOVED (ADR-2054):` comment recording what was there
and why it went, so the next reader does not rediscover the same absence as a gap.

Two things are deliberately **kept**, and the distinction is the substance of this decision:

The `ComputeAPSP` handler is retained even though its kernel is gone. It returns an explicit
NFR-7 refusal naming the O(n²) memory reason. An endpoint that refuses clearly is not dead
code — deleting it would turn a documented, intelligible error into a 404 and lose the
explanation.

The `ComputeShortestPaths` and `ComputeConnectedComponents` handlers on `GPUManagerActor` and
`GraphAnalyticsSupervisor` are retained. Phase 1 assessed them as unreachable, and in the
tree as found they were — but ADR-2053 makes them the live entry point for the
`/pathfinding/*` routes once those are retargeted off the standalone actors. Deleting code
that is about to become load-bearing was caught during implementation and reversed.

## Consequences

`ConnectedComponentsActor`'s CPU fallback no longer has a `cached_edges` source. That is
correct rather than a regression: with `UpdateComponentEdges` having no senders, the fallback
was already guaranteed to produce all-singleton components, so it was returning a confidently
wrong answer. ADR-2053's wiring of the GPU path is what makes connected components work.

Removing `StreamingPipeline` narrows `gpu/mod.rs`'s public surface. Nothing consumed it, so no
consumer breaks; if a streaming pipeline is wanted later it should be designed against the
current broadcast path (which, per BROADCAST-001, is a full-snapshot rate-limiter and
visibility cull, not a delta stream) rather than resurrected from this code.

The APSP `.ptx` artefacts are **retained**: the same module also carries
`select_landmarks_kernel` and `stress_majorization_barneshut_kernel`, so it is not an artefact
for the removed kernel alone.

Removing the `apsp_module` load deletes a startup PTX parse and, more importantly, an error
path that logged "GPU DEGRADED: GPU APSP is DISABLED for this process" — alarming operators
about the loss of a capability NFR-7 forbids permanently and that no code could have used. The
`apsp_ptx` parameter is retained in the constructor signature and explicitly ignored, so
callers keep their shape and a future APSP revival has an obvious seam.

## Verification

`grep -rn "REMOVED (ADR-2054)" src/ crates/` — six deletion sites, each with a recorded reason:
`connected_components_actor.rs:276` (`Handler<UpdateComponentEdges>`),
`socket_flow_handler/mod.rs:14` and `actor_messages.rs:230` (`PushDirective` + handler),
`socket_flow_handler/types.rs:219` (`queue_directive`) and `:603`
(`get_pending_directives` + the `pending_directives` field),
`gpu/mod.rs:34` (`TSNode`, `TSEdge`, `VisualAnalyticsGPU`, `VisualAnalyticsEngine`).

`ls src/gpu/streaming_pipeline.rs` — absent.
`grep -rn "UpdateComponentEdges" src/ --include=*.rs` — only the removal comments remain.
`grep -rn "#if 0" crates/visionclaw-gpu/src/cuda_sources/gpu_landmark_apsp.cu` — the guarded
kernel body is gone; only the explanatory header comment remains.

Each item was re-grepped for senders/callers immediately before deletion rather than trusting
the Phase 1 assessment — which is how the `ComputeShortestPaths` reversal above was caught.

`cargo check -p visionclaw-server` — **exit 0, zero errors**, with every Phase 2 change in the
tree. (An earlier run in this phase was blocked by concurrent breakage in three files owned by
other leads; those were fixed by their owners and the check was re-run clean.)

Verification ran on the uncommitted working tree above the recorded SHA; `verified_paths` is
empty for that reason.
