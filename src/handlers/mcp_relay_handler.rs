use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde_json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use log::{debug, error, info, warn};
use std::time::Duration;

#[derive(Message)]
#[rtype(result = "()")]
struct ClientText(String);

#[derive(Message)]
#[rtype(result = "()")]
struct ClientBinary(Vec<u8>);

#[derive(Message)]
#[rtype(result = "()")]
struct OrchestratorText(String);

#[derive(Message)]
#[rtype(result = "()")]
struct OrchestratorBinary(Vec<u8>);

pub struct MCPRelayActor {
    client_id: String,
    orchestrator_tx: Option<Arc<Mutex<SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, TungsteniteMessage>>>>,
    self_addr: Option<Addr<Self>>,
}

impl MCPRelayActor {
    fn new() -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            orchestrator_tx: None,
            self_addr: None,
        }
    }
    
    fn connect_to_orchestrator(&mut self, ctx: &mut <Self as Actor>::Context) {
        let orchestrator_url = std::env::var("ORCHESTRATOR_WS_URL")
            .unwrap_or_else(|_| "ws://powerdev:3000/ws".to_string());
            
        info!("[MCP Relay] Connecting to orchestrator at: {}", orchestrator_url);
        let addr = ctx.address();
        self.self_addr = Some(addr.clone());
        
        actix::spawn(async move {
            match connect_async(&orchestrator_url).await {
                Ok((ws_stream, _)) => {
                    info!("[MCP Relay] Connected to orchestrator");
                    let (tx, mut rx) = ws_stream.split();
                    let tx = Arc::new(Mutex::new(tx));
                    
                    // Send connection success to actor
                    addr.do_send(OrchestratorText("connected".to_string()));
                    
                    // Forward messages from orchestrator to client
                    while let Some(msg) = rx.next().await {
                        match msg {
                            Ok(TungsteniteMessage::Text(text)) => {
                                addr.do_send(OrchestratorText(text));
                            }
                            Ok(TungsteniteMessage::Binary(bin)) => {
                                addr.do_send(OrchestratorBinary(bin));
                            }
                            Ok(TungsteniteMessage::Close(_)) => {
                                info!("[MCP Relay] Orchestrator connection closed");
                                break;
                            }
                            Ok(TungsteniteMessage::Ping(data)) => {
                                let tx_clone = tx.clone();
                                actix::spawn(async move {
                                    let mut tx_guard = tx_clone.lock().await;
                                    let _ = tx_guard.send(TungsteniteMessage::Pong(data)).await;
                                });
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
                    
                    info!("[MCP Relay] Orchestrator connection handler ended");
                }
                Err(e) => {
                    error!("[MCP Relay] Failed to connect to orchestrator: {}", e);
                    // Retry after delay
                    actix::clock::sleep(Duration::from_secs(5)).await;
                    addr.do_send(OrchestratorText("retry".to_string()));
                }
            }
        });
    }
}

impl Actor for MCPRelayActor {
    type Context = ws::WebsocketContext<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[MCP Relay] Actor started for client: {}", self.client_id);
        
        // Start heartbeat
        ctx.run_interval(Duration::from_secs(30), |_act, ctx| {
            ctx.ping(b"");
        });
        
        // Connect to orchestrator
        self.connect_to_orchestrator(ctx);
    }
    
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("[MCP Relay] Actor stopped for client: {}", self.client_id);
    }
}

// Handle messages from orchestrator
impl Handler<OrchestratorText> for MCPRelayActor {
    type Result = ();
    
    fn handle(&mut self, msg: OrchestratorText, ctx: &mut Self::Context) {
        match msg.0.as_str() {
            "connected" => {
                // Store orchestrator connection
                ctx.text(serde_json::json!({
                    "type": "orchestrator_connected",
                    "timestamp": chrono::Utc::now().timestamp_millis()
                }).to_string());
            }
            "retry" => {
                // Retry connection
                self.connect_to_orchestrator(ctx);
            }
            _ => {
                // Forward message to client
                ctx.text(msg.0);
            }
        }
    }
}

impl Handler<OrchestratorBinary> for MCPRelayActor {
    type Result = ();
    
    fn handle(&mut self, msg: OrchestratorBinary, ctx: &mut Self::Context) {
        // Forward binary message to client
        ctx.binary(msg.0);
    }
}

// WebSocket stream handler for client messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MCPRelayActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                // Client is alive
            }
            Ok(ws::Message::Text(text)) => {
                debug!("[MCP Relay] Received text from client: {}", text);
                
                // Parse and handle message
                if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&text) {
                    // Handle control messages
                    if let Some(msg_type) = msg.get("type").and_then(|t| t.as_str()) {
                        match msg_type {
                            "ping" => {
                                ctx.text(serde_json::json!({
                                    "type": "pong",
                                    "timestamp": chrono::Utc::now().timestamp_millis()
                                }).to_string());
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                
                // Forward to orchestrator if connected
                if let Some(tx) = &self.orchestrator_tx {
                    let tx = tx.clone();
                    let text_clone = text.to_string();
                    
                    actix::spawn(async move {
                        let mut tx_guard = tx.lock().await;
                        if let Err(e) = tx_guard.send(TungsteniteMessage::Text(text_clone)).await {
                            error!("[MCP Relay] Failed to send to orchestrator: {}", e);
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
                        let mut tx_guard = tx.lock().await;
                        if let Err(e) = tx_guard.send(TungsteniteMessage::Binary(bin_vec)).await {
                            error!("[MCP Relay] Failed to send binary to orchestrator: {}", e);
                        }
                    });
                }
            }
            Ok(ws::Message::Close(reason)) => {
                info!("[MCP Relay] Client closed connection: {:?}", reason);
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Nop) => {}
            Err(e) => {
                error!("[MCP Relay] WebSocket error: {}", e);
                ctx.stop();
            }
        }
    }
}

pub async fn mcp_relay_handler(
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    info!("[MCP Relay] New WebSocket connection request");
    ws::start(MCPRelayActor::new(), &req, stream)
}