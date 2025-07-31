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
    client: Option<ClaudeFlowClient>,
    graph_service_addr: Addr<GraphServiceActor>,
    is_connected: bool,
    connection_attempts: u32,
    max_retry_attempts: u32,
}

impl ClaudeFlowActor {
    // ... [keep existing create_mock_agents() method] ...

    pub async fn new(graph_service_addr: Addr<GraphServiceActor>) -> Self {
        let host = std::env::var("CLAUDE_FLOW_HOST").unwrap_or_else(|_| "powerdev".to_string());
        let port = std::env::var("CLAUDE_FLOW_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .unwrap_or(3000);

        info!("ClaudeFlowActor: Attempting to connect to Claude Flow at {}:{}", host, port);

        // Build client with proper error handling
        let client = match ClaudeFlowClientBuilder::new()
            .host(&host)
            .port(port)
            .use_websocket()
            .build()
            .await
        {
            Ok(mut client) => {
                // Try to connect
                match Self::attempt_connection(&mut client).await {
                    Ok(_) => {
                        info!("ClaudeFlowActor: Successfully connected and initialized");
                        Some(client)
                    }
                    Err(e) => {
                        warn!("ClaudeFlowActor: Connection failed: {}. Will retry later.", e);
                        Some(client) // Keep client for retry attempts
                    }
                }
            }
            Err(e) => {
                error!("ClaudeFlowActor: Failed to build client: {}. Running in mock mode.", e);
                None
            }
        };

        Self {
            client,
            graph_service_addr,
            is_connected: client.is_some(),
            connection_attempts: 0,
            max_retry_attempts: 10,
        }
    }

    async fn attempt_connection(client: &mut ClaudeFlowClient) -> Result<(), String> {
        client.connect().await
            .map_err(|e| format!("Connection failed: {}", e))?;
        
        client.initialize().await
            .map_err(|e| format!("Initialization failed: {}", e))?;
        
        Ok(())
    }

    fn schedule_reconnection(&self, ctx: &mut Context<Self>) {
        if self.client.is_none() || self.is_connected {
            return;
        }

        // Exponential backoff: 5s, 10s, 20s, 40s, etc., max 5 minutes
        let delay = std::cmp::min(
            Duration::from_secs(5 * 2_u64.pow(self.connection_attempts)),
            Duration::from_secs(300)
        );

        info!("ClaudeFlowActor: Scheduling reconnection attempt {} in {:?}", 
              self.connection_attempts + 1, delay);

        ctx.run_later(delay, |act, ctx| {
            if act.connection_attempts >= act.max_retry_attempts {
                warn!("ClaudeFlowActor: Max reconnection attempts reached. Staying in mock mode.");
                return;
            }

            act.connection_attempts += 1;

            if let Some(ref mut client) = act.client {
                let mut client_clone = client.clone();
                let addr = ctx.address();

                actix::spawn(async move {
                    match ClaudeFlowActor::attempt_connection(&mut client_clone).await {
                        Ok(_) => {
                            info!("ClaudeFlowActor: Reconnection successful!");
                            addr.do_send(ConnectionEstablished { 
                                client: client_clone,
                                success: true 
                            });
                        }
                        Err(e) => {
                            warn!("ClaudeFlowActor: Reconnection attempt failed: {}", e);
                            addr.do_send(ConnectionEstablished { 
                                client: client_clone,
                                success: false 
                            });
                        }
                    }
                });
            }
        });
    }

    fn poll_for_updates(&self, ctx: &mut Context<Self>) {
        // Provide mock data if not connected
        if !self.is_connected || self.client.is_none() {
            let graph_addr = self.graph_service_addr.clone();
            
            ctx.run_interval(Duration::from_secs(10), move |_act, _ctx| {
                let mock_agents = Self::create_mock_agents();
                info!("Providing {} mock agents for visualization.", mock_agents.len());
                graph_addr.do_send(UpdateBotsGraph { agents: mock_agents });
            });
            return;
        }

        // Poll for real agent updates
        ctx.run_interval(Duration::from_secs(5), |act, ctx| {
            if !act.is_connected || act.client.is_none() {
                return;
            }

            if let Some(ref client) = act.client {
                let client_clone = client.clone();
                let graph_addr = act.graph_service_addr.clone();
                let addr = ctx.address();

                actix::spawn(async move {
                    match client_clone.list_agents(false).await {
                        Ok(agents) => {
                            if !agents.is_empty() {
                                info!("Polled {} active agents from Claude Flow.", agents.len());
                                graph_addr.do_send(UpdateBotsGraph { agents });
                            }
                        }
                        Err(e) => {
                            error!("Failed to poll agents: {}", e);
                            // Check if this is a connection error
                            if e.to_string().contains("connection") || 
                               e.to_string().contains("refused") {
                                addr.do_send(ConnectionLost);
                            }
                        }
                    }
                });
            }
        });
    }

    fn handle_health_checks(&self, ctx: &mut Context<Self>) {
        ctx.run_interval(Duration::from_secs(30), |act, ctx| {
            if !act.is_connected || act.client.is_none() {
                return;
            }

            if let Some(ref client) = act.client {
                let client_clone = client.clone();
                let addr = ctx.address();

                actix::spawn(async move {
                    match client_clone.get_system_health().await {
                        Ok(health) => {
                            if health.status != "healthy" {
                                warn!("System health degraded: {:?}", health);
                            }
                        }
                        Err(e) => {
                            error!("Health check failed: {}", e);
                            addr.do_send(ConnectionLost);
                        }
                    }
                });
            }
        });
    }
}

// Message handlers
#[derive(Message)]
#[rtype(result = "()")]
struct ConnectionEstablished {
    client: ClaudeFlowClient,
    success: bool,
}

impl Handler<ConnectionEstablished> for ClaudeFlowActor {
    type Result = ();

    fn handle(&mut self, msg: ConnectionEstablished, ctx: &mut Context<Self>) {
        if msg.success {
            self.client = Some(msg.client);
            self.is_connected = true;
            self.connection_attempts = 0;
            
            // Restart polling with real data
            self.poll_for_updates(ctx);
            self.handle_health_checks(ctx);
        } else {
            self.client = Some(msg.client);
            self.schedule_reconnection(ctx);
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct ConnectionLost;

impl Handler<ConnectionLost> for ClaudeFlowActor {
    type Result = ();

    fn handle(&mut self, _msg: ConnectionLost, ctx: &mut Context<Self>) {
        warn!("ClaudeFlowActor: Connection lost, switching to mock mode");
        self.is_connected = false;
        self.connection_attempts = 0;
        self.schedule_reconnection(ctx);
        
        // Restart with mock data
        self.poll_for_updates(ctx);
    }
}

impl Actor for ClaudeFlowActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        info!("ClaudeFlowActor started. Setting up polling and health checks.");
        self.poll_for_updates(ctx);
        
        if self.is_connected && self.client.is_some() {
            self.handle_health_checks(ctx);
        } else {
            self.schedule_reconnection(ctx);
        }
    }

    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("ClaudeFlowActor stopped. Disconnecting from Claude Flow.");
        if let Some(mut client) = self.client.take() {
            actix::spawn(async move {
                if let Err(e) = client.disconnect().await {
                    error!("Failed to disconnect: {}", e);
                }
            });
        }
    }
}

// Handler for InitializeSwarm with proper error handling
impl Handler<InitializeSwarm> for ClaudeFlowActor {
    type Result = ResponseFuture<Result<(), String>>;

    fn handle(&mut self, msg: InitializeSwarm, ctx: &mut Context<Self>) -> Self::Result {
        info!("ClaudeFlowActor: Initializing swarm with topology: {}, max_agents: {}", 
              msg.topology, msg.max_agents);

        // If no client, try to create one
        if self.client.is_none() {
            let host = std::env::var("CLAUDE_FLOW_HOST").unwrap_or_else(|_| "powerdev".to_string());
            let port = std::env::var("CLAUDE_FLOW_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse::<u16>()
                .unwrap_or(3000);

            return Box::pin(async move {
                match ClaudeFlowClientBuilder::new()
                    .host(&host)
                    .port(port)
                    .use_websocket()
                    .build()
                    .await
                {
                    Ok(mut client) => {
                        // Try to connect and initialize swarm
                        if let Err(e) = Self::attempt_connection(&mut client).await {
                            return Err(format!("Failed to connect: {}", e));
                        }

                        // Continue with swarm initialization...
                        match client.init_swarm(&msg.topology, Some(msg.max_agents)).await {
                            Ok(swarm_info) => {
                                info!("Swarm initialized: {}", swarm_info);
                                Ok(())
                            }
                            Err(e) => Err(format!("Failed to initialize swarm: {}", e))
                        }
                    }
                    Err(e) => Err(format!("Failed to create client: {}", e))
                }
            });
        }

        // Use existing client
        let client = self.client.as_ref().unwrap().clone();
        let actor_addr = ctx.address();
        
        Box::pin(async move {
            // Implementation continues as before but with proper error handling...
            match client.init_swarm(&msg.topology, Some(msg.max_agents)).await {
                Ok(swarm_info) => {
                    info!("Swarm initialized: {}", swarm_info);
                    actor_addr.do_send(MarkConnected { connected: true });
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to initialize swarm: {}", e);
                    actor_addr.do_send(ConnectionLost);
                    Err(format!("Failed to initialize swarm: {}", e))
                }
            }
        })
    }
}