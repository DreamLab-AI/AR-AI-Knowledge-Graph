//! REST surface for the Insight Ingestion Loop v1 (REC-10, PRD-023 WP-12).
//!
//! Two reads, mounted under `/api/insight-loop`:
//!   * `GET /api/insight-loop/trace`            — every case's five-stage loop
//!     trace (newest first, `?limit=` bounded) plus the aggregate Mesh Velocity
//!     over the closed loops.
//!   * `GET /api/insight-loop/trace/{case_id}`  — one case's five-stage trace.
//!
//! The loop is assembled by [`crate::services::insight_loop`] from the governed
//! write-back queue's persisted stage timestamps: propose / queued (proposal
//! row), broker_decision / merged_enrichment (decision row). The amplification
//! stage is labelled `planned` (v1 scope). When a loop has closed once end to
//! end with monotonic timestamps, the read fires `CANARY-VC-REC10-LOOP` as
//! observed live traffic — a computed Mesh Velocity sample, not a synthetic probe.

use actix_web::{web, HttpResponse};
use log::debug;
use serde::Deserialize;

use crate::services::insight_loop::{self, InsightLoopTrace};
use crate::services::liveness_harness::CANARY_REC10_LOOP;
use crate::AppState;

/// Default and cap on traces returned by the batch read.
const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 1000;

#[derive(Debug, Deserialize)]
pub struct TraceQuery {
    limit: Option<i64>,
}

/// Fire `CANARY-VC-REC10-LOOP` when a closed, monotonic loop is present in the
/// read. Observed live traffic only — the predicate is a real computed loop.
async fn maybe_fire_canary(state: &AppState, traces: &[InsightLoopTrace]) {
    if let Some(t) = traces.iter().find(|t| t.loop_closed && t.monotonic) {
        let evidence = format!(
            "insight loop closed end to end: case={} mesh_velocity_ms={:?} \
             stages=propose→queued→decision→merged (amplification planned)",
            t.case_id, t.mesh_velocity_ms
        );
        if let Err(e) = state.liveness_harness.observe(CANARY_REC10_LOOP, &evidence).await {
            debug!("[insight-loop] REC-10 canary observe skipped: {e}");
        }
    }
}

/// `GET /api/insight-loop/trace`
pub async fn traces(state: web::Data<AppState>, q: web::Query<TraceQuery>) -> HttpResponse {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let rows = match state.sqlite_enrichment_repository.loop_traces(limit).await {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": format!("loop-trace read failed: {e}") }));
        }
    };
    let summary = insight_loop::summarise(&rows);
    maybe_fire_canary(&state, &summary.traces).await;
    HttpResponse::Ok().json(summary)
}

/// `GET /api/insight-loop/trace/{case_id}`
pub async fn trace_by_case(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let case_id = path.into_inner();
    match state.sqlite_enrichment_repository.loop_trace_for(&case_id).await {
        Ok(Some(row)) => {
            let trace = insight_loop::build_trace(&row);
            maybe_fire_canary(&state, std::slice::from_ref(&trace)).await;
            HttpResponse::Ok().json(trace)
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "not-found",
            "message": format!("no insight-loop trace for case {case_id}"),
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": format!("loop-trace read failed: {e}") })),
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/insight-loop")
            .route("/trace", web::get().to(traces))
            .route("/trace/{case_id}", web::get().to(trace_by_case)),
    );
}
