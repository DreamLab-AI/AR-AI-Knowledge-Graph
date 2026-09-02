---
title: VisionClaw Current Architecture Baseline
doc_id: VC-BASELINE
version: 0.2.0
status: draft-for-ratification
verified_commit: 73540faa0
date: 2026-08-31
sources:
  - Cargo.toml
  - docker-compose.unified.yml
  - src/app_state.rs
  - src/main.rs
  - src/actors/supervisor.rs
  - src/actors/graph_service_supervisor.rs
  - src/actors/gpu/mod.rs
  - src/actors/gpu/gpu_manager_actor.rs
  - src/services/github_sync_service.rs
  - src/middleware/rbac_gate.rs
  - src/services/role_store.rs
  - src/middleware/public_demo.rs
  - crates/visionclaw-protocol/src/lib.rs
  - crates/visionclaw-protocol/src/protocols/binary_settings_protocol.rs
  - src/utils/binary_protocol.rs
changelog:
  - 0.2.0 — Data pipeline: the authored corpus is an Obsidian vault (docs/VAULT-corpus-format.md); inclusion gate is frontmatter `public: true` or `owl-class` (ADR-2040), superseding the Logseq `public:: true` property line.
  - 0.1.1 — Corrected wire-protocol citation: WireNodeDataItemV3 struct is defined in src/utils/binary_protocol.rs (webxr crate), not the visionclaw-protocol lib.rs doc-comment.
---

# VisionClaw Current Architecture Baseline

## Purpose

Ground-truth map of what actually runs today: the effective stores, processes,
actor topology, crate layout, data pipeline, GPU/XR/client surfaces, and trust
boundaries. Code wins over legacy ADR prose; every load-bearing claim cites
`file:line` at commit `73540faa0`.

## Current State

### Stores (effective persistence)

Persistence is embedded, not networked. Neo4j is fully removed — no `neo4rs` or
`neo4j` dependency exists in `Cargo.toml` (grep returns nothing; legacy ADR-132).

- **Oxigraph** (RocksDB-backed SPARQL 1.1 quad store) is the canonical knowledge
  graph + ontology store. `oxigraph = { version = "0.4" }`, `Cargo.toml:80`;
  default features include `persistence-oxigraph`, `Cargo.toml:249,268`. Opened
  once at `data/oxigraph` and shared by both repositories:
  `OxigraphOntologyRepository::open(...)` then
  `OxigraphGraphRepository::from_store(store.clone())` — `src/app_state.rs:449-456`.
- **SQLite** carries all non-triple state, one DB file per single-writer domain
  under `data/`:
  - `settings.sqlite3` — settings + RBAC role store, `src/app_state.rs:459-465`.
  - `enrichment.sqlite3` — durable `EnrichmentProposal` lifecycle, isolated writer,
    `src/app_state.rs:472-481`.
  - `liveness.sqlite3` — liveness/canary rows, `src/app_state.rs:487-491`.
  - `kpi.sqlite3` — KPI snapshot + lineage, `src/app_state.rs:517-522`.
- **Storage root**: `DATA_DIR` (default `./data`), mounted from the Docker named
  volume `visionclaw-data:/app/data` (`docker-compose.unified.yml:95,190,303`);
  `DATA_ROOT=/app/data` in production, `docker-compose.unified.yml:172`.

### Processes and ports

Single application container `visionclaw_container`
(`docker-compose.unified.yml:47`) running the Actix backend plus a Vite dev
server behind nginx:

- **actix backend** — `SYSTEM_NETWORK_PORT` default `4000`
  (`docker-compose.unified.yml:6`); health `GET /api/health`
  (`docker-compose.unified.yml:24,137`). Started via `HttpServer::new`,
  `src/main.rs:852`.
- **nginx** — dev `3001` (`DEV_NGINX_PORT`, `docker-compose.unified.yml:126`);
  production readiness `GET /readyz` on `3001` (`:211`).
- **Vite** — dev server `5173` (`VITE_DEV_SERVER_PORT`,
  `docker-compose.unified.yml:63`), proxying the API on `4000` (`:64`).

### Crate layout

The monolith split (legacy ADR-090) is **real but incomplete**. The root binary
is renamed `visionclaw-server` (`Cargo.toml` `[package] name = "visionclaw-server"`)
with `src/lib.rs` + `src/main.rs`. The workspace declares the root plus **nine**
`crates/` members (`Cargo.toml` `[workspace].members`):

`visionclaw-contracts`, `visionclaw-domain`, `visionclaw-protocol`,
`visionclaw-adapters`, `visionclaw-gpu`, `visionclaw-ontology`,
`visionclaw-actors`, `visionclaw-xr-presence`, `visionclaw-analytics-oracle`
(a tenth crate dir, `graph-cognition-extract`, is present on disk).
`xr-client/rust` (Godot gdext, Quest APK cdylib) and `agentbox/crates/headroom-napi`
are workspace-**excluded** so a server build does not compile them.

**Actor extraction is unfinished**: `src/actors/*.rs` holds **25** actor
source files while `crates/visionclaw-actors/src` holds **11** (mostly message
modules + `supervisor.rs`, `voice_commands.rs`, `protected_settings_actor.rs`).
The live supervisor tree still runs from `src/actors/`, not the crate.

### Actor system topology

Two independent Actix supervision trees, both spawned from `AppState::new`
(`src/app_state.rs`).

**Graph/service tree** — root `GraphServiceSupervisor`
(`src/actors/graph_service_supervisor.rs:422`, `impl Actor` at `:1281`), started
at `src/app_state.rs:807-809` over the Oxigraph-backed repository. It self-wires
its children in `initialize_actors` and owns the position-broadcast path:
`GraphStateActor` (`graph_state_actor.rs:834`) →
`PhysicsOrchestratorActor` (`physics_orchestrator_actor.rs:1084`) →
`ClientCoordinatorActor` (`client_coordinator_actor.rs:1151`) for WebSocket push
(`graph_service_supervisor.rs:2010,2025`). The live `ClientCoordinatorActor` is
started directly in `AppState` (`src/app_state.rs:709`) and rebound into the
supervisor via `SetClientCoordinatorAddr` (`src/app_state.rs:944` region) because
the supervisor's own child has an empty client registry. Peer actors started in
`AppState`: `MetadataActor` (`:802`), `AgentBeamActor` (`:719`, fans 0x23 agent
frames), plus `OntologyActor`, `SemanticProcessorActor`, `MetadataActor`,
`OptimizedSettingsActor`, `ProtectedSettingsActor`, `WorkspaceActor`,
`PresenceActor`, `TaskOrchestratorActor`, `ElevationActor`,
`DecisionElevationActor`, `VoiceInterfaceActor`, `MultiMcpVisualizationActor`,
`AgentMonitorActor` (each `impl Actor` in the corresponding `src/actors/*.rs`).

A generic restart supervisor also exists (`SupervisorActor`,
`src/actors/supervisor.rs:124`) with backoff, restart-window caps, and a
drain/`InitiateGracefulShutdown` path (`:207,236` region).

**GPU tree** — root `GPUManagerActor` (`src/actors/gpu/gpu_manager_actor.rs:57`,
`impl Actor` at `:140`), started at `src/app_state.rs:965` when GPU is enabled.
Refactored from a "God Actor" into a coordinator that spawns four subsystem
supervisors (`gpu_manager_actor.rs:88-121`), documented in `gpu/mod.rs:1-33`:

- `ResourceSupervisor` → `GPUResourceActor` (GPU init, timeouts).
- `PhysicsSupervisor` → `ForceComputeActor`, `StressMajorizationActor`,
  `ConstraintActor` / `OntologyConstraintActor`, `SemanticForcesActor`.
- `AnalyticsSupervisor` → `ClusteringActor`, `AnomalyDetectionActor`,
  `PageRankActor`.
- `GraphAnalyticsSupervisor` → `ShortestPathActor`, `ConnectedComponentsActor`.

Subsystems receive `SharedGPUContext` via a decentralised `GPUContextBus`
broadcast (`gpu/context_bus.rs`), not a central handle.

### Data pipeline (GitHub → Oxigraph → client)

`GitHubSyncService` (`src/services/github_sync_service.rs`) synchronises markdown
from the source repo into Oxigraph. The source is the authored **Obsidian
vault** — plain markdown with YAML frontmatter, specified in
[`VAULT-corpus-format.md`](VAULT-corpus-format.md), synced from the GitHub repo
(still literally named `jjohare/visionGraph`) with base path `pages/`. A page is
ingested as a KG node iff its frontmatter carries `public: true` **or** a
non-empty `owl-class` (formal data bypasses the publish gate); absence of both
is private, fail-closed. Legacy Logseq property lines (`public:: true`,
`owl:class::`) are tolerated only in a page's leading property block for the
bounded window named in ADR-2040. The gate anchors on parsed metadata, never on
the file path. The live ingest path extracts JSON-LD blocks
and writes **quads** through `OxigraphOntologyRepository` — `sync_graphs()`
(`:264`) / `sync_graphs_with()` (`:272`), `insert_quads_to_store()` (`:1085`);
module header `:4-6`. It clears then repopulates the store, resolves bridge edges
in a final pass (`:341-352`), rebuilds the OWL assert graph
`urn:ngm:graph:ontology:assert` (`:917`), and runs reasoning + persists inference
via `store_inference_results()` (`:1147`). The service is wired to
`GPUManagerActor` so `POST /api/admin/sync` can dispatch semantic constraints to
the GPU (`src/main.rs:449-457`; registered `src/app_state.rs:990-996`). Positions
then flow GPU → `ForceComputeActor` broadcast → `PhysicsOrchestratorActor` →
`ClientCoordinatorActor` → WebSocket binary.

### GPU physics pipeline

Dual Rust/CUDA `SimParams` (212 bytes, static assertions; legacy ADR-138/141).
`ForceComputeActor` runs force integration and drives the periodic
full-position broadcast; the SSSP result map (`node_sssp`,
`src/app_state.rs:695-698`) feeds wire slot 28. Ontology constraints
(legacy ADR-098) are wired and live via `ENABLE_CONSTRAINTS`. DAG-rank radial
layout is live (commit `73540faa0`, "hierarchical" edge label). Analytics
kernels are Partial — see divergences.

### Live wire protocol

The canonical node-data encoder is `src/utils/binary_protocol.rs` in the webxr
crate. One **52-byte** `WireNodeDataItemV3` frame, version byte `0x03`,
`centrality@48`, stride 52 (`struct WireNodeDataItemV3` at
`src/utils/binary_protocol.rs:41-51`, `WIRE_V3_ITEM_SIZE == 52` at `:72-82`).
The `visionclaw-protocol` crate's own `binary_protocol` module was removed as a
stale 48-byte copy; its `src/lib.rs:11-24` doc-comment now only points to the
webxr encoder as the single source of truth. An optional **V5 envelope**
`[0x05][u64 broadcast_seq][V3 body]` is emitted/parsed alongside `0x03`
(`crates/visionclaw-protocol/src/protocols/binary_settings_protocol.rs:222,249,350,378`).
Separate `0x23`/`0x43`/`0x44` frame types carry agent/other channels. Legacy
ADR-061 ("28B") and ADR-031 ("48B") are both stale; the V5 envelope has no
owning ADR (the Protocol Registry becomes its owner).

### XR + React clients

- **XR**: Godot 4 gdext client (`xr-client/rust`, separate workspace, Quest APK
  cdylib) forced to `gl_compatibility` — Vulkan multiview black-screens on
  SteamVR/Linux/NVIDIA (legacy ADR-136); glow/bloom off, Linear tonemap. 90fps
  validated only on VIVE + dual RTX 6000; Quest 3 unmeasured, APK unbuilt.
  Presence via `visionclaw-xr-presence` + `PresenceActor`
  (`src/actors/presence_actor.rs:380`).
- **React/web**: Vite client (`client/`) served through nginx, consuming the
  binary WebSocket position stream and the `/api` REST surface.

### Trust boundaries

RBAC is live (legacy ADR-142): Owner > Admin > Editor > Viewer on NIP-98 pubkeys,
enforced by `RbacGate` middleware (`src/middleware/rbac_gate.rs`), role store in
`settings.sqlite3` (`src/services/role_store.rs`). **Shipped posture is open by
default via compose**: the structural code default for `RBAC_PUBLIC_READS` is
`false` (`rbac_gate.rs:122-128`, `unwrap_or(false)`), but `docker-compose.unified.yml:78`
sets `RBAC_PUBLIC_READS=1`, `:79` sets `RBAC_ALLOW_OWNERLESS=1`, and an unassigned
authenticated pubkey resolves to `Editor` (`role_store.rs:191`, test `:546-549`).
`PUBKEY_VISIBILITY_FILTER` defaults `1` in compose (`:86`); the code-level demo
read-only guard defaults off (`public_demo.rs:29-33`, `unwrap_or(false)`).

## Known divergences and open items

- **Security fixes landing 2026-08-31** (write-up reflects post-fix state):
  (1) `PUBKEY_VISIBILITY_FILTER` default flipped ON (encoder existed but was
  inert); (2) NIP-98 single-use event-id replay cache added in
  `src/utils/nip98.rs` (was ±60s window, no replay protection); (3) agentbox AoE
  `:9095` gains token auth (was `--auth none`), staged for next image rebuild.
- `?token=` accepted on `/wss` in release, contradicting legacy ADR-011 — medium,
  log-hygiene; header path also exists.
- NIP-26 delegation not wired — `nostr_bridge.rs` re-signs under the bridge key;
  fail-closed NIP-26 deferred (legacy Phase 5). Pod signing can fall back unsigned
  (agentbox ADR-026).
- **Actor extraction incomplete**: 25 `src/actors/*.rs` vs 11 in
  `crates/visionclaw-actors`. Live tree runs from `src/`.
- **BrokerActor never merged**; main uses a stateless ACSP producer + a
  cherry-picked storage-agnostic domain broker kernel (~936 LOC).
- **GPU analytics Partial**: Louvain (`sigma_tot` race → converges ~0), DBSCAN
  (border points → noise), PageRank (dangling-kernel block bound bug); LOF fixed.
  Node embeddings (legacy ADR-072) are effectively random (hash bag-of-characters).
- **Sources-of-truth conflict**: Oxigraph (132) vs Pod write-master (050/052) vs
  GitHub `public::true` (051) vs RuVector agent memory (030). `deleteAgentMemory()`
  has no reverse tombstone to RuVector — deletion does not revoke agent memory.
- **No estate-wide erasure/backup design**: `scripts/backup-sqlite.sh`
  (2026-08-31) covers SQLite only; no point-in-time Oxigraph backup, no
  cross-store consistent restore, no RPO/RTO.
- **Identifier grammars unreconciled** (vc:, urn:visionclaw:, visionclaw:owner:,
  hex/npub), DID doc ADR-074-D2' vs ADR-125 conflict.
- **SOPS** (legacy ADR-109) accepted 2026-05-09, never executed — `.env`
  plaintext today, no SOPS artifacts.

## Invariants (must not silently change)

- Persistence is Oxigraph (`data/oxigraph`) + per-domain SQLite files under
  `DATA_DIR`; a single Oxigraph store is shared by ontology and graph
  repositories. No networked graph DB.
- Live node wire format is 52-byte V3 (`0x03`), optional V5 envelope
  (`0x05` + `u64 seq` + V3 body). Any width change is a protocol break.
- Position broadcast path: GPU → `ForceComputeActor` → `PhysicsOrchestratorActor`
  → `ClientCoordinatorActor` → WebSocket. The live `ClientCoordinatorActor` must
  be the one clients register with (registry not empty).
- Backend on `4000`, nginx `3001`, Vite `5173`, all inside `visionclaw_container`.
- RBAC open-by-default is a compose-env choice, not a structural code default;
  changing the compose defaults changes the security posture and requires a named
  security profile.

## Change process

This is a living document. Update it in the same PR as any change to store
topology, actor tree, wire protocol, port map, or trust boundary. Re-verify
`verified_commit` and cited `file:line` anchors on each edit; bump `version`
(semver: patch for citation refresh, minor for new subsystem, major on invariant
change). Historical rationale stays in the legacy corpus
(`docs/adr`, `docs/prd`, `docs/ddd`) — cite, do not narrate. Ratification promotes
`status` from `draft-for-ratification` to `ratified`.
