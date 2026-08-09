use crate::layout::engines::compute_layout;
use crate::layout::types::*;
use crate::ok_json;
use crate::AppState;
use actix_web::{web, HttpResponse, Result};

pub async fn get_layout_modes(_data: web::Data<AppState>) -> Result<HttpResponse> {
    ok_json!(serde_json::json!({
        "current": "forceDirected",
        "available": ["forceDirected", "hierarchical", "radial", "spectral", "temporal", "clustered"],
        "transitioning": false
    }))
}

pub async fn set_layout_mode(
    data: web::Data<AppState>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    let mode_str = body
        .get("mode")
        .and_then(|m| m.as_str())
        .unwrap_or("forceDirected");
    let transition_ms = body
        .get("transitionMs")
        .and_then(|t| t.as_u64())
        .unwrap_or(500);

    let mode: LayoutMode =
        match serde_json::from_value(serde_json::Value::String(mode_str.to_string())) {
            Ok(m) => m,
            Err(_) => LayoutMode::ForceDirected,
        };

    // ForceDirected is handled by the GPU physics engine; no CPU layout needed.
    if mode == LayoutMode::ForceDirected {
        return ok_json!(serde_json::json!({
            "success": true,
            "mode": mode_str,
            "transitionMs": transition_ms,
            "positions": []
        }));
    }

    // Fetch current graph data
    use crate::actors::messages::GetGraphData;
    let graph_data = match data.graph_service_addr.send(GetGraphData).await {
        Ok(Ok(gd)) => gd,
        Ok(Err(e)) => {
            log::error!("set_layout_mode: failed to get graph data: {}", e);
            return ok_json!(serde_json::json!({
                "success": false,
                "error": "Failed to retrieve graph data",
                "mode": mode_str
            }));
        }
        Err(e) => {
            log::error!("set_layout_mode: actor mailbox error: {}", e);
            return ok_json!(serde_json::json!({
                "success": false,
                "error": "Graph service unavailable",
                "mode": mode_str
            }));
        }
    };

    // Convert graph data to the flat slices expected by compute_layout
    let nodes: Vec<(u32, String)> = graph_data
        .nodes
        .iter()
        .map(|n| (n.id, n.label.clone()))
        .collect();

    let edges: Vec<(u32, u32, f32)> = graph_data
        .edges
        .iter()
        .map(|e| (e.source, e.target, e.weight))
        .collect();

    let config = LayoutModeConfig {
        mode: mode.clone(),
        ..LayoutModeConfig::default()
    };

    let raw_positions = compute_layout(&mode, &nodes, &edges, &config);

    // Build JSON position array [{id, x, y, z}, ...]
    let positions: Vec<serde_json::Value> = nodes
        .iter()
        .zip(raw_positions.iter())
        .map(|((id, _label), &(x, y, z))| serde_json::json!({ "id": id, "x": x, "y": y, "z": z }))
        .collect();

    ok_json!(serde_json::json!({
        "success": true,
        "mode": mode_str,
        "transitionMs": transition_ms,
        "positions": positions
    }))
}

pub async fn get_layout_status(_data: web::Data<AppState>) -> Result<HttpResponse> {
    ok_json!(LayoutStatus {
        current_mode: LayoutMode::ForceDirected,
        transitioning: false,
        transition_progress: 1.0,
        iterations: 0,
        converged: false,
        kinetic_energy: 0.0,
        available_modes: vec![
            LayoutMode::ForceDirected,
            LayoutMode::Hierarchical,
            LayoutMode::Radial,
            LayoutMode::Spectral,
            LayoutMode::Temporal,
            LayoutMode::Clustered,
        ],
    })
}

pub async fn set_zones(
    _data: web::Data<AppState>,
    body: web::Json<Vec<ConstraintZone>>,
) -> Result<HttpResponse> {
    // TODO: Forward zones to ForceComputeActor
    ok_json!(serde_json::json!({
        "success": true,
        "zones": body.into_inner().len()
    }))
}

pub async fn get_zones(_data: web::Data<AppState>) -> Result<HttpResponse> {
    ok_json!(serde_json::json!({
        "zones": []
    }))
}

pub async fn reset_layout(data: web::Data<AppState>) -> Result<HttpResponse> {
    use crate::actors::messages::ResetPositions;

    if let Some(addr) = data.get_gpu_compute_addr().await {
        match addr.send(ResetPositions).await {
            Ok(Ok(_)) => {
                ok_json!(serde_json::json!({
                    "success": true,
                    "message": "Layout reset triggered — positions randomized and reheat applied"
                }))
            }
            Ok(Err(e)) => {
                ok_json!(serde_json::json!({
                    "success": false,
                    "message": format!("Reset failed: {}", e)
                }))
            }
            Err(e) => {
                ok_json!(serde_json::json!({
                    "success": false,
                    "message": format!("ForceComputeActor mailbox error: {}", e)
                }))
            }
        }
    } else {
        ok_json!(serde_json::json!({
            "success": false,
            "message": "GPU compute actor not available — layout reset skipped"
        }))
    }
}

pub fn configure_layout_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/layout")
            .route("/modes", web::get().to(get_layout_modes))
            .route("/mode", web::post().to(set_layout_mode))
            .route("/status", web::get().to(get_layout_status))
            .route("/zones", web::post().to(set_zones))
            .route("/zones", web::get().to(get_zones))
            .route("/reset", web::post().to(reset_layout)),
    );
}
