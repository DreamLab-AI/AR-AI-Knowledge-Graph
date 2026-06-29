# ADR-03 — Client State & Workers

Status   : Proposed
Date     : 2026-05-16
Related  : ADR-02 (Binary Protocol), ADR-04 (Rendering), ADR-06 (Auth)

## Context

The client side of the freeze regression was a layering problem disguised
as a performance problem. Position frames flowing from the server at
moderate cadence were causing render-loop saturation because the main
thread was doing too much work per frame and the worker boundary was
being crossed twice per frame by structured-clone copies. On top of that,
broad Zustand selectors made every store touch fan out into a forest of
React re-renders, and `graphDataManager` was delivering the same payload
to subscribers from four code paths. Each of these is independently
correctable; the patches accumulated on `main` (`61968e3d0` narrowing,
`4c126cffc` round-trip elimination, `695be6d6d` map-clearing guard) form
a defensive perimeter around the original architecture rather than
replacing it.

This ADR rebuilds the client state and worker layer from the contract
inward: what crosses the worker boundary, what shape the store has, how
deliveries dedupe, and what the renderer treats as identity. The
implementation choices in commits like `4c126cffc` and `61968e3d0` are
adopted in *intent*; the symptom-level guards (`695be6d6d`, `4fd7e1f9c`)
are not migrated.

## Decision

### D1. Worker boundary and ownership

A single `graphWorker.ts` runs in a dedicated Worker. It is the only
worker in the client. Its responsibilities are:

- **Binary frame parse**: take a transferred `ArrayBuffer`, decode the V3
  payload (header + 28-byte node entries + trailer), and write positions
  into the destination buffer.
- **Destination buffer**: either a `SharedArrayBuffer`-backed
  `Float32Array` (SAB mode), or a worker-owned `Float32Array` that is
  transferred back to main on each call (Comlink mode).
- **Edge-length computation**: for any edge that the renderer flags as
  "layout-feedback eligible", compute current length from current
  positions. This avoids the renderer doing per-edge maths on the main
  thread inside `useFrame`.
- **Frame metadata**: emit `{ frame_id, node_count, dropped }` back to
  main per frame. Used by the dev overlay and by `graphWorkerProxy` to
  detect server-side gaps.

The worker does not:
- own the Zustand store (it does not import `useGraphStore`),
- talk to the WebSocket (the main thread is the WebSocket consumer),
- talk to REST (the main thread is the REST consumer),
- maintain its own copy of topology (it parses frames against whatever
  node count the message header declares).

This is enforced by `graphWorkerProxy.ts` exposing a narrow Comlink-style
surface (D7) — the worker has no other entry points.

### D2. Single-flight binary frame discipline

The WebSocket binary-frame handler holds a single boolean flag,
`_binaryFrameInFlight`. On message arrival:

```
if (_binaryFrameInFlight) {
    _pendingLatest = frame; // newest-wins, max 1 pending
    return;
}
_binaryFrameInFlight = true;
try {
    await processFrame(frame);
    if (_pendingLatest) {
        const next = _pendingLatest;
        _pendingLatest = null;
        // re-enter via microtask so we yield once between frames
        queueMicrotask(() => handleBinaryFrame(next));
    }
} finally {
    _binaryFrameInFlight = false;
}
```

`_pendingLatest` is a *slot*, not a queue. Multiple frames arriving while
processing is in progress collapse to one. This is the entire
coalescing strategy — no separate `BinaryFrameCoalescer` class, no drain
loop, no tick driver. The newest-wins behaviour is correct under V3
full-sync semantics: any subsequent frame fully supersedes the previous.

### D3. SAB primary, Comlink transfer fallback

At module load:

```
const SAB_OK = (
    typeof SharedArrayBuffer === 'function' &&
    self.crossOriginIsolated === true
);
export const WORKER_USES_SAB = SAB_OK;
```

If `SAB_OK`:
- Main thread allocates `SharedArrayBuffer(MAX_NODES * 12)` (x, y, z per
  node, 4 bytes each).
- Wraps it in `Float32Array`. This is `nodePositionsRef.current` for the
  renderer.
- Hands the same SAB handle to the worker once at startup via
  `worker.attachPositionSAB(sab)`.
- On each frame, main calls `worker.writeFrame(transferBuf)`. The worker
  parses and writes positions into the SAB. The renderer reads SAB in
  `useFrame`; no notification to main is required.

If not `SAB_OK`:
- Main thread allocates `ArrayBuffer(MAX_NODES * 12)` and a matching
  `Float32Array` view.
- On each frame, main calls
  `const buf = await worker.writeFrame(Comlink.transfer(frame, [frame]))`.
  The worker parses *into* an internal `ArrayBuffer`, then transfers it
  back: `Comlink.transfer(internal, [internal])`.
- Main re-wraps the returned buffer with `new Float32Array(buf)`. This is
  the new `nodePositionsRef.current`. The previous one is GC'd next tick.

In both modes the buffer crosses the worker boundary by transfer or by
shared reference. Structured clone of position payloads is never allowed.

### D4. Zustand store organisation

`useGraphStore` is decomposed into primitive fields. There is no `graph`
slice that contains everything. Top-level fields are:

- `nodes: Map<NodeId, Node>` — stable reference, mutated via store
  actions only. Selectors read by key, not by iterating the whole map.
- `edges: Map<EdgeId, Edge>` — same discipline as `nodes`.
- `nodeCount: number` — primitive, derived once per topology change.
- `edgeCount: number` — primitive, derived once per topology change.
- `topologyHash: string` — cheap hash of nodeIds + edgeIds. Used by D6
  short-circuit.
- `selection: never lives here` — see PRD non-goals; selection is a
  separate store.
- `physicsState: 'ACTIVE' | 'SETTLED' | 'UNKNOWN'` — primitive, set from
  binary frame metadata. The dev overlay subscribes to this.

The lint rule `no-broad-zustand-selector` enforces that selectors return
primitives or stable map references, not synthesised objects. This rule
is part of the client repo's eslint config (a custom rule) and runs in
CI.

Component subscription pattern:

```
const nodeCount = useGraphStore(s => s.nodeCount); // fine
const node = useGraphStore(s => s.nodes.get(id)); // fine (Map.get is by key)
const summary = useGraphStore(s => ({ n: s.nodeCount, e: s.edgeCount })); // rejected
```

For the rejected pattern, components use two selectors:
```
const n = useGraphStore(s => s.nodeCount);
const e = useGraphStore(s => s.edgeCount);
```
Two narrow subscriptions are cheaper than one broad subscription because
each only re-renders when its primitive changes.

### D5. `graphDataManager` cache + microtask delivery

`graphDataManager` is the single source of REST-fetched graph topology.
Its public surface:

```
subscribe(callback): unsubscribe
getLastGraphData(): GraphData | null
refresh(): Promise<void>
```

Internal state:
- `lastGraphData: GraphData | null`
- `subscribers: Set<Callback>`

On data arrival from REST:
1. If `deepEqualByIdentity(incoming, lastGraphData)` returns true, return
   immediately. This is the dedup short-circuit. Identity here is
   reference equality on the node id list and edge id list (cheap,
   O(n+m)), not deep diff.
2. Else `lastGraphData = incoming`.
3. `queueMicrotask(() => subscribers.forEach(cb => cb(incoming)))`.

There is exactly one delivery path. The four paths on `main`
(REST fetch, websocket "graph-data", retry handler, manual refresh)
all funnel into `graphDataManager.refresh()` or
`graphDataManager._onWebSocketGraph()`, both of which invoke the same
internal `_setData(incoming)` method. Subscribers see one call per data
change.

### D6. `GraphManager` identity short-circuit

`GraphManager.tsx` subscribes to `graphDataManager` and to
`useGraphStore`. It keeps two refs:

- `lastProcessedGraphRef: { current: GraphData | null }`
- `lastShapeRef: { current: { nodeCount: number; edgeCount: number; hash: string } | null }`

On every render-driven reconciliation, before rebuilding edge buffers,
instanced meshes, or label glyph attributes:

```
if (incomingGraph === lastProcessedGraphRef.current) return;
const shape = { nodeCount, edgeCount, hash: topologyHash };
if (shapeEqual(shape, lastShapeRef.current)) {
    lastProcessedGraphRef.current = incomingGraph; // adopt new ref, no rebuild
    return;
}
// real rebuild
rebuildEdgeBuffers(incomingGraph);
rebuildInstancedMeshes(incomingGraph);
lastProcessedGraphRef.current = incomingGraph;
lastShapeRef.current = shape;
```

Reference identity is the fast path. Topology hash is the slow path
fallback for cases where `graphDataManager` legitimately produces a new
reference for the same topology (e.g. metadata-only update). Position
updates do not flow through here at all; they flow directly into the SAB
or the transferred buffer and the renderer reads them in `useFrame`.

### D7. `graphWorkerProxy.ts` surface

The proxy is the only main-thread import that knows about the worker. Its
exported surface is:

```
export const WORKER_USES_SAB: boolean;

export interface GraphWorker {
    attachPositionSAB(sab: SharedArrayBuffer): Promise<void>; // SAB mode only
    writeFrame(frame: ArrayBuffer): Promise<ArrayBuffer | void>;
        // returns transferred buffer in Comlink mode, void in SAB mode
    computeEdgeLengths(edges: EdgeId[]): Promise<Float32Array>;
        // out-of-band, used by layout feedback; transferred back
    getStats(): Promise<{ framesProcessed: number; framesDropped: number }>;
}

export const graphWorker: GraphWorker;
```

The proxy is a singleton. There is no `createGraphWorker()` factory; the
app has one and only one worker for the lifetime of the page. On
HMR-driven module replacement in dev, the proxy detects the stale Comlink
endpoint and reconstructs the worker. Production builds never see this
path.

Nothing else imports the worker module directly. `processBinaryData` in
the WebSocket handler imports `graphWorker` from the proxy and calls
`writeFrame` only.

### D8. WebSocket handler contract

`websocketStore.ts` (or its replacement) exposes a binary-frame handler.
That handler:

1. Receives `MessageEvent<ArrayBuffer>`.
2. Validates the V3 magic prefix.
3. Applies the single-flight guard from D2.
4. Calls `graphWorker.writeFrame(event.data)`. The data argument is
   *transferred*; the WebSocket's internal buffer is gone after this call.
5. On success in SAB mode, returns. The renderer's next `useFrame`
   tick reads new positions.
6. On success in Comlink mode, swaps `nodePositionsRef.current` to the
   returned buffer view.

The handler does not write to Zustand. Position changes are not store
state. The store reflects topology and metadata only.

### D9. Topology / metadata frames

Where the server emits non-position WebSocket messages (graph-data
replacement, settings sync, bot telemetry), those flow through
text-frame handlers that decode JSON and dispatch to the relevant
manager (`graphDataManager`, settings store, bot telemetry handler).
None of these touch the worker. The worker is exclusively for binary
position frames and on-demand edge-length compute.

### D10. Dev overlay surface

A small `useDevOverlay()` hook subscribes to:
- `WORKER_USES_SAB` (constant)
- `graphWorker.getStats()` (polled every 1s in dev only)
- WebSocket frame_id last seen (from single-flight handler)
- Frames dropped counter (from D2's collapse-to-slot behaviour)

The overlay is gated by a dev-mode flag and is not part of the
production bundle. It exists to surface single-flight drops, SAB/Comlink
mode, and server-side gap detection.

## Options considered

### O1. Bring the `main` client forward as-is

Rejected. The `main` client carries `BinaryFrameCoalescer`,
`graphDataLoaded` map-clearing guard, double Comlink round-trip
workarounds, and broad-selector survivor patches. The freeze investigation
established that this configuration is not stable on 4,500-node graphs.

### O2. Replace Comlink with raw `postMessage` + structured clone

Rejected. Structured clone of 140KB per frame at 10Hz is 1.4MB/s of
allocation+copy churn. The transfer-or-SAB discipline (D3) avoids both
the copy and the allocation.

### O3. Move parsing to the main thread, eliminate the worker

Rejected. Parsing on main thread blocks the React render loop during
frame processing. The 4ms typical parse cost compounds with React's
reconciliation budget; under heavy frame load the tab freezes. The
worker exists to keep parse off the main thread.

### O4. Add an explicit queue to the binary-frame handler

Rejected. A queue would deliver every frame eventually, but V3 full-sync
semantics make every intermediate frame redundant when a newer one is
already pending. The slot-based newest-wins design (D2) is both simpler
and more correct.

### O5. Use Zustand's `subscribeWithSelector` middleware globally

Rejected as not sufficient. The middleware enables narrow subscriptions
but does not enforce them. The lint rule (D4) is what prevents future
regression. The middleware is enabled — D4 layers the enforcement on top.

### O6. Single source-of-truth in the worker

Rejected. Putting the Zustand store inside the worker would require
React to subscribe across the worker boundary; current React+Zustand
does not support this without a synchronization layer that would
itself be a significant feature. Worker-owned parse + main-owned store
is the right split.

## Risks

- **R1**: The `no-broad-zustand-selector` lint rule may produce false
  positives on legitimate uses (e.g. returning a stable
  `useShallow`-wrapped object). Mitigation: rule supports allowlist
  comments (`// zustand-broad-selector-allowed: reason`); reviewer
  signs off on each.
- **R2**: SAB availability is a deployment concern. Pages served without
  COOP/COEP headers silently fall back to Comlink. Mitigation: ADR-09
  specifies the headers as part of the deploy contract; dev overlay
  surfaces the mode so a regression is visible.
- **R3**: The single-flight slot means an arbitrarily slow main thread
  can drop most frames. Under V3 full-sync this is correct (latest wins),
  but if frame_ids are monotonically increasing and the gap is large, the
  user sees a "jump" instead of motion. Mitigation: D2 plus ADR-02's max
  10Hz cadence under ACTIVE makes the worst-case gap small; dev overlay
  surfaces the dropped-frames counter so the regression is visible.
- **R4**: Identity short-circuit in D6 relies on `graphDataManager` not
  producing a fresh reference per delivery. The current implementation
  on baseline holds the reference stably; future contributors must not
  break this. Mitigation: a small unit test asserts that two consecutive
  `refresh()` calls with identical server response produce the same
  reference.
- **R5**: The Comlink fallback path is exercised rarely (only when
  `crossOriginIsolated` is false). It risks bit-rot. Mitigation: a
  dev-mode env variable `VITE_FORCE_COMLINK=1` forces the fallback
  path so developers exercise it during normal development.

## Rejected from main as buggy / unjustified

- **`348d23c62` / `17c0f913a` (BinaryFrameCoalescer drain loop)**. The
  coalescer existed to manage frame bursts; ADR-02's settlement-gated
  cadence caps bursts server-side. The class is removed entirely (D2's
  single-flight slot replaces it). Bringing it forward is not justified
  from first principles — it solves a problem that no longer exists.

- **`695be6d6d` / `4fd7e1f9c` (graphDataLoaded map-clearing guard, then
  its revert)**. The guard tried to prevent a race where binary frames
  arrived before `graphDataManager` had populated the node map.
  Symptom-level fix. The architectural fix is D8: position frames don't
  touch the node map at all — they write SAB/transferred buffers. The
  race the guard addresses does not exist under D8. Neither the guard
  nor its revert is migrated.

- **`c09f8725a` (freeze fix at 1FPS broadcast)**. The 1FPS rate was a
  band-aid. ADR-02 owns the rate decision (settlement-gated, max 10Hz).
  The client-side piece (newest-wins single flight) is here as D2. The
  1FPS magic number is not migrated.

- **Double Comlink round-trip removed in `4c126cffc`**. The fix itself
  is correct in intent and adopted as part of D7's surface contract (the
  proxy returns the buffer in the same call). The earlier
  `worker.parse(); worker.getPositions()` pattern is rejected outright —
  it does not appear in the new proxy surface, so it cannot regress.

- **Quadruple delivery in `graphDataManager`**. Adopted as a problem to
  fix, not as code to bring forward. D5 specifies a single delivery
  path; the existing four-path implementation on main is replaced
  wholesale.

- **Broad Zustand selectors narrowed in `61968e3d0`**. The narrowing is
  adopted in intent (D4). The specific selectors changed in that commit
  are reference for the migration sweep, but D4's lint rule is the
  enforcement mechanism going forward, not a one-off audit.

## Bugs and smells at the reset point (41979d33e)

To flag for migration awareness, not for rollback baseline:

- The baseline does not yet have `WORKER_USES_SAB` capability detection
  exposed as a constant; SAB availability is checked ad-hoc at
  callsites. Address as part of D3 implementation.
- The baseline has at least one component (`GraphCanvas.tsx` at
  baseline) that subscribes to a broad selector. Sweep these as part of
  D4 implementation; the lint rule will surface them all in one pass.
- The baseline `graphDataManager` predates the `lastGraphData` field;
  the cache is implicit and per-subscriber. D5 introduces the explicit
  field and the microtask dispatch.
- The baseline binary-frame handler does not have a `_binaryFrameInFlight`
  flag; multiple frames in flight is possible. D2 introduces the flag and
  the newest-wins slot.
- `graphWorkerProxy.ts` at baseline may export a wider surface than D7
  permits. Trim to the D7 contract during migration; anything else is
  dead weight or backdoor access to worker internals.
- The baseline `useFrame` in `GraphManager.tsx` reads SAB on every frame
  unconditionally. This is correct and stays — D6's short-circuit
  applies to topology rebuild, not to per-frame position read.
- Two graph-consumer components exist at baseline (`GraphCanvas.tsx`,
  `GraphViewport.tsx`). Both must read SAB from the same `nodePositionsRef`
  and must both go through `graphWorkerProxy`. There is one worker for
  the page, not one per consumer.

## Cross-references

- ADR-02 D2 sets the cadence this design relies on. If the server ever
  exceeds 10Hz under ACTIVE, D2's slot will drop frames silently and the
  dev overlay will show the loss; the fix is server-side, not here.
- ADR-04 (Rendering) consumes `nodePositionsRef` and the
  `WORKER_USES_SAB` constant. Renderer logic is not duplicated here.
- ADR-06 (Auth) governs `?skipAuth=true` and Nostr token presentation.
  This ADR does not specify how the WebSocket URL is constructed; it
  only specifies what happens once the binary frame arrives.
- The `useDevOverlay()` hook (D10) is the single observability surface
  for this section. It is not a public API; it is dev-mode only.
