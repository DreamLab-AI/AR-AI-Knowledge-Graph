//! ADR-070 CUDA integration hardening — host-side (CPU) logic.
//!
//! This module carries the **testable host logic** for three ADR-070 items so
//! the GPU kernels stay a thin, well-understood shell:
//!
//! * **D2.2** — stability detection third criterion (constraint-force
//!   magnitude). [`evaluate_stability`] combines the existing kinetic-energy /
//!   active-node gate with a constraint-force gate, and
//!   [`max_node_constraint_force`] is a faithful CPU oracle of the per-node
//!   constraint-force magnitude the `force_pass_kernel` accumulates into
//!   `node_constraint_force` — the signal the GPU stability check reads.
//! * **D2.3** — input-edge NaN guard. [`partition_finite_constraints`] rejects
//!   any constraint that references a node with a non-finite position before it
//!   can enter the kernel from an upstream actor.
//! * **D3.1 (P2)** — sparse compute mask. [`build_compute_mask_with_neighbors`]
//!   builds the compacted list of node indices the masked force pass evaluates,
//!   including the 1-hop neighbours of every visible node (the coherence
//!   mitigation from ADR-070 §Risks so removing hidden nodes does not distort
//!   the force field felt by visible nodes). This is the CPU counterpart the
//!   Epic E.4 persona-masking path uploads to the device `compute_mask` buffer.
//!
//! All functions here are pure and free of any CUDA/`cust` dependency so they
//! run under a plain `cargo test` on hosts without a GPU. They operate on
//! primitive slices; callers adapt their richer types (`ConstraintData`,
//! graph nodes) to these inputs.

use std::collections::{BTreeSet, HashMap};

/// Live-kernel constraint discriminants (mirrors the `ConstraintKind` enum in
/// `visionclaw_unified.cu`). Only the kinds that produce a pairwise/position
/// layout force are modelled by the CPU oracle.
pub const KIND_DISTANCE: i32 = 0;
pub const KIND_POSITION: i32 = 1;
pub const KIND_SEPARATION: i32 = 6;

/// Minimal, CUDA-free view of a single constraint — the exact fields the force
/// kernel reads. `ConstraintData` (in the webxr monolith, which carries the
/// GPU-only `bytemuck`/`cust` derives) maps onto this field-for-field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstraintForceInput {
    /// `ConstraintKind` discriminant.
    pub kind: i32,
    /// Number of valid entries in `node_idx` (1–4).
    pub count: i32,
    /// Up to 4 node indices (index space must match `positions`).
    pub node_idx: [i32; 4],
    /// Up to 8 float parameters (params[0] = target/min distance, or target xyz
    /// for POSITION).
    pub params: [f32; 8],
    /// Blend weight.
    pub weight: f32,
}

impl ConstraintForceInput {
    /// The referenced node indices actually consulted by the kernel
    /// (`node_idx[0..count]`, clamped to the 4-slot array).
    pub fn referenced(&self) -> impl Iterator<Item = i32> + '_ {
        let n = self.count.clamp(0, 4) as usize;
        self.node_idx[..n].iter().copied()
    }
}

/// Magnitude of the constraint force a single constraint contributes to the
/// node at `role_idx` (the node's slot within `node_idx`). This mirrors the
/// `force_pass_kernel` math verbatim (progressive activation is treated as
/// fully ramped, i.e. multiplier = 1.0, which is the steady state a stability
/// check evaluates). Returns 0.0 when the constraint contributes nothing
/// (out-of-range partner, degenerate distance, separation beyond `min_dist`,
/// or a non-finite result — the kernel's `isfinite` guard).
fn single_contribution(
    c: &ConstraintForceInput,
    role_idx: usize,
    positions: &[[f32; 3]],
    max_force_per_node: f32,
    position_attraction: f32,
) -> f32 {
    let n = positions.len();
    let my = match positions.get(role_idx) {
        Some(p) => *p,
        None => return 0.0,
    };
    let dist3 = |a: [f32; 3], b: [f32; 3]| -> f32 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    };

    let mag = match c.kind {
        KIND_DISTANCE if c.count >= 2 => {
            let role = c.node_idx[..c.count.clamp(0, 4) as usize]
                .iter()
                .position(|&ix| ix as usize == role_idx);
            let other = match role {
                Some(0) => c.node_idx[1],
                Some(_) => c.node_idx[0],
                None => return 0.0,
            };
            if other < 0 || (other as usize) >= n {
                return 0.0;
            }
            let current = dist3(my, positions[other as usize]);
            let target = c.params[0];
            if current > 1e-6 && current.is_finite() && target > 0.0 {
                let error = current - target;
                let fm = -c.weight * error;
                fm.clamp(-max_force_per_node, max_force_per_node).abs()
            } else {
                0.0
            }
        }
        KIND_POSITION if c.count >= 1 => {
            let target = [c.params[0], c.params[1], c.params[2]];
            let distance = dist3(target, my);
            if distance > 1e-6 && distance.is_finite() {
                let fm = c.weight * distance * position_attraction;
                fm.min(max_force_per_node)
            } else {
                0.0
            }
        }
        KIND_SEPARATION if c.count >= 2 => {
            let role = c.node_idx[..c.count.clamp(0, 4) as usize]
                .iter()
                .position(|&ix| ix as usize == role_idx);
            let other = match role {
                Some(0) => c.node_idx[1],
                Some(_) => c.node_idx[0],
                None => return 0.0,
            };
            if other < 0 || (other as usize) >= n {
                return 0.0;
            }
            let current = dist3(my, positions[other as usize]);
            let min_dist = c.params[0];
            if current > 1e-6 && current.is_finite() && current < min_dist {
                let penetration = min_dist - current;
                (c.weight * penetration).min(max_force_per_node)
            } else {
                0.0
            }
        }
        _ => 0.0,
    };

    if mag.is_finite() {
        mag
    } else {
        0.0
    }
}

/// CPU oracle for the largest per-node constraint-force magnitude in the
/// system — the exact quantity the GPU accumulates into `node_constraint_force`
/// and the signal ADR-070 D2.2 adds as the stability third criterion.
///
/// `positions` is indexed by node index in the *same* space as the constraint
/// `node_idx` values (i.e. the GPU node-buffer index space). Returns 0.0 for an
/// empty system.
pub fn max_node_constraint_force(
    constraints: &[ConstraintForceInput],
    positions: &[[f32; 3]],
    max_force_per_node: f32,
    position_attraction: f32,
) -> f32 {
    let n = positions.len();
    if n == 0 {
        return 0.0;
    }
    let mut per_node = vec![0.0f32; n];
    for c in constraints {
        let cnt = c.count.clamp(0, 4) as usize;
        for &ix in &c.node_idx[..cnt] {
            if ix < 0 || (ix as usize) >= n {
                continue;
            }
            let role_idx = ix as usize;
            per_node[role_idx] += single_contribution(
                c,
                role_idx,
                positions,
                max_force_per_node,
                position_attraction,
            );
        }
    }
    per_node.into_iter().fold(0.0f32, f32::max)
}

/// ADR-070 D2.2 — the stability decision including the constraint-force third
/// criterion.
///
/// `ke_or_motion_stable` is the existing kinetic-energy-OR-active-node gate the
/// GPU stability kernel already computes. The system is stable **only** when
/// that gate holds *and* the largest constraint force is at or below
/// `epsilon`. A non-positive `epsilon` disables the third criterion (opt-out /
/// backwards-compatible), so it can never make a system that used to converge
/// suddenly report unstable.
pub fn evaluate_stability(
    ke_or_motion_stable: bool,
    max_constraint_force: f32,
    epsilon: f32,
) -> bool {
    if !ke_or_motion_stable {
        return false;
    }
    if epsilon <= 0.0 {
        return true;
    }
    // A non-finite constraint force is never "small": treat it as unstable.
    max_constraint_force.is_finite() && max_constraint_force <= epsilon
}

/// ADR-070 D2.3 — input-edge NaN guard.
///
/// Partitions constraint indices into `(kept, rejected)`. A constraint is
/// **rejected** when any node it references has a *known* non-finite position
/// (NaN or ±Inf) in `pos_by_id`. Constraints whose referenced ids are absent
/// from `pos_by_id` are kept (the guard only fires on positions it can actually
/// verify — an unresolved id is an upstream mapping concern, not a NaN, and is
/// dropped by the resolver, never fabricated here). Returns index lists so the
/// caller keeps ownership of its richer constraint objects.
pub fn partition_finite_constraints(
    constraints: &[ConstraintForceInput],
    pos_by_id: &HashMap<i32, [f32; 3]>,
) -> (Vec<usize>, Vec<usize>) {
    let mut kept = Vec::new();
    let mut rejected = Vec::new();
    for (i, c) in constraints.iter().enumerate() {
        let has_nonfinite = c.referenced().any(|id| {
            pos_by_id
                .get(&id)
                .map(|p| !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite()))
                .unwrap_or(false)
        });
        if has_nonfinite {
            rejected.push(i);
        } else {
            kept.push(i);
        }
    }
    (kept, rejected)
}

/// ADR-070 D3.1 (P2) — sparse compute-mask construction.
///
/// Given the node indices that are *visible* under the active persona / filter,
/// build the compacted, ascending list of node indices the masked force pass
/// must evaluate. The mask always includes each visible node **and its 1-hop
/// neighbours** (from the CSR adjacency `row_offsets` / `col_indices`), per the
/// ADR §Risks coherence mitigation: a hidden node still exerts force on a
/// visible neighbour, so it must remain in the compute set even though it is not
/// rendered.
///
/// `row_offsets` has length `num_nodes + 1`; `col_indices[row_offsets[v] ..
/// row_offsets[v+1]]` are the neighbours of node `v`. Out-of-range visible ids
/// and neighbour ids are ignored (defensive against malformed input). The
/// result is deduplicated and sorted ascending — the order the GPU compaction
/// kernel (`build_compute_mask_kernel`) also produces, so host-built and
/// device-built masks are interchangeable.
pub fn build_compute_mask_with_neighbors(
    visible: &[u32],
    row_offsets: &[i32],
    col_indices: &[i32],
    num_nodes: usize,
) -> Vec<i32> {
    let mut set: BTreeSet<i32> = BTreeSet::new();
    let have_csr = row_offsets.len() > num_nodes;
    for &v in visible {
        let v = v as usize;
        if v >= num_nodes {
            continue;
        }
        set.insert(v as i32);
        if !have_csr {
            continue;
        }
        let start = row_offsets[v];
        let end = row_offsets[v + 1];
        if start < 0 || end < start {
            continue;
        }
        let (start, end) = (start as usize, end as usize);
        if end > col_indices.len() {
            continue;
        }
        for &nb in &col_indices[start..end] {
            if nb >= 0 && (nb as usize) < num_nodes {
                set.insert(nb);
            }
        }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ci(kind: i32, ids: &[i32], params: &[f32], weight: f32) -> ConstraintForceInput {
        let mut node_idx = [0i32; 4];
        for (i, &id) in ids.iter().take(4).enumerate() {
            node_idx[i] = id;
        }
        let mut p = [0.0f32; 8];
        for (i, &v) in params.iter().take(8).enumerate() {
            p[i] = v;
        }
        ConstraintForceInput {
            kind,
            count: ids.len().min(4) as i32,
            node_idx,
            params: p,
            weight,
        }
    }

    // ---- D2.2: max_node_constraint_force + evaluate_stability ----

    #[test]
    fn distance_force_at_rest_is_zero() {
        // Two nodes exactly at target distance → error 0 → no force.
        let pos = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
        let cons = vec![ci(KIND_DISTANCE, &[0, 1], &[10.0], 1.0)];
        let m = max_node_constraint_force(&cons, &pos, 1e6, 0.1);
        assert!(m.abs() < 1e-6, "expected ~0, got {m}");
    }

    #[test]
    fn distance_force_stretched_pulls_with_weighted_error() {
        // current 20, target 10, weight 2 → |−2*(20−10)| = 20, both endpoints feel it.
        let pos = vec![[0.0, 0.0, 0.0], [20.0, 0.0, 0.0]];
        let cons = vec![ci(KIND_DISTANCE, &[0, 1], &[10.0], 2.0)];
        let m = max_node_constraint_force(&cons, &pos, 1e6, 0.1);
        assert!((m - 20.0).abs() < 1e-3, "expected 20, got {m}");
    }

    #[test]
    fn distance_force_is_capped_by_max_force_per_node() {
        let pos = vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0]];
        let cons = vec![ci(KIND_DISTANCE, &[0, 1], &[10.0], 5.0)];
        // uncapped would be 5*990 = 4950; cap at 50.
        let m = max_node_constraint_force(&cons, &pos, 50.0, 0.1);
        assert!((m - 50.0).abs() < 1e-3, "expected cap 50, got {m}");
    }

    #[test]
    fn separation_is_one_sided_zero_beyond_min_dist() {
        // 5 apart, min 3 → beyond clamp radius → no push.
        let pos = vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]];
        let cons = vec![ci(KIND_SEPARATION, &[0, 1], &[3.0], 1.0)];
        assert_eq!(max_node_constraint_force(&cons, &pos, 1e6, 0.1), 0.0);
    }

    #[test]
    fn separation_pushes_inside_min_dist() {
        // 1 apart, min 4, weight 2 → penetration 3 → 6.
        let pos = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let cons = vec![ci(KIND_SEPARATION, &[0, 1], &[4.0], 2.0)];
        let m = max_node_constraint_force(&cons, &pos, 1e6, 0.1);
        assert!((m - 6.0).abs() < 1e-3, "expected 6, got {m}");
    }

    #[test]
    fn max_is_taken_over_all_nodes() {
        // Node 1 is in two stretched constraints; its summed magnitude dominates.
        let pos = vec![[0.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 40.0, 0.0]];
        let cons = vec![
            ci(KIND_DISTANCE, &[0, 1], &[10.0], 1.0), // node1 gets 10
            ci(KIND_DISTANCE, &[1, 2], &[10.0], 1.0), // node1 gets |−(40−10)|=30
        ];
        // node1 total = 10 + 30 = 40, the max.
        let m = max_node_constraint_force(&cons, &pos, 1e6, 0.1);
        assert!((m - 40.0).abs() < 1e-3, "expected 40, got {m}");
    }

    #[test]
    fn evaluate_stability_third_criterion() {
        // KE-stable but constraint force above epsilon → NOT stable.
        assert!(!evaluate_stability(true, 5.0, 1.0));
        // KE-stable and constraint force below epsilon → stable.
        assert!(evaluate_stability(true, 0.5, 1.0));
        // Not KE-stable → never stable regardless of forces.
        assert!(!evaluate_stability(false, 0.0, 1.0));
        // epsilon <= 0 disables the criterion (backwards-compatible).
        assert!(evaluate_stability(true, 999.0, 0.0));
        // non-finite constraint force is never "small".
        assert!(!evaluate_stability(true, f32::NAN, 1.0));
        assert!(!evaluate_stability(true, f32::INFINITY, 1.0));
    }

    // ---- D2.3: partition_finite_constraints ----

    #[test]
    fn nan_guard_rejects_constraint_touching_nonfinite_node() {
        let mut pos = HashMap::new();
        pos.insert(10, [0.0, 0.0, 0.0]);
        pos.insert(11, [f32::NAN, 0.0, 0.0]); // bad
        pos.insert(12, [1.0, 2.0, 3.0]);
        let cons = vec![
            ci(KIND_DISTANCE, &[10, 12], &[10.0], 1.0),  // ok
            ci(KIND_DISTANCE, &[10, 11], &[10.0], 1.0),  // touches NaN → reject
            ci(KIND_SEPARATION, &[11, 12], &[3.0], 1.0), // touches NaN → reject
        ];
        let (kept, rejected) = partition_finite_constraints(&cons, &pos);
        assert_eq!(kept, vec![0]);
        assert_eq!(rejected, vec![1, 2]);
    }

    #[test]
    fn nan_guard_rejects_inf_and_keeps_unknown_ids() {
        let mut pos = HashMap::new();
        pos.insert(1, [f32::INFINITY, 0.0, 0.0]);
        // id 2 is absent → unknown, kept; id 1 is Inf → rejected.
        let cons = vec![
            ci(KIND_DISTANCE, &[2, 3], &[10.0], 1.0),
            ci(KIND_DISTANCE, &[1, 2], &[10.0], 1.0),
        ];
        let (kept, rejected) = partition_finite_constraints(&cons, &pos);
        assert_eq!(kept, vec![0]);
        assert_eq!(rejected, vec![1]);
    }

    // ---- D3.1: build_compute_mask_with_neighbors ----

    #[test]
    fn mask_includes_visible_and_one_hop_neighbours() {
        // Path graph 0-1-2-3-4 (undirected CSR).
        // row_offsets/col_indices for adjacency:
        // 0:[1] 1:[0,2] 2:[1,3] 3:[2,4] 4:[3]
        let row = vec![0, 1, 3, 5, 7, 8];
        let col = vec![1, 0, 2, 1, 3, 2, 4, 3];
        let mask = build_compute_mask_with_neighbors(&[2], &row, &col, 5);
        // node 2 + neighbours 1,3
        assert_eq!(mask, vec![1, 2, 3]);
    }

    #[test]
    fn mask_dedups_and_sorts_across_visible_set() {
        let row = vec![0, 1, 3, 5, 7, 8];
        let col = vec![1, 0, 2, 1, 3, 2, 4, 3];
        let mask = build_compute_mask_with_neighbors(&[1, 3], &row, &col, 5);
        // 1 + {0,2}, 3 + {2,4} → {0,1,2,3,4}
        assert_eq!(mask, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn mask_ignores_out_of_range_visible_ids() {
        let row = vec![0, 1, 3, 5, 7, 8];
        let col = vec![1, 0, 2, 1, 3, 2, 4, 3];
        let mask = build_compute_mask_with_neighbors(&[99, 0], &row, &col, 5);
        // only node 0 (+neighbour 1) survives.
        assert_eq!(mask, vec![0, 1]);
    }

    #[test]
    fn mask_without_csr_is_just_visible_nodes() {
        // Empty/short CSR → no neighbour expansion, visible-only.
        let mask = build_compute_mask_with_neighbors(&[2, 0], &[], &[], 5);
        assert_eq!(mask, vec![0, 2]);
    }

    #[test]
    fn mask_empty_visible_is_empty() {
        let row = vec![0, 1, 3, 5, 7, 8];
        let col = vec![1, 0, 2, 1, 3, 2, 4, 3];
        assert!(build_compute_mask_with_neighbors(&[], &row, &col, 5).is_empty());
    }
}
