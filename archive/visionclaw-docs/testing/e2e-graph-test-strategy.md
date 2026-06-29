# End-to-End Graph UX/UI Test Strategy

**Version**: 1.0
**Date**: 2026-05-27
**Scope**: VisionClaw 3D Knowledge Graph -- all graph features, interactions, and data pipeline verification
**Trigger**: R3F/Zustand reconciler bug where node type visibility toggles update the store but the Canvas scene does not re-render

---

## 1. Data Pipeline Verification (REST + WebSocket)

### 1.1 REST API Endpoint Sampling

| Test ID | Endpoint | Method | Verification |
|---------|----------|--------|-------------|
| DP-001 | `/api/graph/data` | GET | Returns `{ nodes: [...], edges: [...] }`. Validate node count > 0, each node has `id`, `metadata`. |
| DP-002 | `/api/graph/data?graph_type=knowledge` | GET | Returns subset; all nodes classify as knowledge_graph via binary type flags. |
| DP-003 | `/api/graph/data?graph_type=ontology` | GET | Ontology subset; nodes carry `owlClassIri` or `hierarchyDepth` metadata. |
| DP-004 | `/api/graph/data?graph_type=agent` | GET | Agent subset; nodes carry `agentType` metadata. |
| DP-005 | `/api/settings` | GET | Returns full settings object. Schema matches `Settings` type definition. |
| DP-006 | `/api/settings` | PUT | Round-trip: write `nodeSize: 1.5`, read back, confirm value persisted. |
| DP-007 | `/api/bots/agents` | GET | Returns agent telemetry array. Validate each entry has `id`, `status`. |
| DP-008 | `/api/physics/parameters` | GET | Returns `PhysicsSettings`-compatible object with `springK`, `repelK`, `gravity`, etc. |
| DP-009 | `/api/physics/parameters` | PUT | Write `springK: 25.0`, read back, confirm GPU picks up new value. |
| DP-010 | `/api/layout/positions` | GET | Returns position array. Each entry has numeric `id`, `x`, `y`, `z`. |

**Execution**: `sudo docker exec visionclaw_container curl -s localhost:4000/api/graph/data | jq '.nodes | length'`

### 1.2 WebSocket Binary Position Protocol

| Test ID | Phase | Verification |
|---------|-------|-------------|
| WS-001 | Connect | Open `ws://192.168.2.132:4000/ws`. Verify handshake completes within 5s. |
| WS-002 | First frame | Receive binary message within 10s of connection. Validate frame length is a multiple of the per-node stride (node_id u32 + xyz f32x3 = 16 bytes minimum). |
| WS-003 | Position decode | Decode first 10 nodes. Verify x/y/z values are finite (no NaN/Infinity). Verify positions are not all (0,0,0). |
| WS-004 | Node count consistency | Compare: REST `/api/graph/data` node count vs. WS frame node count vs. `graphDataManager` internal count (via `evaluate_script`). All three must match within +/- 5 (agent overlay nodes may add a few). |
| WS-005 | Continuous frames | Receive 60 frames. Verify at least some position deltas are non-zero (physics is running). |
| WS-006 | Reconnection | Close WS, wait 3s, reconnect. Verify positions resume within 5s. Verify no NaN positions after reconnect. |

### 1.3 Node Count Consistency Cross-Check

This is a critical invariant. Three independent sources must agree:

```
REST node count  ===  WS position count  ===  InstancedMesh.count (rendered)
```

**Test procedure** (evaluate_script in browser):
```javascript
// 1. REST count
const restResp = await fetch('/api/graph/data');
const restData = await restResp.json();
const restCount = restData.nodes.length;

// 2. Rendered instance count (read from R3F scene)
const meshes = document.querySelector('canvas').__r$.__instances;
// Alternative: use window.__GRAPH_DEBUG__ if instrumented

// 3. Zustand store count
const storeNodes = window.__ZUSTAND_STORE__?.getState()?.graphData?.nodes?.length;
```

---

## 2. Zustand <-> R3F Reconciler Testing

This section targets the exact class of bug we discovered: Zustand store updates that do NOT propagate into the R3F Canvas reconciler tree.

### 2.1 Architecture of the Problem

R3F mounts a **separate React reconciler** inside `<Canvas>`. Components inside Canvas (GraphManager, GemNodes, GlassEdges, etc.) share the same Zustand store instance via module-level singleton, but React's reconciliation is independent. If a Zustand selector returns an object reference that is referentially equal (===) between renders, the R3F component will **not re-render**, even if nested properties changed.

### 2.2 Critical Zustand Selectors Inside Canvas

Every `useSettingsStore()` call inside the Canvas tree is a potential reconciler boundary bug. The complete inventory:

#### GraphManager.tsx (main orchestrator, inside Canvas)

| Selector | Path | Return Type | Risk |
|----------|------|-------------|------|
| `logseqSettings` | `s.settings?.visualisation?.graphs?.logseq` | object | **HIGH** -- returns nested object; Zustand uses `Object.is` which compares by reference. If parent object is not replaced (Immer draft reuse), child changes are invisible. |
| `graphTypeVisuals` | `s.settings?.visualisation?.graphTypeVisuals` | object | **HIGH** -- same nested-object risk. |
| `glowIntensity` | `s.settings?.visualisation?.glow?.intensity ?? 0.3` | number | LOW -- primitive, `Object.is` works correctly. |
| `debugSettings` | `s.settings?.system?.debug` | object | MEDIUM -- object ref risk, but rarely toggled. |
| `nodeFilterSettings` | `s.settings?.nodeFilter` | object | MEDIUM -- object ref risk. |
| `nodeTypeVisibility` | `s.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility` | object | **CRITICAL** -- this is the exact selector involved in the reported bug. Returns `{ knowledge: bool, ontology: bool, agent: bool }`. If Immer produces a structurally-identical-but-referentially-new object, it works. If it reuses the draft, it fails silently. |
| `visionclawPhysics` | `s.settings?.visualisation?.graphs?.visionclaw?.physics` | object | MEDIUM |
| `layoutMode` | `s.settings?.qualityGates?.layoutMode` | string | LOW -- primitive. |
| `settingsRef` (via `.subscribe()`) | full settings | ref (manual) | **LOW** -- uses manual subscription, bypasses reconciler. Always current. |

#### GemNodes.tsx (instanced node rendering, inside Canvas)

| Selector | Path | Return Type | Risk |
|----------|------|-------------|------|
| `gemSettings` | `s.get<GemMaterialSettings>('visualisation.gemMaterial')` | object | **HIGH** -- `.get()` accessor may return same ref if Immer reuses the sub-tree. |
| `qualityGates` | `s.get<QualityGatesSettings>('qualityGates')` | object | **HIGH** -- same risk. |

#### GlassEdges.tsx (edge rendering, inside Canvas)

| Selector | Path | Return Type | Risk |
|----------|------|-------------|------|
| `renderingCeiling` | `s.settings?.visualisation?.rendering?.maxEdgesCeiling` | number | LOW -- primitive. |
| `glowSettings` | `s.get<GlowSettings>('visualisation.glow')` | object | **HIGH** |
| `gemSettings` | `s.get<GemMaterialSettings>('visualisation.gemMaterial')` | object | **HIGH** |

#### GraphCanvas.tsx (Canvas wrapper, partially outside Canvas)

| Selector | Path | Return Type | Risk |
|----------|------|-------------|------|
| `softwareFallbackPolicy` | rendering.softwareFallback | string | LOW |
| `showStats` | system.debug.enablePerformanceDebug | boolean | LOW |
| `enableGlow` | visualisation.glow.enabled | boolean | LOW |
| `ambientLightIntensity` | rendering.ambientLightIntensity | number | LOW |
| `directionalLightIntensity` | rendering.directionalLightIntensity | number | LOW |
| `sceneEffects` | visualisation.sceneEffects | object | **HIGH** |
| `embeddingCloudEnabled` | embeddingCloud.enabled | boolean | LOW |

#### Hooks used inside Canvas

| Hook | File | Selector | Risk |
|------|------|----------|------|
| `useGraphFiltering` | useGraphFiltering.ts | `s.settings?.nodeFilter` | MEDIUM -- object |
| `useGraphVisualState` | useGraphVisualState.ts | settings graph mode | MEDIUM |
| `useFpsMonitor` | useFpsMonitor.ts | `.getState()` (imperative) | LOW |

### 2.3 Test Protocol: Reconciler Boundary Verification

For each HIGH-risk selector, execute this protocol:

**Step 1: Instrument the component** (via `evaluate_script`)
```javascript
// Inject render counter into GraphManager
const origRender = window.__GM_RENDER_COUNT || 0;
window.__GM_RENDER_COUNT = origRender;
// Hook into React DevTools fiber or use console.count
```

**Step 2: Read baseline state**
```javascript
const before = JSON.parse(JSON.stringify(
  window.__ZUSTAND_STORE__?.getState()?.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility
));
```

**Step 3: Mutate via store**
```javascript
window.__ZUSTAND_STORE__?.getState()?.set?.(
  'visualisation.graphs.logseq.nodes.nodeTypeVisibility.knowledge', false
);
```

**Step 4: Verify propagation** (wait 500ms for R3F frame)
```javascript
// Check store updated
const after = window.__ZUSTAND_STORE__?.getState()?.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility;
console.assert(after.knowledge === false, 'Store did not update');

// Check R3F scene updated -- count visible instance meshes
// GemNodes sets mesh.count = number of visible nodes
const canvas = document.querySelector('canvas');
// Use __r$ internals or performance.getEntriesByType to detect re-render
```

**Step 5: Visual confirmation** -- take screenshot, compare node count.

### 2.4 Test Matrix: Reconciler Propagation

| Test ID | Store Path | Component | Expected Effect | Verify Method |
|---------|-----------|-----------|----------------|---------------|
| RC-001 | nodeTypeVisibility.knowledge = false | GraphManager -> typeFilteredNodes | Knowledge nodes disappear | Screenshot: count visible gem-shaped nodes |
| RC-002 | nodeTypeVisibility.ontology = false | GraphManager -> typeFilteredNodes | Crystal orb nodes disappear | Screenshot: count visible orb nodes |
| RC-003 | nodeTypeVisibility.agent = false | GraphManager -> typeFilteredNodes | Capsule nodes disappear | Screenshot: count visible capsule nodes |
| RC-004 | visualisation.glow.intensity = 0 | GraphManager -> glowIntensity | Bloom post-processing fades | Screenshot: compare glow halos |
| RC-005 | visualisation.gemMaterial.roughness = 1.0 | GemNodes -> gemSettings | Nodes become matte | Screenshot: compare specularity |
| RC-006 | visualisation.glow.enabled = false | GraphCanvas -> enableGlow | Post-processing bloom disabled | Screenshot: no bloom |
| RC-007 | rendering.ambientLightIntensity = 0 | GraphCanvas -> ambientLight | Scene goes dark except directional | Screenshot: brightness comparison |
| RC-008 | rendering.directionalLightIntensity = 0 | GraphCanvas -> directionalLight | No directional shadows | Screenshot: shadow comparison |
| RC-009 | visualisation.sceneEffects.enabled = false | GraphCanvas -> WasmSceneEffects | Particles/wisps disappear | Screenshot: particle count = 0 |
| RC-010 | nodeFilter.enabled = true | useGraphFiltering -> visibleNodes | Low-quality nodes hidden | Instance count decreases |

---

## 3. Visual Feature Matrix

### 3.1 Node Visibility Toggles

| Test ID | Feature | Control Path | ON State | OFF State | Verification |
|---------|---------|-------------|----------|-----------|-------------|
| VF-001 | Knowledge Nodes | `visualisation.graphs.logseq.nodes.nodeTypeVisibility.knowledge` | Icosahedron (gem) nodes visible | Gems disappear, orbs/capsules remain | Screenshot diff + instance count |
| VF-002 | Ontology Nodes | `...nodeTypeVisibility.ontology` | Crystal orb (sphere) nodes visible | Orbs disappear | Screenshot diff + instance count |
| VF-003 | Agent Nodes | `...nodeTypeVisibility.agent` | Capsule nodes visible | Capsules disappear | Screenshot diff + instance count |
| VF-004 | All OFF -> All ON | Toggle all three off then on | All nodes reappear | No stale instances, correct colors | Rapid toggle stress: 10 cycles in 5s |

### 3.2 Node Appearance

| Test ID | Feature | Control Path | Range | Verification |
|---------|---------|-------------|-------|-------------|
| VF-010 | Node Size | `...nodes.nodeSize` | 0.2 - 2.0 | Screenshot at min/mid/max. Verify scaling is proportional. |
| VF-011 | Node Color | `...nodes.baseColor` | hex color | Set to `#FF0000`. Verify nodes tint red (base color applied). |
| VF-012 | Metadata Shape | `...nodes.enableMetadataShape` | toggle | ON: MetadataShapes component renders. OFF: hidden. |

### 3.3 Edge Appearance

| Test ID | Feature | Control Path | Range | Verification |
|---------|---------|-------------|-------|-------------|
| VF-020 | Edge Thickness | `...edges.edgeRadius` | 0.005 - 0.1 | Screenshot at min/max. Cylinder radius visually changes. |
| VF-021 | Edge Opacity | `...edges.opacity` | 0 - 1 | At 0: edges invisible. At 1: fully opaque. At 0.5: semi-transparent. |
| VF-022 | Edge Color | `...edges.color` | hex | Set `#FF0000`. Edges turn red (unless edge-type coloring overrides). |
| VF-023 | KG Edge Color | `graphTypeVisuals.knowledgeGraph.edgeColor` | hex | Knowledge-mode edges change color. |

### 3.4 Label Appearance

| Test ID | Feature | Control Path | Verification |
|---------|---------|-------------|-------------|
| VF-030 | Show Labels | `...labels.enableLabels` | ON: text labels visible near nodes. OFF: no labels rendered (InstancedLabels mesh count = 0). |
| VF-031 | Label Size | `...labels.desktopFontSize` | Adjust 0.05 -> 3.0. Labels grow/shrink. |
| VF-032 | Label Color | `...labels.textColor` | Set to `#FF0000`. Label text turns red. |
| VF-033 | Show Metadata | `...labels.showMetadata` | ON: domain badge, quality stars, recency text below label. OFF: only node name. |
| VF-034 | Label Standoff | `...labels.textPadding` | -1.0 to 3.0. Labels move toward/away from node center. |

### 3.5 Lighting and Rendering

| Test ID | Feature | Control Path | Verification |
|---------|---------|-------------|-------------|
| VF-040 | Ambient Light | `rendering.ambientLightIntensity` | 0: scene very dark. 2: scene bright. |
| VF-041 | Direct Light | `rendering.directionalLightIntensity` | 0: no specular highlights. 2: strong highlights/shadows. |

### 3.6 Effects

| Test ID | Feature | Control Path | Verification |
|---------|---------|-------------|-------------|
| VF-050 | Glow/Bloom enable | `visualisation.glow.enabled` | ON: bloom halo around bright nodes. OFF: no post-processing bloom. |
| VF-051 | Glow intensity | `visualisation.glow.intensity` | 0: no visible bloom. 1: strong bloom aura. |
| VF-052 | Scene particles | `visualisation.sceneEffects.enabled` | ON: floating particles in scene. OFF: clean background. |
| VF-053 | Wisp count | `sceneEffects.wispCount` | 0: no wisps. 48: default wisp population. |
| VF-054 | Particle color | `sceneEffects.particleColor` | Set to `#FF0000`. Particles turn red. |

### 3.7 Physics Parameters

| Test ID | Feature | Control Path | Verification |
|---------|---------|-------------|-------------|
| VF-060 | Physics enabled | `...physics.enabled` | ON: nodes continue moving toward equilibrium. OFF: positions freeze. |
| VF-061 | Spring strength | `...physics.springK` | Low (0.01): nodes drift apart. High (100): edges compress tightly. |
| VF-062 | Repulsion | `...physics.repelK` | Low (0): nodes overlap. High (3000): nodes spread far. |
| VF-063 | Gravity | `...physics.gravity` | 0: nodes drift freely. 0.01: strong pull toward center. |
| VF-064 | Damping | `...physics.damping` | 0: high energy, nodes oscillate. 1: instant settle. |
| VF-065 | Layout mode | `qualityGates.layoutMode` | Switch `force-directed` -> `dag-topdown`. Nodes rearrange into tree layout within 5s. |

### 3.8 Quality Gates

| Test ID | Feature | Control Path | Verification |
|---------|---------|-------------|-------------|
| VF-070 | Cluster overlay | `qualityGates.showClusters` | ON: cluster hulls (ClusterHulls component) visible. OFF: no hulls. |
| VF-071 | Ontology forces | `qualityGates.ontologyPhysics` | ON: ontology constraint forces active (visible tighter grouping). |

---

## 4. Interaction Testing

### 4.1 Node Selection

| Test ID | Action | Expected Result | Verification |
|---------|--------|----------------|-------------|
| IX-001 | Click node | Node highlights (selected state). Detail panel shows node metadata. | Screenshot: highlight color matches `selectionHighlightColor`. Evaluate: `selectedNodeId` in store matches clicked node. |
| IX-002 | Click empty space | Selection clears. Detail panel dismisses. | `selectedNodeId === null` |
| IX-003 | Click different node | Previous selection clears, new node selected. | Only one highlight active. |

### 4.2 Node Hover

| Test ID | Action | Expected Result |
|---------|--------|----------------|
| IX-010 | Hover over node | Cursor changes. Tooltip or highlight appears. |
| IX-011 | Hover away | Tooltip/highlight disappears. |

### 4.3 Camera Controls (OrbitControls)

| Test ID | Action | Expected Result |
|---------|--------|----------------|
| IX-020 | Left-click drag on empty space | Camera orbits around center. |
| IX-021 | Scroll wheel | Camera zooms in/out. |
| IX-022 | Right-click drag | Camera pans laterally. |
| IX-023 | Middle-click drag | Camera pans (alternative). |

### 4.4 Drag Interaction

| Test ID | Action | Expected Result |
|---------|--------|----------------|
| IX-030 | Left-click + drag on node | Node follows pointer in 3D (dragDataRef updates). |
| IX-031 | Release drag | Node stays at new position. Physics may pull it back if enabled. |
| IX-032 | Drag state callback | `onDragStateChange(true)` fires on drag start, `(false)` on release. |

### 4.5 Rapid Toggle Stress Test

| Test ID | Scenario | Expected Result |
|---------|----------|----------------|
| IX-040 | Toggle Knowledge Nodes ON/OFF 20 times in 3 seconds | No flicker, no ghost nodes, no stale instances. Final state matches last toggle value. |
| IX-041 | Toggle all three type toggles simultaneously | No rendering artifacts. Instance counts converge within 2 frames. |
| IX-042 | Slide nodeSize from 0.2 to 2.0 in 1 second (continuous) | Smooth scaling, no frame drops below 30fps, no NaN in matrices. |

---

## 5. Cross-Cutting Concerns

### 5.1 WebSocket Reconnection

| Test ID | Scenario | Verification |
|---------|----------|-------------|
| CC-001 | Kill WS connection (browser devtools), wait 5s | WS auto-reconnects. Nodes reappear with non-zero positions within 10s. |
| CC-002 | Backend restart (supervisor restart webxr) | Client reconnects. No permanent stall. Positions resume. |

### 5.2 Settings Persistence

| Test ID | Scenario | Verification |
|---------|----------|-------------|
| CC-010 | Change nodeSize to 1.8, reload page | After reload, nodeSize reads 1.8 from persisted settings. |
| CC-011 | Toggle Knowledge Nodes OFF, reload page | After reload, Knowledge Nodes remain OFF. Zustand persist middleware restores state. |
| CC-012 | Change physics.springK to 50, reload | Physics uses springK=50 after reload. |

### 5.3 SharedArrayBuffer Position Consumers

The SAB (`nodePositionsRef`) is read by multiple R3F components. All must read consistently from the same buffer at the same frame:

| Consumer | File | Read Pattern | Risk |
|----------|------|-------------|------|
| GemNodes | GemNodes.tsx | `useFrame` reads `nodePositionsRef.current` for instance matrix updates | LOW -- ref is stable |
| InstancedLabels | InstancedLabels.tsx | `useFrame` reads `nodePositionsRef.current` for label placement | LOW |
| GlassEdges | GraphManager.tsx (useFrame) | GraphManager reads positions, computes edge endpoints, pushes to GlassEdges | MEDIUM -- edge computation depends on position freshness |
| KnowledgeRings | KnowledgeRings.tsx | `useFrame` reads `nodePositionsRef.current` for ring placement | LOW |

**Test**: After a layout mode transition (force-directed -> dag-topdown), verify all four consumers show consistent positions -- no component lagging by one frame or reading stale data.

### 5.4 Performance Baseline

| Test ID | Metric | Target | Measurement |
|---------|--------|--------|-------------|
| CC-020 | Frame rate (840 nodes, all effects) | >= 30 fps | `performance_start_trace` / `performance_stop_trace` via browser MCP |
| CC-021 | Frame rate (840 nodes, effects OFF) | >= 50 fps | Same |
| CC-022 | Memory (30 min session) | No monotonic growth | `take_memory_snapshot` at t=0, t=15m, t=30m. Compare retained size. |
| CC-023 | Toggle response latency | < 200ms from click to visual change | Performance trace around toggle event. |

---

## 6. Agent Mesh Architecture

### 6.1 Agent Topology

```
                    +------------------------+
                    |   Fleet Commander      |
                    |   (orchestrator)       |
                    +----------+-------------+
                               |
          +--------------------+--------------------+
          |                    |                     |
   +------+------+    +-------+-------+    +--------+-------+
   | Data Layer  |    | Scene Layer   |    | UX Layer       |
   | Team Lead   |    | Team Lead     |    | Team Lead      |
   +------+------+    +-------+-------+    +--------+-------+
          |                    |                     |
   +------+------+    +-------+-------+    +--------+-------+
   | REST Probe  |    | R3F Reconciler|    | Visual         |
   | WS Monitor  |    | Store Integ.  |    | Regression     |
   | Settings    |    | Performance   |    | Interaction    |
   | Pipeline    |    |               |    |                |
   +-------------+    +---------------+    +----------------+
```

### 6.2 Agent Definitions

#### Data Layer Agents

**REST Probe Agent**
- **Tool**: `sudo docker exec visionclaw_container curl ...` (Bash)
- **Scope**: Tests DP-001 through DP-010
- **Cadence**: Run once at session start, then after each settings mutation
- **Output**: JSON validation results stored to memory key `aqe/fleet/data/rest-results`

**WebSocket Monitor Agent**
- **Tool**: `evaluate_script` (inject WS test client into browser page)
- **Scope**: Tests WS-001 through WS-006
- **Cadence**: Continuous monitoring during test session
- **Output**: Frame statistics, position validity, reconnect timing

**Settings Pipeline Agent**
- **Tool**: `evaluate_script` + `sudo docker exec ... curl`
- **Scope**: Full round-trip: UI toggle -> Zustand store -> REST PUT -> REST GET -> Zustand store -> R3F scene
- **Sequence**:
  1. Read current value via `evaluate_script` (Zustand `.getState()`)
  2. Mutate via `evaluate_script` (Zustand `.set()` or `.updateSettings()`)
  3. Wait 500ms
  4. Verify store updated via `evaluate_script`
  5. Verify REST reflects change via `curl`
  6. Take screenshot to verify visual change

#### Scene Layer Agents

**R3F Reconciler Agent** (highest priority)
- **Tool**: `evaluate_script` + `take_screenshot`
- **Scope**: Tests RC-001 through RC-010
- **Protocol**: For each test case:
  1. Inject render counter into target component (via React DevTools hook or `console.count`)
  2. Read baseline render count
  3. Mutate Zustand store via `evaluate_script`
  4. Wait 500ms (3+ R3F frames at 60fps)
  5. Read new render count -- delta must be >= 1
  6. Take screenshot, verify visual change
- **Critical path**: This agent runs first for RC-001/002/003 (node type visibility) since this is the known-broken area

**Store Integrity Agent**
- **Tool**: `evaluate_script`
- **Scope**: Zustand store internal consistency
- **Checks**:
  - `settings` object is deeply frozen (Immer produce creates new refs)
  - Subscriber trie fires correct callbacks for each path mutation
  - RAF-batched notification flushes within one animation frame
  - No circular references in settings object

**Performance Agent**
- **Tool**: `performance_start_trace` / `performance_stop_trace` / `performance_analyze_insight`
- **Scope**: Tests CC-020 through CC-023
- **Protocol**:
  1. Start trace
  2. Execute toggle sequence (all features on, all off, rapid toggle)
  3. Stop trace
  4. Analyze: frame rate distribution, long frames, GC pauses

#### UX Layer Agents

**Visual Regression Agent**
- **Tool**: `take_screenshot`
- **Scope**: All VF-* tests
- **Protocol**: For each visual feature:
  1. Set feature to known baseline state
  2. Take "before" screenshot (labeled `VF-XXX-before.png`)
  3. Change feature to test state
  4. Wait 1s for rendering to settle
  5. Take "after" screenshot (labeled `VF-XXX-after.png`)
  6. Compare: pixel-diff must exceed threshold (5% of canvas pixels changed) for visible features, or be below threshold for features that should not change

**Interaction Agent**
- **Tool**: `click`, `click_at`, `hover`, `drag`, `press_key`
- **Scope**: All IX-* tests
- **Protocol**: Execute pointer interactions on canvas, verify state changes via `evaluate_script`

### 6.3 Execution Order

Phase ordering ensures dependencies are satisfied:

```
Phase 0: Environment Validation (5 min)
  - Navigate to http://192.168.2.132:3001/
  - Wait for canvas to render (canvasReady = true)
  - Verify REST API is responsive
  - Verify WS connects and delivers frames
  - Take baseline screenshot

Phase 1: Data Pipeline (10 min) [REST Probe + WS Monitor]
  - DP-001 through DP-010 (REST validation)
  - WS-001 through WS-006 (WebSocket validation)
  - Cross-check node counts (DP-004 variant)

Phase 2: Reconciler Boundary (15 min) [R3F Reconciler + Store Integrity]
  - RC-001 through RC-003 FIRST (node type visibility -- known bug area)
  - RC-004 through RC-010 (remaining selectors)
  - Store integrity checks (subscriber trie, RAF batching)

Phase 3: Visual Features (20 min) [Visual Regression + Settings Pipeline]
  - VF-001 through VF-003 (node visibility -- depends on Phase 2 pass)
  - VF-010 through VF-071 (all other visual features)
  - Settings round-trip for each feature

Phase 4: Interactions (10 min) [Interaction Agent]
  - IX-001 through IX-032 (click, hover, drag)
  - IX-040 through IX-042 (stress tests)

Phase 5: Cross-Cutting (15 min) [All agents]
  - CC-001 through CC-002 (WS reconnection)
  - CC-010 through CC-012 (persistence)
  - CC-020 through CC-023 (performance)

Phase 6: Report Generation (5 min) [Fleet Commander]
  - Aggregate all agent results
  - Generate pass/fail matrix
  - Store to memory
```

### 6.4 Failure Escalation

| Severity | Condition | Action |
|----------|-----------|--------|
| **BLOCKER** | Any RC-001/002/003 fails (node type visibility does not propagate) | Stop all further visual tests. Report root cause: Zustand selector returns stale object ref. |
| **CRITICAL** | Node count mismatch > 5% between REST/WS/rendered | Investigate data pipeline. May indicate dropped nodes or stale cache. |
| **MAJOR** | Visual feature toggle has no visual effect | Classify as reconciler bug (if store updated) or store bug (if store did not update). |
| **MINOR** | Performance below target but functional | Log metric, continue testing. |
| **INFO** | Feature works but screenshot diff is marginal | Manual review needed. |

### 6.5 Known Fragile Paths (Prioritize These)

1. **`nodeTypeVisibility` selector** (GraphManager.tsx:275-277) -- returns an object from deep in the Immer tree. If `updateSettings` uses `produce()` but only touches one key (e.g., `.knowledge`), the parent object reference may or may not change depending on Immer's structural sharing.

2. **`gemSettings` via `.get()` accessor** (GemNodes.tsx:122) -- the `.get<T>(path)` method walks the settings tree and returns a sub-object. If this is a direct reference into the Immer-produced tree (not a copy), it has the same staleness risk.

3. **`sceneEffects` selector** (GraphCanvas.tsx:162) -- returns an object with ~10 properties. A change to one property (e.g., `particleCount`) must produce a new object ref for the selector to trigger a re-render.

4. **Edge filtering in `useFrame`** (GraphManager.tsx:651-659) -- reads `nodeTypeVisibility` inside the frame loop. If the frame loop callback captures a stale closure over `nodeTypeVisibility`, edges to hidden nodes may persist.

5. **`typeFilteredNodes` useMemo** (GraphManager.tsx:311-325) -- depends on `nodeTypeVisibility`. If the selector does not trigger a re-render, this `useMemo` never recomputes, and GemNodes receives stale `nodes` array.

---

## 7. Browser MCP Tool Mapping

Each test category maps to specific chrome-devtools-mcp tools:

| Agent Role | Primary Tools | Secondary Tools |
|-----------|--------------|-----------------|
| REST Probe | Bash (`docker exec curl`) | -- |
| WS Monitor | `evaluate_script` | `list_console_messages` |
| Settings Pipeline | `evaluate_script`, Bash | `take_screenshot` |
| R3F Reconciler | `evaluate_script`, `take_screenshot` | `list_console_messages` |
| Store Integrity | `evaluate_script` | `get_console_message` |
| Performance | `performance_start_trace`, `performance_stop_trace`, `performance_analyze_insight` | `take_memory_snapshot` |
| Visual Regression | `take_screenshot` | `take_snapshot` (DOM) |
| Interaction | `click`, `click_at`, `hover`, `drag` | `evaluate_script` |

---

## 8. Zustand Selector Hardening Recommendations

Based on this analysis, these selectors should be refactored to use **shallow comparison** or **primitive extraction** to prevent reconciler boundary bugs:

```typescript
// BEFORE (fragile -- object ref comparison):
const nodeTypeVisibility = useSettingsStore(
  s => s.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility
);

// AFTER (robust -- extract primitives):
import { shallow } from 'zustand/shallow';
const { showKnowledge, showOntology, showAgent } = useSettingsStore(
  s => ({
    showKnowledge: s.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility?.knowledge ?? true,
    showOntology: s.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility?.ontology ?? true,
    showAgent: s.settings?.visualisation?.graphs?.logseq?.nodes?.nodeTypeVisibility?.agent ?? true,
  }),
  shallow
);
```

This same pattern should be applied to all HIGH-risk selectors identified in Section 2.2.

---

## 9. Success Criteria

| Criterion | Threshold |
|-----------|-----------|
| All BLOCKER tests pass | 100% (RC-001, RC-002, RC-003) |
| All CRITICAL tests pass | 100% (node count consistency) |
| All MAJOR tests pass | >= 95% |
| All visual toggle tests show measurable pixel diff | >= 90% |
| Frame rate under full load | >= 30 fps |
| Settings persistence round-trip | 100% |
| WS reconnection recovery | < 10s |
| No memory leak over 30 min | Retained heap growth < 10% |
