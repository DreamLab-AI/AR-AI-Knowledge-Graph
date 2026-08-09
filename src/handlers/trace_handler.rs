//! REST surface for the data-moat unified provenance trace (REC-11, PRD-023 WP-12).
//!
//! `GET /api/trace` returns ONE queryable trace joining the ecosystem's live
//! provenance sources on the `did:nostr` attribution the solid-pod contract
//! fixes as the shared key:
//!   * agent-events / hook-trajectory (`kpi_agent_events`, CTC fields since P1),
//!   * broker decisions (`enrichment_decisions`), and
//!   * pod git-marks (solid-pod-rs) — default-off, so reported under
//!     `sources_absent` and incorporated only when a `--features git` pod supplies
//!     them (the contract's head caveat).
//!
//! Query params: `?agent=<did:nostr>` restricts to one identity; `?window_ms=`
//! overrides the 30-day default window. The trace is a read-time JOIN over stores
//! that already exist — not a new store (ADR-130). When the returned trace joins
//! ≥2 live source kinds under one `did:nostr`, the read fires
//! `CANARY-VC-REC11-TRACE` as observed live traffic.

use actix_web::{web, HttpResponse};
use log::debug;
use serde::Deserialize;

use crate::services::liveness_harness::CANARY_REC11_TRACE;
use crate::services::provenance_trace::{ProvenanceTraceService, TRACE_WINDOW_MS};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TraceQuery {
    /// Restrict to one `did:nostr` agent identity.
    agent: Option<String>,
    /// Rolling window in ms (default `TRACE_WINDOW_MS` = 30 days).
    window_ms: Option<i64>,
}

/// `GET /api/trace`
pub async fn unified_trace(state: web::Data<AppState>, q: web::Query<TraceQuery>) -> HttpResponse {
    // The trace is a read-time join over the two live SQLite-backed sources; the
    // pod source is default-off (planned). Build the service from the shared
    // repositories on AppState — cheap Arc clones, no new store.
    let service = ProvenanceTraceService::new(
        state.sqlite_enrichment_repository.clone(),
        state.sqlite_kpi_repository.clone(),
    );
    let window = q.window_ms.unwrap_or(TRACE_WINDOW_MS);
    let trace = match service.query(window, q.agent.as_deref()).await {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "error": e }));
        }
    };

    // Fire CANARY-VC-REC11-TRACE when the trace genuinely joined ≥2 live source
    // kinds under one did:nostr (the REC-11 acceptance). Observed traffic only.
    if trace.joins_multiple_source_kinds() {
        let evidence = format!(
            "unified trace joined {} live source kinds under a shared did:nostr \
             (present={:?} absent={:?} records={} joins={})",
            trace.max_join_span,
            trace.sources_present,
            trace.sources_absent,
            trace.total_records,
            trace.joins.len(),
        );
        if let Err(e) = state
            .liveness_harness
            .observe(CANARY_REC11_TRACE, &evidence)
            .await
        {
            debug!("[trace] REC-11 canary observe skipped: {e}");
        }
    }

    HttpResponse::Ok().json(trace)
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/trace", web::get().to(unified_trace));
}
