use std::sync::Arc;
use std::str::FromStr;
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

        // Use tools/call method for MCP protocol
        let params = json!({
            "name": "agent_spawn",
            "arguments": {
                "type": agent_params.agent_type.to_string(),
                "name": agent_params.name.clone(),
                "capabilities": agent_params.capabilities.clone()
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            // Create AgentStatus from response data
            Ok(AgentStatus {
                agent_id: tool_result.get("agentId")
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

        let params = json!({
            "name": "agent_list",
            "arguments": {}
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            if let Some(agents_data) = tool_result.get("agents") {
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
                                .and_then(|t| t.parse().ok())
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
        } else {
            Err(ConnectorError::InvalidResponse("No result in list agents response".to_string()))
        }
    }

    pub async fn terminate_agent(&self, agent_id: &str) -> Result<()> {
        self.ensure_initialized()?;

        // Note: MCP doesn't have an agent_terminate tool, this would need to be implemented
        // For now, return an error
        Err(ConnectorError::Protocol("Agent termination not supported in MCP".to_string()))
    }

    // Task Management
    pub async fn create_task(&self, task_params: CreateTaskParams) -> Result<Task> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "task_orchestrate",
            "arguments": {
                "task": task_params.description,
                "priority": match task_params.priority {
                    p if p >= 8 => "critical",
                    p if p >= 6 => "high", 
                    p if p >= 4 => "medium",
                    _ => "low"
                },
                "strategy": "adaptive"
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            // Create a Task from the response
            Ok(Task {
                id: tool_result.get("taskId")
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                task_type: task_params.task_type,
                description: task_params.description,
                status: TaskStatus::Pending,
                priority: task_params.priority,
                dependencies: task_params.dependencies.unwrap_or_default(),
                assigned_agent: None,
                created_at: chrono::Utc::now(),
                started_at: None,
                completed_at: None,
                input: task_params.input,
                output: None,
                error: None,
            })
        } else {
            Err(ConnectorError::InvalidResponse("No result in create task response".to_string()))
        }
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>> {
        self.ensure_initialized()?;

        // MCP doesn't have a direct task list tool, use task_status instead
        let params = json!({
            "name": "task_status",
            "arguments": {}
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let _response = transport.send_request(request).await?;

        // For now, return empty list as MCP doesn't provide a full task list
        Ok(vec![])
    }

    pub async fn assign_task(&self, _task_id: &str, _agent_id: &str) -> Result<()> {
        self.ensure_initialized()?;

        // MCP doesn't have direct task assignment, tasks are auto-assigned
        // This is handled by the orchestrator
        Ok(())
    }

    // Memory Management - Enhanced with MCP tools
    pub async fn store_in_memory(&self, key: &str, value: &str, namespace: Option<&str>) -> Result<()> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "memory_usage",
            "arguments": {
                "action": "store",
                "key": key,
                "value": value,
                "namespace": namespace.unwrap_or("default")
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if response.result.is_some() {
            Ok(())
        } else {
            Err(ConnectorError::Protocol("Failed to store memory".to_string()))
        }
    }

    pub async fn retrieve_from_memory(&self, key: &str, namespace: Option<&str>) -> Result<Option<String>> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "memory_usage",
            "arguments": {
                "action": "retrieve",
                "key": key,
                "namespace": namespace.unwrap_or("default")
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            Ok(tool_result.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }

    pub async fn search_memory(&self, pattern: &str, namespace: Option<&str>, limit: Option<u32>) -> Result<Vec<Value>> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "memory_search",
            "arguments": {
                "pattern": pattern,
                "namespace": namespace,
                "limit": limit.unwrap_or(10)
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            if let Some(results) = tool_result.get("results").and_then(|r| r.as_array()) {
                Ok(results.clone())
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }

    // Legacy method for compatibility
    pub async fn store_memory(&self, memory_params: StoreMemoryParams) -> Result<MemoryEntry> {
        self.ensure_initialized()?;

        // Convert to MCP memory_usage tool format
        let memory_key = format!("agent/{}/{}", memory_params.agent_id, chrono::Utc::now().timestamp());
        let memory_value = json!({
            "type": memory_params.memory_type,
            "content": memory_params.content,
            "context": memory_params.context,
            "tags": memory_params.tags
        });

        let params = json!({
            "name": "memory_usage",
            "arguments": {
                "action": "store",
                "key": memory_key,
                "value": serde_json::to_string(&memory_value)?,
                "namespace": "legacy"
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if response.result.is_some() {
            // Create a MemoryEntry for compatibility
            Ok(MemoryEntry {
                id: memory_key.clone(),
                agent_id: memory_params.agent_id,
                session_id: self.session_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                memory_type: memory_params.memory_type,
                content: memory_params.content,
                context: memory_params.context,
                tags: memory_params.tags,
                timestamp: chrono::Utc::now(),
                version: 1,
                parent_id: None,
            })
        } else {
            Err(ConnectorError::InvalidResponse("No result in store memory response".to_string()))
        }
    }

    pub async fn query_memory(&self, query_params: QueryMemoryParams) -> Result<Vec<MemoryEntry>> {
        self.ensure_initialized()?;

        // Use memory_search tool for querying
        let search_pattern = query_params.search.unwrap_or_else(|| {
            if let Some(agent_id) = &query_params.agent_id {
                format!("agent/{}/*", agent_id)
            } else {
                "*".to_string()
            }
        });

        let params = json!({
            "name": "memory_search",
            "arguments": {
                "pattern": search_pattern,
                "namespace": "legacy",
                "limit": query_params.limit.unwrap_or(100)
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            // Convert results to MemoryEntry objects
            let mut entries = vec![];
            if let Some(results) = tool_result.get("results").and_then(|r| r.as_array()) {
                for result in results {
                    if let Ok(memory_data) = serde_json::from_value::<Value>(result.clone()) {
                        // Try to extract the stored value
                        if let Some(value_str) = memory_data.get("value").and_then(|v| v.as_str()) {
                            if let Ok(stored_data) = serde_json::from_str::<Value>(value_str) {
                                // Create MemoryEntry from stored data
                                entries.push(MemoryEntry {
                                    id: memory_data.get("key").and_then(|k| k.as_str()).unwrap_or("unknown").to_string(),
                                    agent_id: query_params.agent_id.clone().unwrap_or_else(|| "unknown".to_string()),
                                    session_id: self.session_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
                                    memory_type: stored_data.get("type")
                                        .and_then(|t| serde_json::from_value(t.clone()).ok())
                                        .unwrap_or(MemoryType::Observation),
                                    content: stored_data.get("content")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    context: stored_data.get("context")
                                        .cloned()
                                        .unwrap_or(json!({})),
                                    tags: stored_data.get("tags")
                                        .and_then(|t| serde_json::from_value(t.clone()).ok())
                                        .unwrap_or_default(),
                                    timestamp: chrono::Utc::now(),
                                    version: stored_data.get("version")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(1) as u32,
                                    parent_id: stored_data.get("parent_id")
                                        .and_then(|p| p.as_str())
                                        .map(|s| s.to_string()),
                                });
                            }
                        }
                    }
                }
            }
            Ok(entries)
        } else {
            Ok(vec![])
        }
    }

    // System Monitoring - Enhanced with MCP tools
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "health_check",
            "arguments": {}
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        let status = if response.result.is_some() { "healthy" } else { "unhealthy" };
        
        Ok(SystemHealth {
            status: status.to_string(),
            components: HashMap::new(),
            timestamp: chrono::Utc::now(),
        })
    }

    pub async fn get_performance_report(&self, timeframe: Option<&str>) -> Result<Value> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "performance_report",
            "arguments": {
                "timeframe": timeframe.unwrap_or("24h"),
                "format": "detailed"
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            serde_json::from_str(content)
                .map_err(|e| ConnectorError::InvalidResponse(format!("Failed to parse performance report: {}", e)))
        } else {
            Err(ConnectorError::InvalidResponse("No result in performance report response".to_string()))
        }
    }

    pub async fn get_system_metrics(&self) -> Result<SystemMetrics> {
        self.ensure_initialized()?;

        // Use performance_report tool to get metrics
        let params = json!({
            "name": "performance_report",
            "arguments": {
                "format": "detailed",
                "timeframe": "24h"
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            // Convert to SystemMetrics
            Ok(SystemMetrics {
                total_requests: tool_result.get("requests")
                    .and_then(|r| r.get("total"))
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0),
                successful_requests: tool_result.get("requests")
                    .and_then(|r| r.get("successful"))
                    .and_then(|s| s.as_u64())
                    .unwrap_or(0),
                failed_requests: tool_result.get("requests")
                    .and_then(|r| r.get("failed"))
                    .and_then(|f| f.as_u64())
                    .unwrap_or(0),
                average_response_time: tool_result.get("performance")
                    .and_then(|p| p.get("average_response_time"))
                    .and_then(|a| a.as_f64())
                    .unwrap_or(0.0),
                active_sessions: tool_result.get("sessions")
                    .and_then(|s| s.get("active"))
                    .and_then(|a| a.as_u64())
                    .unwrap_or(0) as u32,
                tool_invocations: tool_result.get("tool_invocations")
                    .and_then(|t| t.as_object())
                    .map(|obj| obj.iter()
                        .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                        .collect())
                    .unwrap_or_default(),
                errors: tool_result.get("errors")
                    .and_then(|e| e.as_object())
                    .map(|obj| obj.iter()
                        .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                        .collect())
                    .unwrap_or_default(),
                last_reset: chrono::Utc::now(),
            })
        } else {
            Err(ConnectorError::InvalidResponse("No result in system metrics response".to_string()))
        }
    }

    // Swarm Operations - Enhanced with MCP tools
    pub async fn init_swarm(&self, topology: &str, max_agents: Option<u32>) -> Result<String> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "swarm_init",
            "arguments": {
                "topology": topology,
                "maxAgents": max_agents,
                "strategy": "adaptive"
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            if let Some(swarm_id) = tool_result.get("swarmId").and_then(|s| s.as_str()) {
                Ok(swarm_id.to_string())
            } else {
                Err(ConnectorError::InvalidResponse("No swarmId in response".to_string()))
            }
        } else {
            Err(ConnectorError::InvalidResponse("No result in init swarm response".to_string()))
        }
    }

    pub async fn get_swarm_status(&self) -> Result<SwarmStatus> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "swarm_status",
            "arguments": {}
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            // Convert response data to SwarmStatus
            Ok(SwarmStatus {
                id: tool_result.get("swarmId")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                topology: tool_result.get("topology")
                    .and_then(|t| t.as_str())
                    .unwrap_or("hierarchical")
                    .to_string(),
                strategy: tool_result.get("strategy")
                    .and_then(|s| s.as_str())
                    .unwrap_or("adaptive")
                    .to_string(),
                total_agents: tool_result.get("agentCount")
                    .and_then(|a| a.as_u64())
                    .unwrap_or(0) as u32,
                active_agents: tool_result.get("activeAgents")
                    .and_then(|a| a.as_u64())
                    .unwrap_or(0) as u32,
                total_tasks: tool_result.get("taskCount")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0) as u32,
                completed_tasks: tool_result.get("completedTasks")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0) as u32,
                uptime: tool_result.get("uptime")
                    .and_then(|u| u.as_u64())
                    .unwrap_or(0),
                status: tool_result.get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("active")
                    .to_string(),
            })
        } else {
            Err(ConnectorError::InvalidResponse("No result in swarm status response".to_string()))
        }
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

        let params = json!({
            "name": "neural_train",
            "arguments": {
                "pattern_type": pattern_type,
                "training_data": training_data,
                "epochs": epochs.unwrap_or(50)
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            serde_json::from_str(content)
                .map_err(|e| ConnectorError::InvalidResponse(format!("Failed to parse neural train response: {}", e)))
        } else {
            Err(ConnectorError::InvalidResponse("No result in neural train response".to_string()))
        }
    }

    pub async fn neural_predict(&self, model_id: &str, input: &str) -> Result<Value> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "neural_predict",
            "arguments": {
                "modelId": model_id,
                "input": input
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            serde_json::from_str(content)
                .map_err(|e| ConnectorError::InvalidResponse(format!("Failed to parse neural predict response: {}", e)))
        } else {
            Err(ConnectorError::InvalidResponse("No result in neural predict response".to_string()))
        }
    }

    // Task Orchestration
    pub async fn orchestrate_task(&self, task: &str, strategy: Option<&str>, priority: Option<&str>) -> Result<String> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "task_orchestrate",
            "arguments": {
                "task": task,
                "strategy": strategy.unwrap_or("adaptive"),
                "priority": priority.unwrap_or("medium")
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            let tool_result: Value = serde_json::from_str(content)?;
            
            if let Some(task_id) = tool_result.get("taskId").and_then(|t| t.as_str()) {
                Ok(task_id.to_string())
            } else {
                Err(ConnectorError::InvalidResponse("No taskId in response".to_string()))
            }
        } else {
            Err(ConnectorError::InvalidResponse("No result in orchestrate task response".to_string()))
        }
    }

    pub async fn get_task_status(&self, task_id: &str) -> Result<Value> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "task_status",
            "arguments": {
                "taskId": task_id
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            serde_json::from_str(content)
                .map_err(|e| ConnectorError::InvalidResponse(format!("Failed to parse task status response: {}", e)))
        } else {
            Err(ConnectorError::InvalidResponse("No result in task status response".to_string()))
        }
    }

    pub async fn get_task_results(&self, task_id: &str) -> Result<Value> {
        self.ensure_initialized()?;

        let params = json!({
            "name": "task_results",
            "arguments": {
                "taskId": task_id
            }
        });

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(request).await?;

        if let Some(result) = response.result {
            // Parse the tool call result
            let content = result.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|text| text.as_str())
                .ok_or_else(|| ConnectorError::InvalidResponse("Invalid tool response format".to_string()))?;
            
            serde_json::from_str(content)
                .map_err(|e| ConnectorError::InvalidResponse(format!("Failed to parse task results response: {}", e)))
        } else {
            Err(ConnectorError::InvalidResponse("No result in task results response".to_string()))
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
            transport_type: TransportType::WebSocket, // Default to WebSocket
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