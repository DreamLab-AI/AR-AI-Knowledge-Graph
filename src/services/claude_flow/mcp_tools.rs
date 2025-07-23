use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// All available Claude Flow MCP tools
#[derive(Debug, Clone)]
pub enum McpTool {
    // Swarm Coordination Tools (12)
    SwarmInit { topology: String, max_agents: Option<u32>, strategy: Option<String> },
    AgentSpawn { agent_type: String, name: Option<String>, capabilities: Option<Vec<String>>, swarm_id: Option<String> },
    TaskOrchestrate { task: String, strategy: Option<String>, priority: Option<String>, dependencies: Option<Vec<String>> },
    SwarmStatus { swarm_id: Option<String> },
    AgentList { swarm_id: Option<String> },
    AgentMetrics { agent_id: Option<String> },
    SwarmMonitor { swarm_id: Option<String>, interval: Option<f64> },
    TopologyOptimize { swarm_id: Option<String> },
    LoadBalance { swarm_id: Option<String>, tasks: Option<Vec<Value>> },
    CoordinationSync { swarm_id: Option<String> },
    SwarmScale { swarm_id: Option<String>, target_size: Option<u32> },
    SwarmDestroy { swarm_id: String },

    // Neural Network Tools (15)
    NeuralStatus { model_id: Option<String> },
    NeuralTrain { pattern_type: String, training_data: String, epochs: Option<u32> },
    NeuralPatterns { action: String, operation: Option<String>, outcome: Option<String>, metadata: Option<Value> },
    NeuralPredict { model_id: String, input: String },
    ModelLoad { model_path: String },
    ModelSave { model_id: String, path: String },
    WasmOptimize { operation: Option<String> },
    InferenceRun { model_id: String, data: Vec<Value> },
    PatternRecognize { data: Vec<Value>, patterns: Option<Vec<Value>> },
    CognitiveAnalyze { behavior: String },
    LearningAdapt { experience: Value },
    NeuralCompress { model_id: String, ratio: Option<f64> },
    EnsembleCreate { models: Vec<String>, strategy: Option<String> },
    TransferLearn { source_model: String, target_domain: String },
    NeuralExplain { model_id: String, prediction: Value },

    // Memory & Persistence Tools (12)
    MemoryUsage { action: String, key: Option<String>, value: Option<String>, namespace: Option<String>, ttl: Option<u64> },
    MemorySearch { pattern: String, namespace: Option<String>, limit: Option<u32> },
    MemoryPersist { session_id: Option<String> },
    MemoryNamespace { namespace: String, action: String },
    MemoryBackup { path: Option<String> },
    MemoryRestore { backup_path: String },
    MemoryCompress { namespace: Option<String> },
    MemorySync { target: String },
    CacheManage { action: String, key: Option<String> },
    StateSnapshot { name: Option<String> },
    ContextRestore { snapshot_id: String },
    MemoryAnalytics { timeframe: Option<String> },

    // Analysis & Monitoring Tools (13)
    TaskStatus { task_id: String },
    TaskResults { task_id: String },
    BenchmarkRun { suite: Option<String> },
    BottleneckAnalyze { component: Option<String>, metrics: Option<Vec<Value>> },
    PerformanceReport { timeframe: Option<String>, format: Option<String> },
    TokenUsage { operation: Option<String>, timeframe: Option<String> },
    MetricsCollect { components: Option<Vec<String>> },
    TrendAnalysis { metric: String, period: Option<String> },
    CostAnalysis { timeframe: Option<String> },
    QualityAssess { target: String, criteria: Option<Vec<String>> },
    ErrorAnalysis { logs: Option<Vec<Value>> },
    UsageStats { component: Option<String> },
    HealthCheck { components: Option<Vec<String>> },

    // GitHub Integration Tools (8)
    GithubRepoAnalyze { repo: String, analysis_type: Option<String> },
    GithubPrManage { repo: String, pr_number: Option<u32>, action: String },
    GithubIssueTrack { repo: String, action: String },
    GithubReleaseCoord { repo: String, version: String },
    GithubWorkflowAuto { repo: String, workflow: Value },
    GithubCodeReview { repo: String, pr: u32 },
    GithubSyncCoord { repos: Vec<String> },
    GithubMetrics { repo: String },

    // DAA (Decentralized Autonomous Agents) Tools (8)
    DaaAgentCreate { agent_type: String, capabilities: Option<Vec<String>>, resources: Option<Value> },
    DaaCapabilityMatch { task_requirements: Vec<String>, available_agents: Option<Vec<Value>> },
    DaaResourceAlloc { resources: Value, agents: Option<Vec<Value>> },
    DaaLifecycleManage { agent_id: String, action: String },
    DaaCommunication { from: String, to: String, message: Value },
    DaaConsensus { agents: Vec<Value>, proposal: Value },
    DaaFaultTolerance { agent_id: String, strategy: Option<String> },
    DaaOptimization { target: String, metrics: Option<Vec<Value>> },

    // Workflow Tools (11)
    WorkflowCreate { name: String, steps: Vec<Value>, triggers: Option<Vec<Value>> },
    SparcMode { mode: String, task_description: String, options: Option<Value> },
    WorkflowExecute { workflow_id: String, params: Option<Value> },
    WorkflowExport { workflow_id: String, format: Option<String> },
    AutomationSetup { rules: Vec<Value> },
    PipelineCreate { config: Value },
    SchedulerManage { action: String, schedule: Option<Value> },
    TriggerSetup { events: Vec<Value>, actions: Vec<Value> },
    WorkflowTemplate { action: String, template: Option<Value> },
    BatchProcess { items: Vec<Value>, operation: String },
    ParallelExecute { tasks: Vec<Value> },

    // System & Utilities Tools (8)
    TerminalExecute { command: String, args: Option<Vec<String>> },
    ConfigManage { action: String, config: Option<Value> },
    FeaturesDetect { component: Option<String> },
    SecurityScan { target: String, depth: Option<String> },
    BackupCreate { components: Option<Vec<String>>, destination: Option<String> },
    RestoreSystem { backup_id: String },
    LogAnalysis { log_file: String, patterns: Option<Vec<String>> },
    DiagnosticRun { components: Option<Vec<String>> },
}

impl McpTool {
    /// Get the tool name for MCP protocol
    pub fn name(&self) -> &'static str {
        match self {
            // Swarm Coordination
            McpTool::SwarmInit { .. } => "swarm_init",
            McpTool::AgentSpawn { .. } => "agent_spawn",
            McpTool::TaskOrchestrate { .. } => "task_orchestrate",
            McpTool::SwarmStatus { .. } => "swarm_status",
            McpTool::AgentList { .. } => "agent_list",
            McpTool::AgentMetrics { .. } => "agent_metrics",
            McpTool::SwarmMonitor { .. } => "swarm_monitor",
            McpTool::TopologyOptimize { .. } => "topology_optimize",
            McpTool::LoadBalance { .. } => "load_balance",
            McpTool::CoordinationSync { .. } => "coordination_sync",
            McpTool::SwarmScale { .. } => "swarm_scale",
            McpTool::SwarmDestroy { .. } => "swarm_destroy",

            // Neural Network
            McpTool::NeuralStatus { .. } => "neural_status",
            McpTool::NeuralTrain { .. } => "neural_train",
            McpTool::NeuralPatterns { .. } => "neural_patterns",
            McpTool::NeuralPredict { .. } => "neural_predict",
            McpTool::ModelLoad { .. } => "model_load",
            McpTool::ModelSave { .. } => "model_save",
            McpTool::WasmOptimize { .. } => "wasm_optimize",
            McpTool::InferenceRun { .. } => "inference_run",
            McpTool::PatternRecognize { .. } => "pattern_recognize",
            McpTool::CognitiveAnalyze { .. } => "cognitive_analyze",
            McpTool::LearningAdapt { .. } => "learning_adapt",
            McpTool::NeuralCompress { .. } => "neural_compress",
            McpTool::EnsembleCreate { .. } => "ensemble_create",
            McpTool::TransferLearn { .. } => "transfer_learn",
            McpTool::NeuralExplain { .. } => "neural_explain",

            // Memory & Persistence
            McpTool::MemoryUsage { .. } => "memory_usage",
            McpTool::MemorySearch { .. } => "memory_search",
            McpTool::MemoryPersist { .. } => "memory_persist",
            McpTool::MemoryNamespace { .. } => "memory_namespace",
            McpTool::MemoryBackup { .. } => "memory_backup",
            McpTool::MemoryRestore { .. } => "memory_restore",
            McpTool::MemoryCompress { .. } => "memory_compress",
            McpTool::MemorySync { .. } => "memory_sync",
            McpTool::CacheManage { .. } => "cache_manage",
            McpTool::StateSnapshot { .. } => "state_snapshot",
            McpTool::ContextRestore { .. } => "context_restore",
            McpTool::MemoryAnalytics { .. } => "memory_analytics",

            // Analysis & Monitoring
            McpTool::TaskStatus { .. } => "task_status",
            McpTool::TaskResults { .. } => "task_results",
            McpTool::BenchmarkRun { .. } => "benchmark_run",
            McpTool::BottleneckAnalyze { .. } => "bottleneck_analyze",
            McpTool::PerformanceReport { .. } => "performance_report",
            McpTool::TokenUsage { .. } => "token_usage",
            McpTool::MetricsCollect { .. } => "metrics_collect",
            McpTool::TrendAnalysis { .. } => "trend_analysis",
            McpTool::CostAnalysis { .. } => "cost_analysis",
            McpTool::QualityAssess { .. } => "quality_assess",
            McpTool::ErrorAnalysis { .. } => "error_analysis",
            McpTool::UsageStats { .. } => "usage_stats",
            McpTool::HealthCheck { .. } => "health_check",

            // GitHub Integration
            McpTool::GithubRepoAnalyze { .. } => "github_repo_analyze",
            McpTool::GithubPrManage { .. } => "github_pr_manage",
            McpTool::GithubIssueTrack { .. } => "github_issue_track",
            McpTool::GithubReleaseCoord { .. } => "github_release_coord",
            McpTool::GithubWorkflowAuto { .. } => "github_workflow_auto",
            McpTool::GithubCodeReview { .. } => "github_code_review",
            McpTool::GithubSyncCoord { .. } => "github_sync_coord",
            McpTool::GithubMetrics { .. } => "github_metrics",

            // DAA
            McpTool::DaaAgentCreate { .. } => "daa_agent_create",
            McpTool::DaaCapabilityMatch { .. } => "daa_capability_match",
            McpTool::DaaResourceAlloc { .. } => "daa_resource_alloc",
            McpTool::DaaLifecycleManage { .. } => "daa_lifecycle_manage",
            McpTool::DaaCommunication { .. } => "daa_communication",
            McpTool::DaaConsensus { .. } => "daa_consensus",
            McpTool::DaaFaultTolerance { .. } => "daa_fault_tolerance",
            McpTool::DaaOptimization { .. } => "daa_optimization",

            // Workflow
            McpTool::WorkflowCreate { .. } => "workflow_create",
            McpTool::SparcMode { .. } => "sparc_mode",
            McpTool::WorkflowExecute { .. } => "workflow_execute",
            McpTool::WorkflowExport { .. } => "workflow_export",
            McpTool::AutomationSetup { .. } => "automation_setup",
            McpTool::PipelineCreate { .. } => "pipeline_create",
            McpTool::SchedulerManage { .. } => "scheduler_manage",
            McpTool::TriggerSetup { .. } => "trigger_setup",
            McpTool::WorkflowTemplate { .. } => "workflow_template",
            McpTool::BatchProcess { .. } => "batch_process",
            McpTool::ParallelExecute { .. } => "parallel_execute",

            // System & Utilities
            McpTool::TerminalExecute { .. } => "terminal_execute",
            McpTool::ConfigManage { .. } => "config_manage",
            McpTool::FeaturesDetect { .. } => "features_detect",
            McpTool::SecurityScan { .. } => "security_scan",
            McpTool::BackupCreate { .. } => "backup_create",
            McpTool::RestoreSystem { .. } => "restore_system",
            McpTool::LogAnalysis { .. } => "log_analysis",
            McpTool::DiagnosticRun { .. } => "diagnostic_run",
        }
    }

    /// Convert tool to MCP arguments
    pub fn to_arguments(&self) -> Value {
        match self {
            McpTool::SwarmInit { topology, max_agents, strategy } => {
                let mut args = json!({ "topology": topology });
                if let Some(max) = max_agents { args["maxAgents"] = json!(max); }
                if let Some(s) = strategy { args["strategy"] = json!(s); }
                args
            }
            McpTool::AgentSpawn { agent_type, name, capabilities, swarm_id } => {
                let mut args = json!({ "type": agent_type });
                if let Some(n) = name { args["name"] = json!(n); }
                if let Some(c) = capabilities { args["capabilities"] = json!(c); }
                if let Some(s) = swarm_id { args["swarmId"] = json!(s); }
                args
            }
            McpTool::TaskOrchestrate { task, strategy, priority, dependencies } => {
                let mut args = json!({ "task": task });
                if let Some(s) = strategy { args["strategy"] = json!(s); }
                if let Some(p) = priority { args["priority"] = json!(p); }
                if let Some(d) = dependencies { args["dependencies"] = json!(d); }
                args
            }
            McpTool::SwarmStatus { swarm_id } => {
                let mut args = json!({});
                if let Some(s) = swarm_id { args["swarmId"] = json!(s); }
                args
            }
            McpTool::AgentList { swarm_id } => {
                let mut args = json!({});
                if let Some(s) = swarm_id { args["swarmId"] = json!(s); }
                args
            }
            McpTool::AgentMetrics { agent_id } => {
                let mut args = json!({});
                if let Some(a) = agent_id { args["agentId"] = json!(a); }
                args
            }
            McpTool::MemoryUsage { action, key, value, namespace, ttl } => {
                let mut args = json!({ "action": action });
                if let Some(k) = key { args["key"] = json!(k); }
                if let Some(v) = value { args["value"] = json!(v); }
                if let Some(n) = namespace { args["namespace"] = json!(n); }
                if let Some(t) = ttl { args["ttl"] = json!(t); }
                args
            }
            McpTool::MemorySearch { pattern, namespace, limit } => {
                let mut args = json!({ "pattern": pattern });
                if let Some(n) = namespace { args["namespace"] = json!(n); }
                if let Some(l) = limit { args["limit"] = json!(l); }
                args
            }
            McpTool::PerformanceReport { timeframe, format } => {
                let mut args = json!({});
                if let Some(t) = timeframe { args["timeframe"] = json!(t); }
                if let Some(f) = format { args["format"] = json!(f); }
                args
            }
            McpTool::NeuralTrain { pattern_type, training_data, epochs } => {
                let mut args = json!({ "pattern_type": pattern_type, "training_data": training_data });
                if let Some(e) = epochs { args["epochs"] = json!(e); }
                args
            }
            // Add more conversions as needed...
            _ => json!({}), // Default empty args for tools not fully implemented
        }
    }
}

/// Parse tool response from MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub success: bool,
    #[serde(flatten)]
    pub data: Value,
}

impl ToolResponse {
    /// Parse from MCP response content
    pub fn from_mcp_response(response: &super::types::McpResponse) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(result) = &response.result {
            if let Some(content) = result.as_object().and_then(|obj| obj.get("content")) {
                if let Some(text_content) = content.get(0).and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
                    let data: Value = serde_json::from_str(text_content)?;
                    Ok(ToolResponse {
                        success: data.get("success").and_then(|s| s.as_bool()).unwrap_or(false),
                        data,
                    })
                } else {
                    Err("No text content in MCP response".into())
                }
            } else {
                Err("No content in MCP response".into())
            }
        } else {
            Err("No result in MCP response".into())
        }
    }
}