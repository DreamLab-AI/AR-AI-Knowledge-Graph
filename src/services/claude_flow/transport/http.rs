use async_trait::async_trait;
use reqwest::{Client, Url};
use std::time::Duration;
use crate::services::claude_flow::error::{ConnectorError, Result};
use crate::services::claude_flow::transport::Transport;
use crate::services::claude_flow::types::{McpRequest, McpResponse, McpNotification};

pub struct HttpTransport {
    client: Client,
    base_url: Url,
    auth_token: Option<String>,
    connected: bool,
}

impl HttpTransport {
    pub fn new(host: &str, port: u16, auth_token: Option<String>) -> Result<Self> {
        let base_url = Url::parse(&format!("http://{}:{}", host, port))
            .map_err(|e| ConnectorError::Connection(format!("Invalid URL: {}", e)))?;
        
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        Ok(Self {
            client,
            base_url,
            auth_token,
            connected: false,
        })
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn connect(&mut self) -> Result<()> {
        // Test connection with health check
        let health_url = self.base_url.join("/health")
            .map_err(|e| ConnectorError::Connection(e.to_string()))?;
        
        let mut request = self.client.get(health_url);
        
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }
        
        let response = request.send().await?;
        
        if response.status().is_success() {
            self.connected = true;
            Ok(())
        } else {
            Err(ConnectorError::Connection(format!(
                "Health check failed with status: {}",
                response.status()
            )))
        }
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }
    
    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse> {
        if !self.connected {
            return Err(ConnectorError::Connection("Not connected".to_string()));
        }
        
        let rpc_url = self.base_url.join("/rpc")
            .map_err(|e| ConnectorError::Connection(e.to_string()))?;
        
        let mut http_request = self.client
            .post(rpc_url)
            .json(&request);
        
        if let Some(token) = &self.auth_token {
            http_request = http_request.bearer_auth(token);
        }
        
        let response = http_request.send().await?;
        
        if response.status().is_success() {
            let mcp_response = response.json::<McpResponse>().await?;
            
            // Check for MCP-level errors
            if let Some(error) = &mcp_response.error {
                return Err(ConnectorError::McpError {
                    code: error.code,
                    message: error.message.clone(),
                });
            }
            
            Ok(mcp_response)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(ConnectorError::Http(format!("HTTP {}: {}", status, error_text)))
        }
    }
    
    async fn send_notification(&mut self, notification: McpNotification) -> Result<()> {
        if !self.connected {
            return Err(ConnectorError::Connection("Not connected".to_string()));
        }
        
        let rpc_url = self.base_url.join("/rpc")
            .map_err(|e| ConnectorError::Connection(e.to_string()))?;
        
        let mut request = self.client
            .post(rpc_url)
            .json(&notification);
        
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }
        
        let response = request.send().await?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(ConnectorError::Http(format!("HTTP {}: {}", status, error_text)))
        }
    }
    
    async fn receive_notification(&mut self) -> Result<Option<McpNotification>> {
        // HTTP transport doesn't support receiving notifications
        // Use WebSocket transport for bidirectional communication
        Ok(None)
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
}