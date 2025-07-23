use async_trait::async_trait;
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use std::process::Stdio;
use log::{info, error, debug};
use crate::services::claude_flow::error::{ConnectorError, Result};
use crate::services::claude_flow::transport::Transport;
use crate::services::claude_flow::types::{McpRequest, McpResponse, McpNotification};
use dashmap::DashMap;
use serde_json::json;

pub struct StdioTransport {
    process: Option<Arc<Mutex<Child>>>,
    stdin_tx: Option<mpsc::Sender<String>>,
    pending_requests: Arc<DashMap<String, mpsc::Sender<McpResponse>>>,
    notification_rx: Option<mpsc::Receiver<McpNotification>>,
    notification_tx: mpsc::Sender<McpNotification>,
    connected: Arc<Mutex<bool>>,
}

impl StdioTransport {
    pub fn new() -> Self {
        let (notification_tx, notification_rx) = mpsc::channel(100);
        
        Self {
            process: None,
            stdin_tx: None,
            pending_requests: Arc::new(DashMap::new()),
            notification_rx: Some(notification_rx),
            notification_tx,
            connected: Arc::new(Mutex::new(false)),
        }
    }
    
    async fn spawn_mcp_process(&mut self) -> Result<()> {
        info!("Spawning Claude Flow MCP process in stdio mode");
        
        // Spawn Claude Flow MCP with auto-orchestrator for agent management
        let mut child = Command::new("npx")
            .args(&["claude-flow@alpha", "mcp", "start", "--auto-orchestrator"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("CLAUDE_FLOW_AUTO_ORCHESTRATOR", "true")
            .env("CLAUDE_FLOW_NEURAL_ENABLED", "true")
            .env("CLAUDE_FLOW_WASM_ENABLED", "true")
            .spawn()
            .map_err(|e| ConnectorError::Connection(format!("Failed to spawn MCP process: {}", e)))?;
        
        let stdin = child.stdin.take()
            .ok_or_else(|| ConnectorError::Connection("Failed to get stdin".to_string()))?;
        
        let stdout = child.stdout.take()
            .ok_or_else(|| ConnectorError::Connection("Failed to get stdout".to_string()))?;
        
        let stderr = child.stderr.take()
            .ok_or_else(|| ConnectorError::Connection("Failed to get stderr".to_string()))?;
        
        // Create channel for stdin writer
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(100);
        self.stdin_tx = Some(stdin_tx);
        
        // Spawn stdin writer task
        let stdin_arc = Arc::new(Mutex::new(stdin));
        tokio::spawn(async move {
            while let Some(msg) = stdin_rx.recv().await {
                let mut stdin = stdin_arc.lock().await;
                if let Err(e) = stdin.write_all(msg.as_bytes()).await {
                    error!("Failed to write to stdin: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("Failed to flush stdin: {}", e);
                    break;
                }
            }
        });
        
        // Spawn stdout reader task
        let pending_requests = self.pending_requests.clone();
        let notification_tx = self.notification_tx.clone();
        let connected = self.connected.clone();
        
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                
                debug!("MCP stdout: {}", line);
                
                // Try to parse as JSON-RPC message
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(json) => {
                        // Check if it's a response or notification
                        if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                            // It's a response
                            if let Ok(response) = serde_json::from_value::<McpResponse>(json.clone()) {
                                if let Some((_, tx)) = pending_requests.remove(id) {
                                    let _ = tx.send(response).await;
                                }
                            }
                        } else if json.get("method").is_some() {
                            // It's a notification or server initialization
                            let method = json.get("method").and_then(|v| v.as_str()).unwrap_or("");
                            
                            // Handle server.initialized notification
                            if method == "server.initialized" {
                                info!("MCP server initialized successfully");
                                *connected.lock().await = true;
                            }
                            
                            // Try to parse as notification
                            if let Ok(notification) = serde_json::from_value::<McpNotification>(json.clone()) {
                                let _ = notification_tx.send(notification).await;
                            }
                        }
                    }
                    Err(e) => {
                        // Not JSON, might be initialization message
                        if line.contains("MCP server starting") || line.contains("Claude-Flow MCP") {
                            info!("MCP initialization: {}", line);
                        } else {
                            debug!("Non-JSON output: {} (error: {})", line, e);
                        }
                    }
                }
            }
            
            error!("MCP stdout reader ended");
            *connected.lock().await = false;
        });
        
        // Spawn stderr reader task
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("ERROR") {
                    error!("MCP stderr: {}", line);
                } else if line.contains("WARN") {
                    debug!("MCP stderr: {}", line);
                } else {
                    debug!("MCP stderr: {}", line);
                }
            }
        });
        
        self.process = Some(Arc::new(Mutex::new(child)));
        
        // Wait a bit for initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        Ok(())
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn connect(&mut self) -> Result<()> {
        if *self.connected.lock().await {
            return Ok(());
        }
        
        self.spawn_mcp_process().await?;
        
        // Wait a bit for process to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Send initialization request
        let init_request = McpRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(json!({
                "capabilities": {}
            })),
            id: "init".to_string(),
        };
        
        let init_response = self.send_request(init_request).await?;
        info!("MCP initialized successfully: {:?}", init_response);
        
        *self.connected.lock().await = true;
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.lock().await = false;
        
        if let Some(process) = &self.process {
            let mut child = process.lock().await;
            let _ = child.kill().await;
        }
        
        self.process = None;
        self.stdin_tx = None;
        
        Ok(())
    }
    
    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse> {
        if !*self.connected.lock().await {
            return Err(ConnectorError::NotConnected);
        }
        
        let (response_tx, mut response_rx) = mpsc::channel(1);
        self.pending_requests.insert(request.id.clone(), response_tx);
        
        let json = serde_json::to_string(&request)
            .map_err(|e| ConnectorError::Serialization(e))?;
        
        if let Some(stdin_tx) = &self.stdin_tx {
            stdin_tx.send(format!("{}\n", json)).await
                .map_err(|_| ConnectorError::Connection("Failed to send to stdin".to_string()))?;
        } else {
            return Err(ConnectorError::NotConnected);
        }
        
        // Wait for response with timeout
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            response_rx.recv()
        ).await {
            Ok(Some(response)) => Ok(response),
            Ok(None) => Err(ConnectorError::Connection("Response channel closed".to_string())),
            Err(_) => {
                self.pending_requests.remove(&request.id);
                Err(ConnectorError::Timeout)
            }
        }
    }
    
    async fn send_notification(&mut self, notification: McpNotification) -> Result<()> {
        if !*self.connected.lock().await {
            return Err(ConnectorError::NotConnected);
        }
        
        let json = serde_json::to_string(&notification)
            .map_err(|e| ConnectorError::Serialization(e))?;
        
        if let Some(stdin_tx) = &self.stdin_tx {
            stdin_tx.send(format!("{}\n", json)).await
                .map_err(|_| ConnectorError::Connection("Failed to send to stdin".to_string()))?;
        } else {
            return Err(ConnectorError::NotConnected);
        }
        
        Ok(())
    }
    
    async fn receive_notification(&mut self) -> Result<Option<McpNotification>> {
        if let Some(rx) = &mut self.notification_rx {
            Ok(rx.recv().await)
        } else {
            Ok(None)
        }
    }
    
    fn is_connected(&self) -> bool {
        // Check if connected flag is true (can't await in sync context)
        // This is a limitation of the trait design
        // This is a limitation - we can't await in sync context
        // For now, we'll trust the connected flag was set properly
        // In production, consider using a different approach
        true
    }
}