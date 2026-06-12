//! ACSP relay client: signs unsigned ACSP events with the panel keypair and
//! publishes them to the forum relay; subscribes to kind-31403 ActionResponse
//! events so agentic actors receive human decisions.
//!
//! Built on `nostr_sdk::Client` (relay pool with automatic reconnection, OK
//! acknowledgement handling, subscription streams) — no hand-rolled relay
//! websockets.
//!
//! The relay only accepts kinds 31400-31402 from pubkeys registered in its
//! `agent_registry` D1 table; [`AcspClient::pubkey_hex`] is logged at startup
//! so an admin can register it (`POST /api/governance/agents/register`,
//! NIP-98 admin-gated). Until registered, publishes fail with
//! `blocked: pubkey not in agent registry` — surfaced as an error here.

use log::{debug, info, warn};
use nostr_sdk::prelude::*;

use super::events::{ActionResponse, UnsignedAcspEvent, KIND_ACTION_RESPONSE};

/// A human decision delivered for a previously opened broker case.
#[derive(Debug, Clone)]
pub struct CaseDecision {
    /// The case id (the 31402 d-tag this response answers).
    pub case_id: String,
    /// `approve` / `reject` / `amend` / `delegate`.
    pub action: String,
    pub reasoning: String,
    /// Pubkey of the responding admin (relay enforces admin-only 31403).
    pub responder_pubkey: String,
}

pub struct AcspClient {
    keys: Keys,
    client: Client,
}

impl AcspClient {
    /// Build from a 64-hex secret key and connect the relay pool to the forum
    /// relay.
    pub async fn connect(secret_hex: &str, forum_relay_url: &str) -> Result<Self, String> {
        let secret_key =
            SecretKey::from_hex(secret_hex).map_err(|e| format!("ACSP secret key: {e}"))?;
        let keys = Keys::new(secret_key);
        let client = Client::new(keys.clone());
        client
            .add_relay(forum_relay_url)
            .await
            .map_err(|e| format!("ACSP add_relay: {e}"))?;
        client.connect().await;
        info!(
            "[ACSP] panel identity {} connected to {} (register this pubkey in the relay agent_registry)",
            keys.public_key().to_hex(),
            forum_relay_url
        );
        Ok(Self { keys, client })
    }

    /// x-only pubkey (hex) — must be registered in the relay's agent_registry.
    pub fn pubkey_hex(&self) -> String {
        self.keys.public_key().to_hex()
    }

    /// Sign an unsigned ACSP event and publish it, awaiting relay
    /// acknowledgement from the pool.
    pub async fn publish(&self, ev: &UnsignedAcspEvent) -> Result<String, String> {
        let tags: Vec<Tag> = ev
            .tags
            .iter()
            .filter(|t| !t.is_empty())
            .map(|t| Tag::custom(TagKind::Custom(t[0].clone().into()), t[1..].to_vec()))
            .collect();

        let event = EventBuilder::new(Kind::Custom(ev.kind), &ev.content)
            .tags(tags)
            .sign_with_keys(&self.keys)
            .map_err(|e| format!("ACSP sign failed: {e}"))?;

        let output = self
            .client
            .send_event(&event)
            .await
            .map_err(|e| format!("ACSP publish failed: {e}"))?;
        if output.success.is_empty() {
            let reason = output
                .failed
                .values()
                .next()
                .cloned()
                .unwrap_or_else(|| "no relay accepted the event".into());
            return Err(format!("forum rejected: {reason}"));
        }
        debug!("[ACSP] published kind {} event {}", ev.kind, output.id());
        Ok(output.id().to_hex())
    }

    /// Long-lived subscription for kind-31403 ActionResponse events whose
    /// d-tag starts with `case_prefix` (each agentic actor namespaces its case
    /// ids, e.g. `vc-elev-`). Decisions are delivered through `sink`. The SDK
    /// pool owns reconnection; this future runs until the receiver side of
    /// `sink` drops.
    pub async fn run_decision_subscription(
        &self,
        case_prefix: String,
        sink: tokio::sync::mpsc::UnboundedSender<CaseDecision>,
    ) {
        let filter = Filter::new()
            .kind(Kind::Custom(KIND_ACTION_RESPONSE))
            .since(Timestamp::now());
        if let Err(e) = self.client.subscribe(filter, None).await {
            warn!("[ACSP] decision subscribe failed: {e}");
            return;
        }

        let mut notifications = self.client.notifications();
        loop {
            match notifications.recv().await {
                Ok(RelayPoolNotification::Event { event, .. }) => {
                    if let Some(decision) = decision_from_event(&event, &case_prefix) {
                        if sink.send(decision).is_err() {
                            info!("[ACSP] decision sink closed; subscription ending");
                            return;
                        }
                    }
                }
                Ok(RelayPoolNotification::Shutdown) => {
                    warn!("[ACSP] relay pool shut down; decision subscription ending");
                    return;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("[ACSP] decision stream lagged, skipped {n} notifications");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    }
}

/// Convert a kind-31403 event into a [`CaseDecision`] when its d-tag falls in
/// our case namespace.
pub fn decision_from_event(event: &Event, case_prefix: &str) -> Option<CaseDecision> {
    if event.kind != Kind::Custom(KIND_ACTION_RESPONSE) {
        return None;
    }
    let case_id = event
        .tags
        .iter()
        .find(|t| t.kind() == TagKind::SingleLetter(SingleLetterTag::lowercase(Alphabet::D)))
        .and_then(|t| t.content())?
        .to_string();
    if !case_id.starts_with(case_prefix) {
        return None;
    }
    let content: ActionResponse = serde_json::from_str(&event.content).ok()?;
    Some(CaseDecision {
        case_id,
        action: content.action,
        reasoning: content.reasoning,
        responder_pubkey: event.pubkey.to_hex(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_response(case_id: &str, action: &str) -> Event {
        let keys = Keys::generate();
        EventBuilder::new(
            Kind::Custom(KIND_ACTION_RESPONSE),
            serde_json::json!({"action": action, "reasoning": "Human approve via governance UI"})
                .to_string(),
        )
        .tags([Tag::identifier(case_id)])
        .sign_with_keys(&keys)
        .unwrap()
    }

    #[test]
    fn parses_matching_decision() {
        let ev = signed_response("vc-elev-42", "approve");
        let d = decision_from_event(&ev, "vc-elev-").unwrap();
        assert_eq!(d.case_id, "vc-elev-42");
        assert_eq!(d.action, "approve");
        assert_eq!(d.responder_pubkey, ev.pubkey.to_hex());
    }

    #[test]
    fn ignores_foreign_namespace_and_other_kinds() {
        assert!(decision_from_event(&signed_response("other-1", "approve"), "vc-elev-").is_none());
        let keys = Keys::generate();
        let wrong_kind = EventBuilder::new(Kind::Custom(31401), "{}")
            .tags([Tag::identifier("vc-elev-1")])
            .sign_with_keys(&keys)
            .unwrap();
        assert!(decision_from_event(&wrong_kind, "vc-elev-").is_none());
    }
}
