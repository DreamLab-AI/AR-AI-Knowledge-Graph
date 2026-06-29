# WORKTREE-PLAN — Phase 4: Client State & Workers

Author  : worktree-planner
Branch  : impl/phase-4-client-state (off radical-rollback @ d260a6158)
Date    : 2026-05-16
Depends : Phase 3 binary-protocol branch must be merged before any code
          changes here land on main. ADR-02 cadence contract (settlement-gated,
          max 10 Hz) is a pre-condition for the single-flight design in ADR-03 D2.

---

## 1. Phase 4 Task Breakdown

| ID  | Unit of Work                           | Files                                                                                              | Acceptance Criteria                                                 | Complexity |
|-----|----------------------------------------|----------------------------------------------------------------------------------------------------|---------------------------------------------------------------------|------------|
| T1  | Worker surface trim to D7 contract     | `graph.worker.ts`, `graphWorkerProxy.ts`                                                           | Proxy exports exactly: `WORKER_USES_SAB`, `attachPositionSAB`, `writeFrame`, `computeEdgeLengths`, `getStats`. All other methods removed or made private. Structural test passes. | High |
| T2  | SAB/Comlink capability detection       | `graphWorkerProxy.ts`                                                                              | `WORKER_USES_SAB` constant set at module load. Dev overlay reads it. `VITE_FORCE_COMLINK=1` forces fallback path. | Medium |
| T3  | Single-flight binary-frame guard       | `client/src/store/websocket/index.ts`                                                              | `_binaryFrameInFlight` boolean + `_pendingLatest` slot replaces current `_pendingBinaryFrame`/`_binaryFrameScheduled` pair. `finally` block releases guard. `queueMicrotask` re-entry between frames. | High |
| T4  | Remove `BinaryFrameCoalescer` surface  | `client/src/store/websocket/index.ts`, any import of coalescer                                     | No `BinaryFrameCoalescer` class. No drain loop. No `drainLoopCoalescer.tick()` call in `useFrame`. | Low |
| T5  | SAB primary write path                 | `graph.worker.ts`, `graphWorkerProxy.ts`                                                           | `writeFrame(buf)` parses V3 header, writes x/y/z into SAB-backed `Float32Array`. Main thread reads SAB in `useFrame` without an additional Comlink call. | High |
| T6  | Comlink transfer fallback path         | `graph.worker.ts`, `graphWorkerProxy.ts`                                                           | When `SAB_OK === false`, `writeFrame` transfers its internal `ArrayBuffer` back; main re-wraps as `Float32Array`. Structured clone never occurs. `VITE_FORCE_COMLINK=1` exercises this path. | High |
| T7  | Remove double Comlink round-trip       | `graphWorkerProxy.ts`, `graphDataManager.ts`                                                       | No `worker.processBinaryData()` followed by `worker.getGraphData()` or `worker.getPositions()`. `writeFrame` is the single call per frame. | Medium |
| T8  | Remove `graphDataLoaded` guard         | `graph.worker.ts`                                                                                  | `graphDataLoaded` flag and `pendingBinaryFrames` queue removed. D8 (positions never touch node map) makes the race impossible. | Low |
| T9  | `graphDataManager` single-path delivery | `graphDataManager.ts`                                                                              | Single `_setData(incoming)` internal method. `queueMicrotask` dispatch to subscribers. Identity check on node-id list + edge-id list short-circuits duplicate deliveries. `lastGraphData` field is public-readable. | High |
| T10 | `graphDataManager` subscribe dedup     | `graphDataManager.ts`                                                                              | `subscribe(cb)` uses a `Set<Callback>`. Duplicate registrations are no-ops. Unsubscribe removes from set. Four-path delivery consolidated to one. | Medium |
| T11 | Zustand store decomposition (ADR D4)   | `client/src/store/` (new `graphStore.ts` or reshaping existing)                                    | `useGraphStore` exposes: `nodes: Map<NodeId,Node>`, `edges: Map<EdgeId,Edge>`, `nodeCount`, `edgeCount`, `topologyHash`, `physicsState`. No composite `graph` slice. | High |
| T12 | Narrow selector sweep                  | `GraphManager.tsx`, `GraphCanvas.tsx`, `GemNodes.tsx`, `GlassEdges.tsx`, `useGraphFiltering.ts`, `WasmSceneEffects.tsx`, `SystemHealthIndicator.tsx`, `AgentControlPanel.tsx`, `PerformanceControlPanel.tsx` | All selectors return primitives or stable map references. Flagged broad selectors replaced (see Section 5). | High |
| T13 | `no-broad-zustand-selector` lint rule  | `.eslintrc.json` or `eslint.config.js`, new `rules/no-broad-zustand-selector.js`                  | Rule blocks `s => ({...})` and `s => s.graph` patterns. Allowlist comment `// zustand-broad-selector-allowed: <reason>` suppresses. CI runs lint on every commit. | Medium |
| T14 | `GraphManager` identity short-circuit  | `GraphManager.tsx`                                                                                 | `lastProcessedGraphRef` and `lastShapeRef` refs. Reference equality fast path. Topology hash slow path. Position updates via SAB never trigger rebuild. | High |
| T15 | WebSocket handler D8 contract          | `client/src/store/websocket/index.ts`, `client/src/store/websocket/binaryProtocol.ts`             | Binary frame handler validates V3 magic, applies single-flight guard (T3), calls `graphWorker.writeFrame(transfer(buf, [buf]))`. Does not write to Zustand. | Medium |
| T16 | Topology/metadata text-frame handlers  | `client/src/store/websocket/textMessageHandler.ts`, `graphDataManager.ts`                         | `graph-data` JSON frames routed to `graphDataManager.refresh()`. `bot-telemetry` frames routed to Section 7 handler on main thread (per CC-14). No worker involvement. | Medium |
| T17 | Dev overlay hook                       | New `hooks/useDevOverlay.ts`                                                                       | Reads `WORKER_USES_SAB`, polls `graphWorker.getStats()` every 1s (dev only). Exposes `framesDropped`, `physicsState`, `sabMode`. Gated by `import.meta.env.DEV`. | Low |
| T18 | Worker structural test                 | `managers/__tests__/graphWorkerProxy.test.ts`                                                      | Asserts proxy surface matches D7 contract. Asserts `writeFrame` is the only entry point for binary data. | Medium |
| T19 | Single-flight regression test          | `store/websocket/__tests__/singleFlight.test.ts`                                                   | Simulates three concurrent binary frames; asserts only 2 calls reach `writeFrame` (first + newest-wins pending). | Medium |
| T20 | Re-render cascade regression test      | `components/__tests__/GraphManager.cascade.test.tsx`                                               | Mounts GraphManager; delivers 60 binary frames via mock WebSocket; asserts GraphManager renders fewer than 5 times (SAB path skips React entirely). | High |

**Total tasks: 20**

---

## 2. Worker Boundary Contract

### Worker owns

- **Binary frame parsing**: receive a transferred `ArrayBuffer`, read V3 magic
  (`0xV3F0`), per-node 28-byte entries (4-byte `node_id` + 12-byte xyz), trailer
  `node_count`. No other formats.
- **Position writes**: write parsed x/y/z into the SAB-backed `Float32Array` (SAB
  mode) or into a worker-owned `ArrayBuffer` that is transferred back to main
  (Comlink mode). No structured clone.
- **Edge-length computation** (on demand): `computeEdgeLengths(edges: EdgeId[])` for
  layout-feedback use by Section 4 renderer. Result transferred back.
- **Frame metadata**: emit `{ frame_id, node_count, dropped }` synchronously in the
  `writeFrame` return value. Main thread reads this once per frame.

### Worker does NOT own

- **Zustand store**: the worker has no import of any store module.
- **WebSocket**: the main thread is the WebSocket consumer; it transfers raw buffers.
- **REST orchestration**: `graphDataManager` on the main thread handles all REST fetches.
- **Topology state**: the worker parses positions against the `node_count` in the
  frame header. It does not maintain a copy of the node list.
- **Telemetry decoding (CC-14)**: agent telemetry arrives as text-frame JSON on the
  WebSocket. It is decoded on the main thread by the Section 7 text-frame handler.
  The worker proxy surface has exactly four methods (D7); it is closed to telemetry.
  Single-flight discipline for telemetry uses the Section 7 coalescer (DDD-07 D11).
- **Client-side physics / tweening**: the worker's current `tick()` method, tween
  settings, force-physics settings, and all client interpolation code are removed
  as part of T1. GPU physics (Section 1) is the sole source of positions; Section 3
  delivers them to the canvas without additional interpolation.

---

## 3. Single-Flight Invariant

The baseline already has a partial mechanism in
`client/src/store/websocket/index.ts` (`_pendingBinaryFrame`, `_binaryFrameScheduled`,
`queueMicrotask`). This is synchronous-slot-only — it handles the case of a new
frame arriving before the previous microtask fires, but it does not guard against
the `await` inside `processBinaryData` (which crosses the Comlink boundary). The
ADR-03 D2 design is the correct full guard.

### Replace with (T3)

```
_binaryFrameInFlight: boolean  — closed over in the WebSocket connect scope
_pendingLatest: ArrayBuffer | null  — one-element slot, newest-wins

function handleBinaryFrame(frame: ArrayBuffer): void {
    if (_binaryFrameInFlight) {
        _pendingLatest = frame;   // drops any previously pending frame
        return;
    }
    _binaryFrameInFlight = true;
    processFrame(frame)           // async, crosses Comlink boundary
        .then(() => {
            if (_pendingLatest !== null) {
                const next = _pendingLatest;
                _pendingLatest = null;
                queueMicrotask(() => handleBinaryFrame(next));
            }
        })
        .catch(err => logger.error('frame processing error', err))
        .finally(() => { _binaryFrameInFlight = false; });
}
```

Key properties:
- At most one frame is being processed across the `await`. No parallel Comlink calls.
- At most one frame is pending. Intermediate frames are discarded. This is correct
  under V3 full-sync: the newest frame wholly supersedes any older one.
- The `queueMicrotask` re-entry yields once between frames, allowing the React render
  loop a chance to run before the next frame begins.
- The `finally` block releases the guard even on error, preventing permanent deadlock.
- `_pendingLatest` is the entire coalescing strategy. There is no `BinaryFrameCoalescer`
  class, no drain loop, no tick driver.

### What this replaces

The current `_pendingBinaryFrame`/`_binaryFrameScheduled` pattern (lines 237-264 of
`store/websocket/index.ts`) is removed and replaced by the guard above (T3). The
`scheduleBinaryProcessing` function is deleted.

---

## 4. SAB-vs-Comlink Decision Tree

Implemented once at module load in `graphWorkerProxy.ts` (T2):

```
┌─ Module load ────────────────────────────────────────────┐
│                                                          │
│  SAB_OK = (typeof SharedArrayBuffer === 'function')      │
│           && (self.crossOriginIsolated === true)         │
│                                                          │
│  export const WORKER_USES_SAB: boolean = SAB_OK          │
│                                                          │
└─────────────────────────────────┬────────────────────────┘
                                  │
              ┌───────────────────┴────────────────────┐
              │ SAB_OK === true                        │ SAB_OK === false
              ▼                                        ▼
   Worker startup:                          Per-frame call:
   allocate SharedArrayBuffer               worker.writeFrame(
   wrap as Float32Array                       Comlink.transfer(frame, [frame])
   call worker.attachPositionSAB(sab)       )
   nodePositionsRef.current = view          → returns transferred ArrayBuffer
                                            main re-wraps:
   Per-frame call:                            new Float32Array(returnedBuf)
   worker.writeFrame(                       nodePositionsRef.current = view
     Comlink.transfer(frame, [frame])       previous Float32Array → GC
   )
   → returns void
   renderer reads SAB in useFrame
   (no notification needed)
```

Rules that must never be violated:
- A frame buffer crosses the worker boundary by `Comlink.transfer()` only. The
  source `ArrayBuffer` is neutered after the call; the WebSocket's internal copy is
  gone. If the WebSocket provides a `Blob`, convert to `ArrayBuffer` first, then
  transfer.
- In SAB mode, main never calls `worker.getPositions()` after `worker.writeFrame()`.
  The SAB is the shared view; the renderer reads it directly.
- In Comlink mode, `writeFrame` is a single round-trip: main sends, worker returns
  the same buffer (or a worker-owned one). The two-call pattern `parse() + get()`
  is permanently rejected.
- `VITE_FORCE_COMLINK=1` overrides `SAB_OK` to `false` regardless of
  `crossOriginIsolated` status, so developers exercise the fallback path during
  normal development.

---

## 5. Zustand Narrowing Audit

### Selectors found in the current codebase

| Location | Selector | Verdict |
|---|---|---|
| `GraphManager.tsx:270` | `s => s.settings?.visualisation?.graphs?.logseq` | Broad — returns composite object. Split into individual primitives or use `useShallow`. |
| `GraphManager.tsx:271` | `s => s.settings?.visualisation?.graphTypeVisuals` | Broad — composite object. Split. |
| `GraphManager.tsx:272` | `s => s.settings?.visualisation?.glow?.intensity ?? 0.3` | Acceptable — returns number primitive. |
| `GraphManager.tsx:273` | `s => s.settings?.system?.debug` | Broad — returns `DebugSettings` object. Decompose into `enablePhysicsDebug`, `enableNodeDebug`, etc. |
| `GraphManager.tsx:274` | `s => s.settings?.nodeFilter` | Broad — composite. Split into `nodeFilter.enabled`, `nodeFilter.qualityThreshold`, etc. |
| `GraphManager.tsx:275` | `s => s.settings?.visualisation?.nodeTypeVisibility` | Broad — composite. Split per type flag. |
| `GraphManager.tsx:469` | `s => s.settings?.visualisation?.graphs?.visionclaw?.physics` | Broad — composite. Split individual physics scalars. |
| `GraphManager.tsx:481` | `s => s.settings?.visualisation?.graphs?.visionclaw?.layoutMode` | Acceptable if `layoutMode` is a string/enum primitive. Verify. |
| `GraphCanvas.tsx:109` | `s => s.settings?.visualisation?.sceneEffects` | Broad — composite. Split into individual effect booleans. |
| `GlassEdges.tsx:143` | `s => s.get<GlowSettings>('visualisation.glow')` | Broad — returns object. Split into `glowEnabled`, `glowIntensity`, etc. |
| `GlassEdges.tsx:148` | `s => s.get<GemMaterialSettings>('visualisation.gemMaterial')` | Broad — returns object. Split into individual material scalars. |
| `GemNodes.tsx:115` | `s => s.get<GemMaterialSettings>('visualisation.gemMaterial')` | Broad — same as above. |
| `GemNodes.tsx:119` | `s => s.get<QualityGatesSettings>('qualityGates')` | Broad — returns object. Split into `maxNodeCount` etc. |
| `useGraphFiltering.ts:49` | `s => s.settings?.nodeFilter` | Broad — see `GraphManager.tsx:274` entry. |
| `WasmSceneEffects.tsx:642` | `s => s.get<SceneEffectsSettings>('visualisation.sceneEffects')` | Broad — returns object. Split by effect. |
| `GraphCanvas.tsx:105-110` (6 lines) | Individual primitives with `?.` + `?? default` | Acceptable — all return boolean or number. |
| `SystemHealthIndicator.tsx:267-268` | `s => s.settingsSyncEnabled`, `s => s.setSettingsSyncEnabled` | Acceptable — primitive and stable action reference. |

### Broad selectors to rewrite (T12)

1. `GraphManager.tsx:270` — `logseqSettings` — split into `logseq_linkWidth`, `logseq_nodeOpacity`, etc. as individual selectors.
2. `GraphManager.tsx:271` — `graphTypeVisuals` — split per primitive.
3. `GraphManager.tsx:273` — `debugSettings` — three separate selectors: `enablePhysicsDebug`, `enableNodeDebug`, `enablePerformanceDebug`.
4. `GraphManager.tsx:274` — `nodeFilterSettings` — split into primitives consumed by the filter predicate.
5. `GraphManager.tsx:469` — `visionclawPhysics` — split into scalar physics params.
6. `GraphCanvas.tsx:109` — `sceneEffects` — one selector per effect toggle.
7. `GlassEdges.tsx:143,148` — two `s.get<>()` calls returning objects — split each.
8. `GemNodes.tsx:115,119` — same pattern — split.
9. `useGraphFiltering.ts:49` — `nodeFilter` — split into the three filter primitives actually used.
10. `WasmSceneEffects.tsx:642` — split per-effect.

### Lint rule (T13)

Custom ESLint rule `no-broad-zustand-selector` in `eslint-rules/`:

- Flags any `useXxxStore(s => s.someField)` where the selector function body is not
  a member expression ending in a primitive field or a `.get<T>()` returning a known
  scalar type.
- Flags `s => ({...})` object-literal returns.
- Flags `s => s` (whole store).
- Allowlist: `// zustand-broad-selector-allowed: <reason>` on the preceding line.
  Each allowlist entry requires a reviewer sign-off in the PR.
- Enforced in CI via `eslint --max-warnings 0`.

---

## 6. `graphDataManager` Cache and Delivery

### Current state (baseline)

`graphDataManager` has four delivery paths: REST fetch in `fetchInitialData`,
WebSocket `graph-data` message dispatch in `textMessageHandler`, a retry handler,
and a manual `refresh()`. Each path independently calls `notifyGraphDataListeners`,
which does an async round-trip to the worker (`graphWorkerProxy.getGraphData()`) to
retrieve the current data before invoking subscribers. This means identical payloads
can fire subscribers four times per topology change.

### Target design (T9, T10)

```
class GraphDataManager {
    private lastGraphData: GraphData | null = null;
    private subscribers: Set<GraphDataChangeListener> = new Set();

    private _setData(incoming: GraphData): void {
        // Identity check on stable id lists (O(n+m), not deep-equal)
        if (this.lastGraphData !== null
            && incoming.nodes.length === this.lastGraphData.nodes.length
            && incoming.edges.length === this.lastGraphData.edges.length
            && incoming.nodes[0]?.id === this.lastGraphData.nodes[0]?.id) {
            return;  // same topology, skip
        }
        this.lastGraphData = incoming;
        queueMicrotask(() => {
            this.subscribers.forEach(cb => cb(incoming));
        });
    }

    subscribe(cb: GraphDataChangeListener): () => void {
        this.subscribers.add(cb);
        if (this.lastGraphData !== null) {
            // Deliver current data synchronously to new subscriber
            queueMicrotask(() => cb(this.lastGraphData!));
        }
        return () => this.subscribers.delete(cb);
    }

    getLastGraphData(): GraphData | null {
        return this.lastGraphData;
    }

    refresh(): Promise<void> {
        // REST fetch, then calls _setData
    }

    _onWebSocketGraph(payload: unknown): void {
        // Parse, then calls _setData
    }
}
```

Key properties:
- `lastGraphData` is set once per genuine topology change.
- One `queueMicrotask` per change — one notification to all subscribers.
- The `Set` deduplicates registrations; duplicate `subscribe(cb)` calls for the same
  function reference are no-ops.
- The identity check is cheap: length equality plus first-node id equality catches
  the common case of re-delivery of the same REST response. For edge cases (same
  length, different first id), the check passes and subscribers are notified; this
  is correct.
- The `graphDataLoaded` guard in the worker (T8) is removed because the race it
  addresses (binary frames arriving before the node map is populated) cannot occur
  under D8: position frames write to SAB/transferred-buffer and never consult the
  node map on the main thread.

---

## 7. `GraphManager` Short-Circuit

### Current state (baseline)

`GraphManager.tsx` (1349 lines) has no `lastProcessedGraphRef`. Every re-render
triggered by any Zustand subscription (including the many broad selectors from
Section 5) runs the full edge-buffer rebuild and instanced-mesh reconciliation.
A position update that arrives via SAB does not itself trigger a React re-render,
but any Zustand subscriber firing during the same frame tick does — and any of the
broad selectors can cause that.

### Target design (T14)

Two refs are added at the top of the component function:

```
const lastProcessedGraphRef = useRef<GraphData | null>(null);
const lastShapeRef = useRef<{ nodeCount: number; edgeCount: number; hash: string } | null>(null);
```

In the effect that subscribes to `graphDataManager`:

```
function handleGraphData(incoming: GraphData): void {
    if (incoming === lastProcessedGraphRef.current) {
        return;  // reference identity fast path — same object, no rebuild
    }
    const shape = {
        nodeCount: incoming.nodes.length,
        edgeCount: incoming.edges.length,
        hash: topologyHash(incoming),  // cheap: join first+last node id + counts
    };
    if (shapeEqual(shape, lastShapeRef.current)) {
        // Same topology, different reference (e.g. metadata-only update)
        lastProcessedGraphRef.current = incoming;
        return;  // adopt new reference, skip rebuild
    }
    // Genuine topology change — rebuild
    rebuildEdgeBuffers(incoming);
    rebuildInstancedMeshes(incoming);
    lastProcessedGraphRef.current = incoming;
    lastShapeRef.current = shape;
}
```

In `useFrame` (position read path): unchanged. The renderer reads SAB directly on
every frame. The short-circuit applies only to topology rebuild, never to per-frame
position reads.

The `topologyHash` function: `${nodes.length}-${edges.length}-${nodes[0]?.id ?? ''}-${nodes[nodes.length-1]?.id ?? ''}`.
This is O(1) and avoids sorting or full traversal.

---

## 8. Spawn Plan

When Phase 4 implementation begins, three agents are spawned:

### Agent 1 — Coder: Worker Proxy Refactor

**Scope**: T1, T2, T3, T4, T5, T6, T7, T8, T15

**Primary files**:
- `client/src/features/graph/workers/graph.worker.ts`
- `client/src/features/graph/managers/graphWorkerProxy.ts`
- `client/src/store/websocket/index.ts`
- `client/src/store/websocket/binaryProtocol.ts`

**Acceptance gate**: T18 structural test passes; `WORKER_USES_SAB` constant exported
and readable; `writeFrame` is the only binary entry point; single-flight guard
present with `finally` release; no `BinaryFrameCoalescer` reference in the tree.

**Isolation**: worktree `wt-add phase-4-coder`; merges back on green CI.

### Agent 2 — Tester: Regression Test Suite

**Scope**: T18, T19, T20

**Primary files**:
- `client/src/features/graph/managers/__tests__/graphWorkerProxy.test.ts` (T18)
- `client/src/store/websocket/__tests__/singleFlight.test.ts` (T19)
- `client/src/features/graph/components/__tests__/GraphManager.cascade.test.tsx` (T20)

**Test scenarios**:
- T18: Assert proxy exports exactly the D7 surface. Assert that calling any removed
  method (e.g. `tick`, `processBinaryData`, `getGraphData`) throws a type error at
  compile time (TS strict).
- T19: Single-flight test — mock `writeFrame` with a 50ms delay; fire three frames;
  assert `writeFrame` called exactly twice (first + newest-wins pending); assert the
  second call receives the third buffer (not the second).
- T20: Cascade test — mount `GraphManager` with a mock `graphDataManager`; deliver
  60 binary frames via a mock SAB write; assert React DevTools `renderCount` is
  less than 5 during frame delivery (topology unchanged, no rebuild triggered).

**Isolation**: same worktree as Agent 1 (tests depend on the contracts Agent 1 creates).
Agent 2 starts after Agent 1 has drafted the interface; they can work in parallel
once the TypeScript types are stable.

### Agent 3 — Code Reviewer: Selector Narrowness Audit

**Scope**: T12, T13

**Primary files**: all files listed in Section 5 broad-selector table.

**Checklist**:
1. For each broad selector in the table, confirm that splitting into primitives
   does not silently break a memoization pattern downstream (e.g. `useMemo` whose
   dependency list assumed a single object reference).
2. Confirm each replaced selector uses two separate `useXxxStore` calls rather than
   a synthesised object or `useShallow` on a composite — unless `useShallow` is the
   only safe option, in which case document the reason in the allowlist comment.
3. Write the `no-broad-zustand-selector` lint rule (T13) and apply it to the tree.
   Confirm zero lint errors after the sweep.
4. Confirm that `useGraphStore` (the new store from T11) has no composite slice
   accessor in any component.

**Isolation**: worktree `wt-add phase-4-reviewer`; this agent's changes are
independent of Agent 1 (different files) and can be merged in parallel after review.

---

## Risk Register

### R1 — Worker surface trim breaks existing consumers (HIGH)

The current `graphWorkerProxy.ts` exports 18+ public methods. Removing all but 4
will cause TypeScript compilation errors in any component that calls the removed
methods. Mitigation: run `tsc --noEmit` immediately after T1 to surface all
breakage; fix call sites before merging. The coder agent must audit every import of
`graphWorkerProxy` before trimming.

### R2 — SAB path exercised rarely in CI (HIGH)

CI environments typically do not serve COOP/COEP headers, so `crossOriginIsolated`
is false in test runs. The SAB path (`WORKER_USES_SAB === true`) is never exercised
by the test suite unless specifically forced. Mitigation: add a dedicated CI job
that sets the Vite dev server to serve COOP/COEP headers and runs T19/T20 in that
context. `VITE_FORCE_COMLINK=1` is not sufficient — it only tests the fallback.

### R3 — `graphDataManager` identity check produces false negatives (MEDIUM)

The cheap identity check (length + first-node-id) will pass (trigger notification)
for any topology where the server reorders nodes without changing count or first-id.
This is not a correctness bug — false negatives cause a rebuild that was not
strictly necessary but produce correct output. The risk is performance regression
if the server emits cosmetically different but structurally identical payloads at
high frequency. Mitigation: monitor the `lastProcessedGraphRef` fast-path hit rate
in the dev overlay; if it is low, tighten the check to include a hash of the last
node-id as well.

---

## Summary

| Metric | Value |
|---|---|
| Total tasks | 20 |
| High complexity | 8 (T1, T3, T5, T6, T9, T11, T12, T14, T20) |
| Medium complexity | 8 (T2, T7, T10, T13, T15, T16, T18, T19) |
| Low complexity | 4 (T4, T8, T17, T18 — structural test) |
| Agents spawned | 3 (coder, tester, reviewer) |
| Top 3 risks | R1 (worker surface trim), R2 (SAB CI coverage), R3 (identity-check false negatives) |
