//! Enrichment-proposals governance decision endpoint — closes the broker
//! write-back loop with a DURABLE store and a REAL KG write.
//!
//! agentbox `management-api/routes/broker-bridge.js:361` POSTs broker decisions
//! to VisionClaw at `POST /api/enrichment-proposals/:id/decide`. This handler
//!   1. validates the broker decision payload,
//!   2. mints PROV-O provenance URNs through the converged `crate::uri` minter
//!      (an `execution` activity URN content-addressed over the decision, and a
//!      `kg` proposal URN when the broker pubkey scopes it),
//!   3. PERSISTS the decision into the durable [`SqliteEnrichmentRepository`]
//!      (`data/enrichment.sqlite3`) — INSERT decision + UPDATE proposal status,
//!      atomically — and
//!   4. on an *attributed approval*, performs a REAL fenced Oxigraph write into
//!      `:summary` via [`OxigraphOntologyRepository::append_derived_summary`],
//!      then flips `writeback_committed`.
//!
//! ## writeback_triggered vs writeback_committed
//!
//! The previous in-memory ghost (a `Lazy<Mutex<Vec>>` + a `writeback_triggered`
//! flag) CLAIMED a write-back but performed no write — `triggered` was a lie.
//! That is deleted. Now the two facts are separated and both persisted:
//!   * `writeback_triggered` — outcome qualifies (approve) AND attributed.
//!   * `writeback_committed`  — the Oxigraph derived write actually returned Ok.
//! The agentbox broker-bridge should key true closure off `committed`.
//!
//! Unattributed approvals are recorded durably (status transitions) but
//! `writeback_committed` stays `false`: there is no owner DID to scope an
//! owner-less KG node, so no fact is written. This preserves the loop while
//! refusing to write owner-less facts — by design, not a bug.

use actix_web::{web, HttpRequest, HttpResponse};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::actors::messages::BroadcastMessage;
use crate::actors::ClientCoordinatorActor;
use crate::adapters::sqlite_enrichment_repository::{EnrichmentProposal as StoredProposal, StoredDecision};
use crate::uri;
use crate::AppState;

/// Broker decision payload (the exact body agentbox broker-bridge POSTs).
#[derive(Debug, Clone, Deserialize)]
pub struct BrokerDecisionRequest {
    /// The verdict: "approve" / "reject" / etc. Accepts `outcome` (agentbox
    /// field name) or `decision`/`verdict` as aliases.
    #[serde(alias = "decision", alias = "verdict")]
    pub outcome: String,
    /// Deciding broker's did:nostr hex pubkey (attribution). Optional: an
    /// unattributed decision is recorded with `attributed: false` rather than
    /// rejected, so the loop stays closed for render compatibility.
    #[serde(default, alias = "pubkey")]
    pub broker_pubkey: Option<String>,
    /// Free-text rationale.
    #[serde(default, alias = "note")]
    pub reasoning: Option<String>,
}

/// Classify an outcome → `(writeback, status)`. `writeback` is true for
/// approval verbs; `status` is the coarse decided sub-state mirrored from the
/// durable store's mapping. Shared by `record_decision` and the persistence path.
fn classify(outcome: &str) -> (bool, &'static str) {
    let o = outcome.trim().to_ascii_lowercase();
    match o.as_str() {
        "approve" | "approved" | "accept" | "accepted" | "promote" => (true, "approved"),
        s if s.starts_with("reject") => (false, "rejected"),
        _ => (false, "reviewed"),
    }
}

/// A recorded governance decision (the durable audit record).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecordedDecision {
    pub case_id: String,
    pub outcome: String,
    /// True iff a structurally-valid broker pubkey attributed the decision.
    pub attributed: bool,
    pub broker_pubkey: Option<String>,
    pub reasoning: Option<String>,
    /// Whether this outcome triggers a KG write-back (approve + later gated on
    /// attribution at the write site).
    pub writeback_triggered: bool,
    /// PROV-O activity URN (`urn:visionclaw:execution:<sha256-12>`).
    pub activity_urn: String,
    /// `urn:visionclaw:kg:<pubkey>:<sha256-12>` when attributed, else `None`.
    pub proposal_urn: Option<String>,
    /// Owner DID when attributed (`did:nostr:<pubkey>`).
    pub owner_did: Option<String>,
    pub decided_at_ms: u64,
}

/// JSON response — mirrors the shape the agentbox broker-bridge expects, now
/// with the truthful `writeback_committed` bit alongside `writeback_triggered`.
#[derive(Debug, Serialize)]
struct DecideResponse {
    success: bool,
    decision: String,
    attributed: bool,
    writeback_triggered: bool,
    /// True only when the Oxigraph derived write actually landed.
    writeback_committed: bool,
    activity_urn: String,
    proposal_urn: Option<String>,
    owner_did: Option<String>,
}

/// WS broadcast envelope for the audit surface.
#[derive(Debug, Serialize)]
struct DecisionBroadcast<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    data: &'a RecordedDecision,
}

/// Shared service credential for unattended service-to-service callers. Mirrors
/// the established sibling pattern in `image_gen_handler::agent_key` so the
/// agentbox broker-bridge authenticates against the same `VISIONCLAW_AGENT_KEY`
/// it already uses for the image-gen agent-submit route.
fn agent_key() -> String {
    std::env::var("VISIONCLAW_AGENT_KEY").unwrap_or_else(|_| "changeme-agent-key".to_string())
}

/// Returns `Ok(())` when the request bears the valid service credential, else
/// an `Unauthorized` response the caller can short-circuit on.
fn require_agent_key(req: &HttpRequest) -> Result<(), HttpResponse> {
    let provided = req
        .headers()
        .get("x-agent-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided != agent_key() {
        warn!("[enrichment-decide] rejected: invalid or missing X-Agent-Key");
        return Err(HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "error": "Invalid or missing X-Agent-Key header",
        })));
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Pure core: validate + mint provenance + build the durable record. Unit-
/// testable without the actix actor or the store.
pub(crate) fn record_decision(
    case_id: &str,
    req: &BrokerDecisionRequest,
) -> Result<RecordedDecision, &'static str> {
    let outcome = req.outcome.trim();
    if case_id.trim().is_empty() {
        return Err("empty case id");
    }
    if outcome.is_empty() {
        return Err("empty decision outcome");
    }

    let (writeback_triggered, _status) = classify(outcome);

    // Attribution: a structurally-valid 64-hex pubkey ⇒ attributed. A malformed
    // or absent pubkey is recorded as unattributed rather than rejected.
    let (attributed, owner_did, proposal_urn) = match req.broker_pubkey.as_deref() {
        Some(pk) if uri::is_pubkey_hex(pk) => {
            let did = uri::did_nostr(pk).ok();
            let kg = uri::kg(pk, format!("enrichment-proposal:{case_id}")).ok();
            (true, did, kg)
        }
        _ => (false, None, None),
    };

    // PROV-O activity: content-addressed over the full decision tuple so the
    // same decision is idempotent and the crossing round-trips via BC20.
    let activity_urn = uri::execution(format!(
        "enrichment-decide:{case_id}:{outcome}:{}",
        req.broker_pubkey.as_deref().unwrap_or("anon")
    ));

    Ok(RecordedDecision {
        case_id: case_id.to_string(),
        outcome: outcome.to_string(),
        attributed,
        broker_pubkey: req.broker_pubkey.clone(),
        reasoning: req.reasoning.clone(),
        writeback_triggered,
        activity_urn,
        proposal_urn,
        owner_did,
        decided_at_ms: now_ms(),
    })
}

/// Build the summary triples emitted into `:summary` on an approved + attributed
/// decision. Uses the minted proposal URN as the subject so the KG node is
/// owner-scoped + content-addressed.
fn summary_triples_for(record: &RecordedDecision) -> Vec<(String, String, String)> {
    const P_ENRICHMENT_DECISION: &str =
        "https://narrativegoldmine.com/ns/v1#enrichmentDecision";
    let subject = record
        .proposal_urn
        .clone()
        .unwrap_or_else(|| format!("urn:ngm:enrichment-proposal:{}", record.case_id));
    vec![(
        subject,
        P_ENRICHMENT_DECISION.to_string(),
        record.outcome.clone(),
    )]
}

/// `POST /api/enrichment-proposals/{id}/decide`.
pub async fn decide(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<BrokerDecisionRequest>,
    state: web::Data<AppState>,
    client_coordinator: web::Data<actix::Addr<ClientCoordinatorActor>>,
) -> HttpResponse {
    // Gate first: service-to-service call from the agentbox broker-bridge.
    if let Err(resp) = require_agent_key(&req) {
        return resp;
    }

    let case_id = path.into_inner();

    let record = match record_decision(&case_id, &body) {
        Ok(r) => r,
        Err(e) => {
            warn!("[enrichment-decide] rejected case={case_id}: {e}");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": e,
            }));
        }
    };

    let repo = &state.sqlite_enrichment_repository;

    // Ensure a proposal row exists. The broker may decide a case VisionClaw has
    // not ingested yet — create a pending stub so the lifecycle stays closed.
    if matches!(repo.get(&case_id).await, Ok(None)) {
        let stub = StoredProposal {
            case_id: case_id.clone(),
            category: Some("knowledge_enrichment".to_string()),
            source_iri: record.proposal_urn.clone(),
            proposal_json: serde_json::json!({
                "broker_pubkey": record.broker_pubkey,
                "reasoning": record.reasoning,
            }),
            status: "pending".to_string(),
            created_at: 0,
            updated_at: 0,
        };
        if let Err(e) = repo.create_or_update(&stub).await {
            warn!("[enrichment-decide] stub create failed case={case_id}: {e}");
        }
    }

    // Persist the decision (atomic INSERT decision + UPDATE proposal.status).
    let stored = StoredDecision {
        case_id: case_id.clone(),
        outcome: record.outcome.clone(),
        attributed: record.attributed,
        broker_pubkey: record.broker_pubkey.clone(),
        reasoning: record.reasoning.clone(),
        writeback_triggered: record.writeback_triggered,
        writeback_committed: false,
        activity_urn: record.activity_urn.clone(),
        proposal_urn: record.proposal_urn.clone(),
        owner_did: record.owner_did.clone(),
        decided_at_ms: record.decided_at_ms as i64,
    };
    if let Err(e) = repo.record_decision(&stored).await {
        warn!("[enrichment-decide] persist failed case={case_id}: {e}");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": format!("failed to persist decision: {e}"),
        }));
    }

    // REAL write-back: on an attributed approval, write the fenced summary into
    // Oxigraph :summary, then mark committed. Unattributed approvals are
    // recorded but NOT written (no owner DID to scope an owner-less KG node).
    let mut writeback_committed = false;
    if record.writeback_triggered && record.attributed {
        match state
            .ontology_repository
            .append_derived_summary(
                record.owner_did.as_deref(),
                &record.activity_urn,
                summary_triples_for(&record),
            )
            .await
        {
            Ok(()) => {
                if let Err(e) = repo
                    .mark_writeback_committed(&case_id, &record.activity_urn)
                    .await
                {
                    warn!("[enrichment-decide] commit-mark failed case={case_id}: {e}");
                }
                writeback_committed = true;
            }
            Err(e) => {
                warn!("[enrichment-decide] derived write failed case={case_id}: {e}");
                writeback_committed = false;
            }
        }
    }

    // Broadcast to connected clients so the loop is observable.
    if let Ok(json) = serde_json::to_string(&DecisionBroadcast {
        type_: "enrichment_decision",
        data: &record,
    }) {
        client_coordinator.do_send(BroadcastMessage { message: json });
    }

    info!(
        "[enrichment-decide] case={case_id} outcome={} attributed={} triggered={} committed={} activity={}",
        record.outcome,
        record.attributed,
        record.writeback_triggered,
        writeback_committed,
        record.activity_urn
    );
    debug!("[enrichment-decide] full record: {record:?}");

    HttpResponse::Ok().json(DecideResponse {
        success: true,
        decision: record.outcome.clone(),
        attributed: record.attributed,
        writeback_triggered: record.writeback_triggered,
        writeback_committed,
        activity_urn: record.activity_urn.clone(),
        proposal_urn: record.proposal_urn.clone(),
        owner_did: record.owner_did.clone(),
    })
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/enrichment-proposals/{id}/decide",
        web::post().to(decide),
    );
}

/// Broker-facing projection of the durable enrichment store (WS-12 dependency).
///
/// WS-12's `broker_inbox_handler` imports `store::{EnrichmentProposal,
/// ProposalStatus}` and projects them into the `{cases:[...],total:N}` wire
/// shape the agentbox broker-bridge consumes. The accessors here map the durable
/// [`crate::adapters::sqlite_enrichment_repository`] rows into this stable
/// broker-facing shape; the inbox handler reads `state.sqlite_enrichment_repository`
/// and converts via [`EnrichmentProposal::from_stored`].
pub mod store {
    use super::StoredProposal;
    use crate::adapters::sqlite_enrichment_repository::SqliteEnrichmentRepository;
    use once_cell::sync::OnceCell;
    use serde::Serialize;
    use std::sync::Arc;

    /// Process-global handle to the durable enrichment repository, set once at
    /// startup from `AppState`. Lets the broker read accessors (`all`/`get`)
    /// reach the SINGLE durable store without threading `web::Data<AppState>`
    /// through every read call site — the read and write surfaces share one
    /// source of truth (the SQLite store), not two.
    static REPO: OnceCell<Arc<SqliteEnrichmentRepository>> = OnceCell::new();

    /// Install the durable repository handle. Idempotent; later calls are no-ops.
    /// Called from `AppState::new` after the repo is opened.
    pub fn init(repo: Arc<SqliteEnrichmentRepository>) {
        let _ = REPO.set(repo);
    }

    /// Page bound for `all()` reads — the bridge paginates client-side, but cap
    /// the projection to avoid unbounded reads.
    const ALL_LIMIT: i64 = 500;

    /// All proposals (newest-updated first), projected into the broker-facing
    /// shape. Empty when the store handle is not yet installed.
    pub async fn all() -> Vec<EnrichmentProposal> {
        match REPO.get() {
            Some(repo) => repo
                .list(None, ALL_LIMIT, 0)
                .await
                .unwrap_or_default()
                .iter()
                .map(EnrichmentProposal::from_stored)
                .collect(),
            None => Vec::new(),
        }
    }

    /// One proposal by id, projected into the broker-facing shape.
    pub async fn get(id: &str) -> Option<EnrichmentProposal> {
        let repo = REPO.get()?;
        repo.get(id)
            .await
            .ok()
            .flatten()
            .map(|p| EnrichmentProposal::from_stored(&p))
    }

    /// Coarse broker case status. `as_str` returns the lowercase strings the
    /// agentbox broker-bridge filters on (`broker-bridge.js:204`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    pub enum ProposalStatus {
        Pending,
        Claimed,
        Decided,
    }

    impl ProposalStatus {
        pub fn as_str(&self) -> &'static str {
            match self {
                ProposalStatus::Pending => "pending",
                ProposalStatus::Claimed => "claimed",
                ProposalStatus::Decided => "decided",
            }
        }

        /// Map the durable fine-grained status string to the coarse enum.
        /// `pending` → Pending; anything decided (`approved`/`rejected`/
        /// `reviewed`) → Decided; `claimed` → Claimed.
        pub fn from_durable(s: &str) -> Self {
            match s {
                "pending" => ProposalStatus::Pending,
                "claimed" => ProposalStatus::Claimed,
                _ => ProposalStatus::Decided,
            }
        }
    }

    /// Broker-facing proposal projection. Field names are the WS-12 contract;
    /// the inbox handler's `From` impl depends on them.
    #[derive(Debug, Clone, Serialize)]
    pub struct EnrichmentProposal {
        pub id: String,
        pub status: ProposalStatus,
        pub target_path: Option<String>,
        pub content: Option<String>,
        pub enrichment_type: Option<String>,
        pub proposer_did: Option<String>,
        pub reasoning_summary: Option<String>,
        pub reasoning_hash: Option<String>,
        pub broker_did: Option<String>,
        pub proposal_urn: Option<String>,
        pub activity_urn: Option<String>,
        pub created_at_ms: u64,
        pub decided_at_ms: Option<u64>,
    }

    impl EnrichmentProposal {
        /// Project a durable store row into the broker-facing shape. Metadata
        /// fields are pulled from the `proposal_json` blob best-effort.
        pub fn from_stored(p: &StoredProposal) -> Self {
            let j = &p.proposal_json;
            let s = |k: &str| j.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
            EnrichmentProposal {
                id: p.case_id.clone(),
                status: ProposalStatus::from_durable(&p.status),
                target_path: s("target_path").or_else(|| s("source_file")),
                content: s("content").or_else(|| s("proposed_enrichment")),
                enrichment_type: s("enrichment_type"),
                proposer_did: s("proposed_by").or_else(|| s("agent_did")),
                reasoning_summary: s("reasoning_summary").or_else(|| s("reasoning")),
                reasoning_hash: s("reasoning_hash"),
                broker_did: s("broker_did").or_else(|| s("broker_pubkey")),
                proposal_urn: p.source_iri.clone().or_else(|| s("proposal_urn")),
                activity_urn: s("activity_urn"),
                created_at_ms: (p.created_at.max(0) as u64) * 1000,
                decided_at_ms: if p.status == "pending" {
                    None
                } else {
                    Some((p.updated_at.max(0) as u64) * 1000)
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PK: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn attributed_approval_mints_provenance_and_triggers_writeback() {
        let req = BrokerDecisionRequest {
            outcome: "approve".into(),
            broker_pubkey: Some(PK.into()),
            reasoning: Some("looks good".into()),
        };
        let rec = record_decision("case-7", &req).expect("record");
        assert!(rec.attributed);
        assert!(rec.writeback_triggered);
        assert_eq!(rec.owner_did.as_deref(), Some(&*format!("did:nostr:{PK}")));
        assert!(rec
            .activity_urn
            .starts_with("urn:visionclaw:execution:sha256-12-"));
        assert!(rec
            .proposal_urn
            .as_deref()
            .unwrap()
            .starts_with(&format!("urn:visionclaw:kg:{PK}:sha256-12-")));
        assert!(uri::parse(&rec.activity_urn).is_ok());
        assert!(uri::parse(rec.proposal_urn.as_deref().unwrap()).is_ok());
    }

    #[test]
    fn unattributed_decision_is_recorded_not_rejected() {
        let req = BrokerDecisionRequest {
            outcome: "reject".into(),
            broker_pubkey: None,
            reasoning: None,
        };
        let rec = record_decision("case-9", &req).expect("record");
        assert!(!rec.attributed);
        assert!(!rec.writeback_triggered);
        assert!(rec.owner_did.is_none());
        assert!(rec.proposal_urn.is_none());
        assert!(uri::parse(&rec.activity_urn).is_ok());
    }

    #[test]
    fn malformed_pubkey_downgrades_to_unattributed() {
        let req = BrokerDecisionRequest {
            outcome: "approve".into(),
            broker_pubkey: Some("not-a-real-pubkey".into()),
            reasoning: None,
        };
        let rec = record_decision("case-1", &req).expect("record");
        assert!(!rec.attributed, "malformed pubkey ⇒ unattributed, not error");
        assert!(rec.proposal_urn.is_none());
    }

    #[test]
    fn empty_inputs_are_rejected() {
        let ok = BrokerDecisionRequest {
            outcome: "approve".into(),
            broker_pubkey: None,
            reasoning: None,
        };
        assert!(record_decision("", &ok).is_err());
        let empty_outcome = BrokerDecisionRequest {
            outcome: "  ".into(),
            broker_pubkey: None,
            reasoning: None,
        };
        assert!(record_decision("case-x", &empty_outcome).is_err());
    }

    #[test]
    fn outcome_alias_decision_deserialises() {
        let from_decision: BrokerDecisionRequest =
            serde_json::from_str(r#"{"decision":"accepted","pubkey":null}"#).unwrap();
        assert_eq!(from_decision.outcome, "accepted");
        let rec = record_decision("case-2", &from_decision).unwrap();
        assert!(rec.writeback_triggered);
    }

    #[test]
    fn classify_matches_status_mapping() {
        assert_eq!(classify("approve"), (true, "approved"));
        assert_eq!(classify("Accepted"), (true, "approved"));
        assert_eq!(classify("reject"), (false, "rejected"));
        assert_eq!(classify("amend"), (false, "reviewed"));
    }
}
