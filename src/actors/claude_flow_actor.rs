use actix::prelude::*;
use std::time::Duration;
use log::{info, error, warn};
use crate::services::claude_flow::{ClaudeFlowClient, ClaudeFlowClientBuilder, AgentStatus, AgentProfile, AgentType};
use crate::actors::messages::UpdateBotsGraph;
use crate::actors::GraphServiceActor;
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;

pub struct ClaudeFlowActor {
    client: ClaudeFlowClient,
    graph_service_addr: Addr<GraphServiceActor>,
    is_connected: bool,
}

impl ClaudeFlowActor {
    fn create_mock_agents() -> Vec<AgentStatus> {
        vec![
            AgentStatus {
                agent_id: "coordinator-001".to_string(),
                status: "active".to_string(),
                session_id: Uuid::new_v4().to_string(),
                profile: AgentProfile {
                    name: "System Coordinator".to_string(),
                    agent_type: AgentType::Coordinator,
                    capabilities: vec![
                        "orchestration".to_string(),
                        "task-management".to_string(),
                        "resource-allocation".to_string(),
                    ],
                    system_prompt: None,
                    max_concurrent_tasks: 10,
                    priority: 10,
                    retry_policy: Default::default(),
                    environment: None,
                    working_directory: None,
                },
                timestamp: Utc::now(),
                active_tasks_count: 3,
                completed_tasks_count: 15,
                failed_tasks_count: 0,
                total_execution_time: 45000,
                average_task_duration: 3000.0,
                success_rate: 100.0,
                current_task: None,
                metadata: HashMap::new(),
            },
            AgentStatus {
                agent_id: "researcher-001".to_string(),
                status: "active".to_string(),
                session_id: Uuid::new_v4().to_string(),
                profile: AgentProfile {
                    name: "Research Agent".to_string(),
                    agent_type: AgentType::Researcher,
                    capabilities: vec![
                        "data-gathering".to_string(),
                        "analysis".to_string(),
                        "report-generation".to_string(),
                    ],
                    system_prompt: None,
                    max_concurrent_tasks: 5,
                    priority: 8,
                    retry_policy: Default::default(),
                    environment: None,
                    working_directory: None,
                },
                timestamp: Utc::now(),
                active_tasks_count: 2,
                completed_tasks_count: 20,
                failed_tasks_count: 1,
                total_execution_time: 60000,
                average_task_duration: 3000.0,
                success_rate: 95.2,
                current_task: None,
                metadata: HashMap::new(),
            },
            AgentStatus {
                agent_id: "coder-001".to_string(),
                status: "active".to_string(),
                session_id: Uuid::new_v4().to_string(),
                profile: AgentProfile {
                    name: "Code Developer".to_string(),
                    agent_type: AgentType::Coder,
                    capabilities: vec![
                        "implementation".to_string(),
                        "refactoring".to_string(),
                        "debugging".to_string(),
                    ],
                    system_prompt: None,
                    max_concurrent_tasks: 3,
                    priority: 9,
                    retry_policy: Default::default(),
                    environment: None,
                    working_directory: None,
                },
                timestamp: Utc::now(),
                active_tasks_count: 1,
                completed_tasks_count: 30,
                failed_tasks_count: 2,
                total_execution_time: 90000,
                average_task_duration: 3000.0,
                success_rate: 93.8,
                current_task: None,
                metadata: HashMap::new(),
            },
            AgentStatus {
                agent_id: "analyst-001".to_string(),
                status: "active".to_string(),
                session_id: Uuid::new_v4().to_string(),
                profile: AgentProfile {
                    name: "Data Analyst".to_string(),
                    agent_type: AgentType::Analyst,
                    capabilities: vec![
                        "data-analysis".to_string(),
                        "visualization".to_string(),
                        "insights".to_string(),
                    ],
                    system_prompt: None,
                    max_concurrent_tasks: 4,
                    priority: 7,
                    retry_policy: Default::default(),
                    environment: None,
                    working_directory: None,
                },
                timestamp: Utc::now(),
                active_tasks_count: 2,
                completed_tasks_count: 18,
                failed_tasks_count: 0,
                total_execution_time: 54000,
                average_task_duration: 3000.0,
                success_rate: 100.0,
                current_task: None,
                metadata: HashMap::new(),
            },
            AgentStatus {
                agent_id: "tester-001".to_string(),
                status: "idle".to_string(),
                session_id: Uuid::new_v4().to_string(),
                profile: AgentProfile {
                    name: "Test Engineer".to_string(),
                    agent_type: AgentType::Tester,
                    capabilities: vec![
                        "unit-testing".to_string(),
                        "integration-testing".to_string(),
                        "validation".to_string(),
                    ],
                    system_prompt: None,
                    max_concurrent_tasks: 6,
                    priority: 8,
                    retry_policy: Default::default(),
                    environment: None,
                    working_directory: None,
                },
                timestamp: Utc::now(),
                active_tasks_count: 0,
                completed_tasks_count: 25,
                failed_tasks_count: 3,
                total_execution_time: 75000,
                average_task_duration: 3000.0,
                success_rate: 89.3,
                current_task: None,
                metadata: HashMap::new(),
            },
        ]
    }

    pub async fn new(graph_service_addr: Addr<GraphServiceActor>) -> Self {
        // Configuration for the MCP connection should come from .env
        // Since Claude Flow is running in this container on port 8081,
        // we need to connect to it via HTTP/WebSocket
        let host = std::env::var("CLAUDE_FLOW_HOST").unwrap_or_else(|_| "powerdev".to_string());
        let port = std::env::var("CLAUDE_FLOW_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .unwrap_or(3000);

        info!("ClaudeFlowActor: Connecting to Claude Flow at {}:{}", host, port);

        // Use HTTP transport to connect to Claude Flow running in this container
        let mut client = ClaudeFlowClientBuilder::new()
            .host(&host)
            .port(port)
            .use_websocket()  // Use WebSocket transport for real-time communication
            .build()
            .await
            .expect("Failed to build ClaudeFlowClient");

        // Try to connect, but handle failures gracefully
        let is_connected = match client.connect().await {
            Ok(_) => {
                info!("ClaudeFlowActor: Successfully connected to Claude Flow MCP");
                match client.initialize().await {
                    Ok(_) => {
                        info!("ClaudeFlowActor: Successfully initialized Claude Flow session");
                        true
                    }
                    Err(e) => {
                        error!("ClaudeFlowActor: Failed to initialize Claude Flow session: {}. Running in degraded mode.", e);
                        // Continue without Claude Flow - the actor can still function
                        false
                    }
                }
            }
            Err(e) => {
                error!("ClaudeFlowActor: Failed to connect to Claude Flow: {}. Running in degraded mode.", e);
                warn!("ClaudeFlowActor: Claude Flow features will be unavailable. This might be due to:");
                warn!("  - Claude Flow MCP server not running on port {}", port);
                warn!("  - Network connectivity issues");
                warn!("  - Authentication/protocol mismatch");
                info!("ClaudeFlowActor: Using mock agents for visualization instead.");
                // Continue without Claude Flow - provide mock data
                false
            }
        };

        Self { client, graph_service_addr, is_connected }
    }

    fn poll_for_updates(&self, ctx: &mut Context<Self>) {
        // If not connected, provide mock data for visualization
        if !self.is_connected {
            let graph_addr = self.graph_service_addr.clone();

            // Create mock agents for visualization
            ctx.run_interval(Duration::from_secs(10), move |_act, _ctx| {
                let mock_agents = Self::create_mock_agents();
                info!("Providing {} mock agents for visualization.", mock_agents.len());
                graph_addr.do_send(UpdateBotsGraph { agents: mock_agents });
            });
            return;
        }

        // Poll for agent updates every 5 seconds
        ctx.run_interval(Duration::from_secs(5), |act, _ctx| {
            if !act.is_connected {
                return;
            }

            let client = act.client.clone();
            let graph_addr = act.graph_service_addr.clone();

            actix::spawn(async move {
                match client.list_agents(false).await {
                    Ok(agents) => {
                        if !agents.is_empty() {
                            info!("Polled {} active agents from Claude Flow.", agents.len());
                            // Send the agent data to the GraphServiceActor to be processed
                            graph_addr.do_send(UpdateBotsGraph { agents });
                        }
                    }
                    Err(e) => error!("Failed to poll agents from Claude Flow: {}", e),
                }
            });
        });
    }

    fn handle_reconnection(&self, ctx: &mut Context<Self>) {
        // Only check health if connected
        if !self.is_connected {
            return;
        }

        // Check connection health every 30 seconds
        ctx.run_interval(Duration::from_secs(30), |act, _ctx| {
            if !act.is_connected {
                return;
            }

            let client = act.client.clone();

            actix::spawn(async move {
                match client.get_system_health().await {
                    Ok(health) => {
                        if health.status != "healthy" {
                            warn!("ClaudeFlowActor: System health check failed: {:?}", health);
                        }
                    }
                    Err(e) => {
                        error!("ClaudeFlowActor: Health check failed: {}", e);
                        // In degraded mode, we just log the error
                    }
                }
            });
        });
    }
}

impl Actor for ClaudeFlowActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        info!("ClaudeFlowActor started. Scheduling polling and health checks.");
        self.poll_for_updates(ctx);
        self.handle_reconnection(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("ClaudeFlowActor stopped. Disconnecting from Claude Flow.");
        let mut client = self.client.clone();
        actix::spawn(async move {
            if let Err(e) = client.disconnect().await {
                error!("Failed to disconnect from Claude Flow: {}", e);
            }
        });
    }
}

// Message handlers for controlling the Claude Flow integration

#[derive(Message)]
#[rtype(result = "Result<Vec<AgentStatus>, String>")]
pub struct GetActiveAgents;

impl Handler<GetActiveAgents> for ClaudeFlowActor {
    type Result = ResponseFuture<Result<Vec<AgentStatus>, String>>;

    fn handle(&mut self, _msg: GetActiveAgents, _ctx: &mut Context<Self>) -> Self::Result {
        if !self.is_connected {
            // Return mock agents when not connected
            return Box::pin(async move {
                Ok(ClaudeFlowActor::create_mock_agents())
            });
        }

        let client = self.client.clone();
        Box::pin(async move {
            client.list_agents(false)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct SpawnClaudeAgent {
    pub agent_type: String,
    pub name: String,
    pub capabilities: Vec<String>,
}

impl Handler<SpawnClaudeAgent> for ClaudeFlowActor {
    type Result = ResponseFuture<Result<(), String>>;

    fn handle(&mut self, msg: SpawnClaudeAgent, _ctx: &mut Context<Self>) -> Self::Result {
        if !self.is_connected {
            return Box::pin(async move {
                // In mock mode, just log and return success
                info!("Mock mode: Would spawn agent '{}' of type '{}'", msg.name, msg.agent_type);
                Ok(())
            });
        }

        let client = self.client.clone();

        Box::pin(async move {
            use crate::services::claude_flow::{client::SpawnAgentParams, AgentType};

            let agent_type = match msg.agent_type.as_str() {
                "coordinator" => AgentType::Coordinator,
                "researcher" => AgentType::Researcher,
                "coder" => AgentType::Coder,
                "analyst" => AgentType::Analyst,
                "architect" => AgentType::Architect,
                "tester" => AgentType::Tester,
                _ => AgentType::Specialist,
            };

            let params = SpawnAgentParams {
                agent_type,
                name: msg.name,
                capabilities: Some(msg.capabilities),
                system_prompt: None,
                max_concurrent_tasks: Some(3),
                priority: Some(5),
                environment: None,
                working_directory: None,
            };

            client.spawn_agent(params)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct TerminateClaudeAgent {
    pub agent_id: String,
}

impl Handler<TerminateClaudeAgent> for ClaudeFlowActor {
    type Result = ResponseFuture<Result<(), String>>;

    fn handle(&mut self, msg: TerminateClaudeAgent, _ctx: &mut Context<Self>) -> Self::Result {
        let client = self.client.clone();

        Box::pin(async move {
            client.terminate_agent(&msg.agent_id)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

use crate::actors::messages::InitializeSwarm;

impl Handler<InitializeSwarm> for ClaudeFlowActor {
    type Result = ResponseFuture<Result<(), String>>;

    fn handle(&mut self, msg: InitializeSwarm, ctx: &mut Context<Self>) -> Self::Result {
        info!("ClaudeFlowActor: Initializing swarm with topology: {}, max_agents: {}", 
              msg.topology, msg.max_agents);

        let mut client = self.client.clone();
        let graph_addr = self.graph_service_addr.clone();
        let actor_addr = ctx.address();
        
        Box::pin(async move {
            // First ensure we're connected
            if let Err(e) = client.connect().await {
                error!("Failed to connect to Claude Flow: {}", e);
                return Err(format!("Failed to connect to Claude Flow: {}", e));
            }

            // Initialize the MCP session
            if let Err(e) = client.initialize().await {
                error!("Failed to initialize Claude Flow session: {}", e);
                return Err(format!("Failed to initialize Claude Flow session: {}", e));
            }

            // Initialize swarm with the requested configuration
            match client.init_swarm(&msg.topology, Some(msg.max_agents)).await {
                Ok(swarm_info) => {
                    info!("Swarm initialized successfully: {}", swarm_info);
                    
                    // If neural enhancement is enabled, train neural patterns
                    if msg.enable_neural {
                        info!("Training neural patterns for enhanced coordination");
                        // Train neural patterns for better coordination
                        if let Err(e) = client.train_neural_pattern("coordination", "swarm optimization", Some(50)).await {
                            warn!("Failed to train neural patterns: {}", e);
                        }
                    }
                    
                    // Now spawn the requested agent types
                    let mut spawn_errors = Vec::new();
                    
                    for agent_type in &msg.agent_types {
                        use crate::services::claude_flow::{client::SpawnAgentParams, AgentType};
                        
                        let agent_type_enum = match agent_type.as_str() {
                            "coordinator" => AgentType::Coordinator,
                            "researcher" => AgentType::Researcher,
                            "coder" => AgentType::Coder,
                            "analyst" => AgentType::Analyst,
                            "architect" => AgentType::Architect,
                            "tester" => AgentType::Tester,
                            "optimizer" => AgentType::Optimizer,
                            "reviewer" => AgentType::Reviewer,
                            "documenter" => AgentType::Documenter,
                            _ => AgentType::Specialist,
                        };
                        
                        let agent_name = format!("{} Agent", 
                            agent_type.chars().next().unwrap().to_uppercase().to_string() + &agent_type[1..]);
                        
                        let params = SpawnAgentParams {
                            agent_type: agent_type_enum,
                            name: agent_name,
                            capabilities: Some(vec![]),
                            system_prompt: msg.custom_prompt.clone(),
                            max_concurrent_tasks: Some(3),
                            priority: Some(5),
                            environment: None,
                            working_directory: None,
                        };
                        
                        if let Err(e) = client.spawn_agent(params).await {
                            spawn_errors.push(format!("Failed to spawn {} agent: {}", agent_type, e));
                        }
                    }
                    
                    if !spawn_errors.is_empty() {
                        warn!("Some agents failed to spawn: {:?}", spawn_errors);
                    }
                    
                    // Mark the actor as connected and restart polling
                    actor_addr.do_send(MarkConnected { connected: true });
                    
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to initialize swarm: {}", e);
                    Err(format!("Failed to initialize swarm: {}", e))
                }
            }
        })
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct MarkConnected {
    connected: bool,
}

impl Handler<MarkConnected> for ClaudeFlowActor {
    type Result = ();

    fn handle(&mut self, msg: MarkConnected, ctx: &mut Context<Self>) {
        self.is_connected = msg.connected;
        if msg.connected {
            info!("ClaudeFlowActor marked as connected, restarting polling");
            self.poll_for_updates(ctx);
        }
    }
}