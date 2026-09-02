---
id: ADR-2029
title: Force channels map over the flat struct; execution.rs is the flag authority and ENABLE_CONSTRAINTS is residency-owned
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 9423abdb37e5a7a59840a06dc587fd423b8c9e53
verified_paths: [src/models/force_channels.rs, src/utils/unified_gpu_compute/execution.rs, src/models/simulation_params.rs]
owner: jjohare
review_trigger: array-backed force-term refactor (deferred step 2), or any new host→GPU conversion path
repo: visionclaw
domain: GPU-wire-abi
lineage: legacy ADR-098 (keystone residency wire — flip the already-allocated bit from residency; 18,933 constraints live), ADR-138 (mapping-layer-now / array-backed-later + guarded Constraints round-trip), ADR-141 (phased programme consuming the seam).
---

# ADR-2029 — Force channels map over the flat struct; execution.rs is the flag authority and ENABLE_CONSTRAINTS is residency-owned

## Context
The ten force terms are scattered scalars plus feature-flag bits inside the flat
SimParams (ADR-2028). A future refactor wants them array-backed, but shipping that
now would churn the ABI. Three separate host→GPU conversion paths derive the same
`repulsion/spring/centering` enable bits from the `scalar > 0` rule, risking drift.
Constraints are residency-owned: the bit means "constraints are resident", set from
`num_constraints > 0` (18,933 live per ADR-098), not a user preference. A naive UI
toggle on it would fight the residency system every tick.

## Decision
Force terms are exposed through a bounded `ForceChannel` enum that maps each channel
to its existing scalar + flag — a view/mutator seam, not an array rewrite (the
array-backed step 2 is deliberately not built). `execute_physics_step` is the single
flag authority: it rebuilds `feature_flags` from scratch, calls `to_sim_params()`, then
overwrites the flag word — setting `ENABLE_CONSTRAINTS` iff `num_constraints > 0` and
the runtime SSSP bit. Consequently `Constraints` is a read-only channel: its `apply()`
is a no-op, so any UI constraints toggle is inert by design. This forecloses letting
the registry own constraint enablement and forecloses a second flag-deriving authority.

## Consequences
Flag derivation has one owner, so paths cannot disagree on the constraint or SSSP bit,
and the seam lets the array-backed refactor land later touching only channel bodies.
The costs: `Constraints.apply()` silently doing nothing surprises anyone expecting the
registry to gate it; the three conversion paths still each derive the simple enable bits
(only the authoritative path adds the residency/runtime bits); and the deferred step 2
means the "array-backed" promise is documented but unshipped. A UI author must know the
constraints channel is diagnostic-read-only.

## Verification
Re-checked at e0f8cd896: `src/models/force_channels.rs` — `apply()` returns early when
`is_read_only()` (lines 191–203), `is_read_only` matches only `Constraints` (lines 210–212),
and tests `only_constraints_is_read_only` / `constraints_apply_is_a_noop` (from line ~372).
`src/utils/unified_gpu_compute/execution.rs` — `execute_physics_step` rebuilds
`feature_flags` from 0 (line 884), sets `ENABLE_CONSTRAINTS` when `num_constraints > 0`
(lines 903–904), calls `to_sim_params()` then overwrites the word (lines 912–913).
`From`/`to_sim_params` impls at `simulation_params.rs:222,236,319`. Mapping layer present;
array-backed refactor absent — implementation is partial as recorded.
Re-verified at `542d63d1d` after the ADR-141 formatting sweep (test-only line
wrapping of a `constraint_max_force_per_node` assertion in `force_channels.rs`) —
the flag-derivation semantics are unchanged.
