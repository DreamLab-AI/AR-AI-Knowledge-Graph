use actix::{prelude::*, Actor, Handler, Message};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{trace, debug, error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::app_state::AppState;
use crate::utils::binary_protocol;
use crate::types::vec3::Vec3Data;
use crate::utils::socket_flow_messages::{BinaryNodeData, PingMessage, PongMessage};

// Constants for throttling debug logs
const DEBUG_LOG_SAMPLE_RATE: usize = 10; // Only log 1 in 10 updates

// Default values for deadbands if not provided in settings
const DEFAULT_POSITION_DEADBAND: f32 = 0.01; // 1cm deadband
const DEFAULT_VELOCITY_DEADBAND: f32 = 0.005; // 5mm/s deadband
// Default values for dynamic update rate
const BATCH_UPDATE_WINDOW_MS: u64 = 200;  // Check motion every 200ms

// Note: Now using u32 node IDs throughout the system

/// Struct to hold pre-read WebSocket settings to avoid blocking in async context
#[derive(Clone, Debug)]
pub struct PreReadSocketSettings {
    pub min_update_rate: u32,
    pub max_update_rate: u32,
    pub motion_threshold: f32,
    pub motion_damping: f32,
    pub heartbeat_interval_ms: u64, // Added for heartbeat
    pub heartbeat_timeout_ms: u64,  // Added for heartbeat
}

// Old ClientManager struct removed - now using ClientManagerActor

// Message to set client ID after registration
#[derive(Message)]
#[rtype(result = "()")]
struct SetClientId(usize);

// Implement handler for SetClientId message
impl Handler<SetClientId> for SocketFlowServer {
    type Result = ();

    fn handle(&mut self, msg: SetClientId, _ctx: &mut Self::Context) -> Self::Result {
        self.client_id = Some(msg.0);
        info!("[WebSocket] Client assigned ID: {}", msg.0);
    }
}

// Implement handler for BroadcastPositionUpdate message
impl Handler<BroadcastPositionUpdate> for SocketFlowServer {
    type Result = ();

    fn handle(&mut self, msg: BroadcastPositionUpdate, ctx: &mut Self::Context) -> Self::Result {
        if !msg.0.is_empty() {
            // Encode the binary message
            let binary_data = binary_protocol::encode_node_data(&msg.0);
            
            // Send to client directly (permessage-deflate handles compression)
            ctx.binary(binary_data);
            
            // Debug logging - limit to avoid spamming logs
            if self.should_log_update() {
                trace!("[WebSocket] Position update sent: {} nodes", msg.0.len());
            }
        }
    }
}
/// Message type for broadcasting position updates to clients
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct BroadcastPositionUpdate(pub Vec<(u32, BinaryNodeData)>);

// Import the new messages
use crate::actors::messages::{SendToClientBinary, SendToClientText};

impl Handler<SendToClientBinary> for SocketFlowServer {
    type Result = ();

    fn handle(&mut self, msg: SendToClientBinary, ctx: &mut Self::Context) {
        ctx.binary(msg.0);
    }
}

impl Handler<SendToClientText> for SocketFlowServer {
    type Result = ();

    fn handle(&mut self, msg: SendToClientText, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

pub struct SocketFlowServer {
    app_state: Arc<AppState>,
    client_id: Option<usize>,
    client_manager_addr: actix::Addr<crate::actors::client_manager_actor::ClientManagerActor>,
    last_ping: Option<u64>,
    update_counter: usize, // Counter for throttling debug logs
    last_activity: std::time::Instant, // Track last activity time
    heartbeat_timer_set: bool, // Flag to track if heartbeat timer is set
    // Fields for batched updates and deadband filtering
    _node_position_cache: HashMap<String, BinaryNodeData>, // Dead Code: Field is never read
    last_sent_positions: HashMap<String, Vec3Data>,
    last_sent_velocities: HashMap<String, Vec3Data>,
    position_deadband: f32, // Minimum position change to trigger an update
    velocity_deadband: f32, // Minimum velocity change to trigger an update
    // Performance metrics
    last_transfer_size: usize,
    last_transfer_time: Instant,
    total_bytes_sent: usize,
    update_count: usize,
    nodes_sent_count: usize,
    
    // Dynamic update rate fields
    last_batch_time: Instant, // Last time we sent a batch of updates
    current_update_rate: u32,  // Current rate in updates per second
    // Store pre-read settings directly
    min_update_rate: u32,
    max_update_rate: u32,
    motion_threshold: f32,
    motion_damping: f32,
    // heartbeat_interval_ms: u64, // Unused
    // heartbeat_timeout_ms: u64, // Unused
    nodes_in_motion: usize,    // Counter for nodes currently in motion
    total_node_count: usize,   // Total node count for percentage calculation
    last_motion_check: Instant, // Last time we checked motion percentage,
}

impl SocketFlowServer {
    pub fn new(app_state: Arc<AppState>, pre_read_settings: PreReadSocketSettings, client_manager_addr: actix::Addr<crate::actors::client_manager_actor::ClientManagerActor>) -> Self {
        let min_update_rate = pre_read_settings.min_update_rate;
        let max_update_rate = pre_read_settings.max_update_rate;
        let motion_threshold = pre_read_settings.motion_threshold;
        let motion_damping = pre_read_settings.motion_damping;
        // let heartbeat_interval_ms = pre_read_settings.heartbeat_interval_ms; // Unused
        // let heartbeat_timeout_ms = pre_read_settings.heartbeat_timeout_ms; // Unused

        // Use position and velocity deadbands from constants
        let position_deadband = DEFAULT_POSITION_DEADBAND;
        let velocity_deadband = DEFAULT_VELOCITY_DEADBAND;

        // Start at max update rate and adjust dynamically based on motion
        let current_update_rate = max_update_rate;

        Self {
            app_state,
            client_id: None,
            client_manager_addr,
            last_ping: None,
            update_counter: 0,
            last_activity: std::time::Instant::now(),
            heartbeat_timer_set: false,
            _node_position_cache: HashMap::new(), // Dead Code: Field is never read
            last_sent_positions: HashMap::new(),
            last_sent_velocities: HashMap::new(),
            position_deadband,
            velocity_deadband,
            last_transfer_size: 0,
            last_transfer_time: Instant::now(),
            total_bytes_sent: 0,
            last_batch_time: Instant::now(),
            update_count: 0,
            nodes_sent_count: 0,
            current_update_rate,
            min_update_rate,
            max_update_rate,
            motion_threshold,
            motion_damping,
            // heartbeat_interval_ms, // Unused
            // heartbeat_timeout_ms, // Unused
            nodes_in_motion: 0,
            total_node_count: 0,
            last_motion_check: Instant::now()
        }
    }

    fn handle_ping(&mut self, msg: PingMessage) -> PongMessage {
        self.last_ping = Some(msg.timestamp);
        PongMessage {
            type_: "pong".to_string(),
            timestamp: msg.timestamp,
        }
    }
    
    
    // Helper method to determine if we should log this update (for throttling)
    fn should_log_update(&mut self) -> bool {
        self.update_counter = (self.update_counter + 1) % DEBUG_LOG_SAMPLE_RATE;
        self.update_counter == 0
    }
    
    // Check if a node's position or velocity has changed enough to warrant an update
    fn has_node_changed_significantly(&mut self, node_id: &str, new_position: Vec3Data, new_velocity: Vec3Data) -> bool {
        let position_changed = if let Some(last_position) = self.last_sent_positions.get(node_id) {
            // Calculate Euclidean distance between last sent position and new position
            let dx = new_position.x - last_position.x;
            let dy = new_position.y - last_position.y;
            let dz = new_position.z - last_position.z;
            let distance_squared = dx*dx + dy*dy + dz*dz;
            
            // Check if position has changed by more than the deadband
            distance_squared > self.position_deadband * self.position_deadband
        } else {
            // First time seeing this node, always consider it changed
            true
        };
        
        let velocity_changed = if let Some(last_velocity) = self.last_sent_velocities.get(node_id) {
            // Calculate velocity change magnitude
            let dvx = new_velocity.x - last_velocity.x;
            let dvy = new_velocity.y - last_velocity.y;
            let dvz = new_velocity.z - last_velocity.z;
            let velocity_change_squared = dvx*dvx + dvy*dvy + dvz*dvz;
            
            // Check if velocity has changed by more than the deadband
            velocity_change_squared > self.velocity_deadband * self.velocity_deadband
        } else {
            // First time seeing this node's velocity, always consider it changed
            true
        };
        
        // Update stored values if changed
        if position_changed || velocity_changed {
            self.last_sent_positions.insert(node_id.to_string(), new_position);
            self.last_sent_velocities.insert(node_id.to_string(), new_velocity);
            return true;
        }
        
        false
    }

    // Calculate the current update interval based on the dynamic rate
    fn get_current_update_interval(&self) -> std::time::Duration {
        let millis = (1000.0 / self.current_update_rate as f64) as u64;
        std::time::Duration::from_millis(millis)
    }
    
    // Calculate the percentage of nodes in motion
    fn calculate_motion_percentage(&self) -> f32 {
        if self.total_node_count == 0 {
            return 0.0;
        }
        
        (self.nodes_in_motion as f32) / (self.total_node_count as f32)
    }
    
    // Update the dynamic rate based on current motion
    fn update_dynamic_rate(&mut self) {
        // Only recalculate periodically to avoid rapid changes
        let now = Instant::now();
        let batch_window = std::time::Duration::from_millis(BATCH_UPDATE_WINDOW_MS);
        let elapsed = now.duration_since(self.last_batch_time);
        
        // If we've waited at least the batch window time, or this is the first update
        if elapsed >= batch_window {
            // Calculate the current motion percentage
            let motion_pct = self.calculate_motion_percentage();
            
            // Adjust the update rate based on the motion percentage
            if motion_pct > self.motion_threshold {
                // Gradually increase rate for high motion scenarios
                self.current_update_rate = ((self.current_update_rate as f32) * self.motion_damping + 
                                           (self.max_update_rate as f32) * (1.0 - self.motion_damping)) as u32;
            } else {
                // Gradually decrease rate for low motion scenarios
                self.current_update_rate = ((self.current_update_rate as f32) * self.motion_damping + 
                                           (self.min_update_rate as f32) * (1.0 - self.motion_damping)) as u32;
            }
            
            // Ensure rate stays within min and max bounds
            self.current_update_rate = self.current_update_rate.clamp(self.min_update_rate, self.max_update_rate);
            
            // Update the last motion check time
            self.last_motion_check = now;
        }
    }

    // New method to mark a batch as sent
    // fn mark_batch_sent(&mut self) { self.last_batch_time = Instant::now(); } // Dead Code
    
    // New method to collect nodes that have changed position
    // fn collect_changed_nodes(&mut self) -> Vec<(u16, BinaryNodeData)> { // Dead Code
    //     let mut changed_nodes = Vec::new();
        
    //     for (node_id, node_data) in self._node_position_cache.drain() { // Adjusted to use _node_position_cache
    //         if let Ok(node_id_u16) = node_id.parse::<u16>() {
    //             changed_nodes.push((node_id_u16, node_data));
    //         }
    //     }
        
    //     changed_nodes
    // }
}

impl Actor for SocketFlowServer {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Register this client with the client manager actor
        let addr = ctx.address();
        let addr_clone = addr.clone();
        
        // Use actix's runtime to avoid blocking in the actor's started method
        let cm_addr = self.client_manager_addr.clone();
        actix::spawn(async move {
            use crate::actors::messages::RegisterClient;
            match cm_addr.send(RegisterClient { addr: addr_clone }).await {
                Ok(Ok(id)) => {
                    // Send a message back to the actor with its client ID
                    addr.do_send(SetClientId(id));
                },
                Ok(Err(e)) => {
                    error!("ClientManagerActor failed to register client: {}", e);
                },
                Err(e) => {
                    error!("Failed to send RegisterClient message to ClientManagerActor: {}", e);
                }
            }
        });
    
        info!("[WebSocket] New client connected");
        self.last_activity = std::time::Instant::now();
        
        // We'll retrieve client ID asynchronously via message
        self.client_id = None;

        // Set up server-side heartbeat ping to keep connection alive
        if !self.heartbeat_timer_set {
            ctx.run_interval(std::time::Duration::from_secs(5), |act, ctx| {
                // Send a heartbeat ping every 5 seconds
                trace!("[WebSocket] Sending server heartbeat ping");
                ctx.ping(b"");
                
                // Update last activity timestamp to prevent client-side timeout
                act.last_activity = std::time::Instant::now();
            });
        }

        // Send simple connection established message
        let response = serde_json::json!({
            "type": "connection_established",
            "timestamp": chrono::Utc::now().timestamp_millis()
        });

        if let Ok(msg_str) = serde_json::to_string(&response) {
            ctx.text(msg_str);
            self.last_activity = std::time::Instant::now();
        }

        // Send a "loading" message to indicate the client should display a loading indicator
        let loading_msg = serde_json::json!({
            "type": "loading",
            "message": "Calculating initial layout..."
        });
        ctx.text(serde_json::to_string(&loading_msg).unwrap_or_default());
        self.last_activity = std::time::Instant::now();
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        // Unregister this client when it disconnects
        if let Some(client_id) = self.client_id {
            let cm_addr = self.client_manager_addr.clone();
            actix::spawn(async move {
                use crate::actors::messages::UnregisterClient;
                if let Err(e) = cm_addr.send(UnregisterClient { client_id }).await {
                    error!("Failed to unregister client from ClientManagerActor: {}", e);
                }
            });
            info!("[WebSocket] Client {} disconnected", client_id);
        }
    }
}

// Helper function to fetch nodes without borrowing from the actor
// Update signature to work with actor system
async fn fetch_nodes(
    app_state: Arc<AppState>,
    settings_addr: actix::Addr<crate::actors::settings_actor::SettingsActor>
) -> Option<(Vec<(u32, BinaryNodeData)>, bool)> {
    // Fetch raw nodes asynchronously from GraphServiceActor
    use crate::actors::messages::GetGraphData;
    let graph_data = match app_state.graph_service_addr.send(GetGraphData).await {
        Ok(Ok(data)) => data,
        Ok(Err(e)) => {
            error!("[WebSocket] Failed to get graph data: {}", e);
            return None;
        },
        Err(e) => {
            error!("[WebSocket] Failed to send message to GraphServiceActor: {}", e);
            return None;
        }
    };
    
    if graph_data.nodes.is_empty() {
        debug!("[WebSocket] No nodes to send! Empty graph data.");
        return None;
    }

    // Get debug settings from SettingsActor
    use crate::actors::messages::GetSettingByPath;
    let debug_enabled = match settings_addr.send(GetSettingByPath { path: "system.debug.enabled".to_string() }).await {
        Ok(Ok(value)) => value.as_bool().unwrap_or(false),
        _ => false,
    };
    let debug_websocket = match settings_addr.send(GetSettingByPath { path: "system.debug.enable_websocket_debug".to_string() }).await {
        Ok(Ok(value)) => value.as_bool().unwrap_or(false),
        _ => false,
    };
    let detailed_debug = debug_enabled && debug_websocket;

    if detailed_debug {
        debug!("Raw nodes count: {}, showing first 5 nodes IDs:", graph_data.nodes.len());
        for (i, node) in graph_data.nodes.iter().take(5).enumerate() {
            debug!("  Node {}: id={} (numeric), metadata_id={} (filename)",
                i, node.id, node.metadata_id);
        }
    }
    
    let mut nodes = Vec::with_capacity(graph_data.nodes.len());
    for node in &graph_data.nodes { // Iterate over a slice
        // node.id is already a u32, no need to parse
        let node_id = node.id;
        let node_data = BinaryNodeData {
            position: node.data.position,
            velocity: node.data.velocity,
            mass: node.data.mass,
            flags: node.data.flags,
            padding: node.data.padding,
        };
        nodes.push((node_id, node_data));
    }
    
    if nodes.is_empty() {
        return None;
    }
    
    // Return nodes and debug flag
    Some((nodes, detailed_debug))
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SocketFlowServer {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                debug!("[WebSocket] Received ping");
                ctx.pong(&msg);
                self.last_activity = std::time::Instant::now();
            }
            Ok(ws::Message::Pong(_)) => {
                // Logging every pong creates too much noise, only log in detailed debug mode
                // Note: We'll skip the debug check here to avoid blocking the actor
                self.last_activity = std::time::Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                info!("Received text message: {}", text);
                self.last_activity = std::time::Instant::now();
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(msg) => {
                        match msg.get("type").and_then(|t| t.as_str()) {
                            Some("ping") => {
                                if let Ok(ping_msg) =
                                    serde_json::from_value::<PingMessage>(msg.clone())
                                {
                                    let pong = self.handle_ping(ping_msg);
                                    self.last_activity = std::time::Instant::now();
                                    if let Ok(response) = serde_json::to_string(&pong) {
                                        ctx.text(response);
                                    }
                                }
                            }
                            Some("requestInitialData") => {
                                info!("Client requested initial data - sending authoritative server state");

                                // Use a smaller initial interval to start updates quickly
                                let initial_interval = std::time::Duration::from_millis(10);
                                let app_state = self.app_state.clone();
                                let settings_addr = self.app_state.settings_addr.clone();
                                
                                // First check if we should log this update
                                let should_log = self.should_log_update();
                                
                                ctx.run_later(initial_interval, move |_act, ctx| {
                                    // Wrap the async function in an actor future
                                    let fut = fetch_nodes(app_state.clone(), settings_addr.clone());
                                    let fut = actix::fut::wrap_future::<_, Self>(fut);
                                    
                                    ctx.spawn(fut.map(move |result, act, ctx| {
                                        if let Some((nodes, detailed_debug)) = result {
                                            // Now that we're back in the actor context, we can filter the nodes
                                            // Filter nodes to only include those that have changed significantly
                                            let mut filtered_nodes = Vec::new();
                                            for (node_id, node_data) in &nodes {
                                                let node_id_str = node_id.to_string();
                                                let position = node_data.position.clone();
                                                let velocity = node_data.velocity.clone();
                                                
                                                // Apply filtering before adding to filtered nodes
                                                if act.has_node_changed_significantly(
                                                    &node_id_str,
                                                    position.clone(),
                                                    velocity.clone()
                                                ) {
                                                    filtered_nodes.push((*node_id, node_data.clone()));
                                                }
                                                
                                                if detailed_debug && filtered_nodes.len() <= 5 {
                                                    debug!("Including node {} in update", node_id_str);
                                                }
                                            }
                                            
                                            // If no nodes have changed significantly, don't send an update
                                            if filtered_nodes.is_empty() {
                                                return;
                                            }
                                            
                                            // Encode only the nodes that have changed significantly
                                            let binary_data = binary_protocol::encode_node_data(&filtered_nodes);
                                            
                                            // Update motion metrics for dynamic rate adjustment
                                            act.total_node_count = filtered_nodes.len();
                                              
                                            // Count nodes in motion (with non-zero velocity)
                                            let moving_nodes = filtered_nodes.iter()
                                                .filter(|(_, node_data)| {
                                                    let vel = &node_data.velocity;
                                                    vel.x.abs() > 0.001 || vel.y.abs() > 0.001 || vel.z.abs() > 0.001
                                                })
                                                .count();
                                            
                                            act.nodes_in_motion = moving_nodes;
                                            
                                            // Update the dynamic rate based on current motion
                                            act.update_dynamic_rate();
                                            
                                            // Get the current update interval for the next update
                                            let update_interval = act.get_current_update_interval();
                                            
                                            if detailed_debug && should_log {
                                                debug!("[WebSocket] Motion: {}/{} nodes, Rate: {} updates/sec, Interval: {:?}",
                                                    moving_nodes, filtered_nodes.len(), act.current_update_rate, update_interval);
                                            }
                                            
                                            if detailed_debug && should_log && !binary_data.is_empty() {
                                                trace!("[WebSocket] Encoded binary data: {} bytes for {} nodes", binary_data.len(), filtered_nodes.len());
                                                
                                                // Log details about a sample node to track position changes
                                                if !filtered_nodes.is_empty() {
                                                    let node = &filtered_nodes[0];
                                                    debug!(
                                                        "Sample node: id={}, pos=[{:.2},{:.2},{:.2}], vel=[{:.2},{:.2},{:.2}]",
                                                        node.0, 
                                                        node.1.position.x, node.1.position.y, node.1.position.z,
                                                        node.1.velocity.x, node.1.velocity.y, node.1.velocity.z
                                                    );
                                                }
                                            }

                                            // Only send data if we have nodes to update
                                            if !filtered_nodes.is_empty() {
                                                // Send binary data directly (permessage-deflate handles compression)
                                                
                                                // Update performance metrics
                                                act.last_transfer_size = binary_data.len();
                                                act.total_bytes_sent += binary_data.len();
                                                act.update_count += 1;
                                                act.nodes_sent_count += filtered_nodes.len();
                                                let now = Instant::now();
                                                let elapsed = now.duration_since(act.last_transfer_time);
                                                act.last_transfer_time = now;
                                                
                                                // Schedule the next update using the dynamic rate
                                                let next_interval = act.get_current_update_interval();
                                                
                                                // Use a simple recursive approach to restart the cycle
                                                let _app_state = act.app_state.clone();
                                                let _settings_addr = act.app_state.settings_addr.clone();
                                                                ctx.run_later(next_interval, move |act, ctx| {
                                                                    // Recursively call the handler to restart the cycle
                                                                    <SocketFlowServer as StreamHandler<Result<ws::Message, ws::ProtocolError>>>::handle(act, Ok(ws::Message::Text("{\"type\":\"requestPositionUpdates\"}".to_string().into())), ctx);
                                                                });
                                                
                                                // Log performance metrics periodically
                                                if detailed_debug && should_log {
                                                    let avg_bytes_per_update = if act.update_count > 0 {
                                                        act.total_bytes_sent / act.update_count
                                                    } else { 0 };
                                                    
                                                    debug!("[WebSocket] Transfer: {} bytes, {} nodes, {:?} since last, avg {} bytes/update",
                                                        binary_data.len(), filtered_nodes.len(), elapsed, avg_bytes_per_update);
                                                }
                                                
                                                ctx.binary(binary_data);
                                            } else if detailed_debug && should_log {
                                                // Log keepalive
                                                debug!("[WebSocket] Sending keepalive (no position changes)");
                                            }
                                        }
                                    }));
                                });

                                let response = serde_json::json!({
                                    "type": "updatesStarted",
                                    "timestamp": chrono::Utc::now().timestamp_millis()
                                });
                                if let Ok(msg_str) = serde_json::to_string(&response) {
                                    self.last_activity = std::time::Instant::now();
                                    ctx.text(msg_str);
                                }
                            }
                            Some("enableRandomization") => {
                                if let Ok(enable_msg) = serde_json::from_value::<serde_json::Value>(msg.clone()) {
                                    let enabled = enable_msg.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false);
                                    info!("Client requested to {} node position randomization (server-side randomization removed)", 
                                         if enabled { "enable" } else { "disable" });
                                    
                                    // Server-side randomization has been removed, but we still acknowledge the client's request
                                    // to maintain backward compatibility with existing clients
                                    actix::spawn(async move {
                                        // Log that we received the request but server-side randomization is no longer supported
                                        info!("Node position randomization request acknowledged, but server-side randomization is no longer supported");
                                        info!("Client-side randomization is now used instead");
                                    });
                                }
                            }
                            Some("requestSwarmPositions") => {
                                info!("Client requested swarm position updates");
                                
                                // Send swarm positions
                                let app_state = self.app_state.clone();
                                
                                ctx.spawn(actix::fut::wrap_future::<_, Self>(async move {
                                    // Get swarm positions from the swarm handler
                                    let swarm_nodes = crate::handlers::swarm_handler::get_swarm_positions(&app_state.swarm_client).await;
                                    
                                    if swarm_nodes.is_empty() {
                                        return vec![];
                                    }
                                    
                                    // Convert to binary format
                                    let mut nodes_data = Vec::new();
                                    for node in swarm_nodes {
                                        let node_data = BinaryNodeData {
                                            position: node.data.position.clone(),
                                            velocity: node.data.velocity.clone(),
                                            mass: node.data.mass,
                                            flags: node.data.flags | 0x80, // Set high bit to indicate swarm node
                                            padding: node.data.padding,
                                        };
                                        nodes_data.push((node.id, node_data));
                                    }
                                    
                                    nodes_data
                                }).map(|nodes_data, _act, ctx| {
                                    if !nodes_data.is_empty() {
                                        // Encode and send swarm positions
                                        let binary_data = binary_protocol::encode_node_data(&nodes_data);
                                        
                                        info!("Sending swarm positions: {} nodes, {} bytes", 
                                            nodes_data.len(), binary_data.len());
                                        
                                        ctx.binary(binary_data);
                                    }
                                }));
                                
                                // Send confirmation
                                let response = serde_json::json!({
                                    "type": "swarmUpdatesStarted",
                                    "timestamp": chrono::Utc::now().timestamp_millis()
                                });
                                if let Ok(msg_str) = serde_json::to_string(&response) {
                                    ctx.text(msg_str);
                                }
                            }
                            _ => {
                                warn!("[WebSocket] Unknown message type: {:?}", msg);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("[WebSocket] Failed to parse text message: {}", e);
                        let error_msg = serde_json::json!({
                            "type": "error",
                            "message": format!("Failed to parse text message: {}", e)
                        });
                        if let Ok(msg_str) = serde_json::to_string(&error_msg) {
                            ctx.text(msg_str);
                        }
                    }
                }
            }
            Ok(ws::Message::Binary(data)) => {
                // Enhanced logging for binary message reception
                info!("Received binary message, length: {}", data.len());
                self.last_activity = std::time::Instant::now();
                
                // Enhanced logging for binary messages (28 bytes per node now with u32 IDs)
                if data.len() % 28 != 0 {
                    warn!(
                        "Binary message size mismatch: {} bytes (not a multiple of 28, remainder: {})",
                        data.len(),
                        data.len() % 28
                    );
                }
                
                match binary_protocol::decode_node_data(&data) {
                    Ok(nodes) => {
                        info!("Decoded {} nodes from binary message", nodes.len());
                        let _nodes_vec: Vec<_> = nodes.clone().into_iter().collect();

                        // CRITICAL FIX: Remove node count limitation to allow processing batches from randomization
                        // Previous code only allowed 2 nodes maximum, which blocked randomization batches
                        {
                            let app_state = self.app_state.clone();
                            let nodes_vec: Vec<_> = nodes.clone().into_iter().collect();

                            let fut = async move {
                                for (node_id, node_data) in &nodes_vec {
                                    // Debug logging for node ID tracking
                                    if *node_id < 5 {
                                        debug!(
                                            "Processing binary update for node ID: {} with position [{:.3}, {:.3}, {:.3}]",
                                            node_id, node_data.position.x, node_data.position.y, node_data.position.z
                                        );
                                    }
                                }

                                // Update node positions using actor messages
                                for (node_id, node_data) in nodes_vec {
                                    debug!("Updated position for node ID {} to [{:.3}, {:.3}, {:.3}]",
                                         node_id, node_data.position.x, node_data.position.y, node_data.position.z);
                                    
                                    // Send update message to GraphServiceActor (now uses u32 directly)
                                    use crate::actors::messages::UpdateNodePosition;
                                    if let Err(e) = app_state.graph_service_addr.send(UpdateNodePosition {
                                        node_id: node_id,
                                        position: node_data.position.into(),
                                        velocity: node_data.velocity.into(),
                                    }).await {
                                        error!("Failed to update node position in GraphServiceActor: {}", e);
                                    }
                                }
                                
                                info!("Updated node positions from binary data (preserving server-side properties)");

                                // Trigger layout recalculation
                                info!("Preparing to recalculate layout after client-side node position update");
                                
                                // Get physics settings from SettingsActor and trigger simulation
                                use crate::actors::messages::GetSettingByPath;
                                let settings_addr = app_state.settings_addr.clone();
                                
                                // Get physics settings
                                if let Ok(Ok(_iterations_val)) = settings_addr.send(GetSettingByPath { path: "visualisation.physics.iterations".to_string() }).await {
                                    if let Ok(Ok(_spring_val)) = settings_addr.send(GetSettingByPath { path: "visualisation.physics.spring_strength".to_string() }).await {
                                        if let Ok(Ok(_repulsion_val)) = settings_addr.send(GetSettingByPath { path: "visualisation.physics.repulsion_strength".to_string() }).await {
                                            // Send simulation step message to GraphServiceActor
                                            use crate::actors::messages::SimulationStep;
                                            if let Err(e) = app_state.graph_service_addr.send(SimulationStep).await {
                                                error!("Failed to trigger simulation step: {}", e);
                                            } else {
                                                info!("Successfully triggered layout recalculation");
                                            }
                                        }
                                    }
                                }
                            };

                            let fut = fut.into_actor(self);
                            ctx.spawn(fut.map(|_, _, _| ()));
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode binary message: {}", e);
                        let error_msg = serde_json::json!({
                            "type": "error",
                            "message": format!("Failed to decode binary message: {}", e)
                        });
                        if let Ok(msg_str) = serde_json::to_string(&error_msg) {
                            ctx.text(msg_str);
                        }
                    }
                }
            }
            Ok(ws::Message::Close(reason)) => {
                info!("[WebSocket] Client initiated close: {:?}", reason);
                ctx.close(reason); // Use client's reason for closing
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {
                warn!("[WebSocket] Received unexpected continuation frame");
            }
            Ok(ws::Message::Nop) => {
                debug!("[WebSocket] Received Nop");
            }
            Err(e) => {
                error!("[WebSocket] Error in WebSocket connection: {}", e);
                // Close with protocol error status code before stopping
                ctx.close(Some(ws::CloseReason::from(ws::CloseCode::Protocol)));
            }
        }
    }
}

pub async fn socket_flow_handler(
    req: HttpRequest,
    stream: web::Payload,
    app_state_data: web::Data<AppState>, // Renamed for clarity
    pre_read_ws_settings: web::Data<PreReadSocketSettings>, // New data
) -> Result<HttpResponse, Error> {
    let app_state_arc = app_state_data.into_inner(); // Get the Arc<AppState>
    
    // Get ClientManagerActor address from AppState
    let client_manager_addr = app_state_arc.client_manager_addr.clone();
    
    // Get debug settings from SettingsActor
    use crate::actors::messages::GetSettingByPath;
    let settings_addr = app_state_arc.settings_addr.clone();
    
    let debug_enabled = match settings_addr.send(GetSettingByPath { path: "system.debug.enabled".to_string() }).await {
        Ok(Ok(value)) => value.as_bool().unwrap_or(false),
        _ => false,
    };
    let debug_websocket = match settings_addr.send(GetSettingByPath { path: "system.debug.enable_websocket_debug".to_string() }).await {
        Ok(Ok(value)) => value.as_bool().unwrap_or(false),
        _ => false,
    };
    let should_debug = debug_enabled && debug_websocket;

    if should_debug {
        debug!("WebSocket connection attempt from {:?}", req.peer_addr());
    }

    // Check for WebSocket upgrade
    if !req.headers().contains_key("Upgrade") {
        return Ok(HttpResponse::BadRequest().body("WebSocket upgrade required"));
    }
    
    // Pass the ClientManagerActor address to SocketFlowServer::new
    let ws = SocketFlowServer::new(app_state_arc, pre_read_ws_settings.get_ref().clone(), client_manager_addr);

    // Start WebSocket with compression enabled (permessage-deflate)
    // Prefer WsResponseBuilder for setting protocols
    match ws::WsResponseBuilder::new(ws, &req, stream)
        .protocols(&["permessage-deflate"])
        .start()
    {
        Ok(response) => {
            info!("[WebSocket] Client connected successfully with compression support");
            Ok(response)
        }
        Err(e) => {
            error!("[WebSocket] Failed to start WebSocket: {}", e);
            Err(e)
        }
    }
}
