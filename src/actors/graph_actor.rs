//! Graph Service Actor to replace Arc<RwLock<GraphService>>

use actix::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::time::Duration;
use log::{debug, info, warn, error};
// use actix::fut::WrapFuture; // Unused import
 
use crate::actors::messages::*;
use crate::actors::client_manager_actor::ClientManagerActor;
use crate::models::node::Node;
use crate::models::edge::Edge;
use crate::models::metadata::MetadataStore;
use crate::models::graph::GraphData;
use crate::utils::socket_flow_messages::{BinaryNodeData, glam_to_vec3data}; // Added glam_to_vec3data
use crate::utils::binary_protocol;
use crate::actors::gpu_compute_actor::GPUComputeActor;

pub struct GraphServiceActor {
    graph_data: Arc<GraphData>, // Changed to Arc<GraphData>
    node_map: HashMap<u32, Node>,
    // gpu_compute_addr: Option<Addr<GPUComputeActor>>, // Unused
    client_manager: Addr<ClientManagerActor>,
    simulation_running: AtomicBool,
    shutdown_complete: Arc<AtomicBool>,
    next_node_id: AtomicU32,
    bots_graph_data: GraphData, // Add a new field for the bots graph
}

impl GraphServiceActor {
    pub fn new(
        client_manager: Addr<ClientManagerActor>,
        _gpu_compute_addr: Option<Addr<GPUComputeActor>>, // Marked as unused
    ) -> Self {
        Self {
            graph_data: Arc::new(GraphData::new()), // Changed to Arc::new
            node_map: HashMap::new(),
            // gpu_compute_addr, // Unused
            client_manager,
            simulation_running: AtomicBool::new(false),
            shutdown_complete: Arc::new(AtomicBool::new(false)),
            next_node_id: AtomicU32::new(1),
            bots_graph_data: GraphData::new(),
        }
    }

    pub fn get_graph_data(&self) -> &GraphData { // Returns a reference to the inner GraphData
        &self.graph_data // Dereferences Arc<GraphData> to &GraphData
    }

    pub fn get_node_map(&self) -> &HashMap<u32, Node> {
        &self.node_map
    }

    pub fn add_node(&mut self, node: Node) {
        let node_id = node.id; // Store the ID before moving node
        
        // Update node_map
        self.node_map.insert(node.id, node.clone());
        
        let graph_data_mut = Arc::make_mut(&mut self.graph_data);
        // Add to graph data if not already present
        if !graph_data_mut.nodes.iter().any(|n| n.id == node.id) {
            graph_data_mut.nodes.push(node);
        } else {
            // Update existing node
            if let Some(existing) = graph_data_mut.nodes.iter_mut().find(|n| n.id == node.id) {
                *existing = node; // Move node here instead of cloning
            }
        }
        
        debug!("Added/updated node: {}", node_id);
    }

    pub fn remove_node(&mut self, node_id: u32) {
        // Remove from node_map
        self.node_map.remove(&node_id);
        
        let graph_data_mut = Arc::make_mut(&mut self.graph_data);
        // Remove from graph data
        graph_data_mut.nodes.retain(|n| n.id != node_id);
        
        // Remove related edges
        graph_data_mut.edges.retain(|e| e.source != node_id && e.target != node_id);
        
        debug!("Removed node: {}", node_id);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        let edge_id = edge.id.clone(); // Store the ID before moving edge
        
        let graph_data_mut = Arc::make_mut(&mut self.graph_data);
        // Add to graph data if not already present
        if !graph_data_mut.edges.iter().any(|e| e.id == edge.id) {
            graph_data_mut.edges.push(edge);
        } else {
            // Update existing edge
            if let Some(existing) = graph_data_mut.edges.iter_mut().find(|e| e.id == edge.id) {
                *existing = edge; // Move edge here instead of cloning
            }
        }
        
        debug!("Added/updated edge: {}", edge_id);
    }

    pub fn remove_edge(&mut self, edge_id: &str) {
        Arc::make_mut(&mut self.graph_data).edges.retain(|e| e.id != edge_id);
        debug!("Removed edge: {}", edge_id);
    }

    pub fn build_from_metadata(&mut self, metadata: MetadataStore) -> Result<(), String> {
        let mut new_graph_data = GraphData::new(); // Create a new GraphData instance
        self.node_map.clear(); // Clear node_map separately

        // Build nodes from metadata
        // Assuming metadata is MetadataStore which is HashMap<String, crate::models::metadata::Metadata>
        for (filename_with_ext, file_meta_data) in &metadata {
            let node_id_val = self.next_node_id.fetch_add(1, Ordering::SeqCst);
            let metadata_id_val = filename_with_ext.trim_end_matches(".md").to_string();
            
            let mut node = Node::new_with_id(metadata_id_val.clone(), Some(node_id_val));
            node.label = file_meta_data.file_name.trim_end_matches(".md").to_string();
            node.set_file_size(file_meta_data.file_size as u64);
            node.data.flags = 1;

            node.metadata.insert("fileName".to_string(), file_meta_data.file_name.clone());
            node.metadata.insert("fileSize".to_string(), file_meta_data.file_size.to_string());
            node.metadata.insert("nodeSize".to_string(), file_meta_data.node_size.to_string());
            node.metadata.insert("hyperlinkCount".to_string(), file_meta_data.hyperlink_count.to_string());
            node.metadata.insert("sha1".to_string(), file_meta_data.sha1.clone());
            node.metadata.insert("lastModified".to_string(), file_meta_data.last_modified.to_rfc3339());
            if !file_meta_data.perplexity_link.is_empty() {
                node.metadata.insert("perplexityLink".to_string(), file_meta_data.perplexity_link.clone());
            }
            if let Some(last_process) = file_meta_data.last_perplexity_process {
                node.metadata.insert("lastPerplexityProcess".to_string(), last_process.to_rfc3339());
            }
            node.metadata.insert("metadataId".to_string(), metadata_id_val);

            // Add to new_graph_data and self.node_map
            self.node_map.insert(node.id, node.clone());
            new_graph_data.nodes.push(node);
        }

        // Build edges from topic counts
        let mut edge_map: HashMap<(u32, u32), f32> = HashMap::new();
        for (source_filename_ext, source_meta) in &metadata {
            let source_metadata_id = source_filename_ext.trim_end_matches(".md");
            if let Some(source_node) = self.node_map.values().find(|n| n.metadata_id == source_metadata_id) {
                for (target_filename_ext, count) in &source_meta.topic_counts {
                    let target_metadata_id = target_filename_ext.trim_end_matches(".md");
                    if let Some(target_node) = self.node_map.values().find(|n| n.metadata_id == target_metadata_id) {
                        if source_node.id != target_node.id {
                            let edge_key = if source_node.id < target_node.id { (source_node.id, target_node.id) } else { (target_node.id, source_node.id) };
                            *edge_map.entry(edge_key).or_insert(0.0) += *count as f32;
                        }
                    }
                }
            }
        }

        for ((source_id, target_id), weight) in edge_map {
            new_graph_data.edges.push(Edge::new(source_id, target_id, weight));
        }
        
        // Populate metadata in new_graph_data (assuming metadata is MetadataStore)
        new_graph_data.metadata = metadata.clone(); // Clone the entire store

        self.graph_data = Arc::new(new_graph_data); // Replace the old Arc with the new one
        
        info!("Built graph from metadata: {} nodes, {} edges",
              self.graph_data.nodes.len(), self.graph_data.edges.len());
        
        Ok(())
    }

    pub fn update_node_positions(&mut self, positions: Vec<(u32, BinaryNodeData)>) {
        let mut updated_count = 0;
        let graph_data_mut = Arc::make_mut(&mut self.graph_data);
        
        for (node_id, position_data) in positions {
            // Update in node_map
            if let Some(node) = self.node_map.get_mut(&node_id) {
                node.data.position = position_data.position;
                node.data.velocity = position_data.velocity;
                updated_count += 1;
            }
            
            // Update in graph_data.nodes
            if let Some(node) = graph_data_mut.nodes.iter_mut().find(|n| n.id == node_id) {
                node.data.position = position_data.position;
                node.data.velocity = position_data.velocity;
            }
        }
        
        debug!("Updated positions for {} nodes", updated_count);
    }

    fn start_simulation_loop(&mut self, ctx: &mut Context<Self>) {
        if self.simulation_running.load(Ordering::SeqCst) {
            warn!("Simulation already running");
            return;
        }

        self.simulation_running.store(true, Ordering::SeqCst);
        info!("Starting physics simulation loop");

        // Start the simulation interval
        ctx.run_interval(Duration::from_millis(16), |actor, _ctx| {
            if !actor.simulation_running.load(Ordering::SeqCst) {
                return;
            }

            actor.run_simulation_step();
        });
    }

    fn run_simulation_step(&mut self) {
        // Run physics calculation (GPU or CPU fallback)
        match self.calculate_layout() {
            Ok(updated_positions) => {
                if !updated_positions.is_empty() {
                    // Update positions
                    self.update_node_positions(updated_positions.clone());
                    
                    // Broadcast to clients
                    if let Ok(binary_data) = self.encode_node_positions(&updated_positions) {
                        self.client_manager.do_send(BroadcastNodePositions { 
                            positions: binary_data 
                        });
                    }
                }
            }
            Err(e) => {
                error!("Physics simulation step failed: {}", e);
            }
        }
    }

    fn calculate_layout(&self) -> Result<Vec<(u32, BinaryNodeData)>, String> {
        // For now, always use CPU fallback since GPU actor communication is async
        // TODO: Refactor simulation loop to handle async GPU computation properly
        self.calculate_layout_cpu()
    }

    /*
    fn initiate_gpu_computation(&self, ctx: &mut Context<Self>) {
        // Send GPU computation request if GPU compute actor is available
        if let Some(ref gpu_compute_addr) = self.gpu_compute_addr {
            // Update graph data in GPU
            let graph_data_for_gpu = crate::models::graph::GraphData { // Renamed to avoid conflict
                nodes: self.graph_data.nodes.clone(),
                edges: self.graph_data.edges.clone(),
                metadata: self.graph_data.metadata.clone(), // Include metadata
                id_to_metadata: self.graph_data.id_to_metadata.clone(), // Include id_to_metadata
            };
            
            gpu_compute_addr.do_send(UpdateGPUGraphData { graph: graph_data_for_gpu });
            
            // Request computation
            let addr = gpu_compute_addr.clone();
            // Collect node IDs beforehand to avoid borrowing `self` inside the async block
            let node_ids_in_order: Vec<u32> = self.graph_data.nodes.iter().map(|n| n.id).collect();

            let future = async move {
                match addr.send(ComputeForces).await {
                    Ok(Ok(())) => {
                        // Now get the results
                        match addr.send(GetNodeData).await {
                            Ok(Ok(node_data)) => {
                                // Convert to position updates
                                let mut positions = Vec::new();
                                for (index, data) in node_data.iter().enumerate() {
                                    // Use the pre-collected node_ids_in_order
                                    if let Some(node_id) = node_ids_in_order.get(index) {
                                        positions.push((*node_id, data.clone()));
                                    }
                                }
                                positions
                            },
                            _ => Vec::new()
                        }
                    },
                    _ => Vec::new()
                }
            };
            
            // Convert future to ActorFuture and spawn it
            ctx.wait(future.into_actor(self).map(|positions, actor, _ctx| {
                if !positions.is_empty() {
                    actor.update_node_positions(positions.clone());
                    
                    // Broadcast to clients
                    if let Ok(binary_data) = actor.encode_node_positions(&positions) {
                        actor.client_manager.do_send(BroadcastNodePositions {
                            positions: binary_data
                        });
                    }
                }
            }));
        }
    }
    */

    fn calculate_layout_cpu(&self) -> Result<Vec<(u32, BinaryNodeData)>, String> {
        // Simple CPU physics simulation
        let mut updated_positions = Vec::new();
        
        for node in &self.graph_data.nodes {
            // Simple physics: apply some random movement for demo
            let mut new_data = node.data.clone();
            new_data.position.x += (rand::random::<f32>() - 0.5) * 0.1;
            new_data.position.y += (rand::random::<f32>() - 0.5) * 0.1;
            new_data.position.z += (rand::random::<f32>() - 0.5) * 0.1;
            
            updated_positions.push((node.id, new_data));
        }
        
        Ok(updated_positions)
    }

    fn encode_node_positions(&self, positions: &[(u32, BinaryNodeData)]) -> Result<Vec<u8>, String> {
        // Now binary_protocol expects (u32, BinaryNodeData) directly
        Ok(binary_protocol::encode_node_data(positions))
    }
}

impl Actor for GraphServiceActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("GraphServiceActor started");
        self.start_simulation_loop(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.simulation_running.store(false, Ordering::SeqCst);
        self.shutdown_complete.store(true, Ordering::SeqCst);
        info!("GraphServiceActor stopped");
    }
}

// Message handlers
impl Handler<GetGraphData> for GraphServiceActor {
    type Result = Result<GraphData, String>; // Result type changed from Arc<GraphData>
 
    fn handle(&mut self, _msg: GetGraphData, _ctx: &mut Self::Context) -> Self::Result {
        info!("DEBUG_VERIFICATION: GraphServiceActor handling GetGraphData with OWNED data clone strategy.");
        Ok((*self.graph_data).clone()) // Clones the GraphData itself
    }
}

impl Handler<UpdateNodePositions> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: UpdateNodePositions, _ctx: &mut Self::Context) -> Self::Result {
        self.update_node_positions(msg.positions);
        Ok(())
    }
}

impl Handler<AddNode> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: AddNode, _ctx: &mut Self::Context) -> Self::Result {
        self.add_node(msg.node);
        Ok(())
    }
}

impl Handler<RemoveNode> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: RemoveNode, _ctx: &mut Self::Context) -> Self::Result {
        self.remove_node(msg.node_id);
        Ok(())
    }
}

impl Handler<AddEdge> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: AddEdge, _ctx: &mut Self::Context) -> Self::Result {
        self.add_edge(msg.edge);
        Ok(())
    }
}

impl Handler<RemoveEdge> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: RemoveEdge, _ctx: &mut Self::Context) -> Self::Result {
        self.remove_edge(&msg.edge_id);
        Ok(())
    }
}

impl Handler<GetNodeMap> for GraphServiceActor {
    type Result = Result<HashMap<u32, Node>, String>;

    fn handle(&mut self, _msg: GetNodeMap, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.node_map.clone())
    }
}

impl Handler<BuildGraphFromMetadata> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: BuildGraphFromMetadata, _ctx: &mut Self::Context) -> Self::Result {
        self.build_from_metadata(msg.metadata)
    }
}

impl Handler<StartSimulation> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, _msg: StartSimulation, ctx: &mut Self::Context) -> Self::Result {
        self.start_simulation_loop(ctx);
        Ok(())
    }
}

impl Handler<StopSimulation> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, _msg: StopSimulation, _ctx: &mut Self::Context) -> Self::Result {
        self.simulation_running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

impl Handler<UpdateNodePosition> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: UpdateNodePosition, _ctx: &mut Self::Context) -> Self::Result {
        // Update node in the node map
        if let Some(node) = self.node_map.get_mut(&msg.node_id) {
            // Preserve existing mass and flags
            let original_mass = node.data.mass;
            let original_flags = node.data.flags;
            
            node.data.position = glam_to_vec3data(msg.position);
            node.data.velocity = glam_to_vec3data(msg.velocity);
            
            // Restore mass and flags
            node.data.mass = original_mass;
            node.data.flags = original_flags;
        } else {
            debug!("Received update for unknown node ID: {}", msg.node_id);
            return Err(format!("Unknown node ID: {}", msg.node_id));
        }
        
        // Update corresponding node in graph
        let graph_data_mut = Arc::make_mut(&mut self.graph_data);
        for node_in_graph_data in &mut graph_data_mut.nodes { // Iterate over mutable graph_data
            if node_in_graph_data.id == msg.node_id {
                // Preserve mass and flags
                let original_mass = node_in_graph_data.data.mass;
                let original_flags = node_in_graph_data.data.flags;
                
                node_in_graph_data.data.position = glam_to_vec3data(msg.position);
                node_in_graph_data.data.velocity = glam_to_vec3data(msg.velocity);
                
                // Restore mass and flags
                node_in_graph_data.data.mass = original_mass;
                node_in_graph_data.data.flags = original_flags;
                break;
            }
        }
        
        Ok(())
    }
}

impl Handler<SimulationStep> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, _msg: SimulationStep, _ctx: &mut Self::Context) -> Self::Result {
        // Just run one simulation step
        self.run_simulation_step();
        Ok(())
    }
}

impl Handler<UpdateGraphData> for GraphServiceActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: UpdateGraphData, _ctx: &mut Self::Context) -> Self::Result {
        info!("Updating graph data with {} nodes, {} edges",
              msg.graph_data.nodes.len(), msg.graph_data.edges.len());
        
        // Update graph data by creating a new Arc
        self.graph_data = Arc::new(msg.graph_data);
        
        // Rebuild node map
        self.node_map.clear();
        for node in &self.graph_data.nodes { // Dereferences Arc for iteration
            self.node_map.insert(node.id, node.clone());
        }
        
        info!("Graph data updated successfully");
        Ok(())
    }
}

impl Handler<UpdateBotsGraph> for GraphServiceActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateBotsGraph, _ctx: &mut Context<Self>) -> Self::Result {
        // This logic converts `AgentStatus` objects into `Node` and `Edge` objects
        let mut nodes = vec![];
        let edges = vec![]; // Edges can be derived from agent communication patterns
        
        // Use a high ID range (starting at 10000) to avoid conflicts with main graph
        let bot_id_offset = 10000;
        
        for (i, agent) in msg.agents.iter().enumerate() {
            let node_id = bot_id_offset + i as u32;
            
            // Create a node for each agent
            let mut node = Node::new_with_id(agent.agent_id.clone(), Some(node_id));
            
            // Set node properties based on agent status
            node.color = Some(match agent.profile.agent_type {
                crate::services::claude_flow::AgentType::Coordinator => "#FF6B6B".to_string(),
                crate::services::claude_flow::AgentType::Researcher => "#4ECDC4".to_string(),
                crate::services::claude_flow::AgentType::Coder => "#45B7D1".to_string(),
                crate::services::claude_flow::AgentType::Analyst => "#FFA07A".to_string(),
                crate::services::claude_flow::AgentType::Architect => "#98D8C8".to_string(),
                crate::services::claude_flow::AgentType::Tester => "#F7DC6F".to_string(),
                _ => "#95A5A6".to_string(),
            });
            
            node.label = agent.profile.name.clone();
            // node.shape = "circle".to_string(); // Node struct doesn't have a shape field
            node.size = Some(20.0 + (agent.active_tasks_count as f32 * 5.0)); // Size based on activity
            
            // Add metadata
            node.metadata.insert("agent_type".to_string(), format!("{:?}", agent.profile.agent_type));
            node.metadata.insert("status".to_string(), agent.status.clone());
            node.metadata.insert("active_tasks".to_string(), agent.active_tasks_count.to_string());
            node.metadata.insert("completed_tasks".to_string(), agent.completed_tasks_count.to_string());
            
            nodes.push(node);
        }
        
        // Update the bots graph data
        self.bots_graph_data.nodes = nodes;
        self.bots_graph_data.edges = edges;
        
        // Create a message to broadcast the bots graph update to clients
        // This would need to be implemented based on how the client expects bot data
        // For now, we'll just log the update
        info!("Updated bots graph with {} agents", msg.agents.len());
        
        // TODO: Implement broadcasting bots graph to clients
        // This might involve creating a new message type or modifying the existing
        // binary protocol to support dual graphs
    }
}
