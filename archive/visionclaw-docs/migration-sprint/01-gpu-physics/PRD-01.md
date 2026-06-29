# PRD-01 — GPU Physics & Force Engines

## 1. Capability statement

VisionClaw performs force-directed layout of a knowledge graph (~5k nodes,
~12k edges) and a parallel agent graph (~50–500 nodes) on the GPU at
interactive rates, with multiple selectable force models, settlement-aware
broadcast cadence, and physics parameters that scale to the node-degree
distribution of the data.

## 2. Why this exists

The baseline at `41979d33e` already delivers a working force-directed engine
on CUDA with mass-aware physics, an Octree, and a single force model. The
target on `main` extends this with:

- **ForceAtlas2 LinLog** kernel for better separation of densely-connected
  hubs from peripheral nodes.
- **Five layout engines** selectable at runtime (force-directed, stress
  majorization, hierarchical, circular, geographic).
- **Mass-aware physics** with node-degree-derived mass values so high-degree
  hubs respond more sluggishly to spring forces and visually anchor the
  layout.
- **Cell-bounds AABB kernel** for spatial partitioning with the launch
  parameter fix (`num_nodes` blocks, not `grid_cells`).
- **NaN / Inf velocity gate** with grid sentinel for numerical safety.
- **Boundary reflect** for nodes hitting the simulation volume edge.
- **Asymmetric split** of force kernels for separate handling of attractive
  vs repulsive components.
- **Degree-weighted gravity** replacement (with the inversion fix from
  commit `9ffd74b99` and gravity-undo from `364e650b3`).
- **GPU self-init** in `ForceComputeActor` to survive cross-thread supervisor
  context replacement.
- **`catch_unwind` on physics panics** to keep the actor alive after a CUDA
  panic, with automatic recovery.
- **Periodic full broadcast** every 300 iterations to recover stuck clients
  whose nodes have converged to zero velocity (BroadcastOptimizer delta
  filter would otherwise withhold their positions forever).

## 3. Users and use cases

- **Knowledge worker** browsing the public Logseq graph (~5k nodes). Expects
  the layout to settle within 5 seconds of load and to remain stable under
  pan / zoom / settings adjustments.
- **Operator** experimenting with layout modes. Expects to switch between
  force-directed and stress majorization at runtime without restarting and
  without ghost positions.
- **Researcher** loading a much larger graph (50k node aspirational ceiling).
  Expects degradation to be graceful (slower settle, still interactive
  camera) rather than crash or freeze.

## 4. Acceptance criteria

A1. **Layout converges**. With default parameters, the 5k-node knowledge
    graph reaches RMS velocity < 0.01 within 600 frames at 60Hz wall-clock
    (≈10s). Confirmed by `BroadcastOptimizer` issuing the periodic full
    broadcast at iteration 300 with non-zero filtered count.

A2. **Layout mode switching is hot**. Changing layout mode through the
    settings panel applies within one settle cycle (≤2s) without graph
    teardown, without ghost positions, and without re-loading from the
    repository.

A3. **NaN safety**. A pathological input (zero-distance node pair, isolated
    node, repulsion-only edge) never produces NaN positions delivered to
    the client. The grid sentinel kernel clamps before the broadcast stage.

A4. **GPU panic survival**. A simulated CUDA panic (test harness:
    deliberate divide-by-zero in a kernel) results in actor recovery within
    one tick, with the supervisor logging the panic but not tearing down
    the actor system.

A5. **Periodic broadcast cadence**. Once layout converges, position
    broadcasts continue at a low-rate heartbeat (≥1 broadcast / 5s) so that
    clients connecting late always receive a full position frame within
    the heartbeat window.

A6. **Layout engines parity**. All five engines (force-directed, stress
    majorization, hierarchical, circular, geographic) load and run from a
    clean cold start without engine-specific configuration files. Each
    produces a non-degenerate layout (no all-coincident points) on the
    default 5k-node graph.

## 5. Non-goals

- 50k+ nodes at interactive rates (aspirational; document the ceiling but
  do not optimize for it in this sprint).
- WebGPU compute. The GPU pipeline is CUDA-only by design; WebGPU is a
  client-side rendering concern (Section 4).
- Constraint solving beyond what ontology relationships provide. SHACL or
  arbitrary user constraints are not in scope.
- Re-implementing layout in JavaScript / WASM for CPU-only fallback. The
  CPU fallback is a static initial placement (random sphere) only.

## 6. Acceptance evidence to gather during implementation

- Headless benchmark run of all five engines on the bundled 5k-node fixture.
- Panic-injection test harness output showing actor survival.
- Settlement profile: RMS velocity vs frame number, plotted to confirm
  convergence shape matches expected log-decay.
- BroadcastOptimizer logs showing the 300-iter periodic broadcast firing.

## 7. Out-of-scope smells flagged for ADR review

The `main` HEAD code contains several commits whose existence suggests
underlying fragility. The ADR must address whether to bring forward the
fix or the underlying design that needed fixing:

- `cbac7532a fix: merge GPU positions with Neo4j on incremental graph upload`
  — implies the upload path was overwriting computed positions. Address by
  designing upload as additive-only, not blind-replace.
- `aee17f6e6 fix: always randomize zero-position nodes on every GPU upload`
  — implies repeated all-zero uploads. Address by gating uploads on
  meaningful position delta.
- `1e46a780a fix: pre-allocate GPU buffers for 8192 nodes (was 1000)`
  — magic number. Address with a configured upper bound and clear OOM
  handling, not pre-allocation guesswork.
- `6364f0059 fix: resize aabb_block_results and partial_kinetic_energy on
  node count change` — implies adjacent buffer sizes were not invariant.
  Address by colocating buffers in a single struct with a shared resize
  operation.
