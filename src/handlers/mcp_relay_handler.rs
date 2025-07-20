use actix::{prelude::*, Actor, AsyncContext, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{info, error, debug, warn};
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::sync::Mutex;

/// MCP Relay Actor - Relays WebSocket messages between frontend and orchestrator
pub struct MCPRelayActor {
    /// Client ID for logging
    client_id: String,
    /// Connection to the orchestrator WebSocket
    orchestrator_tx: Option<Arc<Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, TungsteniteMessage>>>>,
    /// Actor address for sending messages back to this actor
    self_addr: Option<Addr<Self>>,
}

impl MCPRelayActor {
    pub fn new() -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            orchestrator_tx: None,
            self_addr: None,
        }
    }

    /// Connect to the orchestrator WebSocket
    fn connect_to_orchestrator(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        let orchestrator_url = std::env::var("MCP_ORCHESTRATOR_WS_URL")
            .unwrap_or_else(|_| "ws://orchestrator:9001".to_string());
        
        info!("[MCP Relay] Connecting to orchestrator at {}", orchestrator_url);
        
        let client_id = self.client_id.clone();
        let addr = ctx.address();
        
        // Spawn async task to connect to orchestrator
        actix::spawn(async move {
            match connect_async(&orchestrator_url).await {
                Ok((ws_stream, _)) => {
                    info!("[MCP Relay] Connected to orchestrator for client {}", client_id);
                    
                    let (tx, mut rx) = ws_stream.split();
                    let tx = Arc::new(Mutex::new(tx));
                    
                    // Send the transmitter back to the actor
                    addr.do_send(OrchestratorConnected(tx.clone()));
                    
                    // Handle messages from orchestrator
                    while let Some(msg) = rx.next().await {
                        match msg {
                            Ok(TungsteniteMessage::Text(text)) => {
                                debug!("[MCP Relay] Received text from orchestrator: {}", text);
                                addr.do_send(OrchestratorMessage(text));
                            }
                            Ok(TungsteniteMessage::Binary(bin)) => {
                                debug!("[MCP Relay] Received binary from orchestrator: {} bytes", bin.len());
                                addr.do_send(OrchestratorBinary(bin));
                            }
                            Ok(TungsteniteMessage::Close(_)) => {
                                info!("[MCP Relay] Orchestrator connection closed");
                                break;
                            }
                            Ok(TungsteniteMessage::Ping(data)) => {
                                if let Ok(mut tx) = tx.lock().await {
                                    let _ = tx.send(TungsteniteMessage::Pong(data)).await;
                                }
                            }
                            Ok(TungsteniteMessage::Pong(_)) => {
                                // Pong received, connection is alive
                            }
                            Ok(_) => {}
                            Err(e) => {
                                error!("[MCP Relay] Error receiving from orchestrator: {}", e);
                                break;
                            }
                        }
                    }
                    
                    // Notify actor that orchestrator disconnected
                    addr.do_send(OrchestratorDisconnected);
                }
                Err(e) => {
                    error!("[MCP Relay] Failed to connect to orchestrator: {}", e);
                    // Retry after delay
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    addr.do_send(RetryOrchestratorConnection);
                }
            }
        });
    }
}

/// Message types for internal communication
#[derive(Message)]
#[rtype(result = "()")]
struct OrchestratorConnected(Arc<Mutex<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, TungsteniteMessage>>>);

#[derive(Message)]
#[rtype(result = "()")]
struct OrchestratorMessage(String);

#[derive(Message)]
#[rtype(result = "()")]
struct OrchestratorBinary(Vec<u8>);

#[derive(Message)]
#[rtype(result = "()")]
struct OrchestratorDisconnected;

#[derive(Message)]
#[rtype(result = "()")]
struct RetryOrchestratorConnection;

impl Actor for MCPRelayActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[MCP Relay] New client connected: {}", self.client_id);
        self.self_addr = Some(ctx.address());
        
        // Connect to orchestrator
        self.connect_to_orchestrator(ctx);
        
        // Send welcome message to client
        ctx.text(serde_json::json!({
            "type": "connection_established",
            "source": "mcp_relay",
            "clientId": self.client_id,
            "timestamp": chrono::Utc::now().timestamp_millis()
        }).to_string());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("[MCP Relay] Client disconnected: {}", self.client_id);
    }
}

/// Handle orchestrator connection established
impl Handler<OrchestratorConnected> for MCPRelayActor {
    type Result = ();

    fn handle(&mut self, msg: OrchestratorConnected, ctx: &mut Self::Context) {
        self.orchestrator_tx = Some(msg.0);
        
        // Notify client that orchestrator is connected
        ctx.text(serde_json::json!({
            "type": "orchestrator_connected",
            "timestamp": chrono::Utc::now().timestamp_millis()
        }).to_string());
    }
}

/// Handle messages from orchestrator
impl Handler<OrchestratorMessage> for MCPRelayActor {
    type Result = ();

    fn handle(&mut self, msg: OrchestratorMessage, ctx: &mut Self::Context) {
        // Forward to client
        ctx.text(msg.0);
    }
}

/// Handle binary messages from orchestrator
impl Handler<OrchestratorBinary> for MCPRelayActor {
    type Result = ();

    fn handle(&mut self, msg: OrchestratorBinary, ctx: &mut Self::Context) {
        // Forward to client
        ctx.binary(msg.0);
    }
}

/// Handle orchestrator disconnection
impl Handler<OrchestratorDisconnected> for MCPRelayActor {
    type Result = ();

    fn handle(&mut self, _: OrchestratorDisconnected, ctx: &mut Self::Context) {
        self.orchestrator_tx = None;
        
        // Notify client
        ctx.text(serde_json::json!({
            "type": "orchestrator_disconnected",
            "timestamp": chrono::Utc::now().timestamp_millis()
        }).to_string());
        
        // Attempt to reconnect
        ctx.run_later(std::time::Duration::from_secs(5), |act, ctx| {
            act.connect_to_orchestrator(ctx);
        });
    }
}

/// Handle retry connection
impl Handler<RetryOrchestratorConnection> for MCPRelayActor {
    type Result = ();

    fn handle(&mut self, _: RetryOrchestratorConnection, ctx: &mut Self::Context) {
        self.connect_to_orchestrator(ctx);
    }
}

/// Handle WebSocket messages from client
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MCPRelayActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                debug!("[MCP Relay] Received text from client: {}", text);
                
                // Forward to orchestrator if connected
                if let Some(tx) = &self.orchestrator_tx {
                    let tx = tx.clone();
                    let text_clone = text.to_string();
                    
                    actix::spawn(async move {
                        if let Ok(mut tx) = tx.lock().await {
                            if let Err(e) = tx.send(TungsteniteMessage::Text(text_clone)).await {
                                error!("[MCP Relay] Failed to send to orchestrator: {}", e);
                            }
                        }
                    });
                } else {
                    warn!("[MCP Relay] Client message received but orchestrator not connected");
                    ctx.text(serde_json::json!({
                        "type": "error",
                        "message": "Orchestrator not connected",
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    }).to_string());
                }
            }
            Ok(ws::Message::Binary(bin)) => {
                debug!("[MCP Relay] Received binary from client: {} bytes", bin.len());
                
                // Forward to orchestrator if connected
                if let Some(tx) = &self.orchestrator_tx {
                    let tx = tx.clone();
                    let bin_vec = bin.to_vec();
                    
                    actix::spawn(async move {
                        if let Ok(mut tx) = tx.lock().await {
                            if let Err(e) = tx.send(TungsteniteMessage::Binary(bin_vec)).await {
                                error!("[MCP Relay] Failed to send binary to orchestrator: {}", e);
                            }
                        }
                    });
                }
            }
            Ok(ws::Message::Ping(msg)) => {
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                // Client is alive
            }
            Ok(ws::Message::Close(reason)) => {
                info!("[MCP Relay] Client closing connection: {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

/// HTTP handler for MCP relay WebSocket endpoint
pub async fn mcp_relay_handler(
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    info!("[MCP Relay] New WebSocket connection request");
    
    // Create new relay actor
    let actor = MCPRelayActor::new();
    
    // Start WebSocket with the actor
    ws::start(actor, &req, stream)
}