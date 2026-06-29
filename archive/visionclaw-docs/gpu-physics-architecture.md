# GPU Physics Architecture

## Overview

VisionClaw uses a CUDA-accelerated physics simulation pipeline built on the Actix
actor framework. GPU kernels compute force-directed graph layout, and results stream
to WebSocket clients in real time.

## Data Flow

```
PhysicsOrchestratorActor
    |
    | SimulationStep (timer-driven or sequential)
    v
ForceComputeActor (CUDA GPU)
    |
    | 1. Upload positions/edges to GPU
    | 2. Launch CUDA kernels (repulsion, attraction, damping, constraints)
    | 3. Read back positions from device memory
    |
    | PhysicsStepCompleted { step_duration_ms, nodes_broadcast, iteration, kinetic_energy }
    v
PhysicsOrchestratorActor
    |
    | BroadcastPositions { positions: Vec<BinaryNodeDataClient> }
    v
ClientCoordinatorActor
    |
    | Binary WebSocket frames (per-client filtered, delta-compressed)
    v
WebSocket clients
```

### Sequential Pipeline

The pipeline is sequential, not timer-raced:

1. `PhysicsOrchestratorActor` sends `ComputeForces` to `ForceComputeActor`.
2. `ForceComputeActor` runs GPU kernels, reads back positions, sends
   `PhysicsStepCompleted` back to the orchestrator.
3. The orchestrator converts positions to `BinaryNodeDataClient`, applies broadcast
   optimization (delta threshold, spatial culling), and sends `BroadcastPositions`
   to `ClientCoordinatorActor`.
4. `ClientCoordinatorActor` serialises binary frames and pushes to each connected
   WebSocket client, evicting slow clients that back up the send buffer.
5. The orchestrator schedules the next step only after broadcast completes.

### Backpressure

`NetworkBackpressure` (src/gpu/backpressure.rs) gates broadcasts. If the network
layer cannot keep up, physics continues but broadcasts are skipped. A
`PositionBroadcastAck` message closes the feedback loop.

### Broadcast Optimization

`BroadcastOptimizer` (src/gpu/broadcast_optimizer.rs) reduces bandwidth:

- **Delta threshold**: nodes that moved less than `delta_threshold` world units are
  excluded from the frame.
- **Spatial culling**: nodes outside the client camera frustum are excluded.
- **Target FPS**: configurable via `ConfigureBroadcastOptimization` message.

## CUDA Build Pipeline

### build.rs

The Cargo build script (`build.rs`) compiles all `.cu` files under `src/utils/` to
PTX and links host-callable FFI symbols into a static library.

**Architecture detection** (priority order):

1. `CUDA_ARCH` environment variable (always wins).
2. `nvidia-smi --query-gpu=compute_cap` auto-detection (native builds only).
3. Default: `sm_75` (Turing baseline).

Docker builds (`DOCKER_ENV` set) **skip nvidia-smi** and default to `sm_75`. This
prevents the build-machine GPU (e.g. sm_89) from being baked into the image, which
would fail on a different runtime GPU.

**PTX compilation** for each `.cu` file:

```
nvcc -ptx -arch sm_{CUDA_ARCH} -o $OUT_DIR/{name}.ptx src/utils/{name}.cu --use_fast_math -O3
```

**Host object compilation** (for FFI-linked kernels):

```
nvcc -c -gencode=arch=compute_{ARCH},code=[sm_{ARCH},compute_{ARCH}] \
     src/utils/{name}.cu -o $OUT_DIR/{name}.o --use_fast_math -O3 -Xcompiler -fPIC -dc
```

The `-gencode` flag embeds both CUBIN (native code for the target arch) and PTX
(portable intermediate). At runtime, the CUDA driver JIT-compiles the embedded PTX
for GPUs with higher compute capability than the build target.

**Device linking** combines all `.o` files via `nvcc -dlink` and archives them into
`libthrust_wrapper.a`.

**ISA downgrade at build time**: after PTX compilation, build.rs rewrites the
`.version 9.N` directive in each `.ptx` file to `.version 9.0` when the toolkit
emits ISA > 9.0. This prevents `CUDA_ERROR_INVALID_PTX` (222) when the runtime
driver is older than the build toolkit.

### PTX path export

Each compiled PTX file path is exported as a Cargo environment variable:

| Module | Env Var |
|--------|---------|
| visionclaw_unified.cu | `VISIONCLAW_UNIFIED_PTX_PATH` |
| gpu_clustering_kernels.cu | `GPU_CLUSTERING_KERNELS_PTX_PATH` |
| dynamic_grid.cu | `DYNAMIC_GRID_PTX_PATH` |
| gpu_aabb_reduction.cu | `GPU_AABB_REDUCTION_PTX_PATH` |
| gpu_landmark_apsp.cu | `GPU_LANDMARK_APSP_PTX_PATH` |
| sssp_compact.cu | `SSSP_COMPACT_PTX_PATH` |
| visionclaw_unified_stability.cu | `VISIONCLAW_UNIFIED_STABILITY_PTX_PATH` |
| ontology_constraints.cu | `ONTOLOGY_CONSTRAINTS_PTX_PATH` |
| pagerank.cu | `PAGERANK_PTX_PATH` |
| gpu_connected_components.cu | `GPU_CONNECTED_COMPONENTS_PTX_PATH` |

## PTX Portability (src/utils/ptx.rs)

The runtime PTX loader handles mismatches between build-time and runtime
environments.

### Loading strategy

1. **Docker**: check pre-compiled PTX first, then fall back to runtime `nvcc -ptx`.
2. **Native**: use build-time PTX (from env var path), validate it, fall back to
   pre-compiled copies in `src/utils/ptx/`, then runtime compilation.

### Runtime GPU arch detection

`detect_runtime_cuda_arch()` queries `nvidia-smi --query-gpu=compute_cap` at first
call, caches the result for the process lifetime, and falls back to `sm_75` if
detection fails. The `CUDA_ARCH` env var overrides detection.

### Runtime driver ISA detection

`detect_max_ptx_isa()` determines the highest PTX ISA version the installed driver
can JIT-compile by parsing the CUDA version from `nvidia-smi` output:

| CUDA Driver | Max PTX ISA |
|-------------|-------------|
| 13.0        | 9.0         |
| 13.1        | 9.1         |
| 13.2+       | 9.2         |
| 12.6+       | 8.5         |
| 12.4+       | 8.4         |
| 12.2+       | 8.2         |
| 12.0-12.1   | 8.0         |
| 11.x        | 7.8         |

### Automatic ISA downgrade

`downgrade_ptx_isa_if_needed()` rewrites the `.version` directive in loaded PTX when
the ISA version exceeds what the driver supports. This runs at load time, after
build-time downgrade, as an additional safety net.

### Runtime compilation fallback

If no valid PTX file is found, `compile_ptx_fallback_sync_module()` invokes `nvcc`
at runtime:

```
nvcc -ptx -std=c++17 -arch=sm_{detected_arch} src/utils/{module}.cu -o /tmp/ptx_{module}_{timestamp}.ptx
```

Unique temp filenames prevent race conditions when multiple modules compile
concurrently.

## Hardware Requirements

### Minimum

- **GPU**: CUDA compute capability 7.5 (Turing architecture)
  - GeForce RTX 2060/2070/2080 series
  - Quadro RTX 4000/5000/6000/8000
  - Tesla T4
- **Driver**: CUDA 13.0+ (driver 580.x+)
- **Toolkit**: CUDA 13.0+

### Recommended

- **GPU**: CUDA compute capability 8.6 (Ampere architecture)
  - GeForce RTX 3060/3070/3080/3090
  - RTX A4000/A5000/A6000
- **Driver**: CUDA 13.2 (driver 595.x+)
- **Toolkit**: CUDA 13.2 (installed in Docker image)

### Docker defaults

| Setting | Value | Override |
|---------|-------|---------|
| Build arch | sm_75 | `--build-arg CUDA_ARCH=86` |
| Toolkit | CUDA 13.1-13.2 (CachyOS pacman) | N/A |
| CUDA_HOME | /opt/cuda | N/A |

## Troubleshooting

### PTX ISA mismatch: CUDA_ERROR_INVALID_PTX (error 222)

**Symptom**: `cuModuleLoadData` fails with error 222 at startup.

**Cause**: the PTX `.version` directive specifies an ISA version newer than what the
runtime driver supports. Happens when the build toolkit (e.g. CUDA 13.2, ISA 9.2)
is newer than the runtime driver (e.g. CUDA 13.0, ISA 9.0).

**Fix**:
1. Update the GPU driver to match the toolkit version.
2. Or set `CUDA_ARCH` to a lower value and rebuild.
3. The runtime ISA downgrade (`downgrade_ptx_isa_if_needed`) should catch this
   automatically. If it does not, check `nvidia-smi` output is parseable.

### Architecture mismatch: build GPU != runtime GPU

**Symptom**: suboptimal performance or `CUDA_ERROR_NO_BINARY_FOR_GPU`.

**Cause**: PTX was compiled for a different `sm_` target than the runtime GPU.

**Fix**:
1. Docker builds default to `sm_75` which JIT-compiles to any `sm_75+` GPU. This
   is correct for portability.
2. For maximum performance on a known GPU, set `CUDA_ARCH` to match (e.g. `86`
   for Ampere, `89` for Ada Lovelace).
3. The gencode flags in build.rs embed both CUBIN and PTX, so JIT fallback works
   automatically for the host-linked kernels.

### gpu_init_in_progress stuck state

**Symptom**: physics simulation never starts. Logs show
`gpu_init_in_progress stuck` after 30 seconds.

**Cause**: `ForceComputeActor` failed to send `GPUInitialized` back to
`PhysicsOrchestratorActor`. Common reasons:
- CUDA context creation failed silently.
- PTX loading failed (see ISA mismatch above).
- ForceComputeActor mailbox was closed before init completed.

**Recovery** (automatic):
1. **30-second timeout**: `PhysicsOrchestratorActor.initialize_gpu_if_needed()`
   checks `gpu_init_started_at` elapsed time and resets `gpu_init_in_progress`
   after 30 seconds, allowing retry.
2. **GPUInitFailed message**: `ForceComputeActor` sends `GPUInitFailed` after
   exhausting its retry budget, which immediately unblocks the orchestrator.
3. **Address replacement**: when a new `ForceComputeActor` address is stored via
   `StoreGPUComputeAddress` and the old address was disconnected,
   `gpu_init_in_progress` is reset.

**Manual recovery**: restart the service. The orchestrator re-initialises on
startup.

### ForceComputeActor silent failure

**Symptom**: `GPUInitialized` message never arrives. No `GPUInitFailed` either.

**Cause**: the actor panicked during CUDA initialisation (e.g. driver crash, OOM)
and Actix dropped it without sending any message.

**Detection**: `PhysicsOrchestratorActor` periodic health check
(`!gpu_initialized && !gpu_init_in_progress && gpu_compute_addr.is_some()`)
detects a disconnected address and retries init.

**Fix**: check `nvidia-smi` for GPU health. Look for `Xid` errors in `dmesg`.
Verify the GPU has sufficient free memory for the graph size.

## Layout Mode System

VisionClaw supports 6 layout algorithms configurable at runtime via physics mode selector:

| Mode | Algorithm | Use Case | GPU Kernel |
|------|-----------|----------|-----------|
| **Force-Directed** | Repulsion + attraction + gravity + damping | General graphs, hierarchies | VisionflowUnified (28 kernels) |
| **ForceAtlas2 LinLog** | LinLog kernel with degree-scaled mass and adaptive speed | Community detection, large networks | VisionflowUnified (lin-log variant) |
| **Spectral** | Eigenvector-based layout (Laplacian spectrum) | Bipartite graphs, graph matching | DynamicGrid + AABB reduction |
| **Hierarchical** | Sugiyama layering + crossing minimization | DAG visualization, flow diagrams | SemanticForces (hierarchy kernel) |
| **Radial** | Polar coordinates, concentric circles by centrality | Star/hub networks, semantic wheels | VisionflowUnified (radial variant) |
| **Temporal** | Time-aware positioning with temporal edges | Sequence/timeline graphs | VisionflowUnified (with temporal forces) |
| **Clustered** | Constraint zones for node type separation | Multi-partite graphs, ontologies | VisionflowUnified + OntologyConstraints |

### ForceAtlas2 LinLog Mode (Default)

**Algorithm**: Combines repulsive and logarithmic attractive forces to reveal community structure:

```
F_repel = k_r / (r + epsilon)   // Traditional Coulomb repulsion
F_attr  = k_a * ln(r)           // LinLog: logarithmic attraction
F_total = F_repel + F_attr
```

**Parameters**:
- `linLogStrength`: weight of logarithmic component (0-1, default 0.5)
- `swingSpeed`: inertia multiplier for force application (0-2, default 1.0)
- `tractionSpeed`: velocity damping per iteration (0-1, default 0.2)

**Degree-Scaled Mass**: Hub nodes (high-degree) receive higher inertia:
```
mass = 1.0 + degree / avg_degree
```

This prevents hubs from drifting excessively while peripheral nodes settle quickly.

### Per-Node Adaptive Speed

Each node tracks individual swing (inertia) and traction (damping):

```
swing[node]    = |force[node] - last_force[node]|  // Velocity variance
speed[node]    = swing[node] * swingSpeed / (1 + traction * traction_iterations)

position[node] += velocity[node] * speed[node] * dt
```

Nodes with high oscillation slow down automatically, reducing vibration without global damping.

### Constraint Zones

Enforce spatial separation for different node types:

| Zone Type | Constraint | GPU Kernel |
|-----------|-----------|-----------|
| **Disjoint** | Repel incompatible classes | `apply_disjoint_classes_kernel` |
| **Alignment** | Attract class hierarchies | `apply_subclass_hierarchy_kernel` |
| **Identity** | Co-locate owl:sameAs nodes | `apply_sameas_colocate_kernel` |
| **Symmetry** | Symmetrical property edges | `apply_inverse_symmetry_kernel` |
| **Cardinality** | Enforce functional property limits | `apply_functional_cardinality_kernel` |

Zones partition 3D space into regions; nodes within a zone are clamped to that region's bounding box.

---

## PTX Modules

| Module | Kernel Set | Kernels |
|--------|-----------|:-------:|
| VisionflowUnified | Core force-directed layout (repulsion, springs, gravity, integration, constraints) + degree-weighted gravity + ForceAtlas2 LinLog kernel | 28 |
| SemanticForces | DAG hierarchy, type clustering, collision, attribute springs, physicality/role/maturity clustering | 15 |
| GpuClusteringKernels | K-means, DBSCAN, Louvain community detection, LOF/Z-Score anomaly, stress majorization | 22 |
| DynamicGrid | Spatial grid for neighbour queries | 0 (utility) |
| GpuAabbReduction | Axis-aligned bounding box reduction | 1 |
| GpuLandmarkApsp | Landmark-based all-pairs shortest paths + Barnes-Hut stress majorization | 3 |
| SsspCompact | Single-source shortest path (frontier compaction) | 2 |
| VisionflowUnifiedStability | Kinetic energy reduction, stability gate, convergence detection | 4 |
| OntologyConstraints | Disjoint class separation, subclass hierarchy, sameAs co-location, inverse symmetry, functional cardinality | 5 |
| Pagerank | Power iteration centrality with dangling node handling and shared-memory optimization | 8 |
| GpuConnectedComponents | Label propagation connected component detection | 3 |
| **Total** | | **91** |

## GPU Analytics API Endpoints

| Endpoint | Method | Feature |
|----------|--------|---------|
| `/api/analytics/pagerank/compute` | POST | GPU PageRank centrality (damping, max iterations, epsilon) |
| `/api/analytics/pagerank/result` | GET | Cached PageRank scores |
| `/api/analytics/pagerank/clear` | POST | Evict PageRank cache |
| `/api/analytics/pathfinding/sssp` | POST | Single-source shortest path (delta-stepping) |
| `/api/analytics/pathfinding/apsp` | POST | Landmark-based all-pairs shortest path |
| `/api/analytics/pathfinding/connected-components` | POST | GPU label propagation |
| `/api/analytics/clustering/dbscan` | POST | GPU DBSCAN (epsilon, min_points) |
| `/api/analytics/clustering/run` | POST | GPU K-means / Louvain / spectral |
| `/api/analytics/community/detect` | POST | GPU community detection |
| `/api/clustering/dbscan` | POST | Direct DBSCAN (populates binary broadcast) |
| `/api/clustering/start` | POST | Start clustering (populates binary broadcast) |
| `/api/graph/positions` | GET | GPU-computed position snapshot with bounding box |

## Binary Protocol V3 Node Frame (48 bytes)

| Offset | Size | Field | Source |
|:------:|:----:|-------|--------|
| 0 | 4 | node_id (bits 0-25) + type flags (bits 26-31) | Graph |
| 4 | 12 | position x, y, z (3 × f32) | GPU physics |
| 16 | 12 | velocity vx, vy, vz (3 × f32) | GPU physics |
| 28 | 4 | SSSP distance (f32) | GPU analytics |
| 32 | 4 | SSSP parent (i32) | GPU analytics |
| 36 | 4 | cluster_id (u32) | GPU K-means/DBSCAN/Louvain |
| 40 | 4 | anomaly_score (f32) | GPU LOF/Z-Score |
| 44 | 4 | community_id (u32) | GPU Louvain |

## Key Source Files

| File | Purpose |
|------|---------|
| `build.rs` | CUDA compilation, PTX generation, static library linking |
| `src/utils/ptx.rs` | PTX loading, ISA detection, runtime compilation fallback |
| `src/utils/visionclaw_unified.cu` | 28 core physics kernels + degree-weighted gravity |
| `src/utils/semantic_forces.cu` | 15 semantic force kernels (DAG, type, physicality, role, maturity) |
| `src/utils/gpu_clustering_kernels.cu` | 22 clustering/community/anomaly kernels |
| `src/utils/ontology_constraints.cu` | 5 OWL constraint enforcement kernels |
| `src/actors/physics_orchestrator_actor.rs` | Simulation lifecycle, fast-settle convergence |
| `src/actors/gpu/force_compute_actor.rs` | GPU kernel dispatch, position readback, degree-weighted gravity |
| `src/actors/gpu/clustering_actor.rs` | K-means, DBSCAN, Louvain GPU dispatch |
| `src/actors/gpu/semantic_forces_actor.rs` | Semantic force FFI bridge (15 kernels) |
| `src/actors/gpu/ontology_constraint_actor.rs` | Ontology constraint PTX dispatch (5 kernels) |
| `src/actors/client_coordinator_actor.rs` | WebSocket broadcast to clients |
| `src/actors/messages/physics_messages.rs` | All physics/GPU message types |
| `src/gpu/kernel_bridge.rs` | Safe FFI wrappers for semantic force kernels |
| `src/gpu/backpressure.rs` | Network backpressure for broadcast gating |
| `scripts/rust-backend-wrapper.sh` | Runtime CUDA_ARCH auto-detection |
| `src/gpu/broadcast_optimizer.rs` | Delta compression and spatial culling |
