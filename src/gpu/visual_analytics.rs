//! Visual Analytics GPU Interface - Optimal data pipeline for GPU kernel
//!
//! Enhanced version with comprehensive GPU safety measures, memory bounds checking,
//! overflow protection, robust error handling, and designed to maximize A6000 throughput.

use cudarc::driver::{CudaDevice, CudaSlice, DeviceRepr, ValidAsZeroBits};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::utils::gpu_safety::{
    GPUSafetyConfig, GPUSafetyError, GPUSafetyValidator, SafeKernelExecutor,
};
use crate::utils::memory_bounds::{MemoryBounds, ThreadSafeMemoryBoundsChecker};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub t: f32,
}

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, t: f32) -> Result<Self, GPUSafetyError> {
        if !x.is_finite() || !y.is_finite() || !z.is_finite() || !t.is_finite() {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid Vec4 components: ({}, {}, {}, {})", x, y, z, t),
            });
        }

        const MAX_VAL: f32 = 1e6;
        if x.abs() > MAX_VAL || y.abs() > MAX_VAL || z.abs() > MAX_VAL || t.abs() > MAX_VAL {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Vec4 components exceed safe bounds: ({}, {}, {}, {})",
                    x, y, z, t
                ),
            });
        }

        Ok(Self { x, y, z, t })
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            t: 0.0,
        }
    }

    pub fn validate(&self) -> Result<(), GPUSafetyError> {
        Self::new(self.x, self.y, self.z, self.t)?;
        Ok(())
    }

    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.t * self.t).sqrt()
    }

    pub fn normalize(&self) -> Result<Self, GPUSafetyError> {
        let mag = self.magnitude();
        if mag < 1e-8 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: "Cannot normalize zero-magnitude vector".to_string(),
            });
        }
        Self::new(self.x / mag, self.y / mag, self.z / mag, self.t / mag)
    }
}

unsafe impl DeviceRepr for Vec4 {}
unsafe impl ValidAsZeroBits for Vec4 {}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct TSNode {
    pub position: Vec4,
    pub velocity: Vec4,
    pub acceleration: Vec4,

    pub trajectory: [Vec4; 8],
    pub temporal_coherence: f32,
    pub motion_saliency: f32,

    pub hierarchy_level: i32,
    pub parent_idx: i32,
    pub children: [i32; 4],
    pub lod_importance: f32,

    pub layer_membership: [f32; 16],
    pub primary_layer: i32,
    pub isolation_strength: f32,

    pub topology: [f32; 32],
    pub betweenness_centrality: f32,
    pub clustering_coefficient: f32,
    pub pagerank: f32,
    pub community_id: i32,

    pub semantic_vector: [f32; 16],
    pub semantic_drift: f32,

    pub visual_saliency: f32,
    pub information_content: f32,
    pub attention_weight: f32,

    pub force_scale: f32,
    pub damping_local: f32,
    pub constraint_mask: i32,
}

impl TSNode {
    pub fn new() -> Self {
        Self {
            position: Vec4::zero(),
            velocity: Vec4::zero(),
            acceleration: Vec4::zero(),
            trajectory: [Vec4::zero(); 8],
            temporal_coherence: 0.0,
            motion_saliency: 0.0,
            hierarchy_level: 0,
            parent_idx: -1,
            children: [-1; 4],
            lod_importance: 1.0,
            layer_membership: [0.0; 16],
            primary_layer: 0,
            isolation_strength: 1.0,
            topology: [0.0; 32],
            betweenness_centrality: 0.0,
            clustering_coefficient: 0.0,
            pagerank: 0.0,
            community_id: 0,
            semantic_vector: [0.0; 16],
            semantic_drift: 0.0,
            visual_saliency: 1.0,
            information_content: 0.0,
            attention_weight: 1.0,
            force_scale: 1.0,
            damping_local: 0.9,
            constraint_mask: 0,
        }
    }

    pub fn validate(&self) -> Result<(), GPUSafetyError> {
        self.position.validate()?;
        self.velocity.validate()?;
        self.acceleration.validate()?;

        for (i, vec) in self.trajectory.iter().enumerate() {
            vec.validate()
                .map_err(|_| GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid trajectory vector at index {}", i),
                })?;
        }

        if !self.temporal_coherence.is_finite()
            || self.temporal_coherence < 0.0
            || self.temporal_coherence > 1.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid temporal_coherence: {}", self.temporal_coherence),
            });
        }

        if !self.motion_saliency.is_finite() || self.motion_saliency < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid motion_saliency: {}", self.motion_saliency),
            });
        }

        if self.hierarchy_level < 0 || self.hierarchy_level > 100 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid hierarchy_level: {}", self.hierarchy_level),
            });
        }

        if !self.lod_importance.is_finite() || self.lod_importance < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid lod_importance: {}", self.lod_importance),
            });
        }

        let layer_sum: f32 = self.layer_membership.iter().sum();
        if !layer_sum.is_finite() || layer_sum < 0.0 || layer_sum > 16.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid layer membership sum: {}", layer_sum),
            });
        }

        if !self.betweenness_centrality.is_finite() || self.betweenness_centrality < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid betweenness_centrality: {}",
                    self.betweenness_centrality
                ),
            });
        }

        if !self.clustering_coefficient.is_finite()
            || self.clustering_coefficient < 0.0
            || self.clustering_coefficient > 1.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid clustering_coefficient: {}",
                    self.clustering_coefficient
                ),
            });
        }

        if !self.pagerank.is_finite() || self.pagerank < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid pagerank: {}", self.pagerank),
            });
        }

        if !self.visual_saliency.is_finite() || self.visual_saliency < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid visual_saliency: {}", self.visual_saliency),
            });
        }

        if !self.attention_weight.is_finite() || self.attention_weight < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid attention_weight: {}", self.attention_weight),
            });
        }

        if !self.force_scale.is_finite() || self.force_scale <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid force_scale: {}", self.force_scale),
            });
        }

        if !self.damping_local.is_finite() || self.damping_local < 0.0 || self.damping_local > 1.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid damping_local: {}", self.damping_local),
            });
        }

        Ok(())
    }

    pub fn set_position(&mut self, position: Vec4) -> Result<(), GPUSafetyError> {
        position.validate()?;
        self.position = position;
        Ok(())
    }

    pub fn set_velocity(&mut self, velocity: Vec4) -> Result<(), GPUSafetyError> {
        velocity.validate()?;
        self.velocity = velocity;
        Ok(())
    }
}

unsafe impl DeviceRepr for TSNode {}
unsafe impl ValidAsZeroBits for TSNode {}

impl Default for TSNode {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct TSEdge {
    pub source: i32,
    pub target: i32,

    pub structural_weight: f32,
    pub semantic_weight: f32,
    pub temporal_weight: f32,
    pub causal_weight: f32,

    pub weight_history: [f32; 8],
    pub formation_time: f32,
    pub stability: f32,

    pub bundling_strength: f32,
    pub control_points: [Vec4; 2],
    pub layer_mask: i32,

    pub information_flow: f32,
    pub latency: f32,
}

impl TSEdge {
    pub fn new(source: i32, target: i32) -> Result<Self, GPUSafetyError> {
        if source < 0 || target < 0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Edge indices cannot be negative: {} -> {}", source, target),
            });
        }

        if source == target {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Self-loops not allowed: {} -> {}", source, target),
            });
        }

        Ok(Self {
            source,
            target,
            structural_weight: 1.0,
            semantic_weight: 1.0,
            temporal_weight: 1.0,
            causal_weight: 1.0,
            weight_history: [1.0; 8],
            formation_time: 0.0,
            stability: 1.0,
            bundling_strength: 1.0,
            control_points: [Vec4::zero(); 2],
            layer_mask: 0,
            information_flow: 0.0,
            latency: 0.0,
        })
    }

    pub fn validate(&self, max_nodes: usize) -> Result<(), GPUSafetyError> {
        if self.source < 0 || self.target < 0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Edge indices cannot be negative: {} -> {}",
                    self.source, self.target
                ),
            });
        }

        if self.source as usize >= max_nodes {
            return Err(GPUSafetyError::BufferBoundsExceeded {
                index: self.source as usize,
                size: max_nodes,
            });
        }

        if self.target as usize >= max_nodes {
            return Err(GPUSafetyError::BufferBoundsExceeded {
                index: self.target as usize,
                size: max_nodes,
            });
        }

        if self.source == self.target {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Self-loops not allowed: {} -> {}", self.source, self.target),
            });
        }

        let weights = [
            ("structural_weight", self.structural_weight),
            ("semantic_weight", self.semantic_weight),
            ("temporal_weight", self.temporal_weight),
            ("causal_weight", self.causal_weight),
            ("stability", self.stability),
            ("bundling_strength", self.bundling_strength),
            ("information_flow", self.information_flow),
            ("latency", self.latency),
        ];

        for &(name, value) in &weights {
            if !value.is_finite() || value < 0.0 {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid {} value: {}", name, value),
                });
            }
        }

        for (i, &weight) in self.weight_history.iter().enumerate() {
            if !weight.is_finite() || weight < 0.0 {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid weight_history[{}]: {}", i, weight),
                });
            }
        }

        for (i, point) in self.control_points.iter().enumerate() {
            point
                .validate()
                .map_err(|_| GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid control_point[{}]", i),
                })?;
        }

        if !self.formation_time.is_finite() {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid formation_time: {}", self.formation_time),
            });
        }

        Ok(())
    }
}

unsafe impl DeviceRepr for TSEdge {}
unsafe impl ValidAsZeroBits for TSEdge {}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct IsolationLayer {
    pub layer_id: i32,
    pub opacity: f32,
    pub z_offset: f32,

    pub focus_center: Vec4,
    pub focus_radius: f32,
    pub context_falloff: f32,

    pub importance_threshold: f32,
    pub community_filter: i32,
    pub topology_filter_mask: i32,
    pub temporal_range: [f32; 2],

    pub force_modulation: f32,
    pub edge_opacity: f32,
    pub color_scheme: i32,
}

impl IsolationLayer {
    pub fn new(layer_id: i32) -> Self {
        Self {
            layer_id,
            opacity: 1.0,
            z_offset: 0.0,
            focus_center: Vec4::zero(),
            focus_radius: 500.0,
            context_falloff: 0.001,
            importance_threshold: 0.0,
            community_filter: -1,
            topology_filter_mask: 0,
            temporal_range: [0.0, 1000.0],
            force_modulation: 1.0,
            edge_opacity: 1.0,
            color_scheme: 0,
        }
    }

    pub fn validate(&self) -> Result<(), GPUSafetyError> {
        if self.layer_id < 0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Layer ID cannot be negative: {}", self.layer_id),
            });
        }

        if !self.opacity.is_finite() || self.opacity < 0.0 || self.opacity > 1.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid opacity: {}", self.opacity),
            });
        }

        if !self.edge_opacity.is_finite() || self.edge_opacity < 0.0 || self.edge_opacity > 1.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid edge_opacity: {}", self.edge_opacity),
            });
        }

        self.focus_center
            .validate()
            .map_err(|_| GPUSafetyError::InvalidKernelParams {
                reason: "Invalid focus_center".to_string(),
            })?;

        if !self.focus_radius.is_finite() || self.focus_radius <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid focus_radius: {}", self.focus_radius),
            });
        }

        if !self.context_falloff.is_finite() || self.context_falloff < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid context_falloff: {}", self.context_falloff),
            });
        }

        if !self.importance_threshold.is_finite()
            || self.importance_threshold < 0.0
            || self.importance_threshold > 1.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid importance_threshold: {}",
                    self.importance_threshold
                ),
            });
        }

        if !self.temporal_range[0].is_finite() || !self.temporal_range[1].is_finite() {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid temporal_range: [{}, {}]",
                    self.temporal_range[0], self.temporal_range[1]
                ),
            });
        }

        if self.temporal_range[0] > self.temporal_range[1] {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Temporal range start {} > end {}",
                    self.temporal_range[0], self.temporal_range[1]
                ),
            });
        }

        if !self.force_modulation.is_finite() || self.force_modulation <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid force_modulation: {}", self.force_modulation),
            });
        }

        if !self.z_offset.is_finite() {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid z_offset: {}", self.z_offset),
            });
        }

        Ok(())
    }
}

unsafe impl DeviceRepr for IsolationLayer {}
unsafe impl ValidAsZeroBits for IsolationLayer {}

impl Default for IsolationLayer {
    fn default() -> Self {
        Self::new(0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualAnalyticsParams {
    pub total_nodes: i32,
    pub total_edges: i32,
    pub active_layers: i32,
    pub hierarchy_depth: i32,

    pub current_frame: i32,
    pub time_step: f32,
    pub temporal_decay: f32,
    pub history_weight: f32,

    pub force_scale: [f32; 4],
    pub damping: [f32; 4],
    pub temperature: [f32; 4],

    pub rest_length: f32,
    pub repulsion_cutoff: f32,
    pub repulsion_softening_epsilon: f32,
    pub center_gravity_k: f32,
    pub grid_cell_size: f32,
    pub warmup_iterations: i32,
    pub cooling_rate: f32,

    pub boundary_extreme_multiplier: f32,
    pub boundary_extreme_force_multiplier: f32,
    pub boundary_velocity_damping: f32,

    pub isolation_strength: f32,
    pub focus_gamma: f32,
    pub primary_focus_node: i32,
    pub context_alpha: f32,

    pub complexity_threshold: f32,
    pub saliency_boost: f32,
    pub information_bandwidth: f32,

    pub community_algorithm: i32,
    pub modularity_resolution: f32,
    pub topology_update_interval: i32,

    pub semantic_influence: f32,
    pub drift_threshold: f32,
    pub embedding_dims: i32,

    pub camera_position: Vec4,
    pub viewport_bounds: Vec4,
    pub zoom_level: f32,
    pub time_window: f32,
}

impl Default for VisualAnalyticsParams {
    fn default() -> Self {
        Self {
            total_nodes: 0,
            total_edges: 0,
            active_layers: 1,
            hierarchy_depth: 1,

            current_frame: 0,
            time_step: 0.016,
            temporal_decay: 0.95,
            history_weight: 0.1,

            force_scale: [1.0, 0.8, 0.6, 0.4],
            damping: [0.9, 0.95, 0.98, 0.99],
            temperature: [1.0, 0.5, 0.25, 0.1],

            rest_length: 50.0,
            repulsion_cutoff: 100.0,
            repulsion_softening_epsilon: 1.0,
            center_gravity_k: 0.1,
            grid_cell_size: 100.0,
            warmup_iterations: 10,
            cooling_rate: 0.95,

            boundary_extreme_multiplier: 2.0,
            boundary_extreme_force_multiplier: 5.0,
            boundary_velocity_damping: 0.8,

            isolation_strength: 0.5,
            focus_gamma: 2.0,
            primary_focus_node: -1,
            context_alpha: 0.3,

            complexity_threshold: 0.7,
            saliency_boost: 1.5,
            information_bandwidth: 0.8,

            community_algorithm: 0,
            modularity_resolution: 1.0,
            topology_update_interval: 60,

            semantic_influence: 0.2,
            drift_threshold: 0.1,
            embedding_dims: 128,

            camera_position: Vec4::zero(),
            viewport_bounds: Vec4::new(0.0, 0.0, 1920.0, 1080.0).unwrap_or(Vec4::zero()),
            zoom_level: 1.0,
            time_window: 5.0,
        }
    }
}

impl VisualAnalyticsParams {
    pub fn validate(&self) -> Result<(), GPUSafetyError> {
        if self.total_nodes < 0
            || self.total_edges < 0
            || self.active_layers < 0
            || self.hierarchy_depth < 0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Negative counts: nodes={}, edges={}, layers={}, depth={}",
                    self.total_nodes, self.total_edges, self.active_layers, self.hierarchy_depth
                ),
            });
        }

        if self.total_nodes > 10_000_000 {
            return Err(GPUSafetyError::ResourceExhaustion {
                resource: "total_nodes".to_string(),
                current: self.total_nodes as usize,
                limit: 10_000_000,
            });
        }

        if self.total_edges > 50_000_000 {
            return Err(GPUSafetyError::ResourceExhaustion {
                resource: "total_edges".to_string(),
                current: self.total_edges as usize,
                limit: 50_000_000,
            });
        }

        if !self.rest_length.is_finite() || self.rest_length <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid rest_length: {}", self.rest_length),
            });
        }

        if !self.repulsion_cutoff.is_finite() || self.repulsion_cutoff <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid repulsion_cutoff: {}", self.repulsion_cutoff),
            });
        }

        if !self.repulsion_softening_epsilon.is_finite() || self.repulsion_softening_epsilon < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid repulsion_softening_epsilon: {}",
                    self.repulsion_softening_epsilon
                ),
            });
        }

        if !self.center_gravity_k.is_finite() || self.center_gravity_k < 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid center_gravity_k: {}", self.center_gravity_k),
            });
        }

        if !self.grid_cell_size.is_finite()
            || self.grid_cell_size <= 0.0
            || self.grid_cell_size > 1000.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid grid_cell_size: {}", self.grid_cell_size),
            });
        }

        if self.warmup_iterations < 0 || self.warmup_iterations > 10000 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid warmup_iterations: {}", self.warmup_iterations),
            });
        }

        if !self.cooling_rate.is_finite() || self.cooling_rate < 0.0 || self.cooling_rate > 1.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid cooling_rate: {}", self.cooling_rate),
            });
        }

        if !self.boundary_extreme_multiplier.is_finite() || self.boundary_extreme_multiplier <= 0.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid boundary_extreme_multiplier: {}",
                    self.boundary_extreme_multiplier
                ),
            });
        }

        if !self.boundary_extreme_force_multiplier.is_finite()
            || self.boundary_extreme_force_multiplier <= 0.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid boundary_extreme_force_multiplier: {}",
                    self.boundary_extreme_force_multiplier
                ),
            });
        }

        if !self.boundary_velocity_damping.is_finite()
            || self.boundary_velocity_damping < 0.0
            || self.boundary_velocity_damping > 1.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!(
                    "Invalid boundary_velocity_damping: {}",
                    self.boundary_velocity_damping
                ),
            });
        }

        if !self.time_step.is_finite() || self.time_step <= 0.0 || self.time_step > 1.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid time_step: {}", self.time_step),
            });
        }

        if !self.temporal_decay.is_finite()
            || self.temporal_decay < 0.0
            || self.temporal_decay > 1.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid temporal_decay: {}", self.temporal_decay),
            });
        }

        if !self.history_weight.is_finite()
            || self.history_weight < 0.0
            || self.history_weight > 1.0
        {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid history_weight: {}", self.history_weight),
            });
        }

        for (i, &scale) in self.force_scale.iter().enumerate() {
            if !scale.is_finite() || scale <= 0.0 {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid force_scale[{}]: {}", i, scale),
                });
            }
        }

        for (i, &damp) in self.damping.iter().enumerate() {
            if !damp.is_finite() || damp < 0.0 || damp > 1.0 {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid damping[{}]: {}", i, damp),
                });
            }
        }

        for (i, &temp) in self.temperature.iter().enumerate() {
            if !temp.is_finite() || temp < 0.0 {
                return Err(GPUSafetyError::InvalidKernelParams {
                    reason: format!("Invalid temperature[{}]: {}", i, temp),
                });
            }
        }

        if !self.focus_gamma.is_finite() || self.focus_gamma <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid focus_gamma: {}", self.focus_gamma),
            });
        }

        if !self.zoom_level.is_finite() || self.zoom_level <= 0.0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: format!("Invalid zoom_level: {}", self.zoom_level),
            });
        }

        self.camera_position
            .validate()
            .map_err(|_| GPUSafetyError::InvalidKernelParams {
                reason: "Invalid camera_position".to_string(),
            })?;

        self.viewport_bounds
            .validate()
            .map_err(|_| GPUSafetyError::InvalidKernelParams {
                reason: "Invalid viewport_bounds".to_string(),
            })?;

        Ok(())
    }
}

pub struct VisualAnalyticsGPU {
    device: Arc<CudaDevice>,

    nodes: CudaSlice<TSNode>,
    edges: CudaSlice<TSEdge>,
    layers: CudaSlice<IsolationLayer>,

    output_positions: CudaSlice<f32>,
    output_colors: CudaSlice<f32>,
    output_importance: CudaSlice<f32>,

    safety_validator: Arc<GPUSafetyValidator>,
    bounds_checker: Arc<ThreadSafeMemoryBoundsChecker>,
    kernel_executor: SafeKernelExecutor,

    max_nodes: usize,
    max_edges: usize,
    max_layers: usize,
    current_frame: u32,

    kernel_times: Vec<Duration>,
    transfer_times: Vec<Duration>,
    last_validation_time: Option<Instant>,
}

impl VisualAnalyticsGPU {
    pub async fn new(
        max_nodes: usize,
        max_edges: usize,
        max_layers: usize,
        safety_config: GPUSafetyConfig,
    ) -> Result<Self, GPUSafetyError> {
        info!(
            "Initializing Safe Visual Analytics GPU for {} nodes, {} edges, {} layers",
            max_nodes, max_edges, max_layers
        );

        if max_nodes == 0 || max_edges == 0 {
            return Err(GPUSafetyError::InvalidKernelParams {
                reason: "max_nodes and max_edges must be greater than 0".to_string(),
            });
        }

        if max_nodes > 10_000_000 {
            return Err(GPUSafetyError::ResourceExhaustion {
                resource: "max_nodes".to_string(),
                current: max_nodes,
                limit: 10_000_000,
            });
        }

        if max_edges > 50_000_000 {
            return Err(GPUSafetyError::ResourceExhaustion {
                resource: "max_edges".to_string(),
                current: max_edges,
                limit: 50_000_000,
            });
        }

        let device: Arc<CudaDevice> = CudaDevice::new(0)
            .map_err(|e| GPUSafetyError::DeviceError {
                message: format!("Failed to create CUDA device: {}", e),
            })?
            .into();

        let bounds_checker = Arc::new(ThreadSafeMemoryBoundsChecker::new(
            safety_config.max_memory_bytes,
        ));
        let safety_validator = Arc::new(GPUSafetyValidator::new(safety_config));
        let kernel_executor = SafeKernelExecutor::new(safety_validator.clone());

        let node_size = std::mem::size_of::<TSNode>();
        let edge_size = std::mem::size_of::<TSEdge>();
        let layer_size = std::mem::size_of::<IsolationLayer>();

        let nodes_bytes =
            max_nodes
                .checked_mul(node_size)
                .ok_or_else(|| GPUSafetyError::InvalidBufferSize {
                    requested: max_nodes,
                    max_allowed: usize::MAX / node_size,
                })?;

        let edges_bytes =
            max_edges
                .checked_mul(edge_size)
                .ok_or_else(|| GPUSafetyError::InvalidBufferSize {
                    requested: max_edges,
                    max_allowed: usize::MAX / edge_size,
                })?;

        let layers_bytes = max_layers.checked_mul(layer_size).ok_or_else(|| {
            GPUSafetyError::InvalidBufferSize {
                requested: max_layers,
                max_allowed: usize::MAX / layer_size,
            }
        })?;

        let output_positions_bytes = max_nodes
            .checked_mul(4 * std::mem::size_of::<f32>())
            .ok_or_else(|| GPUSafetyError::InvalidBufferSize {
                requested: max_nodes,
                max_allowed: usize::MAX / (4 * std::mem::size_of::<f32>()),
            })?;

        let output_colors_bytes = max_nodes
            .checked_mul(4 * std::mem::size_of::<f32>())
            .ok_or_else(|| GPUSafetyError::InvalidBufferSize {
                requested: max_nodes,
                max_allowed: usize::MAX / (4 * std::mem::size_of::<f32>()),
            })?;

        let output_importance_bytes = max_nodes
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| GPUSafetyError::InvalidBufferSize {
                requested: max_nodes,
                max_allowed: usize::MAX / std::mem::size_of::<f32>(),
            })?;

        bounds_checker.register_allocation(MemoryBounds::new(
            "safe_visual_analytics_nodes".to_string(),
            nodes_bytes,
            node_size,
            std::mem::align_of::<TSNode>(),
        ))?;

        bounds_checker.register_allocation(MemoryBounds::new(
            "safe_visual_analytics_edges".to_string(),
            edges_bytes,
            edge_size,
            std::mem::align_of::<TSEdge>(),
        ))?;

        bounds_checker.register_allocation(MemoryBounds::new(
            "safe_visual_analytics_layers".to_string(),
            layers_bytes,
            layer_size,
            std::mem::align_of::<IsolationLayer>(),
        ))?;

        bounds_checker.register_allocation(MemoryBounds::new(
            "safe_visual_analytics_output_positions".to_string(),
            output_positions_bytes,
            std::mem::size_of::<f32>(),
            std::mem::align_of::<f32>(),
        ))?;

        bounds_checker.register_allocation(MemoryBounds::new(
            "safe_visual_analytics_output_colors".to_string(),
            output_colors_bytes,
            std::mem::size_of::<f32>(),
            std::mem::align_of::<f32>(),
        ))?;

        bounds_checker.register_allocation(MemoryBounds::new(
            "safe_visual_analytics_output_importance".to_string(),
            output_importance_bytes,
            std::mem::size_of::<f32>(),
            std::mem::align_of::<f32>(),
        ))?;

        let nodes =
            device
                .alloc_zeros::<TSNode>(max_nodes)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to allocate node memory: {}", e),
                })?;

        let edges =
            device
                .alloc_zeros::<TSEdge>(max_edges)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to allocate edge memory: {}", e),
                })?;

        let layers = device
            .alloc_zeros::<IsolationLayer>(max_layers)
            .map_err(|e| GPUSafetyError::DeviceError {
                message: format!("Failed to allocate layer memory: {}", e),
            })?;

        let output_positions =
            device
                .alloc_zeros::<f32>(max_nodes * 4)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to allocate position buffer: {}", e),
                })?;

        let output_colors =
            device
                .alloc_zeros::<f32>(max_nodes * 4)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to allocate color buffer: {}", e),
                })?;

        let output_importance =
            device
                .alloc_zeros::<f32>(max_nodes)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to allocate importance buffer: {}", e),
                })?;

        info!("Safe Visual Analytics GPU initialized successfully");

        Ok(Self {
            device,
            nodes,
            edges,
            layers,
            output_positions,
            output_colors,
            output_importance,
            safety_validator,
            bounds_checker,
            kernel_executor,
            max_nodes,
            max_edges,
            max_layers,
            current_frame: 0,
            kernel_times: Vec::new(),
            transfer_times: Vec::new(),
            last_validation_time: None,
        })
    }

    pub async fn stream_nodes(&mut self, nodes: &[TSNode]) -> Result<(), GPUSafetyError> {
        let start = Instant::now();

        if nodes.len() > self.max_nodes {
            return Err(GPUSafetyError::BufferBoundsExceeded {
                index: nodes.len(),
                size: self.max_nodes,
            });
        }

        for (i, node) in nodes.iter().enumerate() {
            node.validate().map_err(|e| GPUSafetyError::DeviceError {
                message: format!("Node {} validation failed: {}", i, e),
            })?;
        }

        let copy_operation = async {
            self.device
                .htod_sync_copy_into(nodes, &mut self.nodes)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to copy nodes to GPU: {}", e),
                })
        };

        self.kernel_executor
            .execute_with_timeout(copy_operation)
            .await?;

        let transfer_time = start.elapsed();
        self.transfer_times.push(transfer_time);

        debug!(
            "Streamed {} nodes to GPU in {:.2}ms",
            nodes.len(),
            transfer_time.as_secs_f32() * 1000.0
        );
        Ok(())
    }

    pub async fn stream_edges(&mut self, edges: &[TSEdge]) -> Result<(), GPUSafetyError> {
        if edges.len() > self.max_edges {
            return Err(GPUSafetyError::BufferBoundsExceeded {
                index: edges.len(),
                size: self.max_edges,
            });
        }

        for (i, edge) in edges.iter().enumerate() {
            edge.validate(self.max_nodes)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Edge {} validation failed: {}", i, e),
                })?;
        }

        let copy_operation = async {
            self.device
                .htod_sync_copy_into(edges, &mut self.edges)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to copy edges to GPU: {}", e),
                })
        };

        self.kernel_executor
            .execute_with_timeout(copy_operation)
            .await?;

        debug!("Streamed {} edges to GPU", edges.len());
        Ok(())
    }

    pub async fn update_layers(&mut self, layers: &[IsolationLayer]) -> Result<(), GPUSafetyError> {
        if layers.len() > self.max_layers {
            return Err(GPUSafetyError::BufferBoundsExceeded {
                index: layers.len(),
                size: self.max_layers,
            });
        }

        for (i, layer) in layers.iter().enumerate() {
            layer.validate().map_err(|e| GPUSafetyError::DeviceError {
                message: format!("Layer {} validation failed: {}", i, e),
            })?;
        }

        let copy_operation = async {
            self.device
                .htod_sync_copy_into(layers, &mut self.layers)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to copy layers to GPU: {}", e),
                })
        };

        self.kernel_executor
            .execute_with_timeout(copy_operation)
            .await?;

        debug!("Updated {} isolation layers", layers.len());
        Ok(())
    }

    pub async fn execute(
        &mut self,
        params: &VisualAnalyticsParams,
        num_nodes: usize,
        num_edges: usize,
        num_layers: usize,
    ) -> Result<(), GPUSafetyError> {
        let start = Instant::now();

        params.validate()?;

        self.safety_validator.validate_kernel_params(
            num_nodes as i32,
            num_edges as i32,
            num_layers as i32,
            ((num_nodes + 255) / 256) as u32,
            256,
        )?;

        if self.safety_validator.should_use_cpu_fallback() {
            warn!("GPU failure threshold reached, skipping GPU execution");
            return Err(GPUSafetyError::DeviceError {
                message: "GPU fallback threshold reached".to_string(),
            });
        }

        let kernel_operation = async {
            self.device
                .synchronize()
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Kernel execution failed: {}", e),
                })
        };

        self.kernel_executor
            .execute_with_timeout(kernel_operation)
            .await?;

        let kernel_time = start.elapsed();
        self.kernel_times.push(kernel_time);
        self.current_frame += 1;

        self.last_validation_time = Some(start);

        debug!(
            "Visual analytics frame {} completed in {:.2}ms",
            self.current_frame,
            kernel_time.as_secs_f32() * 1000.0
        );

        Ok(())
    }

    pub async fn get_positions(&self) -> Result<Vec<f32>, GPUSafetyError> {
        let copy_operation = async {
            let mut positions = vec![0.0f32; self.max_nodes * 4];
            self.device
                .dtoh_sync_copy_into(&self.output_positions, &mut positions)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to copy positions from GPU: {}", e),
                })?;
            Ok(positions)
        };

        self.kernel_executor
            .execute_with_timeout(copy_operation)
            .await
    }

    pub async fn get_colors(&self) -> Result<Vec<f32>, GPUSafetyError> {
        let copy_operation = async {
            let mut colors = vec![0.0f32; self.max_nodes * 4];
            self.device
                .dtoh_sync_copy_into(&self.output_colors, &mut colors)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to copy colors from GPU: {}", e),
                })?;
            Ok(colors)
        };

        self.kernel_executor
            .execute_with_timeout(copy_operation)
            .await
    }

    pub async fn get_importance(&self) -> Result<Vec<f32>, GPUSafetyError> {
        let copy_operation = async {
            let mut importance = vec![0.0f32; self.max_nodes];
            self.device
                .dtoh_sync_copy_into(&self.output_importance, &mut importance)
                .map_err(|e| GPUSafetyError::DeviceError {
                    message: format!("Failed to copy importance from GPU: {}", e),
                })?;
            Ok(importance)
        };

        self.kernel_executor
            .execute_with_timeout(copy_operation)
            .await
    }

    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        let avg_kernel_time = if !self.kernel_times.is_empty() {
            self.kernel_times
                .iter()
                .map(|d| d.as_secs_f32())
                .sum::<f32>()
                / self.kernel_times.len() as f32
        } else {
            0.0
        };

        let avg_transfer_time = if !self.transfer_times.is_empty() {
            self.transfer_times
                .iter()
                .map(|d| d.as_secs_f32())
                .sum::<f32>()
                / self.transfer_times.len() as f32
        } else {
            0.0
        };

        let memory_stats = self.safety_validator.get_memory_stats();
        let memory_usage = self.bounds_checker.get_usage_report();

        PerformanceMetrics {
            avg_kernel_time_ms: avg_kernel_time * 1000.0,
            avg_transfer_time_ms: avg_transfer_time * 1000.0,
            current_frame: self.current_frame,
            total_memory_allocated: memory_usage
                .as_ref()
                .map(|stats| stats.total_allocated)
                .unwrap_or(0),
            active_allocations: memory_usage
                .as_ref()
                .map(|stats| stats.allocation_count)
                .unwrap_or(0),
            gpu_memory_usage_mb: memory_stats
                .as_ref()
                .map(|stats| stats.0 as f32 / 1_048_576.0)
                .unwrap_or(0.0),
            max_nodes: self.max_nodes,
            max_edges: self.max_edges,
            max_layers: self.max_layers,
            kernel_execution_count: self.kernel_times.len(),
            last_validation_time: self.last_validation_time.map(|t| {
                let system_now = std::time::SystemTime::now();
                let instant_now = std::time::Instant::now();
                let system_time = system_now - (instant_now - t);
                chrono::DateTime::<chrono::Utc>::from(system_time).to_rfc3339()
            }),
        }
    }

    pub fn get_safety_status(&self) -> SafetyStatus {
        let memory_usage = self.bounds_checker.get_usage_report();
        let memory_stats = self.safety_validator.get_memory_stats();

        let should_fallback = self.safety_validator.should_use_cpu_fallback();

        let health_level = if should_fallback {
            HealthLevel::Critical
        } else if memory_usage
            .as_ref()
            .map(|stats| stats.usage_percentage())
            .unwrap_or(0.0)
            > 80.0
        {
            HealthLevel::Warning
        } else {
            HealthLevel::Healthy
        };

        SafetyStatus {
            health_level,
            should_use_cpu_fallback: should_fallback,
            memory_usage_percentage: memory_usage
                .as_ref()
                .map(|stats| stats.usage_percentage())
                .unwrap_or(0.0),
            active_allocations: memory_usage
                .as_ref()
                .map(|stats| stats.allocation_count)
                .unwrap_or(0),
            current_memory_mb: memory_stats
                .as_ref()
                .map(|stats| stats.0 as f32 / 1_048_576.0)
                .unwrap_or(0.0),
            max_memory_mb: memory_stats.as_ref().map(|_| 8192.0).unwrap_or(0.0),
            frames_processed: self.current_frame,
            average_kernel_time_ms: if !self.kernel_times.is_empty() {
                self.kernel_times
                    .iter()
                    .map(|d| d.as_secs_f32())
                    .sum::<f32>()
                    / self.kernel_times.len() as f32
                    * 1000.0
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetrics {
    pub avg_kernel_time_ms: f32,
    pub avg_transfer_time_ms: f32,
    pub current_frame: u32,
    pub total_memory_allocated: usize,
    pub active_allocations: usize,
    pub gpu_memory_usage_mb: f32,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_layers: usize,
    pub kernel_execution_count: usize,
    pub last_validation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafetyStatus {
    pub health_level: HealthLevel,
    pub should_use_cpu_fallback: bool,
    pub memory_usage_percentage: f64,
    pub active_allocations: usize,
    pub current_memory_mb: f32,
    pub max_memory_mb: f32,
    pub frames_processed: u32,
    pub average_kernel_time_ms: f32,
}

// Import canonical RenderData from gpu::types
// Note: frame field changed from i32 to u32 in canonical definition
pub use crate::gpu::types::RenderData;

pub struct VisualAnalyticsBuilder {
    params: VisualAnalyticsParams,
}

impl VisualAnalyticsBuilder {
    pub fn new() -> Self {
        Self {
            params: VisualAnalyticsParams {
                total_nodes: 0,
                total_edges: 0,
                active_layers: 1,
                hierarchy_depth: 3,
                current_frame: 0,
                time_step: 0.016,
                temporal_decay: 0.1,
                history_weight: 0.8,
                force_scale: [1.0, 0.5, 0.25, 0.125],
                damping: [0.9, 0.85, 0.8, 0.75],
                temperature: [1.0; 4],

                rest_length: 50.0,
                repulsion_cutoff: 50.0,
                repulsion_softening_epsilon: 0.0001,
                center_gravity_k: 0.0,
                grid_cell_size: 50.0,
                warmup_iterations: 100,
                cooling_rate: 0.001,
                boundary_extreme_multiplier: 2.0,
                boundary_extreme_force_multiplier: 10.0,
                boundary_velocity_damping: 0.5,
                isolation_strength: 1.0,
                focus_gamma: 2.2,
                primary_focus_node: -1,
                context_alpha: 0.3,
                complexity_threshold: 0.5,
                saliency_boost: 1.5,
                information_bandwidth: 100.0,
                community_algorithm: 0,
                modularity_resolution: 1.0,
                topology_update_interval: 30,
                semantic_influence: 0.7,
                drift_threshold: 0.1,
                embedding_dims: 16,
                camera_position: Vec4::zero(),
                viewport_bounds: Vec4 {
                    x: 2000.0,
                    y: 2000.0,
                    z: 1000.0,
                    t: 100.0,
                },
                zoom_level: 1.0,
                time_window: 100.0,
            },
        }
    }

    pub fn with_nodes(mut self, count: i32) -> Self {
        self.params.total_nodes = count;
        self
    }

    pub fn with_edges(mut self, count: i32) -> Self {
        self.params.total_edges = count;
        self
    }

    pub fn with_focus(mut self, node_id: i32, gamma: f32) -> Self {
        self.params.primary_focus_node = node_id;
        self.params.focus_gamma = gamma;
        self
    }

    pub fn with_temporal_decay(mut self, decay: f32) -> Self {
        self.params.temporal_decay = decay;
        self
    }

    pub fn build(self) -> VisualAnalyticsParams {
        self.params
    }
}

pub struct VisualAnalyticsEngine {
    gpu: VisualAnalyticsGPU,
    params: VisualAnalyticsParams,
    nodes: Vec<TSNode>,
    edges: Vec<TSEdge>,
    layers: Vec<IsolationLayer>,
}

impl VisualAnalyticsEngine {
    pub async fn new(max_nodes: usize, max_edges: usize) -> Result<Self, GPUSafetyError> {
        let gpu = VisualAnalyticsGPU::new(
            max_nodes,
            max_edges,
            16,
            crate::utils::gpu_safety::GPUSafetyConfig::default(),
        )
        .await?;
        let params = VisualAnalyticsBuilder::new().build();

        Ok(Self {
            gpu,
            params,
            nodes: Vec::with_capacity(max_nodes),
            edges: Vec::with_capacity(max_edges),
            layers: vec![IsolationLayer::default(); 1],
        })
    }

    pub fn upsert_node(&mut self, id: usize, node: TSNode) {
        if id >= self.nodes.len() {
            self.nodes.resize(id + 1, TSNode::default());
        }
        self.nodes[id] = node;
    }

    pub fn add_edge(&mut self, edge: TSEdge) {
        self.edges.push(edge);
    }

    pub fn focus_on(&mut self, node_id: i32, radius: f32) {
        self.params.primary_focus_node = node_id;
        if !self.layers.is_empty() {
            self.layers[0].focus_center = if node_id >= 0 && (node_id as usize) < self.nodes.len() {
                self.nodes[node_id as usize].position
            } else {
                Vec4::zero()
            };
            self.layers[0].focus_radius = radius;
        }
    }

    pub async fn step(&mut self) -> Result<RenderData, GPUSafetyError> {
        self.gpu.stream_nodes(&self.nodes).await?;
        self.gpu.stream_edges(&self.edges).await?;
        self.gpu.update_layers(&self.layers).await?;

        self.gpu
            .execute(
                &self.params,
                self.nodes.len(),
                self.edges.len(),
                self.layers.len(),
            )
            .await?;

        let positions = self.gpu.get_positions().await?;
        let colors = self.gpu.get_colors().await?;
        let importance = self.gpu.get_importance().await?;

        self.params.current_frame += 1;

        Ok(RenderData {
            positions,
            colors,
            importance,
            frame: self.params.current_frame as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec4_validation() {
        assert!(Vec4::new(1.0, 2.0, 3.0, 4.0).is_ok());

        assert!(Vec4::new(f32::NAN, 2.0, 3.0, 4.0).is_err());

        assert!(Vec4::new(f32::INFINITY, 2.0, 3.0, 4.0).is_err());

        assert!(Vec4::new(1e7, 2.0, 3.0, 4.0).is_err());
    }

    #[test]
    fn test_ts_node_validation() {
        let mut node = TSNode::new();
        assert!(node.validate().is_ok());

        node.position = Vec4 {
            x: f32::NAN,
            y: 0.0,
            z: 0.0,
            t: 0.0,
        };
        assert!(node.validate().is_err());

        let mut node = TSNode::new();
        node.temporal_coherence = -0.5;
        assert!(node.validate().is_err());

        let mut node = TSNode::new();
        node.hierarchy_level = -1;
        assert!(node.validate().is_err());
    }

    #[test]
    fn test_ts_edge_validation() {
        assert!(TSEdge::new(0, 1).is_ok());

        assert!(TSEdge::new(-1, 1).is_err());

        assert!(TSEdge::new(5, 5).is_err());

        let edge = TSEdge::new(0, 1).unwrap();
        assert!(edge.validate(10).is_ok());
        assert!(edge.validate(1).is_err());
    }

    #[test]
    fn test_isolation_layer_validation() {
        let layer = IsolationLayer::new(0);
        assert!(layer.validate().is_ok());

        let mut layer = IsolationLayer::new(-1);
        assert!(layer.validate().is_err());

        let mut layer = IsolationLayer::new(0);
        layer.opacity = 1.5;
        assert!(layer.validate().is_err());

        let mut layer = IsolationLayer::new(0);
        layer.focus_radius = -10.0;
        assert!(layer.validate().is_err());
    }

    #[test]
    fn test_visual_analytics_params_validation() {
        let mut params = VisualAnalyticsParams {
            total_nodes: 1000,
            total_edges: 2000,
            active_layers: 1,
            hierarchy_depth: 3,
            current_frame: 0,
            time_step: 0.016,
            temporal_decay: 0.1,
            history_weight: 0.8,
            force_scale: [1.0, 0.5, 0.25, 0.125],
            damping: [0.9, 0.85, 0.8, 0.75],
            temperature: [1.0; 4],
            rest_length: 10.0,
            repulsion_cutoff: 50.0,
            repulsion_softening_epsilon: 0.001,
            center_gravity_k: 0.01,
            grid_cell_size: 20.0,
            warmup_iterations: 100,
            cooling_rate: 0.95,
            boundary_extreme_multiplier: 1.5,
            boundary_extreme_force_multiplier: 2.0,
            boundary_velocity_damping: 0.8,
            isolation_strength: 1.0,
            focus_gamma: 2.2,
            primary_focus_node: -1,
            context_alpha: 0.3,
            complexity_threshold: 0.5,
            saliency_boost: 1.5,
            information_bandwidth: 100.0,
            community_algorithm: 0,
            modularity_resolution: 1.0,
            topology_update_interval: 30,
            semantic_influence: 0.7,
            drift_threshold: 0.1,
            embedding_dims: 16,
            camera_position: Vec4::zero(),
            viewport_bounds: Vec4 {
                x: 2000.0,
                y: 2000.0,
                z: 1000.0,
                t: 100.0,
            },
            zoom_level: 1.0,
            time_window: 100.0,
        };

        assert!(params.validate().is_ok());

        params.total_nodes = -1;
        assert!(params.validate().is_err());

        params.total_nodes = 20_000_000;
        assert!(params.validate().is_err());

        params.total_nodes = 1000;
        params.time_step = -0.1;
        assert!(params.validate().is_err());
    }
}
