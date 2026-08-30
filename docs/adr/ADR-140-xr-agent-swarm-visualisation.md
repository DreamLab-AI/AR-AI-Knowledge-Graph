# ADR-140: XR Agent-Swarm Visualisation — Embodied Agents, Work Beams, Swarm Tab

**Status:** Proposed (P1 shipped)
**Date:** 2026-08-30
**Deciders:** VisionClaw XR specialist, VisionClaw platform team
**Related:**
- ADR-136 (desktop OpenXR Vive validation target — the client this lands in)
- ADR-137 (XR render offload + runtime quality dials — RenderStore owns per-frame math)
- ADR-138 (GPU force-channel registry — the layout engine agents hover within)
- ADR-139 (immersive interaction adoption programme — the HUD/teleport machinery reused)
- ADR-059 (agent-action `0x23` beam wire, server side — the contract this consumes)
- agentbox ADR-071 (swarm-telemetry contract, producer side — what agents must emit)
- `docs/reference/binary-protocol.md` (the `/wss` wire this rides)

## TL;DR

The desktop web client shows "agent swarms working on nodes"; the Vive XR client
does not. This ADR ports that capability, **redesigned for embodiment**: agent
capsules glide to hover near the node they are working on; a bright directional
**work beam** streams from agent to target node; status shows via a 4-channel
halo colour and the agent's task line; a **Swarm tab** on the HUD control centre
gives a roster with tap-to-teleport. All per-frame cost stays in the Rust
RenderStore — zero new GDScript frame work (ADR-137 discipline).

The core design decision is a **motion-authority split**: the server owns
*which* node an agent works on and its status/task; the XR client owns *where in
the room* the agent hovers. This anchors agents to their target node (the
embodiment goal) with **zero server change**, because the XR client already
knows every node's position in the RenderStore.

## Context

**Wire reality (investigated, not assumed):**

- **`0x23 AGENT_ACTION`** (server `MessageType::AgentAction = 0x23`,
  `src/utils/binary_protocol.rs`) carries `source_agent_id → target_node_id`
  (KG-space) + `action_type` + optional `{intent}` payload. Produced by
  `AgentBeamActor` (`src/actors/agent_beam_actor.rs`), coalesced into one batch
  frame, and fanned to **every** `/wss` client via
  `ClientCoordinatorActor::broadcast_to_all` — the *same ungated binary dispatch
  as position frames_. The XR socket already receives it; no gating fix needed.
- **`0x20 AGENT_STATE_FULL`** is a client-side type definition with **no server
  producer** — it is never emitted. Not consumed.
- **Agent status + `current_task`** reach **no** `/wss` client. The only
  producer (`AgentVisualizationWs`, `/api/visualization/agents/ws`) is a separate
  socket the XR client never opens, and it emits empty placeholder data; the
  real-data `MultiMcpVisualizationActor` is never started. Even desktop's status
  pipeline is a stub today.
- Agent positions are server-authoritative on the binary wire (high-bit
  `AGENT_NODE_FLAG = 0x80000000`), but they encode a desktop force layout, not
  embodied proxemics, and are not wired into the XR avatar path.

**XR reality:** agents render as `AgentAvatar.tscn` scene nodes placed by a
`ProxemicsSolver` social arc **around the user** (`rust/src/proxemics.rs`), not
near their target node. The `AgentMulti` capsule MultiMesh (`GraphScene.tscn`) is
reserved but unused. The `edge_flow.gdshader` (TIME-driven travelling pulse) is
directly restyleable into a work beam. The Wave-2 teleport glide
(`graph_scene.gd` `_teleport_to_node`/`_update_teleport`) and the data-driven HUD
tab loop (`hud.gd` `TAB_ORDER`/`_build_*_page`) are reusable as-is.

## Decision

1. **Consume the existing `0x23` beam frame** for the agent→target-node link and
   the task line (from the payload `intent`). Do **not** invent new wire fields.
2. **Motion-authority split.** Server = source of truth for target node + status
   + task. XR client = authority for the agent's hover position, computed in the
   RenderStore as `position_of(target_node_id) + deterministic_orbit(agent_id)`,
   with the existing proxemics arc as the idle fallback.
3. **Derive a 4-channel status** (`idle | working | blocked | done`) rather than
   adding wire fields: an inbound action ⇒ `working`; the JSON `state` channel
   (when a producer exists) refines it via `render_store::agent_status_code`.
   The status field is an unconstrained `String`, so `blocked`/`done` need no
   schema change — only that producers may set them (see agentbox ADR-071).
4. **All per-frame cost in Rust.** An `agent_registry` in the RenderStore feeds
   the hover glide, a `build_beam_buffer()` on the reserved `AgentMulti`
   MultiMesh (restyled `edge_flow` material), and the Swarm roster — one buffer
   push per phase, GDScript never loops per instance.

## Phases (each shippable)

- **P1 — Data plane (SHIPPED).** XR parses the `0x23` batch frame into an
  `agent_registry` (target, action, derived status, task-from-intent); diagnostics
  `#[func]`s (`agent_count`, `last_agent_action_age_ms`, `agent_actions_total`)
  verify liveness from the HP log before any visuals. 4 unit tests; XR crate green.
- **P3 — Work beam.** `build_beam_buffer()` + restyled `edge_flow` on `AgentMulti`,
  one thin cylinder per active agent→target, status code in `INSTANCE_CUSTOM.a`.
- **P2 — Embodiment.** Hover-to-target in `hunt()` with deterministic orbit
  fan-out; status→halo colour; proxemics arc fallback when idle.
- **P5 — Swarm tab.** `_build_swarm_page()` roster (status dot, name/role, target
  label, task line); row tap → `teleport:agent:<id>` reusing the glide.
- **P4 — Task line.** `current_task` in the proximity label / agent badge.
- **Follow-up (server).** A real status/task producer: an `agent:state`
  `BroadcastMessage` text frame (built like `broker:new_case`) emitted from the
  agent-state producer, riding the existing `/wss` text path → GDScript calls the
  `apply_agent_state` `#[func]` (already present). Sequence: P1 → P3 → P2 → P5 → P4.

## Consequences

- **Positive:** embodiment with zero server change for the core loop; the beam
  affordance rides a wire that already exists; XR stays authoritative over
  proxemics; per-frame budget honoured (ADR-137).
- **Negative / accepted:** richer status (`blocked`/`done`) and a durable task
  line depend on a server producer that does not exist yet (documented follow-up,
  not blocking). Until then, status is `working`/`idle` derived from action
  recency, and the task line is best-effort from the action intent.
- **Contract dependency:** the visualisation only "lights up" if agentbox-side
  agents emit the telemetry in agentbox ADR-071. This ADR is the consumer; that
  one is the producer.
