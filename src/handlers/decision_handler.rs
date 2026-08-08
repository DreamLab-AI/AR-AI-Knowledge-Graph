//! HTTP handler for decision records (PRD-022 W-B / ADR-048).
//!
//! Decisions are first-class, `did:nostr`-attributed Oxigraph nodes. This
//! surface exposes two endpoints:
//!
//!   POST /decisions/record        — mint + write a DecisionRecord (AUTHED)
//!   GET  /decisions/{urn}/trace    — bounded, derived reachability (anon read)
//!
//! `/record` is the ONLY mutating route, so — exactly like the ontology-agent
//! `/propose` door — it is the only one wrapped in `RequireAuth::authenticated()`.
//! The deciding principal is the AUTHENTICATED `did:nostr` pubkey, NEVER a body
//! field: the URN scope and any attribution derive from `auth.pubkey` so a
//! caller cannot self-assert another agent's decision (ADR-048 §Attribution,
//! ADR-047 rule 2).
//!
//! `/trace` returns query-derived, bounded ancestry (or, with
//! `?direction=downstream`, the impact set) over DIRECT `dl:caused` /
//! `dl:precedentFor` links, with the supporting path for each hop. The response
//! is stamped `derived: true` — it is reachability, never a materialised or
//! "Whelk-classified" transitive edge (ADR-048 §"Graph placement").

use std::collections::{HashMap, HashSet};

use actix_web::{web, Error, HttpResponse};
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::middleware::RequireAuth;
use crate::services::decision_service::{
    self, bounded_bfs, direct_links_query, DecisionInput, DecisionService, Direction,
};
use crate::services::ontology_mutation_service::{
    CONFLICT_BLOCKED_PREFIX, ENVELOPE_REJECTED_PREFIX, IDEMPOTENCY_CONFLICT_PREFIX,
};
use crate::settings::auth_extractor::AuthenticatedUser;
use crate::types::ontology_tools::{GateSummary, ProposalReceipt};
use crate::{error_json, ok_json, AppState};

use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Request / response DTOs
// ---------------------------------------------------------------------------

/// Body for `POST /decisions/record`. Carries NO agent/scope/pubkey field —
/// the deciding identity is the authenticated principal, always.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDecisionRequest {
    pub summary: String,
    pub rationale: String,
    pub proposal_urn: Option<String>,
    #[serde(default)]
    pub caused: Vec<String>,
    #[serde(default)]
    pub precedent_for: Vec<String>,
    #[serde(default)]
    pub influenced: Vec<String>,
    #[serde(default)]
    pub considered_inputs: Vec<String>,
    #[serde(default)]
    pub governed_by: Vec<String>,
    /// W-E transaction spine (ADR-049): optional client-supplied idempotency key.
    /// A replay of the same key with an identical decision payload returns the
    /// prior receipt (no-op); a replay with a different payload is rejected (409).
    /// Absent → the spine mints a deterministic payload-derived key.
    ///
    /// The container renames fields to camelCase (`idempotencyKey`), so a client
    /// posting the snake_case `idempotency_key` would otherwise be silently dropped
    /// to `None` and always receive an `auto:<hash>` receipt. The alias accepts BOTH
    /// spellings so a supplied key is honoured either way (root cause of the live
    /// "receipt always shows auto:" defect).
    #[serde(default, alias = "idempotency_key")]
    pub idempotency_key: Option<String>,
    /// PRD-022 W-D: OPTIONAL native signature envelope — a BIP-340 (secp256k1
    /// Schnorr) signature, 64-byte / 128-char hex, by the authenticated principal's
    /// x-only pubkey over `sha256(canonicalize(full decision payload))`. Absent →
    /// unchanged behaviour (verified only when present, or when
    /// `ONTOLOGY_REQUIRE_SIGNED_ENVELOPE` demands it).
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDecisionResponse {
    pub success: bool,
    pub decision_urn: String,
    /// A recorded decision is an asserted node, not a derived closure.
    pub derived: bool,
    pub quads_written: usize,
    /// True when the write was an idempotent content-address replay (no re-write).
    pub replayed: bool,
    /// The idempotent proposal receipt from the spine (content-addressed hashes
    /// of the asserted projection, the provenance quads, and the envelope).
    pub receipt: ProposalReceipt,
    /// Per-gate outcome (conflict / Whelk consistency / ACSP governance).
    pub gates: GateSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceQuery {
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// `ancestry` (default) or `downstream`.
    #[serde(default)]
    pub direction: Option<String>,
}

fn default_max_depth() -> usize {
    5
}

/// Hard cap so an unbounded `?max_depth=` cannot fan the store out. Bounded
/// traversal is a first-class requirement (ADR-048), not a nicety.
const MAX_DEPTH_CAP: usize = 64;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceResponse {
    pub success: bool,
    pub root: String,
    pub direction: String,
    pub max_depth: usize,
    /// Query-derived, bounded reachability — NEVER asserted / "Whelk-classified".
    pub derived: bool,
    pub hops: Vec<decision_service::TraversalHop>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /decisions/record — mint and write a DecisionRecord through the governed
/// spine. Authenticated: the URN scope + `did:nostr` attribution are the caller's
/// verified pubkey (never a body field). The write traverses the conflict gate,
/// the Whelk consistency gate, and the `proposal_spine` single Oxigraph
/// transaction (asserted decision quads + T3 PROV-O provenance quads, atomically)
/// producing an idempotent receipt — there is NO raw insert here (ADR-047 rule 1,
/// ADR-048/049).
pub async fn record_decision(
    auth: AuthenticatedUser,
    decision_service: web::Data<Arc<DecisionService>>,
    request: web::Json<RecordDecisionRequest>,
) -> Result<HttpResponse, Error> {
    let req = request.into_inner();
    info!("decisions/record: authed principal={}", auth.pubkey);

    let idempotency_key = req.idempotency_key.clone();
    let signature = req.signature.clone();
    let input = DecisionInput {
        summary: req.summary,
        rationale: req.rationale,
        proposal_urn: req.proposal_urn,
        caused: req.caused,
        precedent_for: req.precedent_for,
        influenced: req.influenced,
        considered_inputs: req.considered_inputs,
        governed_by: req.governed_by,
    };

    match decision_service
        .record_decision(&auth.pubkey, input, idempotency_key, signature)
        .await
    {
        Ok(success) => {
            info!(
                "decisions/record: {} for {} ({} quad(s))",
                if success.replayed { "replayed" } else { "committed" },
                success.decision_urn,
                success.quads_written
            );
            ok_json!(RecordDecisionResponse {
                success: true,
                decision_urn: success.decision_urn,
                derived: false,
                quads_written: success.quads_written,
                replayed: success.replayed,
                receipt: success.receipt,
                gates: success.gates,
            })
        }
        // A blocking conflict-integrity report that implicates this decision → 409.
        // The delta-scoped report separates the conflicts THIS decision introduces
        // (`blocking`) from pre-existing corpus advisories (`preExisting`); we lift both
        // to the top level for the client gate-chips consumer, keeping the full report.
        Err(e) if e.starts_with(CONFLICT_BLOCKED_PREFIX) => {
            let body = &e[CONFLICT_BLOCKED_PREFIX.len()..];
            let report: serde_json::Value =
                serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({ "raw": body }));
            Ok(HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "conflict_blocked",
                "blockingConflicts": report.get("blocking").cloned().unwrap_or(serde_json::json!([])),
                "preExisting": report.get("preExisting").cloned().unwrap_or(serde_json::json!([])),
                "conflictReport": report
            })))
        }
        // Idempotency key reused with a divergent decision payload → 409 via spine.
        Err(e) if e.starts_with(IDEMPOTENCY_CONFLICT_PREFIX) => {
            Ok(HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "idempotency_conflict",
                "message": &e[IDEMPOTENCY_CONFLICT_PREFIX.len()..]
            })))
        }
        // Signature-envelope precondition failed (fail-closed) → 403.
        Err(e) if e.starts_with(ENVELOPE_REJECTED_PREFIX) => {
            Ok(HttpResponse::Forbidden().json(serde_json::json!({
                "success": false,
                "error": "envelope_rejected",
                "message": &e[ENVELOPE_REJECTED_PREFIX.len()..]
            })))
        }
        Err(e) => {
            error!("decisions/record: {e}");
            error_json!("Decision write failed", e)
        }
    }
}

/// GET /decisions/{urn}/trace — bounded, derived decision-chain traversal.
pub async fn trace_decision(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<TraceQuery>,
) -> Result<HttpResponse, Error> {
    let root = path.into_inner();
    let q = query.into_inner();
    let max_depth = q.max_depth.min(MAX_DEPTH_CAP);
    let direction = match q.direction.as_deref() {
        Some("downstream") => Direction::Downstream,
        _ => Direction::Ancestry,
    };
    let direction_label = match direction {
        Direction::Ancestry => "ancestry",
        Direction::Downstream => "downstream",
    };
    info!(
        "decisions/trace: root={root} direction={direction_label} max_depth={max_depth}"
    );

    let store: Arc<Store> = state.ontology_repository.store().clone();
    let root_for_task = root.clone();

    // Gather the DIRECT-link adjacency by expanding one bounded SPARQL frontier
    // per depth level (≤ max_depth queries) — never a transitive property path.
    let adjacency = tokio::task::spawn_blocking(move || -> Result<HashMap<String, Vec<String>>, String> {
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(root_for_task.clone());
        let mut frontier: Vec<String> = vec![root_for_task];

        for _ in 0..max_depth {
            if frontier.is_empty() {
                break;
            }
            let sparql = direct_links_query(&frontier, direction);
            let results = store.query(&sparql).map_err(|e| e.to_string())?;
            let mut next: Vec<String> = Vec::new();
            if let QueryResults::Solutions(solutions) = results {
                for sol in solutions {
                    let sol = sol.map_err(|e| e.to_string())?;
                    let cur = match sol.get("cur") {
                        Some(Term::NamedNode(n)) => n.as_str().to_string(),
                        _ => continue,
                    };
                    let nb = match sol.get("nb") {
                        Some(Term::NamedNode(n)) => n.as_str().to_string(),
                        _ => continue,
                    };
                    adjacency.entry(cur).or_default().push(nb.clone());
                    if seen.insert(nb.clone()) {
                        next.push(nb);
                    }
                }
            }
            frontier = next;
        }
        Ok(adjacency)
    })
    .await;

    match adjacency {
        Ok(Ok(adjacency)) => {
            let hops = bounded_bfs(&root, max_depth, &adjacency);
            ok_json!(TraceResponse {
                success: true,
                root,
                direction: direction_label.to_string(),
                max_depth,
                derived: true,
                hops,
            })
        }
        Ok(Err(e)) => {
            error!("decisions/trace: query failed: {e}");
            error_json!("Decision trace failed", e)
        }
        Err(e) => {
            error!("decisions/trace: join error: {e}");
            error_json!("Decision trace failed", e.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Route configuration
// ---------------------------------------------------------------------------

/// Mount the decision surface. `/record` is nested in its own sub-scope so it
/// alone carries `RequireAuth::authenticated()` (the single write door), while
/// `/trace` stays an anonymous read — mirroring the ontology-agent split.
pub fn configure_decision_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/decisions")
            .route("/{urn}/trace", web::get().to(trace_decision))
            .service(
                web::scope("/record")
                    .wrap(RequireAuth::authenticated())
                    .route("", web::post().to(record_decision)),
            ),
    );
}
