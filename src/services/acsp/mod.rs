//! Agent Control Surface Protocol (ACSP) producer — VisionClaw side.
//!
//! Lets agentic actors project interactive control panels into the DreamLab
//! forum governance page (`/community/governance`) and route human decisions
//! back, over Nostr kinds 31400-31405. See ADR-110 for the decision and
//! `docs/architecture/agent-control-surface-panels.md` for the pipeline.
//!
//! - [`events`] — serde-exact wire types + unsigned-event builders
//! - [`client`] — `nostr_sdk`-backed signing, publishing and the kind-31403
//!   decision return path

pub mod client;
pub mod events;

pub use client::{AcspClient, CaseDecision};
pub use events::{
    build_action_request, build_panel_definition, build_panel_retired, build_panel_state,
    build_panel_update, ActionPriority, ActionRequest, CaseCategory, CaseSpec, PanelDefinition,
    SubjectKind,
};
