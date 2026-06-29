# DDD-07 — Bots & Agent Telemetry Bounded Context

## Bounded context

The **Agent Telemetry** bounded context owns the ingest, in-memory
representation, and visualisation-side projection of live agent runtime
state. It is sovereign over:

- The telemetry stream consumer (an adapter onto Section 10's transport).
- The in-memory agent set: identities, last-known status snapshots,
  membership edges, transient communication edges.
- TTL sweeping of stale agents.
- The dual-graph X-offset placement and class-flag tagging applied at
  agent introduction time.
- The projection of agent state into the GPU buffer slots and the
  rendering instance buffer.

It does *not* own:

- Spawn, kill, reconfigure, or any other control verbs (agentbox /
  forum — see Section 10).
- The broker, message router, or any swarm-internal coordination
  (agentbox).
- Durable agent identity records — friendly names, persistent capability
  metadata (Section 11 — Persistence; Section 8 — Ontology, for the
  schema).
- The shared physics simulation (Section 1).
- The binary broadcast protocol that carries position frames (Section 2).
- The geometry, material, and instance shader of `AgentCapsule` and
  agent edges (Section 4).

The context's relationship to the others is upstream-consumer of
Section 10's transport, upstream-consumer of Section 8/11's identity
read model, and downstream-publisher of *domain events* that Section 1
and Section 4 react to.

## Ubiquitous language

| Term                   | Definition                                                                          |
|------------------------|-------------------------------------------------------------------------------------|
| **Agent**              | A running unit reported by agentbox. Has a stable id within a session, a type token, a status, and a position. |
| **Telemetry**          | A stream of events describing live agent state. Push-only, ordered per agent, transport-owned by Section 10. |
| **Telemetry event**    | One of `AgentJoined`, `AgentPositionUpdated`, `AgentStatusChanged`, `AgentCommunicated`, `AgentDeparted`, `SwarmSnapshot`, `Heartbeat`. |
| **Swarm**              | A logical grouping of agents under a parent/queen; identified by `swarm_id`. VisionClaw does not enumerate swarms; it observes the membership reported in telemetry. |
| **Membership edge**    | A persistent edge from a queen agent to a child agent expressing parent/queen relationship.       |
| **Communication edge** | A transient edge between two agents created by an `AgentCommunicated` event; decays linearly over `bots.communication_edge_decay`. |
| **Hop-relation**       | The general term for "A talks to B" used in product / UX copy. Internally, every hop-relation is a communication edge. |
| **Class flag**         | The high bits of the 32-bit node id distinguishing agent, knowledge, and ontology subtypes. See ADR-08 §D6 for the canonical class-bit allocation. |
| **Dual-graph X-offset**| The world-space offset (`bots.agent_x_offset`, default 600u) that places the agent graph beside the knowledge graph rather than on top of it. |
| **Coalescer**          | The client-side batch buffer that flushes telemetry events on `requestAnimationFrame` boundaries. |
| **TTL sweeper**        | The server-side process that emits `AgentDeparted` for agents whose last telemetry is older than `bots.agent_ttl_seconds`. |
| **Identity record**    | A durable Oxigraph triple about a long-lived agent (display name, owner Nostr key, persistent capabilities). Read-only from this context. |
| **Control surface**    | The agentbox/forum UI for actually doing something to an agent. Lives outside VisionClaw. |
| **Click-through intent**| The `AgentActionEnvelope` (ADR-10 D3) constructed on user click and dispatched via the session's chosen transport; receiver and schema owned by Section 10. |

## Aggregates

### Aggregate 1: `TelemetryStream`

The single point of entry for agent state into VisionClaw. Holds the
WebSocket adapter, the per-agent ordering invariant, and the coalescer
flush schedule.

**Invariants**:

- Events for a given `agent_id` apply in arrival order; position updates
  may not be reordered relative to one another within an agent.
- The coalescer never grows unbounded: if the agentbox publisher sustains
  a rate above the coalescer drain rate, the *oldest* `AgentPositionUpdated`
  events for an agent are dropped (a newer position supersedes them).
  `AgentJoined`, `AgentStatusChanged`, `AgentCommunicated`, `AgentDeparted`
  are never dropped — they alter the agent set membership or its edges.
- A `SwarmSnapshot` is idempotent: applying it twice yields the same agent
  set; it both adds missing agents and removes ones not present.

**Operations**:

- `subscribe(transport)` → bind the WebSocket consumer.
- `tick()` → called from `requestAnimationFrame`; flushes up to
  `bots.coalescer_max_batch` events into the `AgentSet` aggregate.
- `disconnect_reset()` → on transport loss, schedule clear-after-TTL.

### Aggregate 2: `AgentSet`

The live in-memory model of agents currently in the scene.

**Invariants**:

- Every entry has a non-empty class-flagged 32-bit `node_id` with the
  agent bit set.
- An agent's `swarm_id` is immutable once joined. A re-`AgentJoined`
  with a different `swarm_id` is treated as the agent transferring;
  the old membership edge is removed atomically with the new edge being
  added.
- Communication edges referencing a non-present agent are dropped
  silently — orphan edges never exist in the aggregate.
- An agent's `position` is owned by the physics simulation after the
  initial seed. Telemetry `position` is treated as a *seed* on join
  and as a *correction* on subsequent updates (correction is blended
  toward simulated position at a configurable rate; see ADR-01's class
  parameters).

**Entities held**:

```text
AgentIdentity {
    node_id: u32,            // class-flagged
    external_id: String,     // agentbox-owned stable id
    swarm_id: SwarmId,
    parent_queen_id: Option<u32>,
    colour_token: ColourToken,
    capability_bitmask: u32,
    joined_at: Timestamp,
    last_seen_at: Timestamp,
}

AgentStatus {
    health: HealthLevel,
    status: StatusLabel,
    cpu_usage: f32,
    memory_usage: f32,
    workload: f32,
    tokens: u64,
    age_seconds: u64,
    updated_at: Timestamp,
}

CommunicationEdge {
    from: u32,
    to: u32,
    weight: f32,
    created_at: Timestamp,
    decay_until: Timestamp,
}

MembershipEdge {
    queen: u32,
    child: u32,
}
```

**Operations**:

- `join(AgentIdentity, initial_position)` — idempotent.
- `update_position(node_id, position)` — propagates to physics buffer
  through the Section 1 anti-corruption layer.
- `update_status(node_id, AgentStatus)` — pure state update.
- `record_communication(from, to, weight)` — upserts edge, refreshes
  decay window.
- `depart(node_id)` — removes agent, its incident edges, and clears
  its physics-buffer slot.
- `snapshot_apply(SwarmSnapshot)` — diff + apply.

### Aggregate 3: `IdentityReadModel`

A read-only projection from Section 8's Oxigraph store, providing
display-time enrichment (friendly names, persistent owner metadata)
that the live telemetry does not carry.

**Invariants**:

- Read-only. This aggregate never writes to Oxigraph.
- Cache TTL on identity records is `bots.identity_cache_seconds` (default
  300). The cache miss path is a single SPARQL query through the
  Section 11 port.

## Domain events

Emitted by aggregates in this context; consumed by Sections 1, 4, and
the metrics layer.

```text
AgentJoined {
    node_id: u32,
    external_id: String,
    swarm_id: SwarmId,
    parent_queen_id: Option<u32>,
    colour_token: ColourToken,
    capability_bitmask: u32,
    initial_position: Vec3,
    timestamp: Timestamp,
}

AgentPositionUpdated {
    node_id: u32,
    position: Vec3,
    velocity_hint: Option<Vec3>,
    timestamp: Timestamp,
}

AgentStatusChanged {
    node_id: u32,
    status: AgentStatus,
    timestamp: Timestamp,
}

AgentCommunicated {
    from: u32,
    to: u32,
    weight: f32,
    timestamp: Timestamp,
}

AgentDeparted {
    node_id: u32,
    reason: DepartReason,  // Graceful | TtlExpired | TransportReset
    timestamp: Timestamp,
}

SwarmSnapshotApplied {
    added: Vec<u32>,
    removed: Vec<u32>,
    timestamp: Timestamp,
}
```

Notably absent (would be in scope if this context owned control, which
it does not): `AgentSpawnRequested`, `AgentKilled`, `SwarmInitialised`,
`SwarmReconfigured`. These belong to the agentbox bounded context.

## Commands accepted

This context's command surface is intentionally tiny. The only commands
are user-driven UI intents that originate inside VisionClaw:

- `ResolveClickThrough { node_id, cursor_world_position }` — constructs
  an `AgentActionEnvelope` (ADR-10 D3) and dispatches it on the
  session's chosen transport. Section 10 owns the envelope schema and
  the transport.
- `RequestHoverDetails { node_id }` — returns the latest `AgentStatus`
  plus an `IdentityReadModel` lookup. Pure read.

All other state changes arrive as *events* from the transport, not as
*commands* from the user. This is the read-mostly discipline that
keeps VisionClaw out of the control plane.

## Anti-corruption layer to Section 10 (External Integrations)

Section 10 owns the transport schema. This context defines an internal
event vocabulary (above) and an `IngressAdapter` that translates Section
10's wire frames into internal events. The adapter:

- Validates: rejects malformed frames at the boundary; the internal
  aggregates only ever see well-formed events.
- Normalises: timestamps are converted to the internal monotonic clock.
- Authenticates: rejects frames whose Nostr signature does not validate
  against the configured agentbox public key. Authentication is *the
  transport's* responsibility; the adapter only checks the result is
  green.
- Versions: a single major version field on the wire. Mismatched majors
  cause a hard fail with a user-visible reconnect prompt; minor
  differences are tolerated by ignoring unknown fields.

The adapter is the only code in this context that knows the wire format.

Wire → internal event mapping (the only authoritative dispatch table):

| Wire `type` (ADR-10 D1) | Internal event (this DDD) | Notes |
|--------------------------|----------------------------|-------|
| `snapshot` | `SwarmSnapshot` | Full-state, used on connect / reconnect. |
| `delta` | `AgentPositionUpdated` and/or `AgentStatusChanged` | Dispatch per changed field in payload. |
| `agent_added` | `AgentJoined` | Pure rename. |
| `agent_removed` | `AgentDeparted` | Pure rename. |
| `heartbeat` | `Heartbeat` | Liveness only; no graph mutation. |
| `communication` | `AgentCommunicated` | New in this sprint. |

Any unmapped wire `type` is logged once and dropped per D1's receiver
rules. The contract test exercises every row.

## Anti-corruption layer to Section 1 (GPU Physics)

The physics context (DDD-01) owns the GPU buffer and the simulation.
This context publishes domain events; an adapter inside Section 1
consumes them and projects to the buffer:

- `AgentJoined` → `physics::register_node(node_id, class=Agent, initial_position, class_mass)`.
- `AgentPositionUpdated` → `physics::apply_position_correction(node_id, position)`.
  Implementation detail (lives in Section 1): correction is *not* a hard
  set; it's blended into the simulated position so the physics state
  remains continuous.
- `AgentDeparted` → `physics::deregister_node(node_id)`.

This context never touches the GPU buffer directly. The class flag is
the only piece of agent-specific information the physics layer sees.

## Anti-corruption layer to Section 4 (Rendering)

The rendering context (Section 4) owns `AgentCapsule` geometry, material,
hover overlay glass, and the agent edge cylinder. This context provides
the instance buffer and the hover-data lookup:

- `AgentSet` exposes `instance_buffer()` returning a packed buffer of
  `{ node_id, colour_token_index, capability_bitmask, status_alpha }`
  per agent. Rendering reads it; it does not branch on type strings.
- `CommunicationEdge` and `MembershipEdge` expose iterators that
  feed two separate instanced meshes (transient + persistent). Decay
  computation lives in this context; rendering reads `alpha` as a
  precomputed scalar.
- `RequestHoverDetails` is the only entry point for the hover overlay
  to read live status; rendering does not subscribe to telemetry events
  directly.

## Anti-corruption layer to Section 8 (Ontology / Graph Data)

Section 8 owns the durable identity record schema in Oxigraph. This
context's `IdentityReadModel` is a thin read adapter over Section 8's
read port (defined in Section 11's persistence trait surface). The
adapter:

- Maps Section 8's RDF terms to this context's `AgentIdentity`
  fields. The mapping table lives with the adapter, not in either
  upstream context.
- Caches results per `bots.identity_cache_seconds`.
- Never writes. Telemetry observations never feed back to durable
  identity; durable identity is created by separate agentbox/admin
  workflows out of scope here.

## Read models

Two read-side projections fan out of `AgentSet`:

- **Scene projection** — the instance buffer for rendering and the
  membership/communication edge iterators. Computed each tick.
- **Operator dashboard projection** — total agents, active agents,
  average success rate, total tokens. Computed each tick from
  `AgentStatus` rollups. Replaces the existing `multiAgentMetrics`
  on `BotsDataContext` but with telemetry-derived figures only (no
  client-fabricated values).

## Concurrency model

- The transport adapter runs on the WebSocket worker thread (Section 3).
- Telemetry events are posted to a coalescer queue.
- `TelemetryStream::tick()` runs on the main thread inside
  `requestAnimationFrame` and applies up to `bots.coalescer_max_batch`
  events to `AgentSet` synchronously.
- Domain events are then fanned out to Section 1's adapter (which
  enqueues to the physics actor mailbox) and Section 4's adapter
  (which updates the instance buffer for the next render frame).

The single-threaded coalescer flush is what keeps `AgentSet` free of
locks despite the multi-source input. It is the same single-flight
discipline ADR-03 mandates for client state generally.

## Notable non-events (things that are deliberately not domain events)

- `AgentSpawned` / `AgentSpawnRequested` — control plane; agentbox.
- `SwarmInitialised` — control plane; agentbox.
- `AgentReconfigured` — control plane; agentbox.
- `TaskCreated` / `TaskCompleted` — task model is agentbox's. If a
  task-completion is visualisable, it surfaces as an `AgentStatusChanged`
  with the new status; the task entity itself does not exist here.

Keeping these out of the event vocabulary is the structural enforcement
that this context cannot accidentally grow back into a control plane.
