//! Nostr Bead Provenance Publisher
//!
//! Publishes bead lifecycle events (kind 30001) to a Nostr relay for
//! cryptographic provenance tracking. Each completed brief → debrief cycle
//! emits one signed event carrying the bead_id, brief_id, user pubkey, and
//! debrief path as tags.
//!
//! The event uses kind 30001 (parameterized replaceable, NIP-33) with the
//! bead_id as the `d` tag — so a re-published bead overwrites the previous
//! entry on the relay rather than creating duplicates.
//!
//! Provenance persistence is queued for Phase 2 (ADR-11): the former Neo4j
//! write path has been removed. Future versions will write provenance triples
//! to the Oxigraph quad-store via OxigraphOntologyRepository.
//!   todo!("Phase 2: Oxigraph provenance triples for NostrBeadPublisher")
//!
//! Configure via environment:
//!   VISIONCLAW_NOSTR_PRIVKEY  — 64-char hex secret key for the bridge bot
//!   NOSTR_RELAY_URL           — WebSocket relay URL (default: ws://localhost:7000)

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use log::{debug, error, warn};
use nostr_sdk::prelude::*;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Publishes provenance events to the configured Nostr relay.
#[derive(Clone)]
pub struct NostrBeadPublisher {
    keys: Keys,
    relay_url: String,
    _oxigraph_provenance: Option<Arc<()>>,
}

impl NostrBeadPublisher {
    /// Load from environment. Returns `None` if `VISIONCLAW_NOSTR_PRIVKEY` is absent.
    pub fn from_env() -> Option<Self> {
        let privkey = std::env::var("VISIONCLAW_NOSTR_PRIVKEY")
            .ok()
            .filter(|s| !s.is_empty())?;
        let relay_url =
            std::env::var("NOSTR_RELAY_URL").unwrap_or_else(|_| "ws://localhost:7000".to_string());

        // Validate relay URL scheme to prevent SSRF via env var injection.
        if !relay_url.starts_with("ws://") && !relay_url.starts_with("wss://") {
            error!(
                "[NostrBeadPublisher] NOSTR_RELAY_URL must start with ws:// or wss://: {relay_url}"
            );
            return None;
        }

        let secret_key = SecretKey::from_hex(&privkey)
            .map_err(|e| error!("[NostrBeadPublisher] Invalid VISIONCLAW_NOSTR_PRIVKEY: {e}"))
            .ok()?;

        Some(Self {
            keys: Keys::new(secret_key),
            relay_url,
            _oxigraph_provenance: None,
        })
    }

    /// Inject a graph handle for provenance writes.
    /// Phase 2 (ADR-11): will accept an OxigraphOntologyRepository handle.
    pub fn with_provenance_store(self, _graph: Arc<()>) -> Self {
        self
    }

    /// Publish a bead-complete provenance event (kind 30001, parameterized replaceable).
    ///
    /// Fire-and-forget: callers should `tokio::spawn` this so it does not block
    /// the debrief response path.
    pub async fn publish_bead_complete(
        &self,
        bead_id: &str,
        brief_id: &str,
        user_pubkey: Option<&str>,
        debrief_path: &str,
    ) {
        let mut tags = vec![
            Tag::custom(
                TagKind::Custom("h".into()),
                vec!["visionclaw-activity".to_string()],
            ),
            // `d` tag makes this parameterized replaceable — deduped by (pubkey, kind, d)
            Tag::custom(TagKind::Custom("d".into()), vec![bead_id.to_string()]),
            Tag::custom(TagKind::Custom("bead_id".into()), vec![bead_id.to_string()]),
            Tag::custom(
                TagKind::Custom("brief_id".into()),
                vec![brief_id.to_string()],
            ),
            Tag::custom(
                TagKind::Custom("debrief_path".into()),
                vec![debrief_path.to_string()],
            ),
        ];

        if let Some(pk) = user_pubkey {
            tags.push(Tag::custom(
                TagKind::Custom("user_pubkey".into()),
                vec![pk.to_string()],
            ));
        }

        let event = match EventBuilder::new(Kind::Custom(30001), "")
            .tags(tags)
            .sign_with_keys(&self.keys)
        {
            Ok(e) => e,
            Err(e) => {
                error!("[NostrBeadPublisher] Failed to sign bead event: {e}");
                return;
            }
        };

        match self.send_to_relay(&event).await {
            Ok(()) => {
                debug!(
                    "[NostrBeadPublisher] Published bead {bead_id} (event {})",
                    event.id
                );
                // Oxigraph provenance write: Phase 2 (ADR-11) — not yet implemented.
                // todo!("Phase 2: write SPARQL provenance triples via OxigraphOntologyRepository")
            }
            Err(e) => warn!("[NostrBeadPublisher] Relay publish failed for bead {bead_id}: {e}"),
        }
    }

    async fn send_to_relay(&self, event: &Event) -> Result<(), String> {
        let (ws_stream, _) = connect_async(&self.relay_url)
            .await
            .map_err(|e| format!("connect {}: {e}", self.relay_url))?;

        let (mut write, mut read) = ws_stream.split();

        write
            .send(Message::Text(json!(["EVENT", event]).to_string()))
            .await
            .map_err(|e| format!("send failed: {e}"))?;

        // Wait up to 5 s for the relay's OK response.
        tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
            while let Some(Ok(Message::Text(txt))) = read.next().await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&txt) {
                    if parsed[0] == "OK" {
                        return if parsed[2].as_bool().unwrap_or(false) {
                            Ok(())
                        } else {
                            Err(format!("relay rejected: {}", parsed[3]))
                        };
                    }
                }
            }
            Err("relay closed without OK".to_string())
        })
        .await
        .map_err(|_| "relay response timeout".to_string())?
    }
}
