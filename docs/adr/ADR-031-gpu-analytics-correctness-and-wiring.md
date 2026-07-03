# ADR-031: GPU Analytics Correctness and Wiring

**Status**: Partial — updated 2026-07-03: the typed data-channel and the LOF kernel have landed (`crates/visionclaw-domain/src/analytics.rs` `NodeAnalytics`; `app_state.rs` consumes `binary_protocol::NodeAnalytics`; `compute_lof_kernel` rewritten with `lof_scores` in `crates/visionclaw-gpu/src/cuda_sources/gpu_clustering_kernels.cu`). The remaining kernel-correctness decisions — multi-level Louvain aggregation, DBSCAN border labelling and the PageRank FFI switch — are **not** yet confirmed in code. Originally proposed 2026-06-02.
**Date**: 2026-06-02
**Deciders**: VisionClaw team

## Context

VisionClaw is a Rust (actix) + CUDA GPU dual-graph visualiser with a React / three.js / WebGPU client. The graph-analytics subsystem — Louvain community detection, DBSCAN/label-propagation clustering, PageRank centrality, SSSP, connected components, and anomaly detection — was built over several sprints across the GPU actor tree but never connected end-to-end, and several kernels compute incorrect answers. A live audit (verified against running services and a full code read, 2026-06-02) traced the pipeline and catalogued the defects.

The intended data flow is:

```
HTTP route → handler → actor → GPUManagerActor → ClusteringActor / AnalyticsSupervisor
  → unified_gpu_compute driver → CUDA kernel
  → node_analytics  (Arc<RwLock<HashMap<u32,(cluster_id:u32, anomaly:f32, community:u32)>>>, app_state.rs:347-349)
  → V3 binary protocol  (48 B/node; sssp_distance@28, sssp_parent@32, cluster_id@36, anomaly@40, community@44)
  → WebSocket → client
```

The V3 wire record is **already 48 B with 8 fields** (`binary_protocol.rs:40-49`): `id@0`, `position@4` (12 B), `velocity@16` (12 B), `sssp_distance@28` (f32), `sssp_parent@32` (i32), `cluster_id@36` (u32), `anomaly@40` (f32), `community@44` (u32). The SSSP slots already exist on the wire — the encoder simply feeds them `None` at every broadcast site. A duplicate encoder lives in `crates/visionclaw-protocol/src/binary_protocol.rs` (re-exported via `src/utils/socket_flow_messages.rs:25`), and a **V5 frame variant** (`client_coordinator_actor.rs:420`) prepends `5u8` + an 8-byte sequence (+9-byte offset shift) on the live broadcast path. All three must move in lockstep with any layout change.

The audit found the subsystem is "built but unwired / computing wrong answers." The concrete defects:

- **Louvain** has no aggregation phase and a `sigma_tot` atomic race; modularity converges near zero.
- **DBSCAN** classifies all border points as noise (only core points are ever labelled).
- **PageRank** has a correct global-dangling kernel at `pagerank.cu:186-261`, but the FFI binding wires the buggy per-block one (`metrics.rs:645`) that distributes dangling mass per-block; the result is unnormalised. The fix is an **FFI switch to the correct kernel + deletion of the legacy one**, not a rewrite.
- **LOF anomaly** computes `1/local_density` (`gpu_clustering_kernels.cu:425-427`) rather than the local-outlier-factor ratio.
- **Label propagation** silently shrinks its shared-memory working set, dropping labels (`community.rs:95-106`, kernel `visionclaw_unified.cu:1487/1570`); there are **three** label-propagation implementations to collapse to one.
- **GPU `AnomalyDetectionActor` IS mounted** (`analytics_supervisor.rs:124-127`) and writes a LOF score to `node_analytics.anomaly` (`anomaly_detection_actor.rs:319-333`), but the LOF kernel computes `1/local_density` rather than the local-outlier-factor ratio (`gpu_clustering_kernels.cu:425-427`), so the value is wrong. Separately, the live `/anomaly/toggle` route drives a CPU agent-health heuristic (`anomaly_handlers.rs:14`, `physics.rs:498`) onto the same conceptual surface; there is no `/anomaly/detect` route exposing the GPU result, and dead RNG anomaly code (`anomaly.rs:88-250`) plus a sleep-simulated clustering stub (`clustering_handlers.rs:414`) remain.
- **SSSP has wire slots but the encoder feeds `None`**: `sssp_distance@28` / `sssp_parent@32` exist on every V3 record, but all three broadcast sites hardcode `sssp_data=None` (`socket_flow_handler/actor_messages.rs:51`, `position_updates.rs:558`, `client_coordinator_actor.rs:417`). SSSP reaches the wire with one encoder argument, not a layout change. **PageRank centrality and connected-components labels have no wire slot at all** and are genuinely dropped.
- **`node_analytics` is a 3-slot tuple** `(cluster_id, anomaly, community)` (`app_state.rs:347-349`) — it cannot carry PageRank centrality, which therefore has nowhere to live even when correctly computed. (SSSP distance is not a `node_analytics` field; it is sourced from graph-node SSSP results into the existing wire slot, so the tuple does not block it.)
- **`node_analytics` has at least seven writers**, not one: `clustering_handlers.rs:65`, `clustering.rs:38`, `community.rs:102`, `anomaly.rs:58&66`, `clustering_handler.rs:254&797`, `clustering_actor.rs:236/351/492`, `anomaly_detection_actor.rs:316`. The handler path writes a 1-based masked `cluster_id`; **the actor path writes the raw 0-based community label into BOTH `cluster_id` AND `community_id`** (`clustering_actor.rs`), so the two fields collide and the client `cluster_id == 0 == unclustered` convention is violated on the auto path.
- **Client**: `qualityGates` have no defaults, so correct server analytics are invisible by default; `ClusterHulls` falls back to a spatial-grid JS heuristic that silently masks missing server data; `aiInsights/*` is dead code.

The root cause is structural: a 3-slot tuple as the analytics channel forces every new metric to either squat on an existing slot or be dropped, and the absence of host-side correctness tests let wrong kernels ship. This ADR records the decisions to fix the subsystem. It does not prescribe implementation mechanics.

## Decision

We adopt eight decisions (D1–D8). Each names the chosen direction, its rationale, and the alternatives rejected.

### D1: Multi-level GPU Louvain with a modularity acceptance gate

Replace the single-pass local-move Louvain with a multi-level algorithm: local-move against a **read-only `sigma_tot` snapshot** (double-buffered, so the per-iteration aggregate is fixed for the duration of the pass and the atomic write race is structurally eliminated), followed by **graph aggregation/contraction** into a coarser graph, then iterate the local-move/contract loop until convergence.

A pass is accepted only if it improves modularity. The acceptance gate requires **modularity ≥ 0.3 and converged on the canonical graph**; any pass that regresses modularity is rejected and the previous level's partition is kept.

**Rationale**: The atomic race on `sigma_tot` is a read-modify-write hazard that makes the modularity-gain computation nondeterministic; double-buffering removes the hazard without locks. Without aggregation, single-level Louvain cannot escape its first local optimum, which is why modularity sits near zero. A keep-previous-on-regression rule guarantees monotonic improvement and makes the kernel safe to run repeatedly.

**Alternatives considered**:
- *Lock the `sigma_tot` accumulator*: rejected — serialises the hottest inner loop and defeats the point of a GPU kernel.
- *Leiden instead of multi-level Louvain*: deferred — higher partition quality but a larger kernel rewrite; Louvain with aggregation clears the ≥ 0.3 gate on the canonical graph and is the smaller change.
- *CPU fallback*: rejected — defeats the GPU subsystem's reason to exist and reintroduces a fallback path (against ADR-014's "no fallbacks" principle).

### D2: A typed `NodeAnalytics` struct replaces the 3-slot tuple; wire grows by one f32 for centrality only

The 3-slot `node_analytics` tuple `(cluster_id, anomaly, community)` cannot carry PageRank **centrality**. We replace it with a named struct that mirrors the wire contract:

```rust
struct NodeAnalytics {
    cluster_id:    u32,   // 1-based, 0 = unclustered (see D3)
    community_id:  u32,
    anomaly:       f32,   // real LOF ratio (D7)
    centrality:    f32,   // PageRank, normalised — the ONLY field that needs a new wire slot
    // reserved for future metrics; growth is additive at the struct tail
}
```

**The wire change is minimal, not a two-field expansion.** The V3 record is already 48 B and already carries `sssp_distance@28` and `sssp_parent@32`; SSSP is fixed by wiring the encoder (D-SSSP below / D7), **not** by growing the layout. Only `centrality` lacks a slot. We therefore **append a single `centrality@48` f32, growing the per-node stride from 48 B to 52 B**. Existing offsets (`sssp_distance@28`, `sssp_parent@32`, `cluster_id@36`, `anomaly@40`, `community@44`) are preserved unchanged.

`sssp_distance` is intentionally **not** a `node_analytics` field: it is sourced from the graph node's SSSP result straight into the existing wire slot, so the struct does not gate it. The encoder must feed `sssp_data` at all three broadcast sites (`socket_flow_handler/actor_messages.rs:51`, `position_updates.rs:558`, `client_coordinator_actor.rs:417`) instead of the current hardcoded `None`.

The struct is the single source of truth for offsets. **Both encoder copies move together**: `src/utils/binary_protocol.rs` and the duplicate `crates/visionclaw-protocol/src/binary_protocol.rs` (re-exported via `socket_flow_messages.rs:25`), plus the **V5 framing variant** (`client_coordinator_actor.rs:420`, +9-byte prefix). Encoder and client parser are asserted against the struct offsets in a host-side test.

**Rationale**: Ad-hoc tuple growth is the documented root cause of the encoding drift in D3. A named struct with explicit offsets makes every wire field auditable and lets future metrics grow additively at the tail. Sizing the wire growth to the one field that genuinely needs it (centrality) — rather than re-adding SSSP slots that already exist — keeps the breaking change as small as possible and honours the "maximal use of existing engineering" mandate.

**Wire-format / client-parser impact**: 48 B → 52 B is a breaking stride change; server and client ship together (see Consequences → Migration). Client decode sites that hard-code 48 B or autodetect stride by `payload.length % nodeSize` (`binaryProtocol.ts:258-262`, `BINARY_NODE_SIZE_V3`, `nodeAnalyticsStore.ts:9`, `binary-processor.ts:81-86`, `createBinaryNodeData:432-464`) must all move to 52 B in the same release.

**Alternatives considered**:
- *Re-add SSSP as a "new" slot* (what the original draft implied): rejected — the slots already exist; adding them again would duplicate the field and waste 8 B.
- *Typed secondary analytics frame* (send centrality on a separate channel): rejected as the primary path — adds a second framing format and reintroduces cross-channel drift. Recorded as the fallback if the 4-byte stride increase ever proves too costly.
- *Keep the tuple, overload `community` to also mean centrality*: rejected — exactly the slot-squatting that caused the current defects.

### D3: Single-writer `node_analytics` with one canonical `cluster_id` encoding

`ClusteringActor` becomes the **sole writer** of `node_analytics`. The other six-plus writers (`clustering_handlers.rs:65`, `clustering.rs:38`, `community.rs:102`, `anomaly.rs:58&66`, `clustering_handler.rs:254&797`, `anomaly_detection_actor.rs:316`) are removed or refactored to route their results *through* the actor rather than writing the shared map directly. There is one canonical `cluster_id` encoding — **1-based with `0 = unclustered`** — honoured identically on the **auto** and **manual** paths.

As part of this, the actor's **dup-write bug** is fixed: `clustering_actor.rs` currently writes the raw 0-based community label into BOTH `cluster_id` AND `community_id` (`:236/351/492`). After the fix, `cluster_id` carries the 1-based clustering result and `community_id` carries the Louvain community label; they are distinct fields.

**Rationale**: Seven writers with two encodings (and a field collision) are the source of the auto-vs-manual divergence and the violated `0 == unclustered` client convention. A single writer makes the encoding a single, testable invariant. This aligns with ADR-014 principle 5 ("ClusteringActor writes results").

**Alternatives considered**:
- *Keep both writers, reconcile downstream*: rejected — reconciliation logic is exactly the kind of masking fallback that hides bugs.
- *0-based encoding*: rejected — would require the client to abandon the established `0 == unclustered` sentinel, a wider breaking change for no benefit.

### D4: Make the GPU anomaly result correct and honest; namespace the heuristic path

The GPU `AnomalyDetectionActor` is **already mounted** (`analytics_supervisor.rs:124-127`) and already writes to `node_analytics.anomaly` (`anomaly_detection_actor.rs:319-333`). Two things are wrong, and both are fixed:
1. **The LOF value is wrong.** The kernel computes `1/local_density` instead of the local-outlier-factor ratio (`gpu_clustering_kernels.cu:425-427`). Fixed to emit the real LOF ratio (part of D7).
2. **The live `/anomaly/toggle` route is a different concept on the same surface.** It drives a CPU agent-health heuristic (`anomaly_handlers.rs:14`, `physics.rs:498`). It is **retired or clearly namespaced** as agent-health monitoring, because **graph-structural anomaly and agent-health are different concepts** and must not share a field. A `/anomaly/detect` route (currently absent) exposes the GPU structural result. Dead RNG anomaly code (`anomaly.rs:88-250`) is deleted.

**Rationale**: The live route currently lies — it reports MCP-agent-health heuristics as graph anomaly, and even the GPU path emits a non-LOF number. Conflating the two surfaces and shipping a wrong kernel both corrupt the analytics contract. Separating the surfaces and fixing the kernel lets each carry honest semantics.

**Alternatives considered**:
- *Keep the heuristic as a fallback when GPU is unavailable*: rejected — silent semantic substitution is the failure mode this ADR exists to remove.
- *Delete agent-health entirely*: deferred — it may have operational value, but only under an explicit, separately named surface, never on the graph-anomaly field.

### D5: Topology-based auto-trigger policy decoupled from physics, manifest-configurable

Topology-based analytics (Louvain, connected components, PageRank, anomaly) fire on **graph-load** and on a **bounded interval thereafter**, **decoupled from physics convergence**. Overlapping passes are guarded: a pass must complete within its interval, and a new pass is skipped (not queued) if the previous one is still running. Cadence and per-kernel parameters are **manifest-configurable, not magic constants**.

**Rationale**: Tying analytics to physics convergence means they never run on graphs that never settle, and re-run wastefully on graphs that re-settle. Topology metrics depend on edges, not on layout, so they should track graph mutation, not frame stability. The overlap guard prevents a slow pass from stacking behind the interval and saturating the GPU. Manifest configuration removes the hard-coded cadence constants that make tuning a code change.

**Alternatives considered**:
- *Run every frame*: rejected — wasteful; topology rarely changes per frame.
- *Manual-trigger only*: rejected — leaves the common case (load a graph, see communities) requiring an explicit user action.
- *Queue overlapping passes*: rejected — unbounded queue growth under load; skip-if-running bounds GPU pressure.

### D6: Client contract — render-by-default, opt-in fallback, delete dead code

Three client-side decisions:
1. **Ship `qualityGates` defaults** so correct server analytics render by default. Today the absent defaults make server output invisible.
2. **Demote the `ClusterHulls` spatial-grid JS fallback to explicit opt-in** so it can never silently mask absent server data. When server clusters are missing, the client shows nothing (or an explicit empty state) rather than a fabricated grid.
3. **Delete dead `aiInsights/*` and the orphaned client-side worker Louvain.**

**Rationale**: A client that fabricates clusters when the server sends none hides the very server bugs this ADR fixes — it is a correctness liability, not a resilience feature. Defaults-off `qualityGates` mean the subsystem could be fully fixed server-side and still appear broken. Dead code (worker Louvain, `aiInsights`) competes with the server as a source of truth, contradicting ADR-014's "server is source of truth, client should only render."

**Alternatives considered**:
- *Keep the JS fallback on by default for resilience*: rejected — it is the mechanism that masked the server defects in the first place.
- *Leave dead code in place*: rejected — it confuses contributors about which Louvain is authoritative.

### D7: Correctness-as-contract — host-side tests gate CI

Every analytics kernel gets a **host-side correctness test gating CI**:
- **Louvain**: two-clique graph yields modularity above the gate and the expected 2-community partition.
- **PageRank**: known graphs produce known PageRank vectors within tolerance, and the vector sums to 1 (normalisation).
- **DBSCAN**: border points adjacent to a core point are assigned to that cluster, not labelled noise.
- **LOF**: the local outlier factor ratio matches the reference computation on a seeded point set.

The DBSCAN border-assignment, PageRank dangling-mass normalisation, LOF computation, and label-propagation shared-memory sizing bugs are fixed as part of landing these tests.

Beyond per-kernel known-answer tests, the QE audit obligates:
- **Named fixtures**: a canonical 10,676-node graph fixture (matching the live dataset) plus the small known-answer graphs (two-clique, triangle, star, linear chain). `tests/` currently has no canonical-graph fixture.
- **GPU↔CPU oracle**: each GPU kernel is cross-checked against a CPU reference implementation on the small fixtures (the oracle catches the class of bug — sigma_tot race, per-block dangling — that a single implementation cannot self-detect).
- **Property-based tests**: invariants (cluster_id ≥ 1 or 0; PageRank sums to 1; modularity ∈ [-0.5, 1]; LOF ≥ 0) hold across randomly generated graphs.
- **Golden snapshots**: the encoded 52 B wire record for a fixed fixture is snapshotted so any offset/stride regression fails CI.
- **Measurable NFRs**: NFR-7 (no O(n²) analytics memory — the `approximate_apsp_kernel` allocating 110 MB+ is explicitly out of scope and must not be on the analytics path) and NFR-3 (analytics pass completes within its bounded interval) are asserted with measured bounds, not prose.
- **Struct/offset assertion** (D2): a host test asserts the encoder and TS parser agree with the `NodeAnalytics` offsets.
- **Single-writer assertion** (D3): a test asserts `node_analytics` has exactly one writer and that `cluster_id != community_id` after an auto pass.
- **GPU-path-execution assertion**: each analytics kernel test asserts the GPU kernel *actually executed*, not that a silent CPU fallback produced a coincidentally-matching value. The GPU↔CPU oracle proves the *values* agree; it does **not** prove the *GPU path ran* — a dead GPU path (a missing-symbol PTX load, as bit Louvain when a stale build-hash PTX shadowed the fresh compile) with a working CPU fallback passes a values-match test while the GPU subsystem is silently inert. Generalise the existing `cpu_fallback_count` telemetry idiom (`ontology_constraint_actor.rs:35`) to every analytics kernel and assert it is **zero** after a pass on a GPU-present fixture; a non-zero fallback count fails CI. This closes the silent-CPU-fallback class the GPU-kernel-wiring census below catalogues — the same class that let Louvain regress to CPU undetected.
- **app_state migration**: `src/app_state.rs:347-349` moves from the 3-tuple to the typed struct; existing `tests/analytics_endpoints_test.rs` (JSON-shape only) and `tests/sssp_integration_test.rs` (CPU-only) are extended to assert values, not just shapes.

**Rationale**: These kernels shipped wrong because nothing checked their answers. A correctness test per kernel, gating CI, makes "computes the right answer" a merge requirement rather than a hope. Small known-answer graphs are cheap to evaluate and unambiguous; the GPU↔CPU oracle and property tests catch the nondeterministic race class; golden snapshots pin the wire contract. The GPU-path-execution assertion catches the orthogonal failure the kernel-wiring census exposes — a *correct answer produced by a silently-inert GPU path* — which value-comparison tests cannot detect on their own.

**Alternatives considered**:
- *Manual QA via the visualiser*: rejected — visual inspection cannot distinguish modularity 0.05 from 0.3.
- *Property-based tests only*: complementary, not sufficient — known-answer tests pin exact expected outputs and catch normalisation drift that properties may miss.

### D8: Consolidate duplicate implementations onto one canonical path

The subsystem accreted parallel implementations of the same function; the upgrade collapses each to one canonical path (the user's "consolidation" mandate). The duplicates to remove:
- **Label propagation**: three implementations → one (keep the corrected GPU kernel; delete the rest).
- **Modularity**: a CPU shadow at `clustering_actor.rs:796-819` overrides the GPU modularity result. Delete the CPU shadow; trust the GPU kernel (gated by D7's correctness test).
- **PageRank**: FFI switch from the buggy per-block kernel (`metrics.rs:645`) to the correct global-dangling kernel (`pagerank.cu:186-261`); delete the legacy kernel.
- **Binary protocol encoder**: two copies (`src/utils/binary_protocol.rs` and `crates/visionclaw-protocol/src/binary_protocol.rs`). Keep one canonical encoder; the other re-exports it. No second hand-maintained copy.
- **HTTP analytics paths**: two parallel client→server analytics request paths (`useSemanticService.ts:162-168` → `/api/semantic/centrality`; `GraphAnalysisTab.tsx:117-126` → `runAnalysis include_centrality`). Collapse to one.
- **Client analytics recompute**: `workers/graph.worker.ts:160&431` unconditionally recomputes analytics client-side (`workers/lib/analytics.ts`), gated only by an all-zero check. This is the live client-side shadow; remove it once the server emits correct communities (D6).
- **Out-of-scope kernels**: `compute_zscore_kernel`, `sssp_detect_negative_cycle_kernel` / Bellman-Ford, `approximate_apsp_kernel` (O(n²), 110 MB+, violates NFR-7), and the two stress-majorization kernels are not on the analytics correctness path; they are removed or quarantined, not maintained as live alternatives.
- **Kernel-wiring census**: a 6-agent audit of the full GPU pipeline found that of ~110 compiled CUDA kernels, only ~52 are wired end-to-end, ~20 are CPU-fallback-only, and ~38 are dead — every disconnect following the same pattern (kernel + CPU fallback exist, the Rust FFI bridge was never finished, the CPU path runs silently). This census is the supporting evidence for D7's GPU-path-execution gate (a silent fallback is invisible without it) and bounds the consolidation surface here. The analytics-adjacent kernels its dead-code pass flagged are dispositioned now, not left ambiguous: `select_weighted_centroid_kernel` (keep — K-means++ seeding acceleration), `dbscan_find_neighbors_tiled_kernel` and `dbscan_compact_labels_kernel` (keep, config-gated DBSCAN optimisation variants), `reduce_kinetic_energy_kernel` (consolidate with `calculate_kinetic_energy`). Each kept kernel lands with a D7 GPU-path assertion; none is retained as a silent CPU-only alternative.

**Rationale**: Every duplicate is a second source of truth that can drift from — and silently override — the canonical one (the CPU modularity shadow literally discards the GPU result). Consolidation is a precondition for D7's tests to mean anything: a correctness test on a kernel that a shadow overrides proves nothing.

**Alternatives considered**:
- *Keep duplicates behind feature flags*: rejected — flags multiply the test matrix and preserve the drift risk.
- *Leave out-of-scope kernels compiled but unwired*: acceptable for the stress-majorization/zscore kernels if clearly quarantined; rejected for `approximate_apsp_kernel`, whose mere presence on a code path risks the NFR-7 memory blow-up.

## Consequences

### Positive
- Analytics produce correct answers: Louvain reaches modularity ≥ 0.3, PageRank is normalised, DBSCAN assigns border points, LOF is real, label propagation keeps its labels.
- PageRank centrality reaches the client for the first time via the new `centrality@48` slot; SSSP distance reaches it via the existing `sssp_distance@28` slot once the encoder feed is wired (no layout change needed for SSSP).
- One writer and one `cluster_id` encoding eliminate the auto-vs-manual divergence and restore the `0 == unclustered` convention.
- Real GPU anomaly replaces the fake heuristic; agent-health, if kept, stops masquerading as graph anomaly.
- The client renders correct analytics by default and can no longer fabricate clusters that hide server bugs.
- CI gates make every future kernel change prove correctness, preventing regression to the current state.

### Negative
- **Breaking wire-format change** (D2): the V3 per-node stride grows 48 B → 52 B (one appended `centrality` f32). Server and client are version-locked for this layout and must deploy together; all client decode sites and the V5 framing path move in the same release.
- **Full CUDA rebuild** required for D1, D4, and the D7 kernel fixes (DBSCAN/PageRank/LOF/label-prop). CUDA changes do not benefit from incremental Rust recompilation; budget a full rebuild cycle (≈15 min per the project rebuild guidance).
- Multi-level Louvain with aggregation is more GPU memory and more passes than single-level; the bounded-interval guard (D5) mitigates wall-clock cost but peak memory rises.
- Deleting client dead code and the JS fallback (D6) removes a perceived safety net; graphs with genuinely missing server analytics now show an explicit empty state instead of a fabricated one (this is intended).

### Neutral
- D5 makes analytics cadence a manifest setting; operators tune it without code changes, but must now own that configuration.
- The reserved tail of `NodeAnalytics` (D2) anticipates future metrics; adding one is an additive offset, not a renumbering.

### Wire-format / version impact
The V3 binary protocol gains **one** `f32` field (`centrality@48`) appended after the existing `community@44` slot; the per-node record grows **48 B → 52 B**. `sssp_distance@28` and `sssp_parent@32` already exist and are not re-added — SSSP needs only the encoder feed wired. Existing offsets (`sssp_distance@28`, `sssp_parent@32`, `cluster_id@36`, `anomaly@40`, `community@44`) are preserved so the change is purely additive at the tail, but the stride change is breaking for any parser that hard-codes 48 B or autodetects stride by `length % nodeSize`. The `NodeAnalytics` struct is the canonical offset definition; both encoder copies (`src/utils/binary_protocol.rs`, `crates/visionclaw-protocol`), the V5 framing variant, and the client parser are asserted against it.

### Rebuild cost
D1, D4, the D7 kernel fixes, and the D8 kernel consolidations touch CUDA `.cu` source and require a full rebuild (no incremental path for kernel changes; ≈15 min via the host build tab). D2's struct/offset change touches both Rust encoder copies, the V5 framing path, and the TypeScript client parser. D3, D5, D6, and the D8 client/HTTP consolidations are Rust/TypeScript only and rebuild incrementally (`docker exec visionclaw_container cargo check`).

### Migration
Because D2 is a breaking 48 B → 52 B layout change, server and client ship in lockstep: there is no mixed-version window in which a 48 B client reads the 52 B stride. The `qualityGates` defaults (D6) land in the same client release so the new fields render immediately. The fake `/anomaly/toggle` path (D4) is removed or renamed in the same server release, so no client ever reads the heuristic on the structural-anomaly field after migration.

## Related Decisions
- ADR-014: Semantic Pipeline Unification — establishes "ClusteringActor writes results, binary protocol carries them, client reads them," "server is source of truth," and "no fallbacks." This ADR completes the analytics half of that mandate and removes the fallbacks (D6) ADR-014 prohibited.

## Alternatives Considered

(Per-decision alternatives are recorded inline above. The two cross-cutting alternatives:)

1. **Rewrite the analytics subsystem from scratch on a new framework**: rejected. The kernels and actors exist; the defects are localised (one race, missing aggregation, a wrong-kernel FFI wiring, a mounted-but-wrong LOF kernel, a too-small tuple, and accreted duplicates). Wiring, correcting, and consolidating is a smaller, lower-risk change than a rewrite, and honours the "maximal use of existing engineering" mandate.
2. **Keep the 3-slot tuple and accept that centrality never reaches the client**: rejected. The tuple is the documented root cause of encoding drift, and PageRank is already computed — discarding a correct result at the channel boundary is waste, and the tuple would keep forcing the next metric to squat or be dropped. (SSSP is unaffected: its wire slots already exist.)
