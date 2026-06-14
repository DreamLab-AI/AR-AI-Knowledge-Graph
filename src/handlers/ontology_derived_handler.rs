//! Fenced derived-ontology write surface (WS-9).
//!
//! `POST /api/ontology/derived` accepts a list of quads `{graph, s, p, o}` and
//! writes them into the derived named graphs **`urn:ngm:graph:ontology:summary`**
//! and **`urn:ngm:graph:ontology:observed`** only. Any quad targeting
//! `:assert` or `:inferred` is rejected with HTTP 400 — the **fence**.
//!
//! Defence-in-depth: the fence is enforced at BOTH layers —
//!   1. here, an early 400 before any Oxigraph call, and
//!   2. in [`OxigraphOntologyRepository::append_derived_quads`], which re-checks
//!      `DERIVED_FENCE` so a future direct caller of the repo method cannot
//!      bypass the handler.
//!
//! Scope is `/ontology/derived` (NOT `/ontology`), so it does not collide with
//! the single `/ontology` scope in `ontology_handler::config`. It is mounted at
//! the `/api` level via `main.rs`, and gated `RequireAuth::power_user()` —
//! mirroring the privileged ontology scope (`ontology_handler.rs:917-918`).

use actix_web::{web, HttpResponse};
use log::warn;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::RequireAuth;
use crate::settings::auth_extractor::AuthenticatedUser;
use crate::AppState;

/// Forbidden graphs — mirrored locally for the early 400. Must match
/// `DERIVED_FENCE` in `oxigraph_ontology_repository.rs`.
const FORBIDDEN_GRAPHS: [&str; 2] = [
    "urn:ngm:graph:ontology:assert",
    "urn:ngm:graph:ontology:inferred",
];

/// The only writable derived graphs.
const ALLOWED_GRAPHS: [&str; 2] = [
    "urn:ngm:graph:ontology:summary",
    "urn:ngm:graph:ontology:observed",
];

#[derive(Debug, Deserialize)]
pub struct DerivedQuad {
    pub graph: String,
    pub s: String,
    pub p: String,
    pub o: String,
}

#[derive(Debug, Deserialize)]
pub struct DerivedWriteRequest {
    pub quads: Vec<DerivedQuad>,
}

/// `POST /api/ontology/derived` — fenced quad write to `:summary`/`:observed`.
async fn write_derived(
    _auth: AuthenticatedUser,
    state: web::Data<AppState>,
    body: web::Json<DerivedWriteRequest>,
) -> HttpResponse {
    let body = body.into_inner();

    if body.quads.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "error": "empty quads",
            "detail": "derived write requires at least one quad",
        }));
    }

    // Layer-1 fence: reject before any store interaction.
    for q in &body.quads {
        if FORBIDDEN_GRAPHS.contains(&q.graph.as_str()) {
            warn!(
                "[ontology-derived] rejected fenced graph write: {}",
                q.graph
            );
            return HttpResponse::BadRequest().json(json!({
                "error": "fenced graph",
                "graph": q.graph,
                "detail": "derived writes may not target :assert or :inferred",
            }));
        }
        if !ALLOWED_GRAPHS.contains(&q.graph.as_str()) {
            return HttpResponse::BadRequest().json(json!({
                "error": "unsupported graph",
                "graph": q.graph,
                "detail": "derived writes only to :summary/:observed",
            }));
        }
    }

    let quads: Vec<(String, String, String, String)> = body
        .quads
        .into_iter()
        .map(|q| (q.graph, q.s, q.p, q.o))
        .collect();

    // Layer-2 fence + injection guards live inside append_derived_quads.
    match state.ontology_repository.append_derived_quads(quads).await {
        Ok(n) => HttpResponse::Ok().json(json!({ "success": true, "written": n })),
        Err(e) => {
            warn!("[ontology-derived] write failed: {e}");
            HttpResponse::BadRequest().json(json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// `POST /api/ontology/derived/regenerate` — clear + re-mark `:summary` so the
/// endpoint is non-lossy and idempotent. Clears both derived graphs then writes
/// a provenance marker triple recording the regeneration. A full re-derivation
/// from a SELECT over `:assert`/`:inferred` is the WS-0 LIMIT-clamp follow-on;
/// it is deliberately NOT run unbounded here.
async fn regenerate(_auth: AuthenticatedUser, state: web::Data<AppState>) -> HttpResponse {
    if let Err(e) = state.ontology_repository.clear_derived_graph("summary").await {
        warn!("[ontology-derived] clear summary failed: {e}");
        return HttpResponse::InternalServerError()
            .json(json!({ "success": false, "error": e.to_string() }));
    }
    if let Err(e) = state.ontology_repository.clear_derived_graph("observed").await {
        warn!("[ontology-derived] clear observed failed: {e}");
        return HttpResponse::InternalServerError()
            .json(json!({ "success": false, "error": e.to_string() }));
    }

    // Provenance marker so the regeneration is observable + idempotent.
    let marker_subject = "urn:ngm:derived:regeneration".to_string();
    let activity = format!(
        "urn:visionclaw:execution:regenerate-derived-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let marker = vec![(
        marker_subject,
        "https://narrativegoldmine.com/ns/v1#regeneratedAt".to_string(),
        chrono::Utc::now().to_rfc3339(),
    )];

    match state
        .ontology_repository
        .append_derived_summary(None, &activity, marker)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(json!({
            "success": true,
            "cleared": ["summary", "observed"],
            "activity_urn": activity,
        })),
        Err(e) => {
            warn!("[ontology-derived] regenerate marker failed: {e}");
            HttpResponse::InternalServerError()
                .json(json!({ "success": false, "error": e.to_string() }))
        }
    }
}

/// `GET /api/ontology/derived/{which}` — read a derived graph (summary|observed).
async fn read_derived(
    _auth: AuthenticatedUser,
    state: web::Data<AppState>,
    which: web::Path<String>,
) -> HttpResponse {
    match state.ontology_repository.read_derived_graph(&which).await {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => HttpResponse::BadRequest().json(json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

/// Mounts `/ontology/derived` under the caller's scope (registered at `/api`).
/// `power_user`-gated — same posture as the privileged ontology mutators.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/ontology/derived")
            .wrap(RequireAuth::power_user())
            .route("", web::post().to(write_derived))
            .route("/regenerate", web::post().to(regenerate))
            .route("/{which}", web::get().to(read_derived)),
    );
}
