//! Connected Components Actor - GPU-accelerated graph connectivity analysis
//!
//! This actor implements connected components detection using GPU label propagation.
//! Use cases:
//! - Identifying disconnected graph regions
//! - Graph partitioning analysis
//! - Cluster visualization
//! - Network fragmentation detection

use actix::prelude::*;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::analytics_telemetry::{record_execution, AnalyticsKernel, ExecutionPath};
use super::shared::{GPUState, SharedGPUContext};
use crate::actors::messages::*;

// GPU kernel FFI declarations are now centralized in
// src/utils/unified_gpu_compute/types.rs and accessed through
// UnifiedGPUCompute::run_connected_components_gpu()

/// Connected components computation parameters
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<ConnectedComponentsResult, String>")]
pub struct ComputeConnectedComponents {
    /// Maximum iterations for label propagation
    pub max_iterations: Option<u32>,
    /// Convergence threshold
    pub convergence_threshold: Option<f32>,
}

/// Connected components result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedComponentsResult {
    /// Component label for each node
    pub labels: Vec<u32>,
    /// Number of connected components
    pub num_components: usize,
    /// Size of each component
    pub component_sizes: Vec<usize>,
    /// Largest component size
    pub largest_component_size: usize,
    /// Whether the graph is fully connected
    pub is_connected: bool,
    /// Number of iterations until convergence
    pub iterations: u32,
    /// Computation time in milliseconds
    pub computation_time_ms: u64,
    /// Which compute path this run actually executed on (task #74 zero-fallback
    /// gate). `cpu_fallback` means the GPU kernel failed and a CPU implementation
    /// ran instead — a gated regression, recorded as a `warn` and counted in the
    /// analytics telemetry snapshot.
    pub execution_path: ExecutionPath,
}

/// Component information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    /// Component ID
    pub id: u32,
    /// Nodes in this component
    pub nodes: Vec<u32>,
    /// Number of internal edges
    pub internal_edges: usize,
    /// Density of this component
    pub density: f32,
}

/// Connected components statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedComponentsStats {
    pub total_computations: u64,
    pub avg_computation_time_ms: f32,
    pub avg_num_components: f32,
    pub last_num_components: usize,
}

/// Connected Components Actor
pub struct ConnectedComponentsActor {
    /// GPU state tracking
    gpu_state: GPUState,

    /// Shared GPU context
    shared_context: Option<Arc<SharedGPUContext>>,

    /// Computation statistics
    stats: ConnectedComponentsStats,

    /// Cached edge list (source, target) updated via UpdateComponentEdges message
    cached_edges: Vec<(u32, u32)>,
}

impl ConnectedComponentsActor {
    pub fn new() -> Self {
        Self {
            gpu_state: GPUState::default(),
            shared_context: None,
            stats: ConnectedComponentsStats {
                total_computations: 0,
                avg_computation_time_ms: 0.0,
                avg_num_components: 0.0,
                last_num_components: 0,
            },
            cached_edges: Vec::new(),
        }
    }

    /// CPU-based label propagation fallback
    /// This will be replaced with GPU kernel when available
    fn compute_components_cpu(
        &self,
        num_nodes: usize,
        edges: &[(u32, u32)],
        max_iterations: u32,
    ) -> Result<(Vec<u32>, u32), String> {
        // Initialize each node with its own label
        let mut labels: Vec<u32> = (0..num_nodes as u32).collect();
        let mut changed = true;
        let mut iteration = 0;

        // Build adjacency list
        let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(src, dst) in edges {
            adjacency.entry(src).or_insert_with(Vec::new).push(dst);
            adjacency.entry(dst).or_insert_with(Vec::new).push(src);
        }

        // Propagate minimum label until convergence
        while changed && iteration < max_iterations {
            changed = false;
            iteration += 1;

            let old_labels = labels.clone();

            for node in 0..num_nodes as u32 {
                if let Some(neighbors) = adjacency.get(&node) {
                    // Find minimum label among neighbors
                    let min_neighbor_label = neighbors
                        .iter()
                        .map(|&n| old_labels[n as usize])
                        .min()
                        .unwrap_or(old_labels[node as usize]);

                    // Update to minimum of current label and neighbor labels
                    let new_label = old_labels[node as usize].min(min_neighbor_label);

                    if new_label != old_labels[node as usize] {
                        labels[node as usize] = new_label;
                        changed = true;
                    }
                }
            }
        }

        Ok((labels, iteration))
    }

    /// Analyze component statistics
    fn analyze_components(&self, labels: &[u32]) -> (usize, Vec<usize>, usize, bool) {
        let mut component_sizes: HashMap<u32, usize> = HashMap::new();

        for &label in labels {
            *component_sizes.entry(label).or_insert(0) += 1;
        }

        let num_components = component_sizes.len();
        let sizes: Vec<usize> = component_sizes.values().copied().collect();
        let largest = sizes.iter().max().copied().unwrap_or(0);
        let is_connected = num_components == 1;

        (num_components, sizes, largest, is_connected)
    }

    /// Update statistics
    fn update_stats(&mut self, time_ms: u64, num_components: usize) {
        let total = self.stats.total_computations as f32;

        self.stats.avg_computation_time_ms =
            (self.stats.avg_computation_time_ms * total + time_ms as f32) / (total + 1.0);

        self.stats.avg_num_components =
            (self.stats.avg_num_components * total + num_components as f32) / (total + 1.0);

        self.stats.last_num_components = num_components;
        self.stats.total_computations += 1;
    }
}

impl Default for ConnectedComponentsActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for ConnectedComponentsActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("ConnectedComponentsActor started");
        ctx.notify(InitializeActor);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("ConnectedComponentsActor stopped");
    }
}

// Message Handlers

impl Handler<InitializeActor> for ConnectedComponentsActor {
    type Result = ();

    fn handle(&mut self, _msg: InitializeActor, _ctx: &mut Self::Context) -> Self::Result {
        info!("ConnectedComponentsActor: Initializing");
        self.gpu_state.is_initialized = true;
    }
}

impl Handler<SetSharedGPUContext> for ConnectedComponentsActor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: SetSharedGPUContext, _ctx: &mut Self::Context) -> Self::Result {
        info!("ConnectedComponentsActor: Setting GPU context");
        self.shared_context = Some(msg.context);
        self.gpu_state.is_initialized = true;
        Ok(())
    }
}

impl Handler<ComputeConnectedComponents> for ConnectedComponentsActor {
    type Result = Result<ConnectedComponentsResult, String>;

    fn handle(
        &mut self,
        msg: ComputeConnectedComponents,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("ConnectedComponentsActor: Computing connected components");

        let start_time = Instant::now();

        let max_iterations = msg.max_iterations.unwrap_or(100);

        // Get GPU context and try GPU path first
        let (labels, iterations, execution_path) = match &self.shared_context {
            Some(ctx) => {
                let mut unified_compute = ctx
                    .unified_compute
                    .lock()
                    .map_err(|e| format!("Failed to acquire GPU compute lock: {}", e))?;

                let num_nodes = unified_compute.get_num_nodes();

                // Try GPU-accelerated connected components
                match unified_compute.run_connected_components_gpu(max_iterations as i32) {
                    Ok((gpu_labels, _num_comp)) => {
                        // Task #74: record the GPU path (expected, non-gated).
                        let path = record_execution(
                            AnalyticsKernel::ConnectedComponents,
                            ExecutionPath::Gpu,
                        );
                        let labels: Vec<u32> = gpu_labels.iter().map(|&l| l as u32).collect();
                        (labels, max_iterations, path)
                    }
                    Err(e) => {
                        // Task #74 zero-fallback gate: the GPU kernel failed and a CPU
                        // implementation is about to run. record_execution emits the
                        // gated `warn` and increments the fallback counter so this is
                        // never a silent substitution.
                        warn!(
                            "ConnectedComponentsActor: GPU path failed ({}), falling back to CPU",
                            e
                        );
                        let path = record_execution(
                            AnalyticsKernel::ConnectedComponents,
                            ExecutionPath::CpuFallback,
                        );
                        drop(unified_compute);
                        let (labels, iterations) = self.compute_components_cpu(
                            num_nodes,
                            &self.cached_edges,
                            max_iterations,
                        )?;
                        (labels, iterations, path)
                    }
                }
            }
            None => {
                return Err("GPU context not initialized".to_string());
            }
        };

        let (num_components, component_sizes, largest_component_size, is_connected) =
            self.analyze_components(&labels);

        let computation_time = start_time.elapsed().as_millis() as u64;
        self.update_stats(computation_time, num_components);

        info!(
            "ConnectedComponentsActor: Found {} components in {}ms (path={})",
            num_components,
            computation_time,
            execution_path.as_str()
        );

        Ok(ConnectedComponentsResult {
            labels,
            num_components,
            component_sizes,
            largest_component_size,
            is_connected,
            iterations,
            computation_time_ms: computation_time,
            execution_path,
        })
    }
}

/// Get connected components statistics
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "ConnectedComponentsStats")]
pub struct GetConnectedComponentsStats;

impl Handler<GetConnectedComponentsStats> for ConnectedComponentsActor {
    type Result = MessageResult<GetConnectedComponentsStats>;

    fn handle(
        &mut self,
        _msg: GetConnectedComponentsStats,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        MessageResult(self.stats.clone())
    }
}

impl Handler<UpdateComponentEdges> for ConnectedComponentsActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateComponentEdges, _ctx: &mut Self::Context) -> Self::Result {
        info!(
            "ConnectedComponentsActor: Updated cached edges ({} edges)",
            msg.edges.len()
        );
        self.cached_edges = msg.edges;
    }
}
