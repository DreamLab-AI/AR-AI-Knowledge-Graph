//! Nostr Bridge: source relay → DreamLab Forum relay
//!
//! Subscribes to a source Nostr relay for kind 30001 bead provenance events
//! and republishes them as NIP-29 group messages (kind 9) to the DreamLab
//! forum relay, where admin and whitelisted forum users can query them.
//!
//! The bridge re-signs events with its own keypair so it satisfies the forum
//! relay's whitelist check. The original event ID is preserved in a
//! `source_event` tag for cross-relay audit trails.
//!
//! Configure via environment:
//!   VISIONCLAW_NOSTR_PRIVKEY  — bridge bot signing key (same as publisher)
//!   NOSTR_RELAY_URL           — source relay URL (default: ws://localhost:7000)
//!   FORUM_RELAY_URL           — wss://relay.dreamlab-ai.com (required)
//!
//! When `solid-pod-embed` is enabled, the bridge reads from NOSTR_RELAY_URL
//! (the bead publisher's target) instead of the former JSS sidecar relay.
//!
//! The bridge is designed as a long-running background task. Spawn with:
//!   tokio::spawn(NostrBridge::from_env().unwrap().run());

use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use nostr_sdk::prelude::*;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct NostrBridge {
    keys: Keys,
    source_relay_url: String,
    forum_relay_url: String,
}

impl NostrBridge {
    /// Load from environment. Returns `None` if required vars are absent.
    pub fn from_env() -> Option<Self> {
        let privkey = std::env::var("VISIONCLAW_NOSTR_PRIVKEY")
            .ok()
            .filter(|s| !s.is_empty())?;
        let forum_relay_url = std::env::var("FORUM_RELAY_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let source_relay_url =
            std::env::var("NOSTR_RELAY_URL").unwrap_or_else(|_| "ws://localhost:7000".to_string());

        // Validate relay URL schemes to prevent SSRF via env var injection.
        if !source_relay_url.starts_with("ws://") && !source_relay_url.starts_with("wss://") {
            error!(
                "[NostrBridge] NOSTR_RELAY_URL must start with ws:// or wss://: {source_relay_url}"
            );
            return None;
        }
        if !forum_relay_url.starts_with("ws://") && !forum_relay_url.starts_with("wss://") {
            error!(
                "[NostrBridge] FORUM_RELAY_URL must start with ws:// or wss://: {forum_relay_url}"
            );
            return None;
        }

        let secret_key = SecretKey::from_hex(&privkey)
            .map_err(|e| error!("[NostrBridge] Invalid VISIONCLAW_NOSTR_PRIVKEY: {e}"))
            .ok()?;

        Some(Self {
            keys: Keys::new(secret_key),
            source_relay_url,
            forum_relay_url,
        })
    }

    /// Run the bridge loop indefinitely, reconnecting on failure.
    /// Call via `tokio::spawn(bridge.run())`.
    pub async fn run(self) {
        info!(
            "[NostrBridge] Starting {} → {}",
            self.source_relay_url, self.forum_relay_url
        );
        loop {
            match self.run_once().await {
                Ok(()) => warn!("[NostrBridge] Stream ended unexpectedly, reconnecting in 30s"),
                Err(e) => warn!("[NostrBridge] Connection lost ({e}), reconnecting in 30s"),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    async fn run_once(&self) -> Result<(), String> {
        let (source_stream, _) = connect_async(&self.source_relay_url)
            .await
            .map_err(|e| format!("Source relay connect failed: {e}"))?;

        let (mut src_write, mut src_read) = source_stream.split();

        // Subscribe to kind 30001 bead provenance events only.
        src_write
            .send(Message::Text(
                json!(["REQ", "bridge-sub", {"kinds": [30001]}]).to_string(),
            ))
            .await
            .map_err(|e| format!("REQ send failed: {e}"))?;

        info!("[NostrBridge] Subscribed to source relay for kind 30001");

        while let Some(msg) = src_read.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
                        // ["EVENT", "<sub_id>", <event_object>]
                        if parsed[0] == "EVENT" {
                            if let Some(event_obj) = parsed.get(2) {
                                self.forward_to_forum(event_obj).await;
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => return Err("Source relay closed connection".to_string()),
                Err(e) => return Err(format!("WebSocket error: {e}")),
                _ => {}
            }
        }

        Err("Source relay stream ended".to_string())
    }

    async fn forward_to_forum(&self, original: &Value) {
        // Verify the Nostr event signature before forwarding. Without this, any party
        // that can write to the source relay could inject arbitrary content into the forum.
        let event_json = match serde_json::to_string(original) {
            Ok(s) => s,
            Err(e) => {
                warn!("[NostrBridge] Failed to serialise event for verification: {e}");
                return;
            }
        };
        match Event::from_json(&event_json) {
            Err(e) => {
                warn!("[NostrBridge] Dropping unparseable event: {e}");
                return;
            }
            Ok(ev) => {
                if let Err(e) = ev.verify() {
                    warn!("[NostrBridge] Dropping event with invalid signature: {e}");
                    return;
                }
            }
        }

        let bead_id = tag_value(original, "bead_id").unwrap_or("unknown");
        let brief_id = tag_value(original, "brief_id").unwrap_or("-");
        let debrief_path = tag_value(original, "debrief_path").unwrap_or("-");
        let source_event_id = original["id"].as_str().unwrap_or("");

        let content = format!("bead:{bead_id} brief:{brief_id} path:{debrief_path}");

        let tags = vec![
            Tag::custom(
                TagKind::Custom("h".into()),
                vec!["visionclaw-activity".to_string()],
            ),
            Tag::custom(TagKind::Custom("bead_id".into()), vec![bead_id.to_string()]),
            Tag::custom(
                TagKind::Custom("source_event".into()),
                vec![source_event_id.to_string()],
            ),
        ];

        let event = match EventBuilder::new(Kind::Custom(9), &content)
            .tags(tags)
            .sign_with_keys(&self.keys)
        {
            Ok(e) => e,
            Err(e) => {
                error!("[NostrBridge] Failed to sign forum event: {e}");
                return;
            }
        };

        match self.send_to_forum(&event).await {
            Ok(()) => debug!("[NostrBridge] Forwarded bead {bead_id} to forum relay"),
            Err(e) => warn!("[NostrBridge] Failed to forward bead {bead_id}: {e}"),
        }
    }

    async fn send_to_forum(&self, event: &Event) -> Result<(), String> {
        let (ws_stream, _) = connect_async(&self.forum_relay_url)
            .await
            .map_err(|e| format!("forum connect: {e}"))?;

        let (mut write, mut read) = ws_stream.split();

        write
            .send(Message::Text(json!(["EVENT", event]).to_string()))
            .await
            .map_err(|e| format!("send failed: {e}"))?;

        tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
            while let Some(Ok(Message::Text(txt))) = read.next().await {
                if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
                    if parsed[0] == "OK" {
                        return if parsed[2].as_bool().unwrap_or(false) {
                            Ok(())
                        } else {
                            Err(format!("forum rejected: {}", parsed[3]))
                        };
                    }
                }
            }
            Err("forum relay closed without OK".to_string())
        })
        .await
        .map_err(|_| "forum relay timeout".to_string())?
    }
}

/// Extract the first value of a named tag from a raw Nostr event JSON object.
fn tag_value<'a>(event: &'a Value, tag_name: &str) -> Option<&'a str> {
    event["tags"].as_array()?.iter().find_map(|t| {
        if t[0].as_str() == Some(tag_name) {
            t[1].as_str()
        } else {
            None
        }
    })
}
