use crate::utils::json::to_json;
use crate::utils::time;
use actix::{Actor, AsyncContext};
use actix_web_actors::ws;
use chrono::Utc;
use log::warn;
use serde_json::json;
use std::time::{Duration, Instant};

/// ADR-031 item 4: Server-to-client directives carried in heartbeat pong frames.
///
/// When the server has a pending action for a connected client it populates
/// `get_pending_directives()`. The directive list is embedded in the next
/// `send_pong` call and cleared by the client on receipt.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "directive", rename_all = "snake_case")]
pub enum HeartbeatDirective {
    /// Ask the client to reload its configuration from the server.
    ReloadConfig,
    /// Ask the client to request a full graph/position sync.
    ForceFullSync,
    /// Notify the client that a newer server version is available.
    UpdateAvailable { version: String },
}

pub trait WebSocketHeartbeat: Actor<Context = ws::WebsocketContext<Self>>
where
    Self: Sized,
{
    fn get_client_id(&self) -> &str;

    fn get_last_heartbeat(&self) -> Instant;

    fn update_last_heartbeat(&mut self);

    /// ADR-031 item 4: Return and drain any pending server directives for this client.
    ///
    /// Default implementation returns an empty list. Override to drain
    /// `ReloadConfig`, `ForceFullSync`, or `UpdateAvailable` directives.
    fn get_pending_directives(&mut self) -> Vec<HeartbeatDirective> {
        Vec::new()
    }

    fn start_heartbeat(
        &self,
        ctx: &mut ws::WebsocketContext<Self>,
        ping_interval_secs: u64,
        timeout_secs: u64,
    ) where
        Self: actix::Actor<Context = ws::WebsocketContext<Self>> + 'static,
    {
        let ping_duration = Duration::from_secs(ping_interval_secs);
        let timeout_duration = Duration::from_secs(timeout_secs);

        ctx.run_interval(ping_duration, move |act, ctx| {
            if Instant::now().duration_since(act.get_last_heartbeat()) > timeout_duration {
                warn!(
                    "WebSocket client {} heartbeat timeout, disconnecting",
                    act.get_client_id()
                );

                ctx.close(Some(ws::CloseReason {
                    code: ws::CloseCode::Abnormal,
                    description: Some("Heartbeat timeout".to_string()),
                }));
                return;
            }

            ctx.ping(b"heartbeat");
        });
    }

    fn send_ping(&self, ctx: &mut ws::WebsocketContext<Self>)
    where
        Self: actix::Actor<Context = ws::WebsocketContext<Self>>,
    {
        let ping_message = json!({
            "type": "ping",
            "timestamp": time::now(),
            "client_id": self.get_client_id()
        });

        if let Ok(msg) = to_json(&ping_message) {
            ctx.text(msg);
        }
    }

    fn send_pong(&mut self, ctx: &mut ws::WebsocketContext<Self>)
    where
        Self: actix::Actor<Context = ws::WebsocketContext<Self>>,
    {
        // ADR-031 item 4: embed pending directives in pong so the server can
        // push operational commands (ReloadConfig, ForceFullSync, etc.) to
        // clients at heartbeat time without a separate message type.
        let directives = self.get_pending_directives();
        let pong_message = if directives.is_empty() {
            json!({
                "type": "pong",
                "timestamp": time::now(),
                "client_id": self.get_client_id()
            })
        } else {
            json!({
                "type": "pong",
                "timestamp": time::now(),
                "client_id": self.get_client_id(),
                "directives": directives
            })
        };

        if let Ok(msg) = to_json(&pong_message) {
            ctx.text(msg);
        }
    }

    fn handle_heartbeat_message(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut ws::WebsocketContext<Self>,
    ) -> bool
    where
        Self: actix::Actor<Context = ws::WebsocketContext<Self>>,
    {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.update_last_heartbeat();
                ctx.pong(&msg);
                true
            }
            Ok(ws::Message::Pong(_)) => {
                self.update_last_heartbeat();
                true
            }
            _ => false,
        }
    }
}

pub struct HeartbeatConfig {
    pub ping_interval_secs: u64,
    pub timeout_secs: u64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            ping_interval_secs: 5,
            timeout_secs: 30,
        }
    }
}

impl HeartbeatConfig {
    pub fn new(ping_interval_secs: u64, timeout_secs: u64) -> Self {
        Self {
            ping_interval_secs,
            timeout_secs,
        }
    }

    pub fn fast() -> Self {
        Self::new(2, 10)
    }

    pub fn slow() -> Self {
        Self::new(15, 60)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "type")]
pub enum CommonWebSocketMessage {
    #[serde(rename = "ping")]
    Ping {
        timestamp: chrono::DateTime<Utc>,
        client_id: String,
    },

    #[serde(rename = "pong")]
    Pong {
        timestamp: chrono::DateTime<Utc>,
        client_id: String,
        /// ADR-031 item 4: Server directives piggy-backed on the pong frame.
        /// Omitted from JSON when empty to keep normal pong frames minimal.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        directives: Vec<HeartbeatDirective>,
    },

    #[serde(rename = "connection_established")]
    ConnectionEstablished {
        client_id: String,
        timestamp: chrono::DateTime<Utc>,
    },

    #[serde(rename = "error")]
    Error {
        message: String,
        timestamp: chrono::DateTime<Utc>,
    },
}
