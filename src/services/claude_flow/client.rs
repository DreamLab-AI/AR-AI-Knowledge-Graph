use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use super::error::{ConnectorError, Result};
use super::transport::{Transport, http::HttpTransport, websocket::WebSocketTransport};
use super::types::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ClaudeFlowClient {
    transport: Arc<Mutex<Box<dyn Transport>>>,
    session_id: Option<String>,
    initialized: bool,
}

impl ClaudeFlowClient {
    pub async fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport: Arc::new(Mutex::new(transport)),
            session_id: None,
            initialized: false,
        }
    }
    
    pub async fn connect(&mut self) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.connect().await?;
        Ok(())
    }
    
    pub async fn disconnect(&mut self) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.disconnect().await?;
        self.initialized = false;
        self.session_id = None;
        Ok(())
    }
    
    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: ProtocolVersion {
                major: 2024,
                minor: 11,
                patch: 5,
            },
            client_info: ClientInfo {
                name: "Claude Flow Rust Connector".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: ClientCapabilities {
                tools: ToolCapabilities {
                    list_changed: true,
                },
            },
        };
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "initialize".to_string(),
            params: Some(serde_json::to_value(params)?),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            let init_result: InitializeResult = serde_json::from_value(result)?;
            self.initialized = true;
            self.session_id = Some(Uuid::new_v4().to_string());
            Ok(init_result)
        } else {
            Err(ConnectorError::InvalidResponse("No result in initialize response".to_string()))
        }
    }
    
    // Agent Management
    pub async fn spawn_agent(&self, agent_params: SpawnAgentParams) -> Result<AgentStatus> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "agents/spawn".to_string(),
            params: Some(serde_json::to_value(agent_params)?),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in spawn agent response".to_string()))
        }
    }
    
    pub async fn list_agents(&self, include_terminated: bool) -> Result<Vec<AgentStatus>> {
        self.ensure_initialized()?;
        
        let params = serde_json::json!({
            "includeTerminated": include_terminated
        });
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "agents/list".to_string(),
            params: Some(params),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in list agents response".to_string()))
        }
    }
    
    pub async fn terminate_agent(&self, agent_id: &str) -> Result<()> {
        self.ensure_initialized()?;
        
        let params = serde_json::json!({
            "agentId": agent_id
        });
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "agents/terminate".to_string(),
            params: Some(params),
        };
        
        let mut transport = self.transport.lock().await;
        let _ = transport.send_request(request).await?;
        Ok(())
    }
    
    // Task Management
    pub async fn create_task(&self, task_params: CreateTaskParams) -> Result<Task> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tasks/create".to_string(),
            params: Some(serde_json::to_value(task_params)?),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in create task response".to_string()))
        }
    }
    
    pub async fn list_tasks(&self) -> Result<Vec<Task>> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tasks/list".to_string(),
            params: None,
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in list tasks response".to_string()))
        }
    }
    
    pub async fn assign_task(&self, task_id: &str, agent_id: &str) -> Result<()> {
        self.ensure_initialized()?;
        
        let params = serde_json::json!({
            "taskId": task_id,
            "agentId": agent_id
        });
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tasks/assign".to_string(),
            params: Some(params),
        };
        
        let mut transport = self.transport.lock().await;
        let _ = transport.send_request(request).await?;
        Ok(())
    }
    
    // Memory Management
    pub async fn store_memory(&self, memory_params: StoreMemoryParams) -> Result<MemoryEntry> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "memory/store".to_string(),
            params: Some(serde_json::to_value(memory_params)?),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in store memory response".to_string()))
        }
    }
    
    pub async fn query_memory(&self, query_params: QueryMemoryParams) -> Result<Vec<MemoryEntry>> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "memory/query".to_string(),
            params: Some(serde_json::to_value(query_params)?),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in query memory response".to_string()))
        }
    }
    
    // System Monitoring
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "system/health".to_string(),
            params: None,
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in system health response".to_string()))
        }
    }
    
    pub async fn get_system_metrics(&self) -> Result<SystemMetrics> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "metrics/get".to_string(),
            params: None,
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in system metrics response".to_string()))
        }
    }
    
    // Swarm Operations
    pub async fn get_swarm_status(&self) -> Result<SwarmStatus> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "swarm/status".to_string(),
            params: None,
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in swarm status response".to_string()))
        }
    }
    
    // Tool Information
    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        self.ensure_initialized()?;
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/list".to_string(),
            params: None,
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(ConnectorError::InvalidResponse("No result in list tools response".to_string()))
        }
    }
    
    // Execute arbitrary command
    pub async fn execute_command(&self, command: &str, args: Vec<String>) -> Result<serde_json::Value> {
        self.ensure_initialized()?;
        
        let params = serde_json::json!({
            "command": command,
            "args": args
        });
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "commands/execute".to_string(),
            params: Some(params),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        if let Some(result) = response.result {
            Ok(result)
        } else {
            Err(ConnectorError::InvalidResponse("No result in execute command response".to_string()))
        }
    }
    
    // Helper methods
    fn ensure_initialized(&self) -> Result<()> {
        if !self.initialized {
            Err(ConnectorError::Protocol("Client not initialized".to_string()))
        } else {
            Ok(())
        }
    }
    
    pub fn session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }
    
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

// Builder pattern for client configuration
pub struct ClaudeFlowClientBuilder {
    host: String,
    port: u16,
    auth_token: Option<String>,
    use_websocket: bool,
}

impl Default for ClaudeFlowClientBuilder {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8081,
            auth_token: None,
            use_websocket: false,
        }
    }
}

impl ClaudeFlowClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }
    
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }
    
    pub fn use_websocket(mut self) -> Self {
        self.use_websocket = true;
        self
    }
    
    pub fn use_http(mut self) -> Self {
        self.use_websocket = false;
        self
    }
    
    pub async fn build(self) -> Result<ClaudeFlowClient> {
        let transport: Box<dyn Transport> = if self.use_websocket {
            Box::new(WebSocketTransport::new(&self.host, self.port, self.auth_token))
        } else {
            Box::new(HttpTransport::new(&self.host, self.port, self.auth_token)?)
        };
        
        Ok(ClaudeFlowClient::new(transport).await)
    }
}

// Parameter types for API calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentParams {
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(rename = "systemPrompt", skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(rename = "maxConcurrentTasks", skip_serializing_if = "Option::is_none")]
    pub max_concurrent_tasks: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    #[serde(rename = "workingDirectory", skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskParams {
    #[serde(rename = "type")]
    pub task_type: String,
    pub description: String,
    pub priority: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<String>>,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMemoryParams {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
    pub content: String,
    pub context: serde_json::Value,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMemoryParams {
    #[serde(rename = "agentId", skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}