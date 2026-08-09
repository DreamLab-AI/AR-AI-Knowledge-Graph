use actix_web::{web, HttpResponse, Result};
use log::{info, warn};
use uuid::Uuid;

use crate::ok_json;
use crate::services::agent_visualization_protocol::McpServerType;
use crate::utils::mcp_tcp_client::create_mcp_client;

use super::state::ANOMALY_STATE;
use super::types::{Anomaly, AnomalyDetectionConfig, AnomalyResponse, AnomalyStats};

/// Agent-health anomaly heuristic. Scores live agent telemetry (CPU/memory/health/
/// success-rate) from the MCP server into `ANOMALY_STATE`. This is DISTINCT from
/// graph-structural anomaly detection (`POST /analytics/anomaly/detect`), which runs
/// GPU LOF / Z-score over graph topology and writes `node_analytics.anomaly`. The two
/// never share a store: agent-health lives in `ANOMALY_STATE`, structural in
/// `node_analytics`. Keep them namespaced so neither overwrites the other.
pub async fn toggle_anomaly_detection(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    request: web::Json<AnomalyDetectionConfig>,
) -> Result<HttpResponse> {
    info!("Toggling anomaly detection: enabled={}", request.enabled);

    let mut state = ANOMALY_STATE.lock().await;
    state.enabled = request.enabled;
    state.method = request.method.clone();
    state.sensitivity = request.sensitivity;
    state.window_size = request.window_size;
    state.update_interval = request.update_interval;

    if request.enabled {
        start_anomaly_detection().await;
    } else {
        state.anomalies.clear();
        state.stats = AnomalyStats::default();
    }

    ok_json!(AnomalyResponse {
        success: true,
        anomalies: None,
        stats: Some(state.stats.clone()),
        enabled: Some(state.enabled),
        method: Some(state.method.clone()),
        error: None,
    })
}

pub async fn get_current_anomalies() -> Result<HttpResponse> {
    let state = ANOMALY_STATE.lock().await;

    if !state.enabled {
        return ok_json!(AnomalyResponse {
            success: true,
            anomalies: Some(vec![]),
            stats: Some(AnomalyStats::default()),
            enabled: Some(false),
            method: None,
            error: None,
        });
    }

    ok_json!(AnomalyResponse {
        success: true,
        anomalies: Some(state.anomalies.clone()),
        stats: Some(state.stats.clone()),
        enabled: Some(state.enabled),
        method: Some(state.method.clone()),
        error: None,
    })
}

pub async fn get_anomaly_config() -> Result<HttpResponse> {
    let state = ANOMALY_STATE.lock().await;

    ok_json!(serde_json::json!({
        "success": true,
        "config": {
            "enabled": state.enabled,
            "method": state.method,
            "sensitivity": state.sensitivity,
            "window_size": state.window_size,
            "update_interval": state.update_interval
        },
        "stats": state.stats,
        // Agent-health heuristic dimensions actually evaluated by
        // start_anomaly_detection (CPU/memory/health/success-rate thresholds).
        // For graph-structural detection (LOF / Z-score) use /analytics/anomaly/detect.
        "supported_methods": [
            "high_cpu",
            "high_memory",
            "low_health",
            "low_success_rate"
        ]
    }))
}

pub(crate) async fn start_anomaly_detection() {
    tokio::spawn(async move {
        info!("Starting real anomaly detection using MCP agent data");

        let host = std::env::var("MCP_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = std::env::var("MCP_TCP_PORT")
            .unwrap_or_else(|_| "9500".to_string())
            .parse::<u16>()
            .unwrap_or(9500);

        let mcp_client = create_mcp_client(&McpServerType::ClaudeFlow, &host, port);

        let agents = match mcp_client.query_agent_list().await {
            Ok(agent_list) => {
                info!("Analyzing {} agents for anomalies", agent_list.len());
                agent_list
            }
            Err(e) => {
                warn!(
                    "Failed to get agents from MCP server for anomaly detection: {}",
                    e
                );
                Vec::new()
            }
        };

        let mut state = ANOMALY_STATE.lock().await;
        let mut detected_anomalies = Vec::new();

        for agent in &agents {
            if agent.performance.cpu_usage > 90.0 {
                detected_anomalies.push(Anomaly {
                    id: Uuid::new_v4().to_string(),
                    node_id: agent.agent_id.clone(),
                    r#type: "high_cpu".to_string(),
                    severity: if agent.performance.cpu_usage > 95.0 {
                        "critical"
                    } else {
                        "high"
                    }
                    .to_string(),
                    score: agent.performance.cpu_usage / 100.0,
                    description: format!(
                        "Agent {} has critically high CPU usage: {:.1}%",
                        agent.name, agent.performance.cpu_usage
                    ),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    metadata: Some(serde_json::json!({
                        "agent_name": agent.name,
                        "agent_type": agent.agent_type,
                        "cpu_usage": agent.performance.cpu_usage,
                        "memory_usage": agent.performance.memory_usage
                    })),
                });
            }

            if agent.performance.memory_usage > 85.0 {
                detected_anomalies.push(Anomaly {
                    id: Uuid::new_v4().to_string(),
                    node_id: agent.agent_id.clone(),
                    r#type: "high_memory".to_string(),
                    severity: if agent.performance.memory_usage > 95.0 {
                        "critical"
                    } else {
                        "medium"
                    }
                    .to_string(),
                    score: agent.performance.memory_usage / 100.0,
                    description: format!(
                        "Agent {} has high memory usage: {:.1}%",
                        agent.name, agent.performance.memory_usage
                    ),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    metadata: Some(serde_json::json!({
                        "agent_name": agent.name,
                        "memory_usage": agent.performance.memory_usage
                    })),
                });
            }

            if agent.performance.health_score < 50.0 {
                detected_anomalies.push(Anomaly {
                    id: Uuid::new_v4().to_string(),
                    node_id: agent.agent_id.clone(),
                    r#type: "low_health".to_string(),
                    severity: if agent.performance.health_score < 25.0 {
                        "critical"
                    } else {
                        "high"
                    }
                    .to_string(),
                    score: 1.0 - (agent.performance.health_score / 100.0),
                    description: format!(
                        "Agent {} has critically low health score: {:.1}",
                        agent.name, agent.performance.health_score
                    ),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    metadata: Some(serde_json::json!({
                        "agent_name": agent.name,
                        "health_score": agent.performance.health_score,
                        "error_count": agent.metadata.error_count
                    })),
                });
            }

            if agent.performance.success_rate < 70.0 && agent.performance.tasks_completed > 5 {
                detected_anomalies.push(Anomaly {
                    id: Uuid::new_v4().to_string(),
                    node_id: agent.agent_id.clone(),
                    r#type: "low_success_rate".to_string(),
                    severity: "medium".to_string(),
                    score: 1.0 - (agent.performance.success_rate / 100.0),
                    description: format!(
                        "Agent {} has low task success rate: {:.1}%",
                        agent.name, agent.performance.success_rate
                    ),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    metadata: Some(serde_json::json!({
                        "agent_name": agent.name,
                        "success_rate": agent.performance.success_rate,
                        "tasks_completed": agent.performance.tasks_completed,
                        "tasks_failed": agent.performance.tasks_failed
                    })),
                });
            }
        }

        state.anomalies = detected_anomalies;

        state.stats = AnomalyStats {
            total: state.anomalies.len() as u32,
            critical: state
                .anomalies
                .iter()
                .filter(|a| a.severity == "critical")
                .count() as u32,
            high: state
                .anomalies
                .iter()
                .filter(|a| a.severity == "high")
                .count() as u32,
            medium: state
                .anomalies
                .iter()
                .filter(|a| a.severity == "medium")
                .count() as u32,
            low: state
                .anomalies
                .iter()
                .filter(|a| a.severity == "low")
                .count() as u32,
            last_updated: Some(chrono::Utc::now().timestamp() as u64),
        };

        info!(
            "Detected {} real anomalies from agent data: {} critical, {} high, {} medium, {} low",
            state.stats.total,
            state.stats.critical,
            state.stats.high,
            state.stats.medium,
            state.stats.low
        );
    });
}
