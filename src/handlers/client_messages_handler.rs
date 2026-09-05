use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{debug, info, warn};
use serde_json::json;
use std::time::{Duration, Instant};

use crate::AppState;

pub struct ClientMessagesWs {
    app_state: web::Data<AppState>,
    last_heartbeat: Instant,
}

impl ClientMessagesWs {
    pub fn new(app_state: web::Data<AppState>) -> Self {
        Self {
            app_state,
            last_heartbeat: Instant::now(),
        }
    }

    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Duration::from_secs(30), |act, ctx| {
            if Instant::now().duration_since(act.last_heartbeat) > Duration::from_secs(90) {
                warn!("Client messages WebSocket heartbeat timeout, disconnecting");
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    fn start_message_stream(&self, ctx: &mut ws::WebsocketContext<Self>) {
        let rx = self.app_state.client_message_rx.clone();

        ctx.run_interval(Duration::from_millis(100), move |_act, ctx| {
            if let Ok(mut receiver) = rx.try_lock() {
                while let Ok(msg) = receiver.try_recv() {
                    let json = json!({
                        "type": "client_message",
                        "content": msg.content,
                        "timestamp": msg.timestamp.to_rfc3339(),
                        "session_id": msg.session_id,
                        "agent_id": msg.agent_id
                    });

                    ctx.text(json.to_string());
                    debug!("Forwarded client message to WebSocket: {}", msg.content);
                }
            }
        });
    }
}

impl Actor for ClientMessagesWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Client messages WebSocket connection established");
        self.start_heartbeat(ctx);
        self.start_message_stream(ctx);

        let init_json = json!({
            "type": "init",
            "status": "connected",
            "message": "Client message stream ready"
        });
        ctx.text(init_json.to_string());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("Client messages WebSocket connection closed");
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ClientMessagesWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                debug!("Received WebSocket text: {}", text);
            }
            Ok(ws::Message::Close(reason)) => {
                info!("Client messages WebSocket closing: {:?}", reason);
                ctx.stop();
            }
            Ok(ws::Message::Binary(_)) => {
                warn!("Binary messages not supported on client messages stream");
            }
            Err(e) => {
                warn!("WebSocket protocol error: {}", e);
                ctx.stop();
            }
            _ => {}
        }
    }
}

pub async fn websocket_client_messages(
    req: HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    info!("New client messages WebSocket connection request");

    // SECURITY (ADR-2044, ADR-2058): WebSocket token validation at upgrade time.
    //
    // The token comes from the `Authorization: Bearer` header. The `?token=`
    // query fallback is compiled in for development builds ONLY: a session token
    // in a query string reaches proxy and access logs, which contradicts legacy
    // ADR-011's header-only stance.
    //
    // It was originally kept for one release on the assumption that XR and
    // native clients could not set headers on an upgrade. That assumption was
    // wrong and is corrected here: the Godot client authenticates POST-connect
    // via `nip98_authenticate_json` (`xr-client/rust/src/signer.rs:110-114`,
    // sending `{"type":"authenticate","event":"<base64>"}`), and neither
    // `xr-client/rust/src/` nor `client/src/services/` contains a `token=`
    // query use at all. With no in-tree consumer the query path is closed in
    // release rather than deprecated. vc-gpu-wire closed the equivalent sites in
    // `socket_flow_handler/` and `fastwebsockets_handler.rs` under ADR-2058.
    //
    // This previously accepted ANY non-empty string: it checked only that a
    // token was present, never that it named a live session, so `?token=x` was
    // sufficient to open the socket. It now resolves the token through
    // `NostrService::get_session`, which as of ADR-2044 also enforces the
    // `AUTH_TOKEN_EXPIRY` window. Unknown, expired and absent tokens all fail
    // closed to 401.
    {
        let token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        // ADR-2058: dev-only `?token=` fallback. Absent from release binaries.
        #[cfg(any(debug_assertions, feature = "dev-auth"))]
        let token = token.or_else(|| {
            let query = req.query_string();
            url::form_urlencoded::parse(query.as_bytes())
                .find(|(k, _)| k == "token")
                .map(|(_, v)| v.to_string())
        });

        let client_ip = req
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let unauthorised = |reason: &str| {
            log::warn!(
                "SECURITY: rejected WebSocket upgrade on /ws/client-messages from {client_ip} — {reason}"
            );
            Ok(HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "Authentication required"})))
        };

        let token = match token {
            Some(t) if !t.is_empty() => t,
            _ => return unauthorised("no bearer token or ?token= present"),
        };

        // Fail closed when the service is absent: without a session store there
        // is nothing to validate against, so the socket must not open.
        let Some(nostr_service) = app_state.nostr_service.clone() else {
            return unauthorised("no NostrService configured — cannot validate the session token");
        };

        if nostr_service.get_session(&token).await.is_none() {
            return unauthorised("token does not name a live, unexpired session");
        }
    }

    let resp = ws::start(ClientMessagesWs::new(app_state), &req, stream);

    match resp {
        Ok(response) => Ok(response),
        Err(e) => {
            warn!("Failed to establish client messages WebSocket: {}", e);
            Err(e)
        }
    }
}
