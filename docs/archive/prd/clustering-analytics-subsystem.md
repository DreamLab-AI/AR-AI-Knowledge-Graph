# PRD — GPU Clustering & Analytics Subsystem

- **Document owner**: Graph analytics lead
- **Status**: Active — sign-off on landing P0
- **Target release**: VisionClaw backend `analytics-v2`
- **Audit basis**: Live + code-read audit, 2026-06-02 (see §3)
- **Related artefacts**:
  - Render performance constraints: [`ADR-013-render-performance.md`](../adr/ADR-013-render-performance.md)
  - Semantic pipeline unification: [`ADR-014-semantic-pipeline-unification.md`](../adr/ADR-014-semantic-pipeline-unification.md)

---

## 1. Summary & problem statement

VisionClaw's GPU analytics subsystem is **built but unwired, and where wired
it computes wrong answers**. The pipeline exists end-to-end on paper — HTTP
route → handler → actor message → `GPUManagerActor` →
`ClusteringActor`/`AnalyticsSupervisor` → `unified_gpu_compute` driver → CUDA
kernel → result download → `node_analytics` map → V3 binary protocol →
WebSocket → React/three.js client — but almost no analytic survives that path
intact.

The map at `app_state.rs:347-349` is the structural bottleneck:
`Arc<RwLock<HashMap<u32, (cluster_id: u32, anomaly: f32, community: u32)>>>`.
It has exactly **three slots**, so PageRank centrality has **no sink that can
reach the wire** (connected-component and landmark-APSP labels are HTTP-only and
out of scope for the wire; see §4 FR-7). SSSP is **not** blocked by the tuple —
its wire slots already exist (see below). The V3 protocol is already
**48 bytes/node with eight fields** (`binary_protocol.rs:40-49`): `id`@0,
`position`@4, `velocity`@16, `sssp_distance`@28, `sssp_parent`@32, `cluster_id`@36,
`anomaly`@40, `community`@44. The `sssp_distance`/`sssp_parent` slots are present
on every record; the encoder simply feeds them `None`. Only PageRank centrality
genuinely lacks a wire slot.

The defects are masked by client-side fallbacks. `ClusterHulls` — the only
analytics renderer enabled by default — silently degrades to a JavaScript
spatial-grid bucketing when `cluster_id` is all-zero, so a permanently broken
Louvain looked like working clustering. The remaining server analytics are
invisible by default because their consumers in `GemNodes` are gated behind
`qualityGates.showClusters` / `showAnomalies` / `showCommunities`, none of
which have defaults (and there is no `showCentrality` / `showSSSP` gate at all —
`settings.ts:664-666`). The GPU `AnomalyDetectionActor` **is** already mounted
(`analytics_supervisor.rs:124-127`) and writes to `node_analytics.anomaly`
(`anomaly_detection_actor.rs:319-333`), but its LOF kernel computes
`1/local_density`, not the LOF ratio. Separately, the live `/anomaly/toggle`
endpoint drives a CPU agent-health heuristic over MCP-agent CPU/memory telemetry
(`anomaly_handlers.rs:14`, `physics.rs:498`) on the same surface, there is no
`/anomaly/detect` route exposing the GPU result, and the wire `anomaly_score`
the client sees is permanently `0`.

This PRD states the current state honestly per analytic (§3), then specifies
the capabilities required to make the subsystem correct, wired, and visible
(§4–§6), with a phased rollout that fixes correctness before exposing anything
to users (§7).

## 2. Goals / Non-goals

### Goals

- **Correctness first.** Every shipped analytic produces a mathematically
  defensible result, verified against a fail-closed test with a known answer.
- **Wire reachability.** Every analytic the product claims to compute reaches
  the client through a single, documented channel — no HTTP-only dead ends.
- **One encoding.** A single writer owns `cluster_id`; the
  `cluster_id == 0 == unclustered` convention holds on **every** path (manual,
  auto, and future).
- **Visible by default.** Server-computed analytics render without the operator
  hand-setting undocumented quality gates.
- **Honest fallbacks.** Client heuristics that substitute for server results
  are opt-in and labelled, never silent.

### Non-goals

- New analytic algorithms beyond the eight already present (§3). Spectral
  clustering, hierarchical community detection, and temporal anomaly models are
  out of scope for `analytics-v2`.
- Replacing the V3 binary protocol wholesale. We append one `centrality` f32
  to the analytics channel (§4 FR-7); we do not redesign node-position or edge
  framing, and we do not re-add the SSSP slots (they already exist).
- CPU fallback parity. If the GPU path is unavailable the subsystem fails
  closed (returns no analytics), it does not silently compute a worse answer.
- Client-side recomputation of any analytic. The live worker-side analytics
  recompute (`workers/graph.worker.ts:160&431` → `workers/lib/analytics.ts`,
  gated only by an all-zero check) and the dead `aiInsights/*` are removed
  (§4 FR-10), not revived.
- Out-of-scope kernels on the analytics path: `compute_zscore_kernel`,
  `sssp_detect_negative_cycle_kernel` / Bellman-Ford, `approximate_apsp_kernel`
  (O(n²), allocates 110 MB+, violates NFR-2), and the two stress-majorization
  kernels are not on the analytics correctness path and are not maintained as
  live alternatives.

## 3. Current-state capability matrix

Each row is grounded in the 2026-06-02 audit (live behaviour + code read).
Status vocabulary: **correct** (right answer, reaches wire), **incorrect**
(wired but wrong answer), **unwired** (computes but no wire sink), **fake**
(live path does not use the GPU result it claims), **dead** (code present, no
caller).

| Analytic | Status | Evidence |
|---|---|---|
| **Louvain / community** | incorrect | No aggregation/coarsening phase; `sigma_tot` atomic race. Modularity ≈ 0, 1433 micro-communities on the canonical 10676-node/107823-edge graph, never converges, 51.8 s/pass. |
| **K-means** | correct | Produces stable cluster labels; reaches `cluster_id` when manual path fires. Only correct clustering analytic today. |
| **DBSCAN** | incorrect | Border points always demoted to noise — `atomicMin` sentinel bug in the neighbour-assignment kernel. |
| **Anomaly (LOF)** | incorrect + fake-on-live-route | GPU `AnomalyDetectionActor` **is mounted** (`analytics_supervisor.rs:124-127`) and writes `node_analytics.anomaly` (`anomaly_detection_actor.rs:319-333`), but the kernel computes `1/local_density`, not the LOF ratio (`gpu_clustering_kernels.cu:425-427`). The live `/anomaly/toggle` route instead drives a CPU agent-health heuristic over MCP-agent CPU/mem (`anomaly_handlers.rs:14`, `physics.rs:498`) on the same surface; there is no `/anomaly/detect` route exposing the GPU result, and dead RNG anomaly code (`anomaly.rs:88-250`) remains. Wire `anomaly_score` the client reads is permanently `0`. |
| **SSSP** | wire slots present, encoder feeds `None` | Runs correctly on GPU. The V3 record already carries `sssp_distance`@28 / `sssp_parent`@32, but all three broadcast sites hardcode `sssp_data = None` (`socket_flow_handler/actor_messages.rs:51`, `position_updates.rs:558`, `client_coordinator_actor.rs:417`), so distances never reach the client. Fixed by wiring the encoder feed, not a layout change. |
| **PageRank** | incorrect + unwired | A correct global-dangling kernel exists (`pagerank.cu:186-261`) but the FFI binding wires the buggy per-block one (`metrics.rs:645`); dangling mass summed per-block → unnormalised. HTTP-only; cannot reach the wire — PageRank centrality is the one metric with no slot in the 3-tuple `node_analytics`. |
| **Connected components** | unwired (out of wire scope) | Computes; HTTP-only; no wire sink. Not slated for the wire in `analytics-v2`. |
| **Landmark-APSP** | unwired (out of wire scope) | Computes; HTTP-only; no wire sink. Not slated for the wire in `analytics-v2`. |
| **Label propagation** | incorrect | Shared-memory region silently shrinks at high label counts → label corruption. |

Two cross-cutting defects compound the above:

- **Trigger gap.** Until a recently added auto-trigger, nothing fired Louvain
  except a manual UI button, so `cluster_id` was permanently `0` in normal
  operation.
- **Encoding divergence.** `node_analytics` has **~7 writers**, not one:
  `clustering_handlers.rs:65`, `clustering.rs:38`, `community.rs:102`,
  `anomaly.rs:58&66`, `clustering_handler.rs:254&797`,
  `clustering_actor.rs:236/351/492`, and `anomaly_detection_actor.rs:316`. The
  handler path writes a 1-based masked `cluster_id`; the actor path writes the
  raw 0-based community label into **BOTH `cluster_id` AND `community_id`** (a
  dup-write bug — the two fields collide). Auto and manual `cluster_id`
  encodings therefore **differ on the wire**, and the auto path violates the
  `cluster_id == 0 == unclustered` client convention.

## 4. Required capabilities & functional requirements

Each FR is a ship unit with a named acceptance criterion. Acceptance is
binary. "Canonical graph" = the 10676-node / 107823-edge fixture used in the
audit, checked in as a test fixture.

### FR-1 — Correct multi-level Louvain

- **Behaviour**: implement the aggregation/coarsening phase (community
  contraction → rebuild adjacency → repeat) and remove the `sigma_tot` atomic
  race (per-community accumulation must be race-free, e.g. segmented reduction
  or per-community atomic into a dedicated slot).
- **Acceptance**:
  - Modularity **≥ 0.30** on the canonical graph.
  - Algorithm **converges** (delta-modularity below epsilon within a bounded
    pass count; no oscillation).
  - Community count is on the order of tens-to-low-hundreds, not 1433.
  - Two-clique unit test (§5 NFR-4) returns exactly two communities,
    modularity ≈ 0.5.

### FR-2 — Correct DBSCAN border assignment

- **Behaviour**: fix the `atomicMin` sentinel so border points are assigned to
  a reachable core's cluster rather than demoted to noise.
- **Acceptance**:
  - Border-assignment unit test (§5 NFR-4): a synthetic point that is within
    `eps` of a core but below `minPts` itself is labelled with the core's
    cluster, **not** noise.
  - Noise count on the canonical graph drops to the expected band (no longer
    dominated by misclassified borders).

### FR-3 — Correct, normalised PageRank

- **Behaviour**: an FFI switch, not a kernel rewrite. A correct global-dangling
  kernel already exists at `pagerank.cu:186-261`; switch the FFI binding at
  `metrics.rs:645` from the buggy per-block kernel to it, then delete the legacy
  per-block kernel. The correct kernel accumulates dangling mass globally so the
  result vector sums to 1.
- **Acceptance**:
  - On a known directed-graph fixture, the PageRank vector matches a reference
    NetworkX/igraph computation to ≤ 1e-4 L1 error.
  - `sum(pr) == 1.0 ± 1e-6`.

### FR-4 — Correct GPU anomaly, honest surfaces

- **Behaviour**: the `AnomalyDetectionActor` is **already mounted**
  (`analytics_supervisor.rs:124-127`) and already writes the
  `node_analytics.anomaly` slot (`anomaly_detection_actor.rs:319-333`) — do
  **not** "mount" it. The fixes are: (1) correct the LOF kernel
  (`gpu_clustering_kernels.cu:425-427`) so it emits the LOF **ratio** (mean
  neighbour LRD / point LRD) instead of `1/local_density`; (2) add an
  `/anomaly/detect` route (currently absent) that exposes the GPU structural
  result; (3) namespace the live `/anomaly/toggle` CPU agent-health heuristic
  (`anomaly_handlers.rs:14`, `physics.rs:498`) as agent-health monitoring so it
  no longer masquerades on the graph-anomaly surface; (4) delete the dead RNG
  anomaly code (`anomaly.rs:88-250`).
- **Acceptance**:
  - On a fixture with N planted outliers, the top-N by anomaly score are
    exactly the planted outliers (precision@N = 1.0).
  - Wire `anomaly_score` reflects the corrected GPU LOF ratio, not CPU telemetry,
    and `/anomaly/detect` returns the GPU result.
  - LOF ≈ 1.0 for points in uniform-density regions (sanity property).
  - The agent-health heuristic is reachable only under its own named surface,
    never on the graph-anomaly field; `anomaly.rs:88-250` is removed.

### FR-5 — Unified `cluster_id` encoding (single writer)

- **Behaviour**: consolidate the **~7 `node_analytics` writers** (§3) to a
  single canonical writer, `ClusteringActor`. All paths (manual K-means,
  auto-Louvain, DBSCAN) funnel cluster labels through it. The writer enforces
  `0 == unclustered` and a single labelling base (define canonical: labels are
  1-based, 0 reserved for unclustered). As part of this, fix the actor's
  dup-write bug at `clustering_actor.rs:236/351/492` — it currently writes the
  raw 0-based community label into BOTH `cluster_id` AND `community_id`; after the
  fix `cluster_id` carries the 1-based clustering result and `community_id`
  carries the Louvain community label, distinct fields.
- **Acceptance**:
  - Auto and manual paths produce **byte-identical** `cluster_id` encoding for
    the same input on the wire.
  - No node that belongs to a cluster carries `cluster_id == 0`; every
    genuinely unclustered node carries `cluster_id == 0`.
  - After an auto pass, `cluster_id != community_id` (the dup-write is gone).
  - `node_analytics` has exactly one writer; the other ~6 are removed or routed
    through `ClusteringActor`.

### FR-6 — Correct label propagation

- **Behaviour**: size the shared-memory region for the worst-case label count
  (or fall back to global memory above a threshold) so the region cannot
  silently shrink.
- **Acceptance**:
  - Label-propagation result is stable across two runs on the canonical graph
    (no corruption-induced nondeterminism).
  - High-label-count stress fixture (≥ 4096 distinct seed labels) produces no
    out-of-bounds / truncated labels.

### FR-7 — Typed analytics struct; append one centrality slot (KEY REQUIREMENT)

- **Behaviour**: replace the 3-slot `node_analytics` tuple at
  `app_state.rs:347-349` with a typed struct
  `NodeAnalytics { cluster_id: u32, community_id: u32, anomaly: f32, centrality: f32 }`.
  Only **PageRank centrality** lacks a wire slot, so the wire grows **minimally**:
  append a single `centrality`@48 f32, taking the per-node stride from
  **48 B → 52 B**. Existing offsets (`sssp_distance`@28, `sssp_parent`@32,
  `cluster_id`@36, `anomaly`@40, `community`@44) are preserved unchanged. This is
  the structural blocker for FR-3 only; SSSP is **not** blocked — its slots
  already exist (FR-8 wires the feed). `sssp_distance` is **not** a
  `node_analytics` field — it is sourced from graph-node SSSP results straight
  into the existing wire slot. The struct is the single source of truth for
  offsets, so **both encoder copies move together**: `src/utils/binary_protocol.rs`
  and the duplicate `crates/visionclaw-protocol/src/binary_protocol.rs`
  (re-exported via `socket_flow_messages.rs:25`), plus the **V5 framing variant**
  (`client_coordinator_actor.rs:420`, +9-byte prefix). The client decode sites
  that hard-code 48 B or autodetect stride by `length % nodeSize` must move to
  52 B in the same release: `nodeAnalyticsStore.ts:9`,
  `binaryProtocol.ts:258-262/68/189-202` (`BINARY_NODE_SIZE_V3`),
  `binary-processor.ts:81-86`, and `createBinaryNodeData:432-464`.
- **Acceptance**:
  - PageRank centrality is present at offset 48 in the per-node wire payload and
    parsed by the client; the stride is 52 B end to end.
  - Existing offsets (28/32/36/40/44) are byte-for-byte unchanged.
  - Server and client ship in lockstep (no mixed 48 B / 52 B window); a client
    that hard-codes 48 B or autodetects by `length % nodeSize` is updated, never
    left to silently misread offsets.
  - `node_analytics` is the named `NodeAnalytics` struct, not an anonymous tuple.
  - Both encoder copies and the V5 framing variant agree with the struct offsets
    (asserted in a host-side test, §6).

### FR-8 — Deliver SSSP distances to the client (encoder feed, not a layout change)

- **Behaviour**: the `sssp_distance`@28 / `sssp_parent`@32 slots **already exist**
  on every V3 record — this is a one-argument encoder fix, independent of FR-7.
  Replace the hardcoded `sssp_data = None` at all three broadcast sites
  (`socket_flow_handler/actor_messages.rs:51`, `position_updates.rs:558`,
  `client_coordinator_actor.rs:417`) with the populated SSSP result. On the
  client, **reuse the existing `ssspDistance`@28** decode in the position frame —
  do not introduce a colliding new name.
- **Acceptance**:
  - After an SSSP run from a chosen source, the client receives per-node
    distances at offset 28 matching the GPU computation; unreachable nodes carry
    a defined sentinel (e.g. `f32::INFINITY` or a documented max).
  - No wire-layout change is made for SSSP; offset 28 is reused, not added.

### FR-9 — Auto-trigger cadence for topology analytics

- **Behaviour**: topology analytics (Louvain/community, PageRank, connected
  components) fire automatically on a defined cadence and on graph-mutation
  events, not only on a manual button. Cadence is configurable and debounced so
  a mutating graph does not thrash the GPU.
- **Acceptance**:
  - On a fresh graph load with no UI interaction, `cluster_id`, `community`,
    and `centrality` are populated on the wire within one cadence interval.
  - Rapid sequential mutations coalesce into a single recompute (debounce
    verified).

### FR-10 — Client defaults, display paths, demote masking fallback

- **Behaviour**: `qualityGates.showClusters` / `showAnomalies` /
  `showCommunities` get explicit defaults (clusters + communities on; anomalies
  on once FR-4 lands), and **two new gates `showCentrality` / `showSSSP` are
  added** — neither exists today (settings at `settings.ts:664-666`). There is
  currently **no display path for centrality or SSSP**: `nodeAnalyticsStore.ts:18-20/99-101`
  and `GemNodes.tsx:279-297` handle only cluster/anomaly/community, so display
  paths for the two new metrics must be added. The `ClusterHulls` spatial-grid JS
  bucketing fallback becomes **explicit opt-in** (a named flag, surfaced in the
  UI), not a silent substitution triggered by all-zero `cluster_id`. The
  **live** client-side analytics recompute (`workers/graph.worker.ts:160&431`
  calling `workers/lib/analytics.ts`, gated only by an all-zero check) is removed
  once the server emits correct communities — it is live, not dead. The dead
  `aiInsights/*` is also removed.
- **Acceptance**:
  - With no operator configuration, a freshly loaded canonical graph shows
    server-computed cluster colouring on `GemNodes` and `ClusterHulls`.
  - Centrality and SSSP have working display paths gated by `showCentrality` /
    `showSSSP`.
  - When `cluster_id` is all-zero, `ClusterHulls` renders nothing (or an empty
    state), and only renders the JS grid fallback if its opt-in flag is set.
  - `aiInsights/*` and the worker-side analytics recompute are deleted; no import
    references remain.

### FR-11 — Consolidate duplicate implementations to one canonical path each

- **Behaviour**: the subsystem accreted parallel implementations of the same
  function; collapse each to a single canonical path:
  - **Label propagation**: three implementations → one (keep the corrected GPU
    kernel from FR-6; delete the rest).
  - **Modularity**: delete the CPU modularity shadow at
    `clustering_actor.rs:796-819` that overrides the GPU modularity result; trust
    the GPU kernel (gated by FR-1 / §6 tests).
  - **PageRank kernel**: the FFI switch + legacy-kernel deletion of FR-3.
  - **Binary-protocol encoder**: two copies
    (`src/utils/binary_protocol.rs` + `crates/visionclaw-protocol/src/binary_protocol.rs`,
    re-exported via `socket_flow_messages.rs:25`) → one canonical encoder; the
    V5 framing variant (`client_coordinator_actor.rs:420`, +9-byte prefix) moves
    in lockstep with the FR-7 wire change.
  - **HTTP analytics paths**: two parallel client→server request paths
    (`useSemanticService.ts:162-168` → `/api/semantic/centrality`;
    `GraphAnalysisTab.tsx:117-126` → `runAnalysis include_centrality`) → one.
  - **Client analytics recompute shadow**: the FR-10 removal of the live
    `workers/graph.worker.ts` recompute.
- **Acceptance**:
  - Exactly one label-propagation implementation, one modularity source (GPU),
    one PageRank kernel, one encoder, and one HTTP analytics request path remain;
    grep for the removed duplicates returns no live callers.
  - The CPU modularity shadow no longer overrides the GPU result (asserted by a
    test that the reported modularity equals the GPU kernel output).

## 5. Non-functional requirements

| ID | Axis | Target | Measurement |
|---|---|---|---|
| **NFR-1** | Louvain per-pass latency | seconds, not 51.8 s; **p95 < 5 s** on the canonical graph | bench harness on canonical fixture, GPU warm |
| **NFR-2** | No per-frame allocations | zero heap allocations on the analytics encode hot path per frame | allocation counter / flamegraph over a 60 s steady-state capture |
| **NFR-3** | Result download cost | analytics download + map update does not stall position streaming; encode stays within the per-tick budget (ADR-013) | frame-time histogram; no dropped position frames during a recompute |
| **NFR-4** | Fail-closed correctness tests | the three reference tests below are required gates, not advisory | CI; a failing reference test blocks the merge |
| **NFR-5** | GPU unavailability | subsystem returns no analytics and signals unavailability; never silently substitutes a wrong answer | fault-injection test with GPU disabled |
| **NFR-6** | Wire payload stability | the 48 B → 52 B stride is pinned; offsets are constants with a single source of truth (the `NodeAnalytics` struct) shared by both encoder copies, the V5 framing path, and the client parser | golden 52 B wire-record snapshot + offset round-trip test (encode → decode → equal) |
| **NFR-7** | No O(n²) analytics memory | no analytics-path kernel allocates O(n²) memory; the `approximate_apsp_kernel` (110 MB+ on the canonical graph) and other out-of-scope kernels are off the analytics path | peak-allocation assertion on the canonical fixture, with a measured byte ceiling, not prose |

**NFR-4 reference tests** (fail-closed, known answers):

- **Two-clique modularity**: two K-cliques joined by one edge → exactly two
  communities, modularity ≈ 0.5 (gates FR-1).
- **Known PageRank vectors**: a small directed graph with a published PageRank
  vector → match to ≤ 1e-4 (gates FR-3).
- **DBSCAN border assignment**: planted core + border + noise → border joins
  the core's cluster, noise stays noise (gates FR-2).

## 6. Acceptance gates / test strategy

The gate is all-or-nothing per phase (§7). One unchecked box blocks the phase
tag.

- **Canonical fixture**: a checked-in **10,676-node / 107,823-edge graph
  fixture** (none exists in `tests/` today) plus the small known-answer graphs
  (two-clique, triangle, star, linear chain) used by the per-kernel tests.
- **Unit / kernel correctness**: each fixed kernel (Louvain aggregation,
  DBSCAN border, PageRank dangling reduction, LOF ratio, label-prop sizing)
  ships with a kernel-level test asserting the numeric result against a CPU
  reference on a small fixture.
- **GPU↔CPU oracle**: each GPU kernel is cross-checked against a CPU reference
  on the small fixtures (the oracle catches the race / per-block-dangling class
  of bug a single implementation cannot self-detect).
- **Property-based invariants**: `cluster_id ≥ 1 or 0`; PageRank sums to 1;
  modularity ∈ [-0.5, 1]; LOF ≥ 0 — asserted across randomly generated graphs.
- **Modularity gate**: a test asserts Louvain modularity **≥ 0.3** on the
  canonical graph (gates FR-1).
- **Reference gates (NFR-4)**: the three fail-closed tests are mandatory.
- **Integration — wire reachability**: a headless client harness subscribes to
  the WebSocket, triggers each analytic, and asserts the corresponding wire
  field is populated and correct. This is the test that would have caught
  every "unwired" row in §3.
- **Encoding-parity test (FR-5)**: run the same graph through the auto and
  manual cluster paths; assert byte-identical `cluster_id` on the wire, and
  assert `cluster_id != community_id` after an auto pass (dup-write gone).
- **Single-writer test (FR-5)**: assert `node_analytics` has exactly one writer.
- **Golden wire snapshot (FR-7)**: snapshot the encoded **52 B** wire record for
  a fixed fixture so any offset/stride regression fails CI; round-trip encode →
  decode → equal across both encoder copies and the V5 framing path.
- **Struct/offset assertion (FR-7)**: a host test asserts the encoder and the TS
  parser agree with the `NodeAnalytics` offsets.
- **Visibility test (FR-10)**: launch the client with no config, assert
  cluster colouring is rendered from server data and that the spatial-grid
  fallback does not fire.
- **Performance gates (NFR-1, NFR-3)**: criterion-style benches on the
  canonical fixture; a 10% regression against the committed baseline fails CI.
  NFR-3 asserts the analytics pass completes within its bounded interval with a
  measured bound, not prose.
- **Memory gate (NFR-7)**: peak-allocation assertion on the canonical fixture
  with a measured byte ceiling; the `approximate_apsp_kernel` and other
  out-of-scope kernels must not be on the analytics path.
- **Extend existing value-blind tests**: `tests/analytics_endpoints_test.rs`
  (JSON-shape only) and `tests/sssp_integration_test.rs` (CPU-only) are extended
  to assert values, not just shapes.
- **Regression**: K-means (the one currently correct analytic) must continue
  to pass throughout; a regression blocks the merge.

## 7. Phased rollout

Phases are sequenced so **correctness lands before exposure**. Nothing in §3
that is "incorrect" or "fake" is made visible to users before its correctness
FR passes.

| Phase | Theme | Ships | Gate |
|---|---|---|---|
| **P0** | Correctness fixes | FR-1 (Louvain), FR-2 (DBSCAN), FR-3 (PageRank FFI switch), FR-4 (LOF-kernel fix), FR-6 (label-prop) | NFR-4 reference tests + kernel unit tests + modularity ≥ 0.3 gate pass; NFR-1 met for Louvain |
| **P1** | Wiring | FR-4 (`/anomaly/detect` route + namespace heuristic + delete dead RNG code; actor already mounted), FR-8 (SSSP encoder feed → existing slot), FR-9 (auto-trigger cadence) | Integration wire-reachability tests pass for anomaly + SSSP + auto topology |
| **P2** | Wire append + struct | FR-7 (typed `NodeAnalytics` struct + append `centrality`@48, 48 B → 52 B), then route PageRank centrality through it | Golden 52 B snapshot + struct/offset assertion pass; both encoder copies + V5 framing in lockstep |
| **P3** | Client visibility, display & cleanup | FR-5 (single writer + dup-write fix), FR-10 (defaults + centrality/SSSP display + opt-in fallback + dead-code removal), FR-11 (consolidation) | Encoding-parity + single-writer + visibility tests pass; duplicate implementations collapsed; `aiInsights/*` and worker recompute removed |

P0 has no user-visible effect (analytics that were invisible stay invisible
until P3) and is therefore safe to land continuously. P3 is the only phase
that changes default client behaviour and is the last to land, after every
analytic it exposes is provably correct and reachable.

Dependency notes: FR-3's wire delivery depends on FR-7 (P2), so the PageRank FFI
switch lands in P0 but centrality only reaches the client in P2. FR-8 (SSSP) does
**not** depend on FR-7 — the slot already exists, so SSSP can wire in P1 ahead of
the P2 struct change. FR-5 depends on FR-1/FR-2 landing first so the single writer
encodes correct labels. FR-10's anomaly default depends on FR-4 (P1).

## 8. Open questions / risks

- **V3 payload growth (FR-7)**: appending **one** `centrality` f32 grows the
  per-node payload from 48 to 52 bytes — an **~8.3% bandwidth increase** on the
  highest-frequency stream (SSSP adds nothing; its slots already exist). Risk:
  position-streaming budget regression on large graphs. Mitigation candidates:
  send centrality on a separate lower-cadence channel rather than
  per-position-frame; or pack centrality at reduced precision (f16). Decision
  required before P2.
- **Auto-trigger cost (FR-9)**: even a correct Louvain at p95 < 5 s is
  expensive to run on every mutation. Open question: event-driven recompute vs
  fixed cadence vs adaptive (recompute only when edge-churn exceeds a
  threshold). Affects GPU contention with position integration.
- **Cluster-label stability across recomputes**: Louvain labels are not stable
  between runs (community IDs can permute). If the client colours by raw
  `cluster_id`, colours will flicker on every auto-recompute. Open question:
  does FR-5's single writer also own label-stabilisation (e.g. match new
  communities to previous by max overlap)? Likely a follow-on FR.
- **Dual-graph scope**: the audit covers the knowledge-graph. The ontology
  graph shares the same analytics actors. Open question: do analytics run on
  both graphs, and does `node_analytics` need to be keyed per graph? Current
  `HashMap<u32, ...>` assumes a single node-id space.
- **SSSP source selection (FR-8)**: who chooses the SSSP source — a fixed
  landmark, user click, or per-query? Affects whether the SSSP slot is a single
  value or needs to be query-scoped.
- **Removing the masking fallback (FR-10)**: the spatial-grid bucketing has
  shipped as the de-facto clustering for an unknown period. Risk: users may
  have come to rely on its visual output. Mitigation: keep it as a labelled
  opt-in (not delete it), and announce the default change.
