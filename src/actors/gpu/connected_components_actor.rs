//! Connected Components Actor - GPU-accelerated graph connectivity analysis
//!
//! This actor implements connected components detection using GPU label propagation.
//! Use cases:
//! - Identifying disconnected graph regions
//! - Graph partitioning analysis
//! - Cluster visualization
//! - Network fragmentation detection

use actix::prelude::*;
use log::info;
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
        }
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
                        // ADR-2054: the CPU fallback here previously ran
                        // `compute_components_cpu` against `cached_edges`, a field only
                        // ever populated by the `UpdateComponentEdges` message. That
                        // message had zero senders tree-wide, so `cached_edges` was
                        // always empty and the "fallback" silently returned every node
                        // as its own singleton component — a fabricated result, not a
                        // real fallback. Removed along with the message and the field;
                        // propagate the GPU failure instead.
                        return Err(format!(
                            "ConnectedComponentsActor: GPU path failed and no CPU fallback is available: {}",
                            e
                        ));
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

// REMOVED (ADR-2054): Handler<UpdateComponentEdges> — the message had zero senders
// tree-wide and only ever fed the now-removed `cached_edges` field.
