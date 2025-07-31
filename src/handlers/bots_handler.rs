use actix_web::{web, HttpResponse, Responder};
use crate::AppState;
use crate::models::graph::GraphData;
use crate::models::metadata::MetadataStore;
use crate::utils::socket_flow_messages::BinaryNodeData;
use crate::models::node::Node;
use crate::types::vec3::Vec3Data;
use crate::models::edge::Edge;
use crate::models::simulation_params::{SimulationParams, SimulationPhase, SimulationMode};
use crate::actors::messages::{GetSettings, InitializeSwarm, GetBotsGraphData};
use crate::services::bots_client::BotsClient;
// use crate::services::claude_flow::mcp_tools::McpTool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use log::{info, debug, error, warn};
use tokio::sync::RwLock;
use std::sync::Arc;
use chrono; // UPDATED: Added for timestamp generation
use glam::Vec3;

// UPDATED: Enhanced BotsAgent to match claude-flow hive-mind agent properties
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
    
    // UPDATED: Additional hive-mind properties
    #[serde(skip)]
    pub position: Vec3,
    #[serde(skip)]
    pub velocity: Vec3,
    #[serde(skip)]
    pub force: Vec3,
    #[serde(skip)]
    pub connections: Vec<String>,
    pub capabilities: Option<Vec<String>>,
    pub current_task: Option<String>,
    pub tasks_active: Option<u32>,
    pub tasks_completed: Option<u32>,
    pub success_rate: Option<f32>,
    pub tokens: Option<u64>,
    pub token_rate: Option<f32>,
    pub activity: Option<f32>,
    pub swarm_id: Option<String>,
    pub agent_mode: Option<String>, // centralized, distributed, strategic
    pub parent_queen_id: Option<String>,
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

// UPDATED: Function to fetch live agent data from claude-flow hive-mind via MCP
async fn fetch_hive_mind_agents(state: &AppState) -> Result<Vec<BotsAgent>, Box<dyn std::error::Error>> {
    if let Some(claude_flow_addr) = &state.claude_flow_addr {
        info!("Fetching live hive-mind agent data via ClaudeFlowActor");
        
        use crate::actors::claude_flow_actor::GetActiveAgents;
        use crate::services::claude_flow::{/*McpTool,*/ AgentType as ClaudeAgentType};
        
        match claude_flow_addr.send(GetActiveAgents).await {
            Ok(Ok(agent_statuses)) => {
                info!("Successfully fetched {} agents from ClaudeFlowActor", agent_statuses.len());
                
                // Convert MCP AgentStatus to BotsAgent
                let mut bots_agents: Vec<BotsAgent> = agent_statuses.into_iter().map(|agent| {
                    // Map claude-flow agent types to visualization types
                    let agent_type = match agent.profile.agent_type {
                        ClaudeAgentType::Coordinator => "coordinator",
                        ClaudeAgentType::Researcher => "researcher",
                        ClaudeAgentType::Coder => "coder",
                        ClaudeAgentType::Analyst => "analyst",
                        ClaudeAgentType::Architect => "architect",
                        ClaudeAgentType::Tester => "tester",
                        ClaudeAgentType::Reviewer => "reviewer",
                        ClaudeAgentType::Optimizer => "optimizer",
                        ClaudeAgentType::Documenter => "documenter",
                        ClaudeAgentType::Monitor => "monitor",
                        ClaudeAgentType::Specialist => "specialist"
                    };
                    
                    // Extract current task from metadata or use active task count
                    let current_task = agent.current_task.map(|t| t.description).or_else(|| {
                        if agent.active_tasks_count > 0 {
                            Some(format!("Processing {} active tasks", agent.active_tasks_count))
                        } else {
                            None
                        }
                    });
                    
                    // Calculate activity level (0-1 based on active tasks vs max)
                    let activity = if agent.profile.max_concurrent_tasks > 0 {
                        (agent.active_tasks_count as f32 / agent.profile.max_concurrent_tasks as f32).min(1.0)
                    } else {
                        0.0
                    };
                    
                    // Determine swarm_id from session or metadata
                    let swarm_id = agent.metadata.get("swarm_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| Some(agent.session_id.clone()));
                    
                    // Determine agent mode based on capabilities
                    let agent_mode = if agent.profile.capabilities.contains(&"distributed".to_string()) {
                        Some("distributed".to_string())
                    } else if agent.profile.capabilities.contains(&"strategic".to_string()) {
                        Some("strategic".to_string())
                    } else {
                        Some("centralized".to_string())
                    };
                    
                    // Calculate token rate (tokens per second)
                    let token_rate = if agent.total_execution_time > 0 {
                        (agent.metadata.get("total_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as f32) / (agent.total_execution_time as f32 / 1000.0)
                    } else {
                        0.0
                    };
                    
                    // Calculate CPU usage based on activity and execution time
                    let cpu_usage = (activity * 100.0 * 0.8 + (token_rate / 20.0).min(20.0)).min(100.0);
                    
                    // Calculate health based on success rate and activity
                    let health = agent.success_rate.min(100.0);
                    
                    // Calculate workload based on active tasks
                    let workload = activity;
                    
                    BotsAgent {
                        id: agent.agent_id,
                        agent_type: agent_type.to_string(),
                        status: agent.status,
                        name: agent.profile.name,
                        cpu_usage,
                        health: health as f32,
                        workload,
                        capabilities: Some(agent.profile.capabilities),
                        current_task: current_task,
                        tasks_active: Some(agent.active_tasks_count as u32),
                        tasks_completed: Some(agent.completed_tasks_count as u32),
                        success_rate: Some(agent.success_rate as f32),
                        tokens: agent.metadata.get("total_tokens").and_then(|v| v.as_u64()),
                        token_rate: Some(token_rate),
                        activity: Some(activity),
                        swarm_id,
                        agent_mode,
                        parent_queen_id: None, // Will be set based on hierarchy detection
                        // Add missing fields with default values
                        position: Vec3::new(0.0, 0.0, 0.0),
                        velocity: Vec3::ZERO,
                        force: Vec3::ZERO,
                        connections: vec![],
                    }
                }).collect();
                
                // Establish hierarchical relationships
                // Find Coordinator agents (acting as Queen-like leaders)
                let queen_ids: Vec<String> = bots_agents.iter()
                    .filter(|a| a.agent_type == "coordinator")
                    .map(|a| a.id.clone())
                    .collect();
                
                // If we have coordinators, assign parent relationships
                if !queen_ids.is_empty() {
                    // Create a mapping of swarm_id to coordinator_id
                    let swarm_coordinators: std::collections::HashMap<String, String> = bots_agents.iter()
                        .filter(|a| a.agent_type == "coordinator")
                        .filter_map(|a| a.swarm_id.as_ref().map(|s| (s.clone(), a.id.clone())))
                        .collect();
                    
                    for agent in &mut bots_agents {
                        if agent.agent_type != "coordinator" {
                            // Assign to a coordinator based on swarm_id or round-robin
                            if let Some(swarm_id) = &agent.swarm_id {
                                // Try to find a coordinator in the same swarm
                                agent.parent_queen_id = swarm_coordinators.get(swarm_id)
                                    .cloned()
                                    .or_else(|| queen_ids.first().cloned());
                            } else {
                                // Assign to first coordinator if no swarm_id
                                agent.parent_queen_id = queen_ids.first().cloned();
                            }
                        }
                    }
                }
                
                Ok(bots_agents)
            }
            Ok(Err(e)) => {
                error!("ClaudeFlowActor returned error: {}", e);
                Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)))
            }
            Err(e) => {
                error!("Failed to communicate with ClaudeFlowActor: {}", e);
                Err(Box::new(e))
            }
        }
    } else {
        // No claude-flow connection available (legacy MCP client path removed)
        warn!("No Claude Flow connection available (neither actor nor MCP client)");
        Ok(vec![]) // Return empty instead of error to allow graceful fallback
    }
}

// UPDATED: Enhanced agent to nodes conversion with hive-mind properties and Queen agent special handling
fn convert_agents_to_nodes(agents: Vec<BotsAgent>) -> Vec<Node> {
    agents.into_iter().enumerate().map(|(idx, agent)| {
        // Map agent ID to numeric ID for physics processing
        let node_id = (idx + 1000) as u32; // Start at 1000 to avoid conflicts

        // UPDATED: Enhanced positioning based on agent type and hierarchy
        let (_radius, vertical_offset) = match agent.agent_type.as_str() {
            "queen" => (0.0, 0.0), // Queen at center
            "coordinator" => (8.0, 0.0), // Inner ring
            "architect" | "design_architect" => (12.0, 2.0), // Architecture level
            "requirements_analyst" | "task_planner" => (15.0, -2.0), // Planning level
            "coder" | "implementation_coder" => (18.0, 0.0), // Implementation level
            "tester" | "quality_reviewer" => (18.0, 4.0), // QA level
            "reviewer" | "steering_documenter" => (20.0, 0.0), // Review level
            _ => (15.0, (idx as f32 - 2.0) * 1.5), // Default positioning
        };
        
        // Use the agent's position that was set by position_agents_hierarchically
        let position = Vec3Data::new(
            agent.position.x,
            agent.position.y,
            vertical_offset
        );

        // UPDATED: Enhanced mass calculation considering agent importance and activity
        let base_mass = match agent.agent_type.as_str() {
            "queen" => 15.0, // Queen agents are heaviest (central gravity)
            "coordinator" => 10.0,
            "architect" | "design_architect" => 8.0,
            _ => 5.0,
        };
        let activity_factor = agent.activity.unwrap_or(0.5);
        let workload_factor = agent.workload + agent.cpu_usage / 100.0;
        let mass = ((base_mass + workload_factor * 5.0 + activity_factor * 3.0) / 3.0).max(1.0) as u8;

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
                
                // UPDATED: Include hive-mind specific metadata
                if let Some(caps) = &agent.capabilities {
                    meta.insert("capabilities".to_string(), caps.join(","));
                }
                if let Some(task) = &agent.current_task {
                    meta.insert("current_task".to_string(), task.clone());
                }
                if let Some(active) = agent.tasks_active {
                    meta.insert("tasks_active".to_string(), active.to_string());
                }
                if let Some(completed) = agent.tasks_completed {
                    meta.insert("tasks_completed".to_string(), completed.to_string());
                }
                if let Some(rate) = agent.success_rate {
                    meta.insert("success_rate".to_string(), rate.to_string());
                }
                if let Some(tokens) = agent.tokens {
                    meta.insert("tokens".to_string(), tokens.to_string());
                }
                if let Some(swarm) = &agent.swarm_id {
                    meta.insert("swarm_id".to_string(), swarm.clone());
                }
                if let Some(mode) = &agent.agent_mode {
                    meta.insert("agent_mode".to_string(), mode.clone());
                }
                if let Some(queen) = &agent.parent_queen_id {
                    meta.insert("parent_queen_id".to_string(), queen.clone());
                }
                
                meta
            },
            file_size: 0,
            node_type: Some(agent.agent_type.clone()),
            size: Some(mass as f32),
            color: None,
            weight: Some(agent.workload),
            group: Some("bots".to_string()), // Could be enhanced to use swarm_id
            user_data: None,
        }
    }).collect()
}

// Create mock hive-mind data for demonstration
fn create_hive_mind_mock_data() -> Vec<BotsAgent> {
    vec![
        BotsAgent {
            id: "queen-alpha".to_string(),
            agent_type: "coordinator".to_string(),
            status: "active".to_string(),
            name: "Queen Coordinator Alpha".to_string(),
            cpu_usage: 25.0,
            health: 98.0,
            workload: 0.8,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            connections: vec![],
            capabilities: Some(vec![
                "strategic_planning".to_string(),
                "resource_allocation".to_string(),
                "coordination".to_string(),
                "quality_control".to_string()
            ]),
            current_task: Some("Coordinating full-stack development swarm".to_string()),
            tasks_active: Some(3),
            tasks_completed: Some(127),
            success_rate: Some(0.94),
            tokens: Some(45230),
            token_rate: Some(12.3),
            activity: Some(0.85),
            swarm_id: Some("hive-mind-001".to_string()),
            agent_mode: Some("centralized".to_string()),
            parent_queen_id: None,
        },
        BotsAgent {
            id: "researcher-beta".to_string(),
            agent_type: "researcher".to_string(),
            status: "active".to_string(),
            name: "Research Agent Beta".to_string(),
            cpu_usage: 67.0,
            health: 92.0,
            workload: 0.9,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            connections: vec![],
            capabilities: Some(vec![
                "data_gathering".to_string(),
                "analysis".to_string(),
                "report_generation".to_string()
            ]),
            current_task: Some("Researching best practices for authentication".to_string()),
            tasks_active: Some(2),
            tasks_completed: Some(89),
            success_rate: Some(0.91),
            tokens: Some(32100),
            token_rate: Some(8.7),
            activity: Some(0.92),
            swarm_id: Some("hive-mind-001".to_string()),
            agent_mode: Some("distributed".to_string()),
            parent_queen_id: Some("queen-alpha".to_string()),
        },
        BotsAgent {
            id: "coder-gamma".to_string(),
            agent_type: "coder".to_string(),
            status: "active".to_string(),
            name: "Implementation Coder Gamma".to_string(),
            cpu_usage: 89.0,
            health: 88.0,
            workload: 0.95,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            connections: vec![],
            capabilities: Some(vec![
                "code_generation".to_string(),
                "refactoring".to_string(),
                "debugging".to_string(),
                "api_development".to_string()
            ]),
            current_task: Some("Implementing JWT authentication service".to_string()),
            tasks_active: Some(6),
            tasks_completed: Some(156),
            success_rate: Some(0.93),
            tokens: Some(67800),
            token_rate: Some(15.2),
            activity: Some(0.97),
            swarm_id: Some("hive-mind-001".to_string()),
            agent_mode: Some("hierarchical".to_string()),
            parent_queen_id: Some("queen-alpha".to_string()),
        },
        BotsAgent {
            id: "tester-delta".to_string(),
            agent_type: "tester".to_string(),
            status: "idle".to_string(),
            name: "Test Engineer Delta".to_string(),
            cpu_usage: 32.0,
            health: 95.0,
            workload: 0.4,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            connections: vec![],
            capabilities: Some(vec![
                "unit_testing".to_string(),
                "integration_testing".to_string(),
                "validation".to_string()
            ]),
            current_task: None,
            tasks_active: Some(0),
            tasks_completed: Some(78),
            success_rate: Some(0.96),
            tokens: Some(23400),
            token_rate: Some(4.8),
            activity: Some(0.0),
            swarm_id: Some("hive-mind-001".to_string()),
            agent_mode: Some("distributed".to_string()),
            parent_queen_id: Some("queen-alpha".to_string()),
        },
    ]
}

// Position agents hierarchically based on their relationships
fn position_agents_hierarchically(agents: &mut Vec<BotsAgent>) {
    use glam::Vec3;
    use std::f32::consts::PI;
    
    // Find coordinators (acting as Queens)
    let coordinator_ids: Vec<String> = agents.iter()
        .filter(|a| a.agent_type == "coordinator")
        .map(|a| a.id.clone())
        .collect();
    
    if coordinator_ids.is_empty() {
        // No hierarchy, position in a circle
        let count = agents.len() as f32;
        for (i, agent) in agents.iter_mut().enumerate() {
            let angle = 2.0 * PI * i as f32 / count;
            agent.position = Vec3::new(angle.cos() * 500.0, angle.sin() * 500.0, 0.0);
        }
        return;
    }
    
    // Position coordinators at the center level
    let coordinator_count = coordinator_ids.len() as f32;
    for (i, coord_id) in coordinator_ids.iter().enumerate() {
        if let Some(coord) = agents.iter_mut().find(|a| &a.id == coord_id) {
            let angle = 2.0 * PI * i as f32 / coordinator_count;
            coord.position = Vec3::new(angle.cos() * 200.0, angle.sin() * 200.0, 100.0);
        }
    }
    
    // Group agents by their parent coordinator
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, agent) in agents.iter().enumerate() {
        if agent.agent_type != "coordinator" {
            if let Some(parent_id) = &agent.parent_queen_id {
                groups.entry(parent_id.clone()).or_insert_with(Vec::new).push(i);
            }
        }
    }
    
    // Position agents around their parent coordinator
    for (parent_id, child_indices) in groups {
        if let Some(parent) = agents.iter().find(|a| a.id == parent_id) {
            let parent_pos = parent.position;
            let child_count = child_indices.len() as f32;
            
            for (j, &child_idx) in child_indices.iter().enumerate() {
                let angle = 2.0 * PI * j as f32 / child_count;
                let _radius = 300.0 + (child_count * 10.0).min(200.0);
                let radius = _radius;
                let offset_x = angle.cos() * radius;
                let offset_y = angle.sin() * radius;
                
                if let Some(child) = agents.get_mut(child_idx) {
                    child.position = Vec3::new(
                        parent_pos.x + offset_x,
                        parent_pos.y + offset_y,
                        parent_pos.z - 50.0
                    );
                }
            }
        }
    }
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

// UPDATED: Get current bots data prioritizing claude-flow hive-mind system
pub async fn get_bots_data(state: web::Data<AppState>) -> impl Responder {
    debug!("get_bots_data endpoint called - fetching from hive-mind system");

    // UPDATED: First try to get live data from claude-flow hive-mind
    match fetch_hive_mind_agents(&**state).await {
        Ok(mut hive_agents) => {
            info!("Successfully fetched {} hive-mind agents", hive_agents.len());
            
            // Position agents hierarchically
            position_agents_hierarchically(&mut hive_agents);
            
            // Convert to nodes for graph visualization
            let nodes = convert_agents_to_nodes(hive_agents);
            
            // Create node ID mapping for edges
            let node_map: HashMap<String, u32> = nodes.iter()
                .map(|node| (node.metadata_id.clone(), node.id))
                .collect();
            
            // Generate hierarchical edges based on parent_queen_id relationships
            let mut edges = Vec::new();
            for node in &nodes {
                if let Some(parent_queen) = node.metadata.get("parent_queen_id") {
                    if let Some(parent_id) = node_map.get(parent_queen) {
                        edges.push(Edge {
                            id: format!("hierarchy-{}-{}", parent_queen, node.metadata_id),
                            source: *parent_id,
                            target: node.id,
                            weight: 2.0, // Strong hierarchical connection
                            edge_type: Some("hierarchy".to_string()),
                            metadata: Some({
                                let mut meta = HashMap::new();
                                meta.insert("type".to_string(), "hierarchy".to_string());
                                meta.insert("description".to_string(), "Queen-Agent hierarchy".to_string());
                                meta
                            }),
                        });
                    }
                }
            }
            
            let graph_data = GraphData {
                nodes,
                edges,
                metadata: MetadataStore::new(), // Use empty metadata for now
                id_to_metadata: HashMap::new(),
            };
            
            return HttpResponse::Ok().json(graph_data);
        }
        Err(e) => {
            warn!("Failed to fetch hive-mind data, falling back to BotsClient: {}", e);
        }
    }

    // Fallback: try to get data from BotsClient (legacy)
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
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            connections: vec![],
            capabilities: None,
            current_task: None,
            tasks_active: None,
            tasks_completed: None,
            success_rate: None,
            tokens: None,
            token_rate: None,
            activity: None,
            swarm_id: None,
            agent_mode: None,
            parent_queen_id: None,
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

    // Try to get data from GraphServiceActor first
    match state.graph_service_addr.send(GetBotsGraphData).await {
        Ok(Ok(graph_data)) => {
            info!("Returning bots data from GraphServiceActor with {} nodes and {} edges",
                graph_data.nodes.len(),
                graph_data.edges.len()
            );
            return HttpResponse::Ok().json(graph_data);
        }
        Ok(Err(e)) => {
            warn!("GraphServiceActor returned error: {}", e);
        }
        Err(e) => {
            warn!("Failed to communicate with GraphServiceActor: {}", e);
        }
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
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
                connections: vec![],
                capabilities: None,
                current_task: None,
                tasks_active: None,
                tasks_completed: None,
                success_rate: None,
                tokens: None,
                token_rate: None,
                activity: None,
                swarm_id: None,
                agent_mode: None,
                parent_queen_id: None,
            },
            BotsAgent {
                id: "agent-2".to_string(),
                agent_type: "coder".to_string(),
                status: "active".to_string(),
                name: "Coder Beta".to_string(),
                cpu_usage: 78.0,
                health: 88.0,
                workload: 0.9,
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
                connections: vec![],
                capabilities: None,
                current_task: None,
                tasks_active: None,
                tasks_completed: None,
                success_rate: None,
                tokens: None,
                token_rate: None,
                activity: None,
                swarm_id: None,
                agent_mode: None,
                parent_queen_id: None,
            },
            BotsAgent {
                id: "agent-3".to_string(),
                agent_type: "tester".to_string(),
                status: "active".to_string(),
                name: "Tester Gamma".to_string(),
                cpu_usage: 32.0,
                health: 92.0,
                workload: 0.5,
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
                connections: vec![],
                capabilities: None,
                current_task: None,
                tasks_active: None,
                tasks_completed: None,
                success_rate: None,
                tokens: None,
                token_rate: None,
                activity: None,
                swarm_id: None,
                agent_mode: None,
                parent_queen_id: None,
            },
            BotsAgent {
                id: "agent-4".to_string(),
                agent_type: "analyst".to_string(),
                status: "active".to_string(),
                name: "Analyst Delta".to_string(),
                cpu_usage: 56.0,
                health: 90.0,
                workload: 0.6,
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
                connections: vec![],
                capabilities: None,
                current_task: None,
                tasks_active: None,
                tasks_completed: None,
                success_rate: None,
                tokens: None,
                token_rate: None,
                activity: None,
                swarm_id: None,
                agent_mode: None,
                parent_queen_id: None,
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
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            connections: vec![],
            capabilities: None,
            current_task: None,
            tasks_active: None,
            tasks_completed: None,
            success_rate: None,
            tokens: None,
            token_rate: None,
            activity: None,
            swarm_id: None,
            agent_mode: None,
            parent_queen_id: None,
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
        // Send initialization message to ClaudeFlowActor with timeout
        let send_future = claude_flow_addr.send(InitializeSwarm {
            topology: request.topology.clone(),
            max_agents: request.max_agents,
            strategy: request.strategy.clone(),
            enable_neural: request.enable_neural,
            agent_types: request.agent_types.clone(),
            custom_prompt: request.custom_prompt.clone(),
        });
        
        // Add 5 second timeout to prevent indefinite hanging
        match tokio::time::timeout(std::time::Duration::from_secs(5), send_future).await {
            Ok(Ok(Ok(_))) => {
                info!("Swarm initialization request sent successfully");
                HttpResponse::Ok().json(serde_json::json!({
                    "success": true,
                    "message": "Swarm initialization started"
                }))
            }
            Ok(Ok(Err(e))) => {
                error!("Failed to initialize swarm: {}", e);
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
            Ok(Err(e)) => {
                error!("Failed to send initialization message: {}", e);
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": "Failed to communicate with Claude Flow service"
                }))
            }
            Err(_) => {
                error!("Timeout waiting for ClaudeFlowActor response");
                warn!("ClaudeFlowActor might be deadlocked or not processing messages");
                HttpResponse::GatewayTimeout().json(serde_json::json!({
                    "success": false,
                    "error": "Claude Flow service timeout - actor may be unresponsive"
                }))
            }
        }
    } else {
        warn!("Claude Flow actor not available - using mock response");
        
        // Return a mock successful response when ClaudeFlowActor is not available
        // This allows the UI to work even without MCP integration
        let mock_agents = vec![
            json!({
                "id": format!("agent-{}", uuid::Uuid::new_v4()),
                "type": "coordinator",
                "name": "Swarm Coordinator",
                "status": "initializing"
            }),
            json!({
                "id": format!("agent-{}", uuid::Uuid::new_v4()),
                "type": "researcher",
                "name": "Research Agent",
                "status": "initializing"
            }),
            json!({
                "id": format!("agent-{}", uuid::Uuid::new_v4()),
                "type": "coder",
                "name": "Code Agent",
                "status": "initializing"
            }),
        ];
        
        HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "message": "Swarm initialization started (mock mode)",
            "swarm_id": format!("swarm-{}", uuid::Uuid::new_v4()),
            "agents": mock_agents,
            "topology": request.topology.clone(),
            "max_agents": request.max_agents,
            "mock_mode": true
        }))
    }
}

// UPDATED: Enhanced agent status endpoint for real-time updates
pub async fn get_agent_status(state: web::Data<AppState>) -> impl Responder {
    debug!("get_agent_status endpoint called for real-time updates");
    
    match fetch_hive_mind_agents(&**state).await {
        Ok(agents) => {
            // Return lightweight status data for frequent polling
            let status_data: Vec<_> = agents.into_iter().map(|agent| {
                serde_json::json!({
                    "id": agent.id,
                    "type": agent.agent_type,
                    "status": agent.status,
                    "health": agent.health,
                    "cpu_usage": agent.cpu_usage,
                    "activity": agent.activity,
                    "current_task": agent.current_task,
                    "tasks_active": agent.tasks_active,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })
            }).collect();
            
            HttpResponse::Ok().json(serde_json::json!({
                "agents": status_data,
                "swarm_health": "active",
                "total_agents": status_data.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
        Err(e) => {
            error!("Failed to get agent status: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch agent status",
                "details": e.to_string()
            }))
        }
    }
}

// UPDATED: Enhanced configuration with new real-time endpoints
pub fn config(cfg: &mut web::ServiceConfig) {
    info!("Configuring bots routes:");
    info!("  - /bots/data (GET) - Full graph data");
    info!("  - /bots/status (GET) - Real-time agent status");
    info!("  - /bots/update (POST) - Update agent positions");
    info!("  - /bots/initialize-swarm (POST) - Initialize new swarm");
    
    cfg.service(
        web::resource("/data")
            .route(web::get().to(get_bots_data))
    )
    .service(
        web::resource("/status")
            .route(web::get().to(get_agent_status))
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