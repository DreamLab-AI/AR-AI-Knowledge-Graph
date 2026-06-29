# PRD-04 — Rendering Layer

Status   : Proposed
Date     : 2026-05-16
Owner    : anthropic@xrsystems.uk
Related  : ADR-04 (this section), ADR-02 (Broadcast → position frames),
           ADR-03 (Client state → SAB / nodePositionsRef), ADR-01 (Physics →
           settled vs active), ADR-05 (Settings → visual modes, edge radius,
           gem material, environment).

## Capability statement

The rendering layer converts the authoritative client-side node and edge
state into a coherent 3D scene under React Three Fiber + three.js r177. It
delivers three node visual classes (knowledge, ontology, agent), a single
edge visual class with per-instance categorisation colour, glyph labels,
ambient scene effects, and a PBR-suitable environment — all at interactive
rates for graphs in the multi-thousand node range, on both real GPUs and
software-rasterised WebGL fallbacks.

The layer is not where node positions are computed (Section 1 / 2) nor where
they are stored (Section 3). It only reads position state — predominantly
through a `SharedArrayBuffer`-backed Float32 view (`nodePositionsRef`) —
and writes it into GPU instance buffers each frame.

## Why this matters

The freeze regression that triggered this sprint had two surfaces. The
server-side surface (broadcast cadence, delta filter) is owned by ADR-02.
The client-side surface (work per frame, allocations per frame, fallback
rendering paths) is partly owned here. Specifically:

- The `<Environment resolution={256}>` HDR generation is *suspected* as a
  freeze contributor under software WebGL, where the PMREM cube-render is
  CPU-bound and can take many seconds.
- `MAX_EDGES = 10_000` was bumped on `main` to `16_000` (commit
  `d1f7f2548`) without addressing the underlying problem: the cap is a
  hardcoded constant, not a capacity that adapts to the loaded graph.
- The wrapper-prop-forwarding pattern in `InstancedLabelsWebGL` is a
  silent footgun: a missing prop causes labels to fall back to stale data
  rather than fail loudly.
- `WasmSceneEffects` introduces a second route into the scene (particles,
  wisps, atmosphere) whose perf characteristics interact with the
  primary geometry passes in ways the baseline does not measure.

These are not handled by reverting to baseline. They are handled by
re-specifying the rendering capability — what it must deliver, where its
boundaries are, what it is not allowed to do.

## Functional requirements

### F1. Three node visual classes

Each class is a single instanced mesh with one geometry and one material:

- **GemNodes** (knowledge). Geometry: Icosahedron, base radius `0.5`.
  Material: PBR glass with transmission, ior, roughness — `GemNodeMaterial`.
- **CrystalOrb** (ontology). Geometry: Sphere, base radius `0.5`. Material:
  PBR glass tuned for higher transmission, lower roughness —
  `CrystalOrbMaterial`.
- **AgentCapsule** (agents). Geometry: Capsule, radius `0.3`, height `0.6`.
  Material: emissive-tinted opaque — `AgentCapsuleMaterial`.

Each class instance is positioned, scaled (`computeNodeScale * nodeSize`),
and tinted from a per-node colour buffer. Class membership is determined
by the node's type bits. See ADR-08 §D6.

### F2. Single edge visual class with per-instance category colour

`GlassEdges` is an `InstancedMesh` of unit-height cylinders. Each instance:

- is placed at the midpoint of the (source, target) pair,
- is scaled along its local Y axis to the inter-node distance,
- is rotated by the quaternion that maps the local Y axis to the
  normalised src→tgt vector,
- is shortened at each end by the source/target radii so the cylinder
  visually connects geometry *surfaces*, not centres.

The per-instance colour buffer encodes edge category (knowledge edge,
namespace edge, ontology relation, agent communication). The base material
colour is held at white when per-instance colours are active so the
multiply does not tint the categories.

### F3. Edge capacity is configured, not magic

`MAX_EDGES` ceases to exist as a top-level `const`. Edge capacity is:

- **Initialised** to `min(currentEdgeCount * growth_factor, ceiling)` at
  first non-empty `points` prop.
- **Grown** when `currentEdgeCount > current_capacity` by reallocating
  the `InstancedMesh` and its `instanceColor` buffer to
  `max(currentEdgeCount, current_capacity * 2)`, up to `ceiling`.
- **Ceiling** is read from settings (`rendering.maxEdgesCeiling`,
  default `64_000`), surfaced in the control panel, persisted.

The reallocation path is the only place `new THREE.InstancedMesh(...)`
runs after first mount. It is the only place that disposes the previous
mesh. There is no other code path that recreates the edge mesh.

### F4. Progressive edge reveal

On mount and on each `points` prop change, edges are revealed over
multiple frames to avoid an initial-mount cost spike when graphs with
many edges first load. The reveal rate is tied to the node reveal batch
(`revealBatch`, default `120`) scaled by `0.67`, with a configured
minimum and maximum. Once `edgeRevealRef >= totalEdgesRef`, the reveal
finishes and the mesh count is pinned to total.

Reveal does not interact with capacity growth (F3): capacity is sized
for the *final* count before reveal begins; reveal only controls
`mesh.count`.

### F5. Labels with two-phase useFrame

`InstancedLabels` (the typed wrapper) and `InstancedLabelsWebGL` (the
renderer) render glyph labels for visible nodes:

- **Every frame** (Phase 1, cheap): patch `aLabelPos` instance positions
  from the SAB `nodePositionsRef`. No allocations. No layout.
- **Every 3 frames** (Phase 2, expensive): perform frustum cull, then
  rebuild the full per-glyph layout via `layoutTextInline()` writing
  directly to `InstancedBufferAttribute` arrays. No `GlyphInstance[]`
  allocation per node.

Both phases capture `nodePositionsRef?.current` once at the top of
`useFrame` and reuse the same value through both branches. Both phases
read the same `nodeIdToIndexMap`.

### F6. Wrapper prop forwarding is statically enforced

`InstancedLabelsWebGL` (and any future per-renderer wrapper) consumes the
same prop interface as `InstancedLabels`. Every prop on the typed parent
is forwarded explicitly; the wrapper's prop type is derived from the
parent's prop type (`type WebGLProps = InstancedLabelsProps;`) so the
TypeScript compiler rejects a missing-prop case at the call site.

`nodePositionsRef`, in particular, must always reach the renderer. The
renderer must not silently fall back to `labelPositionsRef` when
`nodePositionsRef` is absent at runtime; if it ever is, that is a bug
upstream and the renderer logs once at warn level.

### F7. WasmSceneEffects

`WasmSceneEffects` provides scene-level visual effects (background
particles, wisps, atmosphere fog) backed by a Rust crate compiled to
WebAssembly at `client/crates/scene-effects/`. The contract:

- Buffers are zero-copy: WASM exposes `get_*_ptr()` and `get_*_len()`,
  the bridge constructs `Float32Array` views over
  `WebAssembly.Memory.buffer`. No per-frame copy.
- Effect counts and intensities are driven by `sceneEffects.*` settings,
  with documented defaults (`particleCount=256`, `wispCount=48`).
- All three effects are individually toggleable; turning all of them off
  removes the component subtree entirely (no idle work).
- The component is mounted **inside** the Canvas, after the geometry
  passes, so its uniforms can reference the same camera that renders
  nodes and edges.

The WASM module is a build-time artefact; its source crate is in-tree
but is not re-compiled per browser load. The compiled `.wasm` is
versioned alongside the client bundle.

### F8. Environment with software-rendering fallback

`<Environment resolution={256}>` is used to provide an HDR environment
map for PBR transmission on `GemNodes` and `CrystalOrb`. The render path
must:

- Detect at startup whether WebGL is software-rasterised
  (`UNMASKED_RENDERER_WEBGL` containing strings such as `swiftshader`,
  `llvmpipe`, `software`).
- On hardware GL: render `<Environment>` normally with the configured
  resolution.
- On software GL: replace `<Environment>` with one of two fallbacks:
  - **F8a**: a low-resolution static cube map loaded once via
    `CubeTextureLoader` and assigned to `scene.environment`.
  - **F8b**: no environment at all; gem materials fall back to a
    non-transmission preset.

The choice between F8a and F8b is a setting
(`rendering.softwareFallback`, default `static-cube`). The runtime
detection and the resulting decision are logged once at info level so
that "why is glass not refracting" is answerable from console.

### F9. Surface-to-surface edge offset

Edges visually touch the geometry of their endpoints, not their centres.
Source and target radii are computed as
`computeNodeScale(node, ...) * nodeSize`. The cylinder is shortened by
`(srcR + tgtR)` along the connection axis, and its midpoint is shifted
to be the midpoint of the shortened span.

For all three node geometries (Icosahedron r=0.5, Sphere r=0.5, Capsule
r=0.3 h=0.6), the same offset formula applies. The capsule's
non-spherical envelope is approximated by `0.3` for the offset; visual
mismatch at the capsule's flat-ends is acceptable and is not corrected.

### F10. No frame allocations

The rendering layer must not allocate per frame in any of:
node-pose update, edge-matrix update, label-position update, label-layout
rebuild, scene-effects buffer view construction. Pre-allocated typed
arrays and `InstancedBufferAttribute` are reused across frames. The
allocator pressure target is "no measurable major GC during a 60s
panning session on a 5k-node graph."

## Acceptance criteria

- **A1**: A 5,000-node, 20,000-edge graph mounts without an initial-frame
  freeze; reveal completes within ~3 seconds.
- **A2**: Edge capacity growth (`current_capacity` doubling up to
  `ceiling`) is observable as a single allocation event in the
  performance profile, not as continuous reallocation.
- **A3**: `MAX_EDGES` is not present as a top-level constant in any
  rendering source file. Capacity is read from settings.
- **A4**: All three node classes render with the geometry and material
  specified in F1. Per-class instance counts match Section 8's classifier.
- **A5**: Per-instance edge colours match category. Switching a node's
  type at runtime is reflected on the next layout rebuild without a full
  mesh recreate.
- **A6**: Labels track node positions every frame; the every-3-frame
  layout rebuild produces no visible "label snap" because Phase 1 keeps
  positions current.
- **A7**: Removing the `nodePositionsRef` prop from an
  `InstancedLabelsWebGL` call site is a TypeScript compile error,
  not a runtime degradation.
- **A8**: On a software WebGL context, the page reaches steady-state
  render within 5 seconds of mount. The Environment fallback decision
  is logged.
- **A9**: `WasmSceneEffects` with all three effects enabled adds no
  measurable per-frame allocation (heap snapshot delta < 64KB over 1000
  frames).
- **A10**: Edges visually touch node surfaces, not centres, for all three
  node classes.

## Non-goals

- **N1**: Custom WebGPU renderer. Three.js r177 with WebGL2 / WebGL2
  software is the target. WebGPU is a future ADR with its own implications
  for material compilation.
- **N2**: Per-edge animated shaders beyond what `GlassEdgeMaterial`
  already provides. Edge-flow, ribbons, particle-rails are scope creep.
- **N3**: Post-processing pipeline overhaul. `GemPostProcessing` is kept
  as-is from baseline; this section neither extends nor reworks it.
- **N4**: Label collision avoidance / hierarchical declutter. Labels
  show one per visible node, frustum-culled. Smarter declutter is a
  separate workstream.
- **N5**: XR-specific rendering paths. Section 12 owns the Godot XR
  client; the web renderer described here is not XR-aware beyond what
  R3F gives for free.
- **N6**: Re-evaluating the choice of R3F. The framework is fixed.

## Dependencies

- Section 1 (Physics) — provides the node position stream and the
  settled/active state events that this section consumes indirectly via
  Section 2.
- Section 2 (Binary protocol) — defines the V3 frame format from which
  `nodePositionsRef` is populated.
- Section 3 (Client state) — owns `nodePositionsRef` lifecycle, SAB
  allocation, single-flight discipline. This section is a *reader*, not
  an owner.
- Section 5 (Settings) — exposes `rendering.maxEdgesCeiling`,
  `rendering.softwareFallback`, `revealBatch`, `gemMaterial.*`,
  `sceneEffects.*`.
- Section 8 (Ontology / KG data) — determines node class membership
  (knowledge / ontology / agent) via type bits.

## Migration approach

1. **Baseline already has GemNodes, CrystalOrb, AgentCapsule,
   GlassEdges, InstancedLabels(WebGL), WasmSceneEffects** (per file
   inventory at `41979d33e`). Carry these forward as the structural
   spine.
2. **Reject the `d1f7f2548` MAX_EDGES bump as a symptom-level fix.**
   Replace the constant with the F3 configurable-capacity scheme. The
   bump is unnecessary once capacity is dynamic.
3. **Add software-WebGL detection to `GraphCanvas.tsx`** (F8). Branch
   on the detected renderer string at Canvas init.
4. **Tighten the InstancedLabels wrapper prop type** so missing
   `nodePositionsRef` is a compile error.
5. **Audit `WasmSceneEffects` for per-frame allocation** (A9) and fix
   any bridge-side `new Float32Array(...)` in `useFrame`.
6. **Surface settings** (Section 5 dependency).

## Open questions resolved here

- *Should we drop Environment entirely?* No. F8 keeps it on hardware GL
  where it is cheap and adds real material fidelity; F8 only sheds it
  on software where it is the suspected freeze source.
- *Should edge capacity ever shrink?* No. Once grown to a count, the
  capacity stays. Memory cost is bounded by `ceiling` and is small
  relative to total scene memory.
- *Should reveal be settling-aware?* Tempting (reveal faster when
  physics is settled, slower when active). Rejected for this section as
  premature; revisit if profiling shows reveal stealing budget from
  the layout rebuild during active settling.
