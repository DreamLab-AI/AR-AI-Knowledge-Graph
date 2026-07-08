//! Agent Monitor Actor - Monitoring via Management API
//!
//! This actor focuses solely on:
//! - Polling the Management API (port 9090) for active task statuses
//! - Converting tasks to agent nodes
//! - Forwarding updates to GraphServiceSupervisor
//!
//! All task management is handled by TaskOrchestratorActor.
//! This actor only monitors and displays running agents.

use actix::prelude::*;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::time::Duration;

use crate::actors::messages::*;
use crate::services::management_api_client::{ManagementApiClient, TaskInfo};
use visionclaw_domain::types::claude_flow::{
    AgentProfile, AgentStatus, AgentType, ClaudeFlowClient, PerformanceMetrics, TokenUsage,
};
use crate::utils::time;

/// Container telemetry metrics from Management API /v1/status
#[derive(Debug, Clone, Default)]
struct ContainerTelemetry {
    cpu_usage: f32,
    memory_usage_mb: f32,
}

fn task_to_agent_status(task: TaskInfo, telemetry: &ContainerTelemetry) -> AgentStatus {
    use chrono::TimeZone;

    let agent_type_enum = match task.agent.as_str() {
        "coder" => AgentType::Coder,
        "planner" => AgentType::Coordinator,
        "researcher" => AgentType::Researcher,
        "reviewer" => AgentType::Analyst,
        "tester" => AgentType::Tester,
        _ => AgentType::Coordinator,
    };

    let timestamp = chrono::Utc
        .timestamp_millis_opt(task.start_time as i64)
        .single()
        .unwrap_or_else(|| time::now());

    let age = (time::timestamp_millis() - task.start_time as i64) / 1000;

    AgentStatus {
        agent_id: task.task_id.clone(),
        profile: AgentProfile {
            name: format!("{} ({})", task.agent, &task.task_id[..8]),
            agent_type: agent_type_enum,
            capabilities: vec![format!("Provider: {}", task.provider)],
            description: Some(task.task.chars().take(100).collect::<String>()),
            version: "1.0.0".to_string(),
            tags: vec![task.agent.clone(), task.provider.clone()],
        },
        status: format!("{:?}", task.status),
        active_tasks_count: 1,
        completed_tasks_count: 0,
        failed_tasks_count: 0,
        success_rate: 100.0,
        timestamp,
        current_task: None,
        agent_type: task.agent.clone(),
        current_task_description: Some(task.task.clone()),
        capabilities: vec![format!("Provider: {}", task.provider)],
        position: None,
        cpu_usage: telemetry.cpu_usage,
        memory_usage: telemetry.memory_usage_mb,
        health: 1.0,
        activity: 0.8,
        tasks_active: 1,
        tasks_completed: 0,
        success_rate_normalized: 1.0,
        tokens: 0,
        token_rate: 0.0,
        performance_metrics: PerformanceMetrics {
            tasks_completed: 0,
            success_rate: 100.0,
        },
        token_usage: TokenUsage {
            total: 0,
            token_rate: 0.0,
        },
        swarm_id: None,
        agent_mode: Some("active".to_string()),
        parent_queen_id: None,
        processing_logs: None,
        created_at: timestamp.to_rfc3339(),
        age: age as u64,
        workload: Some(0.5),
    }
}

pub struct AgentMonitorActor {
    _client: ClaudeFlowClient,
    graph_service_addr: Addr<crate::actors::GraphServiceSupervisor>,
    management_api_client: ManagementApiClient,

    is_connected: bool,

    polling_interval: Duration,
    #[allow(dead_code)]
    last_poll: DateTime<Utc>,

    agent_cache: HashMap<String, AgentStatus>,

    consecutive_poll_failures: u32,
    last_successful_poll: Option<DateTime<Utc>>,

    /// Cached container telemetry from Management API /v1/status
    container_telemetry: ContainerTelemetry,

    /// ADR-031 item 1: Round-robin poll offset.
    /// Incremented each successful poll so the golden-spiral starting index
    /// rotates, preventing the same agent from always occupying the apex 3-D
    /// position. Adapted from Multica's `pollOffset` daemon fairness pattern.
    poll_offset: usize,
}

impl AgentMonitorActor {
    pub fn new(
        client: ClaudeFlowClient,
        graph_service_addr: Addr<crate::actors::GraphServiceSupervisor>,
    ) -> Self {
        info!("[AgentMonitorActor] Initializing with Management API monitoring");

        let host = std::env::var("MANAGEMENT_API_HOST")
            .unwrap_or_else(|_| "agentic-workstation".to_string());
        let port = std::env::var("MANAGEMENT_API_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(9090);
        // SECURITY: Management API key is required - no insecure fallback
        let api_key = std::env::var("MANAGEMENT_API_KEY").unwrap_or_else(|_| {
            warn!("[AgentMonitorActor] MANAGEMENT_API_KEY not set - Management API client will be disabled");
            String::new()
        });

        let management_api_client = ManagementApiClient::new(host, port, api_key);

        Self {
            _client: client,
            graph_service_addr,
            management_api_client,
            is_connected: false,
            // Idle telemetry cadence. Each tick hits BOTH agentbox /v1/tasks and
            // /v1/status, so 3s flooded the management-api log. Task-status changes
            // still re-poll immediately via the TaskStatusChanged push from
            // TaskOrchestratorActor, so on-demand responsiveness is unaffected;
            // this only governs the idle telemetry refresh.
            polling_interval: Duration::from_secs(15),
            last_poll: time::now(),
            agent_cache: HashMap::new(),
            consecutive_poll_failures: 0,
            last_successful_poll: None,
            container_telemetry: ContainerTelemetry::default(),
            poll_offset: 0,
        }
    }

    fn poll_agent_statuses(&mut self, ctx: &mut Context<Self>) {
        debug!("[AgentMonitorActor] Polling active tasks from Management API");

        let api_client = self.management_api_client.clone();
        let ctx_addr = ctx.address();

        ctx.spawn(
            async move {
                // Fetch tasks and system status concurrently
                let (tasks_result, status_result) = tokio::join!(
                    api_client.list_tasks(),
                    api_client.get_system_status()
                );

                // Extract container telemetry from system status
                let telemetry = match &status_result {
                    Ok(sys_status) => {
                        let cpu = sys_status
                            .system
                            .get("cpu")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0) as f32;
                        let mem = sys_status
                            .system
                            .get("memory")
                            .and_then(|v| {
                                // Try nested "used_mb" first, then top-level numeric
                                v.get("used_mb")
                                    .and_then(|m| m.as_f64())
                                    .or_else(|| v.as_f64())
                            })
                            .unwrap_or(0.0) as f32;
                        ContainerTelemetry {
                            cpu_usage: cpu,
                            memory_usage_mb: mem,
                        }
                    }
                    Err(e) => {
                        debug!(
                            "[AgentMonitorActor] System status unavailable, using defaults: {}",
                            e
                        );
                        ContainerTelemetry::default()
                    }
                };

                match tasks_result {
                    Ok(task_list) => {
                        let active_count = task_list.active_tasks.len();
                        debug!(
                            "[AgentMonitorActor] Retrieved {} active tasks from Management API",
                            active_count
                        );

                        let agents: Vec<AgentStatus> = task_list
                            .active_tasks
                            .into_iter()
                            .map(|task| task_to_agent_status(task, &telemetry))
                            .collect();

                        ctx_addr.do_send(ProcessAgentStatuses {
                            agents,
                            telemetry,
                        });
                    }
                    Err(e) => {
                        error!("[AgentMonitorActor] Management API query failed: {}", e);
                        ctx_addr.do_send(RecordPollFailure);
                    }
                }
            }
            .into_actor(self),
        );
    }

    /// Delay before the next poll, with exponential backoff on consecutive
    /// failures. The Management API shares a per-key rate-limit bucket with
    /// task creation (settings commands, swarm init); polling at a fixed 3s
    /// through a 429 storm both floods the log and starves that bucket so
    /// legitimate task creation is rejected. Backing off lets the bucket
    /// refill. base→2x per failure, capped at 90s; resets to base on success.
    /// Cap is deliberately above agentbox's 60s rate-limit window: that limiter
    /// uses `continueExceeding`, which re-arms the full window on every
    /// over-limit hit. A retry landing at the 60s boundary would re-lock the
    /// bucket forever, so we wait past the window to let it fully refill.
    fn next_poll_delay(&self) -> Duration {
        if self.consecutive_poll_failures == 0 {
            return self.polling_interval;
        }
        let shift = self.consecutive_poll_failures.min(5);
        let scaled = self.polling_interval.saturating_mul(1u32 << shift);
        scaled.min(Duration::from_secs(90))
    }

    /// Self-rescheduling poll loop. Each tick polls once, then schedules the
    /// next tick using `next_poll_delay()` so the cadence adapts to failures.
    fn schedule_next_poll(&self, ctx: &mut Context<Self>) {
        let delay = self.next_poll_delay();
        ctx.run_later(delay, |act, ctx| {
            act.poll_agent_statuses(ctx);
            act.schedule_next_poll(ctx);
        });
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct ProcessAgentStatuses {
    agents: Vec<AgentStatus>,
    telemetry: ContainerTelemetry,
}

impl Actor for AgentMonitorActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[AgentMonitorActor] Started - beginning MCP TCP polling");

        self.is_connected = true;

        ctx.address()
            .do_send(crate::actors::messages::InitializeActor);
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("[AgentMonitorActor] Stopped");
    }
}

impl Handler<crate::actors::messages::InitializeActor> for AgentMonitorActor {
    type Result = ();

    fn handle(
        &mut self,
        _msg: crate::actors::messages::InitializeActor,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("[AgentMonitorActor] Initializing periodic polling (deferred from started)");

        ctx.run_later(Duration::from_millis(100), |act, ctx| {
            act.poll_agent_statuses(ctx);
            act.schedule_next_poll(ctx);
        });
    }
}

/// Build the default mock swarm agents for dev mode (MOCK_AGENTS=true).
fn build_mock_swarm_agents() -> Vec<AgentStatus> {
    use crate::utils::time;

    let mock_defs: Vec<(&str, &str, &str, &str, &str)> = vec![
        ("mock-coordinator", "Claude Opus 4.6 (Coordinator)", "coordinator", "active", "Orchestrating swarm topology and task routing"),
        ("mock-coder-1", "Coder Agent", "coder", "active", "Implementing feature branch with TDD"),
        ("mock-reviewer-1", "QE Reviewer", "reviewer", "active", "Reviewing PR #42 for security and correctness"),
        ("mock-researcher-1", "Research Agent", "researcher", "active", "Searching RuVector memory for related patterns"),
        ("mock-memory-1", "RuVector Memory Specialist", "memory", "idle", "Indexing 384-dim embeddings into HNSW graph"),
    ];

    mock_defs
        .into_iter()
        .map(|(id, name, agent_type, status, task)| {
            let agent_type_enum = match agent_type {
                "coordinator" => AgentType::Coordinator,
                "coder" => AgentType::Coder,
                "reviewer" => AgentType::Analyst,
                "researcher" => AgentType::Researcher,
                _ => AgentType::Generic,
            };
            let now = time::now();
            AgentStatus {
                agent_id: id.to_string(),
                profile: AgentProfile {
                    name: name.to_string(),
                    agent_type: agent_type_enum,
                    capabilities: vec![agent_type.to_string()],
                    description: Some(task.to_string()),
                    version: "1.0.0".to_string(),
                    tags: vec![agent_type.to_string(), "mock".to_string()],
                },
                status: status.to_string(),
                active_tasks_count: if status == "idle" { 0 } else { 1 },
                completed_tasks_count: 0,
                failed_tasks_count: 0,
                success_rate: 100.0,
                timestamp: now,
                current_task: None,
                agent_type: agent_type.to_string(),
                current_task_description: Some(task.to_string()),
                capabilities: vec![agent_type.to_string()],
                position: None,
                cpu_usage: 12.0,
                memory_usage: 128.0,
                health: 1.0,
                activity: if status == "idle" { 0.2 } else { 0.8 },
                tasks_active: if status == "idle" { 0 } else { 1 },
                tasks_completed: 0,
                success_rate_normalized: 1.0,
                tokens: 0,
                token_rate: 0.0,
                performance_metrics: PerformanceMetrics {
                    tasks_completed: 0,
                    success_rate: 100.0,
                },
                token_usage: TokenUsage {
                    total: 0,
                    token_rate: 0.0,
                },
                swarm_id: Some("mock-swarm-001".to_string()),
                agent_mode: Some("mock".to_string()),
                parent_queen_id: None,
                processing_logs: None,
                created_at: now.to_rfc3339(),
                age: 0,
                workload: Some(if status == "idle" { 0.2 } else { 0.7 }),
            }
        })
        .collect()
}

impl Handler<ProcessAgentStatuses> for AgentMonitorActor {
    type Result = ();

    fn handle(&mut self, msg: ProcessAgentStatuses, _ctx: &mut Self::Context) {
        // If management API returned 0 agents and MOCK_AGENTS is enabled, inject mock swarm
        let agents = if msg.agents.is_empty()
            && std::env::var("MOCK_AGENTS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false)
        {
            info!("[AgentMonitorActor] No real agents found, MOCK_AGENTS=true — injecting mock swarm");
            build_mock_swarm_agents()
        } else {
            msg.agents
        };

        info!(
            "[AgentMonitorActor] Processing {} agent statuses (cpu={:.1}%, mem={:.0}MB)",
            agents.len(),
            msg.telemetry.cpu_usage,
            msg.telemetry.memory_usage_mb
        );

        // Cache latest container telemetry
        self.container_telemetry = msg.telemetry;

        let agent_count = agents.len() as f32;
        // ADR-031 item 1: capture offset before the closure (borrow-checker).
        let spiral_offset = self.poll_offset;
        let agent_count_usize = agents.len().max(1);
        let agents_list: Vec<crate::services::bots_client::Agent> = agents
            .iter()
            .enumerate()
            .map(|(i, status)| {
                // Distribute agents in a golden angle spiral on a sphere.
                // Round-robin: rotate start index so the same agent does not
                // always occupy the apex position each poll cycle.
                let spiral_i = (i + spiral_offset) % agent_count_usize;
                let golden_angle = std::f32::consts::PI * (3.0 - (5.0_f32).sqrt());
                let theta = golden_angle * spiral_i as f32;
                let y_pos = 1.0 - (spiral_i as f32 / (agent_count - 1.0).max(1.0)) * 2.0;
                let radius_at_y = (1.0 - y_pos * y_pos).sqrt();
                let scale = 15.0; // Radius of agent sphere

                crate::services::bots_client::Agent {
                    id: status.agent_id.clone(),
                    name: status.profile.name.clone(),
                    agent_type: format!("{:?}", status.profile.agent_type).to_lowercase(),
                    status: status.status.clone(),
                    x: radius_at_y * theta.cos() * scale,
                    y: y_pos * scale,
                    z: radius_at_y * theta.sin() * scale,
                    cpu_usage: status.cpu_usage,
                    memory_usage: status.memory_usage,
                    health: status.health,
                    workload: status.activity,
                    created_at: Some(status.timestamp.to_rfc3339()),
                    age: Some(
                        (time::timestamp_seconds() - status.timestamp.timestamp()) as u64 * 1000,
                    ),
                    // Local monitor snapshot carries no agentbox DID (COM-14).
                    did_nostr: None,
                }
            })
            .collect();

        let message = UpdateBotsGraph { agents: agents_list };
        info!(
            "[AgentMonitorActor] Sending graph update with {} agents",
            agents.len()
        );
        self.graph_service_addr.do_send(message);

        if !agents.is_empty() {
            self.agent_cache.clear();
            for agent in agents {
                self.agent_cache.insert(agent.agent_id.clone(), agent);
            }
        }

        self.consecutive_poll_failures = 0;
        self.last_successful_poll = Some(time::now());
        // ADR-031 item 1: advance round-robin offset for next poll cycle.
        self.poll_offset = self.poll_offset.wrapping_add(1);
    }
}

impl Handler<RecordPollFailure> for AgentMonitorActor {
    type Result = ();

    fn handle(&mut self, _: RecordPollFailure, _ctx: &mut Self::Context) {
        self.consecutive_poll_failures += 1;
        warn!(
            "[AgentMonitorActor] Poll failure recorded - {} consecutive failures, backing off to {:?} before next poll",
            self.consecutive_poll_failures,
            self.next_poll_delay()
        );
    }
}

impl Handler<UpdateAgentCache> for AgentMonitorActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateAgentCache, _ctx: &mut Self::Context) {
        debug!(
            "[AgentMonitorActor] Updating agent cache with {} agents",
            msg.agents.len()
        );

        self.agent_cache.clear();
        for agent in msg.agents {
            self.agent_cache.insert(agent.agent_id.clone(), agent);
        }

        debug!(
            "[AgentMonitorActor] Agent cache updated: {} agents",
            self.agent_cache.len()
        );
    }
}

/// ADR-031 item 3: Observational status inference.
///
/// `TaskOrchestratorActor` sends this when a task transitions to/from Running.
/// Rather than waiting up to 3 s for the next scheduled poll, we trigger an
/// immediate Management API re-poll so status changes are reflected instantly.
impl Handler<TaskStatusChanged> for AgentMonitorActor {
    type Result = ();

    fn handle(&mut self, msg: TaskStatusChanged, ctx: &mut Self::Context) {
        debug!(
            "[AgentMonitorActor] TaskStatusChanged: agent_type={}, running={}. \
             Triggering immediate re-poll.",
            msg.agent_type, msg.running_task_count
        );
        self.poll_agent_statuses(ctx);
    }
}
