use actix_web::{web, HttpResponse, Result};
use log::error;

use crate::actors::messages::{
    ConfigureStressMajorization, GetStressMajorizationConfig, GetStressMajorizationStats,
    ResetStressMajorizationSafety, TriggerStressMajorization, UpdateStressMajorizationParams,
};
use crate::AppState;
use crate::{error_json, ok_json, service_unavailable};
use visionclaw_domain::models::constraints::AdvancedParams;

pub async fn trigger_stress_majorization(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(gpu_actor) = data.get_gpu_compute_addr().await {
        match gpu_actor.send(TriggerStressMajorization).await {
            Ok(Ok(())) => ok_json!(serde_json::json!({
                "success": true,
                "message": "Stress majorization triggered successfully"
            })),
            Ok(Err(e)) => {
                error!("Stress majorization failed: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e
                })))
            }
            Err(e) => {
                error!("Failed to communicate with GPU actor: {}", e);
                error_json!("Internal server error")
            }
        }
    } else {
        service_unavailable!("GPU compute actor not available")
    }
}

pub async fn get_stress_majorization_stats(
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(gpu_actor) = data.get_gpu_compute_addr().await {
        match gpu_actor.send(GetStressMajorizationStats).await {
            Ok(Ok(stats)) => ok_json!(serde_json::json!({
                "success": true,
                "stats": stats
            })),
            Ok(Err(e)) => {
                error!("Failed to get stress majorization stats: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e
                })))
            }
            Err(e) => {
                error!("Failed to get stress majorization stats: {}", e);
                error_json!("Failed to retrieve statistics")
            }
        }
    } else {
        service_unavailable!("GPU compute actor not available")
    }
}

pub async fn reset_stress_majorization_safety(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(gpu_actor) = data.get_gpu_compute_addr().await {
        match gpu_actor.send(ResetStressMajorizationSafety).await {
            Ok(Ok(())) => ok_json!(serde_json::json!({
                "success": true,
                "message": "Stress majorization safety state reset successfully"
            })),
            Ok(Err(e)) => {
                error!("Failed to reset stress majorization safety: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e
                })))
            }
            Err(e) => {
                error!("Failed to communicate with GPU actor: {}", e);
                error_json!("Internal server error")
            }
        }
    } else {
        service_unavailable!("GPU compute actor not available")
    }
}

pub async fn update_stress_majorization_params(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    params: web::Json<AdvancedParams>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(gpu_actor) = data.get_gpu_compute_addr().await {
        let msg = UpdateStressMajorizationParams {
            params: params.into_inner(),
        };

        match gpu_actor.send(msg).await {
            Ok(Ok(())) => ok_json!(serde_json::json!({
                "success": true,
                "message": "Stress majorization parameters updated successfully"
            })),
            Ok(Err(e)) => {
                error!("Failed to update stress majorization parameters: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e
                })))
            }
            Err(e) => {
                error!("Failed to communicate with GPU actor: {}", e);
                error_json!("Internal server error")
            }
        }
    } else {
        service_unavailable!("GPU compute actor not available")
    }
}

/// P1-1: Configure Stress Majorization runtime parameters
pub async fn configure_stress_majorization(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    config: web::Json<ConfigureStressMajorization>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(gpu_actor) = data.get_gpu_compute_addr().await {
        match gpu_actor.send(config.into_inner()).await {
            Ok(Ok(())) => ok_json!(serde_json::json!({
                "success": true,
                "message": "Stress majorization configuration updated successfully"
            })),
            Ok(Err(e)) => {
                error!("Failed to configure stress majorization: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e
                })))
            }
            Err(e) => {
                error!("Failed to communicate with GPU actor: {}", e);
                error_json!("Internal server error")
            }
        }
    } else {
        service_unavailable!("GPU compute actor not available")
    }
}

/// P1-1: Get current Stress Majorization configuration
pub async fn get_stress_majorization_config(
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(gpu_actor) = data.get_gpu_compute_addr().await {
        match gpu_actor.send(GetStressMajorizationConfig).await {
            Ok(Ok(config)) => ok_json!(serde_json::json!({
                "success": true,
                "config": config
            })),
            Ok(Err(e)) => {
                error!("Failed to get stress majorization config: {}", e);
                Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e
                })))
            }
            Err(e) => {
                error!("Failed to communicate with GPU actor: {}", e);
                error_json!("Internal server error")
            }
        }
    } else {
        service_unavailable!("GPU compute actor not available")
    }
}
