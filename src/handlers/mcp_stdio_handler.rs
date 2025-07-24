use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use log::{debug, error, info, warn};
use std::time::Duration;

#[derive(Message)]
#[rtype(result = "()")]
struct ClientText(String);

#[derive(Message)]
#[rtype(result = "()")]
struct McpResponse(String);

#[derive(Message)]
#[rtype(result = "()")]
struct StartMcp;

pub struct MCPStdioHandler {
    client_id: String,
    mcp_process: Option<Child>,
    mcp_tx: Option<mpsc::Sender<String>>,
}

impl MCPStdioHandler {
    fn new() -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            mcp_process: None,
            mcp_tx: None,
        }
    }
    
    fn start_mcp_process(&mut self, ctx: &mut <Self as Actor>::Context) {
        info!("[MCP Stdio] Starting Claude Flow MCP process...");
        
        let addr = ctx.address();
        
        actix::spawn(async move {
            // Start the MCP process
            let working_dir = std::env::var("CLAUDE_FLOW_DIR")
                .unwrap_or_else(|_| "/workspace/ext/claude-flow".to_string());
            
            let mut child = match Command::new("npx")
                .args(&["claude-flow@alpha", "mcp", "start", "--stdio"])
                .current_dir(&working_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env("CLAUDE_FLOW_AUTO_ORCHESTRATOR", "true")
                .env("CLAUDE_FLOW_NEURAL_ENABLED", "true")
                .env("CLAUDE_FLOW_WASM_ENABLED", "true")
                .spawn()
            {
                Ok(child) => {
                    info!("[MCP Stdio] Claude Flow MCP process started successfully");
                    child
                }
                Err(e) => {
                    error!("[MCP Stdio] Failed to start MCP process: {}", e);
                    return;
                }
            };
            
            // Get stdin/stdout handles
            let stdin = child.stdin.take().expect("Failed to get stdin");
            let stdout = child.stdout.take().expect("Failed to get stdout");
            let stderr = child.stderr.take().expect("Failed to get stderr");
            
            // Create channel for sending to MCP
            let (tx, mut rx) = mpsc::channel::<String>(100);
            
            // Send transmitter to actor
            addr.do_send(SetMcpChannel(tx.clone()));
            
            // Spawn task to handle stderr
            let stderr_reader = BufReader::new(stderr);
            let mut stderr_lines = stderr_reader.lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = stderr_lines.next_line().await {
                    warn!("[MCP Stdio] stderr: {}", line);
                }
            });
            
            // Spawn task to read from MCP stdout
            let stdout_reader = BufReader::new(stdout);
            let mut stdout_lines = stdout_reader.lines();
            let addr_clone = addr.clone();
            
            tokio::spawn(async move {
                while let Ok(Some(line)) = stdout_lines.next_line().await {
                    if !line.trim().is_empty() {
                        debug!("[MCP Stdio] Received: {}", line);
                        addr_clone.do_send(McpResponse(line));
                    }
                }
                info!("[MCP Stdio] MCP process stdout closed");
            });
            
            // Handle writing to MCP stdin
            let mut stdin = stdin;
            while let Some(msg) = rx.recv().await {
                debug!("[MCP Stdio] Sending to MCP: {}", msg);
                if let Err(e) = stdin.write_all(format!("{}\n", msg).as_bytes()).await {
                    error!("[MCP Stdio] Failed to write to MCP: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("[MCP Stdio] Failed to flush MCP stdin: {}", e);
                    break;
                }
            }
            
            info!("[MCP Stdio] Shutting down MCP process");
            let _ = child.kill().await;
        });
    }
    
    fn send_to_mcp(&self, message: String) {
        if let Some(tx) = &self.mcp_tx {
            let tx = tx.clone();
            tokio::spawn(async move {
                if let Err(e) = tx.send(message).await {
                    error!("[MCP Stdio] Failed to send to MCP channel: {}", e);
                }
            });
        } else {
            warn!("[MCP Stdio] MCP not connected, cannot send message");
        }
    }
}

impl Actor for MCPStdioHandler {
    type Context = ws::WebsocketContext<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[MCP Stdio] WebSocket handler started for client: {}", self.client_id);
        
        // Start heartbeat
        ctx.run_interval(Duration::from_secs(30), |_act, ctx| {
            ctx.ping(b"");
        });
        
        // Start MCP process
        self.start_mcp_process(ctx);
        
        // Send initialization after a short delay to ensure MCP is ready
        ctx.run_later(Duration::from_secs(1), |act, ctx| {
            let init_msg = json!({
                "jsonrpc": "2.0",
                "id": "init-1",
                "method": "initialize",
                "params": {
                    "protocolVersion": {
                        "major": 2024,
                        "minor": 11,
                        "patch": 5
                    },
                    "clientInfo": {
                        "name": "Rust MCP Direct Client",
                        "version": "1.0.0"
                    },
                    "capabilities": {
                        "tools": {
                            "listChanged": true
                        }
                    }
                }
            });
            
            act.send_to_mcp(init_msg.to_string());
            
            // Notify client that MCP is initializing
            ctx.text(json!({
                "type": "mcp_status",
                "status": "initializing",
                "timestamp": chrono::Utc::now().timestamp_millis()
            }).to_string());
        });
    }
    
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("[MCP Stdio] WebSocket handler stopped for client: {}", self.client_id);
        
        // Clean up MCP process
        if let Some(mut process) = self.mcp_process.take() {
            tokio::spawn(async move {
                let _ = process.kill().await;
            });
        }
    }
}

// Handle MCP responses
impl Handler<McpResponse> for MCPStdioHandler {
    type Result = ();
    
    fn handle(&mut self, msg: McpResponse, ctx: &mut Self::Context) {
        // Forward MCP response to WebSocket client
        ctx.text(msg.0);
    }
}

// Handle MCP process started
impl Handler<StartMcp> for MCPStdioHandler {
    type Result = ();
    
    fn handle(&mut self, _msg: StartMcp, ctx: &mut Self::Context) {
        // MCP process is ready
        ctx.text(json!({
            "type": "mcp_status",
            "status": "connected",
            "timestamp": chrono::Utc::now().timestamp_millis()
        }).to_string());
    }
}

// WebSocket stream handler for client messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MCPStdioHandler {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                // Client is alive
            }
            Ok(ws::Message::Text(text)) => {
                debug!("[MCP Stdio] Received from client: {}", text);
                
                // Parse and validate JSON-RPC message
                if let Ok(msg) = serde_json::from_str::<Value>(&text) {
                    // Handle control messages
                    if let Some(msg_type) = msg.get("type").and_then(|t| t.as_str()) {
                        match msg_type {
                            "ping" => {
                                ctx.text(json!({
                                    "type": "pong",
                                    "timestamp": chrono::Utc::now().timestamp_millis()
                                }).to_string());
                                return;
                            }
                            _ => {}
                        }
                    }
                    
                    // Forward valid JSON-RPC messages to MCP
                    if msg.get("jsonrpc").is_some() {
                        self.send_to_mcp(text.to_string());
                    }
                } else {
                    warn!("[MCP Stdio] Invalid JSON from client: {}", text);
                    ctx.text(json!({
                        "type": "error",
                        "message": "Invalid JSON format",
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    }).to_string());
                }
            }
            Ok(ws::Message::Binary(_)) => {
                warn!("[MCP Stdio] Binary messages not supported");
                ctx.text(json!({
                    "type": "error",
                    "message": "Binary messages not supported",
                    "timestamp": chrono::Utc::now().timestamp_millis()
                }).to_string());
            }
            Ok(ws::Message::Close(reason)) => {
                info!("[MCP Stdio] Client closed connection: {:?}", reason);
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Nop) => {}
            Err(e) => {
                error!("[MCP Stdio] WebSocket error: {}", e);
                ctx.stop();
            }
        }
    }
}

pub async fn mcp_stdio_handler(
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    info!("[MCP Stdio] New WebSocket connection request");
    ws::start(MCPStdioHandler::new(), &req, stream)
}

// Handler to store the MCP channel sender
impl MCPStdioHandler {
    fn set_mcp_tx(&mut self, tx: mpsc::Sender<String>) {
        self.mcp_tx = Some(tx);
    }
}

// Message to provide the channel
#[derive(Message)]
#[rtype(result = "()")]
struct SetMcpChannel(mpsc::Sender<String>);

impl Handler<SetMcpChannel> for MCPStdioHandler {
    type Result = ();
    
    fn handle(&mut self, msg: SetMcpChannel, _ctx: &mut Self::Context) {
        self.set_mcp_tx(msg.0);
    }
}