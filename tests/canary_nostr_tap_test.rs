//! RES-a Nostr-tap mapper (PRD-023 WP-11 AC3, ADR-130 D3).
//!
//! Exercises the PURE event → observation mapping WITHOUT a live relay: each
//! test constructs a `TapEvent` (directly, or via `TapEvent::from_value` fed a
//! parsed relay frame) and asserts the `TapDecision`. No socket, no key
//! material, no harness — the crypto lives in the connection layer, and the
//! mapper only ever reads `TapEvent::sig_verified`.

use serde_json::json;
use visionclaw_server::services::canary_nostr_tap::{
    map_event_to_observation, TapDecision, TapEvent, LIVENESS_CANARY_KIND, LIVENESS_CANARY_TAG,
};

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MALLORY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn allow(pubkeys: &[&str]) -> Vec<String> {
    pubkeys.iter().map(|s| s.to_string()).collect()
}

/// A well-formed, signature-verified fire from `pubkey` for `canary_id`.
fn fire(pubkey: &str, canary_id: &str, evidence: &str) -> TapEvent {
    TapEvent {
        id: "eventid00".to_string(),
        pubkey: pubkey.to_string(),
        kind: LIVENESS_CANARY_KIND,
        tags: vec![vec!["t".to_string(), LIVENESS_CANARY_TAG.to_string()]],
        content: json!({ "canary_id": canary_id, "evidence": evidence }).to_string(),
        sig_verified: true,
    }
}

#[test]
fn accepts_a_valid_signed_allow_listed_fire() {
    let ev = fire(
        ALICE,
        "CANARY-VC-REC2-CASE",
        "broker:new_case then broker:case_decided",
    );
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Accepted {
            canary_id,
            evidence,
        } => {
            assert_eq!(canary_id, "CANARY-VC-REC2-CASE");
            // Evidence discloses provenance (source pubkey + event id) so the
            // fire log is honest that this fire arrived over the Nostr tap.
            assert!(
                evidence.contains(ALICE),
                "evidence must name the source pubkey"
            );
            assert!(
                evidence.contains("eventid00"),
                "evidence must name the event id"
            );
            assert!(
                evidence.contains("broker:new_case"),
                "evidence must carry the repo payload"
            );
        }
        other => panic!("expected Accepted, got {other:?}"),
    }
}

#[test]
fn rejects_an_unverified_signature() {
    let mut ev = fire(ALICE, "CANARY-VC-REC2-CASE", "x");
    ev.sig_verified = false;
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Rejected { reason } => assert!(reason.contains("unverified signature")),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn rejects_the_wrong_kind() {
    let mut ev = fire(ALICE, "CANARY-VC-REC2-CASE", "x");
    ev.kind = 31402; // an ACSP control kind, not a fire announcement
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Rejected { reason } => assert!(reason.contains("kind 31402")),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn rejects_a_missing_liveness_canary_tag() {
    let mut ev = fire(ALICE, "CANARY-VC-REC2-CASE", "x");
    ev.tags = vec![vec!["t".to_string(), "some-other-topic".to_string()]];
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Rejected { reason } => assert!(reason.contains(LIVENESS_CANARY_TAG)),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn rejects_a_pubkey_not_on_the_allow_list() {
    let ev = fire(MALLORY, "CANARY-VC-REC2-CASE", "x");
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Rejected { reason } => assert!(reason.contains("not allow-listed")),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn rejects_every_fire_when_the_allow_list_is_empty() {
    let ev = fire(ALICE, "CANARY-VC-REC2-CASE", "x");
    match map_event_to_observation(&ev, &[]) {
        TapDecision::Rejected { reason } => assert!(reason.contains("no allow-listed pubkeys")),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn rejects_malformed_content_json() {
    let mut ev = fire(ALICE, "CANARY-VC-REC2-CASE", "x");
    ev.content = "not json at all".to_string();
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Rejected { reason } => assert!(reason.contains("malformed content JSON")),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn rejects_an_empty_canary_id() {
    let ev = fire(ALICE, "   ", "x");
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Rejected { reason } => assert!(reason.contains("canary_id is empty")),
        other => panic!("expected Rejected, got {other:?}"),
    }
}

#[test]
fn evidence_falls_back_when_the_repo_supplies_none() {
    let mut ev = fire(ALICE, "CANARY-VC-RESA-KG", "");
    // content with canary_id but no evidence field at all
    ev.content = json!({ "canary_id": "CANARY-VC-RESA-KG" }).to_string();
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Accepted {
            canary_id,
            evidence,
        } => {
            assert_eq!(canary_id, "CANARY-VC-RESA-KG");
            assert!(evidence.contains("(no evidence provided)"));
        }
        other => panic!("expected Accepted, got {other:?}"),
    }
}

#[test]
fn allow_list_match_is_case_insensitive() {
    // Relays and clients differ on hex casing; the allow-list compares
    // case-insensitively so an upper-cased pubkey still matches.
    let ev = fire(&ALICE.to_uppercase(), "CANARY-VC-RESA-KG", "x");
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Accepted { .. } => {}
        other => panic!("expected Accepted for case-differing pubkey, got {other:?}"),
    }
}

#[test]
fn from_value_parses_a_raw_relay_event_object() {
    // A raw Nostr event object as it arrives in ["EVENT", sub, {..}]. from_value
    // is fed the parsed frame (no relay), and the mapper accepts it.
    let obj = json!({
        "id": "abc123",
        "pubkey": ALICE,
        "created_at": 1_700_000_000u64,
        "kind": 1,
        "tags": [["t", LIVENESS_CANARY_TAG], ["client", "solid-pod-rs"]],
        "content": json!({ "canary_id": "CANARY-VC-REC11-TRACE", "evidence": "pod provenance trail" }).to_string(),
        "sig": "00"
    });
    let ev = TapEvent::from_value(&obj, true).expect("well-formed event parses");
    assert_eq!(ev.pubkey, ALICE);
    assert_eq!(ev.kind, 1);
    assert!(ev
        .tags
        .iter()
        .any(|t| t == &vec!["t".to_string(), LIVENESS_CANARY_TAG.to_string()]));
    match map_event_to_observation(&ev, &allow(&[ALICE])) {
        TapDecision::Accepted { canary_id, .. } => assert_eq!(canary_id, "CANARY-VC-REC11-TRACE"),
        other => panic!("expected Accepted, got {other:?}"),
    }
}

#[test]
fn from_value_rejects_a_shapeless_event() {
    // Missing pubkey/kind → not even a candidate fire.
    let obj = json!({ "id": "x", "content": "{}" });
    assert!(TapEvent::from_value(&obj, true).is_none());
}
