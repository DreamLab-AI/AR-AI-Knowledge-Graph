# ADR-07 — Bots & Agent Telemetry

Status      : Proposed
Date        : 2026-05-16
Supersedes  : (no prior ADR; this is the first explicit telemetry contract)
Related     : ADR-01 (GPU Physics), ADR-02 (Binary Protocol), ADR-03 (Client
              State), ADR-04 (Rendering), ADR-08 (Ontology), ADR-10
              (External Integrations), ADR-11 (Persistence)

## Context

The bots feature on `main@HEAD` accumulated three overlapping mechanisms
for getting agent data into the VisionClaw client:

1. **REST polling** of `/graph/data` every 3–15s via
   `AgentPollingService.ts` → returns the full 4519-node knowledge graph
   payload (no `graph_type` filter applied), filtered client-side by
   class-flag bit. This is the second-largest contributor to the freeze
   regression that triggered this sprint.
2. **REST POST** of agent graph data from the client into the server
   (`POST /api/bots/graph`) which writes into a process-global
   `BOTS_GRAPH: Lazy<Arc<RwLock<GraphData>>>` in `bots_handler.rs`. This
   inverts ownership: the client is acting as a writer to server state.
3. **WebSocket binary frames** carrying mixed knowledge-graph and agent
   position updates demultiplexed by class-flag bits in the node id.
   This is the only mechanism that scales and is the keeper.

In addition, the same feature folder hosts a full control-plane:
swarm initialisation, agent spawning, topology configuration, prompt
injection, task creation. That control-plane is being moved to
agentbox + the external forum (see ADR-10) which removes the duplication
and consolidates the trust domain.

This ADR records the decisions that retain only the *visualisation*
concern in VisionClaw and remove or relocate everything else.

## Decision

### D1. Telemetry is push-only, WebSocket-only

Inbound agent data uses a single transport: the WebSocket telemetry
stream owned by Section 10. There is no polling. There is no client-to-
server write of agent graph state.

Justification: the polling path is the freeze surface this sprint exists
to eliminate. Push-only telemetry naturally bounds bandwidth to actual
swarm activity and avoids the full-KG payload trap.

### D2. `AgentPollingService` and its hook are deleted

Files removed in implementation Phase 7:

- `client/src/features/bots/services/AgentPollingService.ts`
- `client/src/features/bots/hooks/useAgentPolling.ts`
- `client/src/features/bots/config/pollingConfig.ts`
- `client/src/features/bots/utils/pollingPerformance.ts`
- `client/src/features/bots/docs/polling-system.md`

`BotsDataContext` no longer imports `agentPollingService` and no longer
exposes `pollNow` / `configurePolling` on its public type. The polling
status field on the context type is removed. Any UI surface displaying
"polling activity level" is removed with it.

### D3. Single server-side agent graph store

The static `BOTS_GRAPH: Lazy<Arc<RwLock<GraphData>>>` in
`bots_handler.rs` is deleted. Agent nodes and edges live in the same
`GraphStateActor` instance that owns the knowledge graph, discriminated
by class-flag bits on the node id. See ADR-08 §D6 for the canonical
class-bit allocation.

Justification: two graphs in one actor share the physics simulation
(ADR-01), share the broadcast pipeline (ADR-02), and share the
class-aware repulsion / gravity policies. A separate process-global
store guarantees drift.

### D4. Dual-graph X-offset is a layout constant, not a per-frame compute

Agent nodes are placed in world space at `+X = bots.agent_x_offset`
(default 600 units) relative to the knowledge-graph centroid. The offset
is applied once at agent spawn (when a telemetry event introduces an
agent the simulation has not seen before) and the simulation maintains
the separation through the per-class force model.

A per-class gravity coefficient draws the agent graph back toward its
own centroid `(+X, 0, 0)` and the knowledge graph back toward the origin.
The class flag in the GPU buffer drives the choice without rendering-
side branching.

### D5. Communication edges are transient with linear decay

A telemetry `AgentCommunicated { from_agent_id, to_agent_id, weight, timestamp }`
event (per DDD-07 and ADR-10 D1's `communication` wire envelope) creates
an edge in the agent graph with TTL `bots.communication_edge_decay`
(default 5.0s). Edge alpha falls linearly to zero over the TTL window.
On the next event between the same pair, the edge is refreshed in place;
weight is the max of the existing and the new weight.

Edges are coalesced server-side: bursts within the coalescer window are
collapsed to one effective edge before being broadcast. This avoids
"every chat message creates a separate edge object" thrash.

### D6. Parent/child swarm membership edges are persistent

A second edge class — `SwarmMembership` — encodes parent/queen
relationships from the telemetry `parent_queen_id` field. These edges
live for the lifetime of the agent and have distinct rendering treatment
(see Section 4). They are not subject to the communication-edge decay.

### D7. Per-agent telemetry is the source of truth for all live state

Every agent property displayed in the hover overlay
(`status`, `health`, `cpu_usage`, `memory_usage`, `workload`, `tokens`,
`age`, `swarm_id`, `parent_queen_id`, `capabilities`) is read from the
latest telemetry event for that agent. There is no client-side fabrication
of these values, no client-side polling of `/api/agents/{id}`, no
hardcoded default.

If telemetry stops arriving for an agent, the overlay shows the
last-known values for up to `bots.agent_ttl_seconds` (default 60s); after
the TTL the agent is removed from the scene entirely (via an explicit
`AgentDeparted` event from the server-side TTL sweeper, never via
client-side timeout).

### D8. Click-through delegates to Section 10

Clicking an agent capsule constructs an `AgentActionEnvelope` (ADR-10
D3 plus `crates/visionclaw-contracts/src/agent_action.rs`) and
dispatches it via the session's chosen transport. VisionClaw does not
render a control panel in-process and does not embed an iframe of one;
the renderer's responsibility ends at envelope dispatch.

The `crates/visionclaw-contracts` Rust crate is the source-of-truth
contract surface created in the implementation phase; ADR-10 D3 owns
the envelope schema.

### D9. Type metadata flows in telemetry; rendering never branches on type

Agent type is communicated as:

- a `colour_token` (string keyed into Section 4's palette), and
- a `capability_bitmask` (u32; bits assigned by agentbox).

The rendering layer reads these two fields. It never inspects an
`agent_type` string. Adding a new agent type in agentbox does not require
a VisionClaw change.

### D10. Empty-swarm cost is zero

When no agents are present, the telemetry WebSocket emits one heartbeat
per 30s and the agent graph contributes zero nodes/edges to physics,
zero instances to rendering, and zero allocations to the coalescer.
The `BotsDataContext` reduces to an empty array map.

### D11. Backpressure via coalescer batch

The telemetry consumer is a single coalescer that accumulates events
until the next `requestAnimationFrame` and flushes up to
`bots.coalescer_max_batch` events (default 64) per frame. Events beyond
the batch carry into the next frame. The coalescer guarantees ordering
within an agent: the most recent event for a given agent id wins for
status fields, while position updates apply in arrival order to feed
the physics actor.

### D12. Control-plane routes removed via two-phase deprecation

Removal is staged. Phase 7a freezes each route at `410 Gone` with a
`Link: <successor>; rel="successor-version"` header and a structured
JSON body; the route remains registered so external callers receive a
machine-readable failure. Phase 7b (~30 days after Phase 7a lands)
deletes the route, handler, and request types entirely.

| Baseline route | Successor (agentbox) | Phase 7a | Phase 7b |
|----------------|----------------------|----------|----------|
| POST /api/bots/initialize-swarm | POST {agentbox}/swarms/initialize | 410+Link | Deleted |
| POST /api/bots/spawn-agent-hybrid | POST {agentbox}/agents/spawn | 410+Link | Deleted |
| POST /api/bots/spawn-agent (legacy) | (same) | 410+Link | Deleted |
| POST /api/bots/data, POST /api/bots/update | (no successor — was inverted client→server graph write; see D3) | 410+Link to D3 | Deleted |
| DELETE /api/bots/remove-task/{id} | DELETE {agentbox}/tasks/{id} | 410+Link | Deleted |

Phase 7a response body shape:

```json
{
  "status": 410,
  "error": "route_deprecated",
  "successor": "POST {agentbox}/swarms/initialize",
  "deprecated_since": "<ISO-8601 date when Phase 7a landed>",
  "scheduled_removal": "<ISO-8601 date Phase 7b is scheduled>"
}
```

Routes retained (read-only telemetry until `/ws/agent-telemetry` reaches
parity) and re-homed under `src/handlers/telemetry_handler.rs`:

- `GET /api/agents/identity/{id}` — read-only lookup of durable agent
  identity (display name, persistent capability metadata) from Section
  11's Oxigraph store. No mutation.
- `GET /api/bots/{status,agents,data}` — read-only snapshots of the same
  data the WebSocket telemetry stream carries; retained as a polling
  fallback while clients migrate.
- The Section 10-owned WebSocket telemetry intake. Stays a thin adapter.

ADR-10 D7 mirrors the deprecated-route names as a CI guard against
re-introduction; after Phase 7b, the grep gate hard-fails on any of
these names appearing under `src/handlers/`.

## Options considered

### O1. Keep both polling and WebSocket telemetry as fallbacks

Rejected. The polling path was justified historically as "in case the
WebSocket disconnects". But the WebSocket has its own reconnection
discipline (Section 2), and the polling path's payload (full 4519-node
KG) is precisely the freeze surface this sprint exists to remove. The
fallback rationale is also wrong on the merits: a polling fallback that
pulls the wrong dataset is worse than no fallback. A reconnect should
re-sync from the same telemetry source on reconnect.

### O2. Replace `AgentPollingService` with a smaller targeted poll
(`/api/agents` only)

Rejected. An agent-only poll endpoint would solve the payload-size
problem but not the architectural problem: clients should not be asking
"have things changed?" for live state. Agentbox knows when things change
and is the natural publisher. A pull-only path also doesn't carry
communication events (which are momentary by nature) without inventing
a new poll-the-event-log endpoint, which is essentially a worse
WebSocket.

### O3. Keep `BotsControlPanel.tsx` for "convenience"

Rejected. Convenience UI in the visualisation surface is the source of
the duplicated control-plane. Two places to start a swarm means two
trust domains, two configuration surfaces, two failure modes for the
same operation. The hard rule: control-plane is single-sourced in
agentbox/forum.

### O4. Per-type physics branching (the "make agents repel knowledge-
graph nodes harder" request)

Rejected as currently scoped. Per-type physics is a real requirement
but lives in ADR-01's `class_charge` / `class_mass` GPU buffers, which
are already per-node and class-aware. ADR-07 simply confirms that this
section feeds class identifiers into those buffers; it does not invent a
parallel branching mechanism in the rendering or telemetry layers.

### O5. Push-only telemetry with class-tagged ids + single graph store
(this ADR)

Adopted. Single ingress path, no client-to-server writes for live state,
no duplicated graph store, no control-plane in VisionClaw.

## Risks

- **R1**: Telemetry WebSocket disconnect leaves the agent graph stale.
  Mitigation: on disconnect, the client clears the agent set after
  `bots.agent_ttl_seconds`; on reconnect, the server replays the current
  agent set as a single `SwarmSnapshot` event (defined by Section 10).
- **R2**: A misbehaving agentbox could flood telemetry. Mitigation: the
  coalescer batch cap (D11) plus the server-side coalescer (D5) bound
  the rate the client must process. The WebSocket layer also enforces
  Nostr-authenticated ingress so an unauthenticated flood is rejected
  at the transport.
- **R3**: Removing control endpoints breaks any external integrations
  that posted to `/api/bots/initialize-swarm`. Mitigation: the only
  known caller is the agentbox launcher itself, which is in scope for
  the agentbox migration anyway. Add a deprecation-window response
  (`410 Gone` with a `Link` header to the new endpoint location) for
  one release.
- **R4**: Click-through forwarder must work for non-running agents
  (e.g., the click happens just after `AgentDeparted` fires). Mitigation:
  Section 10's resolver accepts a possibly-stale `agent_id` and renders
  an agentbox "agent not found" page; VisionClaw does not pre-validate.

## Rejected from main as buggy / unjustified

- `AgentPollingService.ts` in its entirety — D2.
- `pollingPerformance.ts`, `pollingConfig.ts` — D2.
- `BotsControlPanel.tsx` — D8 (control-plane in visualisation surface).
- `POST /api/bots/graph` client-to-server graph write path — D3.
- The duplicated `BOTS_GRAPH` static — D3.
- The dual-source-of-truth pattern where `BotsDataContext` merged
  polled state and WebSocket state with last-writer-wins — replaced
  by single-source D1.
- Any rendering branch on `agent_type` string constants — D9.

## Bugs and smells at the reset point (41979d33e)

To flag for migration awareness:

- The baseline already contains `AgentPollingService.ts` with a
  hardcoded 2-second active interval. The freeze regression is not
  visible at the baseline only because the baseline knowledge graph
  in test fixtures was smaller. The polling design is wrong at the
  baseline; the freeze just made it visible later.
- `bots_handler.rs` at baseline already exposes the swarm-initialise
  and spawn-agent routes. These are out of scope for VisionClaw from
  day one; the baseline contains them but agentbox was not yet ready.
  Migration removes them now that agentbox is.
- The class-flag bit layout is already in place at baseline and
  survives forward unchanged. See ADR-08 §D6 for the canonical
  class-bit allocation; document it as the cross-section invariant.
- `BotsVisualizationDebugInfo.tsx` exists at baseline and is fine
  (read-only debug overlay). Survives forward.
- `AgentTelemetryStream.tsx` at baseline mixes telemetry rendering
  with subscribe/unsubscribe control buttons. Split per PRD §7.
