use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use std::sync::Arc;
use std::time::Duration;
use crate::services::claude_flow::error::{ConnectorError, Result};
use crate::services::claude_flow::transport::Transport;
use crate::services::claude_flow::types::{McpRequest, McpResponse, McpNotification};
use dashmap::DashMap;
use uuid::Uuid;

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct WebSocketTransport {
    url: String,
    auth_token: Option<String>,
    ws_stream: Option<Arc<Mutex<WsStream>>>,
    pending_requests: Arc<DashMap<String, mpsc::Sender<McpResponse>>>,
    notification_rx: Option<mpsc::Receiver<McpNotification>>,
    notification_tx: mpsc::Sender<McpNotification>,
    connected: bool,
}

impl WebSocketTransport {
    pub fn new(host: &str, port: u16, auth_token: Option<String>) -> Self {
        let url = format!("ws://{}:{}/ws", host, port);
        let (notification_tx, notification_rx) = mpsc::channel(100);
        
        Self {
            url,
            auth_token,
            ws_stream: None,
            pending_requests: Arc::new(DashMap::new()),
            notification_rx: Some(notification_rx),
            notification_tx,
            connected: false,
        }
    }
    
    async fn handle_incoming_messages(
        ws_stream: Arc<Mutex<WsStream>>,
        pending_requests: Arc<DashMap<String, mpsc::Sender<McpResponse>>>,
        notification_tx: mpsc::Sender<McpNotification>,
    ) {
        loop {
            let message = {
                let mut stream = ws_stream.lock().await;
                stream.next().await
            };
            
            match message {
                Some(Ok(Message::Text(text))) => {
                    // Try to parse as response first
                    if let Ok(response) = serde_json::from_str::<McpResponse>(&text) {
                        if let Some((_, sender)) = pending_requests.remove(&response.id) {
                            let _ = sender.send(response).await;
                        }
                    }
                    // Try to parse as notification
                    else if let Ok(notification) = serde_json::from_str::<McpNotification>(&text) {
                        let _ = notification_tx.send(notification).await;
                    }
                }
                Some(Ok(Message::Close(_))) => break,
                Some(Err(e)) => {
                    log::error!("WebSocket error: {}", e);
                    break;
                }
                None => break,
                _ => {}
            }
        }
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn connect(&mut self) -> Result<()> {
        let url = if let Some(token) = &self.auth_token {
            format!("{}?auth={}", self.url, token)
        } else {
            self.url.clone()
        };
        
        let (ws_stream, _) = connect_async(&url).await?;
        let ws_stream = Arc::new(Mutex::new(ws_stream));
        self.ws_stream = Some(ws_stream.clone());
        
        // Spawn message handler
        let pending_requests = self.pending_requests.clone();
        let notification_tx = self.notification_tx.clone();
        tokio::spawn(Self::handle_incoming_messages(
            ws_stream,
            pending_requests,
            notification_tx,
        ));
        
        self.connected = true;
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        if let Some(ws_stream) = &self.ws_stream {
            let mut stream = ws_stream.lock().await;
            let _ = stream.close(None).await;
        }
        
        self.ws_stream = None;
        self.connected = false;
        self.pending_requests.clear();
        
        Ok(())
    }
    
    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse> {
        if !self.connected || self.ws_stream.is_none() {
            return Err(ConnectorError::Connection("Not connected".to_string()));
        }
        
        let (response_tx, mut response_rx) = mpsc::channel(1);
        self.pending_requests.insert(request.id.clone(), response_tx);
        
        // Send request
        let ws_stream = self.ws_stream.as_ref().unwrap();
        let message = Message::Text(serde_json::to_string(&request)?);
        
        {
            let mut stream = ws_stream.lock().await;
            stream.send(message).await?;
        }
        
        // Wait for response with timeout
        match tokio::time::timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(response)) => {
                // Check for MCP-level errors
                if let Some(error) = &response.error {
                    return Err(ConnectorError::McpError {
                        code: error.code,
                        message: error.message.clone(),
                    });
                }
                Ok(response)
            }
            Ok(None) => Err(ConnectorError::InvalidResponse("Channel closed".to_string())),
            Err(_) => {
                self.pending_requests.remove(&request.id);
                Err(ConnectorError::Timeout)
            }
        }
    }
    
    async fn send_notification(&mut self, notification: McpNotification) -> Result<()> {
        if !self.connected || self.ws_stream.is_none() {
            return Err(ConnectorError::Connection("Not connected".to_string()));
        }
        
        let ws_stream = self.ws_stream.as_ref().unwrap();
        let message = Message::Text(serde_json::to_string(&notification)?);
        
        let mut stream = ws_stream.lock().await;
        stream.send(message).await?;
        
        Ok(())
    }
    
    async fn receive_notification(&mut self) -> Result<Option<McpNotification>> {
        if let Some(rx) = &mut self.notification_rx {
            Ok(rx.try_recv().ok())
        } else {
            Ok(None)
        }
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
}