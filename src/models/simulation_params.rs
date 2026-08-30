//! Simulation parameters — re-exported from `visionclaw-domain` per ADR-090.
//!
//! The framework-agnostic data shapes (SimulationParams, SettleMode,
//! SimulationMode, SimulationPhase, FeatureFlags) live in the domain crate.
//! This module retains the GPU-aligned `SimParams` struct and the conversion
//! impls that depend on `crate::config::dev_config` — those cannot live in
//! the domain crate because they pull in CUDA/runtime config.

use bytemuck::{Pod, Zeroable};
use cudarc::driver::DeviceRepr;
use cust_core::DeviceCopy;

// Re-export the domain-owned shapes so existing
// `use crate::models::simulation_params::SimulationParams` imports keep working.
pub use visionclaw_domain::models::simulation_params::{
    FeatureFlags, SettleMode, SimulationMode, SimulationParams, SimulationPhase,
};

use visionclaw_domain::types::layout::LayoutMode;
use visionclaw_domain::types::physics_config::{
    AutoBalanceConfig, AutoPauseConfig, PhysicsSettings,
};

// GPU-aligned simulation parameters. Mirrors the CUDA `SimParams` struct;
// must match its size and layout exactly (see `const _:()` assertion below).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SimParams {
    pub dt: f32,
    pub damping: f32,
    pub warmup_iterations: u32,
    pub cooling_rate: f32,

    pub spring_k: f32,
    pub rest_length: f32,

    pub repel_k: f32,
    pub repulsion_cutoff: f32,
    pub repulsion_softening_epsilon: f32,

    pub center_gravity_k: f32,
    pub max_force: f32,
    pub max_velocity: f32,

    pub grid_cell_size: f32,

    pub feature_flags: u32,
    pub seed: u32,
    pub iteration: i32,

    pub separation_radius: f32,
    pub cluster_strength: f32,
    pub alignment_strength: f32,
    pub temperature: f32,
    pub viewport_bounds: f32,
    pub sssp_alpha: f32,
    pub boundary_damping: f32,

    pub constraint_ramp_frames: u32,
    pub constraint_max_force_per_node: f32,

    pub stability_threshold: f32,
    pub min_velocity_threshold: f32,

    pub world_bounds_min: f32,
    pub world_bounds_max: f32,
    pub cell_size_lod: f32,
    pub k_neighbors_max: u32,
    pub anomaly_detection_radius: f32,
    pub learning_rate_default: f32,

    pub norm_delta_cap: f32,
    pub position_constraint_attraction: f32,
    pub lof_score_min: f32,
    pub lof_score_max: f32,
    pub weight_precision_multiplier: f32,
    // Stress majorization params live on CPU (SemanticProcessorActor); not in GPU SimParams.
    /// Gravity pull toward origin. Added at end to preserve repr(C) layout.
    pub gravity: f32,

    // ForceAtlas2 / LinLog parameters
    pub lin_log_mode: u32,
    pub scaling_ratio: f32,
    pub adaptive_speed: u32,
    pub global_speed: f32,

    // DAG radial hierarchy bias (PHASE 2). `dag_bias_k` = 0 disables the term.
    // Added at the end to preserve the existing repr(C) prefix layout.
    pub dag_bias_k: f32,
    pub dag_level_distance: f32,

    // Layout mode selector (ADR-141 P1). GPU-visible discriminant of the active
    // `LayoutMode` (0=ForceDirected … 5=Clustered). Mirrors the CUDA field added at
    // the tail of `SimParams`; the seam P2–P4 branch on. Added at the end to
    // preserve the existing repr(C) prefix layout.
    pub layout_mode: u32,

    // ADR-141 P2 stratified planes. `plane_bias_k` = 0 disables the term.
    // Added at the end to preserve the existing repr(C) prefix layout.
    pub plane_bias_k: f32,
    pub plane_spacing: f32,

    // ADR-141 P3 radial shell centre. The dag_radial_bias term springs each node
    // onto its shell around this point; (0,0,0) = origin = legacy DAG behaviour.
    // Added at the end to preserve the existing repr(C) prefix layout.
    pub radial_center_x: f32,
    pub radial_center_y: f32,
    pub radial_center_z: f32,
}

// SAFETY: SimParams is repr(C) with only POD types; safe for GPU transfer.
unsafe impl DeviceRepr for SimParams {}
unsafe impl DeviceCopy for SimParams {}

impl Default for SimParams {
    fn default() -> Self {
        Self::new()
    }
}

impl SimParams {
    pub fn new() -> Self {
        let params = SimulationParams::new();
        SimParams::from(&params)
    }

    pub fn set_iteration(&mut self, iteration: i32) {
        self.iteration = iteration;
    }

    pub fn to_simulation_params(&self) -> SimulationParams {
        SimulationParams {
            enabled: true,
            auto_balance: false,
            auto_balance_interval_ms: 100,
            auto_balance_config: AutoBalanceConfig::default(),
            auto_pause_config: AutoPauseConfig::default(),
            equilibrium_stability_counter: 0,
            is_physics_paused: false,
            iterations: 100,
            dt: self.dt,
            repel_k: self.repel_k,
            damping: self.damping,
            // Carry the authoritative GPU value rather than a hardcoded override.
            boundary_damping: self.boundary_damping,
            viewport_bounds: self.viewport_bounds,
            enable_bounds: true,
            max_velocity: self.max_velocity,
            max_force: self.max_force,
            // Carry the authoritative GPU value rather than a hardcoded 0.0.
            spring_k: self.spring_k,
            separation_radius: self.separation_radius,
            center_gravity_k: self.center_gravity_k,
            temperature: self.temperature,
            // alignment_strength / compute_mode / min_distance are internal-only
            // fields with no GPU source; default them deterministically.
            alignment_strength: self.alignment_strength,
            cluster_strength: self.cluster_strength,
            compute_mode: 0,
            min_distance: 1.0,
            max_repulsion_dist: self.repulsion_cutoff,
            warmup_iterations: self.warmup_iterations,
            cooling_rate: self.cooling_rate,
            rest_length: self.rest_length,
            use_sssp_distances: true,
            sssp_alpha: Some(self.sssp_alpha),
            constraint_ramp_frames: self.constraint_ramp_frames,
            constraint_max_force_per_node: self.constraint_max_force_per_node,
            repulsion_softening_epsilon: self.repulsion_softening_epsilon,
            grid_cell_size: self.grid_cell_size,
            // Carry the authoritative GPU value rather than a hardcoded 0.0001.
            gravity: self.gravity,
            phase: SimulationPhase::Dynamic,
            mode: SimulationMode::Remote,
            settle_mode: SettleMode::default(),
            // graph_separation_x / axis_compression_z / enable_dual_disc_layout
            // are CPU-side projection params with no field in the GPU-aligned
            // SimParams struct, so this reverse conversion cannot recover the live
            // value. Source them from the canonical PhysicsSettings::default().
            graph_separation_x: PhysicsSettings::default().graph_separation_x,
            axis_compression_z: PhysicsSettings::default().axis_compression_z,
            enable_dual_disc_layout: PhysicsSettings::default().enable_dual_disc_layout,
            layout_mode: LayoutMode::from_gpu_u32(self.layout_mode),
            lin_log_mode: self.lin_log_mode != 0,
            scaling_ratio: self.scaling_ratio,
            adaptive_speed: self.adaptive_speed != 0,
            global_speed: self.global_speed,
            dag_bias_k: self.dag_bias_k,
            dag_level_distance: self.dag_level_distance,
            plane_bias_k: self.plane_bias_k,
            plane_spacing: self.plane_spacing,
            radial_center: [
                self.radial_center_x,
                self.radial_center_y,
                self.radial_center_z,
            ],
            // Per-population spring multipliers (LinLog identity). This GPU-side
            // struct has no per-population source, so default to 1.0.
            spring_k_knowledge: 1.0,
            spring_k_ontology: 1.0,
            spring_k_agent: 1.0,
        }
    }
}

/// Local extension trait so existing call sites can keep using
/// `params.to_sim_params()` even though `SimulationParams` itself lives in
/// the domain crate (which knows nothing about CUDA-aligned `SimParams`).
pub trait ToSimParams {
    fn to_sim_params(&self) -> SimParams;
}

impl ToSimParams for SimulationParams {
    fn to_sim_params(&self) -> SimParams {
        SimParams::from(self)
    }
}

// Compile-time size assertion: SimParams must match the CUDA struct exactly.
const _: () = assert!(std::mem::size_of::<SimParams>() == 204);

impl From<&SimParams> for SimulationParams {
    fn from(params: &SimParams) -> Self {
        params.to_simulation_params()
    }
}

impl From<&SimulationParams> for SimParams {
    fn from(params: &SimulationParams) -> Self {
        let mut feature_flags = 0;
        if params.repel_k > 0.0 {
            feature_flags |= FeatureFlags::ENABLE_REPULSION;
        }
        if params.spring_k > 0.0 {
            feature_flags |= FeatureFlags::ENABLE_SPRINGS;
        }
        if params.center_gravity_k > 0.0 {
            feature_flags |= FeatureFlags::ENABLE_CENTERING;
        }
        if params.use_sssp_distances {
            feature_flags |= FeatureFlags::ENABLE_SSSP_SPRING_ADJUST;
        }

        SimParams {
            dt: params.dt,
            damping: params.damping,
            warmup_iterations: params.warmup_iterations,
            cooling_rate: params.cooling_rate,
            spring_k: params.spring_k,
            rest_length: params.rest_length,
            repel_k: params.repel_k,
            repulsion_cutoff: params.max_repulsion_dist,
            repulsion_softening_epsilon: params.repulsion_softening_epsilon,
            center_gravity_k: params.center_gravity_k,
            max_force: params.max_force,
            max_velocity: params.max_velocity,
            grid_cell_size: params.grid_cell_size,
            feature_flags,
            seed: 1337,
            iteration: 0,
            separation_radius: params.separation_radius,
            cluster_strength: params.cluster_strength,
            alignment_strength: params.alignment_strength,
            temperature: params.temperature,
            viewport_bounds: if params.enable_bounds {
                params.viewport_bounds
            } else {
                0.0
            },
            sssp_alpha: params.sssp_alpha.unwrap_or(0.0),
            boundary_damping: params.boundary_damping,
            constraint_ramp_frames: params.constraint_ramp_frames,
            constraint_max_force_per_node: params.constraint_max_force_per_node,

            stability_threshold: crate::config::dev_config::physics().stability_threshold,
            min_velocity_threshold: crate::config::dev_config::physics().min_velocity_threshold,

            world_bounds_min: crate::config::dev_config::physics().world_bounds_min,
            world_bounds_max: crate::config::dev_config::physics().world_bounds_max,
            cell_size_lod: crate::config::dev_config::physics().cell_size_lod,
            k_neighbors_max: crate::config::dev_config::physics().k_neighbors_max,
            anomaly_detection_radius: crate::config::dev_config::physics().anomaly_detection_radius,
            learning_rate_default: crate::config::dev_config::physics().learning_rate_default,

            norm_delta_cap: crate::config::dev_config::physics().norm_delta_cap,
            position_constraint_attraction: crate::config::dev_config::physics()
                .position_constraint_attraction,
            lof_score_min: crate::config::dev_config::physics().lof_score_min,
            lof_score_max: crate::config::dev_config::physics().lof_score_max,
            weight_precision_multiplier: crate::config::dev_config::physics()
                .weight_precision_multiplier,
            gravity: params.gravity,
            lin_log_mode: if params.lin_log_mode { 1 } else { 0 },
            scaling_ratio: params.scaling_ratio,
            adaptive_speed: if params.adaptive_speed { 1 } else { 0 },
            global_speed: params.global_speed,
            dag_bias_k: params.dag_bias_k,
            dag_level_distance: params.dag_level_distance,
            layout_mode: params.layout_mode.as_gpu_u32(),
            plane_bias_k: params.plane_bias_k,
            plane_spacing: params.plane_spacing,
            radial_center_x: params.radial_center[0],
            radial_center_y: params.radial_center[1],
            radial_center_z: params.radial_center[2],
        }
    }
}

impl From<&PhysicsSettings> for SimParams {
    fn from(physics: &PhysicsSettings) -> Self {
        let mut feature_flags = 0;
        if physics.repel_k > 0.0 {
            feature_flags |= FeatureFlags::ENABLE_REPULSION;
        }
        if physics.spring_k > 0.0 {
            feature_flags |= FeatureFlags::ENABLE_SPRINGS;
        }
        if physics.center_gravity_k > 0.0 {
            feature_flags |= FeatureFlags::ENABLE_CENTERING;
        }
        // Enable SSSP spring adjustment for ontology-aware edge rest lengths.
        feature_flags |= FeatureFlags::ENABLE_SSSP_SPRING_ADJUST;

        SimParams {
            dt: physics.dt,
            damping: physics.damping,
            warmup_iterations: physics.warmup_iterations,
            cooling_rate: physics.cooling_rate,
            spring_k: physics.spring_k,
            rest_length: physics.rest_length,
            repel_k: physics.repel_k,
            repulsion_cutoff: physics.max_repulsion_dist,
            repulsion_softening_epsilon: physics.repulsion_softening_epsilon,
            center_gravity_k: physics.center_gravity_k,
            max_force: physics.max_force,
            max_velocity: physics.max_velocity,
            grid_cell_size: physics.grid_cell_size,
            feature_flags,
            seed: 1337,
            iteration: 0,
            separation_radius: physics.separation_radius,
            cluster_strength: physics.cluster_strength,
            // alignment_strength is no longer a user-facing setting and the
            // kernel never reads this field; feed 0.0 to keep it inert while
            // preserving the 172-byte repr(C) layout.
            alignment_strength: 0.0,
            temperature: physics.temperature,
            viewport_bounds: if physics.enable_bounds {
                physics.bounds_size
            } else {
                0.0
            },
            sssp_alpha: physics.sssp_alpha,
            boundary_damping: physics.boundary_damping,
            constraint_ramp_frames: physics.constraint_ramp_frames,
            constraint_max_force_per_node: physics.constraint_max_force_per_node,

            stability_threshold: crate::config::dev_config::physics().stability_threshold,
            min_velocity_threshold: crate::config::dev_config::physics().min_velocity_threshold,

            world_bounds_min: crate::config::dev_config::physics().world_bounds_min,
            world_bounds_max: crate::config::dev_config::physics().world_bounds_max,
            cell_size_lod: crate::config::dev_config::physics().cell_size_lod,
            k_neighbors_max: crate::config::dev_config::physics().k_neighbors_max,
            anomaly_detection_radius: crate::config::dev_config::physics().anomaly_detection_radius,
            learning_rate_default: crate::config::dev_config::physics().learning_rate_default,

            norm_delta_cap: crate::config::dev_config::physics().norm_delta_cap,
            position_constraint_attraction: crate::config::dev_config::physics()
                .position_constraint_attraction,
            lof_score_min: crate::config::dev_config::physics().lof_score_min,
            lof_score_max: crate::config::dev_config::physics().lof_score_max,
            weight_precision_multiplier: crate::config::dev_config::physics()
                .weight_precision_multiplier,
            gravity: physics.gravity,
            lin_log_mode: if physics.lin_log_mode { 1 } else { 0 },
            scaling_ratio: physics.scaling_ratio,
            adaptive_speed: if physics.adaptive_speed { 1 } else { 0 },
            global_speed: physics.global_speed,
            dag_bias_k: physics.dag_bias_k,
            dag_level_distance: physics.dag_level_distance,
            // PhysicsSettings carries no layout mode — the authoritative mode rides
            // SimulationParams.layout_mode (set via POST /api/layout/mode). Default
            // to ForceDirected here so this wire path never silently forces a mode.
            layout_mode: LayoutMode::ForceDirected.as_gpu_u32(),
            plane_bias_k: physics.plane_bias_k,
            plane_spacing: physics.plane_spacing,
            // PhysicsSettings carries no radial centre — it is actor-authoritative
            // (owned by SetRadialLayout). Default to the origin so this wire path
            // never resets an active radial centre back to nothing.
            radial_center_x: 0.0,
            radial_center_y: 0.0,
            radial_center_z: 0.0,
        }
    }
}
