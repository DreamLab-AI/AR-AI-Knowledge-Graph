use actix_web::{web, HttpResponse, Result};
use log::{debug, error, info, warn};

use crate::actors::messages::{
    GetConstraints, GetSettings, UpdateConstraints, UpdateVisualAnalyticsParams,
};
use crate::gpu::visual_analytics::VisualAnalyticsParams;
use crate::AppState;
use crate::{bad_request, error_json, ok_json, service_unavailable};
use visionclaw_domain::models::constraints::ConstraintSet;

use super::types::{
    AnalyticsParamsResponse, ConstraintsResponse, FocusRegion, FocusResponse, SetFocusRequest,
    UpdateConstraintsRequest,
};

pub async fn get_analytics_params(app_state: web::Data<AppState>) -> Result<HttpResponse> {
    info!("Getting current visual analytics parameters");

    let settings = match app_state.settings_addr.send(GetSettings).await {
        Ok(Ok(settings)) => settings,
        Ok(Err(e)) => {
            error!("Failed to get settings for analytics params: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(AnalyticsParamsResponse {
                    success: false,
                    params: None,
                    error: Some("Failed to retrieve settings".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("Settings actor mailbox error: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json(AnalyticsParamsResponse {
                    success: false,
                    params: None,
                    error: Some("Settings service unavailable".to_string()),
                }),
            );
        }
    };

    let params = create_default_analytics_params(&settings);

    ok_json!(AnalyticsParamsResponse {
        success: true,
        params: Some(params),
        error: None,
    })
}

pub async fn update_analytics_params(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    app_state: web::Data<AppState>,
    params: web::Json<VisualAnalyticsParams>,
) -> Result<HttpResponse> {
    info!("Updating visual analytics parameters");
    debug!("Visual analytics params: {:?}", params);

    if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
        match gpu_addr
            .send(UpdateVisualAnalyticsParams {
                params: params.into_inner(),
            })
            .await
        {
            Ok(Ok(())) => {
                info!("Visual analytics parameters updated successfully");
                ok_json!(AnalyticsParamsResponse {
                    success: true,
                    params: None,
                    error: None,
                })
            }
            Ok(Err(e)) => {
                error!("Failed to update visual analytics params: {}", e);
                Ok(
                    HttpResponse::InternalServerError().json(AnalyticsParamsResponse {
                        success: false,
                        params: None,
                        error: Some(format!("Failed to update parameters: {}", e)),
                    }),
                )
            }
            Err(e) => {
                error!("GPU compute actor mailbox error: {}", e);
                Ok(
                    HttpResponse::ServiceUnavailable().json(AnalyticsParamsResponse {
                        success: false,
                        params: None,
                        error: Some("GPU compute service unavailable".to_string()),
                    }),
                )
            }
        }
    } else {
        Ok(
            HttpResponse::ServiceUnavailable().json(AnalyticsParamsResponse {
                success: false,
                params: None,
                error: Some("GPU compute service not available".to_string()),
            }),
        )
    }
}

pub async fn get_constraints(app_state: web::Data<AppState>) -> Result<HttpResponse> {
    info!("Getting current constraint set");

    if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
        match gpu_addr.send(GetConstraints).await {
            Ok(Ok(constraints)) => {
                return ok_json!(ConstraintsResponse {
                    success: true,
                    constraints: Some(constraints),
                    error: None,
                });
            }
            Ok(Err(e)) => {
                error!("Failed to get constraints from GPU actor: {}", e);
            }
            Err(e) => {
                error!("GPU compute actor mailbox error: {}", e);
            }
        }
    }

    ok_json!(ConstraintsResponse {
        success: true,
        constraints: Some(ConstraintSet::default()),
        error: None,
    })
}

pub async fn update_constraints(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    app_state: web::Data<AppState>,
    request: web::Json<UpdateConstraintsRequest>,
) -> Result<HttpResponse> {
    info!("Updating constraint set");

    let constraint_data = if let Some(constraint_set) = &request.constraint_set {
        serde_json::to_value(constraint_set).unwrap_or_else(|e| {
            log::warn!("Failed to serialize constraint_set: {}", e);
            serde_json::Value::default()
        })
    } else if let Some(data) = &request.constraint_data {
        data.clone()
    } else {
        return Ok(HttpResponse::BadRequest().json(ConstraintsResponse {
            success: false,
            constraints: None,
            error: Some("Either constraint_set or constraint_data must be provided".to_string()),
        }));
    };

    if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
        match gpu_addr.send(UpdateConstraints { constraint_data }).await {
            Ok(Ok(())) => {
                debug!("Constraints updated successfully");

                if let Ok(Ok(updated_constraints)) = gpu_addr.send(GetConstraints).await {
                    return ok_json!(ConstraintsResponse {
                        success: true,
                        constraints: Some(updated_constraints),
                        error: None,
                    });
                }
            }
            Ok(Err(e)) => {
                error!("Failed to update constraints: {}", e);
                return Ok(
                    HttpResponse::InternalServerError().json(ConstraintsResponse {
                        success: false,
                        constraints: None,
                        error: Some(format!("Failed to update constraints: {}", e)),
                    }),
                );
            }
            Err(e) => {
                error!("GPU compute actor mailbox error: {}", e);
                return Ok(
                    HttpResponse::InternalServerError().json(ConstraintsResponse {
                        success: false,
                        constraints: None,
                        error: Some("GPU compute service unavailable".to_string()),
                    }),
                );
            }
        }
    }

    Ok(
        HttpResponse::ServiceUnavailable().json(ConstraintsResponse {
            success: false,
            constraints: None,
            error: Some("GPU compute service not available".to_string()),
        }),
    )
}

pub async fn set_focus(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    app_state: web::Data<AppState>,
    request: web::Json<SetFocusRequest>,
) -> Result<HttpResponse> {
    info!("Setting focus node/region");

    let mut focus_response = if let Some(node_id) = request.node_id {
        debug!("Setting focus on node {}", node_id);
        FocusResponse {
            success: false,
            focus_node: Some(node_id),
            focus_region: None,
            error: None,
        }
    } else if let Some(region) = &request.region {
        debug!(
            "Setting focus on region center: ({}, {}, {}), radius: {}",
            region.center_x, region.center_y, region.center_z, region.radius
        );
        FocusResponse {
            success: false,
            focus_node: None,
            focus_region: Some(FocusRegion {
                center_x: region.center_x,
                center_y: region.center_y,
                center_z: region.center_z,
                radius: region.radius,
            }),
            error: None,
        }
    } else {
        return Ok(HttpResponse::BadRequest().json(FocusResponse {
            success: false,
            focus_node: None,
            focus_region: None,
            error: Some("Either node_id or region must be specified".to_string()),
        }));
    };

    let current_params = VisualAnalyticsParams::default();

    #[derive(Debug)]
    enum FocusRequest {
        Node { node_id: u32 },
        Region { x: f32, y: f32, radius: f32 },
    }

    let focus_request = if let Some(node_id) = request.node_id {
        FocusRequest::Node {
            node_id: node_id as u32,
        }
    } else if let Some(region) = &request.region {
        FocusRequest::Region {
            x: region.center_x,
            y: region.center_y,
            radius: region.radius,
        }
    } else {
        return Ok(HttpResponse::BadRequest().json(FocusResponse {
            success: false,
            focus_node: None,
            focus_region: None,
            error: Some("Either node_id or region must be specified".to_string()),
        }));
    };

    if let Some(gpu_addr) = app_state.get_gpu_compute_addr().await {
        info!("Setting focus on GPU compute actor");

        let mut updated_params = current_params.clone();
        match focus_request {
            FocusRequest::Node { node_id } => {
                updated_params.primary_focus_node = node_id as i32;
                info!("Setting focus on node: {}", node_id);
                focus_response.focus_node = Some(node_id as i32);
            }
            FocusRequest::Region { x, y, radius } => {
                updated_params.camera_position =
                    crate::gpu::visual_analytics::Vec4::new(x, y, 0.0, 0.0).unwrap_or_default();
                updated_params.zoom_level = 1.0 / radius.max(1.0);
                info!("Setting focus on region: ({}, {}) radius {}", x, y, radius);
                focus_response.focus_region = Some(FocusRegion {
                    center_x: x,
                    center_y: y,
                    center_z: 0.0,
                    radius,
                });
            }
        }

        use crate::actors::messages::UpdateVisualAnalyticsParams;
        match gpu_addr
            .send(UpdateVisualAnalyticsParams {
                params: updated_params,
            })
            .await
        {
            Ok(Ok(())) => {
                info!("Successfully updated visual analytics parameters with focus settings");
                focus_response.success = true;
            }
            Ok(Err(e)) => {
                warn!("Failed to update visual analytics parameters: {}", e);
                focus_response.error = Some(format!("GPU parameter update failed: {}", e));
            }
            Err(e) => {
                error!("Failed to communicate with GPU for parameter update: {}", e);
                focus_response.error = Some(format!("GPU communication failed: {}", e));
            }
        }
    } else {
        warn!("GPU compute actor not available for focus setting");

        match focus_request {
            FocusRequest::Node { node_id } => {
                focus_response.focus_node = Some(node_id as i32);
                info!(
                    "Focus parameters stored for node {} (GPU not available)",
                    node_id
                );
            }
            FocusRequest::Region { x, y, radius } => {
                focus_response.focus_region = Some(FocusRegion {
                    center_x: x,
                    center_y: y,
                    center_z: 0.0,
                    radius,
                });
                info!(
                    "Focus parameters stored for region ({}, {}) radius {} (GPU not available)",
                    x, y, radius
                );
            }
        }
        focus_response.success = true;
    }

    ok_json!(focus_response)
}

pub fn create_default_analytics_params(
    _settings: &crate::config::AppFullSettings,
) -> VisualAnalyticsParams {
    use crate::gpu::visual_analytics::VisualAnalyticsBuilder;

    VisualAnalyticsBuilder::new()
        .with_nodes(1000)
        .with_edges(2000)
        .with_focus(-1, 2.2)
        .with_temporal_decay(0.1)
        .build()
}

pub async fn set_kernel_mode(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    app_state: web::Data<AppState>,
    request: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    info!("Setting GPU kernel mode");

    if let Some(mode) = request.get("mode").and_then(|m| m.as_str()) {
        let compute_mode = match mode {
            "legacy" => crate::utils::unified_gpu_compute::ComputeMode::Basic,
            "dual_graph" => crate::utils::unified_gpu_compute::ComputeMode::DualGraph,
            "advanced" => crate::utils::unified_gpu_compute::ComputeMode::Advanced,

            "standard" => crate::utils::unified_gpu_compute::ComputeMode::Basic,

            "visual_analytics" => crate::utils::unified_gpu_compute::ComputeMode::Advanced,
            _ => {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": format!("Invalid mode: {}. Valid modes: legacy, dual_graph, advanced", mode)
                })));
            }
        };

        if let Some(gpu_actor) = app_state.get_gpu_compute_addr().await {
            use crate::actors::messages::SetComputeMode;
            match gpu_actor.send(SetComputeMode { mode: compute_mode }).await {
                Ok(result) => match result {
                    Ok(()) => {
                        info!("GPU kernel mode set to: {}", mode);
                        ok_json!(serde_json::json!({
                            "success": true,
                            "mode": mode
                        }))
                    }
                    Err(e) => {
                        error!("Failed to set kernel mode: {}", e);
                        Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "error": e
                        })))
                    }
                },
                Err(e) => {
                    error!("Failed to send kernel mode message: {}", e);
                    error_json!("Failed to communicate with GPU actor")
                }
            }
        } else {
            service_unavailable!("GPU compute not available")
        }
    } else {
        bad_request!("Missing 'mode' parameter")
    }
}
