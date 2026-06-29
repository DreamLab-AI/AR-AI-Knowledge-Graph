# PRD-07 — Bots & Agent Telemetry

## 1. Capability statement

VisionClaw ingests an **agent telemetry stream** produced by the external
agentbox runtime and renders the live state of a swarm — agent positions,
status, communication edges, parent/child swarm membership — as a second
graph drawn alongside the knowledge graph in the same 3D scene. VisionClaw
neither hosts the broker nor controls the agents; it is a read-mostly
visualisation surface for telemetry that originates elsewhere.

Click-through on an agent capsule opens the agent's control surface in
agentbox / the external forum (forward navigation only; the forwarding
contract is owned by Section 10).

## 2. Why this exists

The baseline at `41979d33e` and `main@HEAD` both ship a "bots" feature
folder (`client/src/features/bots`) and a server-side bots handler
(`src/handlers/bots_handler.rs`) that conflate two distinct concerns:

1. **Visualisation** of running agents — positions, status, edges.
2. **Control** of the swarm — initialisation, spawning, configuration,
   topology changes, prompt injection, task creation.

The control plane has now been re-homed in **agentbox + external forum**
(see Section 10). Continuing to host control endpoints inside VisionClaw
creates a duplicated, drift-prone surface and confuses the security model
(VisionClaw is a Nostr-authenticated visualisation, agentbox is a separate
trust domain).

This section migrates only the visualisation concern forward. The control
endpoints are removed from VisionClaw's surface and replaced by a single
"forward to agentbox" gesture on click-through.

Secondary motivation: `AgentPollingService` polls `/graph/data` every
3–15 seconds on the existing implementation. That endpoint returns the
**full 4519-node knowledge graph** payload because the polling path does
not (and structurally cannot easily) filter by graph type at the URL the
client uses. This polling cycle is the second-largest contributor to the
tab freeze that triggered this whole sprint, after the BroadcastOptimizer
delta-filter bug fixed in Section 1. The polling path is removed.

## 3. Users and use cases

- **Operator** running a swarm from agentbox. Opens VisionClaw alongside
  the agentbox control panel. Expects to see new agents appear in the
  scene within ~1s of spawning in agentbox, with correct type marker and
  initial position.
- **Researcher** observing emergent communication patterns. Expects edges
  between communicating agents to appear and decay (hop-relations) without
  manual refresh.
- **Knowledge worker** browsing the knowledge graph who has no swarm
  running. Expects the agent graph region to be empty and consume no
  bandwidth. Telemetry must be silent when there are no agents.

## 4. Acceptance criteria

A1. **Telemetry-driven only**. Every agent visible in the scene corresponds
    to a telemetry event received within the last `agent_ttl` (default 60s).
    Agents with no telemetry inside the TTL are removed from the scene.
    No agent is ever instantiated client-side from a setting, a constant,
    or a poll response.

A2. **No polling path in active code**. The file
    `client/src/features/bots/services/AgentPollingService.ts` and its
    hook `useAgentPolling.ts` are removed. `BotsDataContext` does not
    instantiate or call `agentPollingService`. The only network surface
    feeding `BotsDataContext` is the WebSocket telemetry stream defined
    by Section 10.

A3. **Dual-graph X-offset placement**. Agent nodes are placed in a
    distinct world-space region (default `+X = 600 units`) so the agent
    graph and the knowledge graph do not overlap. The offset is a
    config-level constant, not a per-frame calculation. See ADR-08 §D6
    for the canonical class-bit allocation that discriminates agent vs
    knowledge nodes.

A4. **AgentCapsule rendering is data-driven**. Section 4 owns the
    `AgentCapsule` geometry and material. This section provides the
    instance buffer (position, scale, status colour token, capability
    bitmask). Rendering must not branch on agent type strings; type is
    expressed by the colour token and the capability bitmask, both
    derived from telemetry on the server side.

A5. **Communication edges decay**. An `AgentCommunicated` event (DDD-07;
    `communication` wire envelope per ADR-10 D1) from agent A to
    agent B causes a transient edge between A and B with a configurable
    decay (default 5s linear fade). After decay the edge disappears
    unless renewed by a fresh event. Parent/child swarm membership edges
    are persistent for the lifetime of the agent.

A6. **Hover overlay shows live telemetry**. Hovering over an agent capsule
    shows: id, type, status, health, cpu_usage, memory_usage, workload,
    tokens, age, swarm_id, parent_queen_id. All values come from the
    most recent telemetry for that agent; the overlay never shows stale
    data older than `agent_ttl`.

A7. **Click forwards to external control surface**. Clicking an agent
    capsule dispatches an `AgentActionEnvelope` (ADR-10 D3 plus
    `crates/visionclaw-contracts/src/agent_action.rs`) on the session's
    chosen transport (BroadcastChannel, deep-link, or postMessage).
    Section 10 owns the envelope schema and the transport selection.
    VisionClaw does not render the control UI in-process.

A8. **Empty-swarm bandwidth floor**. With no agents reported, the
    telemetry WebSocket sends at most one heartbeat per 30s. No idle
    polling. Verified by capturing network traffic over a 5-minute
    idle window.

A9. **Backpressure tolerance**. A burst of 500 telemetry events in <1s
    (swarm initialisation) does not freeze the UI thread. The telemetry
    stream is consumed by a coalescer that batches into the next animation
    frame.

## 5. Non-goals

- **Spawn forms**. Owned by agentbox / forum.
- **Agent control buttons** (pause, restart, kill, reconfigure). Owned
  by agentbox / forum.
- **Broker / message-routing internals**. Owned by agentbox.
- **Swarm topology configuration UI**. Owned by forum.
- **Prompt injection, task creation**. Owned by agentbox.
- **Multi-MCP discovery, agent type catalogues**. Owned by agentbox; the
  client must not maintain a hardcoded type registry.
- **CPU-only fallback**. Telemetry visualisation needs no GPU compute
  beyond the existing physics actor (Section 1).
- **Persistence of telemetry beyond the live session**. The agent graph
  is ephemeral. Only durable agent identities (long-lived bot accounts)
  go to Oxigraph; this is a Section 11 concern and surfaces here only as
  a read-side lookup for friendly names.

## 6. Acceptance evidence to gather during implementation

- Network capture demonstrating no `/graph/data` polling from
  `BotsDataContext` over a 5-minute window.
- Network capture demonstrating idle heartbeat cadence (≤1 msg / 30s)
  with zero agents present.
- Stress fixture: 500-agent telemetry replay confirming UI frame time
  stays under 32ms p95 during burst.
- Removal commit removing `AgentPollingService.ts`, `useAgentPolling.ts`,
  `pollingConfig.ts`, `pollingPerformance.ts`, and the `polling-system.md`
  doc, with no remaining import references.
- Click-through trace showing a well-formed `AgentActionEnvelope`
  (`type === "visionclaw:agent-action"`, `schema_version === 1`,
  required `agent_id`, `kind`, `node_class`) dispatched on the session's
  chosen transport per ADR-10 D3.

## 7. Out-of-scope smells flagged for ADR review

The current implementation contains several patterns whose existence
suggests the wrong architecture; the ADR resolves each:

- **`BotsDataContext` holds `pollNow` and `configurePolling`**. A pure
  visualisation surface should not expose polling control. Address by
  narrowing the context to consume-only of the telemetry stream.
- **`BotsControlPanel.tsx` exists inside VisionClaw**. A control panel
  is by definition control-plane. Address by deleting the file as part
  of this section and forwarding any retained read-only UI (system
  health summary) to a non-control component.
- **`InitializeSwarmRequest`, `SpawnAgentHybridRequest` types live in
  `bots_handler.rs`**. These are control-plane request shapes. Address
  by deleting these handler routes; only the telemetry sink endpoint
  (push from agentbox into VisionClaw) and the click-through forwarder
  remain.
- **`AgentTelemetryStream.tsx` mixes telemetry display with control
  affordances**. Split into pure-display vs. an "open in agentbox"
  link-out component.
- **Server-side static `BOTS_GRAPH: Lazy<Arc<RwLock<GraphData>>>`**
  in `bots_handler.rs` is a process-global mutable graph for the agent
  visualisation, populated by polling-style writes from the client.
  Address by making the server-side agent graph live in the same
  `GraphStateActor` that owns the knowledge graph (with class-flag
  discrimination), populated by inbound telemetry, not by client-driven
  POSTs.

## 8. Interaction with adjacent sections

- **Section 1 (GPU Physics)**: Agent nodes participate in the same
  physics simulation. They share `PhysicsGpuBuffers`. Class-flag bits
  drive per-class force scaling (see DDD-01 §Class). No per-type physics
  branching is hardcoded; differentiation is parameterised.

- **Section 2 (Binary Protocol)**: Agent position frames travel on the
  same binary V3 channel as knowledge-graph positions. Class-flag bits
  on the 32-bit node id let the client de-multiplex without a separate
  channel.

- **Section 3 (Client State)**: The agent telemetry stream is delivered
  via WebSocket and decoded on the main thread (text-frame path; see
  ADR-03 §D9). The worker proxy surface is closed to four methods per
  ADR-03 §D7 and does not handle telemetry. Single-flight discipline for
  telemetry updates is enforced by the Section-7 coalescer (DDD-07 D11),
  not by the worker's frame guard.

- **Section 4 (Rendering)**: Owns `AgentCapsule` geometry, material,
  hover overlay glass, edge cylinder geometry. This section provides
  the data buffers only.

- **Section 8 (Ontology / Graph Data)**: Owns the canonical agent
  *identity* records (durable bot accounts) in Oxigraph. This section
  consumes those as a read-side lookup for display names. Ephemeral
  runtime state is *not* written back to Section 8.

- **Section 10 (External Integrations)**: Specifies the telemetry
  transport (WebSocket frame schema, intake handshake, authentication).
  This section consumes that contract; it does not negotiate it.

- **Section 11 (Persistence)**: Agent runtime state is **in-memory
  only**. Only durable agent identity records are persisted
  (in Oxigraph). This section never writes to SQLite or Oxigraph.

## 9. Configuration surface

Settings owned by this section, exposed through the Section 5 control
panel under `bots.*`:

| Key                              | Default | Purpose                                  |
|----------------------------------|---------|------------------------------------------|
| `bots.agent_x_offset`            | 600     | World-space offset for dual-graph layout |
| `bots.agent_ttl_seconds`         | 60      | Drop agent if no telemetry within window |
| `bots.communication_edge_decay`  | 5.0     | Linear fade seconds for transient edges  |
| `bots.idle_heartbeat_seconds`    | 30      | Telemetry heartbeat cadence when idle    |
| `bots.coalescer_max_batch`       | 64      | Telemetry events flushed per animation tick |
| `bots.click_forward_target`      | agentbox| Resolver hint for control-surface intent |

No setting hardcodes specific agent types. Type metadata arrives entirely
in telemetry.

## 10. Migration steps (post-sprint, executed in Phase 7 per README)

1. Add the telemetry WebSocket consumer in `BotsDataContext` consuming
   the Section 10 frame schema. Feature-flag it `bots.use_telemetry_v2`.
2. Verify telemetry path delivers parity for a recorded swarm session.
3. Flip the flag default to on.
4. Delete `AgentPollingService.ts`, `useAgentPolling.ts`,
   `pollingConfig.ts`, `pollingPerformance.ts`, `polling-system.md`.
5. Delete control-plane routes from `bots_handler.rs`
   (`InitializeSwarmRequest`, `SpawnAgentHybridRequest`, etc.). Keep
   only the telemetry sink and a forwarder endpoint that resolves
   click-through to an agentbox URL.
6. Move agent graph state from the static `BOTS_GRAPH` into the
   `GraphStateActor` with class-flag discrimination.
7. Remove `BotsControlPanel.tsx`; retain `SystemHealthPanel.tsx` as
   read-only summary if it survives a value review.
8. Document the click-through intent contract in Section 10's ADR.
