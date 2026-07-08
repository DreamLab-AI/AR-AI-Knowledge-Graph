//! BrokerCase aggregate root (ADR-041) with ADR-057 canonical category.
//!
//! A `BrokerCase` is the broker's unit of work. It wraps a foreign subject
//! (a workflow proposal, a skill package, a share-intent, an escalation) and
//! owns its lifecycle: Open → UnderReview → Decided, with explicit terminal
//! states for Delegated, Promoted, and Precedent paths.
//!
//! Invariants enforced here (fail-closed at the aggregate):
//! 1. **Append-only decision history.** Decisions are never mutated or
//!    removed. Each recorded decision appends one `DecisionHistoryEntry`.
//! 2. **No self-review.** A broker cannot decide on a case they authored
//!    (i.e. `assigned_to` cannot equal `created_by`, and the decider pubkey
//!    cannot equal `created_by`).
//! 3. **Provenance chain.** Every decision references the case id, the prior
//!    decision id (if any), and the deciding broker pubkey.
//! 4. **Terminal state idempotency.** Once `Decided`/`Closed`, no further
//!    decisions are accepted (they would produce `CaseInvariantError`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use super::broker_decision::DecisionOutcome;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CaseInvariantError {
    #[error("self-review forbidden: broker {broker} is the case creator")]
    SelfReview { broker: String },

    #[error("case already terminal in state {0:?}; no further decisions allowed")]
    AlreadyTerminal(CaseState),

    #[error("provenance chain broken: expected prior decision {expected:?}, got {actual:?}")]
    BrokenProvenance {
        expected: Option<String>,
        actual: Option<String>,
    },

    #[error("invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: CaseState, to: CaseState },

    #[error("amendment outcome requires a non-empty diff")]
    MissingAmendmentDiff,

    #[error("delegation outcome requires a non-empty delegate pubkey")]
    MissingDelegateTarget,
}

// ---------------------------------------------------------------------------
// Value objects
// ---------------------------------------------------------------------------

/// Canonical broker case category per ADR-057 reconciliation.
///
/// `ContributorMeshShare` is the unified label for any case that moves a
/// contributor-owned artefact across the share-state ladder (Private → Team
/// → Mesh). The concrete subject is disambiguated by `SubjectKind`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseCategory {
    /// Contributor work-artifact promotion (canonical per ADR-057).
    ContributorMeshShare,
    /// Workflow proposal submitted for review (ADR-042 linkage).
    WorkflowReview,
    /// Policy-engine exception request (ADR-045 linkage).
    PolicyException,
    /// KPI drift alert (ADR-043 linkage).
    TrustAlert,
    /// Manual broker submission with no pre-existing upstream source.
    ManualSubmission,
    /// PRD-013 write-back gating: enrichment proposed by an agent or the
    /// discovery pipeline, requiring broker approval before committing back
    /// to the source pod/repository via the WriteBackSaga.
    KnowledgeEnrichment,
}

/// Discriminator for the subject referenced by a `ContributorMeshShare` case.
/// Pairs with the upstream aggregate id inside `SubjectRef`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    /// BC18 `WorkArtifact` (contributor studio).
    WorkArtifact,
    /// BC19 `SkillPackage`.
    SkillPackage,
    /// Automation orchestrator proposal.
    AutomationProposal,
    /// Policy override request.
    PolicyException,
    /// Opaque / catch-all (used during migration).
    Opaque,
}

/// Three-rung share-state ladder per ADR-057 / ADR-051.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShareState {
    Private,
    Team,
    Mesh,
}

impl ShareState {
    /// Is the transition `self -> next` monotonic (forward only)?
    pub fn can_advance_to(self, next: ShareState) -> bool {
        matches!(
            (self, next),
            (ShareState::Private, ShareState::Team)
                | (ShareState::Team, ShareState::Mesh)
                | (ShareState::Private, ShareState::Mesh)
        )
    }
}

/// Opaque reference to the upstream subject.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRef {
    pub kind: SubjectKind,
    /// Upstream aggregate id (e.g. workflow-proposal id, skill-package id).
    pub id: String,
    /// Starting share state, used only for `ContributorMeshShare` cases.
    #[serde(default)]
    pub from_state: Option<ShareState>,
    /// Requested target state (if any).
    #[serde(default)]
    pub to_state: Option<ShareState>,
}

/// Aggregate lifecycle state (mirrors ADR-041 state diagram, expanded to
/// keep terminal paths distinct).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseState {
    Open,
    UnderReview,
    Decided,
    Delegated,
    Promoted,
    Precedent,
    Closed,
}

impl CaseState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            CaseState::Decided
                | CaseState::Delegated
                | CaseState::Promoted
                | CaseState::Precedent
                | CaseState::Closed
        )
    }
}

/// Append-only audit row for a single broker decision recorded on a case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionHistoryEntry {
    pub decision_id: String,
    pub outcome: DecisionOutcome,
    pub broker_pubkey: String,
    pub decided_at: DateTime<Utc>,
    /// Id of the preceding history entry (None for the first decision).
    pub prior_decision_id: Option<String>,
    pub reasoning: String,
}

// ---------------------------------------------------------------------------
// Aggregate root
// ---------------------------------------------------------------------------

/// The broker's unit of work. Identity = `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerCase {
    pub id: String,
    pub category: CaseCategory,
    pub subject: SubjectRef,
    pub title: String,
    pub summary: String,
    pub state: CaseState,
    pub priority: u8,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub assigned_to: Option<String>,
    pub history: Vec<DecisionHistoryEntry>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl BrokerCase {
    /// Factory. Cases start `Open` with no history.
    pub fn new(
        id: impl Into<String>,
        category: CaseCategory,
        subject: SubjectRef,
        title: impl Into<String>,
        summary: impl Into<String>,
        created_by: impl Into<String>,
        priority: u8,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            category,
            subject,
            title: title.into(),
            summary: summary.into(),
            state: CaseState::Open,
            priority,
            created_by: created_by.into(),
            created_at: now,
            updated_at: now,
            assigned_to: None,
            history: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Claim a case for review. No-op if already under review by the same
    /// broker; rejected if the broker is the original author (self-review
    /// guard).
    pub fn claim(&mut self, broker_pubkey: impl Into<String>) -> Result<(), CaseInvariantError> {
        let b = broker_pubkey.into();
        if b == self.created_by {
            return Err(CaseInvariantError::SelfReview { broker: b });
        }
        match self.state {
            CaseState::Open => {
                self.state = CaseState::UnderReview;
                self.assigned_to = Some(b);
                self.updated_at = Utc::now();
                Ok(())
            }
            CaseState::UnderReview => {
                // Idempotent re-claim by the same broker is fine.
                if self.assigned_to.as_deref() == Some(b.as_str()) {
                    Ok(())
                } else {
                    Err(CaseInvariantError::InvalidTransition {
                        from: self.state,
                        to: CaseState::UnderReview,
                    })
                }
            }
            other => Err(CaseInvariantError::AlreadyTerminal(other)),
        }
    }

    /// Release a claim back to the pool.
    pub fn release(&mut self) -> Result<(), CaseInvariantError> {
        if self.state != CaseState::UnderReview {
            return Err(CaseInvariantError::InvalidTransition {
                from: self.state,
                to: CaseState::Open,
            });
        }
        self.state = CaseState::Open;
        self.assigned_to = None;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Append a decision. Enforces all invariants.
    ///
    /// The terminal `CaseState` depends on the outcome:
    /// - Approve / Reject / Amend -> `Decided`
    /// - Delegate                 -> `Delegated`
    /// - Promote                  -> `Promoted`
    /// - Precedent                -> `Precedent`
    pub fn record_decision(
        &mut self,
        decision_id: impl Into<String>,
        outcome: DecisionOutcome,
        broker_pubkey: impl Into<String>,
        reasoning: impl Into<String>,
    ) -> Result<&DecisionHistoryEntry, CaseInvariantError> {
        let broker = broker_pubkey.into();

        // Invariant 2: no self-review.
        if broker == self.created_by {
            return Err(CaseInvariantError::SelfReview { broker });
        }

        // Invariant 4: terminal idempotency.
        if self.state.is_terminal() {
            return Err(CaseInvariantError::AlreadyTerminal(self.state));
        }

        // Outcome-specific payload checks.
        match &outcome {
            DecisionOutcome::Amend { diff } if diff.trim().is_empty() => {
                return Err(CaseInvariantError::MissingAmendmentDiff);
            }
            DecisionOutcome::Delegate { delegate_to } if delegate_to.trim().is_empty() => {
                return Err(CaseInvariantError::MissingDelegateTarget);
            }
            _ => {}
        }

        let prior_decision_id = self.history.last().map(|e| e.decision_id.clone());
        let entry = DecisionHistoryEntry {
            decision_id: decision_id.into(),
            outcome: outcome.clone(),
            broker_pubkey: broker,
            decided_at: Utc::now(),
            prior_decision_id,
            reasoning: reasoning.into(),
        };

        // Invariant 1: append-only.
        self.history.push(entry);

        // Transition state based on outcome.
        self.state = match outcome {
            DecisionOutcome::Approve | DecisionOutcome::Reject | DecisionOutcome::Amend { .. } => {
                CaseState::Decided
            }
            DecisionOutcome::Delegate { .. } => CaseState::Delegated,
            DecisionOutcome::Promote { .. } => CaseState::Promoted,
            DecisionOutcome::Precedent { .. } => CaseState::Precedent,
        };
        self.updated_at = Utc::now();

        Ok(self.history.last().expect("just pushed"))
    }

    /// Current latest decision id (the head of the provenance chain).
    pub fn latest_decision_id(&self) -> Option<&str> {
        self.history.last().map(|e| e.decision_id.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::broker::broker_decision::DecisionOutcome;

    fn sample_case(created_by: &str) -> BrokerCase {
        BrokerCase::new(
            "case-1",
            CaseCategory::ContributorMeshShare,
            SubjectRef {
                kind: SubjectKind::WorkArtifact,
                id: "art-1".into(),
                from_state: Some(ShareState::Private),
                to_state: Some(ShareState::Team),
            },
            "Promote artefact",
            "Move X to team pod",
            created_by,
            50,
        )
    }

    #[test]
    fn new_case_is_open_with_no_history() {
        let c = sample_case("alice");
        assert_eq!(c.state, CaseState::Open);
        assert!(c.history.is_empty());
        assert_eq!(c.category, CaseCategory::ContributorMeshShare);
    }

    #[test]
    fn self_review_rejected_on_claim() {
        let mut c = sample_case("alice");
        let err = c.claim("alice").unwrap_err();
        matches!(err, CaseInvariantError::SelfReview { .. });
    }

    #[test]
    fn self_review_rejected_on_decide() {
        let mut c = sample_case("alice");
        // Simulate state as if another broker claimed it.
        c.state = CaseState::UnderReview;
        c.assigned_to = Some("alice".into());
        let err = c
            .record_decision("dec-1", DecisionOutcome::Approve, "alice", "looks good")
            .unwrap_err();
        assert!(matches!(err, CaseInvariantError::SelfReview { .. }));
    }

    #[test]
    fn approval_flow_records_history() {
        let mut c = sample_case("alice");
        c.claim("bob").unwrap();
        let entry = c
            .record_decision("dec-1", DecisionOutcome::Approve, "bob", "ok")
            .unwrap()
            .clone();
        assert_eq!(entry.decision_id, "dec-1");
        assert_eq!(c.state, CaseState::Decided);
        assert_eq!(c.history.len(), 1);
        assert_eq!(c.latest_decision_id(), Some("dec-1"));
    }

    #[test]
    fn terminal_state_rejects_further_decisions() {
        let mut c = sample_case("alice");
        c.claim("bob").unwrap();
        c.record_decision("dec-1", DecisionOutcome::Approve, "bob", "ok")
            .unwrap();
        let err = c
            .record_decision("dec-2", DecisionOutcome::Reject, "bob", "changed mind")
            .unwrap_err();
        assert!(matches!(err, CaseInvariantError::AlreadyTerminal(_)));
    }

    #[test]
    fn amend_requires_diff() {
        let mut c = sample_case("alice");
        c.claim("bob").unwrap();
        let err = c
            .record_decision(
                "dec-1",
                DecisionOutcome::Amend { diff: "   ".into() },
                "bob",
                "fix",
            )
            .unwrap_err();
        assert_eq!(err, CaseInvariantError::MissingAmendmentDiff);
    }

    #[test]
    fn delegate_requires_target() {
        let mut c = sample_case("alice");
        c.claim("bob").unwrap();
        let err = c
            .record_decision(
                "dec-1",
                DecisionOutcome::Delegate {
                    delegate_to: "".into(),
                },
                "bob",
                "reassign",
            )
            .unwrap_err();
        assert_eq!(err, CaseInvariantError::MissingDelegateTarget);
    }

    #[test]
    fn share_state_monotonic() {
        assert!(ShareState::Private.can_advance_to(ShareState::Team));
        assert!(ShareState::Team.can_advance_to(ShareState::Mesh));
        assert!(ShareState::Private.can_advance_to(ShareState::Mesh));
        assert!(!ShareState::Team.can_advance_to(ShareState::Private));
        assert!(!ShareState::Mesh.can_advance_to(ShareState::Team));
    }

    #[test]
    fn provenance_chain_links_prior_decision() {
        let mut c = sample_case("alice");
        c.claim("bob").unwrap();
        c.record_decision(
            "dec-1",
            DecisionOutcome::Delegate {
                delegate_to: "carol".into(),
            },
            "bob",
            "handoff",
        )
        .unwrap();
        // After delegation the case state is terminal (Delegated); simulate
        // re-opening for downstream review by resetting back to Open (the
        // broker service would do this during reassignment). Once reopened,
        // the next decision's prior-id should link back to dec-1.
        c.state = CaseState::UnderReview;
        c.assigned_to = Some("carol".into());
        c.record_decision("dec-2", DecisionOutcome::Approve, "carol", "ok")
            .unwrap();
        assert_eq!(c.history[1].prior_decision_id.as_deref(), Some("dec-1"));
    }
}
