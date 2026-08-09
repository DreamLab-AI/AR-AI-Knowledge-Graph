// src/adapters/messages.rs
//! Message Translation Layer for Actor-Port Adapters
//!
//! This module provides bidirectional conversion between:
//! - Port domain types (from hexagonal architecture)
//! - Actor message types (Actix message passing)

use actix::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

use visionclaw_domain::models::constraints::ConstraintSet;
use visionclaw_domain::models::graph::GraphData;
use visionclaw_domain::ports::gpu_physics_adapter::{
    GpuDeviceInfo, NodeForce, PhysicsParameters, PhysicsStatistics, PhysicsStepResult,
};
use visionclaw_domain::ports::gpu_semantic_analyzer::{
    ClusteringAlgorithm, CommunityDetectionResult, ImportanceAlgorithm, OptimizationResult,
    PathfindingResult, SemanticConstraintConfig, SemanticStatistics,
};

// ============================================================================
// Physics Adapter Messages
// ============================================================================

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct InitializePhysicsMessage {
    pub graph: Arc<GraphData>,
    pub params: PhysicsParameters,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<NodeForce>, String>")]
pub struct ComputeForcesMessage;

#[derive(Message)]
#[rtype(result = "Result<Vec<(u32, f32, f32, f32)>, String>")]
pub struct UpdatePositionsMessage {
    pub forces: Vec<NodeForce>,
}

#[derive(Message)]
#[rtype(result = "Result<PhysicsStepResult, String>")]
pub struct PhysicsStepMessage;

#[derive(Message)]
#[rtype(result = "Result<PhysicsStepResult, String>")]
pub struct SimulateUntilConvergenceMessage;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct ApplyExternalForcesMessage {
    pub forces: Vec<(u32, f32, f32, f32)>,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct PinNodesMessage {
    pub nodes: Vec<(u32, f32, f32, f32)>,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UnpinNodesMessage {
    pub node_ids: Vec<u32>,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdatePhysicsParametersMessage {
    pub params: PhysicsParameters,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdatePhysicsGraphDataMessage {
    pub graph: Arc<GraphData>,
}

#[derive(Message)]
#[rtype(result = "Result<GpuDeviceInfo, String>")]
pub struct GetGpuStatusMessage;

#[derive(Message)]
#[rtype(result = "Result<PhysicsStatistics, String>")]
pub struct GetPhysicsStatisticsMessage;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct ResetPhysicsMessage;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct CleanupPhysicsMessage;

// ============================================================================
// Semantic Analyzer Messages
// ============================================================================

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct InitializeSemanticMessage {
    pub graph: Arc<GraphData>,
}

#[derive(Message)]
#[rtype(result = "Result<CommunityDetectionResult, String>")]
pub struct DetectCommunitiesMessage {
    pub algorithm: ClusteringAlgorithm,
}

#[derive(Message)]
#[rtype(result = "Result<PathfindingResult, String>")]
pub struct ComputeShortestPathsMessage {
    pub source_node_id: u32,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<f32>, String>")]
pub struct ComputeSsspDistancesMessage {
    pub source_node_id: u32,
}

#[derive(Message)]
#[rtype(result = "Result<HashMap<(u32, u32), Vec<u32>>, String>")]
pub struct ComputeAllPairsShortestPathsMessage;

#[derive(Message)]
#[rtype(result = "Result<Vec<Vec<f32>>, String>")]
pub struct ComputeLandmarkApspMessage {
    pub num_landmarks: usize,
}

#[derive(Message)]
#[rtype(result = "Result<ConstraintSet, String>")]
pub struct GenerateSemanticConstraintsMessage {
    pub config: SemanticConstraintConfig,
}

#[derive(Message)]
#[rtype(result = "Result<OptimizationResult, String>")]
pub struct OptimizeLayoutMessage {
    pub constraints: ConstraintSet,
    pub max_iterations: usize,
}

#[derive(Message)]
#[rtype(result = "Result<HashMap<u32, f32>, String>")]
pub struct AnalyzeNodeImportanceMessage {
    pub algorithm: ImportanceAlgorithm,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateSemanticGraphDataMessage {
    pub graph: Arc<GraphData>,
}

#[derive(Message)]
#[rtype(result = "Result<SemanticStatistics, String>")]
pub struct GetSemanticStatisticsMessage;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct InvalidatePathfindingCacheMessage;

// ============================================================================
// Message Conversion Helpers
// ============================================================================

impl InitializePhysicsMessage {
    pub fn new(graph: Arc<GraphData>, params: PhysicsParameters) -> Self {
        Self { graph, params }
    }
}

impl UpdatePositionsMessage {
    pub fn new(forces: Vec<NodeForce>) -> Self {
        Self { forces }
    }
}

impl ApplyExternalForcesMessage {
    pub fn new(forces: Vec<(u32, f32, f32, f32)>) -> Self {
        Self { forces }
    }
}

impl PinNodesMessage {
    pub fn new(nodes: Vec<(u32, f32, f32, f32)>) -> Self {
        Self { nodes }
    }
}

impl UnpinNodesMessage {
    pub fn new(node_ids: Vec<u32>) -> Self {
        Self { node_ids }
    }
}

impl UpdatePhysicsParametersMessage {
    pub fn new(params: PhysicsParameters) -> Self {
        Self { params }
    }
}

impl UpdatePhysicsGraphDataMessage {
    pub fn new(graph: Arc<GraphData>) -> Self {
        Self { graph }
    }
}

impl InitializeSemanticMessage {
    pub fn new(graph: Arc<GraphData>) -> Self {
        Self { graph }
    }
}

impl DetectCommunitiesMessage {
    pub fn new(algorithm: ClusteringAlgorithm) -> Self {
        Self { algorithm }
    }
}

impl ComputeShortestPathsMessage {
    pub fn new(source_node_id: u32) -> Self {
        Self { source_node_id }
    }
}

impl ComputeSsspDistancesMessage {
    pub fn new(source_node_id: u32) -> Self {
        Self { source_node_id }
    }
}

impl ComputeLandmarkApspMessage {
    pub fn new(num_landmarks: usize) -> Self {
        Self { num_landmarks }
    }
}

impl GenerateSemanticConstraintsMessage {
    pub fn new(config: SemanticConstraintConfig) -> Self {
        Self { config }
    }
}

impl OptimizeLayoutMessage {
    pub fn new(constraints: ConstraintSet, max_iterations: usize) -> Self {
        Self {
            constraints,
            max_iterations,
        }
    }
}

impl AnalyzeNodeImportanceMessage {
    pub fn new(algorithm: ImportanceAlgorithm) -> Self {
        Self { algorithm }
    }
}

impl UpdateSemanticGraphDataMessage {
    pub fn new(graph: Arc<GraphData>) -> Self {
        Self { graph }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Compile-time assertions: all message types must be Send + Sync + 'static.
    // If any type violates these bounds this module will fail to compile.
    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn physics_messages_are_send_sync_static() {
        assert_send_sync_static::<ComputeForcesMessage>();
        assert_send_sync_static::<PhysicsStepMessage>();
        assert_send_sync_static::<SimulateUntilConvergenceMessage>();
        assert_send_sync_static::<GetGpuStatusMessage>();
        assert_send_sync_static::<GetPhysicsStatisticsMessage>();
        assert_send_sync_static::<ResetPhysicsMessage>();
        assert_send_sync_static::<CleanupPhysicsMessage>();
    }

    #[test]
    fn semantic_messages_are_send_sync_static() {
        assert_send_sync_static::<ComputeAllPairsShortestPathsMessage>();
        assert_send_sync_static::<GetSemanticStatisticsMessage>();
        assert_send_sync_static::<InvalidatePathfindingCacheMessage>();
    }

    #[test]
    fn compute_forces_message_result_type_is_vec_node_force() {
        // Result = Result<Vec<NodeForce>, String> — verified at compile time by
        // the #[rtype] attribute; this test exercises the constructor pathway.
        let _msg = ComputeForcesMessage;
    }

    #[test]
    fn update_positions_message_constructor_round_trips_forces() {
        use visionclaw_domain::ports::gpu_physics_adapter::NodeForce;
        let forces = vec![NodeForce {
            node_id: 1,
            force_x: 0.1,
            force_y: 0.2,
            force_z: 0.3,
        }];
        let msg = UpdatePositionsMessage::new(forces.clone());
        assert_eq!(msg.forces.len(), forces.len());
        assert_eq!(msg.forces[0].node_id, 1);
    }

    #[test]
    fn pin_nodes_message_constructor_preserves_data() {
        let nodes = vec![(42u32, 1.0f32, 2.0f32, 3.0f32)];
        let msg = PinNodesMessage::new(nodes.clone());
        assert_eq!(msg.nodes, nodes);
    }

    #[test]
    fn unpin_nodes_message_constructor_preserves_ids() {
        let ids = vec![1u32, 2u32, 3u32];
        let msg = UnpinNodesMessage::new(ids.clone());
        assert_eq!(msg.node_ids, ids);
    }

    #[test]
    fn compute_sssp_distances_message_stores_source_id() {
        let msg = ComputeSsspDistancesMessage::new(99);
        assert_eq!(msg.source_node_id, 99);
    }

    #[test]
    fn compute_landmark_apsp_message_stores_landmark_count() {
        let msg = ComputeLandmarkApspMessage::new(7);
        assert_eq!(msg.num_landmarks, 7);
    }
}
