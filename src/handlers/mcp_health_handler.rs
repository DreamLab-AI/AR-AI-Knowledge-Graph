use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use crate::services::mcp_relay_manager::McpRelayManager;

#[derive(Serialize)]
struct McpHealthResponse {
    powerdev_running: bool,
    mcp_relay_running: bool,
    last_logs: Option<String>,
    message: String,
}

/// Health check endpoint for MCP relay
pub async fn check_mcp_health() -> impl Responder {
    let powerdev_running = McpRelayManager::check_powerdev_container();
    let mcp_relay_running = if powerdev_running {
        McpRelayManager::check_relay_status()
    } else {
        false
    };
    
    let last_logs = if powerdev_running {
        McpRelayManager::get_relay_logs(20).ok()
    } else {
        None
    };
    
    let message = match (powerdev_running, mcp_relay_running) {
        (false, _) => "Powerdev container is not running".to_string(),
        (true, false) => "Powerdev is running but MCP relay is not active".to_string(),
        (true, true) => "MCP relay is healthy and running".to_string(),
    };
    
    HttpResponse::Ok().json(McpHealthResponse {
        powerdev_running,
        mcp_relay_running,
        last_logs,
        message,
    })
}

/// Start the MCP relay if not running
pub async fn start_mcp_relay() -> impl Responder {
    match McpRelayManager::ensure_relay_running() {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "message": "MCP relay started successfully"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e
        }))
    }
}

/// Get MCP relay logs
#[derive(Deserialize)]
pub struct LogQuery {
    lines: Option<usize>,
}

pub async fn get_mcp_logs(query: web::Query<LogQuery>) -> impl Responder {
    let lines = query.lines.unwrap_or(50);
    
    match McpRelayManager::get_relay_logs(lines) {
        Ok(logs) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "logs": logs
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e
        }))
    }
}

/// Configure MCP health routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/mcp")
            .route("/health", web::get().to(check_mcp_health))
            .route("/start", web::post().to(start_mcp_relay))
            .route("/logs", web::get().to(get_mcp_logs))
    );
}