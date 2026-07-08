//! Provenance status + cross-namespace URN recording for the ingest hot path
//! (ADR-059 §5, closing the BC20 enforcement gap).
//!
//! Before this module, `ingest::process_frame` treated identity as silently
//! optional ("never reject on absence") and foreign `urn:agentbox:*` source /
//! target URNs rode through as opaque strings — the BC20 anti-corruption layer
//! was never invoked on the VisionClaw side, so a namespace crossing was lost at
//! the federation boundary.
//!
//! This module keeps the fail-open render posture (unauthenticated frames are
//! still accepted and still reach the beam path) but makes provenance
//! *recorded* rather than discarded:
//!   * [`classify`] stamps a [`ProvenanceStatus`] on every frame so the audit
//!     surface can distinguish signed from unsigned actions.
//!   * [`record_crossings`] translates any inbound `urn:agentbox:*` source /
//!     target URN through [`crate::uri::cross_from_agentbox`] (the BC20
//!     counterpart) so the crossing is stored, not dropped.

use serde::Serialize;

use crate::uri::{self, UrnCrossing};

use super::schema::AgentActionEnvelope;

/// Whether an inbound action is attributable to a sovereign identity, and to
/// what degree. The frame is accepted regardless (render compatibility); this is
/// the audit dimension that distinguishes attributed from unattributed frames.
///
/// SECURITY NOTE (audit truth): this is *structural attribution only*, not
/// signature verification. The `agent_action` wire envelope carries a `pubkey`
/// string but **no signature** — neither this repo's [`AgentActionEnvelope`]
/// schema nor the agentbox publisher (`createMcpNotification`) attaches or
/// verifies a Nostr signature. `Attributed` therefore means "a well-formed
/// identity was asserted", NOT "a valid signature was checked". Real
/// cryptographic verification needs a wire-contract change to carry `sig` + the
/// signed event fields (id/created_at/kind/tags/content) on the frame; that is
/// a cross-repo change tracked separately. The variant is deliberately named
/// `Attributed` (not `Signed`) so the trail never overstates trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStatus {
    /// A structurally-valid 64-hex pubkey attributes the action. Records that an
    /// identity was *asserted* — NOT that a signature was verified (see the
    /// type-level security note; the wire carries no signature to verify).
    Attributed,
    /// Identity asserted but malformed (wrong length / non-hex). Recorded as a
    /// distinct degraded state rather than collapsed into anonymous.
    Malformed,
    /// No identity asserted. Accepted for render, flagged unattributed for audit.
    Anonymous,
}

impl ProvenanceStatus {
    /// True iff the action carried a well-formed sovereign attribution. This is
    /// an attribution check, not a signature check (see the type-level note).
    pub fn is_attributed(self) -> bool {
        matches!(self, ProvenanceStatus::Attributed)
    }
}

/// Classify the provenance of an inbound envelope from its `pubkey` field.
///
/// This inspects only the *structure* of the asserted pubkey (64-hex). No
/// signature is verified because the envelope carries none — see the
/// [`ProvenanceStatus`] security note.
pub fn classify(event: &AgentActionEnvelope) -> ProvenanceStatus {
    match event.pubkey.as_deref() {
        Some(pk) if uri::is_pubkey_hex(pk) => ProvenanceStatus::Attributed,
        Some(_) => ProvenanceStatus::Malformed,
        None => ProvenanceStatus::Anonymous,
    }
}

/// The recorded provenance of one ingested frame: status + any namespace
/// crossings translated through BC20. Stored alongside the envelope so the audit
/// trail (ADR-059 Phase 3) sees signed/unsigned + the agentbox→visionclaw map.
#[derive(Debug, Clone, PartialEq)]
pub struct IngestProvenance {
    pub status: ProvenanceStatus,
    /// Translated source URN crossing, when the inbound `source_urn` was a
    /// foreign `urn:agentbox:*` (or already-converged `did:nostr:*`).
    pub source_crossing: Option<UrnCrossing>,
    /// Translated target URN crossing.
    pub target_crossing: Option<UrnCrossing>,
}

/// Translate the envelope's `source_urn` / `target_urn` through the BC20 bridge.
/// A converged `urn:visionclaw:*` URN that is already native (not agentbox) is
/// left untranslated (`None`) — only the federation crossing is recorded here.
pub fn record_crossings(event: &AgentActionEnvelope) -> (Option<UrnCrossing>, Option<UrnCrossing>) {
    let cross = |urn: &Option<String>| urn.as_deref().and_then(uri::cross_from_agentbox);
    (cross(&event.source_urn), cross(&event.target_urn))
}

/// Build the full provenance record for an inbound envelope.
pub fn record(event: &AgentActionEnvelope) -> IngestProvenance {
    let (source_crossing, target_crossing) = record_crossings(event);
    IngestProvenance {
        status: classify(event),
        source_crossing,
        target_crossing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PK: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn envelope(pubkey: Option<&str>, source: Option<&str>, target: Option<&str>) -> AgentActionEnvelope {
        AgentActionEnvelope {
            version: 3,
            id: 1,
            source_agent_id: 7,
            target_node_id: 42,
            action_type: 1,
            action_type_name: "update".into(),
            timestamp: 1748500000000,
            duration_ms: 100,
            source_urn: source.map(str::to_string),
            target_urn: target.map(str::to_string),
            pubkey: pubkey.map(str::to_string),
            token_count: None,
            handoff_id: None,
            verification: None,
            metadata: json!(null),
        }
    }

    #[test]
    fn classify_signed_malformed_anonymous() {
        assert_eq!(classify(&envelope(Some(PK), None, None)), ProvenanceStatus::Attributed);
        assert_eq!(classify(&envelope(Some("xyz"), None, None)), ProvenanceStatus::Malformed);
        assert_eq!(classify(&envelope(None, None, None)), ProvenanceStatus::Anonymous);
        assert!(ProvenanceStatus::Attributed.is_attributed());
        assert!(!ProvenanceStatus::Anonymous.is_attributed());
    }

    #[test]
    fn records_foreign_agentbox_source_crossing() {
        let e = envelope(
            Some(PK),
            Some(&format!("urn:agentbox:thing:{PK}:proposal-1")),
            Some(&format!("urn:agentbox:activity:{PK}:run-1")),
        );
        let p = record(&e);
        assert_eq!(p.status, ProvenanceStatus::Attributed);
        let sc = p.source_crossing.unwrap();
        assert!(sc.visionclaw_id.starts_with(&format!("urn:visionclaw:kg:{PK}:")));
        assert_eq!(sc.agentbox_urn, format!("urn:agentbox:thing:{PK}:proposal-1"));
        let tc = p.target_crossing.unwrap();
        assert!(tc.visionclaw_id.starts_with("urn:visionclaw:execution:"));
    }

    #[test]
    fn native_visionclaw_urn_is_not_a_crossing() {
        // An already-converged urn:visionclaw target is native, not a foreign
        // crossing — record_crossings leaves it untranslated.
        let e = envelope(
            Some(PK),
            None,
            Some(&format!("urn:visionclaw:kg:{PK}:sha256-12-deadbeef0011")),
        );
        let p = record(&e);
        assert!(p.source_crossing.is_none());
        assert!(p.target_crossing.is_none());
    }

    #[test]
    fn did_nostr_source_passes_through_as_crossing() {
        let e = envelope(Some(PK), Some(&format!("did:nostr:{PK}")), None);
        let p = record(&e);
        let sc = p.source_crossing.unwrap();
        assert_eq!(sc.visionclaw_id, format!("did:nostr:{PK}"));
        assert_eq!(sc.owner_did.as_deref(), Some(&*format!("did:nostr:{PK}")));
    }
}
