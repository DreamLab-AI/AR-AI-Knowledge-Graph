use actix_web::{web, HttpResponse, Responder};
use crate::AppState;
use crate::models::graph::GraphData;
use crate::utils::socket_flow_messages::BinaryNodeData;
use crate::models::node::Node;
use crate::types::vec3::Vec3Data;
use crate::models::edge::Edge;
use crate::models::simulation_params::{SimulationParams, SimulationPhase, SimulationMode};
use crate::actors::messages::{GetSettings, InitializeSwarm};
use crate::services::bots_client::BotsClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, debug, error, warn};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BotsAgent {
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
pub struct BotsEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub data_volume: f32,
    pub message_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotsDataRequest {
    pub nodes: Vec<BotsAgent>,
    pub edges: Vec<BotsEdge>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotsResponse {
    pub success: bool,
    pub message: String,
    pub nodes: Option<Vec<Node>>,
    pub edges: Option<Vec<Edge>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeSwarmRequest {
    pub topology: String,
    pub max_agents: u32,
    pub strategy: String,
    pub enable_neural: bool,
    pub agent_types: Vec<String>,
    pub custom_prompt: Option<String>,
}

// Static bots graph data storage
use once_cell::sync::Lazy;
static BOTS_GRAPH: Lazy<Arc<RwLock<GraphData>>> =
    Lazy::new(|| Arc::new(RwLock::new(GraphData::new())));

// Convert bots agents to graph nodes
fn convert_agents_to_nodes(agents: Vec<BotsAgent>) -> Vec<Node> {
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
            group: Some("bots".to_string()),
            user_data: None,
        }
    }).collect()
}

// Convert bots edges to graph edges
fn convert_bots_edges(edges: Vec<BotsEdge>, node_map: &HashMap<String, u32>) -> Vec<Edge> {
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

// Update bots data endpoint
pub async fn update_bots_data(
    state: web::Data<AppState>,
    bots_data: web::Json<BotsDataRequest>,
) -> impl Responder {
    info!("Received bots data update with {} agents and {} communications",
        bots_data.nodes.len(),
        bots_data.edges.len()
    );

    // Convert bots agents to nodes
    let nodes = convert_agents_to_nodes(bots_data.nodes.clone());

    // Create node ID mapping
    let node_map: HashMap<String, u32> = nodes.iter()
        .map(|node| (node.metadata_id.clone(), node.id))
        .collect();

    // Convert bots edges
    let edges = convert_bots_edges(bots_data.edges.clone(), &node_map);

    // Update bots graph data
    {
        let mut bots_graph = BOTS_GRAPH.write().await;
        bots_graph.nodes = nodes.clone();
        bots_graph.edges = edges.clone();

        debug!("Updated bots graph with {} nodes and {} edges",
            bots_graph.nodes.len(),
            bots_graph.edges.len()
        );
    }

    // Process with GPU physics if available
    if let Some(gpu_compute_addr) = &state.gpu_compute_addr {
        use crate::actors::messages::{UpdateGPUGraphData, UpdateSimulationParams, ComputeForces, GetNodeData};

        let bots_graph = BOTS_GRAPH.read().await;

        // Get physics settings
        let settings = match state.settings_addr.send(GetSettings).await {
            Ok(Ok(settings)) => settings,
            Ok(Err(e)) => {
                error!("Failed to get settings: {}", e);
                return HttpResponse::InternalServerError().json(BotsResponse {
                    success: false,
                    message: format!("Failed to get settings: {}", e),
                    nodes: None,
                    edges: None,
                });
            }
            Err(e) => {
                error!("Settings actor mailbox error: {}", e);
                return HttpResponse::InternalServerError().json(BotsResponse {
                    success: false,
                    message: format!("Settings service unavailable: {}", e),
                    nodes: None,
                    edges: None,
                });
            }
        };

        let physics_settings = settings.visualisation.physics.clone();

        // Create simulation parameters for bots
        let params = SimulationParams {
            iterations: physics_settings.iterations / 2, // Fewer iterations for bots
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
        info!("Processing bots layout with GPU");
        if let Err(e) = gpu_compute_addr.send(UpdateGPUGraphData {
            graph: bots_graph.clone(),
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
                // Update bots graph with new positions
                let mut bots_graph = BOTS_GRAPH.write().await;
                for (i, node) in bots_graph.nodes.iter_mut().enumerate() {
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

    HttpResponse::Ok().json(BotsResponse {
        success: true,
        message: "Bots data updated successfully".to_string(),
        nodes: Some(nodes),
        edges: Some(edges),
    })
}

// Get current bots data from BotsClient
pub async fn get_bots_data(state: web::Data<AppState>) -> impl Responder {
    debug!("get_bots_data endpoint called");

    // First try to get data from BotsClient
    if let Some(bots_update) = state.bots_client.get_latest_update().await {
        info!("Returning live bots data with {} agents", bots_update.agents.len());

        // Log agent details for debugging
        for agent in &bots_update.agents {
            debug!("Agent {}: {} ({}), status: {}, cpu: {}, health: {}, workload: {}",
                agent.id, agent.name, agent.agent_type, agent.status,
                agent.cpu_usage, agent.health, agent.workload);
        }

        // Convert BotsClient agents to our BotsAgent format
        let agents: Vec<BotsAgent> = bots_update.agents.iter().map(|agent| BotsAgent {
            id: agent.id.clone(),
            agent_type: agent.agent_type.clone(),
            status: agent.status.clone(),
            name: agent.name.clone(),
            cpu_usage: agent.cpu_usage,
            health: agent.health,
            workload: agent.workload,
        }).collect();

        // Convert to nodes for graph visualization
        let nodes = convert_agents_to_nodes(agents);

        // Create node ID mapping
        let _node_map: HashMap<String, u32> = nodes.iter()
            .map(|node| (node.metadata_id.clone(), node.id))
            .collect();

        // For now, we'll return empty edges as the BotsUpdate doesn't include them
        // In a real implementation, you'd want to add edge data to BotsUpdate
        let edges = Vec::new();

        let graph_data = GraphData {
            nodes,
            edges,
            metadata: HashMap::new(),
            id_to_metadata: HashMap::new(),
        };

        return HttpResponse::Ok().json(graph_data);
    }

    // Fall back to static data if no live data available
    let bots_graph = BOTS_GRAPH.read().await;

    info!("Returning bots data with {} nodes and {} edges",
        bots_graph.nodes.len(),
        bots_graph.edges.len()
    );

    // If no data exists, return some test data for visualization
    if bots_graph.nodes.is_empty() {
        info!("No bots data available, returning test data");

        // Create test agents
        let test_agents = vec![
            BotsAgent {
                id: "agent-1".to_string(),
                agent_type: "coordinator".to_string(),
                status: "active".to_string(),
                name: "Coordinator Alpha".to_string(),
                cpu_usage: 45.0,
                health: 95.0,
                workload: 0.7,
            },
            BotsAgent {
                id: "agent-2".to_string(),
                agent_type: "coder".to_string(),
                status: "active".to_string(),
                name: "Coder Beta".to_string(),
                cpu_usage: 78.0,
                health: 88.0,
                workload: 0.9,
            },
            BotsAgent {
                id: "agent-3".to_string(),
                agent_type: "tester".to_string(),
                status: "active".to_string(),
                name: "Tester Gamma".to_string(),
                cpu_usage: 32.0,
                health: 92.0,
                workload: 0.5,
            },
            BotsAgent {
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
            BotsEdge {
                id: "edge-1".to_string(),
                source: "agent-1".to_string(),
                target: "agent-2".to_string(),
                data_volume: 1024.0,
                message_count: 15,
            },
            BotsEdge {
                id: "edge-2".to_string(),
                source: "agent-1".to_string(),
                target: "agent-3".to_string(),
                data_volume: 512.0,
                message_count: 8,
            },
            BotsEdge {
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
        let edges = convert_bots_edges(test_edges, &node_map);

        let test_graph = GraphData {
            nodes,
            edges,
            metadata: HashMap::new(),
            id_to_metadata: HashMap::new(),
        };

        return HttpResponse::Ok().json(test_graph);
    }

    HttpResponse::Ok().json(&*bots_graph)
}

// Get bots node positions (for WebSocket updates)
pub async fn get_bots_positions(bots_client: &BotsClient) -> Vec<Node> {
    // Try to get live data from BotsClient first
    if let Some(bots_update) = bots_client.get_latest_update().await {
        // Convert BotsClient agents to nodes with positions
        let agents: Vec<BotsAgent> = bots_update.agents.iter().map(|agent| BotsAgent {
            id: agent.id.clone(),
            agent_type: agent.agent_type.clone(),
            status: agent.status.clone(),
            name: agent.name.clone(),
            cpu_usage: agent.cpu_usage,
            health: agent.health,
            workload: agent.workload,
        }).collect();

        return convert_agents_to_nodes(agents);
    }

    // Fall back to static data
    let bots_graph = BOTS_GRAPH.read().await;
    bots_graph.nodes.clone()
}

// Initialize swarm endpoint
pub async fn initialize_swarm(
    state: web::Data<AppState>,
    request: web::Json<InitializeSwarmRequest>,
) -> impl Responder {
    info!("=== INITIALIZE SWARM ENDPOINT CALLED ===");
    info!("Received swarm initialization request: {:?}", request);

    // Get the Claude Flow actor
    if let Some(claude_flow_addr) = &state.claude_flow_addr {
        // Send initialization message to ClaudeFlowActor
        match claude_flow_addr.send(InitializeSwarm {
            topology: request.topology.clone(),
            max_agents: request.max_agents,
            strategy: request.strategy.clone(),
            enable_neural: request.enable_neural,
            agent_types: request.agent_types.clone(),
            custom_prompt: request.custom_prompt.clone(),
        }).await {
            Ok(Ok(_)) => {
                info!("Swarm initialization request sent successfully");
                HttpResponse::Ok().json(serde_json::json!({
                    "success": true,
                    "message": "Swarm initialization started"
                }))
            }
            Ok(Err(e)) => {
                error!("Failed to initialize swarm: {}", e);
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
            Err(e) => {
                error!("Failed to send initialization message: {}", e);
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": "Failed to communicate with Claude Flow service"
                }))
            }
        }
    } else {
        error!("Claude Flow actor not available");
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "success": false,
            "error": "Claude Flow service not available"
        }))
    }
}

// Configure routes for bots endpoints
pub fn config(cfg: &mut web::ServiceConfig) {
    info!("Configuring bots routes:");
    info!("  - /bots/data (GET)");
    info!("  - /bots/update (POST)");
    info!("  - /bots/initialize-swarm (POST)");
    
    cfg.service(
        web::resource("/data")
            .route(web::get().to(get_bots_data))
    )
    .service(
        web::resource("/update")
            .route(web::post().to(update_bots_data))
    )
    .service(
        web::resource("/initialize-swarm")
            .route(web::post().to(initialize_swarm))
    );
}