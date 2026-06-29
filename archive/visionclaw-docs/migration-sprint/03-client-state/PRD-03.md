# PRD-03 — Client State & Workers

Status   : Proposed
Date     : 2026-05-16
Related  : ADR-03 (this section), ADR-02 (Binary Protocol), ADR-04 (Rendering)

## Capability statement

The client maintains a single, coherent view of graph state — nodes, edges,
positions, and selection — across the main thread, the layout worker, and the
renderer, without redundant copies, redundant re-renders, or redundant
deliveries. Position data flows from the WebSocket into the canvas under a
single-flight discipline. Topology data flows from REST into Zustand and is
short-circuited on identical re-deliveries. Selectors are narrow enough that
unrelated state changes never cascade into renderer rebuilds.

## Why this exists

The freeze regression that triggered this sprint had two architectural
contributors on the server (ADR-01, ADR-02) and three on the client:

1. **Broad Zustand selectors.** A handful of components subscribed to whole
   slices (`useGraphStore(s => s.graph)`) rather than individual fields.
   When binary frames arrived at up to 60 Hz, every position update touched
   the slice reference, every subscriber re-rendered, and React's render
   loop drowned the worker postMessage queue.

2. **Double Comlink round-trip.** The binary frame handler did one Comlink
   call to deliver the buffer to the worker for parsing, then a second
   Comlink call to retrieve the parsed positions back to main thread. With
   structured clone, the same data crossed the worker boundary twice per
   frame. Under load this exhibited as multi-hundred-ms stalls.

3. **Quadruple delivery to subscribers.** `graphDataManager` invoked its
   subscribers from four code paths (REST result, websocket "graph-data"
   message, retry handler, manual refresh). Each subscriber rebuilt edge
   buffers and reconciled Zustand. The freeze investigation traced 4×
   delivery of identical payloads as a steady-state cost.

Each defect was patched on `main` with progressively defensive guards, but
the guards encode the bug into the architecture. This PRD specifies the
capability the system needs, and the companion ADR specifies the design
that delivers it without the guards.

## Users and use-cases

### U1. Operator viewing a settled knowledge graph (~4,500 nodes)

The user opens the page; the worker receives initial positions; the canvas
renders; the system goes quiet. No frames flow once layout settles. The
user pans, zooms, selects — operations interact with the renderer's local
state, not the graph store. The tab remains responsive for hours.

### U2. Operator triggering a physics reheat

The user changes a physics parameter or pins a node. The server
re-destabilises the layout, frames flow at up to 10 Hz, the worker patches
SAB positions in-place, the renderer reads SAB in `useFrame`. Main thread
React state is not touched by frame arrivals; only the renderer reads SAB.

### U3. Reconnect after WebSocket drop

The connection drops; positions stop updating; on reconnect the server
sends one full V3 frame; the client treats this as authoritative and
overwrites SAB. No reconciliation is attempted; full-sync semantics
eliminate divergence.

### U4. Cross-origin-isolated environment unavailable

The page loads under a context that does not satisfy `crossOriginIsolated`
(some embedded contexts, browser policy edge cases). SAB is unavailable.
The client falls back to Comlink with explicit `transfer()` zero-copy
delivery of `ArrayBuffer` payloads. The renderer reads from a TypedArray
in main-thread memory instead of SAB. Capability is preserved; performance
degrades gracefully.

### U5. Developer running the page in dev mode with hot-reload

The worker module is rebuilt; Comlink proxy is re-acquired; in-flight
frames are dropped (single-flight semantics make this safe); the next
frame restores steady state. No persistent state leaks across reloads.

## Acceptance criteria

### A1. Single-flight binary frame processing

At any instant the client is processing at most one binary frame. When a
frame arrives while processing is in progress, the new frame replaces the
previous one (newest-wins). The implementation uses a single
`_binaryFrameInFlight` guard owned by the websocket handler. The guard is
released in a `finally` block. There is no queue, no retry, no buffer of
pending frames.

### A2. Zero-copy delivery to worker

When SAB is available (`crossOriginIsolated === true`), positions are
written directly into a `Float32Array` view over a shared buffer. The
worker reads the same buffer; no copy occurs. When SAB is unavailable,
positions are delivered via `Comlink.transfer(buffer, [buffer])`; the
buffer ownership transfers, no structured clone occurs. Under no
circumstance does a frame buffer cross the worker boundary by structured
clone.

### A3. No double Comlink round-trip

The main thread does not request parsed data back from the worker. The
worker either writes SAB (visible to main without re-call) or returns its
result in the same `transfer()` round-trip. The pattern
`worker.parse(buf); worker.getPositions()` is rejected; it must be
`positions = await worker.parse(transferable_buf)` at most.

### A4. Narrow Zustand selectors only

Every consumer of `useGraphStore` selects a primitive field or a stable
reference (not a synthesised object). The lint rule
`no-broad-zustand-selector` enforces this at build time. Specifically:

- `useGraphStore(s => s.nodeCount)` — allowed
- `useGraphStore(s => s.nodeIds)` — allowed if `nodeIds` is a stable
  reference managed by the store
- `useGraphStore(s => ({ count: s.nodeCount, ids: s.nodeIds }))` — rejected
- `useGraphStore(s => s)` — rejected
- `useGraphStore(s => s.graph)` — rejected if `graph` is a composite slice
  rather than a primitive

### A5. Subscription dedup

`graphDataManager` exposes a single `subscribe(callback)` API. Callbacks
are invoked from exactly one code path per data change. The four-path
delivery on `main` is reduced to one: data arrives, manager updates its
`lastGraphData` field, manager schedules one `queueMicrotask` notification.
Identity comparison on `lastGraphData` short-circuits redundant updates.

### A6. GraphManager identity short-circuit

`GraphManager.tsx` keeps a `lastProcessedGraphRef` and a `lastShapeRef`.
On every render, before rebuilding edge buffers or instanced meshes, it
compares incoming graph identity (reference equality) and topology shape
(nodeCount + edgeCount + a topology hash). If both match, the rebuild is
skipped. A graph whose positions update via SAB but whose topology is
unchanged triggers zero rebuilds.

### A7. Worker owns parsing, not state

`graphWorkerProxy.ts` is the only main-thread surface that talks to the
worker. The worker owns: binary frame parsing, position-to-SAB writes,
edge length computation for layout feedback. The main thread owns:
Zustand store, REST orchestration, subscription dispatch,
`graphDataManager` caching, React rendering. There is no shared mutable
JavaScript state between threads; only SAB or transferred buffers cross
the boundary.

### A8. Capability detection at startup

On module load, `graphWorkerProxy` checks `typeof SharedArrayBuffer ===
'function' && self.crossOriginIsolated === true`. The result is captured
in a constant and exposed as `WORKER_USES_SAB`. Components that need
to know the mode (status overlay, dev panel) read this constant; no
runtime branching beyond the initial detection. If detection changes
between page loads (e.g. server now serves COOP/COEP headers), the new
mode takes effect on next load.

### A9. No client-side coalescer

The binary frame coalescer (`BinaryFrameCoalescer` class on `main`) is
not migrated. ADR-02 governs cadence at the server (max 10 Hz under
ACTIVE state, heartbeat-only under SETTLED). The client processes frames
1:1 as they arrive. If the server ever exceeds the agreed cadence, the
client's single-flight guard naturally drops frames; this is a server bug
to be fixed at the broadcast actor, not papered over on the client.

### A10. Graceful handling of zero-frame intervals

Under SETTLED state the client may receive zero frames for 5s+. The
canvas continues to render from SAB (or the last delivered Comlink
buffer). No timeout fires; no "connection stale" warning; the heartbeat
arrives within the bound and confirms liveness. Connection-lost detection
uses WebSocket close events, not frame timeouts.

### A11. Reconnect semantics

On reconnect the next V3 frame is treated as authoritative. The single
buffer (SAB or main-thread) is overwritten. No position interpolation
between old and new state. No reconciliation of "which nodes moved while
disconnected". The full-sync wire format makes this safe by design.

## Non-goals

- **Position prediction / dead reckoning.** Out of scope. The protocol
  carries authoritative positions; the client renders what it receives.
- **Client-side physics.** Out of scope. Layout is GPU-side per ADR-01.
  The worker performs no integration step.
- **Offline mode / state persistence across reload.** Out of scope. On
  reload the client re-fetches REST and re-subscribes; there is no
  IndexedDB cache.
- **Selection state in the graph store.** Selection lives in a separate
  `useSelectionStore` (see Section 4) precisely so that selection changes
  don't trigger graph-store consumers.
- **Backpressure metric in the user-facing UI.** A counter is exposed in
  the dev overlay only. Production users do not see "frames dropped".

## Out-of-scope smells flagged

The following appear in the existing `main` codebase. They are flagged
here as not in scope for Section 3, with a pointer to the section that
owns them:

- **Three-way settings store** (Zustand + REST + WebSocket sync of
  settings). Owned by ADR-05. This PRD treats settings as opaque.
- **Bot telemetry frame routing.** Owned by ADR-07. The worker proxy
  forwards `bot-telemetry` messages to a separate handler; this PRD
  does not specify the handler.
- **XR client state bridge.** Owned by ADR-12. The Godot client has its
  own state model; this PRD covers the React/Three.js client only.
- **`AgentControlSurface` panel.** Lives in agentbox + external forum
  (per README ground rule 6). Not a VisionClaw client concern.
- **`drainLoopCoalescer.tick()`** invoked from `useFrame`. Symptom of
  the `BinaryFrameCoalescer` that A9 rejects. Removed entirely.

## Definition of done

- All A-criteria satisfied with code grounded in the ADR-03 design.
- Lint rule for A4 in place and passing on the migrated client tree.
- Manual test: open the page on a 4,500-node knowledge graph, leave for
  30 minutes under SETTLED state, confirm tab remains responsive
  (interaction latency under 50ms, no GC pause exceeding 200ms in the
  performance profile).
- Manual test: trigger a physics reheat via control panel; confirm
  positions update smoothly and the React DevTools renders panel shows
  no cascade re-renders of unaffected components.
- Manual test: disable COOP/COEP headers; confirm the page falls back to
  Comlink transfer mode and remains functional (acceptance is functional,
  not performance-equivalent).
- Worker proxy passes a structural test that asserts the proxy surface
  matches the contract enumerated in ADR-03 D7.

## Risks acknowledged here (detailed in ADR-03)

- The lint rule for narrow selectors is a static analysis; some legitimate
  patterns may need explicit allowlist entries.
- SAB availability depends on COOP/COEP server headers; these are part of
  the deploy contract (Section 9) and any misconfiguration silently
  degrades to Comlink mode.
- The single-flight guard's newest-wins behaviour means a slow main
  thread will occasionally skip rendering an intermediate frame; this is
  intentional but should be visible in the dev overlay.
