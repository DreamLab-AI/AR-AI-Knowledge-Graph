# QE T5 — Analytics Shadow: Data-Path Audit and Dual-Modularity Evidence

> **✅ SUPERSEDED — FIXES LANDED 2026-06-03.** This is a frozen pre-fix reproduction
> report. Both defects are resolved:
> - **Claim 1 (hulls never render / handler never writes `node_analytics`)**: the
>   clustering spawn task now routes its `Vec<Cluster>` through the
>   `WriteClusterAnalytics` message (`analytics_messages.rs:138`; handled at
>   `clustering_actor.rs:1193`, `gpu_manager_actor.rs:857`, `analytics_supervisor.rs:422`),
>   covering both the GPU and CPU-fallback branches via the single writer.
> - **Claim 2 (dual divergent modularity)**: the CPU shadow `calculate_modularity`
>   is DELETED (only removal comments remain at `clustering_actor.rs:409,886`); a single
>   `modularity_csr` with `MODULARITY_GATE=0.3` (`clustering_actor.rs:1336`) remains.
>
> Authoritative current state: `07-analysis-clustering.md` (§3b, PARALLEL-1) and
> `00-anomaly-register.md`. Hulls render on explicit clustering trigger; auto-trigger
> remains opt-in/OFF by default. Evidence below is retained for historical context.

**Audit date**: 2026-06-03  
**Status**: ~~CONFIRMED DEFECT~~ → RESOLVED — reproduction test in `tests/qe_t5_shadow_modularity.rs`

---

## Claim 1: Cluster Hulls Never Render on the `/analytics/clustering/run` Hot Path

### Full trigger-to-hull data path

| Step | File : Line | What happens |
|------|------------|--------------|
| 1. HTTP POST | `src/handlers/api_handler/analytics/clustering_handlers.rs:21` | `run_clustering()` receives request; inserts a `ClusteringTask` with `status="running"` into `CLUSTERING_TASKS` |
| 2. Spawn | `clustering_handlers.rs:56` | `tokio::spawn` calls `perform_clustering(...)` asynchronously |
| 3. Delegate | `clustering_handlers.rs:323-392` | `perform_clustering()` contacts MCP agent (TCP), then routes to one of `perform_gpu_spectral/kmeans/louvain/default_clustering` in `real_gpu_functions.rs` |
| 4. GPU send | `real_gpu_functions.rs:130 / 187 / 243` | Sends `PerformGPUClustering` to `gpu_manager_addr`; awaits `Vec<Cluster>` back |
| 5. GPU route | `gpu_manager_actor.rs:403-427` | `GPUManagerActor::handle(PerformGPUClustering)` forwards to `AnalyticsSupervisor` |
| 6. Actor dispatch | `analytics_supervisor.rs:601` | `AnalyticsSupervisor` delegates to `ClusteringActor` |
| 7. ClusteringActor | `clustering_actor.rs:1177-1248` | Runs K-means / Louvain / DBSCAN on GPU; **DOES write `node_analytics`** during K-means (line 236) and community detection (line 369) paths |
| 8. Return path | `real_gpu_functions.rs:136,193,249` | Comments claim "`node_analytics` is populated by the central writer in `clustering_handlers::run_clustering`" — **THIS IS FALSE** |
| **BREAK POINT** | `clustering_handlers.rs:63-76` | After `Ok(clusters)` the spawn closure explicitly states "This handler only reports the task result; it does NOT write node_analytics." The handler writes `task.status = "completed"` and `task.clusters = Some(clusters)` — **but never writes `node_analytics`** |
| 9. No-write confirmation | `clustering_handlers.rs:63-76` | `node_analytics` is `None` on `ClusteringActor` for this path unless `SetNodeAnalytics` was previously delivered via `AppState` boot sequence |
| 10. Auto-trigger disabled | `graph_service_supervisor.rs:183-186` | `AutoTriggerConfig::from_env()`: if `VISIONCLAW_AUTO_COMMUNITY_ENABLED` (etc.) env var is absent, `cadence.enabled = false`. **Default is disabled** — so no background job populates `node_analytics` either |
| 11. Wire encoding | `src/utils/binary_protocol.rs` | V3 position frame encodes `cluster_id` from `NodeAnalytics` looked up by masked node id; if `node_analytics` map entry is absent or `cluster_id = 0`, wire sends `0` |
| 12. Client ingest | `client/src/store/websocket/binaryProtocol.ts:381` | `nodeAnalyticsStore.ingest(parsedNodes)` — every frame; if `clusterId = 0` for all nodes, `hasAnalytics` stays `false` |
| 13. Store gate | `client/src/features/analytics/store/nodeAnalyticsStore.ts:97` | `getIndexedBuffer()` returns `null` when `!this.hasAnalytics` |
| 14. Hull layer | `client/src/features/graph/components/ClusterHulls.tsx:259` | `nodeAnalyticsStore.getIndexedBuffer(nodeIdToIndexMap)` returns `null`; `analyticsRef.current` stays `null` |
| 15. Cluster map | `ClusterHulls.tsx:273-340` | `analytics` is null → `hasClusterId = false`, `hasCommunityId = false`; `spatialFallback = false` (DEFAULT), `communityFallback = false` (DEFAULT) → returns empty map |
| 16. No hulls | `ClusterHulls.tsx` | Empty `clusterMap.map` → zero `<mesh>` children → **nothing renders** |

### Exact break point

**`clustering_handlers.rs:63-76`** — The `tokio::spawn` closure that runs `perform_clustering()` explicitly declines to write `node_analytics`. The comment there attributes the write to "ClusteringActor", but `ClusteringActor` only writes `node_analytics` when it holds a non-`None` `NodeAnalyticsMap` (set via `SetNodeAnalytics`). On the `/analytics/clustering/run` path, if `PerformGPUClustering` is dispatched to a `ClusteringActor` clone (line 1189-1194: `Self { node_analytics: self.node_analytics.clone() }`), the analytics map IS cloned — but only if `SetNodeAnalytics` was previously delivered. Without the auto-trigger (disabled by default, `graph_service_supervisor.rs:185-186`) and without a prior GPU clustering run that established the map, the clone is `None` and the write at lines 236-259 and 369-393 silently skips.

The net result: `nodeAnalyticsStore` receives `cluster_id = 0` for every node on every V3 frame → store never sets `hasAnalytics = true` → `getIndexedBuffer()` returns `null` → `ClusterHulls` renders nothing despite `DEFAULT_CLUSTER_HULLS.enabled = true` (`defaults.ts:137`).

---

## Claim 2: Dual Modularity — Two Functions, Divergent Q

### Function A: `modularity_csr` (canonical Newman Q)

**Location**: `src/utils/unified_gpu_compute/community.rs:24-68`

Formula (as implemented):
```
Q = Σ_c [ intra_c / 2m − (Σtot_c / 2m)² ]
```
Where:
- `m = total_weight` (sum of all weighted degrees / 2)
- `intra_c` = sum of CSR edge weights where both endpoints are in community c (double-counted, matching 2m denominator)
- `Σtot_c` = sum of weighted degrees of nodes in community c

This is the standard Newman-Girvan definition. Used as:
- The modularity gate in `perform_community_detection()` (`clustering_actor.rs:380`) — the value that determines whether community assignments are published to `node_analytics`
- Stored as `CommunityDetectionResult.modularity` (the top-level field, `clustering_actor.rs:412`)

### Function B: `calculate_modularity` (shadow heuristic)

**Location**: `src/actors/gpu/clustering_actor.rs:835-858`

Formula (as implemented):
```
m = (Σ_c internal_edges_c + external_edges_c) / 2
Q = Σ_c [ internal_edges_c / (2m) − (internal_edges_c + external_edges_c)² / (2m)² ]
  clamped to [0.0, 1.0]
```
Where:
- `total_edges` is reconstructed from the `Community` struct fields, not from the CSR graph
- `degree_sum` for a community c is its `internal_edges + external_edges` — this is the community's total degree contribution, NOT the sum of individual node degrees (`Σtot_c`)
- The clamping to `max(0.0)` hides negative Q values that legitimately signal anti-modular partitions
- Stored as `CommunityDetectionResult.stats.modularity` (`clustering_actor.rs:397`) — also returned to the HTTP caller via the `stats` field

### Worked example: two K3 triangles joined by one bridge edge (BARBELL_K3)

Graph: 6 nodes, 7 edges.  
Nodes {0,1,2} form triangle-A; nodes {3,4,5} form triangle-B; edge (2,3) is the bridge.  
Partition: A = {0,1,2}, B = {3,4,5}.

**Function A — modularity_csr:**

Edge inventory (undirected, both directions in CSR):
- Node 0: degree 2 (edges to 1,2)
- Node 1: degree 2 (edges to 0,2)  
- Node 2: degree 3 (edges to 0,1,3)
- Node 3: degree 3 (edges to 2,4,5)
- Node 4: degree 2 (edges to 3,5)
- Node 5: degree 2 (edges to 3,4)

`total_weight = m = Σdeg / 2 = (2+2+3+3+2+2)/2 = 7.0`

Community A: `sigtot_A = 2+2+3 = 7`, `intra_A = 2+2+2 = 6` (edges 0↔1, 1↔2, 0↔2, both directions)  
Community B: `sigtot_B = 3+2+2 = 7`, `intra_B = 2+2+2 = 6`

`Q = (6/14 − (7/14)²) + (6/14 − (7/14)²)`  
`Q = (0.4286 − 0.25) + (0.4286 − 0.25)`  
`Q = 0.1786 + 0.1786 = 0.357143`  

**Q_A = 5/14 ≈ 0.3571** (confirmed by existing unit test `barbell_k3_two_communities_is_five_fourteenths` at `community.rs:874`)

**Function B — calculate_modularity:**

The `Community` struct for community A would have:
- `internal_edges = 3` (3 undirected internal edges: 0-1, 1-2, 0-2)
- `external_edges = 1` (1 external edge: 2-3)

Community B:
- `internal_edges = 3` (edges 3-4, 4-5, 3-5)
- `external_edges = 1` (edge 2-3)

`total_edges = (3+1) + (3+1) = 8`  
Note: this double-counts the bridge edge (once for A's external, once for B's external), so `total_edges = 8` but actual edge count is 7. Already divergent from `m=7`.

`m = total_edges / 2 = 4.0`

Community A:  
`e_in_A = internal_edges_A / (2m) = 3 / 8 = 0.375`  
`degree_sum_A = internal_edges_A + external_edges_A = 4`  
`a_sq_A = (degree_sum_A / (2m))² = (4/8)² = 0.25`

Community B: same by symmetry.

`Q = (0.375 − 0.25) + (0.375 − 0.25) = 0.125 + 0.125 = 0.25`  

**Q_B = 0.25**

### Result

| Fixture | Partition | `modularity_csr` (Q_A) | `calculate_modularity` (Q_B) | Divergence |
|---------|-----------|----------------------|------------------------------|------------|
| BARBELL_K3 (7 edges, 6 nodes) | {0,1,2}, {3,4,5} | **0.3571** | **0.25** | 0.1071 — 30% relative error |

The gate threshold is `MODULARITY_GATE = 0.3` (`clustering_actor.rs:1254`).  
`Q_A = 0.357 > 0.3` → gate accepts, community assignments published.  
`Q_B = 0.25 < 0.3` → if the gate used Q_B, it would reject this valid partition.  

The HTTP `/analytics/community/detect` response emits `result.modularity` (Q_A, the correct value) but also includes `stats.modularity = actual_modularity` which is Q_B. Any consumer reading `stats.modularity` receives a value that disagrees with the gate decision.

The `start_clustering` handler (`clustering_handler.rs:232-240`) computes a third, entirely different "Newman approximation" modularity inline from cluster size ratios — neither Q_A nor Q_B — for the `/clustering/start` response field. Three distinct modularity values exist in the system.

---

## Fix Specification (do not implement)

1. **node_analytics write on hot route**: `clustering_handlers.rs:perform_clustering()` (the `tokio::spawn` task, line 56) must obtain `AppState.node_analytics` and, after a successful `ClusteringActor` response, call `apply_modularity_gate()` or the equivalent single-writer update. Alternatively, `ClusteringActor::handle(PerformGPUClustering)` must be restructured so it always has the `NodeAnalyticsMap` before running (guarantee `SetNodeAnalytics` is delivered at boot before any clustering request is accepted). The ADR-031 D3 single-writer contract must assign exactly one caller as owner; currently the contract is declared but not enforced on the hot route.

2. **Canonical modularity**: `modularity_csr` (`community.rs:24`) is the canonical Newman Q. `calculate_modularity` (`clustering_actor.rs:835`) must be deleted. Every response field and gate decision must use `modularity_csr` exclusively. The `stats.modularity` field in `CommunityDetectionStats` must be removed or must be populated from `modularity_csr` — not from `calculate_modularity`.
