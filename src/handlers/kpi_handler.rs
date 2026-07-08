// src/handlers/kpi_handler.rs
//! HTTP surface for the REC-4 four-KPI dashboard (ADR-043 resurrection,
//! ADR-130 Decision 5).
//!
//! Two routes, mounted under `/api`:
//!   * `GET /api/kpi/summary` — compute the two live KPIs (Augmentation Ratio,
//!     Trust Variance) fresh from source events, persist a snapshot with lineage
//!     for each, fire `CANARY-VC-REC4-KPI`, and return the four-tile summary. The
//!     two not-yet-computable KPIs (Mesh Velocity, HITL Precision) return with
//!     `status: "awaiting_data_source"` and the named source — never a value.
//!   * `GET /api/kpi/lineage/{snapshot_id}` — the `DERIVED_FROM` trail for a
//!     persisted snapshot: the source events a KPI value was computed from
//!     (WP-8 AC3).
//!
//! The compute + persist happens on read so the stored series always traces to
//! the events that produced it, and the dashboard reads a value that was just
//! evidenced against live sources.

use actix_web::{web, HttpResponse, Result};

use crate::ok_json;
use crate::services::kpi_compute::KpiComputeService;

/// `GET /api/kpi/summary`
pub async fn summary(service: web::Data<KpiComputeService>) -> Result<HttpResponse> {
    match service.compute_and_persist().await {
        Ok(summary) => ok_json!(summary),
        Err(e) => Ok(HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e }))),
    }
}

/// `GET /api/kpi/lineage/{snapshot_id}`
pub async fn lineage(
    service: web::Data<KpiComputeService>,
    path: web::Path<i64>,
) -> Result<HttpResponse> {
    let snapshot_id = path.into_inner();
    match service.lineage_for(snapshot_id).await {
        Ok(rows) => ok_json!(serde_json::json!({
            "snapshot_id": snapshot_id,
            "lineage": rows,
        })),
        Err(e) => Ok(HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e }))),
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/kpi")
            .route("/summary", web::get().to(summary))
            .route("/lineage/{snapshot_id}", web::get().to(lineage)),
    );
}
