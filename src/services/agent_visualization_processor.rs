use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::services::claude_flow::types::AgentStatus;

/// Processed agent data optimized for GPU visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizedAgent {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub status: String,
    
    // Visual attributes for GPU rendering
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: String,
    pub size: f32,
    pub glow_intensity: f32,
    
    // Performance metrics (0-1 normalized)
    pub health: f32,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub activity_level: f32,
    
    // Task information
    pub active_tasks: u32,
    pub completed_tasks: u32,
    pub success_rate: f32,
    pub current_task: Option<String>,
    
    // Token usage for visual weight
    pub token_usage: u64,
    pub token_rate: f32, // tokens per second
    
    // Metadata for rich tooltips
    pub metadata: AgentMetadata,
    
    // Shape hint for client rendering
    pub shape_type: ShapeType,
    pub animation_state: AnimationState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub created_at: DateTime<Utc>,
    pub age_seconds: u64,
    pub last_activity: DateTime<Utc>,
    pub capabilities: Vec<String>,
    pub error_count: u32,
    pub warning_count: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeType {
    Sphere,      // Default, healthy agents
    Cube,        // Busy/working agents
    Octahedron,  // Coordinators
    Cylinder,    // Analysts
    Torus,       // Testers
    Cone,        // Architects
    Pyramid,     // Error state
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationState {
    Idle,
    Pulsing,
    Rotating,
    Bouncing,
    Glowing,
    Flashing,  // For errors
}

/// Processed edge data for GPU rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizedConnection {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    
    // Visual attributes
    pub strength: f32,      // 0-1, affects line thickness
    pub flow_rate: f32,     // 0-1, affects particle speed
    pub color: String,
    pub particle_count: u32,
    
    // Data metrics
    pub data_volume: u64,   // Total bytes
    pub message_count: u64,
    pub latency_ms: f32,
    pub error_rate: f32,
    
    // Animation hints
    pub is_active: bool,
    pub pulse_frequency: f32,
}

/// Swarm overview for high-level visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmVisualization {
    pub swarm_id: String,
    pub topology: String,
    pub total_agents: u32,
    pub active_agents: u32,
    
    // Performance overview
    pub overall_health: f32,
    pub total_cpu_usage: f32,
    pub total_memory_usage: f32,
    pub total_token_usage: u64,
    pub tokens_per_second: f32,
    
    // Visual clustering hints
    pub clusters: Vec<AgentCluster>,
    
    // Time-series data for graphs
    pub performance_history: Vec<PerformanceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCluster {
    pub id: String,
    pub center: Vec3,
    pub radius: f32,
    pub agent_ids: Vec<String>,
    pub cluster_type: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub active_tasks: u32,
    pub token_rate: f32,
}

/// Complete visualization packet ready for GPU rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVisualizationData {
    pub timestamp: DateTime<Utc>,
    pub swarm: SwarmVisualization,
    pub agents: Vec<VisualizedAgent>,
    pub connections: Vec<VisualizedConnection>,
    
    // Physics hints for GPU solver
    pub physics_config: PhysicsConfig,
    
    // Visual configuration
    pub visual_config: VisualConfig,
}

// Use PhysicsConfig from agent_visualization_protocol module
use crate::services::agent_visualization_protocol::PhysicsConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConfig {
    pub color_scheme: HashMap<String, String>,
    pub size_multipliers: HashMap<String, f32>,
    pub glow_settings: GlowSettings,
    pub animation_speeds: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlowSettings {
    pub base_intensity: f32,
    pub health_multiplier: f32,
    pub activity_multiplier: f32,
    pub error_intensity: f32,
}

/// Main processor that transforms MCP data into visualization-ready format
pub struct AgentVisualizationProcessor {
    token_history: HashMap<String, Vec<(DateTime<Utc>, u64)>>,
    performance_history: HashMap<String, Vec<PerformanceSnapshot>>,
    last_update: DateTime<Utc>,
}

impl AgentVisualizationProcessor {
    pub fn new() -> Self {
        Self {
            token_history: HashMap::new(),
            performance_history: HashMap::new(),
            last_update: Utc::now(),
        }
    }
    
    /// Process raw agent data from MCP into visualization format
    pub fn process_agents(&mut self, agents: Vec<AgentStatus>) -> Vec<VisualizedAgent> {
        agents.into_iter().map(|agent| {
            let agent_type = agent.profile.agent_type.to_string();
            
            // Calculate normalized metrics
            let health = ((agent.success_rate as f32) / 100.0).clamp(0.0, 1.0);
            let cpu_usage = 0.5; // TODO: Get from system metrics
            let memory_usage = 0.3; // TODO: Get from system metrics
            let activity_level = if agent.active_tasks_count > 0 { 0.8 } else { 0.2 };
            
            // Determine visual properties based on agent state
            let (color, shape, animation) = self.get_visual_properties(&agent_type, &agent.status, health);
            
            // Calculate size based on workload
            let size = 1.0 + (agent.active_tasks_count as f32 * 0.2).min(2.0);
            
            // Glow intensity based on activity
            let glow_intensity = 0.3 + activity_level * 0.7;
            
            // Track token usage
            let token_usage = self.get_agent_token_usage(&agent.agent_id);
            let token_rate = self.calculate_token_rate(&agent.agent_id, token_usage);
            
            VisualizedAgent {
                id: agent.agent_id.clone(),
                name: agent.profile.name.clone(),
                agent_type: agent_type.clone(),
                status: agent.status.clone(),
                
                position: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, // Will be set by physics
                velocity: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
                color,
                size,
                glow_intensity,
                
                health,
                cpu_usage,
                memory_usage,
                activity_level,
                
                active_tasks: agent.active_tasks_count,
                completed_tasks: agent.completed_tasks_count,
                success_rate: (agent.success_rate as f32) / 100.0,
                current_task: agent.current_task.as_ref().map(|t| t.description.clone()),
                
                token_usage,
                token_rate,
                
                metadata: AgentMetadata {
                    created_at: agent.timestamp,
                    age_seconds: (Utc::now() - agent.timestamp).num_seconds() as u64,
                    last_activity: agent.timestamp,
                    capabilities: agent.profile.capabilities.clone(),
                    error_count: agent.failed_tasks_count,
                    warning_count: 0,
                    tags: vec![agent_type.clone(), agent.status.clone()],
                },
                
                shape_type: shape,
                animation_state: animation,
            }
        }).collect()
    }
    
    /// Get visual properties based on agent type and state
    fn get_visual_properties(&self, agent_type: &str, status: &str, health: f32) -> (String, ShapeType, AnimationState) {
        let color = match agent_type {
            "coordinator" => "#00FFFF",
            "coder" => "#00FF00",
            "architect" => "#FFA500",
            "analyst" => "#9370DB",
            "tester" => "#FF6347",
            "researcher" => "#FFD700",
            "reviewer" => "#4169E1",
            "optimizer" => "#7FFFD4",
            "documenter" => "#FF69B4",
            _ => "#CCCCCC",
        }.to_string();
        
        let shape = match agent_type {
            "coordinator" => ShapeType::Octahedron,
            "architect" => ShapeType::Cone,
            "analyst" => ShapeType::Cylinder,
            "tester" => ShapeType::Torus,
            _ => match status {
                "error" => ShapeType::Pyramid,
                "busy" => ShapeType::Cube,
                _ => ShapeType::Sphere,
            }
        };
        
        let animation = match status {
            "error" => AnimationState::Flashing,
            "busy" => AnimationState::Rotating,
            "idle" => AnimationState::Idle,
            _ => if health < 0.3 {
                AnimationState::Pulsing
            } else {
                AnimationState::Glowing
            }
        };
        
        (color, shape, animation)
    }
    
    /// Calculate token usage rate
    fn calculate_token_rate(&mut self, agent_id: &str, current_usage: u64) -> f32 {
        let now = Utc::now();
        let history = self.token_history.entry(agent_id.to_string()).or_insert_with(Vec::new);
        
        // Add current reading
        history.push((now, current_usage));
        
        // Keep only last 60 seconds of history
        let cutoff = now - chrono::Duration::seconds(60);
        history.retain(|(time, _)| *time > cutoff);
        
        // Calculate rate
        if history.len() < 2 {
            return 0.0;
        }
        
        let oldest = &history[0];
        let newest = &history[history.len() - 1];
        let time_diff = (newest.0 - oldest.0).num_seconds() as f32;
        
        if time_diff > 0.0 {
            ((newest.1 - oldest.1) as f32) / time_diff
        } else {
            0.0
        }
    }
    
    /// Get token usage for an agent (placeholder - should come from MCP)
    fn get_agent_token_usage(&self, agent_id: &str) -> u64 {
        // TODO: Get from actual MCP data
        1000 + (agent_id.len() as u64 * 100)
    }
    
    /// Create complete visualization data packet
    pub fn create_visualization_packet(
        &mut self,
        agents: Vec<AgentStatus>,
        swarm_id: String,
        topology: String,
    ) -> AgentVisualizationData {
        let processed_agents = self.process_agents(agents);
        
        // Calculate swarm metrics
        let total_agents = processed_agents.len() as u32;
        let active_agents = processed_agents.iter()
            .filter(|a| a.status != "idle" && a.status != "error")
            .count() as u32;
        
        let overall_health = processed_agents.iter()
            .map(|a| a.health)
            .sum::<f32>() / total_agents.max(1) as f32;
        
        let total_cpu_usage = processed_agents.iter()
            .map(|a| a.cpu_usage)
            .sum::<f32>();
        
        let total_token_usage = processed_agents.iter()
            .map(|a| a.token_usage)
            .sum::<u64>();
        
        let tokens_per_second = processed_agents.iter()
            .map(|a| a.token_rate)
            .sum::<f32>();
        
        // Create connections (placeholder - should come from MCP)
        let connections = self.create_connections(&processed_agents);
        
        // Create clusters based on agent types
        let clusters = self.create_clusters(&processed_agents);
        
        AgentVisualizationData {
            timestamp: Utc::now(),
            swarm: SwarmVisualization {
                swarm_id,
                topology,
                total_agents,
                active_agents,
                overall_health,
                total_cpu_usage,
                total_memory_usage: 0.0, // TODO
                total_token_usage,
                tokens_per_second,
                clusters,
                performance_history: vec![], // TODO: Implement history tracking
            },
            agents: processed_agents,
            connections,
            physics_config: PhysicsConfig {
                spring_strength: 0.3,
                link_distance: 25.0,
                damping: 0.92,
                node_repulsion: 20.0,
                gravity_strength: 0.08,
                max_velocity: 0.8,
            },
            visual_config: self.create_visual_config(),
        }
    }
    
    /// Create connections based on agent relationships
    fn create_connections(&self, agents: &[VisualizedAgent]) -> Vec<VisualizedConnection> {
        let mut connections = Vec::new();
        
        // Create connections between coordinators and other agents
        for coordinator in agents.iter().filter(|a| a.agent_type == "coordinator") {
            for agent in agents.iter().filter(|a| a.id != coordinator.id) {
                connections.push(VisualizedConnection {
                    id: format!("{}-{}", coordinator.id, agent.id),
                    source_id: coordinator.id.clone(),
                    target_id: agent.id.clone(),
                    strength: 0.5,
                    flow_rate: agent.activity_level,
                    color: "#4444FF".to_string(),
                    particle_count: 10,
                    data_volume: 1000,
                    message_count: 10,
                    latency_ms: 5.0,
                    error_rate: 0.0,
                    is_active: agent.active_tasks > 0,
                    pulse_frequency: 1.0,
                });
            }
        }
        
        connections
    }
    
    /// Create agent clusters for visual grouping
    fn create_clusters(&self, agents: &[VisualizedAgent]) -> Vec<AgentCluster> {
        let mut clusters = Vec::new();
        let mut type_groups: HashMap<String, Vec<String>> = HashMap::new();
        
        // Group agents by type
        for agent in agents {
            type_groups.entry(agent.agent_type.clone())
                .or_insert_with(Vec::new)
                .push(agent.id.clone());
        }
        
        // Create clusters
        for (agent_type, agent_ids) in type_groups {
            if agent_ids.len() > 1 {
                clusters.push(AgentCluster {
                    id: format!("cluster-{}", agent_type),
                    center: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, // Will be calculated by physics
                    radius: 15.0,
                    agent_ids,
                    cluster_type: agent_type.clone(),
                    color: self.get_cluster_color(&agent_type),
                });
            }
        }
        
        clusters
    }
    
    fn get_cluster_color(&self, agent_type: &str) -> String {
        match agent_type {
            "coordinator" => "#00FFFF33",
            "coder" => "#00FF0033",
            "architect" => "#FFA50033",
            _ => "#FFFFFF22",
        }.to_string()
    }
    
    fn create_visual_config(&self) -> VisualConfig {
        let mut color_scheme = HashMap::new();
        color_scheme.insert("background".to_string(), "#000033".to_string());
        color_scheme.insert("grid".to_string(), "#003366".to_string());
        color_scheme.insert("text".to_string(), "#FFFFFF".to_string());
        
        let mut size_multipliers = HashMap::new();
        size_multipliers.insert("coordinator".to_string(), 1.5);
        size_multipliers.insert("architect".to_string(), 1.3);
        size_multipliers.insert("default".to_string(), 1.0);
        
        let mut animation_speeds = HashMap::new();
        animation_speeds.insert("pulse".to_string(), 2.0);
        animation_speeds.insert("rotate".to_string(), 1.0);
        animation_speeds.insert("glow".to_string(), 0.5);
        
        VisualConfig {
            color_scheme,
            size_multipliers,
            glow_settings: GlowSettings {
                base_intensity: 0.3,
                health_multiplier: 0.5,
                activity_multiplier: 0.8,
                error_intensity: 1.0,
            },
            animation_speeds,
        }
    }
}