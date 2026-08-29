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

use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::Deserialize;

use crate::adapters::sqlite_canary_repository::{CanaryRegistration, CanaryStoreError};
use crate::ok_json;
use crate::services::liveness_harness::{current_sha, LivenessHarness};

/// HTTP header carrying the service credential for the write routes (#2).
#[cfg_attr(any(debug_assertions, feature = "dev-auth"), allow(dead_code))]
const AGENT_KEY_HEADER: &str = "X-Agent-Key";

/// Whether a canary write request (`register` / `observe`) is authorised.
///
/// Release builds (no `debug_assertions`, no `--features dev-auth`) require
/// `X-Agent-Key` to equal `VISIONCLAW_AGENT_KEY`. If the env var is unset or
/// empty the route fails **closed** (every write rejected) — matching the
/// ADR-06 §D11 insecure-defaults posture used elsewhere (see
/// `socket_flow_handler::http_handler::is_insecure_defaults_allowed` and
/// `settings::auth_extractor`).
#[cfg(not(any(debug_assertions, feature = "dev-auth")))]
fn canary_write_authorised(req: &HttpRequest) -> bool {
    let expected = std::env::var("VISIONCLAW_AGENT_KEY").ok();
    let provided = req
        .headers()
        .get(AGENT_KEY_HEADER)
        .and_then(|v| v.to_str().ok());
    check_agent_key(expected.as_deref(), provided)
}

/// Pure credential check, split out so the fail-closed semantics are unit
/// testable without constructing an `HttpRequest` or depending on the build cfg.
///
/// Authorised **only** when a non-empty `VISIONCLAW_AGENT_KEY` is configured and
/// the request presents an exactly matching `X-Agent-Key`. An unset/empty key or
/// a missing/mismatched header both fail closed.
#[cfg_attr(any(debug_assertions, feature = "dev-auth"), allow(dead_code))]
fn check_agent_key(expected: Option<&str>, provided: Option<&str>) -> bool {
    match expected.filter(|s| !s.is_empty()) {
        // #3 (codex): compare in constant time so an attacker cannot recover the
        // key byte-by-byte from response-timing differences. Dependency-free
        // byte-wise fold (`subtle`/`constant_time_eq` are only transitive deps).
        Some(key) => match provided {
            Some(got) => constant_time_eq(key.as_bytes(), got.as_bytes()),
            None => false,
        },
        None => false,
    }
}

/// Constant-time byte-slice equality. The comparison time depends only on the
/// input lengths, never on how many leading bytes match — so it does not leak
/// the secret via timing. (Length inequality short-circuits, matching the
/// `constant_time_eq` crate; the credential's length is not itself sensitive.)
#[cfg_attr(any(debug_assertions, feature = "dev-auth"), allow(dead_code))]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Dev / `dev-auth` builds preserve the existing unauthenticated dev flow: the
/// Godot client posts `observe` without a credential in development. This bypass
/// only exists in builds compiled with `debug_assertions` or `--features
/// dev-auth`; the release counterpart above has no bypass codepath (fail closed).
#[cfg(any(debug_assertions, feature = "dev-auth"))]
fn canary_write_authorised(_req: &HttpRequest) -> bool {
    true
}

fn unauthorised_body() -> serde_json::Value {
    serde_json::json!({
        "error": "missing or invalid X-Agent-Key",
    })
}

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
    req: HttpRequest,
    harness: web::Data<LivenessHarness>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse> {
    if !canary_write_authorised(&req) {
        return Ok(HttpResponse::Unauthorized().json(unauthorised_body()));
    }
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
    req: HttpRequest,
    harness: web::Data<LivenessHarness>,
    path: web::Path<String>,
    body: web::Json<ObserveRequest>,
) -> Result<HttpResponse> {
    if !canary_write_authorised(&req) {
        return Ok(HttpResponse::Unauthorized().json(unauthorised_body()));
    }
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

#[cfg(test)]
mod auth_tests {
    use super::check_agent_key;

    #[test]
    fn missing_key_config_fails_closed() {
        // No VISIONCLAW_AGENT_KEY configured → reject regardless of header.
        assert!(!check_agent_key(None, Some("anything")));
        assert!(!check_agent_key(None, None));
    }

    #[test]
    fn empty_key_config_fails_closed() {
        assert!(!check_agent_key(Some(""), Some("")));
        assert!(!check_agent_key(Some(""), Some("x")));
    }

    #[test]
    fn matching_key_authorised() {
        assert!(check_agent_key(Some("s3cret"), Some("s3cret")));
    }

    #[test]
    fn mismatched_or_missing_header_rejected() {
        assert!(!check_agent_key(Some("s3cret"), Some("wrong")));
        assert!(!check_agent_key(Some("s3cret"), None));
    }
}
