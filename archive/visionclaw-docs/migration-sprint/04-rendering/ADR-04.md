# ADR-04 — Rendering Layer

Status      : Proposed
Date        : 2026-05-16
Supersedes  : —
Related     : ADR-01 (Physics), ADR-02 (Broadcast), ADR-03 (Client State),
              ADR-05 (Settings), PRD-04 (capability).

## Context

The rendering layer at baseline `41979d33e` already contains the structural
pieces: three node classes (`GemNodes`, `CrystalOrb`, `AgentCapsule`),
`GlassEdges`, `InstancedLabels` with a `InstancedLabelsWebGL` wrapper,
`WasmSceneEffects`, and an `<Environment>` in `GraphCanvas.tsx`. Between
baseline and `main`, the churn around this layer is dominated by:

- A `MAX_EDGES` constant bumped from `10_000` to `16_000` (commit
  `d1f7f2548`) because graphs at the upper end were silently truncating
  edges. The bump is symptom-level: the next graph above 16k will silently
  truncate again.
- A latent wrapper-prop-forwarding bug in the label renderer. When
  `InstancedLabelsWebGL` was extracted from `InstancedLabels`, a missing
  `nodePositionsRef` forward caused labels to fall back to the older
  `labelPositionsRef`, producing positions that lagged behind the SAB by
  many frames. Detected by inspection, not by tests.
- The freeze regression. Investigation strongly suggests
  `<Environment resolution={256}>` PMREM generation is a freeze
  contributor on software WebGL contexts (where each cube face is
  CPU-rendered), independent of the Section-2/Section-3 surfaces.
- `WasmSceneEffects` exists at baseline but its allocation discipline
  inside the per-frame bridge has not been audited; the zero-copy
  promise in the architecture notes is not enforced by anything.

This ADR resolves these as a set of design decisions, not as patches.

## Decision

### D1. Edge capacity is dynamic and configured, never magic

`GlassEdges` removes the top-level `MAX_EDGES = 10_000` constant. Capacity
is a runtime value owned by the component:

- On first mount with a non-empty `points` prop, capacity is sized to
  `min(ceil_to_power_of_two(currentEdgeCount * 1.25), ceiling)`.
- Subsequent `points` props that exceed capacity trigger one
  reallocation event: a new `InstancedMesh` is constructed at
  `min(max(currentEdgeCount, current_capacity * 2), ceiling)`, the
  previous mesh is disposed (geometry + material + instanceColor),
  the parent `<primitive>` is reattached.
- `ceiling` is the setting `rendering.maxEdgesCeiling` (default
  `64_000`). When the loaded graph exceeds the ceiling, the renderer
  draws `ceiling` edges and logs a structured warning naming the
  unrendered count. There is no silent truncation.

This eliminates the `d1f7f2548` class of "bump the number" commits. The
ceiling is the only knob, and it surfaces through settings, not source.

### D2. Edge geometry composition is explicit and surface-aware

For each edge `i`, the per-instance matrix is composed as:

```
src = vec3(positions[6i+0], positions[6i+1], positions[6i+2])
tgt = vec3(positions[6i+3], positions[6i+4], positions[6i+5])
delta = tgt - src
len = |delta|
dir = delta / len
srcR = computeNodeScale(srcNode, ...) * nodeSize
tgtR = computeNodeScale(tgtNode, ...) * nodeSize
adjLen = max(0, len - srcR - tgtR)
midpoint = src + dir * (srcR + adjLen * 0.5)
q = quaternionFromUnitVectors(yAxis, dir)
matrix = compose(midpoint, q, vec3(1, adjLen, 1))
```

The cylinder geometry is `CylinderGeometry(radius, radius, 1, ...)` —
unit height. `radius` is `settings.edgeRadius` (default `0.03`). The
Y-scale carries the adjusted length. The quaternion is
`setFromUnitVectors(THREE.Object3D.DEFAULT_UP, dir)`. This is the only
matrix composition path; no other variant lives in the codebase.

When `adjLen <= 0` (overlapping nodes), the instance is collapsed
(matrix scaled to zero in all axes). Counted in a metric; not drawn.

### D3. Progressive edge reveal is bounded and decoupled from capacity

Reveal animates `mesh.count` from `0` to `totalEdgesRef` over multiple
frames. Per frame, `edgeRevealRef += EDGE_REVEAL_BATCH`, where
`EDGE_REVEAL_BATCH = max(1, round(settings.revealBatch * 0.67))`. Reveal
stops at `totalEdgesRef`.

Capacity (D1) is independent: capacity is sized to the *final* count
before reveal begins. Reveal only modulates the draw count. This split
prevents the "reveal triggers reallocation triggers reveal restart"
class of pathology.

### D4. Per-instance edge colour buffer is owned by the mesh

`GlassEdges` pre-allocates `instanceColor` as a
`Float32Array(capacity * 3)` initialised to white (so multiply against
material base colour is neutral). Colour writes come from the per-edge
category classifier:

- Knowledge edge → category-specific colour.
- Namespace edge (`--` prefix derived) → category-specific colour.
- Ontology relation (SUBCLASS_OF, etc.) → category-specific colour.
- Agent communication → category-specific colour.

On capacity growth (D1), the new `instanceColor` is constructed at the
new capacity, prior values copied, and the buffer attribute set on the
new mesh. There is no path where instance colours desync from the
mesh instance count.

When per-instance colours are active, `material.color` is held at
`#ffffff` so the multiply does not tint the categories. The "base
colour" setting then exists only for the no-categories fallback.

### D5. Environment with explicit software-fallback path

`GraphCanvas` detects software WebGL at Canvas-creation time via the
unmasked-renderer extension (`WEBGL_debug_renderer_info`,
`UNMASKED_RENDERER_WEBGL`). The detection happens once per session and is
cached.

Branching:

- **Hardware GL**: `<Environment background={false} resolution={256}>`
  with the existing in-shell `<color>` child renders as today. PBR
  transmission on gem / crystal materials uses the generated cube.
- **Software GL** (`swiftshader`, `llvmpipe`, `software`,
  `microsoft basic render driver`):
  - If `rendering.softwareFallback === "static-cube"` (default):
    `CubeTextureLoader` loads a 6×64×64 baked cube once and assigns
    `scene.environment`. No per-frame PMREM cost.
  - If `rendering.softwareFallback === "off"`: no environment is
    assigned. Materials use `envMapIntensity = 0` and a non-transmission
    preset (`transmission = 0`, `roughness ≥ 0.4`).

The detection result, the chosen fallback, and the reason are all
logged once at info level. There is no UI nag; the rendering remains
usable in all three cases.

### D6. Labels: two-phase useFrame, captured once, no allocations

The `InstancedLabelsWebGL` renderer runs one `useFrame`:

```
const positions = nodePositionsRef?.current;  // captured ONCE per frame
if (!positions) { return; }
frameCounter += 1;

// Phase 1: every frame — patch positions only
patchLabelPositionsFromSAB(positions, nodeIdToIndexMap, aLabelPos);

// Phase 2: every 3 frames — full layout rebuild
if (frameCounter % 3 === 0) {
  cullToFrustum(camera, positions, nodeIdToIndexMap, visibleIndices);
  rebuildGlyphLayout(visibleIndices, layoutTextInline, instanceAttrs);
}
```

`patchLabelPositionsFromSAB` and `rebuildGlyphLayout` are zero-alloc:
both write directly into pre-allocated `InstancedBufferAttribute`
typed arrays. `visibleIndices` is a pre-allocated `Uint32Array`. No
`GlyphInstance[]` is ever materialised.

### D7. Wrapper prop forwarding is statically guaranteed

`InstancedLabelsWebGL` declares its prop type as
`type Props = InstancedLabelsProps`. The parent `InstancedLabels`
forwards via the spread of a fully-typed object whose type is
`InstancedLabelsProps`:

```tsx
const childProps: InstancedLabelsProps = {
  nodes, nodeIdToIndexMap, nodePositionsRef, labelPositionsRef,
  // ...every field, named.
};
return <InstancedLabelsWebGL {...childProps} />;
```

The named-object pattern is mandatory; a bare `<InstancedLabelsWebGL {...props}>`
without the intermediate typed object is rejected in code review.
Static analysis (tsc strict) flags any missing field.

Inside the renderer, the runtime guard exists only as a developer
sanity check: if `nodePositionsRef` is somehow undefined at runtime
(would imply a deeper bug), the renderer logs a single `warn` and
falls back to `labelPositionsRef`. It does not silently degrade.

### D8. WasmSceneEffects bridge: zero-copy enforced

The bridge module (`scene-effects-bridge.ts`, conceptual) follows a
strict pattern:

- On effect initialisation: call WASM `init(particleCount, wispCount)`,
  retrieve pointers via `get_particle_ptr()`, `get_wisp_ptr()`, etc.,
  construct `Float32Array` views over `WebAssembly.Memory.buffer` once.
- On every frame: call WASM `tick(dt)`. The same views remain valid
  because WASM memory is not grown after init.
- WASM growth (`Memory.grow()`) is forbidden during `tick`. The crate
  asserts; the bridge re-creates the views if growth is detected (a
  recovery path, not a normal one).

The per-frame allocation budget for `WasmSceneEffects` is zero. Heap
snapshot tests in `client/src/__tests__` enforce this within a 64KB
tolerance over 1000 frames.

### D9. Node classes are pure renderers

Each of `GemNodes`, `CrystalOrb`, `AgentCapsule` consumes a filtered
view of nodes (by class flag) and renders a single `InstancedMesh`.
None of the three:

- Computes positions (read from SAB).
- Computes scale (delegated to `computeNodeScale`).
- Owns labels (`InstancedLabels` is mounted alongside, not inside).
- Owns selection state (lives in client state, Section 3).

This discipline is what makes the three classes interchangeable
behind a class index and what makes adding a fourth class (if ever
needed) a localised change.

### D10. No per-frame allocations anywhere in the layer

This is a hard architectural rule. CI enforces via the heap-snapshot
tests in D8 and via lint rules that flag `new Float32Array(` /
`new Uint32Array(` / `new Vector3(` inside any function whose name
starts with `useFrame` or that is passed as the callback to
`useFrame(`.

## Options considered

### O1. Bring the `MAX_EDGES=16_000` bump forward as-is

Rejected. The bump is a symptom fix. The underlying problem (hardcoded
cap, silent truncation) recurs at the next scale boundary. D1's dynamic
capacity with a configured ceiling solves the class.

### O2. Switch to BatchedMesh (three.js BatchedMesh) for edges

Rejected for this sprint. `BatchedMesh` in r177 is stabilising but its
per-instance colour story is less mature than `InstancedMesh.instanceColor`,
and it requires per-edge geometry submission rather than per-edge matrix
composition. The benefit (one draw call vs one) is nil for our use case;
the migration cost is non-trivial. Revisit in a future ADR.

### O3. Remove `<Environment>` entirely on all paths

Rejected. PBR transmission on hardware GL is materially better with a
generated environment than without. Removing it makes Gem / CrystalOrb
look flat on every platform, not just the software path. D5's
detect-and-branch keeps the high-quality path where it works and only
sheds it where it costs.

### O4. Compile all per-frame work to WASM

Rejected as scope creep. The hot paths (matrix composition, label
layout) are O(n) over the visible set, vectorised in JS, and within
their budgets when the no-allocation discipline (D10) holds. Moving
them to WASM would re-introduce the per-frame copy cost that D8 is
specifically structured to avoid.

### O5. The decisions above as a set (this ADR)

Adopted. Each decision is local; together they remove the entire class
of "rendering surprise" bugs without coupling decisions across layers.

## Risks

- **R1**: Dynamic edge capacity (D1) introduces one reallocation event on
  graph load. If reallocation lands during the initial reveal, it can
  cause a single-frame stutter. Mitigation: capacity is sized to the
  final count *before* reveal starts (per D3); reallocation only
  triggers on subsequent `points` props that grow beyond capacity, i.e.
  on graph data updates, not on first mount.

- **R2**: The software-WebGL detection (D5) depends on the
  `WEBGL_debug_renderer_info` extension being available. Some browsers
  / privacy modes block it. Mitigation: if the extension is absent,
  detection returns `unknown`; the renderer defaults to the
  hardware path (the existing behaviour). Users on those browsers may
  see the freeze; the metric "renderer=unknown" surfaces in client
  telemetry so we can size the population.

- **R3**: The zero-alloc invariant (D10) is enforced by tests, but the
  surface is large and a future contributor can break it. Mitigation:
  the lint rule (D10) catches the most common breakages; CI heap-snapshot
  tests catch the rest. A regression is a CI failure, not a runtime
  symptom.

- **R4**: `WasmSceneEffects` zero-copy (D8) assumes WASM memory does not
  grow after init. A future addition (e.g. dynamic particle counts) could
  break this. Mitigation: the bridge detects growth and rebuilds views; a
  WASM-side assert catches accidental growth in dev.

- **R5**: D2's surface-to-surface offset uses scalar radii. For the
  Capsule (r=0.3, h=0.6) the radii under-approximate the envelope along
  the capsule's long axis. Mitigation: accepted as visible-but-acceptable.
  An exact capsule-vs-line offset would require per-instance shader
  work that is not justified for an agent node.

## Rejected from main as buggy / unjustified

- `d1f7f2548 fix: bump MAX_EDGES 10000 → 16000` — symptom-level. Replaced
  by D1 (dynamic capacity + configured ceiling). The commit's underlying
  observation (graphs were silently truncated) is valid; the fix is not.

- Any `main` commit that adds a top-level magic constant for capacity to
  another component (search for `const MAX_*` in the rendering tree). To
  be identified during implementation and replaced with the same
  D1-style pattern.

- Any commit that re-introduces a per-frame `new Float32Array(...)` or
  `new THREE.Vector3()` inside `useFrame` in label or edge code. The
  D10 lint rule flags these on import.

## Bugs and smells at the reset point (`41979d33e`)

To flag for migration awareness:

- `GlassEdges.tsx` at baseline has `const MAX_EDGES = 10_000;` as a
  top-level constant. Replace as part of D1.
- `GlassEdges.tsx` initialises `mesh.count = 0` and triggers the first
  reveal batch inside `useMemo` of `mesh`. This means the reveal animation
  is restarted on every `[]` re-mount. Acceptable, but ensure capacity
  growth (D1) does *not* trigger this `useMemo` to re-run.
- `InstancedLabels.tsx` at baseline uses a `<= 3 frames` cadence guard
  derived from a `frameCount` counter that is incremented inside a
  `useFrame` closure capturing a ref. The discipline is correct; the
  cadence constant is a magic `3`. Surface as a setting
  (`rendering.labelLayoutEvery`, default `3`) for future tuning. Keep
  the default behaviour identical.
- `WasmSceneEffects.tsx` is imported from
  `client/src/features/visualisation/components/WasmSceneEffects.tsx`
  (not the graph components directory). The crate at
  `client/crates/scene-effects/` has compiled artefacts in
  `client/src/wasm/scene-effects/`. The two-directory split is fine;
  document it in the file's header so a future reader knows where the
  Rust source lives.
- `GraphCanvas.tsx` at baseline uses `<Environment resolution={256}>`
  unconditionally and includes a comment explaining the choice of
  generated environment over CDN-hosted HDR. The choice (generated, not
  CDN) is correct (D5 keeps it); the unconditional path is what needs
  the software-fallback branch.
- The wrapper-prop pattern in `InstancedLabels.tsx` is the named-object
  pattern already, but the type annotation on the named object is
  implicit. Tighten to explicit `InstancedLabelsProps` (D7).
