# PRD-023: Gap-Close Sprint — VisionClaw Slice

**Owner:** DreamLab AI / VisionClaw platform team
**Status:** Proposed
**Date:** 2026-07-08
**Governed by:** [Meta-PRD Gap-Close Sprint](../../../VisionFlow/docs/PRD-gap-close-sprint.md), [ADR-004 Gap-Close Sprint Governance](../../../VisionFlow/docs/ADR-004-gap-close-sprint-governance.md)
**Bounded context:** [DDD Gap-Close (canon)](../../../VisionFlow/docs/DDD-gap-close-context.md), [DDD Gap-Close VisionClaw](../ddd/ddd-gap-close-visionclaw-context.md)
**Child ADR:** [ADR-130 Gap-Close VisionClaw Decisions](../adr/ADR-130-gap-close-visionclaw-decisions.md)
**Maturity vocabulary:** ADR-002 seven-tier (`historical`, `planned`, `scaffolded`, `standalone`, `integrated`, `federation-verified`, `released`)

## TL;DR

VisionClaw owns 17 of the 28 register gaps and five commitments because it owns three of the four collaboration surfaces: desktop, mixed reality and voice. The register's through line is sharpest here. The desktop embodiment plumbing is present in code (`AppInitializer.tsx:356` calls `websocketService.connect()`, `AgentNodesLayer.tsx:444` polls `/api/bots/agents`), yet no evidence exists that a live agent action ever draws a beam on screen. The `did:nostr` primitive is built (`src/uri/mod.rs`), canonicalised ecosystem-wide by ADR-125, and wired onto nothing. `PushToTalkService.ts` is a complete singleton with zero call sites. A `BrokerActor` is described as live infrastructure in five committed documents and exists only on the unmerged `crashbug` branch. This slice closes the distance between built and wired, and it proves each closure with a liveness canary that observes real traffic, not a design review.

The recon pass corrected the register on three points this PRD carries forward rather than repeating stale wording. REC-1a (ontology `/propose` auth) and REC-1b (`/load` backdoor) are already fixed on `main` (`ontology_agent_handler.rs:353`, `api_handler/ontology/mod.rs:1353`); this slice verifies them with a regression canary and does not re-implement them. D1's register mechanism (a missing `connect()`, `resolveAgentPosition` hardcoded false) does not reproduce in current code; D1's real open condition is whether the poll ever carries non-empty agent data end to end, and the falsification statement is re-derived on that basis. D5's fabricated status is not one hardcoded boolean but two disagreeing fields on one endpoint plus a hardcoded client prop.

## Goals

| Goal | Outcome |
|---|---|
| G1: One verified `did:nostr` per actor node | Selecting any agent yields a `did:nostr` that survives a `uri::did_nostr()` round-trip and a Schnorr challenge before the client trusts it (COM-14 exit) |
| G2: Live activity reaches the screen | A live agent action draws a beam and updates an avatar with the poll started at boot; the wire is proven by a fired canary, not asserted (D1 exit) |
| G3: The operator can steer and judge | Node selection opens per-agent steering; the control centre carries a broker case queue and an ambient ACSP indicator (D2/D3/REC-2 exit) |
| G4: The governed voice loop is audible | A spoken command to a selected agent produces an accepted signed 31402 and a Kokoro TTS acknowledgement, end to end (COM-15/V1 exit) |
| G5: Status never fabricates | No status indicator is decoupled from ground truth; one endpoint reports one MCP state (D5 exit) |
| G6: MR carries identity and intervention | In-headset actor identity is rendered and verified; a spatial intervention affordance and an ambient ACSP indicator exist in the Godot client (M1/COM-18 exit) |
| G7: One live-traffic canary service for the whole sprint | The `LivenessHarness` registers canaries from any repository and records a `CanaryFired` only on observed live traffic (RES-a exit) |

## Non-Goals

- **Fixing the WebXR immersive tree in place.** `client/src/immersive/` is scheduled for deletion by ADR-071 Phase 3. This sprint implements MR behaviour in the Godot client and neutralises the WebXR VR entry; it does not repair dead code (ADR-130 Decision 1).
- **Completing the ADR-071 Phase 3 cutover.** Deleting the WebXR and Vircadia trees and landing the LiveKit Android AAR bridge is a separate track. This slice depends on none of it.
- **Re-implementing REC-1.** The ontology `/propose` auth gate and the `/load` power-user gate are already closed on `main`. This slice records them at their evidenced tier and guards them with a regression canary.
- **Neo4j.** ADR-043 and the `crashbug` `BrokerActor` both assume a Neo4j store the codebase does not run (the graph store is Oxigraph plus SQLite). This slice re-targets both (ADR-130 Decisions 2 and 5).
- **Owning the voice-intent producer or the `did:nostr` mint.** agentbox mints `did:nostr` at spawn and owns `/v1/voice-intent`; VisionClaw carries and verifies the identity and owns capture, binding, call and acknowledgement.

## Work Packages

Each work package states the owned items, the current maturity tier (cited from the register and spot-verified against `main`), the target tier, explicit acceptance criteria, and a falsification statement authored before any implementation.

### WP-1 Identity Keying (COM-14 / D4 / M1)

- **Owns:** COM-14, D4 (desktop), M1 (headset).
- **Current tier:** `scaffolded`. The `did:nostr` primitive is built (`src/uri/mod.rs`: `DID_NOSTR_PREFIX`, `did_nostr()`, `ParsedUri::DidNostr`) and ADR-125 fixes the canonical document shape, but the `Agent` struct (`src/services/bots_client.rs:15`, duplicated at `crates/visionclaw-domain/src/types/mcp_responses.rs:78`) is keyed by `id: String` from the agentbox MCP task id, with no `did_nostr` field anywhere. In the Godot client the DID reaches the avatar node (`scripts/graph_scene.gd:431` calls `avatar.set_meta("did", did)`) but is never rendered or checked (`scripts/avatar.gd:29` writes only the self-reported `display_name` to the nameplate).
- **Target tier:** `federation-verified` (end-to-end runtime proof across agentbox spawn, VisionClaw carry, and headset render).
- **Cross-repo boundary:** agentbox mints and attaches a `did:nostr` at spawn and includes it in the spawn payload; VisionClaw carries it through the `Agent` struct to every surface and verifies a Schnorr signature over a client-issued challenge, matching `did:nostr:{event.pubkey}` against the payload DID, before trust (ADR-130 Decision 6; ADR-125 I3-safe: verification reads `event.pubkey`, never the DID-document verificationMethod).
- **Acceptance criteria:**
  1. `Agent` carries `did_nostr: Option<String>` in both definitions; the spawn path (`call_agent_spawn`, `src/utils/mcp_connection.rs:349`) populates it from the agentbox spawn response.
  2. Selecting any agent node on desktop and in the Godot HUD surfaces the actor's `did:nostr` (or a short pubkey suffix) and a verification badge.
  3. The client issues a challenge, verifies the returned signature against `event.pubkey`, and rejects a node whose `did:nostr` fails the `uri::did_nostr()` round-trip or the signature check.
  4. A signed 31402 addressed at the selected node's `did:nostr` is accepted by the ACSP path.
- **Falsification statement:** *WP-1 is falsified if any surface still keys an agent by `task_id`, if a `did:nostr` is trusted without a verified signature over a challenge, or if the Godot avatar renders a nameplate but no verifiable identity while `graph_scene.gd:431` still holds the DID in metadata.*
- **Canary:** `CANARY-VC-COM14-DID`.

### WP-2 Embodiment Join (D1)

- **Owns:** D1.
- **Current tier:** `scaffolded`. Boot wiring is present (`AppInitializer.tsx:356`, `MainLayout.tsx:72`, `GraphCanvas.tsx:385`); `0x23` `AGENT_ACTION` beam frames decode in `store/websocket/binaryProtocol.ts:420` into `transientBeamStore`, consumed by `TransientBeamsLayer` (`GraphManager.tsx:594`); `resolveAgentPosition` (`GraphManager.tsx:151`) is a real Map lookup, not a hardcoded false. The register's cited failure mechanism is closed. What remains unproven is whether the poll and the beam frames ever carry non-empty live agent data through to a drawn beam.
- **Target tier:** `federation-verified`.
- **Acceptance criteria:**
  1. In a live session with an active agent, `/api/bots/agents` returns a non-empty roster and a `0x23` beam frame is decoded and drawn on screen.
  2. The poll starts at boot without manual action.
  3. `resolveAgentPosition` returns a real position for a live agent node (not false) with server-side data present.
- **Falsification statement:** *WP-2 is falsified if `resolveAgentPosition` returns false at boot with live agent data present server-side, if `/api/bots/agents` returns empty while an agent is active, or if the D1 closure is declared without `CANARY-VC-D1-BEAM` having fired in a live session.*
- **Canary:** `CANARY-VC-D1-BEAM`.

### WP-3 Steering Surface (D2 / D8)

- **Owns:** D2, D8.
- **Current tier:** `scaffolded`. `AgentDetailPanel.tsx` and `BotsControlPanel.tsx` exist and export from the barrel but no component mounts them (grep: only the barrel and their own files reference them). `AgentDetailPanel.tsx:269` is the sole client call site of `/bots/submit-task`; because the panel never mounts, the route never runs live. No client `/bots/interrupt` route exists. D8's aggregate surfaces (`HealthDashboard.tsx`, `SystemHealthPanel.tsx`, `ActivityLogPanel.tsx`) are defined and never imported; the only mounted observability is `AgentTelemetryStream.tsx` (a per-message log inside `StatusCluster`'s flyout).
- **Target tier:** `integrated`.
- **Acceptance criteria:**
  1. Selecting an agent node mounts `AgentDetailPanel` behind that selection.
  2. A steer action invokes `/bots/submit-task` live; a new interrupt route (`/bots/interrupt`, server and client) is reachable and observed.
  3. A swarm-level aggregate view (task success rate, cost, topology) mounts with live data, distinct from the per-message `AgentTelemetryStream`.
- **Falsification statement:** *WP-3 is falsified if `AgentDetailPanel` remains unmounted, if `/bots/submit-task` or `/bots/interrupt` is never invoked from a mounted panel in a live session, or if D8 closes on a dead `HealthDashboard` that no route mounts.*
- **Canary:** `CANARY-VC-D2-STEER`, `CANARY-VC-D8-OBS`.

### WP-4 Control-Centre Governance (REC-2 / D3)

> **Correction (2026-07-22 doc-drift audit):** `src/domain/broker/*` is **not**
> crashbug-only — it shipped to `main` via `c9f2e3539` (the storage-agnostic
> domain kernel this WP describes cherry-picking is already there). Only
> `src/actors/broker_actor.rs` and `src/adapters/neo4j_broker_adapter.rs`
> remain crashbug-only (unmerged, Neo4j-backed). `ElevationActor` now
> defaults **ON** post-REC-2 — the "env-gated off" line below is stale.

- **Owns:** REC-2, D3.
- **Current tier:** `scaffolded`. `main` ships the ADR-110 ACSP producer (`src/services/acsp/{events,client}.rs`, 393 LOC) publishing kinds 31400–31405 to the forum's D1 `broker_cases` store, plus a REST fallback (`enrichment_proposals_handler.rs`, `broker_inbox_handler.rs` with `GET /api/broker/inbox`). No `BrokerActor` exists on `main` (0 grep hits); it lives only on the unmerged `crashbug` branch (`src/domain/broker/*`, `src/actors/broker_actor.rs`, 625 LOC, Neo4j-backed) which five committed documents still cite as live fact. `ElevationActor` (`src/actors/elevation_actor.rs`), the one ACSP consumer, is env-gated off (`ELEVATION_ACTOR_ENABLED`, `app_state.rs:1072`). The client has zero ACSP case surface (grep for `ACSP`/`31402`/`caseQueue` returns nothing).
- **Target tier:** `integrated`.
- **Decision forced:** ADR-130 Decision 2 supersedes the `crashbug` `BrokerActor` transport and its Neo4j adapter, cherry-picks the storage-agnostic domain kernel (`src/domain/broker/{broker_case,broker_decision,precedent_registry,mod}.rs`, 936 LOC of `BrokerCase`/`DecisionOrchestrator`/`DecisionOutcome`/`PrecedentRegistry`), and surfaces the queue against the existing ACSP producer and `broker_inbox_handler`.
- **Acceptance criteria:**
  1. `broker:new_case` and `broker:case_decided` events publish over the existing multiplexed graph socket when a case is queued and decided.
  2. A control-centre case queue subscribes to those events and renders pending judgments; an ambient ACSP indicator shows open-case count.
  3. `ElevationActor` runs by default in a dev/staging profile so the queue carries real cases.
  4. Every document citing `BrokerActor` as live `main` code (CHANGELOG, ADR-033, ADR-041, `docs/explanation/ecosystem-convergence.md`, `docs/reference/rest-api.md`) is corrected to describe the ACSP forum-hosted case queue.
- **Falsification statement:** *WP-4 is falsified if a case parks in `under_review` forever with no decision path, if the control centre shows no pending judgment while the forum holds an open 31402, if any document still names `crashbug`'s `BrokerActor` as live `main` infrastructure, or if REC-2 closes with `ElevationActor` still gated off in every profile.*
- **Canary:** `CANARY-VC-REC2-CASE`.

### WP-5 Voice Loop (COM-15 / V1 / D6 / M5)

- **Owns:** COM-15 (consumer side), V1, D6, M5.
- **Current tier:** `scaffolded`. Whisper STT (`src/services/speech_service.rs:107`) and Kokoro TTS are complete. PTT is a per-user global toggle (`src/services/audio_router.rs`: `UserVoiceSession.ptt_active`) with no selected-agent binding. `VoiceInterfaceActor` routes to the settings assistant only, never to a signed 31402. Client `PushToTalkService.ts` is a complete singleton with zero call sites. No `/v1/voice-intent` route exists server-side (it is agentbox-owned).
- **Target tier:** `federation-verified` (COM-15/V1); `integrated` (D6/M5).
- **Cross-repo boundary:** agentbox owns the `/v1/voice-intent` producer (un-gates it behind a mandate, accepts a scene-selected actor `did:nostr`). VisionClaw owns capture, the selected-agent binding, the call, and the acknowledgement.
- **Acceptance criteria:**
  1. A selected-agent id threads from graph selection state through the PTT-start message into `AudioRouter`/`VoiceInterfaceActor`.
  2. A spoken command to a selected agent builds a signed `acsp::events::ActionRequest` (kind 31402) targeted at that agent's `did:nostr`, accepted by `/v1/voice-intent`.
  3. A Kokoro TTS acknowledgement plays on acceptance.
  4. `PushToTalkService.ts` (or its replacement) is wired into the live voice path.
- **Falsification statement:** *WP-5 is falsified if PTT remains globally scoped with no target `did:nostr`, if a spoken command reaches only the settings assistant and never a signed 31402, if `PushToTalkService` still has zero consumers, or if COM-15 closes without an audible acknowledgement observed in a live session.*
- **Canary:** `CANARY-VC-COM15-PTT`.

### WP-6 Status Honesty (D5)

- **Owns:** D5.
- **Current tier:** `integrated`-with-defect. The `mcpConnected` badge is honestly wired from a real `/bots/status` poll (`BotsDataContext.tsx:304`). Two adjacent fields fabricate: `ControlCenter.tsx:139` passes the literal `websocketStatus="connected"` (never read from the socket lifecycle) into `StatusCluster`, and `consolidated_health_handler.rs:190` hardcodes `ServiceMetrics.mcp_status: "not_configured"` on the same `/api/health` response that computes the correct value a few lines later (`check_mcp_metrics()`, lines 194–221).
- **Target tier:** `integrated`.
- **Acceptance criteria:**
  1. `ControlCenter.tsx:139` reads the real websocket connection status (the `webSocketService.onConnectionStatusChange` pattern already used at `BotsWebSocketIntegration.ts:31`).
  2. `ServiceMetrics.mcp_status` is deleted or populated from the `check_mcp_metrics()` result, giving `/api/health` one source of truth.
- **Falsification statement:** *WP-6 is falsified if any status indicator still reports a value not derived from ground truth, or if `/api/health` returns two disagreeing MCP-status fields.*
- **Canary:** `CANARY-VC-D5-WS` (one-shot: the WS dot flips to disconnected when the socket actually drops).

### WP-7 Contextual Transaction Cost (REC-3)

- **Owns:** REC-3 (the envelope schema; agentbox emits the fields).
- **Current tier:** `scaffolded`. The `/wss/agent-events` envelope (`src/agent_events/schema.rs:24`, `AgentActionEnvelope`) carries `version`, `id`, `source_agent_id`, `target_node_id`, `action_type`, `timestamp`, `duration_ms`, identity fields, and a free-form `metadata: Value`. No handoff-count, token-burden or verification-outcome field exists as a first-class member.
- **Target tier:** `integrated`.
- **Cross-repo boundary:** VisionClaw owns the envelope schema; agentbox emits the fields from `management-api/utils/agent-event-publisher.js`.
- **Acceptance criteria:**
  1. `AgentActionEnvelope` gains typed optional fields `handoff_count: Option<u32>`, `token_burden: Option<u64>`, `verification_outcome: Option<String>`, each `#[serde(default)]` for backward compatibility.
  2. A real DAG emits an envelope carrying populated CTC fields, observed on the wire.
- **Falsification statement:** *WP-7 is falsified if CTC data still rides only the untyped `metadata` blob, or if REC-3 closes without a live envelope carrying a populated typed CTC field.*
- **Canary:** `CANARY-VC-REC3-CTC`.

### WP-8 Four-KPI Dashboard (REC-4 / ADR-043 resurrection)

- **Owns:** REC-4.
- **Current tier:** `planned`. ADR-043 (accepted 2026-04-14) specifies four KPIs with a Neo4j `OrganisationalMetricSnapshot` and `DERIVED_FROM` lineage; zero implementation exists (grep for `MeshVelocity`/`mesh_velocity`/`ADR-043`/`KPI` across `src` and `client/src` returns nothing). The ADR assumes Neo4j as primary store; the codebase runs Oxigraph plus SQLite, so a literal implementation needs a storage redesign.
- **Target tier:** `integrated`.
- **Decision forced:** ADR-130 Decision 5 re-targets ADR-043's storage from Neo4j to a SQLite metrics table (analogous to `sqlite_enrichment_repository.rs`), with an optional Oxigraph named-graph for lineage. Augmentation Ratio and Trust Variance compute first, from existing sources (`agent_events`, `enrichment_proposals` decision outcomes) without new instrumentation.
- **Acceptance criteria:**
  1. At least one KPI (Augmentation Ratio or Trust Variance) computes from real source events and persists a snapshot.
  2. A control-centre dashboard panel renders the computed KPI with its confidence, pushed over the existing WebSocket pattern.
  3. Lineage from a KPI value to its contributing decision events is queryable.
- **Falsification statement:** *WP-8 is falsified if the dashboard displays a KPI not computed from live source events, if the ADR-043 Neo4j assumption ships unchanged against a non-existent store, or if REC-4 closes with no snapshot traceable to its source events.*
- **Canary:** `CANARY-VC-REC4-KPI`.

### WP-9 MR Identity and Intervention (COM-18 / M2 / M4 / M6, M3)

- **Owns:** COM-18, M2, M4, M6, M3 (all in the Godot client per ADR-071 and ADR-130 Decision 1).
- **Current tier:** mixed. M1 identity plumbing reaches the Godot avatar (`graph_scene.gd:431`) but renders nothing (`scaffolded`). M2 has no affordance: `scenes/HUD.tscn` carries five controls (RoomLabel, RoomEntry, JoinButton, MuteToggle, DebugStats) and grep for intervention/governance/acsp/did in `hud.gd`/`xr_boot.gd` returns nothing (`planned`). M3 avatars are primitive geometry (`scenes/Avatar.tscn`: SphereMesh head, two hidden BoxMesh hands, billboard Label3D, no skeleton, no gaze cue, no proxemics) (`scaffolded`). M4 controller-ray targeting is correct in Godot (`interaction.rs`, `graph_scene.gd:303` sources the ray from `XRController3D.global_position`); gaze is architecturally absent (grep for `gaze` in `xr-client` returns nothing) (`scaffolded`). M6 in Godot is correct (`xr_boot.gd:18` sets `get_viewport().use_xr = true` synchronously); the `isXRMode` defect is a WebXR-only bug (`VRGraphCanvas.tsx:66`) in a tree marked for deletion.
- **Target tier:** M1 `integrated`; M2 `integrated`; M4 `integrated`; M6 `integrated` (Godot); M3 `scaffolded` (may reach `integrated` on the geometric-avatar + gaze-cone + proxemics subset; body/face tracking is out of scope and stays out).
- **Decisions forced:** ADR-130 Decision 1 (MR in Godot; WebXR VR entry neutralised with a deprecation guard; WebXR deletion deferred to ADR-071 Phase 3) and ADR-130 Decision 4 (copresence design per the research brief: minimal geometric avatars, head-gaze primary with eye-gaze as progressive enhancement, Hall's-zones proxemics at a 1.5–2.5 m default band on an arc, three selection resolvers into one arbiter).
- **Acceptance criteria:**
  1. M1: `Avatar.tscn`/`avatar.gd` render a `did:nostr` (or pubkey suffix) and a verification badge sourced from the meta already set at `graph_scene.gd:431`, verified by the `signer.rs` Schnorr path before the badge reads verified.
  2. M2: a per-agent-node intervention panel and an ambient ACSP indicator exist in the Godot HUD, triggered by an interaction on an agent-flagged node.
  3. M4: a controller ray resolves a real agent-node selection (non-origin) live in the xr-runtime sidecar, and a head-gaze fallback resolves a selection when both controllers are untracked.
  4. M6: the Godot XR session sets `use_xr` correctly (verified via the sidecar); the WebXR VR entry shows a deprecation notice instead of rendering a broken desktop-as-VR view.
  5. M3: geometric-core avatars carry a gaze cone and a screen-facing DID badge, and a proxemics solver places agents on a forward arc in the 1.5–2.5 m social band; any sub-feature not instantiated is labelled `scaffolded`.
- **Falsification statement:** *WP-9 is falsified if the Godot HUD still carries no identity or intervention control while `graph_scene.gd:431` holds the DID, if the targeting ray casts from world origin in a live sidecar session, if M3 is claimed above `scaffolded` without a proxemics solver and a gaze cue instantiated, or if the WebXR isXRMode bug is claimed fixed by editing a tree ADR-071 marks for deletion.*
- **Verification path:** the `agentbox/xr-runtime` Monado sidecar (Godot 4.3, VNC :5904) is the only executable MR verification route in this environment; no `godot` binary exists in the container.
- **Canaries:** `CANARY-VC-M1-HUD`, `CANARY-VC-COM18-INTERV`, `CANARY-VC-M4-RAY`.

### WP-10 Voice Conversation Layer and Docs Honesty (V3 / V4)

- **Owns:** V3, V4.
- **Current tier:** V3 `scaffolded` (`ConversationContext.pending_clarification`, `voice_commands.rs:88`, is read at `voice_context_manager.rs:321` but never assigned anywhere — dead scaffolding, so "no conversational grounding" is literally true). V4 is a `canon→practice` doc mismatch: `voice-integration.md:11` and `voice-routing.md` describe a voice-to-swarm command path as live with no deprecation notice, while `PushToTalkService` has zero consumers in the audited React client (the docs partly describe the separate Godot native client).
- **Target tier:** V3 `scaffolded`→`integrated` (P2); V4 `integrated` (docs match shipped behaviour).
- **Acceptance criteria:**
  1. V3: `pending_clarification` is populated when STT confidence or the intent gate is ambiguous, and answered on the next utterance; a clarification turn is observed.
  2. V4: both voice docs carry a status banner stating which client each path describes, or retire the swarm-orchestration claim until COM-15/V1 closes.
- **Falsification statement:** *WP-10 is falsified if `pending_clarification` is still never assigned while V3 is claimed above `scaffolded`, or if either voice doc still presents the voice-to-swarm path as live in the React client without a status banner.*
- **Canary:** `CANARY-VC-V3-REPAIR` (P2; observes a clarification turn on low-confidence input).

### WP-11 KG Liveness and the Liveness Harness (RES-a)

- **Owns:** RES-a, and the `LivenessHarness` service for every repository's canaries.
- **Current tier:** `planned`. VisionClaw's own server is the KG backend at port 4000. A real `/api/health` (`consolidated_health_handler.rs:63`) and a `/healthz` probe (`main.rs:865`) exist, but both are passive pull endpoints; nothing polls them, alerts, or records a canary. The `LivenessHarness` is `planned` in the Gap-Close DDD services table.
- **Target tier:** `integrated`.
- **Decision forced:** ADR-130 Decision 3 fixes the harness as a central live-traffic observer in `visionclaw-server`, registrable from any repository, firing only on observed live traffic (never a synthetic probe).
- **Acceptance criteria:**
  1. A tokio interval watchdog self-polls `/api/health` and drives a `kg_backend_up` gauge; a simulated backend loss flips the gauge and raises `CANARY-VC-RESA-KG` rather than failing open silently.
  2. `POST /api/canary/register` accepts a canary registration (`canary_id`, wire descriptor, fire predicate, owner repo, wave) from any repository.
  3. `POST /api/canary/observe/{canary_id}` records a `CanaryFired` from a repository that fires over HTTP; a Nostr-relay tap records fires from repositories that speak only Nostr (forum, solid-pod).
  4. `GET /api/canary/status` returns per-canary `{armed, fired, last_fired_at, observation_count, sha_at_registration}`; a fired canary older than 30 days or than its registration SHA re-arms.
- **Falsification statement:** *WP-11 is falsified if a canary can be marked fired by a synthetic probe rather than observed live traffic, if a foreign repository cannot register or fire a canary against this service, if the KG backend can go unreachable without the gauge flipping and the canary raising, or if a fired canary older than its SHA still counts toward closure.*
- **Canary:** `CANARY-VC-RESA-KG` (and this service is the registry every canary below fires against).

### WP-12 Already-Closed Correctness and Shares (REC-1a / REC-1b / REC-10 / REC-11 / RES-d source)

- **Owns:** REC-1a, REC-1b (verify only); REC-10 (lead), REC-11 (lead), RES-d source (share).
- **Current tier:** REC-1a `integrated` (already fixed: `ontology_agent_handler.rs:353` mounts `/propose` under `RequireAuth::authenticated()` + `RateLimit::per_minute(20)`); REC-1b `integrated` (already fixed: `api_handler/ontology/mod.rs:1353` gates `/load` and `/load-axioms` under `power_user().mutations_only()`, the weaker duplicate route removed). REC-10 `planned`, REC-11 `planned`, RES-d source `planned`.
- **Target tier:** REC-1a/1b `integrated` (verified, guarded by regression canary); REC-10/REC-11 `integrated` (P2); RES-d source `integrated`.
- **Cross-repo boundary:** RES-d — VisionClaw exposes a script-queryable ontology class-count source that the canon's `DriftCounter` consumes. REC-10 — VisionClaw leads the Insight Ingestion Loop v1. REC-11 — VisionClaw leads the single queryable trace.
- **Acceptance criteria:**
  1. A route dump at closure time confirms no unauthenticated ontology ingest route remains (REC-1a/1b regression guard).
  2. A script-queryable endpoint returns a live ontology class count matching Oxigraph, consumed by the canon `DriftCounter` (RES-d).
  3. REC-10: `ontology_propose` → broker decision → merged enrichment closes once end to end across the mesh (P2). REC-11: one query returns a joined trace spanning agent-events, broker decision and provenance (P2).
- **Falsification statement:** *WP-12 is falsified if a route dump reveals any unauthenticated ontology ingest route, if the class-count source drifts from Oxigraph, or if REC-1a/1b are re-scoped as new work rather than recorded at their already-evidenced tier.*
- **Canaries:** `CANARY-VC-REC1-ROUTE` (one-shot regression); `CANARY-VC-RESD-COUNT`; `CANARY-VC-REC10-LOOP` (P2); `CANARY-VC-REC11-TRACE` (P2).

## Liveness Canaries

Every loop-closing item registers a canary against the `LivenessHarness` (WP-11) before it enters a wave's closure set. Correctness canaries fire once (one-shot); loop canaries are standing monitors that must stay green for the wave to remain promoted (DDD open issue 2, resolved per row below).

| Canary ID | Item | Wire observed | Firing means | Mode | Wave |
|---|---|---|---|---|---|
| `CANARY-VC-COM14-DID` | COM-14/D4/M1 | `Agent.did_nostr` verified by Schnorr challenge at selection | A selected node is addressable by a verified `did:nostr` | Standing | P0 |
| `CANARY-VC-D1-BEAM` | D1 | `0x23` `AGENT_ACTION` beam frame decoded with non-empty agent data | Live agent activity reached the screen | Standing | P1 |
| `CANARY-VC-D2-STEER` | D2 | `/bots/submit-task` or `/bots/interrupt` invoked from a mounted panel | Node selection opened a working steering control | Standing | P1 |
| `CANARY-VC-D8-OBS` | D8 | Aggregate swarm dashboard mounted with live poll data | Swarm-level observability is live | One-shot | P1 |
| `CANARY-VC-REC2-CASE` | REC-2/D3 | `broker:new_case` then `broker:case_decided` on the graph socket | The case queue carried a real case to a decision | Standing | P0 |
| `CANARY-VC-COM15-PTT` | COM-15/V1/D6/M5 | Spoken command → signed 31402 → Kokoro TTS ack | The governed voice loop carried one utterance end to end | Standing | P1 |
| `CANARY-VC-D5-WS` | D5 | WS status dot transitions to disconnected on real socket drop | The indicator tracks ground truth | One-shot | P0 |
| `CANARY-VC-REC3-CTC` | REC-3 | Agent-events envelope carrying a populated typed CTC field | CTC data rides the wire | One-shot | P1 |
| `CANARY-VC-REC4-KPI` | REC-4 | A KPI snapshot computed from real source events, pushed to the dashboard | A KPI is computed from live events | Standing | P1 |
| `CANARY-VC-M1-HUD` | M1 | Godot avatar renders a verified DID badge in an xr-runtime session | Identity is surfaced and verified in-headset | Standing | P0 |
| `CANARY-VC-COM18-INTERV` | COM-18/M2 | Headset intervention panel emits a signed 31402/31403 | The MR governance affordance carries traffic | Standing | P2 |
| `CANARY-VC-M4-RAY` | M4 | Controller or gaze ray resolves a non-origin agent-node selection | Targeting resolves a real selection | One-shot | P2 |
| `CANARY-VC-RESA-KG` | RES-a | `kg_backend_up` gauge transition on watchdog poll | KG liveness monitoring is live and fires on loss | Standing | P0 |
| `CANARY-VC-REC1-ROUTE` | REC-1a/1b | Route dump shows no unauthenticated ontology ingest route | The auth gates hold | One-shot | P0 |
| `CANARY-VC-RESD-COUNT` | RES-d | Class-count endpoint returns a live count matching Oxigraph | The counter source is live | One-shot | P1 |
| `CANARY-VC-V3-REPAIR` | V3 | A clarification turn triggered by low-confidence STT | Conversational repair carried a turn | One-shot | P2 |
| `CANARY-VC-REC10-LOOP` | REC-10 | `ontology_propose` → broker decision → merged enrichment timestamps | The insight loop closed once across the mesh | One-shot | P2 |
| `CANARY-VC-REC11-TRACE` | REC-11 | One query returns a joined agent-events + decision + provenance trace | The data-moat trace is queryable | One-shot | P2 |

## Measurement Data Sources

VisionClaw owns the data sources for four of the five measurement commitments (meta-PRD Measurement Commitments). Augmentation Ratio and Trust Variance derive from `enrichment_proposals` decision outcomes and `/wss/agent-events` volume; Contextual Transaction Cost derives from the WP-7 typed envelope; Mesh Velocity derives from the REC-10 loop timestamps. HITL Precision derives from broker decision outcomes surfaced by WP-4.

## Maturity Summary

| Item | Current | Target | Note |
|---|---|---|---|
| COM-14/D4/M1 identity | scaffolded | federation-verified | Struct field + verify-before-trust |
| D1 embodiment | scaffolded | federation-verified | Plumbing present, liveness unproven |
| D2/D8 steering | scaffolded | integrated | Panels exist, unmounted |
| REC-2/D3 case queue | scaffolded | integrated | Domain kernel cherry-picked; transport superseded |
| COM-15/V1/D6/M5 voice | scaffolded | federation-verified / integrated | PTT unwired, no target binding |
| D5 status | integrated-with-defect | integrated | Two fabricated fields to fix |
| REC-3 CTC | scaffolded | integrated | Typed envelope fields |
| REC-4 KPI | planned | integrated | Storage re-targeted off Neo4j |
| M2 affordance | planned | integrated | Net-new Godot HUD surface |
| M3 copresence | scaffolded | scaffolded (may reach integrated) | Labelled if deferred |
| M4 targeting | scaffolded | integrated | Godot ray correct; gaze new |
| M6 isXRMode | scaffolded | integrated (Godot) | WebXR locus deferred to ADR-071 Phase 3 |
| V3 repair | scaffolded | integrated (P2) | Dead `pending_clarification` |
| V4 docs | canon/practice mismatch | integrated | Status banner |
| RES-a / LivenessHarness | planned | integrated | Central live-traffic observer |
| REC-1a/1b | integrated | integrated | Already closed; verify only |
| REC-10/REC-11 | planned | integrated (P2) | Shared leads |
| RES-d source | planned | integrated | Class-count endpoint |

## Cross-Reference

- Meta-PRD VisionClaw work package: `VisionFlow/docs/PRD-gap-close-sprint.md` lines 153–159.
- Identity: `did:nostr:<hex>` per ADR-125 (Multikey `fe70102` form); verification reads `event.pubkey`, never the DID-document verificationMethod (ADR-125 I3).
- XR: Godot 4.3 + gdext + OpenXR per ADR-071; transport per ADR-102 (Protocol V3 `0x03`, 52-byte records; avatar pose `0x43`); copresence per `scratchpad/xr-copresence-research-brief.md`.
- Governance: ACSP kinds 31400–31405 per ADR-110; broker domain per ADR-041 (superseded-in-part, ADR-130 Decision 2).
- KPI: ADR-043 resurrected with storage re-target (ADR-130 Decision 5).
