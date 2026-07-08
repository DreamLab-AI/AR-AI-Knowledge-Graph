//! Judgment Broker domain kernel (ADR-041, superseded-in-part by ADR-110 +
//! ADR-130 Decision 2).
//!
//! This is the storage-agnostic kernel cherry-picked from the unmerged
//! `crashbug` branch onto the ACSP architecture `main` committed to. It exposes
//! the [`BrokerCase`] aggregate root, the [`broker_decision::DecisionOutcome`]
//! value object with its six canonical outcomes, the
//! [`broker_decision::DecisionOrchestrator`] domain service and the
//! [`precedent_registry::PrecedentRegistry`].
//!
//! What was deliberately left behind (ADR-130 Decision 2): the `crashbug`
//! `BrokerActor` transport and its Neo4j adapter. No Neo4j runs in this stack
//! (the graph store is Oxigraph plus SQLite), and ADR-110's stateless ACSP
//! producer replaced the actor transport. The kernel here carries only the
//! decision invariants and the graduated-outcome model; the case queue is
//! surfaced against the ACSP producer ([`crate::services::acsp`]) and the
//! enrichment REST fallback ([`crate::handlers::enrichment_proposals_handler`],
//! [`crate::handlers::broker_inbox_handler`]).
//!
//! Canonical naming (per ADR-057 reconciliation):
//! - Category for contributor work-artifact promotion flows:
//!   [`CaseCategory::ContributorMeshShare`].
//! - Concrete subject is disambiguated via the [`SubjectKind`] discriminator.
//! - Share state ladder: `Private → Team → Mesh`.
//!
//! The kernel's [`CaseCategory`], [`SubjectKind`] and
//! [`broker_decision::DecisionOutcome`] serialise to the same snake_case wire
//! vocabulary the ACSP producer emits in [`crate::services::acsp::events`]; the
//! `broker_kernel_reconciliation` test module there locks that equivalence.

pub mod broker_case;
pub mod broker_decision;
pub mod precedent_registry;

pub use broker_case::{
    BrokerCase, CaseCategory, CaseInvariantError, CaseState, DecisionHistoryEntry, ShareState,
    SubjectKind, SubjectRef,
};
pub use broker_decision::{
    DecisionOrchestrator, DecisionOutcome, DecisionOutcomeReport, OrchestrationError,
    ShareIntentBrokerAdapter, ShareTransitionPlan,
};
pub use precedent_registry::PrecedentRegistry;
