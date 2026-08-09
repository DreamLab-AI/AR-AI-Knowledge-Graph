//! TypeScript-export integration test.
//!
//! Gated on the `typescript-export` feature: default `cargo test` does not
//! pull `ts-rs` or attempt to write `.d.ts` files. CI runs:
//!
//! ```bash
//! cargo test -p visionclaw-contracts --features typescript-export ts_export
//! ```
//!
//! and then asserts the resulting files in `crates/visionclaw-contracts/bindings/`
//! match `client/src/types/contracts/` byte-for-byte (regenerated on every
//! version bump per ADR-10 §D8).
//!
//! When the feature is disabled the file compiles to a no-op stub, so it
//! still participates in `cargo check`.

#![cfg(feature = "typescript-export")]

use ts_rs::TS;
use visionclaw_contracts::{
    agent_action::{
        ActionKind, AgentAction, AgentActionEnvelope, ModifierKeys, NodeClass, WorldPosition,
    },
    enterprise::{
        EnterpriseEventEnvelope, EnterpriseEventKind, EnterpriseRole, MembershipAction,
        MembershipChangePayload, RoleChangePayload, SessionRevokedPayload,
    },
    github_adapter::{ParseErrorKind, ParseErrorReport, ParsedMarkdown},
    telemetry::{
        AgentRecord, AgentRemovedPayload, AgentStatus, AgentTelemetryEnvelope, AgentTelemetryEvent,
        CommunicationPayload, DeltaPayload, HeartbeatPayload, SnapshotPayload,
    },
};

/// Drives the ts-rs `export!` machinery for every public type and asserts
/// that each .d.ts file lands at the expected path.
#[test]
fn ts_export_writes_all_top_level_types() {
    // ts-rs derives a per-type `export()` that writes to the path supplied
    // by `#[ts(export_to = ...)]`, defaulting to `bindings/<TypeName>.ts`.
    AgentActionEnvelope::export_all().expect("AgentActionEnvelope export");
    AgentAction::export_all().expect("AgentAction export");
    ActionKind::export_all().expect("ActionKind export");
    NodeClass::export_all().expect("NodeClass export");
    ModifierKeys::export_all().expect("ModifierKeys export");
    WorldPosition::export_all().expect("WorldPosition export");

    AgentTelemetryEnvelope::export_all().expect("AgentTelemetryEnvelope export");
    AgentTelemetryEvent::export_all().expect("AgentTelemetryEvent export");
    SnapshotPayload::export_all().expect("SnapshotPayload export");
    DeltaPayload::export_all().expect("DeltaPayload export");
    AgentRemovedPayload::export_all().expect("AgentRemovedPayload export");
    HeartbeatPayload::export_all().expect("HeartbeatPayload export");
    CommunicationPayload::export_all().expect("CommunicationPayload export");
    AgentRecord::export_all().expect("AgentRecord export");
    AgentStatus::export_all().expect("AgentStatus export");

    EnterpriseEventEnvelope::export_all().expect("EnterpriseEventEnvelope export");
    EnterpriseEventKind::export_all().expect("EnterpriseEventKind export");
    MembershipChangePayload::export_all().expect("MembershipChangePayload export");
    MembershipAction::export_all().expect("MembershipAction export");
    RoleChangePayload::export_all().expect("RoleChangePayload export");
    EnterpriseRole::export_all().expect("EnterpriseRole export");
    SessionRevokedPayload::export_all().expect("SessionRevokedPayload export");

    ParsedMarkdown::export_all().expect("ParsedMarkdown export");
    ParseErrorReport::export_all().expect("ParseErrorReport export");
    ParseErrorKind::export_all().expect("ParseErrorKind export");
}
