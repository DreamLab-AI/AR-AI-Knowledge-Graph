# QE Anomaly Report: T2 (Doubled Write/Dispatch Paths) + T4 (Validation-Ceiling Mismatches)

> **✅ SUPERSEDED — FIXES LANDED 2026-06-03.** This is a frozen pre-fix reproduction
> report. Both anomalies are resolved:
> - **T2** (doubled write/dispatch): the duplicate `notifyPhysicsUpdate` DIRECT PUT
>   is now a no-op (`physicsSlice.ts:129-138`); slider commits take a single PUT path.
>   The legacy binary drag frame is removed (`useGraphEventHandlers.ts:61-64`) — one
>   JSON `nodeDragUpdate` per tick. Backend physics propagation routes solely via
>   `propagate_physics_to_gpu` (no direct-GPU fallback).
> - **T4** (validation-ceiling mismatch): resolved by the physics-bounds SSOT in
>   `src/actors/gpu/physics_bounds.rs`.
>
> Authoritative current state: `00-anomaly-register.md` (resolution table T2/T4).
> Reproduction evidence below is retained for historical context. Note line cites
> may be stale (e.g. `force_compute_actor.rs:2188` warmup assignment is now at L2203).

> Status: ~~REPRODUCTION EVIDENCE — do NOT fix here~~ → RESOLVED. Minimal fix specs at bottom.
> Investigator: QE static analysis pass, 2026-06-03

---

## T2(a): Doubled HTTP round-trips per physics slider commit

**Claim**: a single slider commit fires two independent PUTs to `/api/settings/physics`.

### Call chain — path 1 (immediate, synchronous in commit)

```
physicsSlice.ts:116        updatePhysics() → state.notifyPhysicsUpdate(graphName, validatedParams)
physicsSlice.ts:130-135    notifyPhysicsUpdate() → settingsApi.updatePhysics(params)
endpoints.ts:85            updatePhysics() → GET /api/settings/physics        (read-merge)
endpoints.ts:90            updatePhysics() → PUT /api/settings/physics        (write)
```

### Call chain — path 2 (debounced, 500 ms later)

```
UnifiedSettingsTabContent.tsx:104-105  updateSettingByPath() → autoSaveManager.queueChange(path, value)
autoSaveManager.ts:97-99              scheduleFlush() sets 500 ms setTimeout
autoSaveManager.ts:124                flushPendingChanges() → settingsApi.updateSettingsByPaths([{path, value}])
endpoints.ts:395-398                  updateSettingsByPaths() routes path 'visualisation.graphs.*.physics.*' → physicsUpdates
endpoints.ts:427                      → updatePhysics(physicsUpdates)   ← calls the SAME updatePhysics function
endpoints.ts:85,90                    → GET /api/settings/physics + PUT /api/settings/physics (second round-trip)
```

**Confirmed duplication**: `notifyPhysicsUpdate` fires path 1 immediately; `autoSaveManager.queueChange` fires path 2 500 ms later. Both reach `updatePhysics()` in `endpoints.ts:79` which performs a GET+PUT pair against `/api/settings/physics`. The backend handler propagates both to `EnhancedSettingsHandler::propagate_physics_updates` which calls `UpdateSimulationParams` each time — see T2(b).

Note: `UnifiedSettingsTabContent` is used for generic field edits via `updateSettingByPath` (line 78, calls `autoSaveManager.queueChange` at line 105). Physics sliders that go through `updatePhysics` in the store trigger `notifyPhysicsUpdate` independently at `physicsSlice.ts:116`. Both code paths are live simultaneously on every slider change.

---

## T2(b): Doubled server-side dispatch of UpdateSimulationParams

**Claim**: a single PUT to `/api/settings/physics` causes `UpdateSimulationParams` to be delivered to `ForceComputeActor` twice.

### Dispatch 1 — settings route → ForceComputeActor directly

```
settings_handler/enhanced.rs:520-562   EnhancedSettingsHandler::propagate_physics_updates()
settings_handler/enhanced.rs:544       SimulationParams::from(physics)
settings_handler/enhanced.rs:546-549   state.get_gpu_compute_addr() → gpu_addr.send(UpdateSimulationParams{..})
```

`state.get_gpu_compute_addr()` returns the `ForceComputeActor` address stored in `AppState` (initialised at `app_state.rs:854`).

### Dispatch 2 — PhysicsOrchestratorActor forwarding

The settings route also notifies `PhysicsOrchestratorActor` via the `GraphServiceSupervisor` chain. `PhysicsOrchestratorActor` explicitly forwards the message onward:

```
physics_orchestrator_actor.rs:1474-1479  Handler<UpdateSimulationParams>::handle()
                                         "Forward UpdateSimulationParams to ForceComputeActor
                                          as guaranteed fallback."
                                         gpu_addr.do_send(msg)   ← second delivery to ForceComputeActor
```

### Effect at ForceComputeActor

Both deliveries pass the idempotency check at `force_compute_actor.rs:2086-2118`. If the params are identical (same slider value, no intervening change) the second dispatch is swallowed by the `physics_unchanged` guard at line 2114. However if any field drifts (e.g., floating-point merge-GET round-trip) the check does NOT absorb it and both deliveries independently execute:

```
force_compute_actor.rs:2188   self.stability_warmup_remaining = 1800;
force_compute_actor.rs:2195   self.reheat_factor = reheat;
```

Each of these resets races with the physics step loop. The second reset re-arms warmup and reheat while the first may already have started the settle cycle, causing a redundant 30-second warmup window and spurious velocity reheat.

---

## T2(c): Two WebSocket frames per drag tick

**Claim**: each throttled drag tick sends both a JSON and a binary frame.

```
useGraphEventHandlers.ts:60-65   sendMessage('nodeDragUpdate', {nodeId, position, timestamp})   ← JSON frame
useGraphEventHandlers.ts:70-75   sendNodePositionUpdates([{nodeId, position, velocity}])         ← binary frame
```

Both calls are inside the single `throttledWebSocketUpdate` callback (lines 54-83), gated by the same `shouldSendPositionUpdates()` check at line 57. The comment at line 67 explicitly labels the binary send "legacy binary position update for backwards compatibility" — confirming deliberate doubling. Every throttle tick (100 ms interval, `POSITION_UPDATE_THROTTLE_MS`) fires both.

---

## T4(a,b,c): Validation-ceiling inconsistency table

| param | Rust default (`physics_config.rs:351`) | TS default (`defaults.ts:23`) | actor path cap (`optimized_settings_actor.rs`) | route validator cap (`settings_routes.rs:132`) | GPU clamp (`visionclaw_unified.cu:745,757,779`) | Rust backstop (`force_compute_actor.rs:133,1788`) | Inconsistent? |
|---|---|---|---|---|---|---|---|
| `repel_k` | **120.0** | **120.0** | **100.0** (line 230) | no range check (check_finite only, line 145) | n/a | n/a | YES — default 120 > actor path cap 100 |
| `max_velocity` | **100.0** | **100.0** | **50.0** (line 236) | **1000.0** (line 132) | `c_params.max_velocity` (parameterised) | **1000.0** (line 133) | YES — actor path cap 50 < default 100 < route cap 1000 = Rust backstop 1000; GPU clamp follows `max_velocity` param so GPU is tighter than backstop only when param < 1000 |
| `spring_k` | 12.0 | 12.0 | **10.0** (line 224) | 500.0 (line 131) | n/a | n/a | YES — default 12 > actor path cap 10 |

### T4(a) — repel_k ceiling conflict

- Boot default: `physics_config.rs:351` `repel_k: 120.0`; `defaults.ts:24` `repelK: 120.0`
- Actor path-pattern ceiling: `optimized_settings_actor.rs:230` `max: 100.0`
- Route validator: `settings_routes.rs:145` only calls `check_finite` (no upper bound on `repel_k`)

Any PUT through the `/api/settings/physics` route accepts `repel_k = 120` (passes `check_finite`). The `OptimizedSettingsActor` path-pattern validator at line 228-231 would reject it with its 100.0 ceiling — but those two validation paths are separate; there is no single gating point. The system boots with a value that would fail actor-path validation.

### T4(b) — max_velocity ceiling conflict

- Boot default: `physics_config.rs:349` `max_velocity: 100.0`
- Actor path cap: `optimized_settings_actor.rs:236` `max: 50.0` (i.e. default exceeds actor-path ceiling by 2x)
- Route validator: `settings_routes.rs:132` `check_range(max_velocity, 0.1, 1000.0)`
- Rust backstop: `force_compute_actor.rs:133` `MAX_VELOCITY_MAGNITUDE = 1_000.0`; applied at line 1788
- GPU clamp: `visionclaw_unified.cu:745,757,779` clamps per-tick velocity to `c_params.max_velocity` (the uploaded param)

When `max_velocity` is at its default (100), the GPU clamps to 100 and the Rust backstop at 1000 is never triggered. When `max_velocity > 1000` (allowed by the route validator up to 1000.0, so this only matters if the ceiling is raised), the GPU clamp is _at_ `max_velocity` and the Rust backstop is at 1000 — meaning the GPU would allow up to `max_velocity` but the Rust backstop would clamp back to 1000, creating a frame where some velocities appear valid post-GPU but are then re-clamped by Rust, triggering the divergence guard at `force_compute_actor.rs:1694` (`v.length() > MAX_VELOCITY_MAGNITUDE`) even on otherwise-healthy frames. The claimed scenario (max_velocity > 1000) cannot be reached via the current route validator (ceiling 1000), but the validator ceiling and the constant are identical with no margin, making the backstop redundant rather than a safety net.

### T4(c) — divergence guard false-fire condition

The divergence guard fires when `v.length() > MAX_VELOCITY_MAGNITUDE` (line 1694). If `max_velocity` were set equal to `MAX_VELOCITY_MAGNITUDE` (1000) and the GPU jitter path (`cu:776-778`) pushes a velocity just over 1000 before the third `vec3_clamp` at line 779, the Rust backstop at 1788 would clamp it again — but `1694` checks the _raw readback_ before the backstop clamp at 1788 is applied. The check at 1694 runs before the clamp at 1783-1790, so a velocity of 1001 (from GPU jitter) would incorrectly count as a bad frame.

---

## Regression Test Files

### 1. Rust unit tests — `/home/devuser/workspace/project/tests/repro_t4_ceiling_consistency.rs`

See that file. Tests assert:
- `PhysicsSettings::default().repel_k <= 100.0` (actor path cap) — **FAILS**: 120.0 > 100.0
- `PhysicsSettings::default().max_velocity <= 50.0` (actor path cap) — **FAILS**: 100.0 > 50.0
- `PhysicsSettings::default().spring_k <= 10.0` (actor path cap) — **FAILS**: 12.0 > 10.0
- `PhysicsSettings::default().max_velocity <= 1000.0` (route cap) — passes
- `MAX_VELOCITY_MAGNITUDE == 1000.0 as route cap` (consistency sentinel) — passes (informational)

### 2. TypeScript unit tests — `/home/devuser/workspace/project/client/src/api/settings/__tests__/repro_t4_ceiling_consistency.test.ts`

See that file. Tests assert:
- `DEFAULT_PHYSICS_SETTINGS.repelK <= ACTOR_PATH_REPEL_K_MAX` (100) — **FAILS**: 120 > 100
- `DEFAULT_PHYSICS_SETTINGS.maxVelocity <= ACTOR_PATH_MAX_VELOCITY_MAX` (50) — **FAILS**: 100 > 50
- `DEFAULT_PHYSICS_SETTINGS.springK <= ACTOR_PATH_SPRING_K_MAX` (10) — **FAILS**: 12 > 10

### 3. TypeScript unit test — doubled PUT per slider commit — `/home/devuser/workspace/project/client/src/store/__tests__/repro_t2_double_put.test.ts`

See that file. Test mocks `axios` and `autoSaveManager`, calls `updatePhysics` in the store, and asserts that exactly **one** PUT to `/api/settings/physics` was made per slider commit. **FAILS** because `notifyPhysicsUpdate` issues one PUT immediately and `autoSaveManager.queueChange` issues a second PUT after debounce.

---

## Minimal Fix Specifications

### T2(a) — single write path per slider commit

**Problem**: `notifyPhysicsUpdate` (immediate) and `autoSaveManager.queueChange` (debounced) both reach `/api/settings/physics`.

**Fix**: Remove `notifyPhysicsUpdate` from `physicsSlice.ts` (or make it a no-op for HTTP persistence). Route ALL server persistence through `autoSaveManager` exclusively. `notifyPhysicsUpdate` can remain for in-process event dispatch (e.g. dispatching a CustomEvent to the graph worker) but must not call `settingsApi.updatePhysics` directly.

### T2(b) — single dispatch of UpdateSimulationParams

**Problem**: `EnhancedSettingsHandler::propagate_physics_updates` (enhanced.rs:546) sends to `ForceComputeActor` directly, AND `PhysicsOrchestratorActor` (physics_orchestrator_actor.rs:1478-1479) forwards the same message.

**Fix**: Remove the direct `state.get_gpu_compute_addr()` send in `enhanced.rs:546-549`. The orchestrator already owns the forwarding responsibility (its comment at line 1474 explicitly states this). The settings handler should send only to the orchestrator (via GraphServiceSupervisor), and the orchestrator delivers to ForceComputeActor. One path, one delivery.

### T2(c) — single WS frame per drag tick

**Problem**: `useGraphEventHandlers.ts:60-74` sends both `sendMessage('nodeDragUpdate', ...)` (JSON) and `sendNodePositionUpdates([...])` (binary) on every throttle tick.

**Fix**: Pick one wire format. The JSON `nodeDragUpdate` is the server-authoritative path (it carries pin-at-position semantics). Remove the binary `sendNodePositionUpdates` call or gate it behind a feature flag that is off by default. Do not remove the binary path until all server consumers of the binary protocol are confirmed to read the JSON path.

### T4 — single validation source of truth for ceilings

**Problem**: three independent ceiling tables exist: (1) `optimized_settings_actor.rs:initialize_path_patterns`, (2) `settings_routes.rs:validate_physics_settings`, (3) `physics_config.rs::default()`. They are inconsistent.

**Fix**: Define one authoritative constants struct/module (e.g. `physics_bounds.rs`) with named constants for each parameter's `(min, max)`. Both validators and the `Default` impl derive from that single source. CI enforces: `default() >= min && default() <= max` for every field via a `#[test]` that imports both. The `optimized_settings_actor` path-pattern table and `validate_physics_settings` both import the same constants.
