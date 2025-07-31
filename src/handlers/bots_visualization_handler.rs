use actix_web::{web, HttpResponse, Responder};
use actix_web_actors::ws;
use actix::{Actor, StreamHandler, AsyncContext, Handler, ActorContext, Message};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};
use log::{info, debug, warn};

use crate::AppState;
use crate::services::agent_visualization_protocol::{
    AgentVisualizationProtocol,
    AgentStateUpdate, PositionUpdate
};

/// WebSocket actor for agent visualization streaming
pub struct AgentVisualizationWs {
    app_state: web::Data<AppState>,
    protocol: AgentVisualizationProtocol,
    last_heartbeat: Instant,
    last_position_update: Instant,
}

impl AgentVisualizationWs {
    pub fn new(app_state: web::Data<AppState>) -> Self {
        Self {
            app_state,
            protocol: AgentVisualizationProtocol::new(),
            last_heartbeat: Instant::now(),
            last_position_update: Instant::now(),
        }
    }
    
    /// Send initial state to client
    fn send_init_state(&self, ctx: &mut ws::WebsocketContext<Self>) {
        // For now, send empty agent list
        let init_json = AgentVisualizationProtocol::create_init_message(
            "swarm-001",
            "hierarchical",
            vec![]
        );
        
        ctx.text(init_json);
        info!("Sent initialization message to client");
    }
    
    /// Start position update stream
    fn start_position_updates(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Duration::from_millis(16), |act, ctx| { // ~60fps
            // Only send if we have updates
            if let Some(update_json) = act.protocol.create_position_update() {
                ctx.text(update_json);
            }
        });
    }
    
    /// Start heartbeat
    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Duration::from_secs(5), |act, ctx| {
            if Instant::now().duration_since(act.last_heartbeat) > Duration::from_secs(10) {
                warn!("WebSocket client heartbeat timeout, disconnecting");
                ctx.stop();
                return;
            }
            
            ctx.ping(b"ping");
        });
    }
}

impl Actor for AgentVisualizationWs {
    type Context = ws::WebsocketContext<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Agent visualization WebSocket connection established");
        
        // Send initial state
        ctx.address().do_send(InitConnection);
        
        // Start heartbeat
        self.start_heartbeat(ctx);
        
        // Start position updates
        self.start_position_updates(ctx);
    }
    
    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Agent visualization WebSocket connection closed");
    }
}

/// Message types
struct InitConnection;

impl Message for InitConnection {
    type Result = ();
}

struct UpdatePositions(Vec<PositionUpdate>);

impl Message for UpdatePositions {
    type Result = ();
}

struct UpdateStates(Vec<AgentStateUpdate>);

impl Message for UpdateStates {
    type Result = ();
}

impl Handler<InitConnection> for AgentVisualizationWs {
    type Result = ();
    
    fn handle(&mut self, _: InitConnection, ctx: &mut Self::Context) {
        self.send_init_state(ctx);
    }
}

impl Handler<UpdatePositions> for AgentVisualizationWs {
    type Result = ();
    
    fn handle(&mut self, msg: UpdatePositions, _ctx: &mut Self::Context) {
        // Buffer position updates
        for update in msg.0 {
            self.protocol.add_position_update(
                update.id,
                update.x,
                update.y,
                update.z,
                update.vx.unwrap_or(0.0),
                update.vy.unwrap_or(0.0),
                update.vz.unwrap_or(0.0)
            );
        }
    }
}

impl Handler<UpdateStates> for AgentVisualizationWs {
    type Result = ();
    
    fn handle(&mut self, msg: UpdateStates, ctx: &mut Self::Context) {
        let state_json = AgentVisualizationProtocol::create_state_update(msg.0);
        ctx.text(state_json);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for AgentVisualizationWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // Handle client requests
                if let Ok(request) = serde_json::from_str::<ClientRequest>(&text) {
                    match request.action.as_str() {
                        "refresh" => {
                            self.send_init_state(ctx);
                        }
                        "pause_updates" => {
                            // TODO: Implement pause logic
                            debug!("Pausing position updates");
                        }
                        "resume_updates" => {
                            // TODO: Implement resume logic
                            debug!("Resuming position updates");
                        }
                        _ => {
                            warn!("Unknown client action: {}", request.action);
                        }
                    }
                }
            }
            Ok(ws::Message::Binary(_)) => {
                warn!("Binary messages not supported");
            }
            Ok(ws::Message::Close(reason)) => {
                info!("WebSocket closing: {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

#[derive(Deserialize)]
struct ClientRequest {
    action: String,
    #[allow(dead_code)]
    params: Option<serde_json::Value>,
}

/// HTTP handlers

/// WebSocket endpoint for agent visualization
pub async fn agent_visualization_ws(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::start(AgentVisualizationWs::new(app_state), &req, stream)
}

/// Get current agent visualization snapshot (for debugging)
pub async fn get_agent_visualization_snapshot(
    _app_state: web::Data<AppState>,
) -> impl Responder {
    // For now, return empty agent list
    let init_json = AgentVisualizationProtocol::create_init_message(
        "swarm-001",
        "hierarchical",
        vec![]
    );
    
    HttpResponse::Ok()
        .content_type("application/json")
        .body(init_json)
}

/// Initialize swarm with specific configuration
#[derive(Deserialize)]
pub struct InitializeSwarmRequest {
    pub topology: String,
    pub max_agents: u32,
    pub agent_types: Vec<String>,
    pub custom_prompt: Option<String>,
}

pub async fn initialize_swarm_visualization(
    req: web::Json<InitializeSwarmRequest>,
    _app_state: web::Data<AppState>,
) -> impl Responder {
    info!("Initializing swarm visualization with topology: {}", req.topology);
    
    // TODO: Forward to Claude Flow actor to actually initialize swarm
    
    HttpResponse::Ok().json(json!({
        "success": true,
        "message": "Swarm initialization started",
        "swarm_id": "swarm-001",
        "topology": req.topology,
        "max_agents": req.max_agents
    }))
}

/// Configure visualization routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/visualization")
            .route("/agents/ws", web::get().to(agent_visualization_ws))
            .route("/agents/snapshot", web::get().to(get_agent_visualization_snapshot))
            .route("/swarm/initialize", web::post().to(initialize_swarm_visualization))
    );
}