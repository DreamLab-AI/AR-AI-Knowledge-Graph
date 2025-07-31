use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use std::collections::{HashMap, HashSet};
use rand::distributions::{Alphanumeric, DistString};
use rand::Rng;
use std::io::{Error, ErrorKind};
use serde_json;
use std::pin::Pin;
use std::time::{Duration, Instant};
use futures::Future;
use log::{info, warn, error, trace};
use scopeguard;

use tokio::fs::File as TokioFile;
use crate::models::graph::GraphData;
use crate::models::node::Node; // Corrected Node import
use crate::models::edge::Edge;
use crate::models::metadata::MetadataStore;
use crate::config::AppFullSettings; // Use AppFullSettings, ClientFacingSettings removed
use crate::utils::gpu_compute::GPUCompute;
use crate::models::simulation_params::{SimulationParams, SimulationPhase, SimulationMode};
use crate::models::pagination::PaginatedGraphData;
// Removed: use crate::handlers::socket_flow_handler::ClientManager;
// ClientManagerActor is used instead
use crate::actors::client_manager_actor::ClientManagerActor;
use actix::Addr; // Added Addr import
use crate::actors::messages::BroadcastNodePositions;
use crate::utils::binary_protocol;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;

// Static flag to prevent multiple simultaneous graph rebuilds
static GRAPH_REBUILD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

// Static flag to track if a simulation loop is already running and current simulation ID
static SIMULATION_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);

// A mutex to synchronize simulation loop creation and shutdown
// This is necessary to avoid race conditions when a new GraphService is created
// while an old one is being shut down
static SIMULATION_MUTEX: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

// Cache configuration
const NODE_POSITION_CACHE_TTL_MS: u64 = 50; // 50ms cache time
const METADATA_FILE_WAIT_TIMEOUT_MS: u64 = 5000; // 5 second wait timeout
const SHUTDOWN_TIMEOUT_MS: u64 = 5000; // 5 second shutdown timeout

// Physics stabilization constants
// const STABLE_THRESHOLD_ITERATIONS: usize = 100; // Number of iterations with minimal movement // Dead Code
// const POSITION_STABILITY_THRESHOLD: f32 = 0.001; // 1mm threshold for stability // Dead Code

// Rate limiting and conflict resolution constants
const UPDATE_RATE_LIMIT_MS: u64 = 16; // ~60fps max update rate
// const POSITION_CONFLICT_THRESHOLD: f32 = 0.001; // 1mm threshold for position conflicts // Dead Code
// const MAX_CONCURRENT_UPDATES: usize = 100; // Maximum number of node updates per batch // Dead Code
const METADATA_FILE_CHECK_INTERVAL_MS: u64 = 100; // Check every 100ms
// Constants for GPU retry mechanism
const MAX_GPU_CALCULATION_RETRIES: u32 = 3;
const GPU_RETRY_DELAY_MS: u64 = 500; // 500ms delay between retries

#[derive(Clone)]
pub struct GraphService {
    graph_data: Arc<RwLock<GraphData>>,
    shutdown_complete: Arc<AtomicBool>,
    node_map: Arc<RwLock<HashMap<u32, Node>>>,
    gpu_compute: Option<Arc<RwLock<GPUCompute>>>,
    node_positions_cache: Arc<RwLock<Option<(Vec<Node>, Instant)>>>,
    last_update: Arc<RwLock<Instant>>,
    _pending_updates: Arc<RwLock<HashMap<u32, (Node, Instant)>>>, // Dead Code
    cache_enabled: bool,
    simulation_id: String,
    // client_manager: Option<Addr<ClientManagerActor>>, // ClientManagerActor address
    _is_initialized: Arc<AtomicBool>, // Dead Code
    shutdown_requested: Arc<AtomicBool>,
}

impl GraphService {
    pub async fn new(
        settings: Arc<RwLock<AppFullSettings>>, // Changed to AppFullSettings
        gpu_compute: Option<Arc<RwLock<GPUCompute>>>,
        client_manager_for_loop: Addr<ClientManagerActor> // Changed to Addr<ClientManagerActor>
    ) -> Self {
        // Get physics settings
        let physics_settings = settings.read().await.visualisation.physics.clone();

        // Generate a unique ID for this GraphService instance
        let simulation_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 8);
        info!("[GraphService::new] Creating new GraphService instance with ID: {}", simulation_id);
        
        // Acquire the mutex to ensure exclusive access during initialization
        let mut guard = SIMULATION_MUTEX.lock().await;
        
        // Check if there's already an instance running
        let is_running = SIMULATION_LOOP_RUNNING.load(Ordering::SeqCst);
        if is_running {
            error!("[GraphService::new] 🚨 CRITICAL: A simulation loop is already running with ID: {}! Creating a new GraphService without shutting down the previous one may cause dual simulation loops.", *guard);
            warn!("[GraphService::new] Current simulation ID: {} will replace previous ID: {}", simulation_id, *guard);
        }
        
        // Create the shared node map
        let node_map = Arc::new(RwLock::new(HashMap::new()));

        if gpu_compute.is_some() {
            info!("[GraphService] GPU compute is enabled - physics simulation will run");
            info!("[GraphService] Testing GPU compute functionality at startup");
            tokio::spawn(Self::test_gpu_at_startup(gpu_compute.clone()));
        } else {
            error!("[GraphService] GPU compute is NOT enabled - physics simulation will use CPU fallback");
        }

        // Create shutdown signal
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        // Create the GraphService with caching enabled 
        let _cache = Arc::new(RwLock::new(Option::<(Vec<Node>, Instant)>::None));
        let graph_service = Self {
            graph_data: Arc::new(RwLock::new(GraphData::default())),
            shutdown_complete: Arc::new(AtomicBool::new(false)),
            node_map: node_map.clone(),
            gpu_compute,
            last_update: Arc::new(RwLock::new(Instant::now())),
            _pending_updates: Arc::new(RwLock::new(HashMap::new())), // Dead Code
            node_positions_cache: Arc::new(RwLock::new(None)),
            cache_enabled: true,
            // client_manager, // Removed
            _is_initialized: Arc::new(AtomicBool::new(false)), // Dead Code
            simulation_id: simulation_id.clone(),
            shutdown_requested: shutdown_requested.clone(),
        };
        
        // Prepare for simulation loop
        let graph_data = Arc::clone(&graph_service.graph_data);
        let node_positions_cache = Arc::clone(&graph_service.node_positions_cache);
        let gpu_compute = graph_service.gpu_compute.clone();
        let loop_simulation_id = simulation_id.clone();
        
        // Log more detailed information about the GPU compute status
        if gpu_compute.is_some() {
            info!("[GraphService] 🔹 GPU compute is enabled and will be used for physics simulation (ID: {})", simulation_id);
            // Try to gather device information
            if let Some(gpu) = &gpu_compute {
                if let Ok(gpu_lock) = gpu.try_read() {
                    info!("[GraphService] GPU device information: iterations={} (ID: {})", gpu_lock.iteration_count, simulation_id);
                }
            }
        } else {
            warn!("[GraphService] 🔸 GPU compute is NOT available - will use CPU fallback for physics (ID: {})", simulation_id);
        }
        
        // Update the current simulation ID in the shared mutex
        *guard = simulation_id.clone();
        
        // Check if a simulation loop is already running and attempt to replace it
        if SIMULATION_LOOP_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            warn!("[GraphService] Simulation loop already running, attempting to replace it (new ID: {})", simulation_id);
            // We're replacing an existing simulation, wait for the flag to be reset
            // by forcing a reset ourselves since we have the mutex
            SIMULATION_LOOP_RUNNING.store(false, Ordering::SeqCst);
            // Then set it again for our new loop
            SIMULATION_LOOP_RUNNING.store(true, Ordering::SeqCst);
        }
        
        // Release the mutex before spawning the task
        drop(guard);
        
        info!("[GraphService] Starting physics simulation loop (ID: {})", loop_simulation_id);
        
        // Clone graph_service twice - one for the async block and one for return
        let _graph_service_clone = graph_service.clone(); // Prefixed with underscore as it's not directly used after cloning for the loop
        let return_service = graph_service.clone();
        let captured_client_manager = client_manager_for_loop.clone(); // Capture ClientManager for the loop
        tokio::spawn(async move {
            let params = SimulationParams {
                iterations: physics_settings.iterations,
                spring_strength: physics_settings.spring_strength,
                repulsion: physics_settings.repulsion_strength,
                damping: physics_settings.damping,
                max_repulsion_distance: physics_settings.repulsion_distance,
                viewport_bounds: physics_settings.bounds_size,
                mass_scale: physics_settings.mass_scale,
                boundary_damping: physics_settings.boundary_damping,
                enable_bounds: physics_settings.enable_bounds,
                time_step: 0.016,  // ~60fps
                phase: SimulationPhase::Dynamic,
                mode: SimulationMode::Remote,
            };
            
            // Create a guard to reset the flag when the task exits
            let loop_guard = scopeguard::guard((), |_| { 
                info!("[Graph] Physics simulation loop exiting, resetting SIMULATION_LOOP_RUNNING flag (ID: {})", loop_simulation_id);
                // Use compare_exchange to safely reset the flag
                if SIMULATION_LOOP_RUNNING.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                    graph_service.shutdown_complete.store(true, Ordering::SeqCst);
                } else {
                    error!("[Graph] Failed to reset SIMULATION_LOOP_RUNNING flag - was already false (ID: {})", 
                           loop_simulation_id);
                }
            });
            
            loop {
                // Check if shutdown was requested
                if shutdown_requested.load(Ordering::SeqCst) {
                    info!("[Graph] Shutdown requested for simulation loop (ID: {})", loop_simulation_id);
                    break;
                }
                
                // Update positions - using loop ID in logs to track which loop is running
                trace!("[Graph:{}] Starting physics calculation iteration", loop_simulation_id);
                let mut graph = graph_data.write().await;
                let mut node_map = node_map.write().await;

                let gpu_status = if gpu_compute.is_some() { "available" } else { "NOT available" };
                trace!("[Graph:{}] GPU compute status: {}, physics enabled: {}",
                       loop_simulation_id, gpu_status, physics_settings.enabled);
                       
                if physics_settings.enabled {
                    if let Some(gpu) = &gpu_compute {
                        if let Err(e) = Self::calculate_layout_with_retry(gpu, &mut graph, &mut node_map, &params).await {
                            error!("[Graph:{}] Error updating positions: {}", loop_simulation_id, e);
                        } else {
                            trace!("[Graph:{}] GPU calculation completed successfully", loop_simulation_id);
                            trace!("[Graph:{}] Successfully calculated layout for {} nodes", loop_simulation_id, graph.nodes.len());
                            
                            // Broadcast position updates to all clients
                            Self::broadcast_positions(captured_client_manager.clone(), &graph.nodes).await;
                        }
                    } else {
                        // Use CPU fallback when GPU is not available
                        trace!("[Graph:{}] GPU compute not available - using CPU fallback for physics calculation", loop_simulation_id);
                        if let Err(e) = Self::calculate_layout_cpu(&mut graph, &mut node_map, &params) {
                            error!("[Graph:{}] Error updating positions with CPU fallback: {}", loop_simulation_id, e);
                        } else {
                            trace!("[Graph:{}] CPU calculation completed successfully", loop_simulation_id);
                            trace!("[Graph:{}] Successfully calculated layout with CPU fallback for {} nodes", loop_simulation_id, graph.nodes.len());
                            
                            // Broadcast position updates to all clients
                            Self::broadcast_positions(captured_client_manager.clone(), &graph.nodes).await;
                        }
                    }
                } else {
                    trace!("[Graph:{}] Physics disabled in settings - skipping physics calculation", loop_simulation_id);
                }
                drop(graph); // Release locks before sleep
                drop(node_map);
                tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
                let mut cache = node_positions_cache.write().await;
                *cache = None;
            }
            drop(loop_guard); // Explicitly drop the guard to trigger the cleanup
        }); 

        return_service
    }
    
    // Helper method to check for update rate limiting
    async fn should_rate_limit(&self) -> bool {
        let now = Instant::now();
        let last = *self.last_update.read().await;
        if now.duration_since(last).as_millis() < UPDATE_RATE_LIMIT_MS as u128 {
            return true;
        }
        *self.last_update.write().await = now;
        false
    }

    // Dead Code: Associated item `resolve_position_conflict` is never used
    // // Helper method to resolve position conflicts
    // fn resolve_position_conflict(current: &Node, update: &Node) -> Node {
    //     let mut resolved = current.clone();
        
    //     // Calculate position differences
    //     let dx = update.data.position.x - current.data.position.x;
    //     let dy = update.data.position.y - current.data.position.y;
    //     let dz = update.data.position.z - current.data.position.z;
        
    //     // If difference is significant, use update position
    //     if dx*dx + dy*dy + dz*dz > POSITION_CONFLICT_THRESHOLD*POSITION_CONFLICT_THRESHOLD { // POSITION_CONFLICT_THRESHOLD is commented out
    //         resolved.data.position = update.data.position.clone();
            
    //         // Average the velocities to smooth transitions
    //         resolved.data.velocity.x = (current.data.velocity.x + update.data.velocity.x) * 0.5;
    //         resolved.data.velocity.y = (current.data.velocity.y + update.data.velocity.y) * 0.5;
    //         resolved.data.velocity.z = (current.data.velocity.z + update.data.velocity.z) * 0.5;
    //     }
        
    //     // Preserve mass and flags from current node
    //     resolved.data.mass = current.data.mass;
    //     resolved.data.flags = current.data.flags;
        
    //     resolved
    // }

    // Dead Code: Associated item `cleanup_pending_updates` is never used
    // // Helper method to clean up old pending updates
    // async fn cleanup_pending_updates(&self) {
    //     let mut pending = self._pending_updates.write().await; // Adjusted to use _pending_updates
    //     let now = Instant::now();
    //     pending.retain(|_, (_, timestamp)| {
    //         now.duration_since(*timestamp).as_millis() < UPDATE_RATE_LIMIT_MS as u128
    //     });
    // }
 
    // Helper method to broadcast position updates to all clients
    async fn broadcast_positions(client_manager_addr: Addr<ClientManagerActor>, nodes: &[Node]) {
        // Encode node data for broadcasting
        // The binary_protocol::encode_node_data expects a slice of (u32, BinaryNodeData)
        // We need to convert our Vec<Node> to this format.
        let positions_to_encode: Vec<(u32, crate::utils::socket_flow_messages::BinaryNodeData)> = nodes.iter().map(|node| (node.id, node.data)).collect();

        let binary_data = binary_protocol::encode_node_data(&positions_to_encode);
        // Send BroadcastNodePositions message to ClientManagerActor
        client_manager_addr.do_send(BroadcastNodePositions { positions: binary_data });
    }

    /// Shutdown the simulation loop to allow creating a new instance
    pub async fn shutdown(&self) {
        info!("[GraphService] Shutting down simulation loop (ID: {})", self.simulation_id);
        
        // Acquire the mutex to ensure we don't have race conditions during shutdown
        let guard = SIMULATION_MUTEX.lock().await;
        
        // Check if this is the currently running simulation
        if *guard != self.simulation_id {
            warn!("[GraphService] Cannot shutdown simulation - current running loop has different ID: {} (this instance ID: {})", 
                  *guard, self.simulation_id);
            return;
        }
        
        // Signal the loop to stop by setting the shutdown flag
        self.shutdown_requested.store(true, Ordering::SeqCst);
        info!("[GraphService] Set shutdown flag for simulation loop (ID: {})", self.simulation_id);
        
        // Reset shutdown complete flag before waiting
        self.shutdown_complete.store(false, Ordering::SeqCst);
        
        // Wait for the loop to fully exit with a 5 second timeout
        let max_attempts = SHUTDOWN_TIMEOUT_MS / 50; // 5 seconds total at 50ms intervals
        for attempt in 0..max_attempts {
            if !SIMULATION_LOOP_RUNNING.load(Ordering::SeqCst) {
                // Double check that shutdown is complete
                if self.shutdown_complete.load(Ordering::SeqCst) {
                    info!("[GraphService] Simulation loop successfully stopped (ID: {})", self.simulation_id);
                    return;
                }
            }
            
            // Log progress every second
            if attempt % 20 == 0 {
                info!("[GraphService] Waiting for simulation loop to stop (attempt {}/{})", attempt, max_attempts);
            }
            
            tokio::time::sleep(Duration::from_millis(50)).await;
            if attempt == max_attempts - 1 {
                error!("[GraphService] Shutdown timeout after {}ms for simulation (ID: {})", 
                    SHUTDOWN_TIMEOUT_MS, self.simulation_id);
            }
        }
    }
    
    /// Get diagnostic information about the simulation status
    pub async fn get_simulation_diagnostics(&self) -> String {
        // Get the current simulation ID from the mutex
        let current_id = match SIMULATION_MUTEX.try_lock() {
            Ok(guard) => {
                let id = guard.clone();
                // Drop the guard immediately to avoid holding it
                drop(guard);
                id
            },
            Err(_) => "Unable to acquire mutex".to_string(),
        };
        
        // Check if this is the active simulation
        let is_active = current_id == self.simulation_id;
        
        // Check the global running flag
        let is_running = SIMULATION_LOOP_RUNNING.load(Ordering::SeqCst);
        
        // Check if shutdown has been requested for this instance
        let shutdown_requested = self.shutdown_requested.load(Ordering::SeqCst);
        
        format!(
            "Simulation Diagnostics:\n- This instance ID: {}\n- Current active ID: {}\n- Is this instance active: {}\n- Global running flag: {}\n- Shutdown requested: {}\n- Has GPU compute: {}",
            self.simulation_id,
            current_id,
            is_active,
            is_running,
            shutdown_requested,
            self.gpu_compute.is_some()
        )
    }
    
    /// Test GPU compute at startup to verify it's working
    async fn test_gpu_at_startup(gpu_compute: Option<Arc<RwLock<GPUCompute>>>) {
        // Add a small delay to let other initialization complete
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        info!("[GraphService] Running GPU startup test");
        
        if let Some(gpu) = &gpu_compute {
            match gpu.read().await.test_compute() {
                Ok(_) => {
                    info!("[GraphService] ✅ GPU test computation succeeded - GPU physics is working");
                },
                Err(e) => {
                    error!("[GraphService] ❌ GPU test computation failed: {}", e);
                    error!("[GraphService] The system will fall back to CPU physics which may be slower");
                    
                    // Try initializing a new GPU instance
                    info!("[GraphService] Attempting to reinitialize GPU...");
                    let _new_gpu = GPUCompute::new(&GraphData::default()).await; // Using _ to avoid unused warning
                }
            }
        } else {
            error!("[GraphService] ❌ No GPU compute instance available for testing");
        }
    }
    
    /// Wait for metadata file to be available (mounted by Docker)
    pub async fn wait_for_metadata_file() -> bool {
        info!("Checking for metadata file from Docker volume mount...");
        
        // Path to metadata file
        let metadata_path = std::path::Path::new("/app/data/metadata/metadata.json");
        
        // Start timer
        let start_time = Instant::now();
        let timeout = Duration::from_millis(METADATA_FILE_WAIT_TIMEOUT_MS);
        
        // Loop until timeout
        while start_time.elapsed() < timeout {
            // Check if file exists and is not empty
            match tokio::fs::metadata(&metadata_path).await {
                Ok(metadata) => {
                    if metadata.is_file() && metadata.len() > 0 {
                        // Try to open and validate the file
                        match TokioFile::open(&metadata_path).await {
                            Ok(_) => {
                                let elapsed = start_time.elapsed();
                                info!("Metadata file found and accessible after {:?}", elapsed);
                                return true;
                            }
                            Err(e) => {
                                trace!("Metadata file exists but couldn't be opened: {}", e);
                                // Continue waiting - might still be being written to
                            }
                        }
                    } else {
                        trace!("Metadata file exists but is empty or not a regular file");
                    }
                }
                Err(e) => {
                    trace!("Waiting for metadata file to be mounted: {}", e);
                }
            }
            
            // Sleep before checking again
            tokio::time::sleep(Duration::from_millis(METADATA_FILE_CHECK_INTERVAL_MS)).await;
        }
        
        // Timeout reached
        warn!("Timed out waiting for metadata file after {:?}", timeout);
        
        // Timeout reached, file not found or accessible
        false
    }

    pub async fn build_graph_from_metadata(metadata: &MetadataStore) -> Result<GraphData, Box<dyn std::error::Error + Send + Sync>> {
        // Check if a rebuild is already in progress
        info!("Building graph from {} metadata entries", metadata.len());
        trace!("Building graph from {} metadata entries", metadata.len());
        
        if GRAPH_REBUILD_IN_PROGRESS.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            warn!("Graph rebuild already in progress, skipping duplicate rebuild");
            return Err("Graph rebuild already in progress".into());
        }      
        
        // Create a guard struct to ensure the flag is reset when this function returns
        struct RebuildGuard;
        impl Drop for RebuildGuard {
            fn drop(&mut self) {
                GRAPH_REBUILD_IN_PROGRESS.store(false, Ordering::SeqCst);
            }
        }
        // This guard will reset the flag when it goes out of scope
        let _guard = RebuildGuard;
        
        let mut graph = GraphData::new();
        let mut edge_map = HashMap::new();
        let mut node_map = HashMap::new();

        // First pass: Create nodes from files in metadata
        let mut valid_nodes = HashSet::new();
        trace!("Creating nodes from {} metadata entries", metadata.len());
        for file_name in metadata.keys() {
            let node_id = file_name.trim_end_matches(".md").to_string();
            valid_nodes.insert(node_id);
        }
        trace!("Created valid_nodes set with {} nodes", valid_nodes.len());

        // Create nodes for all valid node IDs
        for node_id in &valid_nodes {
            // Get metadata for this node, including the node_id if available
            let metadata_entry = graph.metadata.get(&format!("{}.md", node_id));
            let stored_node_id = metadata_entry.map(|m| m.node_id.clone());
            
            // Create node with stored ID or generate a new one if not available
            let stored_node_id_u32 = stored_node_id.and_then(|s| s.parse::<u32>().ok());
            let mut node = Node::new_with_id(node_id.clone(), stored_node_id_u32);
            graph.id_to_metadata.insert(node.id.to_string(), node_id.clone());

            // Get metadata for this node
            if let Some(metadata) = metadata.get(&format!("{}.md", node_id)) {
                // Set file size which also calculates mass
                node.set_file_size(metadata.file_size as u64);  // This will update both file_size and mass
                
                // Set the node label to the file name without extension
                // This will be used as the display name for the node
                node.label = metadata.file_name.trim_end_matches(".md").to_string();
                
                // Set visual properties from metadata
                node.size = Some(metadata.node_size as f32);
                
                // Add metadata fields to node's metadata map
                // Add all relevant metadata fields to ensure consistency
                node.metadata.insert("fileName".to_string(), metadata.file_name.clone());
                
                // Add name field (without .md extension) for client-side metadata ID mapping
                if metadata.file_name.ends_with(".md") {
                    let name = metadata.file_name[..metadata.file_name.len() - 3].to_string();
                    node.metadata.insert("name".to_string(), name.clone());
                    node.metadata.insert("metadataId".to_string(), name);
                } else {
                    node.metadata.insert("name".to_string(), metadata.file_name.clone());
                    node.metadata.insert("metadataId".to_string(), metadata.file_name.clone());
                }
                
                node.metadata.insert("fileSize".to_string(), metadata.file_size.to_string());
                node.metadata.insert("nodeSize".to_string(), metadata.node_size.to_string());
                node.metadata.insert("hyperlinkCount".to_string(), metadata.hyperlink_count.to_string());
                node.metadata.insert("sha1".to_string(), metadata.sha1.clone());
                // Use last_content_change if available, otherwise fall back to last_modified
                let last_modified_to_use = metadata.last_content_change
                    .unwrap_or(metadata.last_modified);
                node.metadata.insert("lastModified".to_string(), last_modified_to_use.to_string());
                
                // Also include both dates for debugging
                if let Some(content_change) = metadata.last_content_change {
                    node.metadata.insert("lastContentChange".to_string(), content_change.to_string());
                }
                if let Some(last_commit) = metadata.last_commit {
                    node.metadata.insert("lastCommit".to_string(), last_commit.to_string());
                }
                
                if !metadata.perplexity_link.is_empty() {
                    node.metadata.insert("perplexityLink".to_string(), metadata.perplexity_link.clone());
                }
                
                if let Some(last_process) = metadata.last_perplexity_process {
                    node.metadata.insert("lastPerplexityProcess".to_string(), last_process.to_string());
                }
                
                // We don't add topic_counts to metadata as it would create circular references
                // and is already used to create edges
                
                // Ensure flags is set to 1 (default active state)
                node.data.flags = 1;
            }

            let node_clone = node.clone();
            graph.nodes.push(node_clone);
            // Store nodes in map by numeric ID for efficient lookups
            node_map.insert(node.id, node);
        }

        // Store metadata in graph
        trace!("Storing {} metadata entries in graph", metadata.len());
        graph.metadata = metadata.clone();
        trace!("Created {} nodes in graph", graph.nodes.len());
        // Second pass: Create edges from topic counts
        for (source_file, metadata) in metadata.iter() {
            let source_id = source_file.trim_end_matches(".md").to_string();
            // Find the node with this metadata_id to get its numeric ID
            let source_node = graph.nodes.iter().find(|n| n.metadata_id == source_id);
            if source_node.is_none() {
                continue; // Skip if node not found
            }
            let source_numeric_id = source_node.unwrap().id;
            
            trace!("Processing edges for source: {} (ID: {})", source_id, source_numeric_id);
            for (target_file, count) in &metadata.topic_counts {
                let target_id = target_file.trim_end_matches(".md").to_string();
                // Find the node with this metadata_id to get its numeric ID
                let target_node = graph.nodes.iter().find(|n| n.metadata_id == target_id);
                if target_node.is_none() {
                    continue; // Skip if node not found
                }
                let target_numeric_id = target_node.unwrap().id;

                trace!("  Edge: {} -> {} (weight: {})", source_numeric_id, target_numeric_id, count);

                // Only create edge if both nodes exist and they're different
                if source_numeric_id != target_numeric_id {
                    let edge_key = if source_numeric_id < target_numeric_id {
                        (source_numeric_id, target_numeric_id)
                    } else {
                        (target_numeric_id, source_numeric_id)
                    };

                    edge_map.entry(edge_key)
                        .and_modify(|weight| *weight += *count as f32)
                        .or_insert(*count as f32);
                }
            }
        }

        // Convert edge map to edges
        trace!("Edge map contains {} unique connections", edge_map.len());
        for ((source, target), weight) in &edge_map {
            trace!("Edge map entry: {} -- {} (weight: {})", source, target, weight);
        }

        trace!("Converting edge map to {} edges", edge_map.len());
        graph.edges = edge_map.into_iter()
            .map(|((source, target), weight)| {
                Edge::new(source, target, weight)
            })
            .collect();

        // Initialize random positions
        Self::initialize_random_positions(&mut graph);

        info!("Built graph with {} nodes and {} edges", graph.nodes.len(), graph.edges.len());
        trace!("Completed graph build: {} nodes, {} edges", graph.nodes.len(), graph.edges.len());
        Ok(graph)
    }


    fn initialize_random_positions(graph: &mut GraphData) {
        let mut rng = rand::thread_rng();
        let node_count = graph.nodes.len() as f32;
        let initial_radius = 3.0; // Increasing radius for better visibility
        let golden_ratio = (1.0 + 5.0_f32.sqrt()) / 2.0;
        
        // Log the initialization process
        info!("Initializing random positions for {} nodes with radius {}", 
             node_count, initial_radius);
        info!("First 5 node numeric IDs: {}", graph.nodes.iter().take(5).map(|n| n.id.to_string()).collect::<Vec<_>>().join(", "));
        info!("First 5 node metadata IDs: {}", graph.nodes.iter().take(5).map(|n| n.metadata_id.clone()).collect::<Vec<_>>().join(", "));
        
        // Use Fibonacci sphere distribution for more uniform initial positions
        for (i, node) in graph.nodes.iter_mut().enumerate() {
            let i_float: f32 = i as f32;
            
            // Calculate Fibonacci sphere coordinates
            let theta = 2.0 * std::f32::consts::PI * i_float / golden_ratio;
            let phi = (1.0 - 2.0 * (i_float + 0.5) / node_count).acos();
            
            // Add slight randomness to prevent exact overlaps
            let r = initial_radius * (0.9 + rng.gen_range(0.0..0.2));
            
            node.set_x(r * phi.sin() * theta.cos());
            node.set_y(r * phi.sin() * theta.sin());
            node.set_z(r * phi.cos());
            
            // Initialize with zero velocity
            node.set_vx(0.0);
            node.set_vy(0.0);
            node.set_vz(0.0);

            // Log first 5 nodes for debugging
            if i < 5 {
                info!("Initialized node {}: id={}, pos=[{:.3},{:.3},{:.3}]", 
                     i,
                     node.id,
                     node.data.position.x, 
                     node.data.position.y, 
                     node.data.position.z);
            }
        }
    }

    /// Helper function to retry GPU layout calculation with exponential backoff
    pub async fn calculate_layout_with_retry(
        gpu_compute: &Arc<RwLock<GPUCompute>>,
        graph: &mut GraphData,
        node_map: &mut HashMap<u32, Node>,
        params: &SimulationParams,
    ) -> std::io::Result<()> {
        trace!("[calculate_layout_with_retry] Starting GPU calculation with retry mechanism");
        let mut last_error: Option<Error> = None;
        
        for attempt in 0..MAX_GPU_CALCULATION_RETRIES {
            match Self::calculate_layout(gpu_compute, graph, node_map, params).await {
                Ok(()) => {
                    if attempt > 0 {
                        info!("[calculate_layout] Succeeded after {} retries", attempt);
                        trace!("[calculate_layout_with_retry] GPU calculation succeeded after retries");
                    }
                    return Ok(());
                }
                Err(e) => {
                    let delay = GPU_RETRY_DELAY_MS * (1 << attempt); // Exponential backoff
                    warn!("[calculate_layout] Failed (attempt {}/{}): {}. Retrying in {}ms...", 
                          attempt + 1, MAX_GPU_CALCULATION_RETRIES, e, delay);
                    last_error = Some(e);
                    
                    if attempt + 1 < MAX_GPU_CALCULATION_RETRIES {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }
        
        // If we get here, all attempts failed
        trace!("[calculate_layout_with_retry] All GPU attempts failed, falling back to CPU");
        error!("[calculate_layout] Failed after {} attempts, falling back to CPU", MAX_GPU_CALCULATION_RETRIES);
        
        // As a fallback, try CPU calculation when GPU fails repeatedly
        match Self::calculate_layout_cpu(graph, node_map, params) {
            Ok(()) => {
                info!("[calculate_layout] Successfully fell back to CPU calculation");
                Ok(())
            }
            Err(cpu_err) => {
                error!("[calculate_layout] CPU fallback also failed: {}", cpu_err);
                // Return the last GPU error as it's likely more relevant
                Err(last_error.unwrap_or_else(|| Error::new(ErrorKind::Other, 
                    format!("All {} GPU retry attempts failed and CPU fallback failed", MAX_GPU_CALCULATION_RETRIES))))
            }
        }
    }

    pub async fn calculate_layout(
        gpu_compute: &Arc<RwLock<GPUCompute>>,
        graph: &mut GraphData,
        node_map: &mut HashMap<u32, Node>,
        params: &SimulationParams,
    ) -> std::io::Result<()> {
        {
            trace!("[calculate_layout] Starting GPU physics calculation for {} nodes, {} edges with mode {:?}",
                  graph.nodes.len(), graph.edges.len(), params.mode);
            
            // Get current timestamp for performance tracking
            let start_time = std::time::Instant::now();

            let mut gpu_compute = gpu_compute.write().await;

            trace!("[calculate_layout] params: iterations={}, spring_strength={:.3}, repulsion={:.3}, damping={:.3}",
                 params.iterations, params.spring_strength, params.repulsion, params.damping);
            
            // Update data and parameters
            if let Err(e) = gpu_compute.update_graph_data(graph) {
                error!("[calculate_layout] Failed to update graph data in GPU: {}, node count: {}", 
                      e, graph.nodes.len());
                // Log more details about the graph for debugging
                if !graph.nodes.is_empty() {
                    trace!("First node: id={}, position=[{:.3},{:.3},{:.3}]", graph.nodes[0].id, graph.nodes[0].data.position.x, graph.nodes[0].data.position.y, graph.nodes[0].data.position.z);
                }
                return Err(e);
            }
            
            if let Err(e) = gpu_compute.update_simulation_params(params) {
                error!("[calculate_layout] Failed to update simulation parameters in GPU: {}", e);
                return Err(e);
            }
            
            // Perform computation step
            if let Err(e) = gpu_compute.step() {
                error!("[calculate_layout] Failed to execute physics step: {}, graph has {} nodes and {} edges", 
                       e, graph.nodes.len(), graph.edges.len());
                return Err(e);
            }
            
            // Get updated positions
            let updated_nodes = match gpu_compute.get_node_data() {
                Ok(nodes) => {
                    trace!("[calculate_layout] Successfully retrieved {} nodes from GPU", nodes.len());
                    nodes
                },
                Err(e) => {
                    error!("[calculate_layout] Failed to get node data from GPU: {}", e);
                    return Err(e);
                }
            };
            
            // Update graph with new positions
            let mut nodes_updated = 0;
            for (i, node) in graph.nodes.iter_mut().enumerate() {
                if i >= updated_nodes.len() {
                    error!("[calculate_layout] Node index out of range: {} >= {}", i, updated_nodes.len());
                    continue;
                }
                
                // Update position and velocity from GPU data
                node.data = updated_nodes[i];
                nodes_updated += 1;
                
                // Update node_map as well
                if let Some(map_node) = node_map.get_mut(&node.id) {
                    map_node.data = updated_nodes[i];
                } else {
                    warn!("[calculate_layout] Node {} not found in node_map", node.id);
                }
            }
            
            // Log performance info
            let elapsed = start_time.elapsed();
            
                // Log sample positions for debugging (first 2 nodes)
                let sample_positions = if graph.nodes.len() >= 2 {
                    format!("[{:.2},{:.2},{:.2}], [{:.2},{:.2},{:.2}]", 
                        graph.nodes[0].data.position.x, graph.nodes[0].data.position.y, graph.nodes[0].data.position.z,
                        graph.nodes[1].data.position.x, graph.nodes[1].data.position.y, graph.nodes[1].data.position.z)
                } else if graph.nodes.len() == 1 {
                    format!("[{:.2},{:.2},{:.2}]", graph.nodes[0].data.position.x, graph.nodes[0].data.position.y, graph.nodes[0].data.position.z)
                } else { "no nodes".to_string() };
            
                trace!("[calculate_layout] Updated positions for {}/{} nodes in {:?}. Sample positions: {}", nodes_updated, graph.nodes.len(), elapsed, sample_positions);
            
            Ok(())
        }
    }

    /// CPU fallback implementation of force-directed graph layout
    pub fn calculate_layout_cpu(
        graph: &mut GraphData,
        node_map: &mut HashMap<u32, Node>,
        params: &SimulationParams,
    ) -> std::io::Result<()> {
        let nodes_len = graph.nodes.len();
        trace!("[calculate_layout_cpu] Starting CPU calculation with {} nodes", nodes_len);
        
        // Early return if there are no nodes to process
        if nodes_len == 0 {
            return Ok(());
        }
        
        // Initialize force accumulators for each node
        let mut forces = vec![(0.0, 0.0, 0.0); nodes_len];
        
        // Calculate repulsive forces between all pairs of nodes
        for i in 0..nodes_len {
            for j in (i+1)..nodes_len {
                let node_i = &graph.nodes[i];
                let node_j = &graph.nodes[j];
                
                // Calculate distance between nodes
                let dx = node_j.data.position.x - node_i.data.position.x;
                let dy = node_j.data.position.y - node_i.data.position.y;
                let dz = node_j.data.position.z - node_i.data.position.z;
                let distance_squared = dx * dx + dy * dy + dz * dz;
                
                // Avoid division by zero and limit maximum repulsion distance
                if distance_squared < 0.0001 { continue; }
                let distance = distance_squared.sqrt();
                if distance > params.max_repulsion_distance { continue; }
                
                // Calculate repulsion strength based on node masses (stored in data.mass) and distance
                let mass_i = (node_i.data.mass as f32 / 255.0) * 10.0 * params.mass_scale;
                let mass_j = (node_j.data.mass as f32 / 255.0) * 10.0 * params.mass_scale;
                let repulsion_factor = params.repulsion * mass_i * mass_j / distance_squared;
                
                // Normalize direction
                let nx = dx / distance;
                let ny = dy / distance;
                let nz = dz / distance;
                
                // Calculate forces (nodes push each other away)
                let fx = nx * repulsion_factor;
                let fy = ny * repulsion_factor;
                let fz = nz * repulsion_factor;
                
                // Apply forces to both nodes (equal and opposite)
                forces[i].0 -= fx;
                forces[i].1 -= fy;
                forces[i].2 -= fz;
                forces[j].0 += fx;
                forces[j].1 += fy;
                forces[j].2 += fz;
            }
        }
        
        // Calculate attractive forces for edges (spring forces)
        for edge in &graph.edges {
            let source_idx = graph.nodes.iter().position(|n| n.id == edge.source);
            let target_idx = graph.nodes.iter().position(|n| n.id == edge.target);
            
            if let (Some(i), Some(j)) = (source_idx, target_idx) {
                let node_i = &graph.nodes[i];
                let node_j = &graph.nodes[j];
                
                // Calculate distance between nodes
                let dx = node_j.data.position.x - node_i.data.position.x;
                let dy = node_j.data.position.y - node_i.data.position.y;
                let dz = node_j.data.position.z - node_i.data.position.z;
                let distance_squared = dx * dx + dy * dy + dz * dz;
                if distance_squared < 0.0001 { continue; }
                let distance = distance_squared.sqrt();
                
                // Spring force increases with distance and edge weight
                let spring_factor = params.spring_strength * edge.weight * distance;
                
                // Normalize direction
                let nx = dx / distance;
                let ny = dy / distance;
                let nz = dz / distance;
                
                // Calculate spring forces (edges pull nodes together)
                let fx = nx * spring_factor;
                let fy = ny * spring_factor;
                let fz = nz * spring_factor;
                
                // Apply spring forces 
                forces[i].0 += fx;
                forces[i].1 += fy;
                forces[i].2 += fz;
                forces[j].0 -= fx;
                forces[j].1 -= fy;
                forces[j].2 -= fz;
            }
        }
        
        // Update velocities and positions for all nodes
        for (i, node) in graph.nodes.iter_mut().enumerate() {            
            // Apply force to velocity with damping
            node.set_vx(node.data.velocity.x * params.damping + forces[i].0 * params.time_step);
            node.set_vy(node.data.velocity.y * params.damping + forces[i].1 * params.time_step);
            node.set_vz(node.data.velocity.z * params.damping + forces[i].2 * params.time_step);
            
            // Update position based on velocity
            node.set_x(node.data.position.x + node.data.velocity.x * params.time_step);
            node.set_y(node.data.position.y + node.data.velocity.y * params.time_step);
            node.set_z(node.data.position.z + node.data.velocity.z * params.time_step);
            
            // Update node_map as well
            if let Some(map_node) = node_map.get_mut(&node.id) {
                map_node.data = node.data.clone();
            }
        }
        
        Ok(())
    }

    pub async fn get_paginated_graph_data(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<PaginatedGraphData, Box<dyn std::error::Error + Send + Sync>> {
        let graph = self.graph_data.read().await;
        
        // Convert page and page_size to usize for vector operations
        let page = page as usize;
        let page_size = page_size as usize;
        let total_nodes = graph.nodes.len();
        
        let start = page * page_size;
        let end = std::cmp::min((page + 1) * page_size, total_nodes);

        let model_page_nodes: Vec<Node> = graph.nodes
            .iter()
            .skip(start)
            .take(end - start)
            .cloned()
            .collect();

        let page_nodes: Vec<crate::utils::socket_flow_messages::Node> = model_page_nodes.iter().map(|model_node| {
            crate::utils::socket_flow_messages::Node {
                id: model_node.id.to_string(), // Convert u32 to String
                metadata_id: model_node.metadata_id.clone(),
                label: model_node.label.clone(),
                data: model_node.data, // BinaryNodeData is the same
                metadata: model_node.metadata.clone(),
                file_size: model_node.file_size, // This field is present in socket_flow_messages::Node but marked #[serde(skip)]
                node_type: model_node.node_type.clone(),
                size: model_node.size,
                color: model_node.color.clone(),
                weight: model_node.weight,
                group: model_node.group.clone(),
                user_data: model_node.user_data.clone(),
            }
        }).collect();

        // Get edges that connect to these nodes
        let node_ids: HashSet<u32> = model_page_nodes.iter()
            .map(|n| n.id)
            .collect();

        let edges: Vec<Edge> = graph.edges
            .iter()
            .filter(|e| node_ids.contains(&e.source) || node_ids.contains(&e.target))
            .cloned()
            .collect();

        Ok(PaginatedGraphData {
            nodes: page_nodes,
            edges,
            metadata: serde_json::to_value(graph.metadata.clone()).unwrap_or_default(),
            total_nodes,
            total_edges: graph.edges.len(),
            total_pages: ((total_nodes as f32 / page_size as f32).ceil()) as u32,
            current_page: page as u32,
        })
    }
    
    // Clear position cache to force a refresh on next request
    pub async fn clear_position_cache(&self) {
        let mut cache = self.node_positions_cache.write().await;
        *cache = None;
    }

    pub async fn get_node_positions(&self) -> Vec<Node> {
        let start_time = Instant::now();

        // First check if we have a valid cached result
        if self.cache_enabled {
            let cache = self.node_positions_cache.read().await;
            if let Some((cached_nodes, timestamp)) = &*cache {
                let age = start_time.duration_since(*timestamp);
                
                // If cache is still fresh, use it
                if age < Duration::from_millis(NODE_POSITION_CACHE_TTL_MS) {
                    trace!("Using cached node positions ({} nodes, age: {:?})",
                           cached_nodes.len(), age);
                    return cached_nodes.clone();
                }
            }
        }

        // No valid cache, fetch from graph data
        let nodes = {
            let graph = self.graph_data.read().await;
            
            // Only log node position data in debug level
            trace!("get_node_positions: reading {} nodes from graph (cache miss)", graph.nodes.len());
            
            // Clone the nodes vector 
            graph.nodes.clone()
        };

        // Update cache with new result
        if self.cache_enabled {
            let mut cache = self.node_positions_cache.write().await;
            *cache = Some((nodes.clone(), start_time));
        }

        let elapsed = start_time.elapsed();
        trace!("Node position fetch completed in {:?} for {} nodes", elapsed, nodes.len());
        
        // Log first 5 nodes only when debug is enabled
        let sample_size = std::cmp::min(5, nodes.len());
        if sample_size > 0 && log::log_enabled!(log::Level::Debug) {
            trace!("Node position sample: {} samples of {} nodes", sample_size, nodes.len());
        }
        nodes
    }

    pub async fn get_graph_data_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, GraphData> {
        self.graph_data.write().await
    }

    pub async fn get_node_map_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, HashMap<u32, Node>> {
        self.node_map.write().await
    }
    
    // Add method to get GPU compute instance
    pub async fn get_gpu_compute(&self) -> Option<Arc<RwLock<GPUCompute>>> {
        self.gpu_compute.clone()
    }
 
    pub async fn update_node_positions(&self, updates: Vec<(u32, Node)>, client_manager_addr: Addr<ClientManagerActor>) -> Result<(), Error> {
        let mut graph = self.graph_data.write().await;
        let mut node_map = self.node_map.write().await;
        
        // Process node updates efficiently
        let mut _updated_count = 0;
        let mut _skipped_count = 0;
        
        // Process updates in batches
        for (node_id_u32, update_node) in updates {
            // Skip if this is a redundant update based on rate limiting
            if self.should_rate_limit().await {
                _skipped_count += 1;
                continue;
            }
            
            // Apply update with conflict resolution if node exists
            if let Some(existing_node) = node_map.get_mut(&node_id_u32) {
                // Create a new node with updated position/velocity but preserving other data
                let mut resolved_node = update_node.clone();
                
                // Preserve important attributes from existing node
                resolved_node.data.mass = existing_node.data.mass;
                resolved_node.data.flags = existing_node.data.flags;
                resolved_node.metadata = existing_node.metadata.clone();
                
                // Update the node in the map
                *existing_node = resolved_node;
                _updated_count += 1;
            }
        }
        
        // Sync graph nodes with node_map
        graph.nodes.iter_mut().for_each(|node| {
            if let Some(map_node) = node_map.get(&node.id) {
                node.data = map_node.data.clone();
            }
        });
        
        // Broadcast all positions
        Self::broadcast_positions(client_manager_addr, &graph.nodes).await;
        
        Ok(())
    }

    pub fn update_positions(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Error>> + '_>> {
        Box::pin(async move {
            if let Some(gpu) = &self.gpu_compute {
                let mut gpu = gpu.write().await;
                gpu.compute_forces()?;
                Ok(())
            } else {
                // Initialize GPU if not already done
                if self.gpu_compute.is_none() {
                    let graph_data_clone = {
                        let guard = self.graph_data.read().await;
                        guard.clone()
                    }; // Read guard is dropped here
                    self.initialize_gpu(&graph_data_clone).await?;
                    return self.update_positions().await;
                }
                Err(Error::new(ErrorKind::Other, "GPU compute not initialized"))
            }
        })
    }
 
pub async fn initialize_gpu(&mut self, graph_data: &GraphData) -> Result<(), Error> {
    info!("Initializing GPU compute system...");
 

        // If GPU is already initialized, don't reinitialize
        if self.gpu_compute.is_some() {
            info!("GPU compute is already initialized, skipping initialization");
            return Ok(());
        }

        match GPUCompute::new(graph_data).await {
            Ok(gpu_instance) => {
                // Try a test computation before accepting the GPU
                {
                    let mut gpu = gpu_instance.write().await;
                    if let Err(e) = gpu.compute_forces() {
                        error!("GPU test computation failed: {}", e);
                        return Err(Error::new(ErrorKind::Other, format!("GPU test computation failed: {}", e)));
                    }
                    info!("GPU test computation succeeded");
                }

                self.gpu_compute = Some(gpu_instance);
                info!("GPU compute system successfully initialized");
                Ok(())
            }
            Err(e) => {
                error!("Failed to initialize GPU compute: {}. Physics simulation will not work.", e);
                Err(Error::new(ErrorKind::Other, format!("GPU initialization failed: {}", e)))
            }
        }
    }

    /// Helper method to check GPU availability and print detailed diagnostics
    pub fn diagnose_gpu_status(gpu_compute: Option<Arc<RwLock<GPUCompute>>>) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        Box::pin(async move {
            info!("[GraphService] Diagnosing GPU status...");
            
            match gpu_compute {
                Some(gpu) => {
                    info!("[GraphService] GPU compute is available in service");
                    // Try a test computation 
                    if let Ok(gpu_lock) = gpu.try_read() {
                        match gpu_lock.test_compute() {
                            Ok(_) => {
                                info!("[GraphService] GPU test computation succeeded");
                                true
                            },
                            Err(e) => {
                                error!("[GraphService] GPU test computation failed: {}", e);
                                false
                            }
                        }
                    } else {
                        info!("[GraphService] Could not acquire GPU lock for diagnostics");
                        false
                    }
                },
                None => {
                    error!("[GraphService] GPU compute is NOT available in service");
                    
                    // Try to initialize it
                    info!("[GraphService] Attempting to initialize GPU on demand...");
                    false
                }
            }
        })
    }

    // Development test function to verify metadata transfer
    #[cfg(test)]
    pub async fn test_metadata_transfer() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use chrono::Utc;
        use std::collections::HashMap;
        use crate::models::metadata::Metadata;

        // Create test metadata
        let mut metadata = crate::models::metadata::MetadataStore::new();
        let file_name = "test.md";
        
        // Create a test metadata entry
        let meta = Metadata {
            file_name: file_name.to_string(),
            file_size: 1000,
            node_size: 1.5,
            hyperlink_count: 5,
            sha1: "abc123".to_string(),
            node_id: "1".to_string(), // This will be parsed to u32 during node creation
            last_modified: Utc::now(),
            last_content_change: Some(Utc::now()),
            last_commit: Some(Utc::now()),
            change_count: Some(1),
            file_blob_sha: Some("test_blob_sha".to_string()),
            perplexity_link: "https://example.com".to_string(),
            last_perplexity_process: Some(Utc::now()),
            topic_counts: HashMap::new(),
        };
        
        metadata.insert(file_name.to_string(), meta.clone());
        
        // Build graph from metadata
        let graph = Self::build_graph_from_metadata(&metadata).await?;
        
        // Check that the graph has one node with the correct metadata
        assert_eq!(graph.nodes.len(), 1);
        
        // Verify metadata_id
        let node = &graph.nodes[0];
        assert_eq!(node.metadata_id, "test");
        
        // Verify metadata fields
        assert!(node.metadata.contains_key("fileName"));
        assert_eq!(node.metadata.get("fileName").unwrap(), "test.md");
        
        assert!(node.metadata.contains_key("fileSize"));
        assert_eq!(node.metadata.get("fileSize").unwrap(), "1000");
        
        assert!(node.metadata.contains_key("nodeSize"));
        assert_eq!(node.metadata.get("nodeSize").unwrap(), "1.5");
        
        assert!(node.metadata.contains_key("hyperlinkCount"));
        assert_eq!(node.metadata.get("hyperlinkCount").unwrap(), "5");
        
        assert!(node.metadata.contains_key("sha1"));
        assert!(node.metadata.contains_key("lastModified"));
        
        // Check flags
        assert_eq!(node.data.flags, 1);

        println!("All metadata tests passed!");
        Ok(())
    }
    
    /// Start a separate broadcast loop to periodically push position updates to all clients
    pub fn start_broadcast_loop(&self, client_manager_addr: Addr<ClientManagerActor>) {
        info!("[GraphService] Starting position broadcast loop for client synchronization...");
 
        // Clone what we need for the async task
        let service_clone = self.clone();
        let simulation_id = self.simulation_id.clone();
        let captured_client_manager_addr = client_manager_addr.clone(); // Capture ClientManagerActor Addr for the loop
 
        // Spawn a new task for the broadcast loop
        tokio::spawn(async move {
            info!("[GraphService:{}] Position broadcast loop starting", simulation_id);
 
            // Main broadcast loop
            loop {
                // Check if shutdown was requested
                if service_clone.shutdown_requested.load(Ordering::SeqCst) {
                    info!("[GraphService:{}] Broadcast loop shutting down due to shutdown request", simulation_id);
                    break;
                }
 
                // Get current node positions
                let nodes = service_clone.get_node_positions().await;
 // Broadcast positions to all clients if we have any
 if !nodes.is_empty() {
     GraphService::broadcast_positions(captured_client_manager_addr.clone(), &nodes).await;
 }

 
                // Sleep to avoid excessive updates
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
 
            info!("[GraphService:{}] Position broadcast loop exited", simulation_id);
        });
        info!("[GraphService] Position broadcast loop started");
    }
}
