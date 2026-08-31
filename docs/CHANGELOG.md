# Changelog

All notable changes to VisionClaw will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Correction (gap-close REC-2, branch `gap-close/2026-07`, 2026-07-08).** Entries
> below attribute governance-panel and case events to a `BrokerActor` in
> `src/actors/broker_actor.rs`. That actor never merged to `main` (it lives only
> on the unmerged `crashbug` branch, tied to a Neo4j store this stack does not
> run). Per ADR-130 Decision 2, `main` ships ADR-110's ACSP producer
> (`ElevationActor` over the ported `src/domain/broker/` kernel), and the
> `broker:new_case` / `broker:case_decided` WebSocket events are emitted from the
> enrichment-decide handler (`services::broker_events`) over the multiplexed graph
> socket. Read "`BrokerActor`" below as "the broker governance publisher".

## [Unreleased] - 2026-08-31

### Added — XR immersive-interaction + layout programme (ADR-137–141)

The week's landing across the Godot/OpenXR client, the GPU layout engine, and
the REST/wire surfaces. New endpoint families are documented in
`docs/reference/rest-api.md`; the RenderStore and force-channel registry have a
dedicated developer reference at
`docs/reference/render-store-and-force-channels.md`.

- **XR render offload + runtime quality dials (ADR-137).** Per-frame position
  hunt and MultiMesh buffer packing moved out of GDScript into a pure-Rust
  `RenderStore` (`xr-client/rust/src/render_store.rs`) exposed through
  `BinaryProtocolClient`; full density (13,164 nodes / 145,692 edges) now renders
  at 90 fps (was ~11 fps in GDScript). Draw budgets are derived from the received
  topology rather than the hardcoded 640/3000 gates; initial-load quality is a
  settings dial (`initialNodeLimit` on `/api/settings/physics`, replacing the
  compiled-in `DEFAULT_INITIAL_NODE_LIMIT`); WebSocket receive cap raised to
  256 MiB. Full-3D force layout is now the default — `axisCompressionZ` removed,
  dual-disc flatten opt-in (`enableDualDiscLayout`, default OFF).
- **V5 wire protocol.** The client decodes the V5 position wrapper
  (`[0x05][u64 broadcast_seq][V3 records]`); documented in
  `docs/reference/binary-protocol.md`. WebSocket upgrade `frame_size` raised so
  large V5 broadcasts are not truncated.
- **GPU force-channel registry (ADR-138).** A bounded `ForceChannel` enum
  (`src/models/force_channels.rs`) mapping each named channel to the existing
  `SimParams` scalar field(s) and feature-flag bit(s) — one enumerable
  "what channels exist / on-off / strength" registry with zero change to the
  struct layout, CUDA kernels, or settings wire (the migration seam toward the
  later array-backed `SimParams` refactor). Includes the pinned-node bitmask and
  DAG radial-bias channel.
- **Graph2VR-class immersive interaction (ADR-139).** Two-hand pinch
  scale/rotate, radial menu, in-graph search, and a node expansion API adopted
  into the Godot/OpenXR client (ideas-level / clean-room mining record — no
  external code or assets vendored; every feature re-implemented against
  VisionClaw's own `RenderStore`, CUDA kernels, and Graph V3 wire).
- **Agent-swarm visualisation in XR (ADR-140, P1 shipped).** Consumes the
  `0x23 AGENT_ACTION` frames (`AgentBeamActor`/`BeamCoalescer`, server side):
  embodied agent capsules hover near their target node, directional work beams
  stream agent→node, 4-channel status halo, and a Swarm tab on the HUD control
  centre with tap-to-teleport. Motion-authority split — server owns *which* node
  + status, XR client owns *where in the room* — so zero server change. All
  per-frame cost stays in the Rust RenderStore.
- **Constrained-layout engine programme (ADR-141, P1–P4 complete; P5/P6
  deferred).** 13-pattern taxonomy audited and mapped to soft force channel / hard
  projection / CPU one-shot placement methods: Sugiyama layers, stratified planes,
  spherical shells, and ego-radial `RadialModes`. New endpoints
  `POST /api/layout/mode` and `POST /api/layout/radial`.
- **Visual query builder.** `POST /api/graph/query/pattern` — build a graph
  pattern query against the semantic planes.
- **Fold ladder.** `GET /api/graph/fold` — hierarchical-density fold levels; the
  fold application layer is the ADR-137 `RenderStore`.

## [Unreleased] - 2026-08-15

### Added — Semantic-trust gap closures (PRD-022 WS-1/WS-2, PRD-010 G4)

Four long-standing scaffolds finished and verified (four-agent mesh, all gates
green; per-workstream detail in the commit messages):

- **SHACL enforcement** — the five `.shacl.ttl` NodeShapes now load at startup
  and drive a shape-derived validator; the ingest gate is enforcing by default
  (`ontology_agent.shacl_mode`, rollback `advisory`), and the trust-status
  endpoint reports the live mode. Live GitHub sync now skips shape-invalid
  blocks instead of silently ingesting them.
- **PROV-O provenance** — unified into the single append-only ledger
  `urn:ngm:graph:provenance`; `propose_create`/`propose_amend`, inference, and
  decision paths all reify (fail-open). New query surface
  `GET /api/ontology/provenance?entity=<urn>[&depth=N]`.
- **Forum NIP-42 AUTH** (`nostr-rust-forum`) — NIP-42 is the universal write
  gate; the pubkey allowlist is authorisation-after-authentication. Escape
  hatch `AUTH_MODE=allowlist`.
- **Mesh federation** (`nostr-rust-forum`) — PRD-010 forum reference
  implementation shipped: IS-Envelope v1 (JCS/RFC 8785), in-crate NIP-26,
  tri-mode `[mesh]` config, CF-compatible MeshTransport, fan-out planner
  gated on live NIP-42 sessions via `SessionAuthBoundary`.

### Fixed

- Three stale test expectations repaired: two V3 wire-format size asserts
  (48 → 52 bytes/node, ADR-031) and an f64-scale epsilon on the f32 GPU
  community-modularity oracle.

## [Unreleased] - 2026-08-10

### Added — Live linkage: reasoned graph → visible graph (`graphUpdated`)

- **Server now signals connected clients when the reasoned graph changes.**
  `GraphServiceSupervisor` broadcasts a debounced `{"type":"graphUpdated",
  "revision":N,"reason":…}` text frame over the existing
  `ClientCoordinatorActor` fan-out on: full database reload (GitHub sync /
  admin reload — fires *after* the reload completes), bulk node ingest,
  runtime edge inserts, full topology replacement, ontology axiom add/remove,
  and stored Whelk inference results. Previously connected clients were never
  told about any structural change and stayed stale until reconnect.
- **Client refetches on signal.** `textMessageHandler.ts` handles
  `graphUpdated` with a 750 ms trailing debounce: refetch REST
  `/api/graph/data` (topology-hash dedup makes no-change refetches free) and
  refresh the inferred-axioms overlay so `InferredEdges` tracks the evolving
  reasoner output live.
- **Self-heal backstop.** `graphDataManager.updateNodePositions` sample-checks
  the id-keyed binary position stream (~every 2 s, 30 s refetch cooldown); if
  frames reference node ids the client never learned, topology is refetched —
  covers missed signals and frame/refetch races.
- Protocol documented in `docs/reference/websocket-protocol.md` (§ Live
  linkage).

### Fixed — Graph interactions restored (drag + double-click)

- **Node drag and double-click→narrativegoldmine both work again.** Five
  compounding faults: StrictMode unmount disposed the GPU-transform picking
  texture while the memoised mesh survived (raycast fed a null buffer);
  `useMemo` ref side-effects leaked from discarded renders so `useFrame` drove
  a phantom mesh while the R3F-committed mesh (with the pointer handlers)
  rendered nothing; the R3F event manager could mount unconnected (async `gl`
  factory); the `onDragStateChange` → OrbitControls-disable wiring was lost
  when `GraphViewport` was retired; and the `WebSocketAdapter` lacked
  `sendMessage`, so hidden `as unknown as` casts threw at runtime — killing the
  `nodeDragStart/Update/End` server pin protocol *and* stranding OrbitControls
  disabled. Pointer capture now uses R3F's capture shim so drags no longer
  stall when the cursor outruns the node. Commit `2644fdeea`.

### Changed — Ontology reasoner is live by default

- **`ontology_validation` feature flag now defaults ON** (`FeatureFlags::default()` in `src/handlers/api_handler/analytics/types.rs`; `FEATURE_FLAGS` initialises from it in `analytics/state.rs`). The Whelk EL reasoner's HTTP read endpoints (`/api/ontology/*`) are therefore exposed by default — the reasoner is no longer opt-in. Commit `ea8d5e360`.
- **Whelk classifier runs at boot** and materialises ~37k inferred axioms into the named graph `urn:ngm:graph:ontology:inferred` (reflexive `X ⊑ X`, `X ⊑ owl:Thing`, `owl:Nothing ⊑ X`, plus the asserted-subclass closure). The "live reasoned graph" is now actually served, not aspirational.

### Fixed

- **`GET /api/ontology/hierarchy` no longer panics.** It was crashing with a `block_on`-in-async panic; it now returns `200` with the full OWL class hierarchy (~12,417 nodes). The class list/hierarchy is fetched async-direct via `state.ontology_repository.list_owl_classes().await` rather than through the sync `QueryHandler`. Commit `2d17365c5`.

## [Unreleased] - 2026-06-21

### Added — Semantic Trust Layer (PRD-022 Phase 1, ADR-127)

- **W3C SHACL shape catalogue** (`crates/visionclaw-ontology/shapes/*.ttl`): 5 shapes for OntologyClass, InferredAxiom, BridgeRecord, KnowledgeNode, AgentNode. Loaded into dedicated `urn:ngm:graph:shapes` named graph via SPARQL migration.
- **Dual-mode SHACL gate** (`shacl_gate.rs`): Enforcing mode (write paths reject violations) and Advisory mode (read paths log and proceed). Gate produces `ShaclGateReport` with severity-tagged violations.
- **W3C PROV-O provenance reification** (`provenance_emitter.rs`): `reify_activity()` inserts PROV-O triples into append-only `urn:ngm:graph:provenance` named graph. `query_agent_activities()` and `count_*()` SPARQL queries for liveness.
- **BC20 provenance crossing** (`receipt-minter.js`): `crossActivityOutbound()` wired into `mintSpendActivity()` — every spend activity auto-crosses through the BC20 bridge to the VisionClaw provenance graph. Fail-open.
- **Trust status endpoint** (`GET /api/ontology-physics/trust-status`): WS-5 liveness canary reporting shapes loaded, provenance triples, gate modes, federation status.
- **SPARQL migrations**: `0002_bootstrap_shapes_graph` and `0003_bootstrap_provenance_graph` added to the migration registry.
- **Docs**: PRD-022, ADR-127 (keystone), DDD Semantic Trust Layer bounded context (BC22).

## [Unreleased] - 2026-05-12

### Added — Agent Control Surface Protocol Integration

> **Superseded design (ADR-130 Decision 2).** The entries in this section
> record the `crashbug`-branch ACSP integration built around a `BrokerActor`
> (`src/actors/broker_actor.rs`) and a `ServerNostrActor`
> (`src/actors/server_nostr_actor.rs`). **Neither file merged to `main`.** What
> actually ships on `main` is the ADR-110 stateless ACSP producer: kinds 31400
> and 31402 are built by `src/services/acsp/events.rs` (`build_panel_definition`
> / `build_action_request`) over the ported `src/domain/broker/` kernel, and the
> `broker:new_case` / `broker:case_decided` WebSocket events are emitted from the
> enrichment-decide handler (`services::broker_events`). Read every `BrokerActor`
> / `ServerNostrActor` reference below as that superseded transport, not shipped
> `main` code.

- **Governance panel publishing** (kind 31400): registers/updates NIP-33
  parameterized replaceable control panels on the Nostr relay. Panel definitions
  include schema type (ActionInbox, Dashboard, ConfigForm, StatusBoard,
  ChatBridge), field definitions, action buttons, layout hints, and
  capabilities. Wire-compatible with `nostr-bbs-core::PanelDefinition`. On `main`
  this ships as `src/services/acsp/events.rs::build_panel_definition` (ADR-110
  ACSP producer); the `crashbug` `src/actors/server_nostr_actor.rs`
  `PublishGovernancePanel` message named in earlier drafts never merged.
- **Action request publishing** (kind 31402): emits a case for human review with
  case ID, title, category, priority, structured fields, and agent reasoning.
  Wire-compatible with `nostr-bbs-core::ActionRequest`. On `main` this ships as
  `src/services/acsp/events.rs::build_action_request` (ADR-110 ACSP producer);
  the `crashbug` `src/actors/server_nostr_actor.rs` `PublishActionRequest`
  message never merged.
- **Broker startup panel** (superseded `crashbug` `src/actors/broker_actor.rs`,
  never merged): the design published a PanelDefinition (kind 31400, d-tag
  `visionclaw-broker`) from `Actor::started()` with 5 fields (case_id, title,
  category, priority, state), 3 actions (approve, reject, escalate), table
  layout, and a 30-second refresh interval, discoverable via relay subscription.
  On `main` the panel is registered by the ADR-110 ACSP producer, not an actor
  `started()` hook.
- **Broker → forum ActionRequest** (superseded `crashbug`
  `src/actors/broker_actor.rs`, never merged): the design published a kind-31402
  ActionRequest to the forum relay on every `SubmitBrokerCase` (all case
  categories, not just KnowledgeEnrichment), with priority mapping u8 90+ →
  Critical, 70+ → High, 40+ → Medium, else → Low. On `main` the equivalent
  kind-31402 case event is produced by the ACSP producer over the
  enrichment-decide path.
- **Broadened Nostr decision events**: `SignBrokerDecision` (kind 30300) is now emitted for all case categories on `DecideBrokerCase`, not just `KnowledgeEnrichment`.
- **NIP-98 enterprise RBAC** (`src/middleware/enterprise_auth.rs`): `nip98-auth` feature gate adds a Nostr NIP-98 authentication path to the `RequireRole` middleware. When enabled, reads `Authorization: Nostr <base64>`, verifies the Schnorr signature, and resolves the signer's pubkey to an `EnterpriseRole` via the `Nip98RoleResolver` trait. `InMemoryRoleMap` provided for dev/test; `Nip98IdentityExt` request extension carries verified pubkey and role. The `X-Enterprise-Role` header path remains as the default when the feature is disabled.
- **Prometheus counters**: `NostrKind::K31400` and `K31402` variants added to `src/services/metrics.rs` for governance event observability.
- **Supported kinds extended**: `SUPPORTED_KINDS` in `src/services/server_identity.rs` now includes 31400 and 31402.
- **Tests**: `handles_publish_governance_panel` and `handles_publish_action_request` in `server_nostr_actor.rs`; 3 NIP-98 tests in `enterprise_auth.rs` (feature-gated).

### Changed

- `RequireRole` middleware now supports dual-path auth: NIP-98 Schnorr verification (when `nip98-auth` feature enabled and resolver attached) or `X-Enterprise-Role` header extraction (default).
- `crashbug` `ServerNostrActor` module doc (superseded design, not on `main`) updated to list 9 message variants across 7 event kinds (was 7 variants across 5 kinds).
- `crashbug` `BrokerActor` imports consolidated (superseded design, not on
  `main`): all governance types (`PublishGovernancePanel`, `PublishActionRequest`,
  `ActionPriority`, `PanelDefinitionPayload`, etc.) imported at module level.

---

## [Unreleased] - 2026-04-23

### Added — Agentbox integration planning

- **PRD-004** (`docs/PRD-004-agentbox-visionclaw-integration.md`): deprecate `multi-agent-docker/` in favour of `agentbox/` as VisionClaw's agent-container subsystem. 5 milestones (M1-M5), 17 port-in rows (P0-P3), 13 agentbox-design-improvement rows (D.1-D.13), 11 explicit rejections, 10 resolved open questions. All milestone exit criteria are now passable predicates.
- **ADR-058** (`docs/adr/ADR-058-mad-to-agentbox-migration.md`): MAD deprecation with four predicate-based gates, mid-cutover rollback protocol (< 5 min target, CI-verified), 30-day post-cutover window with frozen-MAD container.
- **DDD-BC20 AgentboxIntegration** (`docs/ddd-agentbox-integration-context.md`): new bounded context acting as Anti-Corruption Layer between agentbox's adapter protocol and VisionClaw Rust aggregates. `FederationSession` + `AgentExecution` aggregates, five ACL translator modules, composed `SessionHealth` with per-slot degrade policies, `/v1/meta` handshake protocol, signed Ed25519 `LocalFallbackProbe`.
- **Agentbox subdir**: `agentbox/` added as sibling to `multi-agent-docker/` for in-situ radical-upgrade work. Slated for promotion to standalone repo `github.com/DreamLab-AI/agentbox` once stable.
- **QE fleet pre-implementation audit** (RuVector `agentbox-comparison :: qe-fleet-review-2026-04-23`): verdict **Conditional GO** for M1; five P0 doc edits landed before any code work.

### Changed

- `multi-agent-docker/` is on a deprecation track per ADR-058; no new features land there.
- Durable state (beads, pods, memory) is henceforth a VisionClaw-Rust concern, not a container-internal concern. Agentbox is a federation client (see agentbox ADR-005).

## [Unreleased] - 2026-04-18

### Added

#### Insight Migration Loop design corpus
- Phase 1 research: 9 artefacts totalling ~19,700 words defining the dual-tier identity, sigmoid scoring, broker workflow, physics forces, acceptance tests
- ADR-048 Dual-tier identity model (KGNode + OntologyClass with BRIDGE_TO edges)
- ADR-049 Insight-migration broker workflow (MigrationCase subtype, DecisionOrchestrator contract)
- PRD: Insight Migration Loop (3 personas, 10 capabilities, 6 migration KPIs)
- DDD context refinement (BC13 MigrationCandidate aggregate, BC11 MigrationCase)
- 00-master.md: synthesised reconciliation resolving 5 cross-artefact contradictions, 8 blocking questions for owner decision

#### Enterprise & Regression Testing
- Enterprise drawer (`EnterpriseDrawerMount`, `EnterpriseDrawer`) — full-viewport slide-out panel with frosted-glass alpha blend, Ctrl+Shift+E / Cmd+Shift+E toggle, floating FAB button
- `drawer-fx` WASM crate (`client/src/wasm/drawer-fx/`) — Rust flow-field ambient effect for enterprise drawer canvas layer; zero-copy `Float32Array` pattern matching `scene-effects`
- Regression tests: `tests/physics_orchestrator_settle_regression.rs`, `tests/settings_physics_propagation_regression.rs`
- `tests/smoke/nginx-coep-headers.sh` — COEP header smoke test
- Enterprise drawer design document: `docs/design/2026-04-17-enterprise-drawer.md`
- QE audit report: `docs/audits/2026-04-17/` (master, frontend graph loading, backend settings routing, failure patterns, regression risk, regression tests — 6 files)
- `enterprise-standalone.tsx` with `#/drawer-demo` hash route for isolated drawer preview

### Fixed
- **PHYSICS: Dual `ClientCoordinatorActor` instances** — `SocketFlowServer` registered clients in one coordinator instance while `PhysicsOrchestratorActor` broadcast to a second internally-created instance, causing 0 binary frames to reach any connected client. Fixed by injecting the shared `ClientCoordinatorActor` address into `GraphServiceSupervisor::with_client()` and skipping internal creation when an external instance is provided.
- **PHYSICS: `ClientFilter` default filter to zero** — `ClientFilter::default()` had `enabled: true` with empty `filtered_node_ids`, causing `broadcast_with_filter` to produce no payload for fresh clients. Fixed by setting `enabled: false` as the default (opt-in filtering, not opt-out).
- **PHYSICS: `FastSettle` permanent latch** — `FastSettle` mode set `fast_settle_complete = true` and `is_physics_paused = true` on reaching the iteration cap even when energy had not converged, preventing subsequent physics parameter changes from resuming simulation. Fixed by falling back to `Continuous` mode on non-convergent exhaustion rather than halting.
- **PHYSICS: Boundary-pinned node rescue** — Added detection for nodes oscillating at viewport boundary (`|coord| >= viewport_bounds - 1` for 60+ consecutive frames) and teleporting them to randomised interior positions, complementing the existing runaway-node rescue (nodes beyond 10× viewport bounds).
- **SLIDER RANGES: Calibrated physics UI sliders** — Attraction (`attractionK`) capped at 10, Dual Graph Separation (`graphSeparationX`) capped at 500, Flatten to Planes (`zDamping`) capped at 0.1. Previous maximums were orders of magnitude too wide.
- **AUTH: Enterprise endpoints returning 403** — `apiFetch` was not injecting auth headers; added auth header injection mirroring `authRequestInterceptor`. Backend `verify_access` now accepts `Bearer dev-session-token` in non-production environments before NIP-98 path.
- **NGINX: COEP headers lost on Vite proxy routes** — Per-location `add_header` now set for all Vite module proxy paths (`/.vite`, `/node_modules`, `/@vite`, etc.) because `add_header` in a `location` block drops server-level headers.
- **DEBUG: Console spam from RemoteLogger** — Gated `originalConsole.log/debug/info` echo behind `localStorage.debug.consoleLogging === 'true'`; `warn` and `error` continue to echo unconditionally.
- **DEBUG: BotsDataProvider polling churn** — `pollingConfig` literal re-created on every render caused `useAgentPolling` to stop and restart every 2 seconds. Fixed with `useMemo` + `useCallback`.
- **WebSocket: `permessage-deflate` misused as subprotocol** — Removed `.protocols(&["permessage-deflate"])` from WebSocket upgrade handler (it is a WebSocket extension, not a subprotocol; placing it in `.protocols()` produced a malformed negotiation header).
- **WebSocket: Frame size limit** — Added `.frame_size(4 * 1024 * 1024)` to WebSocket upgrade handler; default 64 KiB was silently truncating large V5 broadcasts.
- **First-frame render** — `GraphManager` now polls via `window.setInterval` for non-zero positions from `graphWorkerProxy` and calls R3F `invalidate()` when data arrives, fixing the case where the graph was invisible until window resize triggered a re-render.

---

## [Unreleased] - 2026-04-12

### Added
- Layout mode system with 6 algorithms (ADR-141; corrected 2026-08-31 from the mis-attributed ADR-031 — ADR-031 defines only the `LayoutMode` enum, the constrained-layout engine programme that lands the algorithms is ADR-141)
- ForceAtlas2 LinLog kernel for community-revealing layout
- Spectral, Hierarchical, Radial, Temporal, Clustered layout engines
- PageRank HTTP API endpoints (compute/result/clear)
- DBSCAN standalone clustering API
- GET /api/graph/positions endpoint (GPU position snapshot)
- Layout API endpoints (modes/status/zones/reset)
- Camera auto-fit to graph bounding box
- Degree-weighted node sizing (sqrt scaling, 10.7x ratio)
- Mass-aware physics (hub inertia)
- Dual-graph X-axis offset (graphSeparationX)
- Constraint zone system for node type separation
- 5 ontology constraint specialized GPU kernels
- 7 semantic force GPU FFI bridges
- Stress majorization GPU-only path
- DBSCAN in settings dropdown

### Fixed
- CUDA_ARCH runtime auto-detection (was using stale .env)
- PTX module lookup in community.rs (wrong module for clustering kernels)  
- Two-sheet Z-axis degeneration (polar angle sampling bias)
- Slider ranges capped to sane values (was 50000 max)
- Clustering visualization: analytics panel writes to node_analytics
- Louvain writes to both cluster_id and community_id slots
- Community detection results stored in node_analytics
- Settings auth bypass for dev containers
- Route registration for pagerank/pathfinding (was 404)
- Node ID type flag encoding in binary protocol

---

## [Unreleased] - 2026-02-08

### Client Architecture Overhaul

#### Graph Worker & Physics

- **Position preservation in setGraphData()**: Worker now preserves interpolated positions for existing nodes when setGraphData is called (from initialGraphLoad, filter updates, reconnects). Only genuinely new nodes receive fresh positions, eliminating the visual "explosion" on graph reload.
- **Interpolation fix**: Server physics lerp factor was 1000x too slow due to `deltaTime / 1000` bug (deltaTime is already in seconds from Three.js clock). Fixed to `1 - Math.pow(0.001, deltaTime)`, converging in ~1 second instead of ~16 minutes.
- **Stable ID mapping**: Non-numeric node IDs now use FNV-1a hash (shared `stringToU32` in `client/src/types/idMapping.ts`) instead of unstable `index + 1`. Collision resolution via linear probe. Ensures consistent numeric IDs across setGraphData calls.
- **ForceComputeActor state preservation**: `iteration_count`, `stability_iterations`, and `reheat_factor` are no longer reset when settings updates arrive. Physics simulation maintains continuity across settings changes.

#### WebSocket Architecture

- **WebSocketEventBus** (`client/src/services/WebSocketEventBus.ts`): New typed pub/sub for cross-service WebSocket events. Event categories: `connection:open/close/error`, `message:graph/voice/bots/pod`, `registry:registered/unregistered/closedAll`.
- **WebSocketRegistry** (`client/src/services/WebSocketRegistry.ts`): Central connection lifecycle tracker. All WebSocket services (Voice, Bots, SolidPod, Graph) register/unregister connections through the registry.
- Eliminated `window.webSocketService` global in favour of direct module imports.

#### Settings Pipeline

- **Simplified useSelectiveSettingsStore**: Reduced from 548 to 152 lines. Removed manual caching, TTL, and debouncing; uses Zustand selectors natively.
- **Backend accepts partial JSON**: Physics and quality-gate PUT handlers now merge partial patches into current settings instead of requiring full payloads.
- **Quality gate defaults raised**: `maxNodeCount` increased from 10,000 to 500,000.

#### Visual System

- **MetadataShapes**: Now respects `nodeSize` setting (applies `sizeMultiplier`). Geometry sizes normalized to ~0.5 bounding sphere radius. Settings lookups hoisted out of per-node per-frame loop for performance.
- **KnowledgeRings**: Only renders on nodes positively identified as `knowledge_graph` type. No longer falls back to the `graphMode` default, preventing incorrect ring display on non-knowledge nodes.

#### Code Quality

- Deleted `lucide-react.d.ts` manual type declarations; converted 32 deep-path imports to barrel imports.
- Replaced `window.webSocketService` global with direct imports across all consuming modules.
- Removed V1 binary protocol dead code. Fixed V4 log spam (warn-once pattern).
- Replaced 14 `console.log` calls with proper logger usage.
- Removed dead functions/imports from GraphManager, websocketStore, and graphDataManager.

### Algorithm Pipeline Completion

- Wire SSSP distances into GPU force kernel `d_sssp_dist` buffer (SSSP-aware spring forces now active)
- Implement delta-stepping for GPU SSSP (configurable bucket width)
- Wire GPU APSP kernel (`approximate_apsp_kernel`) into `ShortestPathActor`
- Add multi-source batched SSSP for efficient landmark computation
- Implement LSH (Locality-Sensitive Hashing) replacing O(n^2) pairwise similarity
- Add CPU SIMD vectorization (AVX2/SSE4.1) for physics fallback
- Implement A* search with Euclidean 3D heuristic
- Implement bidirectional Dijkstra for point-to-point queries
- Add semantic pathfinding with trait-based embedding provider

---

## [1.2.0] - 2026-02-08

### Stabilization Sprint - 16 Commits

**Scope:** 6-agent diagnostic swarm identified 11 critical issues. All resolved across 16 commits with
21,853 lines of dead code removed, 474 cargo warnings eliminated, 44 mutation handlers secured,
and the agent telemetry pipeline wired end-to-end.

---

### Added

- **Agent telemetry pipeline** (a86fecda)
  - Management API receives bot metrics
  - `AgentMonitorActor` processes `UpdateBotsGraph` messages
  - `GraphServiceSupervisor` broadcasts binary node data
  - Client `AgentNodesLayer` renders agent nodes in Three.js
  - Full path: REST POST -> Actor message -> Binary WebSocket -> GPU instanced mesh

- **EventBus wiring** (a86fecda)
  - `AuditEventHandler` registered for compliance logging
  - `NotificationEventHandler` registered for real-time alerts
  - Wildcard dispatch: handlers subscribed to `*` receive all domain events
  - Four handler implementations: Graph, Ontology, Audit, Notification

- **Auth middleware on mutations** (3fe44e24)
  - `AuthenticatedUser` extractor added to 44 mutation handlers across 8 endpoint groups
  - Supports `Authorization: Bearer <token>` + `X-Nostr-Pubkey` header
  - Dev bypass via `SETTINGS_AUTH_BYPASS=true` environment variable
  - Endpoint groups secured: analytics, semantic_forces, ontology_physics, bots, constraints, workspace, quest3, ragflow

- **Multi-graph rendering** (dc786e49)
  - Per-node type classification via binary protocol V3 bit flags
  - Bits 26-31 of node ID u32 encode type: 31=agent, 30=knowledge, 26-28=ontology subtypes
  - Parallel display of knowledge graph, ontology graph, and agent graph layers
  - Cluster hulls, KG rings, per-mode edge coloring (5462b9a2)

- **Real graph metrics** (d394fbc5)
  - BFS-based average path length replacing placeholder `4.2`
  - Triangle counting for clustering coefficient replacing placeholder `0.42`
  - Freeman centralization replacing placeholder `0.15`
  - Louvain-style modularity replacing placeholder `0.65`
  - Harmonic mean efficiency replacing placeholder `0.78`

- **Backend stub implementations** (a86fecda)
  - `RemoveOwlClass` directive: real Neo4j DELETE with event publication
  - `RemoveAxiom` directive: real Neo4j DELETE with relationship cleanup
  - `query_nodes`: real Cypher MATCH with property filtering
  - `constraint_stats`: real aggregation from constraint store
  - `extract_property_graph`: real Neo4j property graph extraction

### Changed

- **Docker build** (da510b29)
  - 5-stage multi-stage Dockerfile: deps, build, CUDA, frontend, runtime
  - Cargo dependency caching stage (rebuilds only on Cargo.lock change)
  - Parallel CUDA kernel compilation + frontend build
  - Final stage copies only release binary + static assets

- **Binary protocol V3 node type encoding** (dc786e49)
  - Extended bit flags from 2 bits (agent/knowledge) to 6 bits (26-31)
  - Ontology subtypes: bit 26 = class, bit 27 = individual, bit 28 = property
  - Backward-compatible: V2 clients ignore bits 26-28

- **Client UI honesty** (7ffe33c5, 80b782e7)
  - 11 inert `QualityGate` toggles disabled with tooltip explanations
  - 5 "coming soon" panels replaced with accurate status descriptions
  - Toggles that had no backend wiring now show "Not connected" state

### Fixed

- **Settings persistence** (48822e78)
  - Settings changes no longer reset graph node positions
  - Edge rendering no longer vanishes after settings save
  - Settings actor sends targeted update instead of full graph reload

- **Edge rendering** (a76ae3b8)
  - 3 number/string type mismatches fixed (source/target IDs parsed as strings instead of numbers)
  - All edges silently dropped due to failed ID lookups -- now render correctly

- **Physics startup** (2c67c15d)
  - Physics simulation initializes in correct mode on first load
  - Fixed infinite loop in layout convergence check
  - Fixed overlay panel z-index conflict
  - Fixed spacemouse snap-to-origin on disconnect

- **Node positions** (eef7a786)
  - Instance buffer setup uses actual node positions instead of fallback circle pattern
  - Prevents brief "explosion" animation on initial render

- **Safety / UB elimination** (b03ee44c)
  - `static mut` references in telemetry module replaced with `OnceLock`
  - Eliminates undefined behavior flagged by `static_mut_refs` lint
  - Dead message types removed from actor message enums
  - CUDA kernel function naming corrected (snake_case consistency)

- **Physics mode and overlay** (a7de546e)
  - Physics mode selector correctly switches between force-directed/stress-majorization/manual
  - Infinite loop in convergence detection broken with iteration cap
  - Overlay panel no longer obscures graph canvas
  - Spacemouse input no longer snaps camera to origin on device disconnect

### Removed

- **Dead code pass 1** (6cc9025b) -- 19,261 lines across 36 files
  - Unused actor message variants
  - Unreachable handler branches
  - Commented-out legacy code blocks
  - Orphaned utility functions with zero callers

- **Dead code pass 2** (80b782e7) -- 2,592 lines across additional files
  - Unused imports across all handler modules
  - Dead struct fields and enum variants
  - Unreferenced test helper functions

- **Warning elimination** (627473eb) -- 474 cargo warnings reduced to 0
  - Unused variables prefixed with `_`
  - Unused imports removed
  - Dead code attributes added where removal was unsafe
  - Affects 147 Rust source files

### Security

- **Auth middleware** (3fe44e24)
  - All mutation endpoints now require authentication
  - GET/read endpoints remain public for dashboard consumption
  - `AuthenticatedUser` Actix extractor validates Bearer token + Nostr pubkey
  - `OptionalAuth` extractor available for endpoints needing mixed access

---

### Commit Log

| Hash | Category | Summary |
|:-----|:---------|:--------|
| `627473eb` | refactor | Eliminate 474 cargo warnings across 147 Rust files |
| `d394fbc5` | fix | Replace 5 placeholder graph metrics with real algorithms |
| `b03ee44c` | fix | Eliminate UB in telemetry, remove dead messages, fix CUDA naming |
| `7ffe33c5` | fix | Disable 11 inert QualityGate toggles, replace 5 coming-soon panels |
| `80b782e7` | refactor | Second-pass dead code removal + client UI honesty |
| `a86fecda` | feat | Wire agent telemetry pipeline + EventBus + fix backend stubs |
| `3fe44e24` | security | Add auth middleware to 44 mutation handlers across 8 endpoint groups |
| `48822e78` | fix | Settings changes no longer reset graph positions or vanish edges |
| `eef7a786` | fix | Use actual node positions instead of circle pattern in instance setup |
| `a76ae3b8` | fix | 3 number/string type mismatches causing all edges to silently drop |
| `6cc9025b` | refactor | Remove ~19K lines confirmed dead code across 32 files |
| `da510b29` | build | 5-stage Docker pipeline with dep caching and parallel CUDA/frontend |
| `5462b9a2` | feat | Cluster hulls, KG rings, per-mode edge color + fix physics reheat |
| `a7de546e` | fix | 4 root causes -- physics mode, infinite loop, overlay, spacemouse snap |
| `dc786e49` | feat | Multi-graph rendering with per-node type classification |
| `2c67c15d` | fix | 9 root causes -- physics startup, settings loading, edges, websocket |

### Metrics

| Metric | Before | After |
|:-------|:-------|:------|
| Cargo warnings | 474 | 0 |
| Dead code lines | 21,853 | 0 (removed) |
| Secured mutation handlers | 0 | 44 |
| Placeholder graph metrics | 5 | 0 (real algorithms) |
| Inert UI toggles | 11 | 0 (disabled with explanation) |
| Backend stubs | 5 | 0 (real implementations) |
| `static mut` UB sites | 1 | 0 (`OnceLock`) |
| Rust files touched | -- | 228 |
| Net line delta | -- | -20,406 |

---

## [1.1.0] - 2026-01-12

### Heroic Refactor Sprint - Quality Gate Achievement

**Sprint Duration:** 2026-01-08 to 2026-01-12 (5 days)

See full v1.1.0 changelog in the project root `CHANGELOG.md`.

---

## [1.0.0] - 2025-10-27

### Major Release - Hexagonal Architecture

See full v1.0.0 changelog in the project root `CHANGELOG.md`.
