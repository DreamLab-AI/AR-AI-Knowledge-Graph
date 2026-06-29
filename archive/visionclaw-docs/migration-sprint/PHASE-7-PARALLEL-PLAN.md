# PHASE-7-PARALLEL-PLAN — Implementation Plan

Status     : Planning
Date       : 2026-05-16
Branch     : impl/phase-7-parallel (off radical-rollback @ d260a6158)
Owner      : anthropic@xrsystems.uk
Depends on : Phases 1, 2, 2.5, 3, 4, 5 (landed); Phase 6 (Sections 8, 11 landed)

## Overview

Phase 7 is the final parallel landing of five sections plus the remaining
Auth decisions (D3-D11). Although these sections can true-parallel after
the `visionclaw-contracts` crate lands, a strict in-Phase-7 ordering is
required for two reasons:

1. `crates/visionclaw-contracts` is consumed by Sections 7 (agent-click
   envelope), 10 (telemetry + enterprise envelope types), and 12 (XR
   presence + action contracts). It must land first.
2. Auth D3-D11 is the security gate: routes without a three-state auth
   declaration (`RequireAuth` / `OptionalAuth` / `Public`) do not compile
   past D3's build-script lint. All other sections add routes and must
   therefore land after D3-D11 are in place.

Estimated Phase 7 wall time (fully parallel after step 2): 8-12 engineering
days spread across 6 sub-worktrees.

---

## 1. Per-Section Task Breakdown

### 1.1 Section 05 — Settings & Control Panel

Branch: `impl/phase-7-settings`
Complexity: MEDIUM (4 deliverables, 1 new infra piece)
Consuming from prior phases: ADR-11 `SettingsRepository` trait (Phase 1),
  Nostr auth middleware (Phase 2.5), ADR-05 D5 schema authority (TC-5 landed).

Tasks:

S05-1. Schema generator (ADR-05 D1)
  - Write Node script that parses `client/src/features/settings/config/settings.ts`
    AST and emits:
    - `client/src/types/generated/settings.ts` (TypeScript mirror)
    - `src/config/generated_settings.rs` (Serde structs + embedded schema_version)
  - Wire into `package.json prepublish` and a CI step.
  - Target generator full-run < 2s.
  - Deliverable: generator script + CI check `generated files must match fresh run`.

S05-2. Defaults equality test (ADR-05 D2)
  - Add `client/src/api/__tests__/settingsApi.defaults.test.ts` that posts
    the `DEFAULTS` constant against a test fixture of `AppSettings::default()`
    and asserts deep-equal.
  - Ensures default drift is a CI failure.

S05-3. UI definition declarative audit (ADR-05 D3)
  - Split `settingsUIDefinition.ts` into one file per tab (10 files + index.ts).
    Cap: ~50 entries per file.
  - Migrate imperative widget escapes to declarative entries in `widgetTypes.ts`.
  - Move `viewportSettings.ts` schema fragments into `settings.ts`; move its
    UI fragments into `settingsUIDefinition/viewport.ts`.
  - Deliverable: 100% declarative panel — no `if` statements constructing widgets.

S05-4. Widget/value cross-check (ADR-05 D4)
  - Add `TypeMismatchCell` component: renders schema path, expected shape,
    offending value (truncated, JSON.stringify-encoded).
  - Add regression test in `controlPanel.shape.test.tsx`: feed object into
    primitive widget, assert mismatch cell renders, assert no `[object Object]`.

S05-5. Settings save endpoint (ADR-05 D6)
  - Move save from WebSocket path to `POST /api/settings` behind standard
    Nostr middleware (Phase 2.5).
  - Handler: resolve pubkey → validate via generated validator → call
    `SettingsRepository::save_partial` → return 200 with post-save `AppSettings`.
  - Return 401 (not 403) for anonymous requests.
  - Add integration test: anonymous → 401; authenticated Nostr session → 200.

S05-6. Prune dead UI (ADR-05 D8)
  - Delete 11 inert Quality Gate toggles from `unifiedSettingsConfig.ts` and
    supporting widget files.
  - Delete 5 Coming Soon panel component files.
  - Remove corresponding schema entries from `settings.ts` (triggers generator
    re-run; ensures no stale entries survive in generated mirror).
  - Verify control panel renders at most 10 tabs after deletion.

S05-7. Section 4 cross-cut consolidation (ADR-05 D9)
  - Confirm `GemMaterialSettings` and `GlowSettings` types are generated from
    `settings.ts`, not declared independently.
  - Confirm defaults originate from `settingsApi.ts`.
  - Delete any duplicate type declarations found in renderer files.

Acceptance gates (mapped from PRD-05):
  - AC1: generator check passes in CI (S05-1)
  - AC2: 10 tabs, all populated from definition only (S05-3, S05-6)
  - AC3: no `[object Object]` in regression test (S05-4)
  - AC4: settings save returns 401 anonymous / 200 authenticated (S05-5)
  - AC5: SQLite migration `0001_user_settings.sql` round-trips (Phase 1 ADR-11,
         verified here by integration test)
  - AC6: widget count drops from 227 to ≤ 210 (S05-6)

Estimated effort: 5 days (generator infra is new; rest is refactor + prune).

---

### 1.2 Section 06 — Auth D3-D11

Branch: `impl/phase-7-auth-d3-d11`
Complexity: HIGH (security gate for all other sections, 9 decisions)
Note: D1 (compile-time gate) and D2 (--allow-skip-auth CLI flag) landed in
Phase 2.5. This sub-worktree lands D3 through D11 only.

Tasks:

A06-D3. Three-state route declaration + build-script lint
  - Add `compile_routes_have_auth!` build-script lint that scans
    `src/handlers/**/*.rs` for `.route(` / `.service(` calls inside
    `pub fn config*` and checks for sibling `.wrap(RequireAuth` or
    `.wrap(Public::marker())`.
  - Initially warn; hard error once all existing handlers have been classified
    (A06-D4 complete).
  - Deliverable: lint tool + CI step.

A06-D4. Endpoint audit table implementation
  - Apply the ADR-06 D4 classification table (35 handler modules) to
    `src/main.rs` and each handler's config function:
    - Add `RequireAuth::authenticated()` / `::power_user()` / `::admin()` wraps
      to the 22+ previously unguarded handlers.
    - Restore the `RequireAuth` wrapper on `api_handler::analytics` (currently
      commented out with "temporarily disabled for testing").
    - Delete `solid_proxy_handler.rs` (dead endpoint with auth surface).
    - Delete `settings_handler.rs.temp` (stale file).
  - Deliverable: every route explicitly classified; build-script lint passes.

A06-D5. CSP tightening
  - Update `client/index.html` meta-tag CSP:
    - Remove bare `wss:` / `ws:` from connect-src; replace with `'self'` plus
      explicit Polyhaven allowlist.
    - Remove `'unsafe-inline'` from style-src; add nonce-based styles.
    - Add `frame-ancestors 'none'` and `base-uri 'self'` and `form-action 'self'`.
  - Add Actix middleware that sets `Content-Security-Policy` response header on
    every HTML response with a per-response nonce injected into rendered HTML.
  - Restore commented-out CSP meta-tag in `client/dist/index.html`.
  - Ship in Report-Only mode first (one-week window for violation reports via
    `report-uri`), then switch to enforced.
  - Deliverable: CSP delivered three ways; `'unsafe-inline'` absent from style-src.

A06-D6. Audit log
  - The `audit_log` table schema is owned by ADR-11 §D5 (already enumerated
    there per TC-5). This task wires up the application layer:
    - Add `AuditLog` Actix middleware wrapping state-mutating routes (POST, PUT,
      PATCH, DELETE) after `RequireAuth`.
    - Async write via `tokio::sync::mpsc` bounded channel, capacity 1024.
      Drop-on-full + increment metric counter.
    - Add `GET /api/admin/audit?since=&pubkey=&path=` behind `RequireAuth::admin()`.
  - Deliverable: audit rows written on each authenticated mutation; query endpoint works.

A06-D7. Docker socket: scope or remove
  - Coordinate with Section 09 sub-worktree.
  - If the Docker socket is required (for container enumeration in
    `launch.sh` health checks), add `tecnativa/docker-socket-proxy` sidecar
    with `CONTAINERS=1 POST=0 DELETE=0`.
  - If not required, remove the bind-mount from `docker-compose.unified.yml`.
  - Deliverable: no bare `/var/run/docker.sock` mount in release image.

A06-D8. Trusted proxy boundary
  - Configure Actix to trust `X-Forwarded-Proto` / `X-Forwarded-Host` only
    from the CIDR in `VISIONCLAW_TRUSTED_PROXIES` (default `127.0.0.1/32`).
  - Strip forwarded headers from any other source.
  - Deliverable: NIP-98 URL reconstruction uses validated forwarded headers only.

A06-D9. Handler-layer unwrap audit
  - Scan `src/handlers/` and `src/middleware/` for `unwrap()` calls.
  - Classify each per ADR-06 D9 rules:
    - User-controlled input → replace with `?` returning structured 4xx.
    - Boot-time invariant → `.expect("loaded at boot, invariant")`.
    - Genuinely infallible → `// SAFETY:` comment retained.
  - Enable `clippy::unwrap_used = warn` in `src/handlers/`.
  - Deliverable: zero `unwrap()` on user-controlled parse sites in handlers.

A06-D10. Nostr canonicalisation cross-validation
  - Implement `canonicalise_url` spec as a numbered list (per CC-Gaps item 5):
    scheme as forwarded, host lowercase with default port elided, path as-is,
    query order preserved, fragment stripped.
  - Rust impl: `src/services/nostr_service.rs`.
  - TypeScript impl: `client/src/services/nostrAuthService.ts`.
  - Cross-validation test suite: fixed corpus of URL pairs, both impls must
    produce identical canonical strings.

A06-D11. WebSocket endpoint enumeration
  - Already documented as the canonical 11-row table in ADR-06 §D12 (per T4
    resolution applied in the sprint docs phase). Implementation task:
    add `scripts/ci/check-ws-route-enumeration.sh` that parses `src/main.rs`
    route registrations and asserts every `.route("/ws...")` / `.route("/wss")`
    appears as a row in the table.
  - Remove `/ws/mcp-relay` and `/api/multi-mcp/ws` (re-homed in agentbox per
    ADR-06 D12 "REMOVE Phase 7").

Acceptance gates (PRD-06):
  - AC-A1: `strings target/release/webxr | grep BYPASS` returns 0 (Phase 2.5 gate;
           verify here that D1/D2 remain intact through D11 landing)
  - AC-A2: every route has RequireAuth / OptionalAuth / Public declaration (D3, D4)
  - AC-A3: endpoint audit table implemented and lint passes (D3, D4)
  - AC-A4: CSP `'unsafe-inline'` absent from style-src (D5)
  - AC-A5: audit log rows written and queryable (D6)
  - AC-A6: Docker socket scoped (D7)

Estimated effort: 6 days (endpoint audit + CSP are the largest pieces).

---

### 1.3 Section 07 — Bots & Agent Telemetry

Branch: `impl/phase-7-bots`
Complexity: MEDIUM (significant deletion + one new consumer path)
Depends on: visionclaw-contracts crate (for AgentActionEnvelope types),
            Auth D3-D11 (for route posture of retained telemetry endpoints).

Tasks:

B07-1. Delete polling infrastructure (ADR-07 D2)
  - Remove files:
    - `client/src/features/bots/services/AgentPollingService.ts`
    - `client/src/features/bots/hooks/useAgentPolling.ts`
    - `client/src/features/bots/config/pollingConfig.ts`
    - `client/src/features/bots/utils/pollingPerformance.ts`
    - `client/src/features/bots/docs/polling-system.md`
  - Narrow `BotsDataContext` type: remove `pollNow`, `configurePolling`,
    polling status field.
  - Deliverable: no import of agentPollingService anywhere; PRD-07 A2 verified
    by grep gate.

B07-2. Consolidate server-side agent graph into GraphStateActor (ADR-07 D3)
  - Delete static `BOTS_GRAPH: Lazy<Arc<RwLock<GraphData>>>` in `bots_handler.rs`.
  - Agent nodes and edges added to `GraphStateActor` with class-flag bit
    discrimination (per ADR-08 §D6 allocation, now canonically resolved via TC-1).
  - Dual-graph X-offset: apply `bots.agent_x_offset` (default 600 units) once
    at agent spawn in `GraphStateActor`; maintain via per-class gravity coefficient.

B07-3. Telemetry WebSocket consumer in BotsDataContext (ADR-07 D1)
  - Feature-flag `bots.use_telemetry_v2` defaults false initially.
  - Consumer reads from `/ws/agent-telemetry` (Section 10 contract),
    decoded on main thread per CC-14 resolution (not via worker proxy).
  - Wire-to-internal event mapping via the ACL table from CC-1 resolution:
    `snapshot → SwarmSnapshot`, `delta → AgentPositionUpdated / AgentStatusChanged`,
    `agent_added → AgentJoined`, `agent_removed → AgentDeparted`,
    `heartbeat → Heartbeat`, `communication → AgentCommunicated`.
  - Coalescer: batch up to `bots.coalescer_max_batch` (64) events per rAF.
  - Flip flag default to true after parity verified against recorded swarm session.

B07-4. Phase 7a deprecation shims (ADR-07 D12, T6 resolution)
  - Replace handler bodies for 5 deletion-candidate routes with `410 Gone`
    + `Link: <successor>; rel="successor-version"` responses.
  - Routes: `POST /api/bots/initialize-swarm`, `POST /api/bots/spawn-agent-hybrid`,
    `POST /api/bots/spawn-agent`, `POST /api/bots/data`, `POST /api/bots/update`,
    `DELETE /api/bots/remove-task/{id}`.
  - 410 response body includes `error`, `code`, `message`, `successor`,
    `deprecated_since`, `scheduled_removal` per T6 schema.
  - Expose metric `bots_deprecated_route_calls_total{route}`.
  - Remove client-side callers: `BotsControlPanel.tsx:71,101`,
    `MultiAgentInitializationPrompt.tsx:162-163`, `AgentControlPanel.tsx:110`.

B07-5. Delete BotsControlPanel; retain read-only SystemHealthPanel (ADR-07 D8)
  - Delete `client/src/features/bots/BotsControlPanel.tsx`.
  - Evaluate `SystemHealthPanel.tsx` for read-only value; retain if so, otherwise delete.
  - Split `AgentTelemetryStream.tsx`: extract subscribe/unsubscribe control buttons
    into a separate component that is deleted; keep pure display component.

B07-6. Click-through via AgentActionEnvelope (ADR-07 D8)
  - Wire agent capsule click handler to construct `AgentActionEnvelope` using types
    from `crates/visionclaw-contracts` (imported as generated `.d.ts`).
  - Dispatch via session-chosen transport (BroadcastChannel / deep-link / postMessage)
    per ADR-10 D3.
  - Deliverable: click trace shows well-formed envelope on chosen transport;
    no in-process control panel rendered.

B07-7. Communication edge decay (ADR-07 D5)
  - Server-side: `AgentCommunicated` event creates/refreshes edge with TTL
    `bots.communication_edge_decay` (5.0s). Alpha falls linearly.
  - Coalescer: collapses bursts within window to one effective edge.

B07-8. Add bots settings to Section 5 schema
  - Add 6 settings under `bots.*` namespace to `settings.ts`:
    `agent_x_offset`, `agent_ttl_seconds`, `communication_edge_decay`,
    `idle_heartbeat_seconds`, `coalescer_max_batch`, `click_forward_target`.
  - Re-run generator (S05-1 deliverable).

Acceptance gates (PRD-07):
  - A1: every agent has a telemetry event within `agent_ttl` (B07-3)
  - A2: no import of AgentPollingService anywhere (B07-1 grep gate)
  - A3: dual-graph X-offset applied at spawn (B07-2)
  - A7: click produces well-formed AgentActionEnvelope on chosen transport (B07-6)
  - A8: idle bandwidth ≤ 1 heartbeat per 30s with zero agents (B07-3 ADR-07 D10)

Estimated effort: 4 days (deletion is quick; consumer + deprecation shims are the bulk).

---

### 1.4 Section 09 — Ecosystem Services & Launch

Branch: `impl/phase-7-ecosystem`
Complexity: LOW-MEDIUM (infra only; no application semantics)
Depends on: Auth D7 (Docker socket scoping decision coordination).

Tasks:

E09-1. Finish visionclaw_network migration (ADR-09 D2)
  - Grep tree for `visionclaw_network`; replace every occurrence with
    `${EXTERNAL_NETWORK:-visionclaw_network}` or delete containing file.
  - Confirm `docker-compose.unified.yml` default changed.
  - Delete `scripts/fix_kokoro_network.sh` (reconnect logic moved into
    `start_kokoro` per ADR-09 D5a).
  - Add CI gate: `! grep -rn visionclaw_network . --exclude-dir=.git`.

E09-2. Ecosystem service GPU device configurability (ADR-09 D5b)
  - Replace hardcoded `device=2` for Kokoro with `${KOKORO_GPU_DEVICE:-2}`.
  - Add `WHISPER_GPU_DEVICE` and `XINFERENCE_GPU_DEVICE` env vars.
  - Document defaults in `.env.example`.
  - Single-GPU host default: `device=0` for all three when `SINGLE_GPU=1`.

E09-3. Stopped-container network reconnect generalisation (ADR-09 D5a)
  - Generalise the Kokoro reconnect pattern (fixed in `28c3521bb`) to
    Whisper and Xinference: after `docker start <name>`, check network
    membership and `docker network connect "$ECOSYSTEM_NETWORK" <name>`
    if not already connected.

E09-4. Consumers mapping (ADR-09 D5d, CC-17 resolution)
  - Add §D5d consumers table to ADR-09 (documentation already written in
    the sprint docs; verify it is present and accurate).
  - Implementation: confirm each consumer handler (`speech_socket_handler`,
    `inference_handler`, `ragflow_handler`) references the correct service
    hostname from env vars, not hardcoded IPs.

E09-5. wasm-builder Dockerfile stage (CC-8 resolution)
  - Add `wasm-builder` stage to `Dockerfile.unified` between `rust-deps`
    and `node-builder`:
    - Runs `wasm-pack build --target web --release client/crates/scene-effects/`
    - Outputs to `client/src/wasm/scene-effects/`
  - `node-builder` stage: add `COPY --from=wasm-builder` before `vite build`.
  - Integration test: production image contains non-zero `.js` and `.wasm`
    files in `/usr/share/nginx/html/wasm/scene-effects/`.

E09-6. Content-hash incremental rebuild in rust-backend-wrapper.sh (ADR-09 D7)
  - Implement step 2 of D7: content-hash all `.rs` and `.cu` under `/app/src/`
    and compare against `/app/target/.source-hash`.
  - On hash match: skip rebuild and exec cached binary.
  - On mismatch: surgical remove of `webxr` crate incremental artefacts only,
    then `cargo build`.
  - Preserve registry / git caches across runs.

E09-7. Docker socket coordination (ADR-06 D7)
  - Coordinate with Auth sub-worktree (A06-D7).
  - If Docker socket must remain: add `tecnativa/docker-socket-proxy` to
    `docker-compose.unified.yml`; update `launch.sh` to use the proxy endpoint.
  - If removable: delete bind-mount.
  - Deliverable: ADR-09 records final decision.

Acceptance gates (PRD-09):
  - AC4: no `visionclaw_network` literal in tree (E09-1)
  - AC5: `visionclaw_network` CI grep gate passes (E09-1)
  - AC6: rust-backend-wrapper preserves cargo caches (E09-6)
  - AC7: production image contains WASM artefacts (E09-5)

Estimated effort: 3 days (mostly grep-and-replace + one Dockerfile stage).

---

### 1.5 Section 10 — External Integrations

Branch: `impl/phase-7-external`
Complexity: HIGH (new crate creation; cross-section coordination point)
Note: this sub-worktree MUST LAND FIRST within Phase 7. Other sub-worktrees
that consume `visionclaw-contracts` (Sections 7, 12) cannot proceed
without it.

Tasks:

X10-1. Create crates/visionclaw-contracts (T7 resolution, ADR-10 D3)
  - Scaffold new Rust crate at `crates/visionclaw-contracts/`.
  - Add to workspace `Cargo.toml`.
  - Implement in `src/agent_action.rs`:
    - `AgentActionEnvelope` struct with all fields per ADR-10 D3 schema.
    - `AGENT_ACTION_CHANNEL`, `AGENT_ACTION_DEEP_LINK_TEMPLATE` constants.
    - `AgentActionTargetOrigin` type.
    - `#[derive(ts_rs::TS)]` for `.d.ts` generation.
  - Run `ts-rs` test to generate `client/src/types/contracts/agent-action.d.ts`.
  - Also add envelope types for `AgentTelemetryEnvelope` and
    `EnterpriseEventEnvelope` (referenced from ADR-10 D1 and D5).
  - Publish as `@visionclaw/contracts` npm package skeleton (package.json,
    index.d.ts re-export of generated `.d.ts` files).
  - Deliverable: `cargo build -p visionclaw-contracts` passes; generated `.d.ts`
    committed and CI-checked for byte-identical regeneration.

X10-2. Inbound telemetry WebSocket endpoint (ADR-10 D1)
  - Server-side: add `/ws/agent-telemetry` endpoint.
  - Auth: `RequireAuth` per ADR-06 D12 table.
  - Implement schema_version check: version skew → close frame 4001.
  - Back-pressure: drop-never-queue (drop oldest frames, increment metric
    `telemetry_dropped_frames_total`).
  - Wire unknown `type` → log once, ignore, continue.
  - Missing required fields → drop frame + increment `telemetry_malformed_count`.

X10-3. Telemetry reconnection (ADR-10 D2)
  - Client-side: on disconnect, mark agents stale after 5s.
  - Reconnect with backoff 1s, 2s, 4s, 8s, 16s, cap 30s, jitter ±20%.
  - On reconnect: agentbox sends snapshot; client reconciles add/remove; clears stale.
  - Client never replays local state to agentbox.

X10-4. Enterprise events WebSocket endpoint (ADR-10 D5)
  - Server-side: add `/ws/enterprise-events` endpoint.
  - Auth: `RequireAuth` per ADR-06 D12.
  - Accept three event types: `membership_change`, `role_change`, `session_revoked`.
  - Wire effects: role_change updates JWT claim; session_revoked drops JWT + prompts re-auth.

X10-5. Auth bridge (ADR-10 D4)
  - Implement the 7-step challenge-response flow for forum → VisionClaw identity bridging.
  - Session JWT stored in sessionStorage (not localStorage, not cookie).
  - Challenge replay-resistance: single-use, 60s server-side window.
  - Deliverable: auth bridge test (connect → challenge → sign → verify → JWT issued).

X10-6. Whelk pure-function adapter (ADR-10 D6)
  - Add `src/adapters/ontology/whelk_reasoner.rs` implementing the
    `WhelkReasoner` trait:
    - `fn infer(&self, req: WhelkInferenceRequest) -> Result<WhelkInferenceResponse>`
    - No reads from disk, no network, no global state, no log side effects.
  - Golden-file regression fixture:
    `tests/contracts/external-integrations/whelk_golden.rs` with fixed
    (TBox, ABox) → expected inferred triples.

X10-7. Enterprise guard CI check (ADR-10 D7)
  - Add `cargo xtask check-no-enterprise` CI step that scans disallowed names
    (`broker`, `workflows`, `connectors`, `mesh_metrics`, `policy`, `decision_canvas`,
    `kpi`, `EnterpriseDrawer*`, `enterprise-standalone`) under `src/handlers/`
    and `client/src/features/enterprise/`.
  - Append deprecated bots route name greps per T6 resolution.
  - Deliverable: CI step blocks any re-introduction.

X10-8. BroadcastChannel naming convention (CC-13 resolution)
  - Document `visionclaw:` prefix convention in ADR-10.
  - Add grep CI: every `BroadcastChannel(` literal in `client/src/` matches
    `visionclaw:[a-z-]+`.

X10-9. GitHub adapter documentation (CC-15 resolution)
  - ADR-10 D11 already written in sprint docs. Implementation task:
    verify `src/services/github_sync_service.rs` uses `octocrab`, produces
    `ParsedMarkdown` value objects, and the parse-error envelope matches
    the schema in D11.
  - Add contract test `tests/contracts/external-integrations/github_adapter.rs`.

X10-10. Contract test harness (PRD-10 §7)
  - Add `tests/contracts/external-integrations/` directory.
  - Agentbox-emulator fixture: sends every variant of every envelope;
    consumer accepts or rejects with structured error.
  - AgentActionEnvelope contract test: every variant built, receiver rejects
    wrong type/version; origin check fails on unlisted origin.

Acceptance gates (PRD-10):
  - A1: inbound telemetry carries schema_version; version skew closes 4001 (X10-2)
  - A2: drop-on-backpressure with metric (X10-2)
  - A3: exactly one transport per session, no runtime fallback (X10-1 + Section 7)
  - A4: auth bridge is signature-verified, not cookie-shared (X10-5)
  - A6: whelk runs as pure function (X10-6)
  - A8: no enterprise control logic in codebase (X10-7)

Estimated effort: 6 days (new crate + two new WS endpoints + contract harness are the bulk).

---

### 1.6 Section 12 — XR Client (Godot + gdext)

Branch: `impl/phase-7-xr`
Complexity: MEDIUM-HIGH (greenfield Godot project + gdext bridge)
Depends on: visionclaw-contracts crate (for visionclaw-xr-presence types),
            Auth D3-D11 (for /ws/xr-presence endpoint posture),
            Section 2 (V3 protocol is the input contract — already landed).

Tasks:

XR12-1. Godot project scaffold (ADR-12 D1, PRD-12 F3)
  - Create `xr-client/` with `project.godot`, `xr_boot.gd` boot scene.
  - Add material `.tres` files: `gem.tres`, `crystal_orb.tres`, `agent_capsule.tres`.
  - Configure Android export: `export_presets.cfg`, `android-export-template-config.txt`,
    `permissions-required.md`.
  - Verify headless Godot starts and loads the boot scene without OpenXR.

XR12-2. gdext bridge crate (ADR-12 D3)
  - Create `xr-client/rust/` as a gdext workspace crate.
  - Add `xr-client/visionclaw_xr.gdextension` extension descriptor.
  - Implement three Godot classes:
    - `VisionclawProtocol`: `decode_frame(bytes: PackedByteArray) -> XRGraphState`
      - Zero-copy decode from WebSocket frame into `PackedFloat32Array`.
      - `XRGraphState` resource carries positions, node_ids, type_flags.
      - Must mirror `src/utils/binary_protocol.rs` byte-for-byte (ADR-12 D2).
    - `VisionclawAuth`: `request_challenge(url) -> Signal`,
      `sign_and_verify(challenge) -> String` (returns JWT).
      - Uses `nostr-sdk` for BIP-340 Schnorr signing.
      - Platform secret store impl: `xr-client/rust/src/secret_store.rs`
        with trait + Android Keystore / libsecret / DPAPI impls.
    - `VisionclawPresence`: `start(url) -> Signal`; batches to 30Hz internally.
  - Deliverable: `.so` loads in headless Godot; `VisionclawProtocol::decode_frame`
    produces correct positions from a synthetic V3 frame.

XR12-3. Create crates/visionclaw-xr-presence (ADR-12 D9, PRD-12 F11)
  - Standalone Rust crate with `XRPresenceFrame` struct (≤ 256 bytes).
  - Encodes head pose, hand poses (left/right joint poses, pinch state),
    gaze direction.
  - 30Hz publish to `/ws/xr-presence` (server-side: v1 sink that logs to
    metrics and discards).
  - The presence crate is the shared format for Godot client and any future
    native non-Godot consumers.

XR12-4. Visual primitives: MultiMesh renderers (PRD-12 F3, F4, F5)
  - Three `MultiMeshInstance3D` nodes: gem (Icosahedron r=0.5), crystal orb
    (Sphere r=0.5), agent capsule (Capsule r=0.3 h=0.6).
  - Class membership from type-flag bits in node_id (per ADR-08 §D6 — TC-1 resolved).
    XR client reads bits, does not re-classify.
  - Edges: `MultiMeshInstance3D` of unit-height CylinderMesh (r=0.03, h=1).
    Placed at midpoint, scaled Y to inter-node distance, rotated to align src→tgt.
    Surface-to-surface offset: shorten by `(srcR + tgtR)`.
  - Edge capacity growth: starts at `min(count * 1.5, ceiling)`, doubles on overflow,
    never shrinks, ceiling from project setting `xr/rendering/max_edges_ceiling`
    (default 64,000). No `MAX_EDGES` top-level constant anywhere.

XR12-5. Hand-tracking interactions (PRD-12 F7, ADR-12 D5)
  - Pinch on node (thumb-index distance < 0.025m): highlights node.
  - Pinch-and-drag: lateral camera rig pan.
  - Gaze + pinch (>250ms gaze dwell): selects node, opens floating info panel.
  - Two-handed pinch: pinch-zoom, clamped [0.1, 10.0] scale factor.
  - Controller fallback (ADR-12 D5): trigger = pinch, thumbstick = snap-turn/dolly,
    grip = pinch, menu = settings panel.
  - Interaction state in `xr-client/scripts/interaction_state.gd`.

XR12-6. Comfort policy (PRD-12 F8, ADR-12 D6 — non-negotiable)
  - Snap-turn: exactly 30° rotation increments, 100ms vignette on discontinuity.
  - Vignette on translation: `xr-client/shaders/comfort_vignette.gdshader`.
    Activates above 0.5 m/s; inner radius 0.4, outer 0.9.
  - IPD-aware near plane: `max(0.05, ipd * 0.4)` set at scene start.
  - None of these are configurable in v1.

XR12-7. Label3D pool (PRD-12 F6)
  - Pool of `Label3D` nodes, `BILLBOARD_ENABLED`, recycled per frustum-cull pass.
  - Pool size: `max_visible_labels` (default 512).
  - Layout rebuild every 3 frames; position patch every frame from shared
    `PackedFloat32Array`.
  - Text from V3 label slot or lazy fetch from `GET /graph/node/:id/label`.

XR12-8. Agent telemetry display (PRD-12 F10)
  - Floating Sprite3D + Label3D panels above agent nodes within 5m of camera.
  - Shows: name, truncated current task (40 chars), health indicator.
  - Panel pool pre-allocated at `max_visible_agent_panels` (default 32).

XR12-9. Performance benchmark fixture (ADR-12 D7, PRD-12 F9)
  - Create `xr-client/perf/fixtures/perf_graph_1k.json` (1k-node graph).
  - Write `xr-client/perf/run_benchmark.gd`: loads fixture at 5x scale.
    Records median GPU frame time, p95 GPU frame time, heap allocs/s, settle time.
  - Write `xr-client/perf/regression_check.py`: parses output, compares against
    `xr-client/perf/baselines/quest3.json`, fails build if any metric regresses > 5%.
  - CI: headless half (allocations, settle time) on every PR; GPU half on Quest 3 nightly.

XR12-10. GUT test suite (PRD-12 F12)
  - `test_scene_load.gd`: boot scene loads, all 3 material `.tres` resolve,
    `XRGraphState` resource registered.
  - `test_protocol_decode.gd`: synthetic V3 frame → assert PackedFloat32Array
    length and per-node positions match expected.
  - `test_comfort_policy.gd`: snap-turn produces exactly 30°, vignette activates
    above 0.5 m/s.
  - `test_capacity_growth.gd`: edge MultiMesh capacity doubles on overflow,
    never shrinks, never exceeds ceiling.

Acceptance gates (PRD-12):
  - A1: XR client connects with no server-side changes (XR12-2 + existing server)
  - A2: 90Hz on Quest 3 with 5k nodes in SETTLED state (XR12-9 CI gate)
  - A3: Nostr challenge-response ≤ 1s warm start; JWT in memory only (XR12-2)
  - A4: three node geometries render with specified materials (XR12-1, XR12-4)
  - A6: no `MAX_EDGES` constant anywhere in XR sources (XR12-4)
  - A8: snap-turn exactly 30°, vignette at 0.5 m/s (XR12-6, XR12-10)
  - A12: XR presence at 30Hz without impacting graph data WebSocket (XR12-3)

Estimated effort: 8 days (greenfield Godot project is the largest single item).

---

## 2. Sub-Worktree Spawn Plan

All sub-worktrees branch off `impl/phase-7-parallel`.

```
impl/phase-7-parallel  (base — off radical-rollback @ d260a6158)
├── impl/phase-7-external       [FIRST — landing gate for crate]
├── impl/phase-7-auth-d3-d11    [SECOND — security gate for all routes]
├── impl/phase-7-settings       [THIRD wave — parallel after above two]
├── impl/phase-7-bots           [THIRD wave — parallel, needs contracts crate]
├── impl/phase-7-ecosystem      [THIRD wave — parallel, coordinate D7 with auth]
└── impl/phase-7-xr             [THIRD wave — parallel, needs contracts crate]
```

Worktree creation commands (to be run from the phase-7-parallel worktree or host):

```bash
git worktree add ../phase-7-external    impl/phase-7-external
git worktree add ../phase-7-auth-d3-d11 impl/phase-7-auth-d3-d11
git worktree add ../phase-7-settings    impl/phase-7-settings
git worktree add ../phase-7-bots        impl/phase-7-bots
git worktree add ../phase-7-ecosystem   impl/phase-7-ecosystem
git worktree add ../phase-7-xr          impl/phase-7-xr
```

Each sub-worktree:
1. Runs `lazy init` + `lazy gather` for its task scope.
2. Implements using lazy-fetch plan tracking.
3. Results merge back to `impl/phase-7-parallel` via standard git merge.

Merge order is gated by the ordering decisions in Section 4 below.

---

## 3. Cross-Section Coordination Points

### 3.1 visionclaw-contracts crate (Section 10 → Sections 7, 12)

The most important coordination surface in Phase 7. Three consumers depend on
the crate being published to the workspace before their implementation starts:

| Consumer section | What it imports |
|-----------------|-----------------|
| Section 07 (Bots) | `AgentActionEnvelope` type for click-through dispatch (B07-6) |
| Section 12 (XR) | `visionclaw-xr-presence` crate types; `AgentActionEnvelope` for XR click |
| Section 10 itself | `AgentTelemetryEnvelope`, `EnterpriseEventEnvelope` (self-contained) |

Coordination rule: `impl/phase-7-external` merges its X10-1 deliverable
(`crates/visionclaw-contracts` scaffolded, `cargo build` passing, `.d.ts`
generated and committed) to `impl/phase-7-parallel` BEFORE any of the
third-wave sub-worktrees start implementing their consumer tasks.

The merge need not include X10-2 through X10-10; just the crate skeleton plus
the TypeScript `.d.ts` output is sufficient to unblock downstream.

### 3.2 Auth D3-D11 → Route declarations in all other sections

ADR-06 D3's build-script lint (`compile_routes_have_auth!`) means any section
that adds a new route must declare its auth posture. Sections 7, 9, 10, and 12
all add or retain routes:

| Section | Routes needing posture |
|---------|----------------------|
| 07 Bots | Telemetry handler retained routes (`GET /api/agents/identity/{id}`, etc.) |
| 09 Ecosystem | Speech WebSocket, health endpoints |
| 10 External | `/ws/agent-telemetry`, `/ws/enterprise-events`, auth bridge endpoints |
| 12 XR | `/ws/xr-presence` |

Coordination rule: the auth D3-D11 sub-worktree must merge its A06-D3
(build-script lint in warn mode) and A06-D4 (endpoint audit table) to
`impl/phase-7-parallel` before third-wave sections add their new routes.
The lint begins as a warning so it does not break third-wave work in progress;
it is promoted to hard error only after all handlers are classified.

### 3.3 Docker socket decision (Sections 09 + 06)

ADR-06 D7 and ADR-09 E09-7 must agree on whether to add a socket proxy or
remove the mount. The two sub-worktrees (`impl/phase-7-auth-d3-d11` and
`impl/phase-7-ecosystem`) must coordinate this decision before either touches
`docker-compose.unified.yml`. Coordination mechanism: a shared ADR amendment
tracked as a discussion item in the phase-7-parallel branch.

### 3.4 Settings schema generator (Section 05) → Section 07 bots settings

B07-8 adds 6 new `bots.*` settings to `settings.ts`, which must be re-generated
by the S05-1 generator. The generator must be merged and working before B07-8
is merged. If the generator is not yet available when B07-8 is ready, the
settings addition is held in the bots sub-worktree until S05-1 merges.

### 3.5 Binary protocol decoder parity (Section 12 ↔ src/utils/binary_protocol.rs)

ADR-12 D2 requires `xr-client/rust/src/protocol.rs` to mirror
`src/utils/binary_protocol.rs` byte-for-byte. The TC-1 class-bit allocation
resolution (adopted in sprint docs) canonically defines the bit layout.
If any protocol change lands in Phase 6 (Section 2 sub-tasks), XR sub-worktree
must rebase before merging XR12-2.

---

## 4. Implementation Order within Phase 7

Even though the name says "parallel", strict ordering governs the first two waves.

### Wave 1 — Crate landing (unblocks consumer sections)

Duration: 2 days
Sub-worktree: `impl/phase-7-external`
Deliverable merged to `impl/phase-7-parallel`: `crates/visionclaw-contracts`
scaffold (X10-1 only).

Gate to Wave 2: `cargo build -p visionclaw-contracts` passes on
`impl/phase-7-parallel`; `agent-action.d.ts` committed.

### Wave 2 — Security gate (unblocks route additions in all sections)

Duration: starts immediately after Wave 1 crate merge; runs 3 days in parallel
with remaining Section 10 implementation.
Sub-worktrees: `impl/phase-7-auth-d3-d11` (primary), `impl/phase-7-external`
(continues X10-2 through X10-10).

Gate to Wave 3: Auth D3 build-script lint (in warn mode) and D4 endpoint audit
merged to `impl/phase-7-parallel`.

### Wave 3 — True parallel (all remaining sections)

Duration: starts after Wave 2 gate; runs up to 8 days.
Sub-worktrees: `impl/phase-7-settings`, `impl/phase-7-bots`,
`impl/phase-7-ecosystem`, `impl/phase-7-xr` — all four running in parallel.
`impl/phase-7-external` and `impl/phase-7-auth-d3-d11` continue their
remaining tasks in parallel with Wave 3.

Internal Wave 3 ordering constraints (within-branch only, not cross-worktree):

- `impl/phase-7-settings`: S05-1 (generator) must land before S05-3 or S05-5.
- `impl/phase-7-bots`: B07-1 (delete polling) before B07-3 (add consumer) to
  avoid two-source-of-truth period. B07-6 (click-through) requires crates/visionclaw-contracts.
- `impl/phase-7-ecosystem`: E09-1 (visionclaw_network grep) before E09-3 or E09-7.
- `impl/phase-7-xr`: XR12-1 (scaffold) before XR12-2 (gdext bridge).
  XR12-9 (perf benchmark) required before merge gate.

### Wave 4 — Merge gate

Each sub-worktree requires a passing merge gate before landing on
`impl/phase-7-parallel`:
1. All acceptance criteria above verified by test output.
2. CI lint passes (generator check, visionclaw_network grep, auth lint, enterprise guard).
3. Phase 7b deprecation dates set in ADR-07 D12 (30 days post Phase 7a merge date).

---

## 5. Estimated Overall Complexity

| Section | Wave | Estimated effort (days) | Risk |
|---------|------|------------------------|------|
| 10 External (contracts crate first) | 1+2+3 | 6 | HIGH |
| 06 Auth D3-D11 | 2+3 | 6 | HIGH |
| 12 XR Client | 3 | 8 | MEDIUM-HIGH |
| 05 Settings | 3 | 5 | MEDIUM |
| 07 Bots | 3 | 4 | MEDIUM |
| 09 Ecosystem | 3 | 3 | LOW-MEDIUM |

Total sequential lower bound (Wave 1 → 2 → 3): 2 + 3 + 8 = 13 days.
With true Wave 3 parallelism across 4 sub-worktrees: ~8-10 engineering days
elapsed (Wave 3 is bounded by the XR client, the largest Wave 3 item).

Total engineer-days across all sections: 32.
Expected wall-clock days with 6 parallel sub-worktrees: 8-12 days.

---

## 6. Risk Register

### R1. visionclaw-contracts is consumed before it compiles (HIGH)

Likelihood: Medium. The crate must be scaffolded and building before Sections
7 and 12 start their contract-dependent tasks.

Impact: Sections 7 and 12 sub-worktrees stall waiting for the crate; Wave 3
delay.

Mitigation:
- Wave 1 is strictly scoped to X10-1 (crate scaffold + `.d.ts` gen) only —
  no other Section 10 tasks. Two-day timebox.
- If the crate scaffold takes longer than 2 days, Sections 7 and 12 can start
  their non-contract tasks (B07-1 deletion, XR12-1 Godot scaffold) in parallel
  without waiting.
- The contract types are simple enough to stub with placeholder structs in
  consumer sub-worktrees if needed; a single rebase after X10-1 merges
  replaces stubs with real types.

### R2. Auth D3 build-script lint blocks Wave 3 route additions (HIGH)

Likelihood: Low-medium. If the lint is hard-error from day one and a
third-wave section tries to add a route before D4 audit completes,
CI fails.

Impact: Third-wave sections cannot merge until D4 is complete.

Mitigation:
- ADR-06 D3 explicitly stages the lint: warn first, hard error only after all
  existing handlers are classified. This is the planned posture.
- The wave gate for Wave 3 is D3 (warn mode) and D4 (audit complete) merged
  to `impl/phase-7-parallel`. Third-wave sections route under the audit-completed
  posture.
- New routes added by third-wave sections inherit the three-state discipline;
  they are classified in the same PR that adds the route.

### R3. XR Godot scaffold takes longer than estimated (MEDIUM)

Likelihood: Medium. Greenfield Godot + gdext bridge with platform-specific
secret stores is the most uncertain deliverable in the phase.

Impact: XR sub-worktree is the critical path for Wave 3 (8-day estimate).
If it slips, Phase 7 overall slips.

Mitigation:
- XR12-9 (perf benchmark) is a merge gate; performance regressions must be
  addressed before merge, not after. Implementing the benchmark early gives
  early signal.
- The Godot project scaffold (XR12-1) and the gdext bridge (XR12-2) are
  independent enough to be developed in parallel by two engineers if needed.
- If gdext platform secret stores prove complex, the Android Keystore impl
  can be stubbed with an in-memory impl for the initial merge; the stub is
  replaced in a follow-up PR before a production APK is built.
- The comfort policy (XR12-6) is non-negotiable per ADR-12 D6 but is
  mechanically simple; it does not gate the protocol or auth tasks.

### R4. Docker socket coordination between Sections 06 and 09 (MEDIUM)

Likelihood: Low. Both sections know about the dependency, and the decision
is clearly owned by ADR-06 D7 with Section 09 executing.

Impact: If both sub-worktrees independently modify `docker-compose.unified.yml`
for this, a three-way merge conflict arises on `impl/phase-7-parallel`.

Mitigation:
- Designate `impl/phase-7-ecosystem` as the sole writer of
  `docker-compose.unified.yml` for the socket scope/removal change.
- `impl/phase-7-auth-d3-d11` documents the constraint in ADR-06 D7 only;
  it does not edit docker-compose.unified.yml.
- A blocking comment in the auth sub-worktree PR description reminds the
  reviewer that docker-compose.unified.yml is owned by the ecosystem PR.

### R5. visionclaw-contracts versioning discipline erodes (MEDIUM)

Likelihood: Low initially but grows over time. The contract crate is the
integration surface between VisionClaw and external systems (agentbox, forum).
If schema_version bumping discipline is not enforced, version skew goes
undetected until a cross-system regression.

Impact: Silent misparse of envelopes on one or both sides; agent click
forwarding produces malformed state in agentbox.

Mitigation:
- ADR-10 D8 versioning rules (additive = no bump; incompatible = bump) are
  checked in the `ts-rs` generation test: the committed `.d.ts` must be
  byte-identical to a fresh generation. Any structural change to the Rust
  type forces a regeneration, which makes the diff visible in the PR.
- The contract test harness (X10-10) explicitly tests that receivers reject
  `schema_version !== 1`; new versions require test additions.
- `@visionclaw/contracts` npm package version is gated by the same semver
  rules as `schema_version`; a CI check fails if the npm version and the
  `schema_version` constant diverge.

---

## File Paths Touched (Summary)

### New files created in Phase 7

- `crates/visionclaw-contracts/` — entire new crate (X10-1)
- `crates/visionclaw-xr-presence/` — entire new crate (XR12-3)
- `xr-client/` — entire new Godot project directory (XR12-1 through XR12-10)
- `client/src/types/contracts/agent-action.d.ts` — ts-rs generated (X10-1)
- `client/src/features/settings/config/settingsUIDefinition/` — sharded tab files (S05-3)
- `client/src/features/visualisation/__tests__/controlPanel.shape.test.tsx` (S05-4)
- `src/handlers/telemetry_handler.rs` — re-homed read-only bots endpoints (B07-4)
- `scripts/ci/check-ws-route-enumeration.sh` (A06-D11)
- `tests/contracts/external-integrations/` — contract test harness (X10-10)
- `xr-client/perf/fixtures/perf_graph_1k.json` (XR12-9)
- `xr-client/perf/run_benchmark.gd` (XR12-9)
- `xr-client/perf/regression_check.py` (XR12-9)

### Files deleted in Phase 7

- `client/src/features/bots/services/AgentPollingService.ts` (B07-1)
- `client/src/features/bots/hooks/useAgentPolling.ts` (B07-1)
- `client/src/features/bots/config/pollingConfig.ts` (B07-1)
- `client/src/features/bots/utils/pollingPerformance.ts` (B07-1)
- `client/src/features/bots/docs/polling-system.md` (B07-1)
- `client/src/features/bots/BotsControlPanel.tsx` (B07-5)
- `src/handlers/solid_proxy_handler.rs` (A06-D4)
- `src/handlers/settings_handler.rs.temp` (A06-D4)
- `scripts/fix_kokoro_network.sh` (E09-1)

### Primary files modified in Phase 7

- `src/main.rs` — route auth declarations, startup refusal check (A06-D3, A06-D4)
- `src/settings/auth_extractor.rs` — already D1/D2 gated; D9 unwrap audit
- `src/handlers/api_handler/bots/mod.rs` — 410 Gone shims (B07-4)
- `src/handlers/bots_handler.rs` — delete static BOTS_GRAPH (B07-2)
- `client/index.html` — CSP tightening (A06-D5)
- `client/dist/index.html` — CSP meta-tag restore (A06-D5)
- `docker-compose.unified.yml` — visionclaw_network → visionclaw_network, socket scope (E09-1, E09-7)
- `scripts/launch.sh` — visionclaw_network literals, GPU device env vars (E09-1, E09-2)
- `scripts/rust-backend-wrapper.sh` — content-hash incremental rebuild (E09-6)
- `Dockerfile.unified` — wasm-builder stage addition (E09-5)
- `client/src/features/settings/config/settings.ts` — bots.* settings, Quality Gate removal (B07-8, S05-6)
- `client/src/features/bots/BotsDataContext.tsx` — narrow type, add telemetry consumer (B07-1, B07-3)
