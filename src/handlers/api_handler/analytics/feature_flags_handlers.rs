use actix_web::{web, HttpResponse, Result};
use log::info;

use crate::ok_json;

use super::state::FEATURE_FLAGS;
use super::types::FeatureFlags;

pub async fn get_feature_flags() -> Result<HttpResponse> {
    let flags = FEATURE_FLAGS.lock().await;

    ok_json!(serde_json::json!({
        "success": true,
        "flags": *flags,
        "description": {
            "sssp_integration": "Single-source shortest path integration toggle (display/contract field only — gates no behaviour; see /sssp/toggle and /sssp/status)",
            "ontology_validation": "Enable ontology validation and inference operations"
        }
    }))
}

pub async fn update_feature_flags(
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    request: web::Json<FeatureFlags>,
) -> Result<HttpResponse> {
    info!("Updating analytics feature flags");

    let mut flags = FEATURE_FLAGS.lock().await;
    *flags = request.into_inner();

    ok_json!(serde_json::json!({
        "success": true,
        "message": "Feature flags updated successfully",
        "flags": *flags
    }))
}
