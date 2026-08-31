---
id: ADR-2034
title: The Rust RenderStore owns per-frame instance math under a server-which/client-where authority split
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a change to the INSTANCE_CUSTOM layout, the MultiMesh stride, or the server assuming authority over agent room-position / beam geometry
repo: visionclaw
domain: XR-client
lineage: distils legacy ADR-137 (XR render offload + quality dials, RenderStore owns per-frame math) and ADR-140 (agent-swarm motion-authority split, beam reuse Pillar 2), consuming ADR-059's server-side 0x23 beam wire
---

# ADR-2034 — The Rust RenderStore owns per-frame instance math under a server-which/client-where authority split

## Context

Per-frame instance packing cannot sit in GDScript within the frame budget. Edge and
beam MultiMeshes carry a 12-float transform plus 4 floats of `INSTANCE_CUSTOM`
(relation style / status) = stride 16; a `/12` divisor blanks every edge (regression
in 63d9bb9b8). The swarm needs agent embodiment without a server change: the server
already owns which node an agent works on plus status/task; the room-position of the
capsule and the agent→target work-beam are client concerns.

## Decision

The Rust `RenderStore` packs the entire instance buffer; GDScript does one `set_buffer`
per MultiMesh. `EDGE_STRIDE_TYPED = 16` is authoritative — 12 transform + 4
`INSTANCE_CUSTOM`, readers use `buf.size()/16`. Authority splits: server owns *which*
node + status/task; the XR client owns *where* the capsule hovers and packs the
agent→target beam locally from its own position store (both endpoints resolved through
the fold plan, gated on `drawn`). This forecloses a `/12` (or any non-16) stride and
any server round-trip for capsule position or beam geometry.

## Consequences

- Agent embodiment and work-beams ship with zero server/wire change.
- The stride is a load-bearing constant split across Rust and GDScript; the two must
  move together or edges blank — the 63d9bb9b8 regression is the proof.
- Only P1 (beams) has shipped; later motion-authority pillars remain Proposed, so the
  authority split is real but not yet complete.
- Governing-doc Invariant 3. See `docs/XR-client.md`.

## Verification

Re-checked at `e0f8cd896`: `render_store.rs:105` `EDGE_STRIDE_TYPED=16`;
`graph_scene.gd:1810` and `:1832` `count = buf.size()/16`; `render_store.rs:1373`
`build_beam_buffer` resolves both endpoints from the local store and emits a
stride-16 record; `graph_scene.gd:1822` `_update_beam_multimesh` runs per frame.
Stride regression fixed in 63d9bb9b8.
