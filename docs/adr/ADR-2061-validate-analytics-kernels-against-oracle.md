---
id: ADR-2061
title: Validate the GPU analytics kernels against the CPU reference oracle
date: 2026-09-05
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: Before any claim that Louvain, PageRank or DBSCAN output is trustworthy, or before a release that surfaces community/centrality/cluster values to users
repo: visionclaw
---

# ADR-2061 — Validate the GPU analytics kernels against the CPU reference oracle

## Context

`docs/GPU-wire-abi.md` "Analytics kernel trust status" records that Louvain, PageRank and
DBSCAN carry in-source fix markers but that **no reference-implementation benchmark has
confirmed their outputs**: the fixes are code-verified, not output-verified. Those values
are not internal — they reach clients on the wire at V3 offsets `cluster_id@36`,
`anomaly_score@40`, `community_id@44` and `centrality@48`, so a wrong kernel is a wrong
pixel and a wrong answer to a user-facing query.

The comparison basis already exists and is unused for this purpose.
`crates/visionclaw-analytics-oracle` is a dependency-free CPU reference crate containing
`modularity` (`:303`), `pagerank` (`:342`), `dbscan` (`:392`) and `lof` (`:443`), plus
deterministic graph fixtures — `two_clique` (`:192`), `triangle` (`:206`), `star` (`:215`),
`linear_chain` (`:225`) and `canonical_live_scale` (`:246`) — and
`two_clique_optimal_partition` (`:502`), which is a known-correct answer rather than merely
a comparison. Diagram VC-15.13 carries the divergence note.

This is proposed rather than accepted because closing it is a benchmark and a numerical
tolerance policy, not an edit — it exceeds the bounded scope Phase 2 sets for a FIX.

## Decision

A conformance test suite compares each GPU analytics kernel against the CPU oracle on the
oracle's own fixtures, and the trust table in `GPU-wire-abi.md` records a kernel as trusted
only once its conformance test passes.

Per kernel, the acceptance test is:

- **Louvain / community detection** — on `two_clique`, the partition must have exactly 2
  communities and match `two_clique_optimal_partition` up to label permutation; on `triangle`
  and `star`, exactly 1. On `canonical_live_scale`, GPU modularity must be within `0.02`
  absolute of the oracle's `modularity` for the GPU's own partition, and must not be lower
  than the oracle partition's modularity by more than `0.05`. Louvain is stochastic, so the
  test asserts on modularity quality and community count, never on exact labels.
- **PageRank** — on every fixture, per-node absolute difference from `pagerank(g, 0.85, 100)`
  must be `< 1e-4`, and the ranking order of the top decile must match exactly. Damping and
  iteration count are pinned to the oracle's.
- **DBSCAN** — for pinned `eps`/`min_pts`, the GPU labelling must match `dbscan` exactly up to
  cluster-label permutation, including which points are noise. DBSCAN is deterministic, so
  exact agreement is the bar.
- **LOF / anomaly** — per-point absolute difference from `lof(points, k)` must be `< 1e-3`,
  and the set of points above the 95th percentile must match exactly.

A kernel that fails is marked BROKEN in the trust table and its wire slot publishes a
documented sentinel rather than a plausible-looking wrong number.

## Consequences

Once landed, the trust table stops being an assertion of intent and becomes a test result,
and the "code-fixed but not output-validated" divergence closes for real rather than by
assertion. The suite also becomes the regression gate for any future kernel change, which is
the larger long-term value.

The costs are real: the tests need a GPU in CI or a documented skip, `canonical_live_scale`
makes them slower than a unit test, and the tolerances above are engineering judgement that
will need adjustment on first contact — the numbers are stated here so that adjustment is a
visible amendment to this ADR rather than a quiet edit to a test.

Until this lands, `GPU-wire-abi.md` keeps its trust-status caveat and diagram VC-15.13 keeps
a `PROPOSED ADR-2061:` note. The existing process-global counters
(`analytics_telemetry::snapshot()` / `total_cpu_fallbacks()`, surfaced unconditionally by
`/analytics/gpu-metrics`) remain the only live signal, and they measure execution path, not
correctness — a kernel can run entirely on the GPU and still be wrong.

## Verification

None — `implementation_status: none`. The oracle's contents were verified to exist and to be
suitable as a comparison basis at the working tree above
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`:
`grep -n "pub fn " crates/visionclaw-analytics-oracle/src/lib.rs` lists `modularity:303`,
`pagerank:342`, `dbscan:392`, `lof:443`, `two_clique:192`, `triangle:206`, `star:215`,
`linear_chain:225`, `canonical_live_scale:246`, `two_clique_optimal_partition:502` and
`distinct_communities:510`.

The APSP kernel is deliberately excluded from this suite: it is `#if 0`-quarantined
(`gpu_landmark_apsp.cu:25`, `#endif` at `:65`) and refused at
`src/actors/gpu/shortest_path_actor.rs:353-360` under NFR-7, so there is no live output to
validate.

Acceptance for this ADR: the suite exists, runs in CI (or documents its GPU skip), and every
kernel listed in the `GPU-wire-abi.md` trust table has a passing or explicitly-failing entry
traceable to a test name.
