//! # visionclaw-contracts
//!
//! Cross-boundary typed contracts for VisionClaw ⇄ agentbox ⇄ forum ⇄ XR client.
//!
//! This crate is the **single source of truth** for every envelope that
//! crosses a process boundary in the VisionClaw ecosystem. Rust consumers
//! depend on it directly. TypeScript consumers depend on the generated
//! `@visionclaw/contracts` npm package, whose `.d.ts` files are emitted from
//! the types here via the `typescript-export` feature.
//!
//! ## Modules
//!
//! - [`agent_action`] — outbound `AgentActionEnvelope` (click → agentbox).
//!   Canonical: ADR-10 §D3.
//! - [`telemetry`] — inbound `AgentTelemetryEnvelope` (agentbox → VisionClaw).
//!   Canonical: ADR-10 §D1.
//! - [`enterprise`] — inbound `EnterpriseEventEnvelope` (forum → VisionClaw).
//!   Canonical: ADR-10 §D5.
//! - [`github_adapter`] — `ParsedMarkdown` boundary value-object between the
//!   GitHub transport (Section 10) and the ontology domain (Section 8).
//!   Canonical: ADR-10 §D11 + DDD-08.
//! - [`version`] — schema-version constants. Canonical: ADR-10 §D8.
//!
//! ## Versioning
//!
//! Every envelope carries `schema_version`. Bump rules in ADR-10 §D8:
//! backwards-compatible field additions stay at v1; incompatible changes
//! bump the version and require coordinated deploys. Both sides keep one
//! back-version of support so the deploy window is non-zero.
//!
//! ## TypeScript export
//!
//! Build with `--features typescript-export` and run
//! `cargo test --features typescript-export ts_export` to emit `.d.ts` files
//! to `crates/visionclaw-contracts/bindings/`. See `tests/ts_export.rs`.

#![deny(unsafe_code)]
#![warn(rust_2018_idioms)]
// Field- and variant-level doc-comments are kept brief throughout this crate;
// the module-level doc-comments carry the normative semantics. We intentionally
// do not enable `#![warn(missing_docs)]` because the canonical specifications
// (ADR-10, the T7 resolution) are the source of truth — duplicating them
// field-by-field would create drift.

pub mod agent_action;
pub mod enterprise;
pub mod github_adapter;
pub mod telemetry;
pub mod version;

// ---------------------------------------------------------------------------
// Curated top-level re-exports
// ---------------------------------------------------------------------------
//
// These are the symbols every consumer imports first. Module-level imports
// are still available for callers that want to disambiguate (e.g.
// `agent_action::ActionKind` vs. some hypothetical future kind enum).

pub use crate::version::{SCHEMA_VERSION, SCHEMA_VERSION_STRING};

pub use crate::agent_action::{
    ActionKind, AgentAction, AgentActionEnvelope, AgentActionTargetOrigin, ModifierKeys, NodeClass,
    WorldPosition, AGENT_ACTION_CHANNEL, AGENT_ACTION_DEEP_LINK_TEMPLATE, AGENT_ACTION_TYPE,
};

pub use crate::telemetry::{
    AgentRecord, AgentRemovedPayload, AgentStatus, AgentTelemetryEnvelope, AgentTelemetryEvent,
    CommunicationPayload, DeltaPayload, DepartReason, HeartbeatPayload, SnapshotPayload,
};

pub use crate::enterprise::{
    EnterpriseEventEnvelope, EnterpriseEventKind, EnterpriseRole, MembershipAction,
    MembershipChangePayload, RoleChangePayload, SessionRevokedPayload,
};

pub use crate::github_adapter::{ParseErrorKind, ParseErrorReport, ParsedMarkdown};
