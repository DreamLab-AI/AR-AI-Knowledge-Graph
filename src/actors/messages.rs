//! Message definitions for actor system communication

use actix::prelude::*;
use glam::Vec3;
use serde_json::Value;
use std::collections::HashMap;
use crate::models::node::Node;
use crate::models::edge::Edge;
use crate::models::metadata::MetadataStore;
use crate::config::AppFullSettings;
use crate::models::graph::GraphData as ServiceGraphData;
use crate::utils::socket_flow_messages::BinaryNodeData;
use crate::models::simulation_params::SimulationParams;
use crate::models::graph::GraphData as ModelsGraphData;

// Graph Service Actor Messages
#[derive(Message)]
#[rtype(result = "Result<ServiceGraphData, String>")]
pub struct GetGraphData;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateNodePositions {
    pub positions: Vec<(u32, BinaryNodeData)>,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct AddNode {
    pub node: Node,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct RemoveNode {
    pub node_id: u32,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct AddEdge {
    pub edge: Edge,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct RemoveEdge {
    pub edge_id: String,
}

#[derive(Message)]
#[rtype(result = "Result<HashMap<u32, Node>, String>")]
pub struct GetNodeMap;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct BuildGraphFromMetadata {
    pub metadata: MetadataStore,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct StartSimulation;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateNodePosition {
    pub node_id: u32,
    pub position: Vec3,
    pub velocity: Vec3,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct SimulationStep;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct StopSimulation;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateGraphData {
    pub graph_data: ServiceGraphData,
}

// Settings Actor Messages
#[derive(Message)]
#[rtype(result = "Result<AppFullSettings, String>")]
pub struct GetSettings;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateSettings {
    pub settings: AppFullSettings,
}

#[derive(Message)]
#[rtype(result = "Result<Value, String>")]
pub struct GetSettingByPath {
    pub path: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct SetSettingByPath {
    pub path: String,
    pub value: Value,
}

// Metadata Actor Messages
#[derive(Message)]
#[rtype(result = "Result<MetadataStore, String>")]
pub struct GetMetadata;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateMetadata {
    pub metadata: MetadataStore,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct RefreshMetadata;

// Client Manager Actor Messages
#[derive(Message)]
#[rtype(result = "Result<usize, String>")]
pub struct RegisterClient {
    pub addr: actix::Addr<crate::handlers::socket_flow_handler::SocketFlowServer>,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UnregisterClient {
    pub client_id: usize,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct BroadcastNodePositions {
    pub positions: Vec<u8>,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct BroadcastMessage {
    pub message: String,
}

#[derive(Message)]
#[rtype(result = "Result<usize, String>")]
pub struct GetClientCount;

// Messages for ClientManagerActor to send to individual SocketFlowServer clients
#[derive(Message)]
#[rtype(result = "()")]
pub struct SendToClientBinary(pub Vec<u8>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendToClientText(pub String);

// Claude Flow Actor Messages
use crate::services::claude_flow::AgentStatus;
use crate::models::graph::GraphData;

#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateBotsGraph {
    pub agents: Vec<AgentStatus>,
}

#[derive(Message)]
#[rtype(result = "Result<GraphData, String>")]
pub struct GetBotsGraphData;

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct InitializeSwarm {
    pub topology: String,
    pub max_agents: u32,
    pub strategy: String,
    pub enable_neural: bool,
    pub agent_types: Vec<String>,
    pub custom_prompt: Option<String>,
}

// GPU Compute Actor Messages
#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct InitializeGPU {
    pub graph: ModelsGraphData,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateGPUGraphData {
    pub graph: ModelsGraphData,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateSimulationParams {
    pub params: SimulationParams,
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct ComputeForces;

#[derive(Message)]
#[rtype(result = "Result<Vec<BinaryNodeData>, String>")]
pub struct GetNodeData;

#[derive(Message)]
#[rtype(result = "GPUStatus")]
pub struct GetGPUStatus;

#[derive(Debug, Clone)]
pub struct GPUStatus {
    pub is_initialized: bool,
    pub cpu_fallback_active: bool,
    pub failure_count: u32,
    pub iteration_count: u32,
    pub num_nodes: u32,
}
