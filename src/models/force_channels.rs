//! Named force-channel registry (PHASE 3, mapping layer).
//!
//! # Why this exists
//!
//! The GPU [`SimParams`] struct is a flat, `repr(C)`, 180-byte record whose
//! layout is mirrored byte-for-byte by the CUDA `SimParams` (guarded by a
//! `static_assert`/`const assert` pair on both sides) and whose camelCase fields
//! are the wire contract four shipping clients already speak. Turning it into a
//! literal enum-indexed array of `{enabled, strength}` channels — the eventual
//! architecture — would touch the struct layout, every kernel that reads
//! `c_params.*`, and the settings wire simultaneously. That is too invasive for a
//! single safe pass (see the Phase 3 audit).
//!
//! This module delivers the **mapping layer** for that future refactor without
//! any of the risk: a bounded [`ForceChannel`] enum and a view/mutator that map
//! each channel to the *existing* scalar field(s) and feature-flag bit(s) on
//! `SimParams`. Nothing here changes the struct layout, the kernels, or the wire.
//! It gives the rest of the codebase one enumerable source of truth for "what
//! force channels exist, are they on, and how strong are they", reading and
//! writing through the current representation. When the real array-backed refactor
//! lands, only the bodies of [`ForceChannel::state`] / [`ForceChannel::apply`]
//! change; every caller keeps working.
//!
//! # Channel audit (force terms in the live kernels, 2026-08-30)
//!
//! Terms in `force_pass_kernel` / `force_pass_with_stability_kernel`:
//!   * Repulsion — `repel_k` (or FA2 `scaling_ratio`), gated `ENABLE_REPULSION`.
//!   * Separation — short-range hard push, `separation_radius` + `max_force`.
//!   * Springs — `spring_k` (+ `rest_length`, `spring_scale`, `sssp_alpha`,
//!     LinLog), gated `ENABLE_SPRINGS`.
//!   * Centering — `center_gravity_k`, gated `ENABLE_CENTERING`.
//!   * Constraints — ontology `ConstraintData`, gated `ENABLE_CONSTRAINTS`,
//!     ramp-capped by `constraint_max_force_per_node`.
//!   * DAG radial bias (Phase 2) — `dag_bias_k` + `dag_level_distance`, self-gated
//!     on `dag_bias_k > 0`.
//! Terms in `integrate_pass_kernel`:
//!   * Boundary — soft push, `viewport_bounds` + `boundary_damping`.
//!   * Annealing — velocity jitter, `temperature` + `cooling_rate`.
//! Separate kernels:
//!   * Gravity — `degree_weighted_gravity_kernel`, `gravity`.
//!   * Cluster cohesion — `cluster_cohesion_kernel`, `cluster_strength`.
//!
//! Each of those is exposed here as a [`ForceChannel`]. The registry is
//! **bounded**: adding a force term to the kernels means adding a variant here,
//! which the exhaustive `match`es make impossible to forget.

use crate::models::simulation_params::{FeatureFlags, SimParams};

/// A named, togglable force term in the layout engine. Bounded set — one variant
/// per force term the CUDA kernels evaluate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForceChannel {
    /// Pairwise repulsion (classic inverse-square or FA2 degree-scaled).
    Repulsion,
    /// Short-range hard separation push (min-distance overlap resolution).
    Separation,
    /// Edge spring attraction (Hooke / LinLog).
    Spring,
    /// Pull toward the world origin (center gravity).
    Centering,
    /// Degree-weighted gravity toward the origin.
    Gravity,
    /// Cluster cohesion toward community centroids.
    ClusterCohesion,
    /// Ontology constraint forces (subclass/disjoint/position/…). READ-ONLY in the
    /// registry: its `ENABLE_CONSTRAINTS` flag is rebuilt from constraint
    /// residency (`num_constraints > 0`) every physics step, and its backing
    /// scalar is a ramp cap rather than an on/off strength, so [`apply`] is a
    /// no-op for it (see [`is_read_only`]). Enablement is owned by residency.
    Constraints,
    /// DAG radial hierarchy bias (Phase 2, radialout shells).
    DagRadialBias,
    /// Simulated-annealing velocity jitter.
    Annealing,
    /// Soft world-boundary containment push.
    Boundary,
}

/// The enabled/strength view of a single channel. This is the shape the future
/// array-backed `SimParams` will store directly; today it is *derived* from the
/// flat scalar fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForceChannelState {
    /// Whether the term contributes any force this step.
    pub enabled: bool,
    /// The term's primary scalar coefficient (its meaning is channel-specific —
    /// e.g. a stiffness for `Spring`, a radius for `Separation`). See the audit.
    pub strength: f32,
}

impl ForceChannel {
    /// Every channel, in a stable order. Iterating this is the enumerable
    /// "registry" the flat struct otherwise lacks.
    pub const ALL: [ForceChannel; 10] = [
        ForceChannel::Repulsion,
        ForceChannel::Separation,
        ForceChannel::Spring,
        ForceChannel::Centering,
        ForceChannel::Gravity,
        ForceChannel::ClusterCohesion,
        ForceChannel::Constraints,
        ForceChannel::DagRadialBias,
        ForceChannel::Annealing,
        ForceChannel::Boundary,
    ];

    /// Stable, lowercase identifier for logging / diagnostics / a future
    /// registry wire surface. Not a settings key — the settings wire keeps its
    /// existing per-field camelCase names.
    pub const fn key(self) -> &'static str {
        match self {
            ForceChannel::Repulsion => "repulsion",
            ForceChannel::Separation => "separation",
            ForceChannel::Spring => "spring",
            ForceChannel::Centering => "centering",
            ForceChannel::Gravity => "gravity",
            ForceChannel::ClusterCohesion => "clusterCohesion",
            ForceChannel::Constraints => "constraints",
            ForceChannel::DagRadialBias => "dagRadialBias",
            ForceChannel::Annealing => "annealing",
            ForceChannel::Boundary => "boundary",
        }
    }

    /// The `FeatureFlags` bit that gates this channel, if it is flag-gated.
    /// Channels with no flag are gated purely by their strength being > 0.
    pub const fn feature_flag(self) -> Option<u32> {
        match self {
            ForceChannel::Repulsion => Some(FeatureFlags::ENABLE_REPULSION),
            ForceChannel::Spring => Some(FeatureFlags::ENABLE_SPRINGS),
            ForceChannel::Centering => Some(FeatureFlags::ENABLE_CENTERING),
            ForceChannel::Constraints => Some(FeatureFlags::ENABLE_CONSTRAINTS),
            ForceChannel::Separation
            | ForceChannel::Gravity
            | ForceChannel::ClusterCohesion
            | ForceChannel::DagRadialBias
            | ForceChannel::Annealing
            | ForceChannel::Boundary => None,
        }
    }

    /// Read this channel's `{enabled, strength}` view out of the flat `SimParams`.
    ///
    /// For flag-gated channels `enabled` reflects the feature-flag bit; for the
    /// rest it reflects `strength > 0` (the same test the kernels apply). This is
    /// the single place that knows which scalar backs each channel.
    pub fn state(self, p: &SimParams) -> ForceChannelState {
        let strength = self.strength_of(p);
        let enabled = match self.feature_flag() {
            Some(bit) => p.feature_flags & bit != 0,
            None => strength > 0.0,
        };
        ForceChannelState { enabled, strength }
    }

    /// The backing scalar for this channel.
    fn strength_of(self, p: &SimParams) -> f32 {
        match self {
            ForceChannel::Repulsion => p.repel_k,
            ForceChannel::Separation => p.separation_radius,
            ForceChannel::Spring => p.spring_k,
            ForceChannel::Centering => p.center_gravity_k,
            ForceChannel::Gravity => p.gravity,
            ForceChannel::ClusterCohesion => p.cluster_strength,
            ForceChannel::Constraints => p.constraint_max_force_per_node,
            ForceChannel::DagRadialBias => p.dag_bias_k,
            ForceChannel::Annealing => p.temperature,
            ForceChannel::Boundary => p.viewport_bounds,
        }
    }

    /// Write this channel's `{enabled, strength}` back into the flat `SimParams`,
    /// preserving exactly the semantics the kernels expect:
    ///
    /// * The backing scalar is set to `strength` when enabled, or forced to `0.0`
    ///   when disabled (so a non-flag-gated channel truly goes inert).
    /// * For flag-gated channels the feature-flag bit is set when
    ///   `enabled && strength > 0`, else cleared — mirroring how
    ///   `SimParams::from(&SimulationParams)` derives the flags from the scalars.
    ///
    /// Round-trips with [`state`](Self::state): reading a channel and writing it
    /// straight back leaves an equivalent `SimParams` (see the unit tests).
    pub fn apply(self, p: &mut SimParams, s: ForceChannelState) {
        // Constraints is RESIDENCY-DRIVEN and read-only here (see the variant doc
        // and `is_read_only`). The physics step rebuilds `ENABLE_CONSTRAINTS` from
        // `num_constraints > 0` on every launch (execution.rs), so any flag we set
        // is immediately overwritten; and its backing scalar
        // `constraint_max_force_per_node` is a per-node ramp CAP, not an on/off
        // strength — zeroing it would silently change constraint force semantics.
        // Enablement is therefore owned by constraint residency, not the registry.
        if self.is_read_only() {
            return;
        }
        let value = if s.enabled { s.strength } else { 0.0 };
        self.set_strength(p, value);
        if let Some(bit) = self.feature_flag() {
            if s.enabled && value > 0.0 {
                p.feature_flags |= bit;
            } else {
                p.feature_flags &= !bit;
            }
        }
    }

    /// Whether [`apply`](Self::apply) is a no-op for this channel because its
    /// enablement is owned elsewhere. Only `Constraints` is read-only: its
    /// `ENABLE_CONSTRAINTS` flag is rebuilt every physics step from constraint
    /// residency (`num_constraints > 0`), so the registry cannot meaningfully
    /// toggle it. `state()` still reports it faithfully for reading/diagnostics.
    pub const fn is_read_only(self) -> bool {
        matches!(self, ForceChannel::Constraints)
    }

    fn set_strength(self, p: &mut SimParams, value: f32) {
        match self {
            ForceChannel::Repulsion => p.repel_k = value,
            ForceChannel::Separation => p.separation_radius = value,
            ForceChannel::Spring => p.spring_k = value,
            ForceChannel::Centering => p.center_gravity_k = value,
            ForceChannel::Gravity => p.gravity = value,
            ForceChannel::ClusterCohesion => p.cluster_strength = value,
            ForceChannel::Constraints => p.constraint_max_force_per_node = value,
            ForceChannel::DagRadialBias => p.dag_bias_k = value,
            ForceChannel::Annealing => p.temperature = value,
            ForceChannel::Boundary => p.viewport_bounds = value,
        }
    }
}

/// Snapshot the full channel registry from a `SimParams` — one `{enabled,
/// strength}` per [`ForceChannel::ALL`], in that order. Useful for diagnostics
/// and as the seam the future array-backed struct will expose natively.
pub fn snapshot(p: &SimParams) -> [(ForceChannel, ForceChannelState); 10] {
    ForceChannel::ALL.map(|ch| (ch, ch.state(p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_channels_have_unique_keys() {
        let mut keys: Vec<&str> = ForceChannel::ALL.iter().map(|c| c.key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "channel keys must be unique");
    }

    #[test]
    fn flag_gated_channels_report_flag_state() {
        // Default params enable repulsion/spring/centering via their positive
        // scalars (SimParams::new derives flags from PhysicsSettings defaults).
        let p = SimParams::new();
        // Repulsion is flag-gated: its enabled must track the flag bit, not just
        // the scalar sign.
        let st = ForceChannel::Repulsion.state(&p);
        assert_eq!(
            st.enabled,
            p.feature_flags & FeatureFlags::ENABLE_REPULSION != 0
        );
        assert_eq!(st.strength, p.repel_k);
    }

    #[test]
    fn non_flag_channel_enabled_tracks_positive_strength() {
        let mut p = SimParams::new();
        p.dag_bias_k = 0.0;
        assert!(!ForceChannel::DagRadialBias.state(&p).enabled);
        p.dag_bias_k = 2.5;
        let st = ForceChannel::DagRadialBias.state(&p);
        assert!(st.enabled);
        assert_eq!(st.strength, 2.5);
    }

    #[test]
    fn disabling_a_channel_zeroes_scalar_and_clears_flag() {
        let mut p = SimParams::new();
        p.repel_k = 100.0;
        p.feature_flags |= FeatureFlags::ENABLE_REPULSION;
        ForceChannel::Repulsion.apply(
            &mut p,
            ForceChannelState {
                enabled: false,
                strength: 100.0,
            },
        );
        assert_eq!(p.repel_k, 0.0, "disabled scalar must be zeroed");
        assert_eq!(
            p.feature_flags & FeatureFlags::ENABLE_REPULSION,
            0,
            "disabled flag must be cleared"
        );
    }

    #[test]
    fn enabling_a_channel_sets_scalar_and_flag() {
        let mut p = SimParams::new();
        p.spring_k = 0.0;
        p.feature_flags &= !FeatureFlags::ENABLE_SPRINGS;
        ForceChannel::Spring.apply(
            &mut p,
            ForceChannelState {
                enabled: true,
                strength: 3.0,
            },
        );
        assert_eq!(p.spring_k, 3.0);
        assert_ne!(p.feature_flags & FeatureFlags::ENABLE_SPRINGS, 0);
    }

    #[test]
    fn enabled_but_zero_strength_leaves_flag_clear() {
        // A flag-gated channel enabled with strength 0 contributes nothing, so the
        // flag stays clear — matching SimParams::from's `> 0.0` flag derivation.
        let mut p = SimParams::new();
        ForceChannel::Centering.apply(
            &mut p,
            ForceChannelState {
                enabled: true,
                strength: 0.0,
            },
        );
        assert_eq!(p.center_gravity_k, 0.0);
        assert_eq!(p.feature_flags & FeatureFlags::ENABLE_CENTERING, 0);
    }

    #[test]
    fn state_apply_roundtrip_preserves_effective_state_for_all_channels() {
        // Reading every channel and writing it straight back must preserve the
        // channel's EFFECTIVE state — the core mapping-layer invariant. `enabled`
        // must always match; `strength` must match while enabled. A DISABLED
        // channel's stored strength is immaterial to the kernels (the term is
        // gated off), and `apply` deliberately zeroes it, so it is excluded from
        // the comparison — that zeroing is the intended normalisation, not a bug.
        let original = SimParams::new();
        let mut p = original;
        for ch in ForceChannel::ALL {
            let s = ch.state(&original);
            ch.apply(&mut p, s);
        }
        for ch in ForceChannel::ALL {
            let before = ch.state(&original);
            let after = ch.state(&p);
            assert_eq!(after.enabled, before.enabled, "roundtrip flipped {:?}", ch);
            if before.enabled {
                assert_eq!(
                    after.strength, before.strength,
                    "roundtrip changed enabled strength of {:?}",
                    ch
                );
            }
        }
    }

    #[test]
    fn disabled_channel_with_positive_scalar_stays_disabled_after_roundtrip() {
        // Regression guard for the Constraints-style case: flag-off at rest but a
        // positive backing scalar. state() reports disabled; a state→apply
        // roundtrip must keep it disabled (and inert) rather than accidentally
        // enabling it.
        let mut p = SimParams::new();
        p.constraint_max_force_per_node = 50.0;
        p.feature_flags &= !FeatureFlags::ENABLE_CONSTRAINTS;
        let s = ForceChannel::Constraints.state(&p);
        assert!(!s.enabled);
        ForceChannel::Constraints.apply(&mut p, s);
        assert!(!ForceChannel::Constraints.state(&p).enabled);
        assert_eq!(p.feature_flags & FeatureFlags::ENABLE_CONSTRAINTS, 0);
    }

    #[test]
    fn only_constraints_is_read_only() {
        for ch in ForceChannel::ALL {
            assert_eq!(ch.is_read_only(), ch == ForceChannel::Constraints);
        }
    }

    #[test]
    fn constraints_apply_is_a_noop() {
        // Constraints is residency-driven: apply must not touch the scalar or flag.
        let mut p = SimParams::new();
        p.constraint_max_force_per_node = 42.0;
        p.feature_flags |= FeatureFlags::ENABLE_CONSTRAINTS;
        // Try to disable it via the registry — must have no effect.
        ForceChannel::Constraints.apply(
            &mut p,
            ForceChannelState {
                enabled: false,
                strength: 0.0,
            },
        );
        assert_eq!(
            p.constraint_max_force_per_node, 42.0,
            "scalar must be untouched"
        );
        assert_ne!(
            p.feature_flags & FeatureFlags::ENABLE_CONSTRAINTS,
            0,
            "residency flag must be untouched"
        );
    }

    #[test]
    fn snapshot_covers_every_channel_in_order() {
        let p = SimParams::new();
        let snap = snapshot(&p);
        assert_eq!(snap.len(), ForceChannel::ALL.len());
        for (i, ch) in ForceChannel::ALL.iter().enumerate() {
            assert_eq!(snap[i].0, *ch);
        }
    }
}
