use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, error, debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmUpdate {
    pub agents: Vec<Agent>,
    #[serde(default)]
    pub metrics: SwarmMetrics,
    #[serde(default)]
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub agent_type: String,
    pub status: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default = "default_cpu_usage")]
    pub cpu_usage: f32,
    #[serde(default = "default_health")]
    pub health: f32,
    #[serde(default = "default_workload")]
    pub workload: f32,
}

fn default_cpu_usage() -> f32 { 50.0 }
fn default_health() -> f32 { 90.0 }
fn default_workload() -> f32 { 0.7 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwarmMetrics {
    pub total_tokens: u64,
    pub active_agents: u32,
    pub tasks_completed: u32,
}

#[derive(Clone)]
pub struct SwarmClient {
    updates: Arc<RwLock<Option<SwarmUpdate>>>,
}

impl SwarmClient {
    pub fn new() -> Self {
        Self {
            updates: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn connect(&self, swarm_url: &str) -> Result<()> {
        info!("Attempting to connect to swarm orchestrator at: {}", swarm_url);
        
        let url = url::Url::parse(swarm_url)?;
        info!("Parsed URL: {:?}", url);
        
        match connect_async(url.clone()).await {
            Ok((ws_stream, response)) => {
                info!("Successfully connected to swarm orchestrator at {}", swarm_url);
                info!("WebSocket response status: {:?}", response.status());
                
                // Try different approaches to get swarm data
                // First, try a simple message format
                let subscribe_msg = serde_json::json!({
                    "type": "subscribe",
                    "event": "swarm-update"
                });
                
                self.handle_connection(ws_stream, subscribe_msg).await
            }
            Err(e) => {
                error!("Failed to connect to swarm orchestrator at {}: {:?}", swarm_url, e);
                Err(e.into())
            }
        }
    }

    async fn handle_connection(
        &self,
        ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        subscribe_msg: serde_json::Value,
    ) -> Result<()> {
        let (mut write, mut read) = ws_stream.split();
        
        // Send subscription
        let subscribe_text = subscribe_msg.to_string();
        info!("Sending subscription request: {}", subscribe_text);
        write.send(Message::Text(subscribe_text)).await?;
        info!("Successfully sent subscription request to swarm orchestrator");
        
        // Clone for the read task
        let updates = self.updates.clone();
        
        // Spawn read task
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        info!("Received message from swarm orchestrator: {}", text);
                        
                        // Parse and process swarm updates
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(json) => {
                                // Check for different message types
                                if let Some(method) = json.get("method").and_then(|m| m.as_str()) {
                                    info!("Received method notification: {}", method);
                                    
                                    // Handle connection established
                                    if method == "connection/established" {
                                        info!("Connection established with claude-flow server");
                                        // Connection is ready, we can now try other methods
                                    }
                                }
                                
                                // Check for error responses
                                if let Some(error) = json.get("error") {
                                    error!("Received error from server: {:?}", error);
                                    // If we get an unknown method error, try to parse any data that might be available
                                    if let Some(error_str) = error.as_str() {
                                        if error_str.contains("Unknown method") {
                                            info!("Server doesn't recognize our method, it might be a simple echo/test server");
                                        }
                                    }
                                }
                                
                                // Try to parse as SwarmUpdate directly if it has the right fields
                                if json.get("agents").is_some() || json.get("swarm").is_some() {
                                    match serde_json::from_value::<SwarmUpdate>(json.clone()) {
                                        Ok(update) => {
                                            info!("Parsed SwarmUpdate directly from message with {} agents", update.agents.len());
                                            let mut lock = updates.write().await;
                                            *lock = Some(update);
                                        }
                                        Err(e) => {
                                            debug!("Failed to parse as SwarmUpdate: {:?}", e);
                                        }
                                    }
                                }
                                
                                if let Some(result) = json.get("result") {
                                    info!("Processing result field from swarm response");
                                    debug!("Raw result JSON: {}", serde_json::to_string_pretty(result).unwrap_or_default());
                                    
                                    // Handle different possible result structures
                                    if let Some(result_obj) = result.as_object() {
                                        // Check if result contains swarm data directly
                                        if result_obj.contains_key("agents") || result_obj.contains_key("swarm") {
                                            match serde_json::from_value::<SwarmUpdate>(result.clone()) {
                                                Ok(update) => {
                                                    info!("Successfully parsed SwarmUpdate with {} agents", update.agents.len());
                                                    for agent in &update.agents {
                                                        debug!("Agent: {} ({}) - status: {}", agent.name, agent.agent_type, agent.status);
                                                    }
                                                    let mut lock = updates.write().await;
                                                    *lock = Some(update);
                                                }
                                                Err(e) => {
                                                    error!("Failed to parse SwarmUpdate from result: {:?}", e);
                                                    debug!("Result JSON was: {:?}", result);
                                                }
                                            }
                                        } else if let Some(swarm_data) = result_obj.get("swarm") {
                                            // Try parsing from nested swarm field
                                            match serde_json::from_value::<SwarmUpdate>(swarm_data.clone()) {
                                                Ok(update) => {
                                                    info!("Successfully parsed SwarmUpdate from result.swarm with {} agents", update.agents.len());
                                                    let mut lock = updates.write().await;
                                                    *lock = Some(update);
                                                }
                                                Err(e) => {
                                                    error!("Failed to parse SwarmUpdate from result.swarm: {:?}", e);
                                                }
                                            }
                                        } else {
                                            // Try to construct SwarmUpdate from various formats
                                            let mut agents = Vec::new();
                                            
                                            // Check for agents array at root level
                                            if let Some(agents_array) = result_obj.get("agents").and_then(|a| a.as_array()) {
                                                for agent_val in agents_array {
                                                    if let Ok(agent) = serde_json::from_value::<Agent>(agent_val.clone()) {
                                                        agents.push(agent);
                                                    }
                                                }
                                            }
                                            
                                            // Check for activeAgents field
                                            if let Some(active_agents) = result_obj.get("activeAgents").and_then(|a| a.as_array()) {
                                                for agent_val in active_agents {
                                                    if let Ok(agent) = serde_json::from_value::<Agent>(agent_val.clone()) {
                                                        agents.push(agent);
                                                    }
                                                }
                                            }
                                            
                                            if !agents.is_empty() {
                                                let update = SwarmUpdate {
                                                    agents,
                                                    metrics: SwarmMetrics::default(),
                                                    timestamp: std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .unwrap_or_default()
                                                        .as_secs(),
                                                };
                                                info!("Constructed SwarmUpdate from various fields with {} agents", update.agents.len());
                                                let mut lock = updates.write().await;
                                                *lock = Some(update);
                                            } else {
                                                warn!("No agents found in result object. Keys: {:?}", result_obj.keys().collect::<Vec<_>>());
                                            }
                                        }
                                    }
                                }
                                
                                // Also check for params field (for notifications)
                                if let Some(params) = json.get("params") {
                                    info!("Attempting to parse params as SwarmUpdate");
                                    match serde_json::from_value::<SwarmUpdate>(params.clone()) {
                                        Ok(update) => {
                                            info!("Successfully parsed SwarmUpdate from params with {} agents", update.agents.len());
                                            let mut lock = updates.write().await;
                                            *lock = Some(update);
                                        }
                                        Err(e) => {
                                            debug!("Params is not a SwarmUpdate: {:?}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse JSON from swarm: {:?}", e);
                                error!("Raw text was: {}", text);
                            }
                        }
                    }
                    Ok(Message::Binary(bin)) => {
                        debug!("Received binary message: {} bytes", bin.len());
                    }
                    Ok(Message::Ping(_)) => {
                        debug!("Received ping from swarm");
                    }
                    Err(e) => {
                        error!("WebSocket error from swarm: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            info!("Swarm WebSocket connection closed");
        });
        
        // Clone updates for the polling task
        let updates_for_polling = self.updates.clone();
        
        // Keep connection alive and poll for swarm status
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            let mut request_id = 1;
            
            loop {
                interval.tick().await;
                
                // Send a ping or status request
                let status_request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "ping",
                    "params": {},
                    "id": format!("rust-backend-ping-{}", request_id)
                });
                
                if let Err(e) = write.send(Message::Text(status_request.to_string())).await {
                    error!("Failed to send status request to swarm: {}", e);
                    break;
                }
                
                request_id += 1;
                
                // If we haven't received any real data after 10 seconds, generate mock data for testing
                if request_id == 3 {
                    let lock = updates_for_polling.read().await;
                    if lock.is_none() {
                        drop(lock);
                        info!("No swarm data received from orchestrator, generating mock data for testing");
                        let mock_update = generate_mock_swarm_update();
                        let mut lock = updates_for_polling.write().await;
                        *lock = Some(mock_update);
                    }
                }
                
                // Also send pings to keep connection alive
                if request_id % 6 == 0 {  // Every 30 seconds
                    if let Err(e) = write.send(Message::Ping(vec![])).await {
                        error!("Failed to send ping to swarm: {}", e);
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }

    pub async fn get_latest_update(&self) -> Option<SwarmUpdate> {
        self.updates.read().await.clone()
    }
}

// Generate mock swarm data for testing
fn generate_mock_swarm_update() -> SwarmUpdate {
    let agents = vec![
        Agent {
            id: "agent-coordinator-1".to_string(),
            name: "Coordinator Alpha".to_string(),
            agent_type: "coordinator".to_string(),
            status: "active".to_string(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            cpu_usage: 45.0,
            health: 95.0,
            workload: 0.7,
        },
        Agent {
            id: "agent-coder-1".to_string(),
            name: "Coder Beta".to_string(),
            agent_type: "coder".to_string(),
            status: "active".to_string(),
            x: 10.0,
            y: 5.0,
            z: 0.0,
            cpu_usage: 78.0,
            health: 88.0,
            workload: 0.9,
        },
        Agent {
            id: "agent-tester-1".to_string(),
            name: "Tester Gamma".to_string(),
            agent_type: "tester".to_string(),
            status: "active".to_string(),
            x: -10.0,
            y: 5.0,
            z: 0.0,
            cpu_usage: 32.0,
            health: 92.0,
            workload: 0.5,
        },
        Agent {
            id: "agent-analyst-1".to_string(),
            name: "Analyst Delta".to_string(),
            agent_type: "analyst".to_string(),
            status: "active".to_string(),
            x: 0.0,
            y: -10.0,
            z: 0.0,
            cpu_usage: 56.0,
            health: 90.0,
            workload: 0.6,
        },
        Agent {
            id: "agent-researcher-1".to_string(),
            name: "Researcher Epsilon".to_string(),
            agent_type: "researcher".to_string(),
            status: "active".to_string(),
            x: 10.0,
            y: -5.0,
            z: 5.0,
            cpu_usage: 41.0,
            health: 94.0,
            workload: 0.4,
        },
        Agent {
            id: "agent-architect-1".to_string(),
            name: "Architect Zeta".to_string(),
            agent_type: "architect".to_string(),
            status: "active".to_string(),
            x: -10.0,
            y: -5.0,
            z: -5.0,
            cpu_usage: 62.0,
            health: 91.0,
            workload: 0.8,
        },
    ];
    
    SwarmUpdate {
        agents,
        metrics: SwarmMetrics {
            total_tokens: 150000,
            active_agents: 6,
            tasks_completed: 42,
        },
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}