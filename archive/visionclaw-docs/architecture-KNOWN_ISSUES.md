# VisionClaw — Known Issues

> Last updated: 2026-06-03. Source of truth for open anomalies post-audit.
> Resolved items (T1/T2/T4/T5) are not listed here; see
> [`diagrams/00-anomaly-register.md`](diagrams/00-anomaly-register.md) for
> the full register including resolution detail.

---

## T6 — Binary-Protocol Shadow Crate (under investigation)

**Status:** deferred — dead today, footgun for future imports.

**Diagram:** [`diagrams/05-wire-analytics-types.md §C`](diagrams/05-wire-analytics-types.md)

The live binary encoder/decoder lives in `src/utils/binary_protocol.rs` and
encodes 52-byte V3 records (with centrality at offset @48). A shadow copy
exists in `crates/visionclaw-protocol/src/binary_protocol.rs` and encodes only
48-byte records (`WIRE_V3_ITEM_SIZE=48`, no centrality field). The shadow crate
was extracted as part of ADR-090 (`ddbeee3b` 2026-05-27, renamed `e600c8f4`
2026-05-28) and diverged from `src/` when centrality was added post-extraction.

**Current mitigant:** the shadow crate's analytics encoders/decoders have zero
external callers. Only `lib.rs:49-50` self-exports them. No `src/` code links
against the crate's encoders.

**Risk:** any future `use visionclaw_protocol::binary_protocol::encode_*` import
would silently produce 48-byte frames that the client decoder (expecting 52
bytes, stride 52) would misparse, corrupting all per-node analytics fields from
`@36` onwards.

**Recommended fix (reconcile-forward):** either delete the four dead 48-byte
functions from the shadow crate and add a CI guard preventing their
re-introduction, or bump the crate to 52 bytes and make `src/` delegate to it.
Do not revert the ADR-090 extraction.

**Files:** `crates/visionclaw-protocol/src/binary_protocol.rs` (shadow),
`src/utils/binary_protocol.rs` (live).

---

## T7 — Duplicate CUDA Force-Pass Kernel (under investigation)

**Status:** deferred — maintenance hazard, not user-facing.

**Diagram:** [`diagrams/06-gpu-physics.md`](diagrams/06-gpu-physics.md)

Two CUDA kernels implement force computation in
`unified_gpu_compute/visionclaw_unified.cu`:

- `force_pass_kernel` at line ~252 — plain force pass.
- `force_pass_with_stability_kernel` at line ~2029 — adds stability suppression.

Both are live; selection is controlled by `stability_threshold`. They duplicate
approximately 2,000 lines of PTX. Any bug fix or parameter change must be
mirrored in both kernels. Additionally, disc projection (apply/undo) is
independently re-implemented in the main loop (~line 1816–1831) and in the
`ForceFullBroadcast` handler (~line 2293–2335), applied and undone every step
(3× O(N) on the actor thread).

**Recommended fix:** merge the two force kernels via a single templated kernel
with a compile-time or runtime stability flag; refactor projection as a
GPU-side force rather than a CPU-side apply/undo. Not a drop-in change — the
Z-spring must be re-calibrated against the 56k cross-link springs.

**Files:** `src/unified_gpu_compute/visionclaw_unified.cu`.

---

## T8 — Dead Interaction Managers

**Status:** deferred — dead-code deletion, no user impact.

**Diagram:** [`diagrams/03-interaction-events.md`](diagrams/03-interaction-events.md)

Two interaction system files exist in the codebase with zero imports:

- `client/src/features/graph/services/InteractionManager.ts` (introduced
  `010b1925` 2025-12-25) — born dead, never wired.
- `client/src/features/graph/hooks/useNodeInteraction.ts` (introduced
  `ab6e6b67` 2025-09-25) — born dead beside the live handler.

The live interaction path is `useGraphEventHandlers.ts`.

Additionally, `animationStateRef.hoveredNode` is never written in the live
path, so hover-state is always null (diagram 03 §3).

**Recommended fix:** delete both dead files. Lint/coverage will confirm zero
references. No revert target exists — they were never live.

**Files:** `client/src/features/graph/services/InteractionManager.ts`,
`client/src/features/graph/hooks/useNodeInteraction.ts`.

---

## T5 Behavioural Note — Cluster-Write Timing

**Status:** noted (not a defect — documents expected behaviour post-T5 fix).

After the T5 resolution (2026-06-03), `POST /analytics/clustering/run` now
routes the finished `Vec<Cluster>` through `WriteClusterAnalytics →
GPUManagerActor → AnalyticsSupervisor → ClusteringActor` to write
`node_analytics.cluster_id`. This write happens asynchronously in a
`tokio::spawn` task after `perform_clustering()` returns.

The HTTP response (returning the cluster list) is sent from the spawn task
before the write-back to `node_analytics` completes. A client that reads the
next V3 binary frame immediately after the HTTP `200 OK` may still observe
`cluster_id = 0` for a brief window while the actor write propagates. This is
expected and harmless: the next broadcast cycle will carry the non-zero IDs.

**Diagram:** [`diagrams/07-analysis-clustering.md §3b`](diagrams/07-analysis-clustering.md)
