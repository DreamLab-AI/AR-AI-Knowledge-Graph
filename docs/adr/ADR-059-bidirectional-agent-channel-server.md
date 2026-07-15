# ADR-059: Bi-directional URI-keyed agent activity channel (VisionClaw side)

**Status:** Accepted — Phase 1 + Phase 2a (authenticated ingest) implemented & verified (2026-05-29); Phase 2b **beam render landed** (`agent_beam_actor.rs`, `0x23` frames live end-to-end, 2026-07-15) with the **gluon attractive force deferred** (needs a dedicated transient-edge GPU buffer — see Addendum 2026-07-15); the `:9500` state-poll cutover remains a follow-on
**Date:** 2026-04-28 (Phase 1 design log appended 2026-05-29)
**Author:** VisionClaw platform team
**Supersedes:** — (initial bi-directional design; agent_monitor_actor REST polling remains until Phase 2)
**Related:**
- VisionClaw ADR-048 (dual-tier KGNode/OntologyClass + BRIDGE_TO)
- VisionClaw ADR-050 (sovereign-model: visibility + owner_pubkey + opaque_id + bit-29 privacy flag)
- VisionClaw ADR-058 (MAD → agentbox migration, side-by-side ports)
- Agentbox ADR-013 (canonical URI grammar — `did:nostr:<pubkey>`, `urn:agentbox:<kind>:[<scope>:]<local>`)
- Agentbox ADR-005 (pluggable adapter architecture — events slot)
- Agentbox ADR-008 (privacy filter / OPF middleware)
- Agentbox ADR-009 (embedded Nostr relay; future durable channel)
- **Pair:** Agentbox ADR-014 (bi-directional graph-state ingress for agent reaction — the agentbox side of this contract)

## TL;DR

VisionClaw exposes one new WebSocket endpoint, `/wss/agent-events`, and accepts an additive event envelope that carries the existing numeric IDs **plus** optional `source_urn`, `target_urn`, and `pubkey` fields (per agentbox ADR-013 grammar). Inbound `agent_action` events drive a hybrid **beam + gluon** spring effect for `duration_ms` against the existing GPU semantic-forces kernels — no new CUDA, no new buffers: the beam is a transient coloured edge and the gluon is the attractive spring force that same transient edge already exerts (the `class_charge`-modulation gluon of the original draft is retracted — see §4 and Design log Finding 5). Outbound user-interaction events (`focus`, `select`, `hover`, `drag`) push back to agentbox so agents can react to user attention. Identity is phased: optional pubkey from Phase 1; fail-closed NIP-26 delegation enforcement deferred to a successor ADR. Server-side visibility filtering at the binary encoder is staged separately.

## Context

The current agent → graph path is **REST polling**: `agent_monitor_actor.rs:169-171` calls `Management API:9090` every 3 s, builds `AgentStatus` records, and renders capsule nodes in the GPU spring system. This is one-way and lossy:

1. **Pubkey is dropped.** Inbound events carry `source_agent_id` as a 32-bit hash (`management-api/utils/agent-event-publisher.js:165-189`); the user/operator/agent identity disclosed in agentbox via `did:nostr:<pubkey>` (ADR-013) never reaches VisionClaw.
2. **No URN.** Both `source_agent_id` and `target_node_id` are ints. The viewer (S12) cannot follow links to external surfaces (skills registry, pod credentials, ADRs) because the canonical name is missing.
3. **No reaction path.** Agents in agentbox cannot observe user activity (which node is focused, which is selected). Agent positions are server-driven and ignore user attention.
4. **Bridge mismatch.** Agentbox's `management-api/utils/agent-event-bridge.js` tries to connect TCP `127.0.0.1:9500`, expecting a VisionClaw MCP listener that does not exist; VisionClaw's MCP server actually listens on `:3001` inside `visionclaw_container`. The reconnect storm in `tab 4 pane 0` is the visible artefact.

ADR-050 already provides the identity primitives on the storage side (`KGNode.owner_pubkey`, `KGNode.visibility`, `KGNode.opaque_id`, bit-29 opacification). Agentbox ADR-013 provides the URN grammar on the producer side. The wire format is the missing piece.

## Decision

VisionClaw introduces a single bi-directional channel and a single additive event envelope. Six concrete commitments follow.

### 1. Transport

A new WebSocket endpoint `/wss/agent-events` mounts in `src/main.rs` next to `/wss` (binary positions). Subprotocol token: `vc-agent-events.v1`.

- Frames are JSON text by default. A `binary=true` query param negotiates the existing 0x23 binary frame (agent_event_publisher.js:165-189) for parity with agentbox.
- One socket per session. Authentication uses the same `RequireAuth` extractors as `/api/...` (NIP-98 if `NIP98_OPTIONAL_AUTH=true`, else session cookie + CSRF). The authenticated `pubkey` (if any) becomes the **session pubkey**, surfaced through Phase 5 visibility filtering.
- Real MCP TCP on port 9500 is **not** added; the `agent-event-bridge.js` connect target is changed to `ws://visionclaw_container/wss/agent-events` instead.

The Nostr-relay durable channel (agentbox `[sovereign_mesh].relay`, ADR-009) is reserved for cross-session state and signed authority grants in Phase 5; the hot path is WebSocket-only.

### 2. Event envelope (additive, backward-compatible)

```jsonc
// inbound: agent → VisionClaw
{
  "version": 3,
  "type": "agent_action",
  "id": 12345,
  "timestamp": 1714312345678,
  "source_agent_id": 7,                                      // legacy int, retained
  "source_urn": "did:nostr:abc...",                          // optional, ADR-013
  "target_node_id": 4242,                                    // legacy int, retained
  "target_urn": "urn:visionclaw:kg:<hex-pubkey>:<sha256-12-hex>",  // optional, ADR-050
  "action_type": 1,                                          // 0..5 per AgentActionType
  "duration_ms": 250,
  "pubkey": "abc...",                                        // optional did:nostr hex
  "metadata": { /* free-form */ }
}
```

The legacy fields (`source_agent_id`, `target_node_id`, `action_type`, `duration_ms`) keep their semantics from `agent-event-publisher.js:11-18` and `agent_visualization_protocol.rs:6-28`. The new fields (`source_urn`, `target_urn`, `pubkey`) are **optional in Phase 1 and Phase 2**; their absence falls back to existing rendering. They become **required in Phase 5** when fail-closed identity attribution is enforced.

### 3. Outbound user-interaction events (first slice of bi-directionality)

Phase 2 ships outbound user-interaction events on the same socket. These are **transient, durational, and tied to the live UI session** — they are NOT persisted to Neo4j and do NOT touch ADR-050 storage.

```jsonc
// outbound: VisionClaw → agent
{
  "version": 1,
  "type": "user_interaction",
  "kind": "focus" | "select" | "hover" | "drag",
  "session_id": "uuid-v4",
  "session_pubkey": "abc...",                                // optional did:nostr hex
  "target_node_id": 4242,
  "target_urn": "urn:visionclaw:kg:...",                     // present if known
  "duration_ms": 1500,                                       // expected lifetime
  "timestamp": 1714312345678
}
```

`focus` fires when a node enters the camera centre band (>1.5 s dwell, hysteresis-debounced). `select` fires on click/tap. `hover` fires on raycast ≥ 250 ms. `drag` fires for the duration of an interactive grab. The agentbox subscriber consumes these per Agentbox ADR-014.

### 4. Spring-system semantic — hybrid beam + gluon

When an `agent_action(action_type, target_node_id, duration_ms)` arrives:

1. **Beam (visual edge)**: a transient edge `(agent_node)-[:ACTION { action_type, started_at }]->(target_node)` is appended to the spring graph for `duration_ms`. The renderer maps `action_type` → colour:

   | action_type | name | colour |
   |---|---|---|
   | 0 | QUERY | blue (#3b82f6) |
   | 1 | UPDATE | yellow (#eab308) |
   | 2 | CREATE | green (#22c55e) |
   | 3 | DELETE | red (#ef4444) |
   | 4 | LINK | purple (#a855f7) |
   | 5 | TRANSFORM | cyan (#06b6d4) |

   These match the colour conventions in `agent-event-publisher.js:11-18` so agentbox's `/v1/agent-events/types` endpoint stays the source of truth.

2. **Gluon (transient attractive force)**: the **transient beam edge itself** carries the attractive pull. Because the spring/semantic-forces kernel (`semantic_forces_actor.rs:30-175`) already resolves an attractive force along every edge, appending the transient `(agent)-[:ACTION]->(target)` edge for `duration_ms` *is* the gluon — the agent is drawn toward `target_node_id` for the edge's lifetime with **no per-node buffer write**. The kernel, the 176-byte `SemanticConfigGPU` struct, and all `unified_gpu_compute` buffers are unchanged.

   *Design correction (2026-05-29):* an earlier draft modulated the per-node `class_charge` buffer for the gluon. That is **not implementable as written** and is retracted — see Design log "Finding 5". `class_charge` exists (`construction.rs:55`, default `1.0`) but it is **bulk ontology-clustering metadata loaded at construction** (`execution.rs:573`), uploaded only via `upload_class_metadata(class_ids, class_charges, class_masses)` over the *full* `num_nodes` array (`memory.rs:84-126`). There is no per-node update path; modulating one agent's charge would require a whole-array re-upload per beam and would corrupt domain clustering for `duration_ms`. The transient edge is the kernel-native mechanism and needs no new buffer.

3. **Despawn**: a single `agent_action_despawn_actor` reaps expired transient edges after `duration_ms`. The reap pass is bounded (≤ 1 ms / tick) and runs on the existing actor scheduler. (No charge buffer to zero — the gluon lives and dies with the edge.)

This deliberately reuses the existing GPU pipeline. No CUDA changes. No new buffers in `unified_gpu_compute`.

### 5. Identity attribution — phased

| Phase | Pubkey behaviour | Server enforcement |
|---|---|---|
| 1 | optional in payload; ignored by renderer | none |
| 2 | optional; renderer tints capsule by `hash(pubkey)` colour when present | none |
| 3 | optional; rendered + recorded in agent_action audit table | none |
| 4 | required when payload claims to mutate ADR-050 owned KGNodes | reject `target_urn` mismatch |
| 5 | required + signed + NIP-26 delegation chain validated | fail-closed; deferred to **ADR-061** |

Phase 5 is out of scope for this ADR. ADR-061 will own the NIP-26 delegation grammar and signature verification (depending on agentbox ADR-013 §R2 scope-bearing rules).

### 6. Server-side visibility filtering at the binary encoder

Today the binary frame ships every node, with bit-29 set on private nodes (ADR-050). This wastes bandwidth and lets a determined client correlate opaque IDs across sessions. Phase 4 introduces:

- The session pubkey (from `RequireAuth`) is attached to the WS connection context.
- `socket_flow_handler` (or successor) filters at frame build time:

  ```
  WHERE n.visibility = 'public'
     OR n.owner_pubkey = $session_pubkey
  ```

- Filtering is **fail-closed**: missing session pubkey ⇒ public-only graph. This matches ADR-050 §Visibility transitions.

This is documented for completeness; the change ships as a separate small ADR (**ADR-060: Owner-pubkey filtered binary encoder**) so it can land independently.

## Phasing

| Phase | Deliverable | Scope of code change | Risk |
|---|---|---|---|
| 1 | Canonical ingest schema mirror in new `src/agent_events/schema.rs` (the inbound `notifications/agent_action` envelope with `source_urn`/`target_urn`/`pubkey`); agentbox `agent-event-publisher.js` populates new fields through one builder. **Done 2026-05-29** — see Design log. | ~30 lines each side. | Low. Backward-compatible. |
| 2a | **Done 2026-05-29.** Authenticated `/wss/agent-events` ingest handler in VisionClaw (`src/agent_events/ingest.rs`): token-validated upgrade (`NostrService::get_session`), subprotocol `vc-agent-events.v1`, parse → `is_canonical()` validate → publish to a process-global broadcast hub (`src/agent_events/hub.rs`). agentbox still switches `agent-event-bridge.js` connect target from `tcp://127.0.0.1:9500` to `ws://…/wss/agent-events`. **No GPU/render code** — see Design log Phase 2a. | ~250 lines server (handler + hub + tests). | Low. Additive endpoint; render-decoupled; cargo-verified, 7/7 tests. |
| 2b | Beam + gluon render actor subscribing to `hub::subscribe()` (§4: transient `Edge { transient: bool }` carries both the beam *and* the gluon attractive force + despawn reaper — **no `class_charge` write**, see §4 correction). **Blocked-finding:** the live agent-action render substrate is latent (see Design log), so this is its own increment, not a bolt-on. Separately: the `:9500` MCP-TCP poll carries agent *state snapshots*, not `agent_action` — retiring it requires the WS to also carry state, a contract expansion tracked here. | ~200 lines server GPU/actor + state-channel design. | Medium. Touches spring system + `Edge` struct only; no GPU buffer changes. |
| 3 | Outbound `user_interaction` events from client → server → WS broadcast → agentbox subscriber (Agentbox ADR-014). | ~150 lines client + 80 server. | Low–medium. |
| 4 | Server-side visibility filter at binary encoder (**ADR-060**). | ~80 lines, single handler. | Medium. Test matrix grows. |
| 5 | Mandatory + signed identity + NIP-26 delegation (**ADR-061**). | TBD. | High. New crypto path. |

Phase 1 + Phase 2 land within one sprint. Later phases are independently scheduled.

## Consequences

**Positive.**

- Agentbox's existing 6-action-type emission surface (`agent-event-publisher.js:11-18`) requires no rewrite — only an additive payload.
- The hybrid beam + gluon visual maps directly onto the existing semantic-forces GPU kernel; no new CUDA, no new buffers.
- ADR-050 ownership data finally enters the wire format via optional URN, unblocking the URI resolver (`src/handlers/uri_resolver_handler.rs`) to do real Neo4j lookups in Phase 1+ work.
- The agent-event-bridge ECONNREFUSED storm is resolved without standing up a TCP listener that VisionClaw doesn't want to maintain.
- User-interaction outbound events make agents legibly user-aware — capsules visibly drift toward the user's focus for the duration.

**Negative.**

- Two new structures to keep in sync: `AgentActionEvent` (VisionClaw side) and the JSON-RPC notification (agentbox side). Mitigation: agentbox `agent-event-publisher.js` remains the canonical schema source; VisionClaw mirrors it in `src/agent_events/schema.rs` (landed 2026-05-29). Drift is fenced by a shared cross-repo fixture: the exact `createMcpNotification` output is asserted in `tests/sovereign/agent-event-notification.test.js` (agentbox) **and** parsed by the `#[cfg(test)]` fixture in `schema.rs` (VisionClaw). The mirror also carries `to_binary_event()`, the Phase-2 projection onto the identity-blind `0x23` frame.
- Phased identity means Phase-1 events with no pubkey are indistinguishable from spoofed ones for ~3 phases. Mitigation: visibility filter (Phase 4) is server-side, so unauth'd writes can't clobber ADR-050 owner-private nodes regardless of payload contents.
- Beam edges live alongside ADR-048 `EDGE`/`BRIDGE_TO`/`SUBCLASS_OF`/`RELATES` types but are **transient** and **not persisted to Neo4j**. The renderer must distinguish persistent vs transient edges — handled by a new `transient: bool` flag on the `Edge` struct and skipped by the `load_all_edges` query in `neo4j_graph_repository.rs:431-440`.

**Reversible?** Yes for Phases 1–3 (new fields + new endpoint, additive). Phase 4 (visibility filter) reversibility requires keeping the unfiltered code path behind a feature flag for one release.

## Open questions

1. Should `user_interaction` events also accept an inbound mirror form, so agentbox can echo synthetic interactions for testing? (Recommend yes, behind dev-only header.)
2. Beam colour for `action_type` outside 0–5: should the renderer reject the frame or default to grey? (Recommend default-grey + warn-log.)
3. Phase-2 backpressure: if the WS client falls behind and the server queue exceeds N frames, drop oldest or coalesce by `target_node_id`? (Recommend coalesce-by-target — the visual is duration-based, last-write-wins is correct.)

## Design log (real-time)

### 2026-07-15 — Addendum: Phase 2b beam render LANDED; gluon deferred (shipped-code correction to Finding 5)

Finding 5 (below) concluded the gluon was "free" — that appending the transient
`(agent)-[:ACTION]->(target)` edge *is* the attractive force, with zero GPU-buffer
changes. Shipping Phase 2b proved that even this reduced claim is **not implementable
on the current GPU substrate**. This addendum records the correction and the render
subsystem as actually built; it annotates, and does not rewrite, the entries below.

**(a) The "gluon is free / no new buffer" claim is wrong — the gluon is DEFERRED.**
§4, §4.2, the TL;DR, and Finding 5 all assert the transient beam edge delivers the
attractive pull with no new buffer. The shipped `src/actors/agent_beam_actor.rs:97-173`
(see the `gluon_deferral_note` anchor) defers the gluon because there is **no
incremental edge-insert path on the GPU**:

- GPU edges live in a **packed CSR layout** (`row_offsets` / `col_indices` /
  `edge_weights`), uploaded wholesale by `unified_gpu_compute::memory::initialize_graph`
  / `upload_edges_csr`. Injecting one transient edge forces a `resize_buffers`
  reallocation and a full re-upload of all three CSR arrays.
- `AddEdge` / `RemoveEdge` (`graph_state_actor.rs`) mutate only the in-memory
  `node_map`; they do not propagate to the GPU until a full `BuildGraphFromMetadata`
  rebuild. No per-edge GPU mutation message exists.
- The SSSP and Louvain/community kernels read the same CSR buffers; a mid-flight
  resize/re-upload would race concurrent kernels and destabilise the simulation.
- (The `class_charge` buffer Finding 5 already retracted still does not exist — only
  `class_ids:i32` + `class_masses:f32` under the `physics-v2` gate, neither a per-edge
  force handle.)

Correct future fix: a new incremental **`UpsertTransientEdge { src, tgt, weight,
ttl_ms }` GPU message** that appends into a **dedicated, separate transient-edge
buffer** the spring kernel sums alongside the static CSR, plus a TTL sweep that zeroes
expired entries — with no reallocation of the static CSR. Until that lands **the beam
ships alone**: it embodies the action visually but exerts no attractive force.
Everywhere this ADR (and README / websocket-protocol / subsystems / triptych) says
"beam + gluon delivered", read "beam delivered; gluon deferred".

**(b) Status / Phasing update — Phase 2b beam render LANDED; `0x23` is LIVE, not dead code.**
Finding 4 (below) recorded the `0x23` agent-action frame as having no live broadcaster.
That is no longer true:

- `AgentBeamActor` (`src/actors/agent_beam_actor.rs`) subscribes to
  `agent_events::hub::subscribe()`, projects each envelope onto the identity-blind
  `0x23` frame via `AgentActionEnvelope::to_binary_event` → `AgentActionEvent::encode`,
  and fans it out through `ClientCoordinatorActor`.
- It is **started once at boot** — `src/app_state.rs:681-682`
  (`AgentBeamActor::new(client_manager_addr.clone()).start()`).
- Broadcast is handled by `ClientCoordinatorActor`'s `BroadcastAgentActionFrame`
  handler (`src/actors/client_coordinator_actor.rs:1437-1453`), reusing the exact
  `BroadcastNodePositions` binary dispatch loop.
- So the `0x23 AGENT_ACTION` frame is **live end-to-end** (ingest → hub → beam actor →
  client). Phasing row 2b's "beam" half is DONE; only its "gluon" half is deferred per (a).
- `MultiMcpVisualizationActor` is **still never `.start()`ed** and is absent from
  `AppState` (only referenced in `src/actors/mod.rs` and its own module) — Finding 4's
  note on that actor stands.

**(c) Client render subsystem (as built).** The browser half of the embodiment loop:

- `client/src/features/visualisation/components/TransientBeamsLayer.tsx` — renders the
  transient beams from decoded `0x23` frames.
- `client/src/features/visualisation/semanticEncoding.ts` — **single source of truth**
  for beam colour and shape per action verb (query / update / create / delete / link /
  transform), shared with the memory-burst encoding.
- `client/src/features/visualisation/hooks/useTransientBeams.ts` — the subscription hook.
- `client/src/store/transientBeamStore.ts` — beam state store fed by the `0x23` decode
  path (`store/websocket/binaryProtocol.ts` → `decodeAgentActions`).
- **Legacy renderer removed (2026-07-15):** the parallel bézier/particle renderer chain —
  `AgentActionVisualization.tsx`, `ActionConnectionsLayer.tsx`,
  `useAgentActionVisualization.ts`, `useActionConnections.ts`, and the VR variants
  `VRAgentActionScene.tsx`, `VRActionConnectionsLayer.tsx`, `useVRHandTracking.ts`,
  `useVRConnectionsLOD.ts` — was **deleted** by operator decision for maintainability
  (recoverable from git history). `TransientBeamsLayer` + `semanticEncoding` is now the
  **sole** action renderer on both desktop and VR; the VR path receives it automatically
  because `VRGraphCanvas` mounts `GraphManager`, which mounts `TransientBeamsLayer`. The
  interim opt-in `visualisation.graphTypeVisuals.agent.legacyActionBeams` toggle (Control
  Centre → Agents → Behaviour), added briefly the same day, was removed with the renderer.

### 2026-05-29 — Finding 5: gluon is a transient edge, not a `class_charge` modulation (keystone)

The §4 gluon was originally specified as "modulate the agent capsule's `class_charge`
for `duration_ms`". Verifying the GPU substrate before scheduling Phase 2b proved this
**unimplementable as written**:

- `class_charge` is a real `DeviceBuffer<f32>` (`construction.rs:55`, default `1.0`), **but**
  it is uploaded only through `upload_class_metadata(class_ids, class_charges, class_masses)`
  (`memory.rs:84-126`), which requires the **full `num_nodes` array** and is the
  **ontology-clustering** metadata loaded **at construction** (`execution.rs:573` comment is
  explicit: "ontology metadata buffers loaded at construction"). There is no per-node mutation
  API.
- Modulating one agent's charge per beam would therefore mean re-uploading the entire class
  array on every `agent_action`, and — worse — it would **corrupt domain clustering** for
  `duration_ms`, because the kernel reads `class_charge` as a clustering input, not a transient
  per-agent force handle.

**Decision (keystone):** the **transient beam edge is the gluon**. The spring kernel already
resolves an attractive force along every edge, so appending the transient
`(agent)-[:ACTION]->(target)` edge for `duration_ms` delivers the agent→target pull for free,
with **zero GPU-buffer changes** — fully consistent with §4's "no new CUDA, no new buffers"
promise (which the `class_charge` plan actually violated). §4, §4.3 (despawn), and Phasing row
2b are corrected accordingly; PRD-014 §8 and the `src/agent_events/ingest.rs:16` module comment
carry the same stale claim and are flagged for the code lane (the comment is a one-line
correction owned by the Phase-2b implementer, not this docs pass). This reviving of the existing
`0x23 AGENT_ACTION` frame end-to-end — rather than inventing a new frame — plus the
edge-as-gluon model, is the keystone of the embodied-loop wiring.

*(Aside: the `physics-v2` Cargo feature gates the separate engine modules `src/physics/*` and
`src/gpu/buffers.rs` — a parallel layout-engine refactor lane, unrelated to the gluon. The
gluon ships on the live kernel, not behind `physics-v2`.)*

### 2026-05-29 — Phase 2a landed (ingest seam); render substrate found latent; Phase 2 split

**Phase 2a (authenticated ingest) landed and cargo-verified.** Three new pieces in
`src/agent_events/`: `ingest.rs` (the `/wss/agent-events` actix WS actor + token-validated
upgrade handler, registered in `main.rs` next to `/wss`), `hub.rs` (a process-global
`tokio::sync::broadcast` seam), and ingest unit tests. The handler parses each text frame
against the Phase-1 `AgentActionNotification` mirror, validates `is_canonical()`, and
publishes the envelope to the hub. `cargo check --lib`/`--bins` clean (zero new warnings);
`cargo test --lib agent_events` → 7/7 pass (4 schema + 3 ingest) via
`docker exec visionclaw_container`. **This closes the X2 consume-side debt**: VisionClaw now
*consumes* the pushed `notifications/agent_action` it previously dropped (Finding 1).

**Finding 4 — the agent-action RENDER substrate is latent, the agent-STATE path is the
`:9500` poll.** Phase 2 was scoped in the original table as "new handler + beam+gluon reaper,
~200 lines". On inspection that conflates two unrelated substrates:
- *Agent state* (which agents exist, cpu/health/status) IS live — but via the **deprecated
  `:9500` MCP-TCP poll**: `services/bots_client.rs` spawns a 2 s `query_agent_list()` interval
  and caches it; `get_agent_visualization_snapshot` (`/visualization/agents/snapshot`) reads
  that cache. So `:9500` is load-bearing for *state*.
- *Agent actions* (the transient beams §4 renders) have **no live consumer or renderer**.
  `MultiMcpVisualizationActor` is never `.start()`ed and is absent from `AppState`; the
  outbound `0x23` binary broadcast (`binary_protocol::AgentActionEvent::encode`) is dead code
  never called to broadcast; the only live agent-viz WS (`agent_visualization_ws` at
  `/visualization/agents/ws`) emits an **empty** `Vec<AgentStatus>` placeholder.

**Decision — split Phase 2 into 2a / 2b (escape-hatch respected).** Bolting the beam+gluon
GPU wiring onto a dead render path in the same change would be speculative debt against
unverified substrate. So Phase 2a lands only the *verifiable* seam — receive, authenticate,
validate, buffer (broadcast) — which is the actual federation-boundary debt. Phase 2b owns
the render decision (wire a hub-subscribing actor into the spring system + the transient
`Edge` flag + despawn reaper) and is independently schedulable. The `hub` is the explicit
seam between them: ingest publishes today, render subscribes later, neither imports the other.

**Consequent correction — `:9500` retirement is bigger than "switch the bridge target".**
agentbox's `agent-event-bridge.js` retargeting (Phase 2a) stops the *action* push hitting a
non-existent TCP listener, but the `bots_client` *state* poll still dials `:9500`. Fully
retiring `:9500` requires the WS contract to also carry agent **state snapshots** (a payload
distinct from the §2 `agent_action` envelope) — a contract expansion now tracked in Phasing
row 2b, not silently assumed done.

### 2026-05-29 — Phase 1 landed; producer convergence; two clarifying findings

**Producer half (agentbox) converged and verified.** `agent-event-publisher.js`
now has a *single* canonical wire-envelope builder (`createMcpNotification`);
the deprecated `agent-event-bridge.js` was hand-rolling its own
`notifications/agent_action` literal that silently dropped the ADR-013 identity
(`source_urn`/`target_urn`/`pubkey`) it had just computed. The bridge now routes
through the one builder, so identity reaches the wire on every transport.
Guarded by `tests/sovereign/agent-event-notification.test.js` (agentbox commit
`8005fc3f`).

**Consumer half (VisionClaw) Phase 1 landed.** New `src/agent_events/schema.rs`
mirrors the canonical envelope with `#[cfg(test)]` round-trip + cross-repo
fixture tests. No transport yet (Phase 2). Pending a host build via the tmux-tab-6
loop before marking compile-verified.

**Finding 1 — the inbound path did not exist, not "was lossy."** §Context framed
the REST poll as one-way and lossy. On inspection VisionClaw had **no JSON
consumer of `notifications/agent_action` at all** — `agent_monitor_actor.rs`
polls a *list*, and `bots_client.rs` polls `:9500` for agent state; the pushed
`agent_action` notifications were never read by anything. So Phase 1's attach
point is a *new* ingest module (`src/agent_events/schema.rs`), **not** an
extension of the outbound `agent_visualization_protocol.rs` (which is the
server→browser viz init/update protocol, a different envelope). The Phasing
table row 1 is corrected accordingly.

**Finding 2 — the binary `0x23` frame is identity-blind by design, and should
stay so.** `utils/binary_protocol.rs::AgentActionEvent` is a 15-byte header of
numeric ids only (`source_agent_id`/`target_node_id`/`action_type`/`timestamp`/
`duration_ms`) with no URN/pubkey. Identity belongs in the JSON *ingest*
envelope and is resolved server-side (URN → numeric id, owner persisted per
ADR-050) **before** the GPU binary frame is emitted to the browser. The mirror's
`to_binary_event()` makes this projection explicit: identity in, numeric out.
This keeps the hot GPU wire unchanged (no new buffers, consistent with §4) while
still closing the federation-boundary identity gap on the ingest side.

**Finding 3 — the `:9500` bridge mismatch is now a deprecation, not a bug.**
§Context point 4 noted agentbox's bridge dialled a non-existent `:9500` MCP
listener. agentbox ADR-014 deprecates that bridge; it is now gated behind
`ENABLE_MCP_BRIDGE` (default off) and, when on, emits through the canonical
builder. Phase 2 retires it entirely in favour of the WS subscriber. No VisionClaw
`:9500` listener will ever be built — confirming the original decision to reject
Alternative C (MCP-TCP listener).

## References

- Code:
  - VisionClaw: `src/actors/agent_monitor_actor.rs:169-420`, `src/services/agent_visualization_protocol.rs:6-227`, `src/actors/multi_mcp_visualization_actor.rs:33-120`, `src/actors/gpu/semantic_forces_actor.rs:30-175`, `src/handlers/uri_resolver_handler.rs:1-100`, `src/uri/mod.rs:1-42`, `src/middleware/auth.rs:1-150`, `src/handlers/socket_flow_handler.rs` (Phase 4)
  - Agentbox: `management-api/utils/agent-event-publisher.js:11-224`, `management-api/utils/agent-event-bridge.js:1-150`, `management-api/routes/agent-events.js:51-475`, `management-api/lib/uris.js:72-232`, `management-api/middleware/auth.js:33-99`
- Wire format colour palette: `management-api/utils/agent-event-publisher.js:11-18` (canonical)
- Bit-29 privacy flag: ADR-050 §Opaque ID + Visibility, lines 98-130
- Spring kernel struct: `semantic_forces_actor.rs:30-175` (176-byte SemanticConfigGPU)
