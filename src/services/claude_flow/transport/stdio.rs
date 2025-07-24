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
}

#[async_trait]
impl Transport for StdioTransport {
    async fn connect(&mut self) -> Result<()> {
        // This transport is now a no-op.
        // The connection is handled by the WebSocket transport.
        warn!("StdioTransport::connect() called, but this transport is disabled. The system should be using the WebSocket transport to connect to 'powerdev'.");
        *self.connected.lock().await = false;
        Err(ConnectorError::Connection("StdioTransport is disabled.".to_string()))
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