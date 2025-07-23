use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// MCP Protocol Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

// Initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: ProtocolVersion,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub tools: ToolCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: ProtocolVersion,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub logging: LoggingCapabilities,
    pub tools: ToolCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingCapabilities {
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

// Agent Types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Coordinator,
    Researcher,
    Coder,
    Analyst,
    Architect,
    Tester,
    Reviewer,
    Optimizer,
    Documenter,
    Monitor,
    Specialist,
}

impl AgentType {
    pub fn from_str(s: &str) -> Result<Self, &'static str> {
        match s.to_lowercase().as_str() {
            "coordinator" => Ok(AgentType::Coordinator),
            "researcher" => Ok(AgentType::Researcher),
            "coder" => Ok(AgentType::Coder),
            "analyst" => Ok(AgentType::Analyst),
            "architect" => Ok(AgentType::Architect),
            "tester" => Ok(AgentType::Tester),
            "reviewer" => Ok(AgentType::Reviewer),
            "optimizer" => Ok(AgentType::Optimizer),
            "documenter" => Ok(AgentType::Documenter),
            "monitor" => Ok(AgentType::Monitor),
            "specialist" => Ok(AgentType::Specialist),
            _ => Err("Unknown agent type"),
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Coordinator => write!(f, "coordinator"),
            AgentType::Researcher => write!(f, "researcher"),
            AgentType::Coder => write!(f, "coder"),
            AgentType::Analyst => write!(f, "analyst"),
            AgentType::Architect => write!(f, "architect"),
            AgentType::Tester => write!(f, "tester"),
            AgentType::Reviewer => write!(f, "reviewer"),
            AgentType::Optimizer => write!(f, "optimizer"),
            AgentType::Documenter => write!(f, "documenter"),
            AgentType::Monitor => write!(f, "monitor"),
            AgentType::Specialist => write!(f, "specialist"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub name: String,
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub capabilities: Vec<String>,
    #[serde(rename = "systemPrompt", skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(rename = "maxConcurrentTasks")]
    pub max_concurrent_tasks: u32,
    pub priority: u32,
    #[serde(rename = "retryPolicy", default)]
    pub retry_policy: RetryPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    #[serde(rename = "workingDirectory", skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub status: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub profile: AgentProfile,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "activeTasksCount")]
    pub active_tasks_count: u32,
    #[serde(rename = "completedTasksCount")]
    pub completed_tasks_count: u32,
    #[serde(rename = "failedTasksCount", default)]
    pub failed_tasks_count: u32,
    #[serde(rename = "totalExecutionTime", default)]
    pub total_execution_time: u64,
    #[serde(rename = "averageTaskDuration", default)]
    pub average_task_duration: f64,
    #[serde(rename = "successRate", default)]
    pub success_rate: f64,
    #[serde(rename = "currentTask", skip_serializing_if = "Option::is_none")]
    pub current_task: Option<TaskReference>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReference {
    #[serde(rename = "taskId")]
    pub task_id: String,
    pub description: String,
    #[serde(rename = "startedAt")]
    pub started_at: DateTime<Utc>,
}

// Task Types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Queued,
    Assigned,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub description: String,
    pub priority: u32,
    pub dependencies: Vec<String>,
    #[serde(rename = "assignedAgent", skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<String>,
    pub status: TaskStatus,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "startedAt", skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(rename = "completedAt", skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

// Memory Types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Observation,
    Insight,
    Decision,
    Artifact,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
    pub content: String,
    pub context: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub tags: Vec<String>,
    pub version: u32,
    #[serde(rename = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

// System Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: String,
    pub components: HashMap<String, ComponentHealth>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: String,
    #[serde(rename = "lastCheck")]
    pub last_check: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<HashMap<String, f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    #[serde(rename = "totalRequests")]
    pub total_requests: u64,
    #[serde(rename = "successfulRequests")]
    pub successful_requests: u64,
    #[serde(rename = "failedRequests")]
    pub failed_requests: u64,
    #[serde(rename = "averageResponseTime")]
    pub average_response_time: f64,
    #[serde(rename = "activeSessions")]
    pub active_sessions: u32,
    #[serde(rename = "toolInvocations")]
    pub tool_invocations: HashMap<String, u64>,
    pub errors: HashMap<String, u64>,
    #[serde(rename = "lastReset")]
    pub last_reset: DateTime<Utc>,
}

// Tool Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

// Swarm Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatus {
    pub id: String,
    pub topology: String,
    pub strategy: String,
    #[serde(rename = "totalAgents")]
    pub total_agents: u32,
    #[serde(rename = "activeAgents")]
    pub active_agents: u32,
    #[serde(rename = "totalTasks")]
    pub total_tasks: u32,
    #[serde(rename = "completedTasks")]
    pub completed_tasks: u32,
    pub uptime: u64,
    pub status: String,
}