use actix_web::{web, HttpResponse, Responder};
use crate::AppState;
use crate::models::graph::GraphData;
use crate::utils::socket_flow_messages::BinaryNodeData;
use crate::models::node::Node;
use crate::types::vec3::Vec3Data;
use crate::models::edge::Edge;
use crate::models::simulation_params::{SimulationParams, SimulationPhase, SimulationMode};
use crate::actors::messages::GetSettings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, debug, error, warn};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwarmAgent {
    pub id: String,
    #[serde(rename = "type")]
    pub agent_type: String,
    pub status: String,
    pub name: String,
    pub cpu_usage: f32,
    pub health: f32,
    pub workload: f32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwarmEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub data_volume: f32,
    pub message_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmDataRequest {
    pub nodes: Vec<SwarmAgent>,
    pub edges: Vec<SwarmEdge>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmResponse {
    pub success: bool,
    pub message: String,
    pub nodes: Option<Vec<Node>>,
    pub edges: Option<Vec<Edge>>,
}

// Static swarm graph data storage
use once_cell::sync::Lazy;
static SWARM_GRAPH: Lazy<Arc<RwLock<GraphData>>> = 
    Lazy::new(|| Arc::new(RwLock::new(GraphData::new())));

// Convert swarm agents to graph nodes
fn convert_agents_to_nodes(agents: Vec<SwarmAgent>) -> Vec<Node> {
    agents.into_iter().enumerate().map(|(idx, agent)| {
        // Map agent ID to numeric ID for physics processing
        let node_id = (idx + 1000) as u32; // Start at 1000 to avoid conflicts
        
        // Initialize position in a circular layout centered at origin
        let angle = (idx as f32 / 8.0) * std::f32::consts::TAU;
        let radius = 15.0;
        let position = Vec3Data::new(
            angle.cos() * radius,
            angle.sin() * radius,
            (idx as f32 - 2.0) * 2.0 // Slight vertical spread
        );
        
        // Calculate mass based on workload and CPU usage
        let mass = ((agent.workload + agent.cpu_usage / 100.0) / 2.0 * 10.0 + 1.0) as u8;
        
        Node {
            id: node_id,
            metadata_id: agent.id.clone(),
            label: agent.name,
            data: BinaryNodeData {
                position,
                velocity: Vec3Data::zero(),
                mass,
                flags: if agent.status == "active" { 1 } else { 0 },
                padding: [0, 0],
            },
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("type".to_string(), agent.agent_type.clone());
                meta.insert("status".to_string(), agent.status);
                meta.insert("health".to_string(), agent.health.to_string());
                meta.insert("cpu_usage".to_string(), agent.cpu_usage.to_string());
                meta.insert("workload".to_string(), agent.workload.to_string());
                meta
            },
            file_size: 0,
            node_type: Some(agent.agent_type),
            size: Some(mass as f32),
            color: None,
            weight: Some(agent.workload),
            group: Some("swarm".to_string()),
            user_data: None,
        }
    }).collect()
}

// Convert swarm edges to graph edges
fn convert_swarm_edges(edges: Vec<SwarmEdge>, node_map: &HashMap<String, u32>) -> Vec<Edge> {
    edges.into_iter().filter_map(|edge| {
        // Map string IDs to numeric IDs
        let source_id = node_map.get(&edge.source)?;
        let target_id = node_map.get(&edge.target)?;
        
        Some(Edge {
            id: edge.id,
            source: *source_id,
            target: *target_id,
            weight: (edge.data_volume / 1024.0).max(0.1), // Convert to KB, min 0.1
            edge_type: Some(format!("{} msgs", edge.message_count)),
            metadata: Some(HashMap::new()),
        })
    }).collect()
}

// Update swarm data endpoint
pub async fn update_swarm_data(
    state: web::Data<AppState>,
    swarm_data: web::Json<SwarmDataRequest>,
) -> impl Responder {
    info!("Received swarm data update with {} agents and {} communications", 
        swarm_data.nodes.len(), 
        swarm_data.edges.len()
    );
    
    // Convert swarm agents to nodes
    let nodes = convert_agents_to_nodes(swarm_data.nodes.clone());
    
    // Create node ID mapping
    let node_map: HashMap<String, u32> = nodes.iter()
        .map(|node| (node.metadata_id.clone(), node.id))
        .collect();
    
    // Convert swarm edges
    let edges = convert_swarm_edges(swarm_data.edges.clone(), &node_map);
    
    // Update swarm graph data
    {
        let mut swarm_graph = SWARM_GRAPH.write().await;
        swarm_graph.nodes = nodes.clone();
        swarm_graph.edges = edges.clone();
        
        debug!("Updated swarm graph with {} nodes and {} edges", 
            swarm_graph.nodes.len(), 
            swarm_graph.edges.len()
        );
    }
    
    // Process with GPU physics if available
    if let Some(gpu_compute_addr) = &state.gpu_compute_addr {
        use crate::actors::messages::{UpdateGPUGraphData, UpdateSimulationParams, ComputeForces, GetNodeData};
        
        let swarm_graph = SWARM_GRAPH.read().await;
        
        // Get physics settings
        let settings = match state.settings_addr.send(GetSettings).await {
            Ok(Ok(settings)) => settings,
            Ok(Err(e)) => {
                error!("Failed to get settings: {}", e);
                return HttpResponse::InternalServerError().json(SwarmResponse {
                    success: false,
                    message: format!("Failed to get settings: {}", e),
                    nodes: None,
                    edges: None,
                });
            }
            Err(e) => {
                error!("Settings actor mailbox error: {}", e);
                return HttpResponse::InternalServerError().json(SwarmResponse {
                    success: false,
                    message: format!("Settings service unavailable: {}", e),
                    nodes: None,
                    edges: None,
                });
            }
        };
        
        let physics_settings = settings.visualisation.physics.clone();
        
        // Create simulation parameters for swarm
        let params = SimulationParams {
            iterations: physics_settings.iterations / 2, // Fewer iterations for swarm
            spring_strength: physics_settings.spring_strength * 1.5, // Stronger attraction
            repulsion: physics_settings.repulsion_strength * 0.8, // Less repulsion
            damping: physics_settings.damping,
            max_repulsion_distance: physics_settings.repulsion_distance,
            viewport_bounds: physics_settings.bounds_size,
            mass_scale: physics_settings.mass_scale,
            boundary_damping: physics_settings.boundary_damping,
            enable_bounds: physics_settings.enable_bounds,
            time_step: 0.016,
            phase: SimulationPhase::Dynamic,
            mode: SimulationMode::Remote,
        };
        
        // Send graph data to GPU
        info!("Processing swarm layout with GPU");
        if let Err(e) = gpu_compute_addr.send(UpdateGPUGraphData {
            graph: swarm_graph.clone(),
        }).await {
            warn!("Failed to update GPU graph data: {}", e);
        }
        
        // Update simulation parameters
        if let Err(e) = gpu_compute_addr.send(UpdateSimulationParams {
            params,
        }).await {
            warn!("Failed to update simulation params: {}", e);
        }
        
        // Compute forces
        if let Err(e) = gpu_compute_addr.send(ComputeForces).await {
            warn!("Failed to compute forces: {}", e);
        }
        
        // Get updated node data
        match gpu_compute_addr.send(GetNodeData).await {
            Ok(Ok(node_data)) => {
                // Update swarm graph with new positions
                let mut swarm_graph = SWARM_GRAPH.write().await;
                for (i, node) in swarm_graph.nodes.iter_mut().enumerate() {
                    if i < node_data.len() {
                        node.data = node_data[i].clone();
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to get node data: {}", e);
            }
            Err(e) => {
                warn!("Failed to send GetNodeData: {}", e);
            }
        }
    }
    
    HttpResponse::Ok().json(SwarmResponse {
        success: true,
        message: "Swarm data updated successfully".to_string(),
        nodes: Some(nodes),
        edges: Some(edges),
    })
}

// Get current swarm data
pub async fn get_swarm_data(_state: web::Data<AppState>) -> impl Responder {
    let swarm_graph = SWARM_GRAPH.read().await;
    
    info!("Returning swarm data with {} nodes and {} edges", 
        swarm_graph.nodes.len(), 
        swarm_graph.edges.len()
    );
    
    // If no data exists, return some test data for visualization
    if swarm_graph.nodes.is_empty() {
        info!("No swarm data available, returning test data");
        
        // Create test agents
        let test_agents = vec![
            SwarmAgent {
                id: "agent-1".to_string(),
                agent_type: "coordinator".to_string(),
                status: "active".to_string(),
                name: "Coordinator Alpha".to_string(),
                cpu_usage: 45.0,
                health: 95.0,
                workload: 0.7,
            },
            SwarmAgent {
                id: "agent-2".to_string(),
                agent_type: "coder".to_string(),
                status: "active".to_string(),
                name: "Coder Beta".to_string(),
                cpu_usage: 78.0,
                health: 88.0,
                workload: 0.9,
            },
            SwarmAgent {
                id: "agent-3".to_string(),
                agent_type: "tester".to_string(),
                status: "active".to_string(),
                name: "Tester Gamma".to_string(),
                cpu_usage: 32.0,
                health: 92.0,
                workload: 0.5,
            },
            SwarmAgent {
                id: "agent-4".to_string(),
                agent_type: "analyst".to_string(),
                status: "active".to_string(),
                name: "Analyst Delta".to_string(),
                cpu_usage: 56.0,
                health: 90.0,
                workload: 0.6,
            },
        ];
        
        let test_edges = vec![
            SwarmEdge {
                id: "edge-1".to_string(),
                source: "agent-1".to_string(),
                target: "agent-2".to_string(),
                data_volume: 1024.0,
                message_count: 15,
            },
            SwarmEdge {
                id: "edge-2".to_string(),
                source: "agent-1".to_string(),
                target: "agent-3".to_string(),
                data_volume: 512.0,
                message_count: 8,
            },
            SwarmEdge {
                id: "edge-3".to_string(),
                source: "agent-2".to_string(),
                target: "agent-4".to_string(),
                data_volume: 2048.0,
                message_count: 22,
            },
        ];
        
        // Convert to graph format
        let nodes = convert_agents_to_nodes(test_agents);
        let node_map: HashMap<String, u32> = nodes.iter()
            .map(|node| (node.metadata_id.clone(), node.id))
            .collect();
        let edges = convert_swarm_edges(test_edges, &node_map);
        
        let test_graph = GraphData {
            nodes,
            edges,
            metadata: HashMap::new(),
            id_to_metadata: HashMap::new(),
        };
        
        return HttpResponse::Ok().json(test_graph);
    }
    
    HttpResponse::Ok().json(&*swarm_graph)
}

// Get swarm node positions (for WebSocket updates)
pub async fn get_swarm_positions() -> Vec<Node> {
    let swarm_graph = SWARM_GRAPH.read().await;
    swarm_graph.nodes.clone()
}

// Configure routes for swarm endpoints
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/data")
            .route(web::get().to(get_swarm_data))
    )
    .service(
        web::resource("/update")
            .route(web::post().to(update_swarm_data))
    );
}