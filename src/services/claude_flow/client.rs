use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use super::error::{ConnectorError, Result};
use super::transport::{Transport, http::HttpTransport, websocket::WebSocketTransport, stdio::StdioTransport};
use super::types::*;
use super::mcp_tools::{McpTool, ToolResponse};
use std::collections::HashMap;
use serde_json::{json, Value};
use log::debug;

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
    
    // Agent Management - Updated to use MCP tools
    pub async fn spawn_agent(&self, agent_params: SpawnAgentParams) -> Result<AgentStatus> {
        self.ensure_initialized()?;
        
        // Clone values before moving them
        let agent_name = agent_params.name.clone();
        let agent_capabilities = agent_params.capabilities.clone();
        
        let tool = McpTool::AgentSpawn {
            agent_type: agent_params.agent_type.to_string(),
            name: Some(agent_name.clone()),
            capabilities: agent_capabilities.clone(),
            swarm_id: None,
        };
        
        let response = self.call_tool(tool).await?;
        
        if response.success {
            // Create AgentStatus from response data
            Ok(AgentStatus {
                agent_id: response.data.get("agentId")
                    .and_then(|a| a.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                status: "active".to_string(),
                session_id: self.session_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                profile: AgentProfile {
                    name: agent_params.name,
                    agent_type: agent_params.agent_type,
                    capabilities: agent_params.capabilities.unwrap_or_default(),
                    system_prompt: agent_params.system_prompt,
                    max_concurrent_tasks: agent_params.max_concurrent_tasks.unwrap_or(3),
                    priority: agent_params.priority.unwrap_or(5),
                    retry_policy: Default::default(),
                    environment: agent_params.environment,
                    working_directory: agent_params.working_directory,
                },
                timestamp: chrono::Utc::now(),
                active_tasks_count: 0,
                completed_tasks_count: 0,
                failed_tasks_count: 0,
                total_execution_time: 0,
                average_task_duration: 0.0,
                success_rate: 100.0,
                current_task: None,
                metadata: Default::default(),
            })
        } else {
            Err(ConnectorError::Protocol("Failed to spawn agent".to_string()))
        }
    }
    
    pub async fn list_agents(&self, include_terminated: bool) -> Result<Vec<AgentStatus>> {
        self.ensure_initialized()?;
        
        let tool = McpTool::AgentList { swarm_id: None };
        let response = self.call_tool(tool).await?;
        
        if let Some(agents_data) = response.data.get("agents") {
            let mut agents: Vec<AgentStatus> = vec![];
            
            if let Some(agent_array) = agents_data.as_array() {
                for agent_value in agent_array {
                    // Try to parse as full AgentStatus first
                    if let Ok(agent) = serde_json::from_value::<AgentStatus>(agent_value.clone()) {
                        if !include_terminated || agent.status != "terminated" {
                            agents.push(agent);
                        }
                    } else {
                        // Fallback: create basic AgentStatus from partial data
                        let agent_id = agent_value.get("id")
                            .or_else(|| agent_value.get("agentId"))
                            .and_then(|a| a.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        let agent_type = agent_value.get("type")
                            .and_then(|t| t.as_str())
                            .and_then(|t| AgentType::from_str(t).ok())
                            .unwrap_or(AgentType::Specialist);
                        
                        let status = agent_value.get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        if !include_terminated && status == "terminated" {
                            continue;
                        }
                        
                        agents.push(AgentStatus {
                            agent_id,
                            status,
                            session_id: self.session_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                            profile: AgentProfile {
                                name: agent_value.get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or(&agent_type.to_string())
                                    .to_string(),
                                agent_type,
                                capabilities: agent_value.get("capabilities")
                                    .and_then(|c| c.as_array())
                                    .map(|arr| arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect())
                                    .unwrap_or_default(),
                                system_prompt: None,
                                max_concurrent_tasks: 3,
                                priority: 5,
                                retry_policy: Default::default(),
                                environment: None,
                                working_directory: None,
                            },
                            timestamp: chrono::Utc::now(),
                            active_tasks_count: 0,
                            completed_tasks_count: 0,
                            failed_tasks_count: 0,
                            total_execution_time: 0,
                            average_task_duration: 0.0,
                            success_rate: 100.0,
                            current_task: None,
                            metadata: Default::default(),
                        });
                    }
                }
            }
            Ok(agents)
        } else {
            Ok(vec![])
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
    
    // Memory Management - Enhanced with MCP tools
    pub async fn store_in_memory(&self, key: &str, value: &str, namespace: Option<&str>) -> Result<()> {
        self.ensure_initialized()?;
        
        let tool = McpTool::MemoryUsage {
            action: "store".to_string(),
            key: Some(key.to_string()),
            value: Some(value.to_string()),
            namespace: namespace.map(|n| n.to_string()),
            ttl: None,
        };
        
        let response = self.call_tool(tool).await?;
        
        if response.success {
            Ok(())
        } else {
            Err(ConnectorError::Protocol("Failed to store memory".to_string()))
        }
    }
    
    pub async fn retrieve_from_memory(&self, key: &str, namespace: Option<&str>) -> Result<Option<String>> {
        self.ensure_initialized()?;
        
        let tool = McpTool::MemoryUsage {
            action: "retrieve".to_string(),
            key: Some(key.to_string()),
            value: None,
            namespace: namespace.map(|n| n.to_string()),
            ttl: None,
        };
        
        let response = self.call_tool(tool).await?;
        
        if response.success {
            Ok(response.data.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }
    
    pub async fn search_memory(&self, pattern: &str, namespace: Option<&str>, limit: Option<u32>) -> Result<Vec<Value>> {
        self.ensure_initialized()?;
        
        let tool = McpTool::MemorySearch {
            pattern: pattern.to_string(),
            namespace: namespace.map(|n| n.to_string()),
            limit,
        };
        
        let response = self.call_tool(tool).await?;
        
        if let Some(results) = response.data.get("results").and_then(|r| r.as_array()) {
            Ok(results.clone())
        } else {
            Ok(vec![])
        }
    }
    
    // Legacy method for compatibility
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
    
    // System Monitoring - Enhanced with MCP tools
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        self.ensure_initialized()?;
        
        let tool = McpTool::HealthCheck { components: None };
        let response = self.call_tool(tool).await?;
        
        Ok(SystemHealth {
            status: if response.success { "healthy".to_string() } else { "unhealthy".to_string() },
            components: HashMap::new(),
            timestamp: chrono::Utc::now(),
        })
    }
    
    pub async fn get_performance_report(&self, timeframe: Option<&str>) -> Result<Value> {
        self.ensure_initialized()?;
        
        let tool = McpTool::PerformanceReport {
            timeframe: timeframe.map(|t| t.to_string()),
            format: Some("detailed".to_string()),
        };
        
        let response = self.call_tool(tool).await?;
        Ok(response.data)
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
    
    // Swarm Operations - Enhanced with MCP tools
    pub async fn init_swarm(&self, topology: &str, max_agents: Option<u32>) -> Result<String> {
        self.ensure_initialized()?;
        
        let tool = McpTool::SwarmInit {
            topology: topology.to_string(),
            max_agents,
            strategy: Some("adaptive".to_string()),
        };
        
        let response = self.call_tool(tool).await?;
        
        if let Some(swarm_id) = response.data.get("swarmId").and_then(|s| s.as_str()) {
            Ok(swarm_id.to_string())
        } else {
            Err(ConnectorError::InvalidResponse("No swarmId in response".to_string()))
        }
    }
    
    pub async fn get_swarm_status(&self) -> Result<SwarmStatus> {
        self.ensure_initialized()?;
        
        let tool = McpTool::SwarmStatus { swarm_id: None };
        let response = self.call_tool(tool).await?;
        
        // Convert response data to SwarmStatus
        Ok(SwarmStatus {
            id: response.data.get("swarmId")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string(),
            topology: response.data.get("topology")
                .and_then(|t| t.as_str())
                .unwrap_or("hierarchical")
                .to_string(),
            strategy: response.data.get("strategy")
                .and_then(|s| s.as_str())
                .unwrap_or("adaptive")
                .to_string(),
            total_agents: response.data.get("agentCount")
                .and_then(|a| a.as_u64())
                .unwrap_or(0) as u32,
            active_agents: response.data.get("activeAgents")
                .and_then(|a| a.as_u64())
                .unwrap_or(0) as u32,
            total_tasks: response.data.get("taskCount")
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32,
            completed_tasks: response.data.get("completedTasks")
                .and_then(|c| c.as_u64())
                .unwrap_or(0) as u32,
            uptime: response.data.get("uptime")
                .and_then(|u| u.as_u64())
                .unwrap_or(0),
            status: response.data.get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("active")
                .to_string(),
        })
    }
    
    // Enhanced MCP Tool Support
    pub async fn call_tool(&self, tool: McpTool) -> Result<ToolResponse> {
        self.ensure_initialized()?;
        
        let tool_name = tool.name();
        let arguments = tool.to_arguments();
        
        debug!("Calling MCP tool: {} with args: {:?}", tool_name, arguments);
        
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": tool_name,
                "arguments": arguments
            })),
        };
        
        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;
        
        ToolResponse::from_mcp_response(&response)
            .map_err(|e| ConnectorError::InvalidResponse(format!("Failed to parse tool response: {}", e)))
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
            if let Some(tools) = result.get("tools") {
                Ok(serde_json::from_value(tools.clone())?)
            } else {
                Ok(vec![])
            }
        } else {
            Err(ConnectorError::InvalidResponse("No result in list tools response".to_string()))
        }
    }
    
    // Neural Network Operations
    pub async fn train_neural_pattern(&self, pattern_type: &str, training_data: &str, epochs: Option<u32>) -> Result<Value> {
        self.ensure_initialized()?;
        
        let tool = McpTool::NeuralTrain {
            pattern_type: pattern_type.to_string(),
            training_data: training_data.to_string(),
            epochs,
        };
        
        let response = self.call_tool(tool).await?;
        Ok(response.data)
    }
    
    pub async fn neural_predict(&self, model_id: &str, input: &str) -> Result<Value> {
        self.ensure_initialized()?;
        
        let tool = McpTool::NeuralPredict {
            model_id: model_id.to_string(),
            input: input.to_string(),
        };
        
        let response = self.call_tool(tool).await?;
        Ok(response.data)
    }
    
    // Task Orchestration
    pub async fn orchestrate_task(&self, task: &str, strategy: Option<&str>, priority: Option<&str>) -> Result<String> {
        self.ensure_initialized()?;
        
        let tool = McpTool::TaskOrchestrate {
            task: task.to_string(),
            strategy: strategy.map(|s| s.to_string()),
            priority: priority.map(|p| p.to_string()),
            dependencies: None,
        };
        
        let response = self.call_tool(tool).await?;
        
        if let Some(task_id) = response.data.get("taskId").and_then(|t| t.as_str()) {
            Ok(task_id.to_string())
        } else {
            Err(ConnectorError::InvalidResponse("No taskId in response".to_string()))
        }
    }
    
    pub async fn get_task_status(&self, task_id: &str) -> Result<Value> {
        self.ensure_initialized()?;
        
        let tool = McpTool::TaskStatus {
            task_id: task_id.to_string(),
        };
        
        let response = self.call_tool(tool).await?;
        Ok(response.data)
    }
    
    pub async fn get_task_results(&self, task_id: &str) -> Result<Value> {
        self.ensure_initialized()?;
        
        let tool = McpTool::TaskResults {
            task_id: task_id.to_string(),
        };
        
        let response = self.call_tool(tool).await?;
        Ok(response.data)
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

// Transport selection for the builder
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportType {
    Http,
    WebSocket,
    Stdio,
}

// Builder pattern for client configuration
pub struct ClaudeFlowClientBuilder {
    host: String,
    port: u16,
    auth_token: Option<String>,
    transport_type: TransportType,
}

impl Default for ClaudeFlowClientBuilder {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8081,
            auth_token: None,
            transport_type: TransportType::Stdio, // Default to stdio as it's what Claude Flow provides
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
        self.transport_type = TransportType::WebSocket;
        self
    }
    
    pub fn use_http(mut self) -> Self {
        self.transport_type = TransportType::Http;
        self
    }
    
    pub fn use_stdio(mut self) -> Self {
        self.transport_type = TransportType::Stdio;
        self
    }
    
    pub async fn build(self) -> Result<ClaudeFlowClient> {
        let transport: Box<dyn Transport> = match self.transport_type {
            TransportType::WebSocket => {
                Box::new(WebSocketTransport::new(&self.host, self.port, self.auth_token))
            }
            TransportType::Http => {
                Box::new(HttpTransport::new(&self.host, self.port, self.auth_token)?)
            }
            TransportType::Stdio => {
                Box::new(StdioTransport::new())
            }
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