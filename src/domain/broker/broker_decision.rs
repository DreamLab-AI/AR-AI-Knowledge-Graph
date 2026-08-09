//! BrokerDecision value object + DecisionOrchestrator service (ADR-041).
//!
//! Six canonical `DecisionOutcome` variants per ADR-041 Decision Actions
//! table: Approve, Reject, Amend, Delegate, Promote, Precedent.
//!
//! `DecisionOrchestrator` turns a well-formed decision into the side-effects
//! the aggregate owns: append to history, emit an audit event, and — for
//! `ContributorMeshShare` cases — return a `ShareTransitionPlan` that the
//! `ShareOrchestratorActor` (agent C4, follow-up sprint) consumes. This
//! module deliberately does not perform the WAC/Pod transition itself; it
//! produces a plan and defers execution to C4.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::broker_case::{
    BrokerCase, CaseCategory, CaseInvariantError, DecisionHistoryEntry, ShareState, SubjectRef,
};

// ---------------------------------------------------------------------------
// DecisionOutcome — six variants per ADR-041
// ---------------------------------------------------------------------------

/// The six canonical decision outcomes. Carries payload per variant so the
/// aggregate can enforce invariants at recording time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DecisionOutcome {
    /// Accept as-is.
    Approve,
    /// Reject; `reasoning` on the history entry preserves why.
    Reject,
    /// Accept with amendments. `diff` describes the changes in whatever form
    /// the upstream aggregate understands (e.g. JSON patch).
    Amend { diff: String },
    /// Reassign to another broker (e.g. domain expert).
    Delegate { delegate_to: String },
    /// Approve and promote to a reusable pattern / distribution. `pattern_id`
    /// is the freshly minted id of the downstream artefact.
    Promote { pattern_id: String },
    /// Flag as a precedent so future similar cases can reference it.
    Precedent { scope: String },
}

impl DecisionOutcome {
    pub fn action_str(&self) -> &'static str {
        match self {
            DecisionOutcome::Approve => "approve",
            DecisionOutcome::Reject => "reject",
            DecisionOutcome::Amend { .. } => "amend",
            DecisionOutcome::Delegate { .. } => "delegate",
            DecisionOutcome::Promote { .. } => "promote",
            DecisionOutcome::Precedent { .. } => "precedent",
        }
    }

    /// Map an ACSP / REST-fallback action verb to a canonical outcome so one
    /// vocabulary flows from the wire (`ActionResponse.action`, the agentbox
    /// broker-bridge `outcome` field) into the kernel. Payload-carrying
    /// variants (amend/delegate/promote/precedent) are reconstructed with the
    /// supplied `detail`; when `detail` is absent for a variant that needs it
    /// the mapping fails, letting the caller keep the aggregate's payload
    /// invariants (`MissingAmendmentDiff` / `MissingDelegateTarget`) rather
    /// than fabricating an empty payload.
    ///
    /// Approval synonyms (`approve`/`approved`/`accept`/`accepted`) and reject
    /// synonyms mirror `enrichment_proposals_handler::classify`, so the kernel
    /// and the durable store agree on what an approval is.
    pub fn from_action(action: &str, detail: Option<&str>) -> Option<Self> {
        match action.trim().to_ascii_lowercase().as_str() {
            "approve" | "approved" | "accept" | "accepted" => Some(DecisionOutcome::Approve),
            s if s.starts_with("reject") => Some(DecisionOutcome::Reject),
            "amend" => detail.map(|d| DecisionOutcome::Amend {
                diff: d.to_string(),
            }),
            "delegate" => detail.map(|d| DecisionOutcome::Delegate {
                delegate_to: d.to_string(),
            }),
            "promote" => Some(DecisionOutcome::Promote {
                pattern_id: detail.unwrap_or("").to_string(),
            }),
            "precedent" => detail.map(|d| DecisionOutcome::Precedent {
                scope: d.to_string(),
            }),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Invariant(#[from] CaseInvariantError),

    #[error("share transition planner rejected plan: {0}")]
    ShareTransitionRejected(String),

    #[error("unsupported subject for category {0:?}")]
    UnsupportedSubject(CaseCategory),
}

/// Side-effect plan returned when a `ContributorMeshShare` decision is
/// recorded. Consumed downstream by `ShareOrchestratorActor` (C4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShareTransitionPlan {
    pub case_id: String,
    pub subject: SubjectRef,
    pub from: ShareState,
    pub to: ShareState,
    pub approved_by: String,
}

/// Outcome of orchestrating a single decision.
#[derive(Debug, Clone)]
pub struct DecisionOutcomeReport {
    pub case_id: String,
    pub entry: DecisionHistoryEntry,
    /// Present only for `ContributorMeshShare` + `Approve|Promote` where a
    /// state transition is implied.
    pub share_plan: Option<ShareTransitionPlan>,
}

/// Domain service that coordinates decision recording. It is intentionally a
/// plain struct (no actor state) so it can be called from REST handlers, the
/// `BrokerActor`, or tests without wiring.
#[derive(Debug, Default, Clone)]
pub struct DecisionOrchestrator;

impl DecisionOrchestrator {
    pub fn new() -> Self {
        Self
    }

    /// Record `outcome` against `case`, enforcing aggregate invariants and,
    /// for contributor-mesh-share cases, producing a `ShareTransitionPlan`.
    pub fn decide(
        &self,
        case: &mut BrokerCase,
        decision_id: impl Into<String>,
        outcome: DecisionOutcome,
        broker_pubkey: impl Into<String>,
        reasoning: impl Into<String>,
    ) -> Result<DecisionOutcomeReport, OrchestrationError> {
        let broker_pubkey_s = broker_pubkey.into();
        let outcome_clone = outcome.clone();

        let entry = case
            .record_decision(decision_id, outcome, &broker_pubkey_s, reasoning)?
            .clone();

        let share_plan = match (&case.category, &outcome_clone) {
            (
                CaseCategory::ContributorMeshShare,
                DecisionOutcome::Approve | DecisionOutcome::Promote { .. },
            ) => build_share_plan(case, &broker_pubkey_s)?,
            _ => None,
        };

        Ok(DecisionOutcomeReport {
            case_id: case.id.clone(),
            entry,
            share_plan,
        })
    }
}

fn build_share_plan(
    case: &BrokerCase,
    approved_by: &str,
) -> Result<Option<ShareTransitionPlan>, OrchestrationError> {
    let (Some(from), Some(to)) = (case.subject.from_state, case.subject.to_state) else {
        // No transition requested on the subject; a plain approval is fine
        // but produces no share plan.
        return Ok(None);
    };
    if !from.can_advance_to(to) {
        return Err(OrchestrationError::ShareTransitionRejected(format!(
            "{:?} -> {:?} is not a forward transition",
            from, to
        )));
    }
    Ok(Some(ShareTransitionPlan {
        case_id: case.id.clone(),
        subject: case.subject.clone(),
        from,
        to,
        approved_by: approved_by.to_string(),
    }))
}

// ---------------------------------------------------------------------------
// ShareIntentBrokerAdapter — scaffold
// ---------------------------------------------------------------------------

/// Adapter the contributor studio (BC18) uses to *submit* a share intent to
/// the broker. It translates a raw `ShareIntent` into a `BrokerCase` with
/// `category = ContributorMeshShare`.
///
/// NOTE: the *execution* of the resulting `ShareTransitionPlan` lives in
/// `ShareOrchestratorActor` (agent C4 — follow-up sprint). This adapter is
/// only a scaffold exposing the public shape so BC18 can depend on it today.
pub struct ShareIntentBrokerAdapter;

impl ShareIntentBrokerAdapter {
    /// Build a case from a share-intent tuple. The real BC18 `ShareIntent`
    /// aggregate lives elsewhere; we accept the minimal fields here so the
    /// broker domain has no upstream dependency.
    pub fn case_from_intent(
        case_id: impl Into<String>,
        artifact_id: impl Into<String>,
        from: ShareState,
        to: ShareState,
        created_by: impl Into<String>,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> BrokerCase {
        use super::broker_case::SubjectKind;
        BrokerCase::new(
            case_id,
            CaseCategory::ContributorMeshShare,
            SubjectRef {
                kind: SubjectKind::WorkArtifact,
                id: artifact_id.into(),
                from_state: Some(from),
                to_state: Some(to),
            },
            title,
            summary,
            created_by,
            50,
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::broker::broker_case::{CaseCategory, SubjectKind, SubjectRef};

    fn fresh_case(author: &str) -> BrokerCase {
        BrokerCase::new(
            "case-1",
            CaseCategory::ContributorMeshShare,
            SubjectRef {
                kind: SubjectKind::WorkArtifact,
                id: "art-1".into(),
                from_state: Some(ShareState::Private),
                to_state: Some(ShareState::Team),
            },
            "t",
            "s",
            author,
            50,
        )
    }

    #[test]
    fn all_six_variants_have_stable_action_str() {
        assert_eq!(DecisionOutcome::Approve.action_str(), "approve");
        assert_eq!(DecisionOutcome::Reject.action_str(), "reject");
        assert_eq!(
            DecisionOutcome::Amend { diff: "x".into() }.action_str(),
            "amend"
        );
        assert_eq!(
            DecisionOutcome::Delegate {
                delegate_to: "x".into()
            }
            .action_str(),
            "delegate"
        );
        assert_eq!(
            DecisionOutcome::Promote {
                pattern_id: "x".into()
            }
            .action_str(),
            "promote"
        );
        assert_eq!(
            DecisionOutcome::Precedent { scope: "x".into() }.action_str(),
            "precedent"
        );
    }

    #[test]
    fn from_action_maps_acsp_vocabulary() {
        // Approval synonyms all collapse to Approve (agrees with the durable
        // store's `classify`).
        for v in ["approve", "approved", "Accept", "accepted"] {
            assert_eq!(
                DecisionOutcome::from_action(v, None),
                Some(DecisionOutcome::Approve)
            );
        }
        // Reject prefix matches reject/rejected.
        assert_eq!(
            DecisionOutcome::from_action("reject", None),
            Some(DecisionOutcome::Reject)
        );
        assert_eq!(
            DecisionOutcome::from_action("rejected", None),
            Some(DecisionOutcome::Reject)
        );
        // Payload-carrying variants need their detail; absent detail fails the
        // mapping rather than fabricating an empty payload.
        assert_eq!(DecisionOutcome::from_action("amend", None), None);
        assert_eq!(
            DecisionOutcome::from_action("amend", Some("patch")),
            Some(DecisionOutcome::Amend {
                diff: "patch".into()
            })
        );
        assert_eq!(DecisionOutcome::from_action("delegate", None), None);
        assert_eq!(
            DecisionOutcome::from_action("delegate", Some("carol")),
            Some(DecisionOutcome::Delegate {
                delegate_to: "carol".into()
            })
        );
        // Unknown verb is not a decision outcome.
        assert_eq!(DecisionOutcome::from_action("frobnicate", None), None);
    }

    #[test]
    fn from_action_round_trips_action_str() {
        for oc in [
            DecisionOutcome::Approve,
            DecisionOutcome::Reject,
            DecisionOutcome::Amend { diff: "d".into() },
            DecisionOutcome::Delegate {
                delegate_to: "x".into(),
            },
            DecisionOutcome::Precedent { scope: "s".into() },
        ] {
            let back = DecisionOutcome::from_action(oc.action_str(), Some("d")).unwrap();
            assert_eq!(back.action_str(), oc.action_str());
        }
    }

    #[test]
    fn approve_on_contributor_mesh_share_emits_plan() {
        let mut c = fresh_case("alice");
        c.claim("bob").unwrap();
        let orch = DecisionOrchestrator::new();
        let report = orch
            .decide(&mut c, "dec-1", DecisionOutcome::Approve, "bob", "ok")
            .unwrap();
        let plan = report.share_plan.expect("plan required");
        assert_eq!(plan.from, ShareState::Private);
        assert_eq!(plan.to, ShareState::Team);
        assert_eq!(plan.approved_by, "bob");
    }

    #[test]
    fn reject_produces_no_share_plan() {
        let mut c = fresh_case("alice");
        c.claim("bob").unwrap();
        let orch = DecisionOrchestrator::new();
        let report = orch
            .decide(&mut c, "dec-1", DecisionOutcome::Reject, "bob", "nope")
            .unwrap();
        assert!(report.share_plan.is_none());
    }

    #[test]
    fn self_review_rejected_via_orchestrator() {
        let mut c = fresh_case("alice");
        // Force UnderReview so we exercise the orchestrator's invariant
        // pass-through rather than the `claim` guard.
        c.state = super::super::broker_case::CaseState::UnderReview;
        c.assigned_to = Some("alice".into());
        let orch = DecisionOrchestrator::new();
        let err = orch
            .decide(&mut c, "dec-1", DecisionOutcome::Approve, "alice", "ok")
            .unwrap_err();
        assert!(matches!(err, OrchestrationError::Invariant(_)));
    }

    #[test]
    fn delegate_flow_transitions_state() {
        let mut c = fresh_case("alice");
        c.claim("bob").unwrap();
        let orch = DecisionOrchestrator::new();
        orch.decide(
            &mut c,
            "dec-1",
            DecisionOutcome::Delegate {
                delegate_to: "carol".into(),
            },
            "bob",
            "reassign",
        )
        .unwrap();
        assert_eq!(c.state, super::super::broker_case::CaseState::Delegated);
    }

    #[test]
    fn share_intent_adapter_produces_correct_category() {
        let c = ShareIntentBrokerAdapter::case_from_intent(
            "case-x",
            "art-x",
            ShareState::Private,
            ShareState::Mesh,
            "alice",
            "Promote",
            "Summary",
        );
        assert_eq!(c.category, CaseCategory::ContributorMeshShare);
        assert_eq!(c.subject.from_state, Some(ShareState::Private));
        assert_eq!(c.subject.to_state, Some(ShareState::Mesh));
    }

    #[test]
    fn invalid_share_transition_rejected() {
        let mut c = fresh_case("alice");
        c.subject.from_state = Some(ShareState::Mesh);
        c.subject.to_state = Some(ShareState::Private); // backwards
        c.claim("bob").unwrap();
        let orch = DecisionOrchestrator::new();
        let err = orch
            .decide(&mut c, "dec-1", DecisionOutcome::Approve, "bob", "ok")
            .unwrap_err();
        assert!(matches!(
            err,
            OrchestrationError::ShareTransitionRejected(_)
        ));
    }
}
