use actix_web::{web, HttpResponse, Result};
use log::{debug, error, info, warn};

use crate::actors::messages::GetPhysicsStats;
use crate::AppState;
use crate::{ok_json, service_unavailable};

use super::real_gpu_functions::get_real_gpu_physics_stats;
use super::types::{GPUPhysicsStats, StatsResponse, SystemMetrics};

pub(crate) async fn calculate_network_metrics(
    _app_state: &AppState,
    physics_stats: &Option<GPUPhysicsStats>,
) -> (f32, f32, f32, f32, f32) {
    let active_nodes = physics_stats.as_ref().map(|s| s.nodes_count).unwrap_or(0) as f32;
    let active_edges = physics_stats.as_ref().map(|s| s.edges_count).unwrap_or(0) as f32;

    let bytes_per_node_per_frame = 38.0;
    let frames_per_second = 60.0;
    let seconds_per_minute = 60.0;

    let data_transfer_mb =
        (active_nodes * bytes_per_node_per_frame * frames_per_second * seconds_per_minute)
            / (1024.0 * 1024.0);

    let bandwidth_usage_mbps =
        (active_nodes * bytes_per_node_per_frame * frames_per_second * 8.0) / (1024.0 * 1024.0);

    let cost_per_gb = 0.09;
    let cost_per_mb = cost_per_gb / 1024.0;
    let network_cost_per_mb = cost_per_mb;

    let total_network_cost = data_transfer_mb * network_cost_per_mb;

    let base_latency = 15.0;
    let complexity_factor = (active_edges / (active_nodes + 1.0)).min(10.0);
    let network_latency_ms = base_latency + (complexity_factor * 2.0);

    let base_mcp_latency = 5.0;

    let coordination_overhead = (active_edges / 1000.0).min(20.0);
    let mcp_latency = base_mcp_latency + coordination_overhead;

    let final_network_latency = network_latency_ms + mcp_latency;

    (
        network_cost_per_mb,
        total_network_cost,
        bandwidth_usage_mbps,
        data_transfer_mb,
        final_network_latency,
    )
}

pub async fn get_performance_stats(app_state: web::Data<AppState>) -> Result<HttpResponse> {
    info!("Getting performance statistics");

    let physics_stats: Option<GPUPhysicsStats> =
        if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
            match gpu_addr.send(GetPhysicsStats).await {
                Ok(Ok(stats)) => Some(GPUPhysicsStats {
                    iteration_count: stats.iteration_count,
                    nodes_count: stats.nodes_count,
                    edges_count: stats.edges_count,
                    kinetic_energy: stats.average_velocity,
                    total_forces: 0.0,
                    gpu_enabled: true,
                    compute_mode: format!("{:?}", stats.compute_mode),
                    kernel_mode: String::new(),
                    num_nodes: stats.nodes_count,
                    num_edges: stats.edges_count,
                    num_constraints: 0,
                    num_isolation_layers: 0,
                    stress_majorization_interval: 0,
                    last_stress_majorization: 0,
                    gpu_failure_count: stats.gpu_failure_count,
                    has_advanced_features: false,
                    has_dual_graph_features: false,
                    has_visual_analytics_features: false,
                    stress_safety_stats: super::types::StressMajorizationStats {
                        total_runs: 0,
                        successful_runs: 0,
                        failed_runs: 0,
                        consecutive_failures: 0,
                        emergency_stopped: false,
                        last_error: String::new(),
                        average_computation_time_ms: 0,
                        success_rate: 0.0,
                        is_emergency_stopped: false,
                        emergency_stop_reason: String::new(),
                        avg_computation_time_ms: 0,
                        avg_stress: 0.0,
                        avg_displacement: 0.0,
                        is_converging: false,
                    },
                }),
                Ok(Err(e)) => {
                    warn!("Failed to get physics stats: {}", e);
                    None
                }
                Err(e) => {
                    warn!("GPU compute actor mailbox error: {}", e);
                    None
                }
            }
        } else {
            None
        };

    let (
        network_cost_per_mb,
        total_network_cost,
        bandwidth_usage_mbps,
        data_transfer_mb,
        network_latency_ms,
    ) = calculate_network_metrics(&app_state, &physics_stats).await;

    let active_nodes = physics_stats.as_ref().map(|s| s.nodes_count).unwrap_or(0);
    let active_edges = physics_stats.as_ref().map(|s| s.edges_count).unwrap_or(0);

    // Frame timing not yet instrumented — report zero rather than fabricate
    let frame_time_ms = 0.0f32;
    let fps = 0.0f32;

    let system_metrics = SystemMetrics {
        fps,
        frame_time_ms,
        gpu_utilization: 0.0, // not yet measured — requires GPU-specific query
        memory_usage_mb: 0.0, // not yet measured — requires GPU memory query
        active_nodes,
        active_edges,
        render_time_ms: 0.0, // not yet measured — requires render pipeline instrumentation
        network_cost_per_mb,
        total_network_cost,
        bandwidth_usage_mbps,
        data_transfer_mb,
        network_latency_ms,
    };

    ok_json!(StatsResponse {
        success: true,
        physics_stats: get_real_gpu_physics_stats(&app_state).await,
        visual_analytics_metrics: None,
        system_metrics: Some(system_metrics),
        error: None,
    })
}

pub async fn get_gpu_metrics(
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Retrieving GPU performance metrics");

    // Task #74: per-kernel GPU-vs-fallback execution-path counters are process-global
    // (the GPU analytics actors record them via analytics_telemetry::record_execution),
    // so surface them on every response — including failure branches — without an actor
    // round-trip. A non-zero `total_cpu_fallbacks` means the zero-fallback intent has
    // been violated this process lifetime.
    let analytics_execution = serde_json::json!({
        "per_kernel": crate::actors::gpu::analytics_telemetry::snapshot(),
        "total_cpu_fallbacks": crate::actors::gpu::analytics_telemetry::total_cpu_fallbacks(),
    });

    if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
        use crate::actors::messages::GetGPUMetrics;

        match gpu_addr.send(GetGPUMetrics).await {
            Ok(Ok(metrics)) => {
                info!("GPU metrics retrieved successfully");
                ok_json!(serde_json::json!({
                    "metrics": metrics,
                    "analytics_execution": analytics_execution,
                }))
            }
            Ok(Err(e)) => {
                error!("Failed to get GPU metrics: {}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": e,
                    "gpu_initialized": false,
                    "analytics_execution": analytics_execution
                })))
            }
            Err(e) => {
                error!("GPU actor mailbox error: {}", e);
                service_unavailable!("GPU compute actor unavailable")
            }
        }
    } else {
        warn!("GPU compute actor not available");
        service_unavailable!(
            "GPU compute not available - GPU acceleration is not enabled or not available"
        )
    }
}

pub async fn get_gpu_status(app_state: web::Data<AppState>) -> Result<HttpResponse> {
    info!("Control center requesting comprehensive GPU status");

    let gpu_status = if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
        match gpu_addr
            .send(crate::actors::messages::GetPhysicsStats)
            .await
        {
            Ok(Ok(stats)) => {
                let clustering_tasks = super::state::CLUSTERING_TASKS.lock().await;
                let anomaly_state = super::state::ANOMALY_STATE.lock().await;

                serde_json::json!({
                    "success": true,
                    "gpu_available": true,
                    "status": "active",
                    "compute": {
                        "kernel_mode": "advanced",
                        "nodes_processed": stats.nodes_count,
                        "edges_processed": stats.edges_count,
                        "iteration_count": stats.iteration_count
                    },
                    "analytics": {
                        "clustering_active": !clustering_tasks.is_empty(),
                        "active_clustering_tasks": clustering_tasks.len(),
                        "anomaly_detection_enabled": anomaly_state.enabled,
                        "anomalies_detected": anomaly_state.stats.total,
                        "critical_anomalies": anomaly_state.stats.critical
                    },
                    "performance": {
                        "gpu_utilization": 0.0,  // not yet measured
                        "memory_usage_percent": 0.0,  // not yet measured
                        "temperature": 0.0,  // not yet measured
                        "power_draw": 0.0  // not yet measured
                    },
                    "features": {
                        "stress_majorization": true,
                        "semantic_constraints": true,
                        "sssp_integration": true,
                        "spatial_hashing": true,
                        "real_time_clustering": true,
                        "anomaly_detection": true
                    },
                    "last_updated": chrono::Utc::now().timestamp_millis()
                })
            }
            Ok(Err(e)) => {
                serde_json::json!({
                    "success": false,
                    "gpu_available": false,
                    "status": "error",
                    "error": e,
                    "fallback_active": true
                })
            }
            Err(_) => {
                serde_json::json!({
                    "success": false,
                    "gpu_available": false,
                    "status": "unavailable",
                    "fallback_active": true
                })
            }
        }
    } else {
        serde_json::json!({
            "success": true,
            "gpu_available": false,
            "status": "cpu_only",
            "fallback_active": true,
            "features": {
                "stress_majorization": false,
                "semantic_constraints": false,
                "sssp_integration": false,
                "spatial_hashing": false,
                "real_time_clustering": false,
                "anomaly_detection": false
            }
        })
    };

    ok_json!(gpu_status)
}

pub async fn get_gpu_features(app_state: web::Data<AppState>) -> Result<HttpResponse> {
    info!("Client requesting GPU feature capabilities");

    let features = if let Some(_gpu_addr) = app_state.get_gpu_compute_addr().await {
        serde_json::json!({
            "success": true,
            "gpu_acceleration": true,
            "features": {
                "clustering": {
                    "available": true,
                    "methods": ["kmeans", "spectral", "dbscan", "louvain", "hierarchical", "affinity"],
                    "gpu_accelerated": true,
                    "max_clusters": 50,
                    "max_nodes": 100000
                },
                "anomaly_detection": {
                    "available": true,
                    "methods": ["isolation_forest", "lof", "autoencoder", "statistical", "temporal"],
                    "real_time": true,
                    "gpu_accelerated": true
                },
                "graph_algorithms": {
                    "sssp": true,
                    "stress_majorization": true,
                    "spatial_hashing": true,
                    "constraint_solving": true
                },
                "visualization": {
                    "real_time_updates": true,
                    "dynamic_layout": true,
                    "focus_regions": true,
                    "multi_graph_support": true
                }
            },
            "performance": {
                "expected_speedup": "10-50x",
                "memory_efficiency": "High",
                "concurrent_tasks": true,
                "batch_processing": true
            }
        })
    } else {
        serde_json::json!({
            "success": true,
            "gpu_acceleration": false,
            "features": {
                "clustering": {
                    "available": true,
                    "methods": ["kmeans", "hierarchical", "dbscan"],
                    "gpu_accelerated": false,
                    "max_clusters": 20,
                    "max_nodes": 10000
                },
                "anomaly_detection": {
                    "available": true,
                    "methods": ["statistical"],
                    "real_time": false,
                    "gpu_accelerated": false
                },
                "graph_algorithms": {
                    "sssp": true,
                    "stress_majorization": false,
                    "spatial_hashing": false,
                    "constraint_solving": false
                },
                "visualization": {
                    "real_time_updates": false,
                    "dynamic_layout": false,
                    "focus_regions": true,
                    "multi_graph_support": true
                }
            },
            "performance": {
                "expected_speedup": "1x (CPU baseline)",
                "memory_efficiency": "Standard",
                "concurrent_tasks": false,
                "batch_processing": false
            }
        })
    };

    ok_json!(features)
}
