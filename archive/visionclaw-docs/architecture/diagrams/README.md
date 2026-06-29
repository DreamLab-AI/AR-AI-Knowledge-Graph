# VisionClaw Architecture Diagrams

> Verified source of truth as of 2026-06-03. All diagrams are rendered as
> Mermaid and were produced by static code analysis plus git archaeology on
> that date. Where a diagram and a prose document disagree, the diagram wins.

---

## Diagram Index

| File | Title | Contents |
|------|-------|----------|
| [`00-anomaly-register.md`](00-anomaly-register.md) | Anomaly Register | Ranked cross-cutting defect register (T1–T8), resolution status, git archaeology, and QE triage. T1/T2/T4/T5 resolved 2026-06-03; T6/T7/T8 deferred. |
| [`01-settings-flow.md`](01-settings-flow.md) | Settings Flow | Physics settings write path (client slider → SQLite) and hydration path (client connect → store). Covers the single debounced autoSaveManager persistence path, the single `UpdateSimulationParams` dispatcher, and the unified `physics_bounds` validation SSOT. |
| [`02-population-handoff.md`](02-population-handoff.md) | Dual-Graph Population & WebSocket Handoff | Dual-graph node classification (knowledge vs ontology), `metadata["type"]` as the single authoritative field feeding all readers via `Node::population()` / `Node::population_type()`, and the resolved Z-spray root cause. |
| [`03-interaction-events.md`](03-interaction-events.md) | Interaction Events | All user-interaction paths affecting the graph: node selection, drag (single JSON frame post-T2), hover, NL commands, raycasting strategies, and dead interaction systems (T8). |
| [`04-updates-backoff.md`](04-updates-backoff.md) | Parameter Update and Backoff Cadence | Full lifecycle from `PUT /api/settings/physics` through GPU force computation, FastSettle convergence, `ForceFullBroadcast` snapshot, and idle state. Single persistence path and single `UpdateSimulationParams` dispatcher (T2 resolved). |
| [`05-wire-analytics-types.md`](05-wire-analytics-types.md) | Extended Analytics Wire Types (V3, 52 B) | Complete 52-byte V3 per-node wire layout (node_id@0, pos@4, vel@16, sssp@28, cluster_id@36, anomaly@40, community_id@44, centrality@48), analytics data flow from GPU kernels to render channel, and encoder/decoder inventory including the dead 48-byte shadow crate (T6). |
| [`06-gpu-physics.md`](06-gpu-physics.md) | GPU Physics | One physics step end-to-end: CUDA kernel invocation, shared-Mutex access, divergence guard, disc projection apply/undo cycle, and the duplicate force-pass kernels (T7). |
| [`07-analysis-clustering.md`](07-analysis-clustering.md) | Analysis and Clustering Pipeline | All analytics trigger paths, GPU actor mesh (GPUManagerActor → AnalyticsSupervisor → ClusteringActor/PageRankActor/AnomalyDetectionActor/ShortestPathActor), single `node_analytics` writer per field, single canonical `modularity_csr`, cluster hull render chain, and auto-trigger safety interlock (default OFF). |

---

## Resolution Summary (2026-06-03)

| Theme | Status | Diagram |
|-------|--------|---------|
| T1 — node population SSOT | Resolved | `02-population-handoff.md` |
| T2 — doubled write/dispatch paths | Resolved | `01-settings-flow.md`, `04-updates-backoff.md` |
| T4 — validation ceiling mismatch | Resolved | `01-settings-flow.md` (§3a) |
| T5 — shadow algorithms / hull non-render | Resolved | `07-analysis-clustering.md` |
| T6 — 48-byte shadow protocol crate | Deferred | `05-wire-analytics-types.md` (§C) |
| T7 — duplicate CUDA force kernels | Deferred | `06-gpu-physics.md` |
| T8 — dead interaction managers | Deferred | `03-interaction-events.md` |

Open items are tracked in [`../KNOWN_ISSUES.md`](../KNOWN_ISSUES.md).
