//! Memory Flash Handler — broadcasts RuVector memory access events to all WebSocket clients.
//!
//! POST /api/memory-flash accepts { key, namespace, action } and broadcasts a
//! `memory_flash` WebSocket message to every connected client so the embedding
//! cloud can animate the corresponding point(s).

use actix_web::{web, HttpResponse};
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::actors::messages::BroadcastMessage;
use crate::actors::ClientCoordinatorActor;

/// Payload accepted by POST /api/memory-flash
#[derive(Debug, Deserialize)]
pub struct MemoryFlashRequest {
    /// Memory entry key (e.g. "pattern-auth")
    pub key: String,
    /// Namespace (e.g. "patterns", "personal-context")
    pub namespace: Option<String>,
    /// Action: "store", "search", "retrieve", "delete", "update"
    pub action: Option<String>,
}

/// WebSocket message broadcast to all clients
#[derive(Debug, Serialize)]
struct MemoryFlashBroadcast {
    #[serde(rename = "type")]
    type_: &'static str,
    data: MemoryFlashData,
}

#[derive(Debug, Serialize)]
struct MemoryFlashData {
    key: String,
    namespace: String,
    action: String,
    timestamp: u64,
}

pub async fn handle_memory_flash(
    body: web::Json<MemoryFlashRequest>,
    client_coordinator: web::Data<actix::Addr<ClientCoordinatorActor>>,
) -> HttpResponse {
    let namespace = body.namespace.clone().unwrap_or_default();
    let action = body.action.clone().unwrap_or_else(|| "access".to_string());

    let broadcast = MemoryFlashBroadcast {
        type_: "memory_flash",
        data: MemoryFlashData {
            key: body.key.clone(),
            namespace: namespace.clone(),
            action: action.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        },
    };

    match serde_json::to_string(&broadcast) {
        Ok(json) => {
            let result = client_coordinator
                .send(BroadcastMessage { message: json })
                .await;
            match result {
                Ok(Ok(())) => {
                    debug!(
                        "[MemoryFlash] Broadcast OK: key={} ns={} action={}",
                        body.key, namespace, action
                    );
                    HttpResponse::Ok().json(serde_json::json!({ "ok": true }))
                }
                Ok(Err(e)) => {
                    warn!("[MemoryFlash] Broadcast error: {}", e);
                    HttpResponse::Ok().json(serde_json::json!({ "ok": true, "warn": e }))
                }
                Err(e) => {
                    warn!("[MemoryFlash] Actor mailbox error: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "ok": false,
                        "error": format!("actor error: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            warn!("[MemoryFlash] Serialization error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "ok": false,
                "error": "serialization failed"
            }))
        }
    }
}

/// Batch variant: multiple flashes in one request
#[derive(Debug, Deserialize)]
pub struct MemoryFlashBatchRequest {
    pub events: Vec<MemoryFlashRequest>,
}

pub async fn handle_memory_flash_batch(
    body: web::Json<MemoryFlashBatchRequest>,
    client_coordinator: web::Data<actix::Addr<ClientCoordinatorActor>>,
) -> HttpResponse {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut count = 0;
    for event in &body.events {
        let broadcast = MemoryFlashBroadcast {
            type_: "memory_flash",
            data: MemoryFlashData {
                key: event.key.clone(),
                namespace: event.namespace.clone().unwrap_or_default(),
                action: event.action.clone().unwrap_or_else(|| "access".to_string()),
                timestamp: ts,
            },
        };
        if let Ok(json) = serde_json::to_string(&broadcast) {
            client_coordinator.do_send(BroadcastMessage { message: json });
            count += 1;
        }
    }

    debug!("[MemoryFlash] Batch broadcast: {} events", count);
    HttpResponse::Ok().json(serde_json::json!({ "ok": true, "count": count }))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/memory-flash", web::post().to(handle_memory_flash))
        .route(
            "/memory-flash/batch",
            web::post().to(handle_memory_flash_batch),
        );
}
