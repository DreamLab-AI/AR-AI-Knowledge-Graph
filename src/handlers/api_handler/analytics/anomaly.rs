use chrono::Utc;
use log::{debug, error, info};
use std::collections::HashMap;
use uuid::Uuid;

use super::{Anomaly, AnomalyStats, ANOMALY_STATE};
use crate::actors::messages::{AnomalyMethod, AnomalyParams, RunAnomalyDetection};
use crate::utils::result_helpers::safe_json_number;
use crate::AppState;

/// Request body for `POST /analytics/anomaly/detect` — runs GPU graph-structural
/// anomaly detection (LOF / Z-score) and writes node_analytics.anomaly via the
/// single-writer AnomalyDetectionActor. Distinct from the agent-health heuristic
/// at `/anomaly/toggle`, which scores agent telemetry, not graph topology.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyDetectRequest {
    pub method: String,
    pub k_neighbors: Option<i32>,
    pub radius: Option<f32>,
    pub feature_data: Option<Vec<f32>>,
    pub threshold: Option<f32>,
}

pub async fn run_gpu_anomaly_detection(
    app_state: &actix_web::web::Data<AppState>,
    method: &str,
    k_neighbors: Option<i32>,
    radius: Option<f32>,
    feature_data: Option<Vec<f32>>,
    threshold: Option<f32>,
) -> Result<Vec<Anomaly>, String> {
    info!("Running GPU anomaly detection with method: {}", method);

    // Route through GPUManagerActor → AnalyticsSupervisor → AnomalyDetectionActor
    // (not ForceComputeActor which only stubs anomaly detection)
    let gpu_addr = app_state
        .gpu_manager_addr
        .as_ref()
        .ok_or_else(|| "GPU manager actor not available".to_string())?;

    let anomaly_method = match method {
        "lof" | "local_outlier_factor" => AnomalyMethod::LocalOutlierFactor,
        "zscore" | "z_score" => AnomalyMethod::ZScore,
        _ => return Err(format!("Unsupported anomaly detection method: {}", method)),
    };

    let params = AnomalyParams {
        method: anomaly_method.clone(),
        k_neighbors: k_neighbors.unwrap_or(20),
        radius: radius.unwrap_or(1.0),
        feature_data: feature_data.clone(),
        threshold: threshold.unwrap_or(match anomaly_method {
            AnomalyMethod::LocalOutlierFactor => 1.5,
            AnomalyMethod::ZScore => 2.0,
        }),
    };

    validate_anomaly_params(&params)?;

    let msg = RunAnomalyDetection { params };

    match gpu_addr.send(msg).await {
        Ok(Ok(result)) => {
            info!(
                "GPU anomaly detection completed: {} anomalies found",
                result.num_anomalies
            );
            // ADR-031 D3/D4 single-writer: node_analytics.anomaly is populated
            // exclusively by AnomalyDetectionActor (masked graph-node-id key, stale
            // reset) while it runs the GPU kernels. The previous write here keyed by
            // the raw enumerate index (unmasked GPU buffer position) and so never
            // matched the V3 encoder's masked lookup — a silent no-op, now removed.
            Ok(convert_gpu_anomaly_result_to_anomalies(
                result,
                &anomaly_method,
            ))
        }
        Ok(Err(e)) => {
            error!("GPU anomaly detection failed: {}", e);
            Err(e)
        }
        Err(e) => {
            error!("GPU actor mailbox error: {}", e);
            Err(format!("Failed to communicate with GPU actor: {}", e))
        }
    }
}

pub async fn cleanup_old_anomalies() {
    let mut state = ANOMALY_STATE.lock().await;
    let current_time = Utc::now().timestamp() as u64;
    let retention_period = 3600;

    let initial_count = state.anomalies.len();
    state
        .anomalies
        .retain(|anomaly| current_time - anomaly.timestamp < retention_period);
    let removed_count = initial_count - state.anomalies.len();

    if removed_count > 0 {
        debug!("Cleaned up {} old anomalies", removed_count);

        let mut new_stats = AnomalyStats::default();
        for anomaly in &state.anomalies {
            match anomaly.severity.as_str() {
                "critical" => new_stats.critical += 1,
                "high" => new_stats.high += 1,
                "medium" => new_stats.medium += 1,
                "low" => new_stats.low += 1,
                _ => {}
            }
            new_stats.total += 1;
        }
        new_stats.last_updated = state.stats.last_updated;
        state.stats = new_stats;
    }
}

fn validate_anomaly_params(params: &AnomalyParams) -> Result<(), String> {
    match params.method {
        AnomalyMethod::LocalOutlierFactor => {
            if params.k_neighbors < 1 || params.k_neighbors > 1000 {
                return Err("k_neighbors must be between 1 and 1000".to_string());
            }
            if params.radius <= 0.0 || params.radius > 1000.0 {
                return Err("radius must be between 0.0 and 1000.0".to_string());
            }
            if params.threshold <= 0.0 || params.threshold > 10.0 {
                return Err("LOF threshold must be between 0.0 and 10.0".to_string());
            }
        }
        AnomalyMethod::ZScore => {
            if params.feature_data.is_none() {
                return Err("feature_data is required for Z-score anomaly detection".to_string());
            }
            if let Some(ref features) = params.feature_data {
                if features.is_empty() {
                    return Err("feature_data cannot be empty".to_string());
                }
            }
            if params.threshold <= 0.0 || params.threshold > 10.0 {
                return Err("Z-score threshold must be between 0.0 and 10.0".to_string());
            }
        }
    }
    Ok(())
}

fn convert_gpu_anomaly_result_to_anomalies(
    result: crate::actors::messages::AnomalyResult,
    method: &AnomalyMethod,
) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();
    let current_time = Utc::now().timestamp() as u64;

    match method {
        AnomalyMethod::LocalOutlierFactor => {
            if let Some(lof_scores) = result.lof_scores {
                for (i, &score) in lof_scores.iter().enumerate() {
                    if score > result.anomaly_threshold {
                        let severity = determine_severity_from_lof_score(score);
                        let mut metadata = HashMap::new();
                        metadata.insert(
                            "lof_score".to_string(),
                            serde_json::Value::Number(safe_json_number(score as f64)),
                        );

                        if let Some(ref densities) = result.local_densities {
                            if i < densities.len() {
                                metadata.insert(
                                    "local_density".to_string(),
                                    serde_json::Value::Number(safe_json_number(
                                        densities[i] as f64,
                                    )),
                                );
                            }
                        }

                        anomalies.push(Anomaly {
                            id: Uuid::new_v4().to_string(),
                            node_id: format!("node_{}", i),
                            r#type: "local_outlier".to_string(),
                            severity: severity.to_string(),
                            score,
                            description: format!(
                                "Node has abnormally {} local density (LOF score: {:.3})",
                                severity, score
                            ),
                            timestamp: current_time,
                            metadata: Some(serde_json::Value::Object(
                                metadata.into_iter().collect(),
                            )),
                        });
                    }
                }
            }
        }
        AnomalyMethod::ZScore => {
            if let Some(zscore_values) = result.zscore_values {
                for (i, &score) in zscore_values.iter().enumerate() {
                    let abs_score = score.abs();
                    if abs_score > result.anomaly_threshold {
                        let severity = determine_severity_from_zscore(abs_score);
                        let mut metadata = HashMap::new();
                        metadata.insert(
                            "zscore".to_string(),
                            serde_json::Value::Number(safe_json_number(score as f64)),
                        );
                        metadata.insert(
                            "abs_zscore".to_string(),
                            serde_json::Value::Number(safe_json_number(abs_score as f64)),
                        );

                        anomalies.push(Anomaly {
                            id: Uuid::new_v4().to_string(),
                            node_id: format!("node_{}", i),
                            r#type: "statistical_outlier".to_string(),
                            severity: severity.to_string(),
                            score: abs_score,
                            description: format!(
                                "Statistical outlier with {} significance (Z-score: {:.3})",
                                severity, score
                            ),
                            timestamp: current_time,
                            metadata: Some(serde_json::Value::Object(
                                metadata.into_iter().collect(),
                            )),
                        });
                    }
                }
            }
        }
    }

    anomalies
}

fn determine_severity_from_lof_score(score: f32) -> &'static str {
    if score > 3.0 {
        "critical"
    } else if score > 2.5 {
        "high"
    } else if score > 2.0 {
        "medium"
    } else {
        "low"
    }
}

fn determine_severity_from_zscore(abs_score: f32) -> &'static str {
    if abs_score > 4.0 {
        "critical"
    } else if abs_score > 3.0 {
        "high"
    } else if abs_score > 2.5 {
        "medium"
    } else {
        "low"
    }
}
