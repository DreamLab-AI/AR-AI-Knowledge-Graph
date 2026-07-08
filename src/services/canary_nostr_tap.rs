// src/services/canary_nostr_tap.rs
//! Nostr-relay tap for the [`LivenessHarness`](crate::services::liveness_harness)
//! (RES-a, PRD-023 WP-11 acceptance criterion 3, ADR-130 Decision 3).
//!
//! Nostr-only repositories — `nostr-rust-forum` and `solid-pod-rs` (DDD
//! cross-repo table: they "fire … canaries against this context's
//! `LivenessHarness` over the Nostr tap") — cannot `POST /api/canary/observe`
//! because they speak only Nostr. This tap subscribes to a relay, verifies the
//! events those repositories publish, and maps each accepted fire onto the SAME
//! [`LivenessHarness::observe`](crate::services::liveness_harness::LivenessHarness::observe)
//! path the HTTP route uses. It registers nothing new: a fire lands only on a
//! canary already declared through `register`/`seed_p0_canaries`.
//!
//! ## Wire contract
//!
//! The docs leave the exact tap wire format open ("by subscribing to the wires
//! they already emit"), so this uses the convention fixed for the gap-close:
//! a fire is a **kind-1** note carrying a `["t","liveness-canary"]` tag whose
//! `content` is JSON `{ "canary_id": "...", "evidence": "..." }`, published by a
//! pubkey on the allow-list. This is deliberately close to ACSP (kinds
//! 31400–31405, ADR-110) without colliding with a control kind.
//!
//! ## Configuration (both required for the tap to run)
//!   * `CANARY_TAP_RELAY_URL`      — `ws://`/`wss://` relay to subscribe to.
//!     **Unset ⇒ the tap is disabled** ([`CanaryNostrTap::from_env`] → `None`).
//!   * `CANARY_TAP_ALLOWED_PUBKEYS` — comma-separated x-only hex pubkeys. An
//!     empty list rejects every fire (fail-closed on identity).
//!
//! ## Trust split (why the mapper is pure)
//!
//! The connection layer does the crypto: it parses the relay frame into a
//! [`nostr_sdk::Event`] and runs the BIP-340 signature check, recording only the
//! boolean outcome on [`TapEvent::sig_verified`]. The decision — signature,
//! kind, tag, allow-list, content shape → accept/reject — is
//! [`map_event_to_observation`], a **pure function** unit-tested by feeding it
//! parsed events, with no relay and no key material.
//!
//! ## Failure posture
//!
//! Fail-open: a relay that is down, slow, or absent never blocks the server. The
//! tap runs on its own detached task, reconnects with exponential backoff, and
//! logs every accepted and rejected fire.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use nostr_sdk::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::adapters::sqlite_canary_repository::CanaryStoreError;
use crate::services::liveness_harness::LivenessHarness;

/// The Nostr event kind a liveness-canary fire is published as.
pub const LIVENESS_CANARY_KIND: u16 = 1;
/// The `t`-tag value that marks a note as a liveness-canary fire.
pub const LIVENESS_CANARY_TAG: &str = "liveness-canary";

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const SUBSCRIPTION_ID: &str = "canary-tap";

/// A Nostr event reduced to exactly the fields the tap decision needs. Built
/// from a parsed relay frame; `sig_verified` is stamped by the connection layer
/// after a BIP-340 check so the pure mapper can gate on signature validity
/// WITHOUT performing any cryptography itself.
#[derive(Debug, Clone)]
pub struct TapEvent {
    pub id: String,
    /// x-only hex pubkey of the publisher.
    pub pubkey: String,
    pub kind: u16,
    /// Raw tag rows, each already flattened to its string elements.
    pub tags: Vec<Vec<String>>,
    pub content: String,
    /// Whether the connection layer verified this event's signature.
    pub sig_verified: bool,
}

impl TapEvent {
    /// Reduce a raw Nostr event JSON object (the third element of an `["EVENT",
    /// sub, {...}]` frame) to a [`TapEvent`]. Returns `None` when the mandatory
    /// shape is absent (no `pubkey`, no numeric `kind`) — a structurally invalid
    /// event cannot be a fire. `sig_verified` is supplied by the caller.
    pub fn from_value(obj: &Value, sig_verified: bool) -> Option<Self> {
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pubkey = obj.get("pubkey")?.as_str()?.to_string();
        let kind = u16::try_from(obj.get("kind")?.as_u64()?).ok()?;
        let content = obj
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tags = obj
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.as_array())
                    .map(|row| {
                        row.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<Vec<String>>>()
            })
            .unwrap_or_default();
        Some(Self {
            id,
            pubkey,
            kind,
            tags,
            content,
            sig_verified,
        })
    }

    /// True iff a tag row `[name, value, ..]` is present.
    fn has_tag(&self, name: &str, value: &str) -> bool {
        self.tags
            .iter()
            .any(|row| row.len() >= 2 && row[0] == name && row[1] == value)
    }
}

/// The JSON `content` a repository publishes: which canary fired and the
/// evidence for it.
#[derive(Debug, Deserialize)]
struct CanaryFireContent {
    canary_id: String,
    #[serde(default)]
    evidence: String,
}

/// Outcome of mapping one tap event. `Accepted` carries exactly what the
/// [`LivenessHarness::observe`] call needs; `Rejected` carries a loggable
/// reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapDecision {
    Accepted { canary_id: String, evidence: String },
    Rejected { reason: String },
}

/// PURE event → observation mapping. No IO, no relay, no crypto — it reads only
/// its two arguments. This is the unit-tested core of the tap (feed it parsed
/// [`TapEvent`]s). Rejection order is deliberate: cheapest / most-security-
/// relevant checks first so a rejected event never reaches content parsing.
pub fn map_event_to_observation(ev: &TapEvent, allowed_pubkeys: &[String]) -> TapDecision {
    if !ev.sig_verified {
        return TapDecision::Rejected {
            reason: format!("unverified signature (event {})", ev.id),
        };
    }
    if ev.kind != LIVENESS_CANARY_KIND {
        return TapDecision::Rejected {
            reason: format!("kind {} is not the fire kind {}", ev.kind, LIVENESS_CANARY_KIND),
        };
    }
    if !ev.has_tag("t", LIVENESS_CANARY_TAG) {
        return TapDecision::Rejected {
            reason: format!("missing [\"t\",\"{LIVENESS_CANARY_TAG}\"] tag"),
        };
    }
    if allowed_pubkeys.is_empty() {
        return TapDecision::Rejected {
            reason: "no allow-listed pubkeys configured (CANARY_TAP_ALLOWED_PUBKEYS empty)"
                .to_string(),
        };
    }
    if !allowed_pubkeys
        .iter()
        .any(|p| p.eq_ignore_ascii_case(&ev.pubkey))
    {
        return TapDecision::Rejected {
            reason: format!("pubkey {} is not allow-listed", ev.pubkey),
        };
    }
    let content: CanaryFireContent = match serde_json::from_str(&ev.content) {
        Ok(c) => c,
        Err(e) => {
            return TapDecision::Rejected {
                reason: format!("malformed content JSON: {e}"),
            }
        }
    };
    if content.canary_id.trim().is_empty() {
        return TapDecision::Rejected {
            reason: "content.canary_id is empty".to_string(),
        };
    }
    let evidence = format!(
        "nostr-tap fire from {} (event {}): {}",
        ev.pubkey,
        ev.id,
        if content.evidence.trim().is_empty() {
            "(no evidence provided)"
        } else {
            content.evidence.trim()
        }
    );
    TapDecision::Accepted {
        canary_id: content.canary_id.trim().to_string(),
        evidence,
    }
}

/// Verify a raw Nostr event object's signature via `nostr_sdk`. Returns whether
/// the BIP-340 signature (and the derived event id) check out. An unparseable
/// event is treated as unverified. This is the ONLY cryptographic step, kept out
/// of the pure mapper.
fn verify_event_signature(obj: &Value) -> bool {
    match serde_json::to_string(obj)
        .ok()
        .and_then(|raw| Event::from_json(&raw).ok())
    {
        Some(ev) => ev.verify().is_ok(),
        None => false,
    }
}

/// The connection layer: subscribes to a relay and drives accepted fires into
/// the harness. Thin by design — all decision logic lives in
/// [`map_event_to_observation`].
pub struct CanaryNostrTap {
    harness: Arc<LivenessHarness>,
    relay_url: String,
    allowed_pubkeys: Vec<String>,
}

impl CanaryNostrTap {
    /// Build from the environment. Returns `None` — the tap stays disabled —
    /// when `CANARY_TAP_RELAY_URL` is unset or malformed. An empty allow-list is
    /// permitted but warned about (every fire will be rejected until pubkeys are
    /// listed).
    pub fn from_env(harness: Arc<LivenessHarness>) -> Option<Self> {
        let relay_url = std::env::var("CANARY_TAP_RELAY_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        if !relay_url.starts_with("ws://") && !relay_url.starts_with("wss://") {
            error!(
                "[canary-tap] CANARY_TAP_RELAY_URL must start with ws:// or wss://: {relay_url}"
            );
            return None;
        }
        let allowed_pubkeys = parse_allowed_pubkeys(
            &std::env::var("CANARY_TAP_ALLOWED_PUBKEYS").unwrap_or_default(),
        );
        if allowed_pubkeys.is_empty() {
            warn!(
                "[canary-tap] CANARY_TAP_RELAY_URL is set but CANARY_TAP_ALLOWED_PUBKEYS is empty \
                 — every fire will be rejected until pubkeys are allow-listed"
            );
        }
        Some(Self {
            harness,
            relay_url,
            allowed_pubkeys,
        })
    }

    /// Run forever: connect, subscribe, observe accepted fires, reconnect with
    /// exponential backoff. Fail-open — never returns under normal operation,
    /// never panics, never blocks the runtime. Spawn with `tokio::spawn`.
    pub async fn run(self) {
        info!(
            "[canary-tap] starting: relay={} allow_listed_pubkeys={} kind={} tag={}",
            self.relay_url,
            self.allowed_pubkeys.len(),
            LIVENESS_CANARY_KIND,
            LIVENESS_CANARY_TAG
        );
        let mut backoff = INITIAL_BACKOFF;
        loop {
            match self.run_once().await {
                Ok(()) => {
                    // Stream connected then ended — reset the backoff.
                    info!("[canary-tap] relay stream ended; reconnecting in {INITIAL_BACKOFF:?}");
                    backoff = INITIAL_BACKOFF;
                    tokio::time::sleep(backoff).await;
                }
                Err(e) => {
                    warn!("[canary-tap] relay error ({e}); reconnecting in {backoff:?}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
        }
    }

    /// One connect → subscribe → read loop. Returns `Ok(())` when the relay
    /// closes the stream cleanly, `Err` on a transport failure.
    async fn run_once(&self) -> Result<(), String> {
        let (stream, _) = connect_async(&self.relay_url)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        let (mut write, mut read) = stream.split();

        let mut filter = json!({
            "kinds": [LIVENESS_CANARY_KIND],
            "#t": [LIVENESS_CANARY_TAG],
        });
        // Narrow the subscription to allow-listed authors when we have any; the
        // mapper still enforces the allow-list authoritatively.
        if !self.allowed_pubkeys.is_empty() {
            filter["authors"] = json!(self.allowed_pubkeys);
        }
        write
            .send(Message::Text(
                json!(["REQ", SUBSCRIPTION_ID, filter]).to_string(),
            ))
            .await
            .map_err(|e| format!("REQ send: {e}"))?;
        info!(
            "[canary-tap] subscribed on {} (kind {LIVENESS_CANARY_KIND}, #t {LIVENESS_CANARY_TAG})",
            self.relay_url
        );

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
                        // ["EVENT", "<sub_id>", <event_object>]
                        if parsed.get(0).and_then(|v| v.as_str()) == Some("EVENT") {
                            if let Some(obj) = parsed.get(2) {
                                self.handle_event_obj(obj).await;
                            }
                        }
                    }
                }
                Ok(Message::Ping(payload)) => {
                    let _ = write.send(Message::Pong(payload)).await;
                }
                Ok(Message::Close(_)) => return Ok(()),
                Err(e) => return Err(format!("ws error: {e}")),
                _ => {}
            }
        }
        Ok(())
    }

    /// Verify, map and (on accept) observe a single relay event object. Logs
    /// every accepted and rejected fire.
    async fn handle_event_obj(&self, obj: &Value) {
        let sig_verified = verify_event_signature(obj);
        let tap_event = match TapEvent::from_value(obj, sig_verified) {
            Some(e) => e,
            None => {
                warn!("[canary-tap] rejected fire: malformed event shape (no pubkey/kind)");
                return;
            }
        };
        match map_event_to_observation(&tap_event, &self.allowed_pubkeys) {
            TapDecision::Accepted { canary_id, evidence } => {
                match self.harness.observe(&canary_id, &evidence).await {
                    Ok(fire_id) => info!(
                        "[canary-tap] accepted fire: canary={canary_id} fire_id={fire_id} \
                         pubkey={}",
                        tap_event.pubkey
                    ),
                    Err(CanaryStoreError::NotFound(_)) => warn!(
                        "[canary-tap] rejected fire: canary '{canary_id}' is not registered \
                         (the tap registers nothing new)"
                    ),
                    Err(e) => warn!(
                        "[canary-tap] rejected fire: observe failed for '{canary_id}': {e}"
                    ),
                }
            }
            TapDecision::Rejected { reason } => {
                warn!("[canary-tap] rejected fire from {}: {reason}", tap_event.pubkey);
            }
        }
    }
}

/// Split a comma-separated pubkey list into trimmed, non-empty entries.
fn parse_allowed_pubkeys(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
