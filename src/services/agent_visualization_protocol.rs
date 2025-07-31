use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Initial JSON payload sent to clients for agent visualization setup
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentVisualizationMessage {
    /// Initial complete state - sent once on connection
    #[serde(rename = "init")]
    Initialize(InitializeMessage),
    
    /// Incremental position updates - sent frequently
    #[serde(rename = "positions")]
    PositionUpdate(PositionUpdateMessage),
    
    /// Agent state changes - sent on status/health changes
    #[serde(rename = "state")]
    StateUpdate(StateUpdateMessage),
    
    /// Connection changes - sent when edges are added/removed
    #[serde(rename = "connections")]
    ConnectionUpdate(ConnectionUpdateMessage),
    
    /// Performance metrics - sent periodically
    #[serde(rename = "metrics")]
    MetricsUpdate(MetricsUpdateMessage),
}

/// Complete initialization data for client setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeMessage {
    pub timestamp: i64, // Unix timestamp
    pub swarm_id: String,
    pub topology: String,
    
    /// All agents with full metadata
    pub agents: Vec<AgentInit>,
    
    /// All connections between agents
    pub connections: Vec<ConnectionInit>,
    
    /// Visual configuration for rendering
    pub visual_config: VisualConfig,
    
    /// Physics configuration for GPU solver
    pub physics_config: PhysicsConfig,
    
    /// Initial positions (optional - can be calculated client-side)
    pub positions: HashMap<String, Position>,
}

/// Agent initialization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInit {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub agent_type: String,
    pub status: String,
    
    /// Visual properties
    pub color: String,
    pub shape: String,  // "sphere", "cube", "cone", etc.
    pub size: f32,
    
    /// Performance metrics (0-1 normalized)
    pub health: f32,
    pub cpu: f32,
    pub memory: f32,
    pub activity: f32,
    
    /// Task information
    pub tasks_active: u32,
    pub tasks_completed: u32,
    pub success_rate: f32,
    
    /// Token usage
    pub tokens: u64,
    pub token_rate: f32,
    
    /// Additional metadata
    pub capabilities: Vec<String>,
    pub created_at: i64,
}

/// Connection initialization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInit {
    pub id: String,
    pub source: String,
    pub target: String,
    pub strength: f32,      // 0-1
    pub flow_rate: f32,     // 0-1
    pub color: String,
    pub active: bool,
}

/// Position update - sent frequently via WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdateMessage {
    pub timestamp: i64,
    pub positions: Vec<PositionUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Optional velocity for client-side interpolation
    pub vx: Option<f32>,
    pub vy: Option<f32>,
    pub vz: Option<f32>,
}

/// State changes for agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdateMessage {
    pub timestamp: i64,
    pub updates: Vec<AgentStateUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateUpdate {
    pub id: String,
    pub status: Option<String>,
    pub health: Option<f32>,
    pub cpu: Option<f32>,
    pub memory: Option<f32>,
    pub activity: Option<f32>,
    pub tasks_active: Option<u32>,
    pub current_task: Option<String>,
}

/// Connection updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionUpdateMessage {
    pub timestamp: i64,
    pub added: Vec<ConnectionInit>,
    pub removed: Vec<String>, // connection IDs
    pub updated: Vec<ConnectionStateUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStateUpdate {
    pub id: String,
    pub active: Option<bool>,
    pub flow_rate: Option<f32>,
    pub strength: Option<f32>,
}

/// Performance metrics update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsUpdateMessage {
    pub timestamp: i64,
    pub overall: SwarmMetrics,
    pub agent_metrics: Vec<AgentMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMetrics {
    pub total_agents: u32,
    pub active_agents: u32,
    pub health_avg: f32,
    pub cpu_total: f32,
    pub memory_total: f32,
    pub tokens_total: u64,
    pub tokens_per_second: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub id: String,
    pub tokens: u64,
    pub token_rate: f32,
    pub tasks_completed: u32,
    pub success_rate: f32,
}

/// Position type used in messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Visual configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConfig {
    pub colors: HashMap<String, String>,
    pub sizes: HashMap<String, f32>,
    pub animations: HashMap<String, AnimationConfig>,
    pub effects: EffectsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    pub speed: f32,
    pub amplitude: f32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsConfig {
    pub glow: bool,
    pub particles: bool,
    pub bloom: bool,
    pub shadows: bool,
}

/// Physics configuration for GPU solver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub spring_strength: f32,
    pub link_distance: f32,
    pub damping: f32,
    pub node_repulsion: f32,
    pub gravity_strength: f32,
    pub max_velocity: f32,
}

/// WebSocket protocol handler
pub struct AgentVisualizationProtocol {
    update_interval_ms: u64,
    position_buffer: Vec<PositionUpdate>,
}

impl AgentVisualizationProtocol {
    pub fn new() -> Self {
        Self {
            update_interval_ms: 16, // ~60fps
            position_buffer: Vec::new(),
        }
    }
    
    /// Create initial JSON message for new client
    pub fn create_init_message(
        swarm_id: &str,
        topology: &str,
        agents: Vec<crate::services::claude_flow::types::AgentStatus>,
    ) -> String {
        use crate::services::agent_visualization_processor::AgentVisualizationProcessor;
        
        let mut processor = AgentVisualizationProcessor::new();
        let viz_data = processor.create_visualization_packet(agents, swarm_id.to_string(), topology.to_string());
        
        // Convert to init message format
        let init_agents: Vec<AgentInit> = viz_data.agents.into_iter().map(|agent| {
            AgentInit {
                id: agent.id,
                name: agent.name,
                agent_type: agent.agent_type,
                status: agent.status,
                color: agent.color,
                shape: match agent.shape_type {
                    crate::services::agent_visualization_processor::ShapeType::Sphere => "sphere",
                    crate::services::agent_visualization_processor::ShapeType::Cube => "cube",
                    crate::services::agent_visualization_processor::ShapeType::Octahedron => "octahedron",
                    crate::services::agent_visualization_processor::ShapeType::Cylinder => "cylinder",
                    crate::services::agent_visualization_processor::ShapeType::Torus => "torus",
                    crate::services::agent_visualization_processor::ShapeType::Cone => "cone",
                    crate::services::agent_visualization_processor::ShapeType::Pyramid => "pyramid",
                }.to_string(),
                size: agent.size,
                health: agent.health,
                cpu: agent.cpu_usage,
                memory: agent.memory_usage,
                activity: agent.activity_level,
                tasks_active: agent.active_tasks,
                tasks_completed: agent.completed_tasks,
                success_rate: agent.success_rate,
                tokens: agent.token_usage,
                token_rate: agent.token_rate,
                capabilities: agent.metadata.capabilities,
                created_at: agent.metadata.created_at.timestamp(),
            }
        }).collect();
        
        let init_connections: Vec<ConnectionInit> = viz_data.connections.into_iter().map(|conn| {
            ConnectionInit {
                id: conn.id,
                source: conn.source_id,
                target: conn.target_id,
                strength: conn.strength,
                flow_rate: conn.flow_rate,
                color: conn.color,
                active: conn.is_active,
            }
        }).collect();
        
        let visual_config = VisualConfig {
            colors: viz_data.visual_config.color_scheme,
            sizes: viz_data.visual_config.size_multipliers,
            animations: HashMap::new(), // TODO: Populate from config
            effects: EffectsConfig {
                glow: true,
                particles: true,
                bloom: true,
                shadows: false,
            },
        };
        
        let init_msg = InitializeMessage {
            timestamp: chrono::Utc::now().timestamp(),
            swarm_id: swarm_id.to_string(),
            topology: topology.to_string(),
            agents: init_agents,
            connections: init_connections,
            visual_config,
            physics_config: viz_data.physics_config,
            positions: HashMap::new(), // Let client calculate initial positions
        };
        
        let message = AgentVisualizationMessage::Initialize(init_msg);
        serde_json::to_string(&message).unwrap_or_default()
    }
    
    /// Buffer position update
    pub fn add_position_update(&mut self, id: String, x: f32, y: f32, z: f32, vx: f32, vy: f32, vz: f32) {
        self.position_buffer.push(PositionUpdate {
            id,
            x,
            y,
            z,
            vx: Some(vx),
            vy: Some(vy),
            vz: Some(vz),
        });
    }
    
    /// Create position update message and clear buffer
    pub fn create_position_update(&mut self) -> Option<String> {
        if self.position_buffer.is_empty() {
            return None;
        }
        
        let msg = PositionUpdateMessage {
            timestamp: chrono::Utc::now().timestamp_millis(),
            positions: std::mem::take(&mut self.position_buffer),
        };
        
        let message = AgentVisualizationMessage::PositionUpdate(msg);
        Some(serde_json::to_string(&message).unwrap_or_default())
    }
    
    /// Create state update message
    pub fn create_state_update(updates: Vec<AgentStateUpdate>) -> String {
        let msg = StateUpdateMessage {
            timestamp: chrono::Utc::now().timestamp_millis(),
            updates,
        };
        
        let message = AgentVisualizationMessage::StateUpdate(msg);
        serde_json::to_string(&message).unwrap_or_default()
    }
}