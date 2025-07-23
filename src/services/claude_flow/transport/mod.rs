pub mod http;
pub mod websocket;

use async_trait::async_trait;
use crate::services::claude_flow::error::Result;
use crate::services::claude_flow::types::{McpRequest, McpResponse, McpNotification};

#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse>;
    async fn send_notification(&mut self, notification: McpNotification) -> Result<()>;
    async fn receive_notification(&mut self) -> Result<Option<McpNotification>>;
    fn is_connected(&self) -> bool;
}