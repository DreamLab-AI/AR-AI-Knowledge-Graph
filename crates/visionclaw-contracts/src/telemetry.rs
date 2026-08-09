//! Agent telemetry wire envelopes.
//!
//! Canonical specification: ADR-10 §D1. This module is the source of truth
//! for what arrives at the `/ws/agent-telemetry` endpoint (ADR-06 §D11 row
//! `/ws/agent-telemetry`).
//!
//! ## Anti-corruption layer (CC-1 resolution)
//!
//! The wire types here are NOT the internal domain events. DDD-07 owns the
//! internal events (`AgentJoined`, `AgentPositionUpdated`,
//! `AgentStatusChanged`, `AgentCommunicated`, `AgentDeparted`,
//! `SwarmSnapshot`, `Heartbeat`). The translation table is normative:
//!
//! | Wire (`type`)     | Internal event(s)                                  |
//! |-------------------|----------------------------------------------------|
//! | `snapshot`        | `SwarmSnapshot`                                    |
//! | `delta`           | fans out → `AgentPositionUpdated` and/or `AgentStatusChanged` per field |
//! | `agent_added`     | `AgentJoined`                                      |
//! | `agent_removed`   | `AgentDeparted`                                    |
//! | `heartbeat`       | `Heartbeat`                                        |
//! | `communication`   | `AgentCommunicated`                                |
//!
//! The translation lives in `src/handlers/telemetry_handler.rs` (Section 7
//! consumer). This crate exposes only the wire shapes — the consumer maps
//! them into domain events.
//!
//! ## Failure modes (ADR-10 §D1)
//!
//! - Unknown `type` → log once, ignore, continue (consumer responsibility).
//! - Schema version skew → close frame `4001 schema_version_unsupported`.
//! - Missing required fields → drop frame, increment
//!   `telemetry_malformed_count`.
//! - Back-pressure → drop, never queue (`telemetry_dropped_frames_total`).

use serde::{Deserialize, Serialize};

#[cfg(feature = "typescript-export")]
use ts_rs::TS;

pub use crate::version::SCHEMA_VERSION;

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

/// Top-level telemetry envelope arriving on `/ws/agent-telemetry`.
///
/// Discriminated union by `type`; payload shape varies. Producers always
/// emit `schema_version` at the envelope root so consumers can reject
/// unsupported versions before parsing the payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "typescript-export",
    derive(TS),
    ts(export, export_to = "AgentTelemetryEnvelope.ts")
)]
pub struct AgentTelemetryEnvelope {
    pub schema_version: u32,
    /// agentbox session UUID. Constant for the lifetime of one WS connection.
    pub session_id: String,
    /// Monotonically increasing frame counter, per session.
    pub frame_id: u64,
    /// Unix milliseconds at frame emission.
    pub timestamp_ms: i64,
    /// Discriminated payload.
    #[serde(flatten)]
    pub event: AgentTelemetryEvent,
}

/// Discriminated wire event. The `tag = "type"` and `content = "payload"`
/// adjacent shape matches ADR-10 §D1's JSON literally:
/// `{"type": "snapshot", "payload": { ... }}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "typescript-export",
    derive(TS),
    ts(export, export_to = "AgentTelemetryEvent.ts")
)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AgentTelemetryEvent {
    /// Full agent set. Sent on connect and after every reconnect (ADR-10 §D2).
    Snapshot(SnapshotPayload),
    /// Incremental update for one agent. Consumer fans this out to one or
    /// more internal events per field changed.
    Delta(DeltaPayload),
    /// Topology change — one agent appeared.
    AgentAdded(AgentRecord),
    /// Topology change — one agent disappeared.
    AgentRemoved(AgentRemovedPayload),
    /// Connection-warming keepalive. No data.
    Heartbeat(HeartbeatPayload),
    /// Communication edge between two agents (CC-1 addition).
    Communication(CommunicationPayload),
}

// ---------------------------------------------------------------------------
// Payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct SnapshotPayload {
    pub agents: Vec<AgentRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct DeltaPayload {
    pub agent_id: String,
    /// Field set varies — opaque on the wire and parsed by the consumer.
    /// Modelled as `serde_json::Value` because the field-level schema is
    /// open-ended by design (ADR-10 §D1: "the next snapshot corrects it").
    #[cfg_attr(feature = "typescript-export", ts(type = "Record<string, unknown>"))]
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct AgentRemovedPayload {
    pub agent_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct HeartbeatPayload {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct CommunicationPayload {
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub weight: f64,
}

/// One row from a `snapshot` or one `agent_added` body. Internal events
/// (`AgentJoined`, `SwarmSnapshot`) project this into the DDD-07 aggregate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct AgentRecord {
    pub agent_id: String,
    /// Free-form agent kind label (`researcher`, `coder`, `tester`, …).
    /// Open-ended by design — the agentbox owns the taxonomy.
    pub kind: String,
    pub status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swarm_id: Option<String>,
    pub spawned_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity_ms: Option<i64>,
    /// Opaque agentbox metadata, passed through unmodified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "typescript-export",
        ts(type = "Record<string, unknown> | null")
    )]
    pub metadata: Option<serde_json::Value>,
}

/// Coarse agent status. Renderer maps these to colour / opacity (PRD-07 F4).
/// Wire encoding is `snake_case`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Spawning,
    Running,
    Idle,
    Stopped,
    Errored,
}

/// Reason an agent departed the topology. Currently a single normative value
/// because agentbox emits `agent_removed` without an explicit reason; the
/// enum exists so consumers can pattern-match exhaustively and future
/// reasons can be added without breaking the wire shape (additive).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum DepartReason {
    /// Default / unspecified — what `agent_removed` looks like at v1.
    Removed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshot_envelope_round_trips() {
        let env = AgentTelemetryEnvelope {
            schema_version: SCHEMA_VERSION,
            session_id: "session-1".into(),
            frame_id: 0,
            timestamp_ms: 1_715_856_000_000,
            event: AgentTelemetryEvent::Snapshot(SnapshotPayload {
                agents: vec![AgentRecord {
                    agent_id: "agent-a".into(),
                    kind: "researcher".into(),
                    status: AgentStatus::Running,
                    swarm_id: None,
                    spawned_at_ms: 1_715_856_000_000,
                    last_activity_ms: Some(1_715_856_001_000),
                    metadata: None,
                }],
            }),
        };
        let s = serde_json::to_value(&env).unwrap();
        assert_eq!(s["type"], "snapshot");
        assert_eq!(s["schema_version"], 1);
        assert_eq!(s["payload"]["agents"][0]["status"], "running");

        let back: AgentTelemetryEnvelope = serde_json::from_value(s).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn communication_event_wire_shape() {
        let env = AgentTelemetryEnvelope {
            schema_version: SCHEMA_VERSION,
            session_id: "s1".into(),
            frame_id: 5,
            timestamp_ms: 1_715_856_005_000,
            event: AgentTelemetryEvent::Communication(CommunicationPayload {
                from_agent_id: "a".into(),
                to_agent_id: "b".into(),
                weight: 0.75,
            }),
        };
        let s = serde_json::to_value(&env).unwrap();
        assert_eq!(s["type"], "communication");
        assert_eq!(s["payload"]["from_agent_id"], "a");
        assert_eq!(s["payload"]["weight"], 0.75);
    }

    #[test]
    fn delta_payload_carries_open_ended_fields() {
        let env = AgentTelemetryEnvelope {
            schema_version: SCHEMA_VERSION,
            session_id: "s1".into(),
            frame_id: 1,
            timestamp_ms: 1_715_856_002_000,
            event: AgentTelemetryEvent::Delta(DeltaPayload {
                agent_id: "agent-a".into(),
                fields: json!({ "status": "idle", "last_activity_ms": 1_715_856_002_000_i64 }),
            }),
        };
        let s = serde_json::to_string(&env).unwrap();
        let back: AgentTelemetryEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(env, back);
    }
}
