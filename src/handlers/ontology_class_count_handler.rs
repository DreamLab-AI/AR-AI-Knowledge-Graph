// src/handlers/ontology_class_count_handler.rs
//! Script-queryable ontology class-count source (RES-d, PRD-023 WP-12).
//!
//! `GET /api/ontology/class-count` returns the live count of OWL classes held in
//! Oxigraph, the source the canon `DriftCounter` consumes to detect ontology
//! drift. The count is authoritative — it is read straight from the Oxigraph
//! ontology named graph, not a cached figure.
//!
//! ## Method (documented so a consumer can reproduce it)
//!
//! The count is [`OxigraphOntologyRepository::get_metrics`]`.class_count`, which
//! runs the SPARQL aggregate
//!
//! ```sparql
//! SELECT (COUNT(?s) AS ?n)
//! WHERE { GRAPH <urn:ngm:graph:ontology> { ?s a vc:OntologyClass } }
//! ```
//!
//! against the live store. A successful read fires `CANARY-VC-RESD-COUNT` (the
//! count source is live), observed as the live traffic of the `DriftCounter`
//! query itself.
//!
//! Scope is `/ontology/class-count` (a distinct path from the single `/ontology`
//! scope in `ontology_handler::config`), mounted at the `/api` level. Read-only
//! and unauthenticated by design — a class count is a public liveness figure the
//! canon polls, carrying no ontology content.

use actix_web::{web, HttpResponse, Result};
use visionclaw_domain::ports::ontology_repository::OntologyRepository;

use crate::ok_json;
use crate::services::liveness_harness::{current_sha, CANARY_RESD_COUNT};
use crate::AppState;

/// The Oxigraph named graph the class count is read from (documented for the
/// `DriftCounter` so it can verify the source).
const ONTOLOGY_GRAPH: &str = "urn:ngm:graph:ontology";

/// `GET /api/ontology/class-count`
pub async fn class_count(state: web::Data<AppState>) -> Result<HttpResponse> {
    let metrics = match state.ontology_repository.get_metrics().await {
        Ok(m) => m,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "ontology metrics read failed",
                "message": e.to_string(),
            })));
        }
    };

    let count = metrics.class_count;

    // RES-d: a live read fires the one-shot canary as the DriftCounter's own
    // query traffic — the count source is live. Fail-open: a canary write error
    // never fails the count read the canon depends on.
    let evidence =
        format!("ontology class-count read: {count} classes from Oxigraph <{ONTOLOGY_GRAPH}>");
    if let Err(e) = state
        .liveness_harness
        .observe(CANARY_RESD_COUNT, &evidence)
        .await
    {
        log::warn!("[ontology-class-count] failed to record {CANARY_RESD_COUNT} fire: {e}");
    }

    ok_json!(serde_json::json!({
        "class_count": count,
        "source": "oxigraph",
        "graph": ONTOLOGY_GRAPH,
        "method": "SPARQL COUNT(?s) WHERE GRAPH <urn:ngm:graph:ontology> { ?s a vc:OntologyClass }",
        "sha": current_sha(),
        "observed_at_ms": chrono::Utc::now().timestamp_millis(),
    }))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ontology/class-count").route("", web::get().to(class_count)));
}
