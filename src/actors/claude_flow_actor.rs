use actix::prelude::*;
use std::time::Duration;
use log::{info, error, warn};
use crate::services::claude_flow::{ClaudeFlowClient, ClaudeFlowClientBuilder, AgentStatus};
use crate::actors::messages::UpdateBotsGraph;
use crate::actors::GraphServiceActor;

pub struct ClaudeFlowActor {
    client: ClaudeFlowClient,
    graph_service_addr: Addr<GraphServiceActor>,
}

impl ClaudeFlowActor {
    pub async fn new(graph_service_addr: Addr<GraphServiceActor>) -> Self {
        // Configuration for the MCP connection should come from .env
        let host = std::env::var("CLAUDE_FLOW_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = std::env::var("CLAUDE_FLOW_PORT")
            .unwrap_or_else(|_| "8081".to_string())
            .parse::<u16>()
            .unwrap_or(8081);

        info!("ClaudeFlowActor: Connecting to MCP at {}:{}", host, port);

        let mut client = ClaudeFlowClientBuilder::new()
            .host(host)
            .port(port)
            .use_websocket()
            .build()
            .await
            .expect("Failed to build ClaudeFlowClient");

        client.connect().await.expect("Failed to connect to Claude Flow");
        client.initialize().await.expect("Failed to initialize Claude Flow session");

        info!("ClaudeFlowActor: Successfully connected to Claude Flow MCP.");

        Self { client, graph_service_addr }
    }

    fn poll_for_updates(&self, ctx: &mut Context<Self>) {
        // Poll for agent updates every 5 seconds
        ctx.run_interval(Duration::from_secs(5), |act, _ctx| {
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
        // Check connection health every 30 seconds
        ctx.run_interval(Duration::from_secs(30), |act, ctx| {
            let client = act.client.clone();
            let graph_addr = act.graph_service_addr.clone();
            
            actix::spawn(async move {
                match client.get_system_health().await {
                    Ok(health) => {
                        if health.status != "healthy" {
                            warn!("ClaudeFlowActor: System health check failed: {:?}", health);
                        }
                    }
                    Err(e) => {
                        error!("ClaudeFlowActor: Health check failed, attempting reconnect: {}", e);
                        // TODO: Implement reconnection logic
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