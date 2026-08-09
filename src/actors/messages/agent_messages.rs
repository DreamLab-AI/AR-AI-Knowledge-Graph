//! Agent/bot/swarm-domain messages: Claude Flow agent lifecycle, swarm orchestration,
//! neural network, memory persistence, performance monitoring, and MCP tool responses.
//!
//! Domain-safe types have been moved to `visionclaw_actors::messages::agent_messages`.
//! This file re-exports them and defines the webxr-internal types that cannot move.

// ---------------------------------------------------------------------------
// Re-export everything from the domain crate
// ---------------------------------------------------------------------------

pub use visionclaw_actors::messages::agent_messages::{
    AgentMetrics, AgentUpdate, Bottleneck, BottleneckAnalyze, CloseTcpConnection, ConnectionFailed,
    CoordinationPattern, CoordinationSync, EstablishTcpConnection, GetAgentMetrics,
    GetBotsGraphData, GetCachedAgentStatuses, GetNeuralStatus, GetPerformanceReport,
    GetSwarmStatus, InitializeJsonRpc, InitializeSwarm, LoadBalance, MemoryPersist, MemorySearch,
    MessageFlowEvent, MetricsCollect, NeuralPredict, NeuralStatus, NeuralTrain, PerformanceReport,
    PollAgentStatuses, PollSwarmData, PollSystemMetrics, RecordPollFailure, RecordPollSuccess,
    RetryMCPConnection, SpawnAgent, SpawnAgentCommand, StateSnapshot, SwarmDestroy, SwarmMonitor,
    SwarmMonitorData, SwarmScale, SwarmStatus, SystemMetrics, TaskOrchestrate, TaskStatusChanged,
    TopologyOptimize, UpdateAgentCache,
};

// ---------------------------------------------------------------------------
// Webxr-internal types (cannot move to domain crate)
// ---------------------------------------------------------------------------

use actix::prelude::*;

/// Update the bots graph with agents from the external bots service.
/// Blocked: references `crate::services::bots_client::Agent` (webxr-internal).
#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateBotsGraph {
    pub agents: Vec<crate::services::bots_client::Agent>,
}

/// Injects the `AgentMonitorActor` address into `TaskOrchestratorActor`.
///
/// Blocked: references `actix::Addr<crate::actors::AgentMonitorActor>` (webxr-internal).
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetAgentMonitorAddr {
    pub addr: actix::Addr<crate::actors::AgentMonitorActor>,
}
