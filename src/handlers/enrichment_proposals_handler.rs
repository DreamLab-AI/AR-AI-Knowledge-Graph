//! Enrichment-proposals governance decision endpoint — closes the broker
//! write-back loop.
//!
//! agentbox `management-api/routes/broker-bridge.js:356` POSTs broker decisions
//! to VisionClaw at `POST /api/enrichment-proposals/:id/decide`. Before this
//! handler that route had **zero** matches in `src/` — the broker's write-back
//! call was a call into the void (HTTP 404), so governed elevation could be
//! decided on the agentbox side but never recorded on the VisionClaw side.
//!
//! The durable proposal/KG store is not reachable on `main` yet (the
//! `EnrichmentProposal` aggregate has not merged), so per the close-the-loop
//! mandate this handler does the next-best, non-lossy thing: it
//!   1. validates the broker decision payload,
//!   2. mints PROV-O provenance URNs through the converged `crate::uri` minter
//!      (an `execution` activity URN content-addressed over the decision, and a
//!      `kg` proposal URN when the broker pubkey scopes it), and
//!   3. persists the decision into a process-global [`decision_log`] and
//!      broadcasts an `enrichment_decision` event to every connected client so
//!      the audit surface observes it.
//!
//! When the durable store lands, step 3 becomes "apply to the proposal/KG
//! aggregate"; the wire contract and provenance minting are already correct.

use actix_web::{web, HttpRequest, HttpResponse};
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tokio::sync::broadcast;

use crate::actors::messages::BroadcastMessage;
use crate::actors::ClientCoordinatorActor;
use crate::uri;

/// Decisions that trigger a KG write-back on approval. Mirrors the agentbox
/// `WRITEBACK_DECISIONS` set so both sides agree on which outcomes mutate the KG.
const WRITEBACK_DECISIONS: &[&str] = &["approve", "approved", "accept", "accepted"];

/// Bounded audit fan-out buffer (same posture as `agent_events::hub`).
const DECISION_HUB_CAPACITY: usize = 256;

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

/// A recorded governance decision (the durable audit record).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecordedDecision {
    pub case_id: String,
    pub outcome: String,
    /// True iff a structurally-valid broker pubkey attributed the decision.
    pub attributed: bool,
    pub broker_pubkey: Option<String>,
    pub reasoning: Option<String>,
    /// Whether this outcome triggers a KG write-back.
    pub writeback_triggered: bool,
    /// PROV-O activity URN (`urn:visionclaw:execution:<sha256-12>`).
    pub activity_urn: String,
    /// `urn:visionclaw:kg:<pubkey>:<sha256-12>` when attributed, else `None`.
    pub proposal_urn: Option<String>,
    /// Owner DID when attributed (`did:nostr:<pubkey>`).
    pub owner_did: Option<String>,
    pub decided_at_ms: u64,
}

/// JSON response — mirrors the shape the agentbox broker-bridge expects.
#[derive(Debug, Serialize)]
struct DecideResponse {
    success: bool,
    decision: String,
    attributed: bool,
    writeback_triggered: bool,
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

/// Process-global durable decision log. Mirrors the `agent_events::hub`
/// singleton posture rather than threading a field through `AppState`. When the
/// `EnrichmentProposal` aggregate merges, reads here migrate to that store.
static DECISION_LOG: Lazy<Mutex<Vec<RecordedDecision>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Audit fan-out channel: every recorded decision is published here so a future
/// audit-trail subscriber can observe the signed/unsigned distinction.
static DECISION_HUB: Lazy<broadcast::Sender<RecordedDecision>> =
    Lazy::new(|| broadcast::channel(DECISION_HUB_CAPACITY).0);

/// Subscribe to the recorded-decision audit stream.
pub fn subscribe() -> broadcast::Receiver<RecordedDecision> {
    DECISION_HUB.subscribe()
}

/// Number of decisions recorded so far (audit-surface read).
pub fn decision_count() -> usize {
    DECISION_LOG.lock().map(|g| g.len()).unwrap_or(0)
}

/// Shared service credential for unattended service-to-service callers. Mirrors
/// the established sibling pattern in `image_gen_handler::agent_key` so the
/// agentbox broker-bridge authenticates against the same `VISIONCLAW_AGENT_KEY`
/// it already uses for the image-gen agent-submit route.
fn agent_key() -> String {
    std::env::var("VISIONCLAW_AGENT_KEY").unwrap_or_else(|_| "changeme-agent-key".to_string())
}

/// Constant-time-ish equality on the presented `X-Agent-Key`. Returns `Ok(())`
/// when the request bears the valid service credential, else an `Unauthorized`
/// response the caller can short-circuit on.
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
/// testable without the actix actor (mirrors `agent_events::ingest::process_frame`).
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

    let writeback_triggered = WRITEBACK_DECISIONS
        .iter()
        .any(|d| d.eq_ignore_ascii_case(outcome));

    // Attribution: a structurally-valid 64-hex pubkey ⇒ attributed. A malformed
    // or absent pubkey is recorded as unattributed rather than rejected.
    let (attributed, owner_did, proposal_urn) = match req.broker_pubkey.as_deref() {
        Some(pk) if uri::is_pubkey_hex(pk) => {
            let did = uri::did_nostr(pk).ok();
            // The proposal node, owner-scoped + content-addressed on the case.
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

/// `POST /api/enrichment-proposals/{id}/decide`.
pub async fn decide(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<BrokerDecisionRequest>,
    client_coordinator: web::Data<actix::Addr<ClientCoordinatorActor>>,
) -> HttpResponse {
    // Gate first: this mutating governance route is a service-to-service call
    // from the agentbox broker-bridge, not the human UI (no client/src caller).
    // Require the established service credential before any decision is recorded,
    // broadcast, or attributed — closes the unauthenticated write-back loop.
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

    // Persist into the process-global decision log (durable for this process;
    // migrates to the EnrichmentProposal aggregate when it merges).
    if let Ok(mut log) = DECISION_LOG.lock() {
        log.push(record.clone());
    }

    // Publish to the audit stream + broadcast to connected clients so the loop
    // is observable, not a silent recording.
    let _ = DECISION_HUB.send(record.clone());
    if let Ok(json) = serde_json::to_string(&DecisionBroadcast {
        type_: "enrichment_decision",
        data: &record,
    }) {
        client_coordinator.do_send(BroadcastMessage { message: json });
    }

    info!(
        "[enrichment-decide] case={case_id} outcome={} attributed={} writeback={} activity={}",
        record.outcome, record.attributed, record.writeback_triggered, record.activity_urn
    );
    debug!("[enrichment-decide] full record: {record:?}");

    HttpResponse::Ok().json(DecideResponse {
        success: true,
        decision: record.outcome.clone(),
        attributed: record.attributed,
        writeback_triggered: record.writeback_triggered,
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
        // minted URNs are well-formed per the converged grammar.
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
        // activity URN is still minted — provenance exists for unsigned actions.
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
        // agentbox sends `outcome`; tolerate `decision`/`verdict` aliases too.
        let from_decision: BrokerDecisionRequest =
            serde_json::from_str(r#"{"decision":"accepted","pubkey":null}"#).unwrap();
        assert_eq!(from_decision.outcome, "accepted");
        let rec = record_decision("case-2", &from_decision).unwrap();
        assert!(rec.writeback_triggered);
    }
}
