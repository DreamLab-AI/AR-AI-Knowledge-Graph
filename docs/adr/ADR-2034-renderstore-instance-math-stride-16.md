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
agent→target beam locally from its own position store (the target resolved through
the fold plan and gated on `drawn`; the agent read from its local position). This forecloses a `/12` (or any non-16) stride and
any server round-trip for capsule position or beam geometry.

## Consequences

- Agent embodiment and work-beams ship with zero server/wire change.
- The stride is a load-bearing constant split across Rust and GDScript; the two must
  move together or edges blank — the 63d9bb9b8 regression is the proof.
- Beam packing and active-agent hover motion are implemented. Remaining
  motion-authority acceptance requires current runtime evidence; this source pass
  does not establish that every proposed pillar has shipped.
- Governing-doc Invariant 3. See `docs/XR-client.md`.

## Verification

Re-checked at `e0f8cd896`: `render_store.rs:105` `EDGE_STRIDE_TYPED=16`;
`graph_scene.gd:1810` and `:1832` `count = buf.size()/16`; `render_store.rs:1373`
`build_beam_buffer` resolves both endpoints from the local store and emits a
stride-16 record; `graph_scene.gd:1822` `_update_beam_multimesh` runs per frame.
Stride regression fixed in 63d9bb9b8.

## Closeout extension — 2026-09-04

Work package: **CP-04/06**. Owner remains `jjohare`, with protocol, identity and XR maintainers responsible for their respective boundaries.

Hover motion is implemented in hunt(), beyond the older beams-only summary. Targets are fold-remapped and drawn-gated; agent endpoints are read directly. Action events set WORKING without checking timestamp freshness, while JSON state can independently set done/idle.

**Acceptance condition:** Define action/state precedence and expiry; test reordered completion and old actions, disconnects, folded/hidden endpoints and stable stride-16 buffers; verify one authenticated action visibly on the intended headset.

Dependencies: CP-01 release identity and CP-04 authority where authenticated actions cross the wire. Reopen on the existing review trigger, a changed opcode or a failing freshness/visibility probe. Existing verification and activation fields retain their historical scope; this annex records source/local tests at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`, not a new live certification.

See [rendered-state review](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/rendered-state.md) and [receipt](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/evidence/xr-render-snapshot.json).

## Acceptance progress — 2026-09-05

**Precedence and expiry are now defined and enforced.** The finding was that every
`0x23` action set `WORKING` without consulting its timestamp, while the JSON state
channel could independently set done/idle — so a late-arriving old action
resurrected a completed agent and re-drew its beam. Two independent producers
writing one record made "last writer wins by arrival order" wrong.

The contract, implemented in `RenderStore`:

- Both channels carry a position on the same **wrapping** `u32` server-millisecond
  clock, compared with `ts_is_newer` (RFC 1982 serial arithmetic). A naive `a > b`
  would, at the ~49.7-day wrap, treat every fresh timestamp as ancient and freeze
  every agent's status permanently.
- An update applies only if it is strictly newer than the newest evidence already
  applied from **either** channel (`AgentRec::evidence_ts`). An out-of-order update
  is dropped whole — status, target, action type and task all stand.
- `set_agent_state` keeps its signature and treats an untimestamped JSON update as
  *current*, which is the only honest reading of one: it supersedes evidence
  already seen, while a later action still supersedes it. `set_agent_state_at`
  gives callers with a real server timestamp strict ordering.
- `expire_stale_agents(now, ttl)` demotes a live `WORKING`/`BLOCKED` whose evidence
  is older than `AGENT_EVIDENCE_TTL` (30 s) to idle and clears its target, removing
  the beam. Terminal `DONE` does **not** decay — it is a reported outcome, not a
  live claim. A future-stamped action is not expired by a lagging clock.

Wired live: `BinaryProtocolClient` anchors the server clock from the newest
timestamp in each action batch, sweeps once per `poll()`, and exposes
`agent_evidence_counters()` (`[actions_dropped_stale, states_dropped_stale,
expiries]`).

**Tests run.** `xr-client/rust`: `cargo test` — 227 lib + 75 integration pass. Nine
new cases: a reordered old action cannot resurrect a completed agent (and is not
counted as ingested activity); a genuinely newer action still supersedes a
completion; an out-of-order state update is refused; an untimestamped state update
is current; wrap-safe comparison including an action across the `u32` boundary;
stale evidence demoting a live status and removing its beam, asserted through
`build_beam_buffer`; `BLOCKED` decaying while `DONE` does not; a future-stamped
action surviving a lagging clock; refreshed evidence clearing the expired marker.
The existing 218 tests pass unchanged, including those that set state after an
action — the precedence rule is compatible with them.

Stride-16 beam packing is untouched and remains covered by the existing
`EDGE_STRIDE_TYPED` assertions, which the new expiry test also exercises.

**Governed paths changed.** `xr-client/rust/src/render_store.rs`,
`xr-client/rust/src/binary_protocol.rs`.

**Open.** Disconnects, folded/hidden endpoints and the "one authenticated action
visibly on the intended headset" condition are unaddressed here; the last is
hardware-bound. The clock anchor is an estimate derived from action timestamps
plus locally elapsed time — with no action ever received there is no anchor and
expiry is a no-op, which is correct but means expiry only operates once the data
plane is live.
