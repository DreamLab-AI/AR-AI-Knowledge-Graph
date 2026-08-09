// src/handlers/liveness_harness_handler.rs
//! HTTP surface for the [`LivenessHarness`](crate::services::liveness_harness)
//! (RES-a, ADR-130 Decision 3).
//!
//! Three routes, mounted under `/api`:
//!   * `POST /api/canary/register` — declare a canary (any repository).
//!   * `POST /api/canary/observe/{canary_id}` — record a fire from observed
//!     live traffic that reaches this service over HTTP.
//!   * `GET  /api/canary/status` — per-canary armed/fired state + the
//!     `kg_backend_up` gauge.
//!
//! The harness itself decides whether a fire is fresh (the SHA/30-day staleness
//! rule); this handler is a thin JSON adapter over it.

use actix_web::{web, HttpResponse, Result};
use serde::Deserialize;

use crate::adapters::sqlite_canary_repository::{CanaryRegistration, CanaryStoreError};
use crate::ok_json;
use crate::services::liveness_harness::{current_sha, LivenessHarness};

/// `POST /api/canary/register` body. `canary_id`, `description` and `kind` are
/// required; `owner_repo`, `wave` and `sha_at_registration` are optional so a
/// foreign repository can attribute its own registration.
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub canary_id: String,
    pub description: String,
    /// `standing` | `one-shot` (normalised).
    pub kind: String,
    #[serde(default)]
    pub owner_repo: Option<String>,
    #[serde(default)]
    pub wave: Option<String>,
    #[serde(default)]
    pub sha_at_registration: Option<String>,
}

/// `POST /api/canary/observe/{canary_id}` body — the free-form evidence string
/// describing the live traffic observed.
#[derive(Deserialize)]
pub struct ObserveRequest {
    pub evidence: String,
}

/// Normalise the kind to the two canonical values; anything unrecognised
/// defaults to `standing` (the safer, keeps-monitoring choice).
fn normalise_kind(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "one-shot" | "oneshot" => "one-shot",
        _ => "standing",
    }
}

fn store_error_body(e: &CanaryStoreError) -> serde_json::Value {
    serde_json::json!({ "error": e.to_string() })
}

/// `POST /api/canary/register`
pub async fn register(
    harness: web::Data<LivenessHarness>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse> {
    let reg = CanaryRegistration {
        canary_id: body.canary_id.clone(),
        description: body.description.clone(),
        kind: normalise_kind(&body.kind).to_string(),
        owner_repo: body
            .owner_repo
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        wave: body.wave.clone(),
        sha_at_registration: body
            .sha_at_registration
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_sha),
        registered_at_ms: chrono::Utc::now().timestamp_millis(),
    };

    match harness.register(&reg).await {
        Ok(()) => ok_json!(serde_json::json!({
            "registered": true,
            "canary_id": reg.canary_id,
            "kind": reg.kind,
            "owner_repo": reg.owner_repo,
            "sha_at_registration": reg.sha_at_registration,
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(store_error_body(&e))),
    }
}

/// `POST /api/canary/observe/{canary_id}`
pub async fn observe(
    harness: web::Data<LivenessHarness>,
    path: web::Path<String>,
    body: web::Json<ObserveRequest>,
) -> Result<HttpResponse> {
    let canary_id = path.into_inner();
    match harness.observe(&canary_id, &body.evidence).await {
        Ok(fire_id) => ok_json!(serde_json::json!({
            "fired": true,
            "canary_id": canary_id,
            "fire_id": fire_id,
            "sha": current_sha(),
        })),
        Err(CanaryStoreError::NotFound(_)) => {
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "fired": false,
                "error": format!("unknown canary: {canary_id}"),
            })))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(store_error_body(&e))),
    }
}

/// `GET /api/canary/status`
pub async fn status(harness: web::Data<LivenessHarness>) -> Result<HttpResponse> {
    match harness.status().await {
        Ok(canaries) => ok_json!(serde_json::json!({
            "kg_backend_up": harness.kg_backend_up(),
            "sha": current_sha(),
            "canaries": canaries,
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(store_error_body(&e))),
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/canary")
            .route("/register", web::post().to(register))
            .route("/observe/{canary_id}", web::post().to(observe))
            .route("/status", web::get().to(status)),
    );
}
