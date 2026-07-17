use visionclaw_domain::models::metadata::Metadata;
use visionclaw_domain::models::node::Node;
use crate::services::file_service::FileService;
use crate::types::vec3::Vec3Data;
use crate::{ok_json, error_json, bad_request};
use crate::AppState;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
// GraphService direct import is no longer needed as we use actors
// use crate::services::graph_service::GraphService;
use crate::actors::messages::{AddNodesFromMetadata, GetSettings, GetSettlementState, SettlementSnapshot};
use crate::application::graph::queries::{
    GetAutoBalanceNotifications, GetGraphData, GetNodeMap, GetPhysicsState,
};
use crate::actors::graph_actor::PhysicsState;
use visionclaw_domain::models::graph::GraphData;
use crate::handlers::utils::execute_in_thread;
use hexser::{Hexserror, QueryHandler};

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettlementState {
    pub is_settled: bool,
    pub stable_frame_count: u32,
    pub kinetic_energy: f32,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeWithPosition {

    pub id: u32,
    pub metadata_id: String,
    pub label: String,

    pub position: Vec3Data,
    pub velocity: Vec3Data,


    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,


    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResponse {
    pub nodes: Vec<Node>,
    pub edges: Vec<visionclaw_domain::models::edge::Edge>,
    pub metadata: HashMap<String, Metadata>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResponseWithPositions {
    pub nodes: Vec<NodeWithPosition>,
    pub edges: Vec<visionclaw_domain::models::edge::Edge>,
    pub metadata: HashMap<String, Metadata>,
    pub settlement_state: SettlementState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedGraphResponse {
    pub nodes: Vec<Node>,
    pub edges: Vec<visionclaw_domain::models::edge::Edge>,
    pub metadata: HashMap<String, Metadata>,
    pub total_pages: usize,
    pub current_page: usize,
    pub total_items: usize,
    pub page_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQuery {
    pub query: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub sort: Option<String>,
    pub filter: Option<String>,
    pub graph_type: Option<String>,
    /// When `true`, drop `linked_page` wikilink-stub nodes (and edges touching
    /// them) from the response at source. Mirrors the client `nodeFilter
    /// .includeLinkedPages` gate so the dominant stub population (≈14.7k of
    /// 17.1k nodes) is never transferred when it will only be hidden anyway.
    /// Absent ⇒ no stub filtering (back-compat default).
    pub exclude_linked_pages: Option<bool>,
}

/// The three node populations, mirroring the wire flag bits in
/// `src/utils/binary_protocol.rs` (AGENT `0x80000000`, KNOWLEDGE `0x40000000`,
/// ONTOLOGY via `ONTOLOGY_TYPE_MASK 0x1C000000`). On the REST path node ids are
/// carried *unflagged* (flags are applied only at binary-encode time), so the
/// authoritative classifier here is `node_type` (the SSOT after the T1 fix),
/// with a metadata fallback. `graph_type` absent ⇒ no filtering (all nodes).
#[derive(Clone, Copy, PartialEq, Eq)]
enum PopulationFilter {
    Agent,
    Knowledge,
    Ontology,
}

impl PopulationFilter {
    fn parse(raw: Option<&str>) -> Option<Self> {
        match raw {
            Some("agent") => Some(Self::Agent),
            Some("knowledge") => Some(Self::Knowledge),
            Some("ontology") => Some(Self::Ontology),
            _ => None, // absent / unknown ⇒ no filter
        }
    }

    /// True if a node belongs to this population. Corresponds bit-for-bit to the
    /// `binary_protocol::is_agent_node` / `is_knowledge_node` / `is_ontology_node`
    /// predicates the wire encoder applies, expressed over `node_type`.
    fn matches(self, node_type: Option<&str>, metadata: &HashMap<String, String>) -> bool {
        let nt = node_type.unwrap_or("");
        match self {
            // AGENT_NODE_FLAG (0x80000000)
            Self::Agent => nt == "agent" || nt == "bot" || metadata.contains_key("agentType"),
            // KNOWLEDGE_NODE_FLAG (0x40000000)
            Self::Knowledge => nt == "page" || nt == "linked_page" || nt.is_empty(),
            // ONTOLOGY_TYPE_MASK (0x1C000000) — class/individual/property subtypes
            Self::Ontology => {
                nt.starts_with("owl_")
                    || nt == "ontology_node"
                    || metadata.contains_key("owl_class_iri")
            }
        }
    }
}

/// Fetch the honest, live physics settlement telemetry from the GPU force-compute
/// actor. Returns `None` when the actor is not yet available or no physics tick
/// has produced positions, so callers fall back to run-state-derived defaults.
async fn fetch_settlement(state: &web::Data<AppState>) -> Option<SettlementSnapshot> {
    let gpu_addr = state.get_gpu_compute_addr().await?;
    match gpu_addr.send(GetSettlementState).await {
        Ok(Ok(snapshot)) => Some(snapshot),
        Ok(Err(e)) => {
            debug!("GetSettlementState returned no telemetry yet: {}", e);
            None
        }
        Err(e) => {
            warn!("Mailbox error querying settlement state: {}", e);
            None
        }
    }
}

/// Build the client-facing `SettlementState` from live telemetry when present,
/// else from run-state truth (`is_settled = !running`, zeroed counters). Honest:
/// never claims settled/KE=0 while the GPU actor reports motion.
fn build_settlement_state(
    settlement: Option<&SettlementSnapshot>,
    physics_running: bool,
) -> SettlementState {
    match settlement {
        Some(s) => SettlementState {
            is_settled: s.is_settled,
            stable_frame_count: s.stable_frame_count,
            kinetic_energy: s.kinetic_energy as f32,
        },
        None => SettlementState {
            is_settled: !physics_running,
            stable_frame_count: 0,
            kinetic_energy: 0.0,
        },
    }
}

pub async fn get_graph_data(
    state: web::Data<AppState>,
    query: web::Query<GraphQuery>,
    _req: HttpRequest,
) -> impl Responder {
    info!("Received request for graph data (CQRS Phase 1D), graph_type={:?}", query.graph_type);

    
    let graph_handler = state.graph_query_handlers.get_graph_data.clone();
    let node_map_handler = state.graph_query_handlers.get_node_map.clone();
    let physics_handler = state.graph_query_handlers.get_physics_state.clone();

    
    let graph_future = execute_in_thread(move || graph_handler.handle(GetGraphData));
    let node_map_future = execute_in_thread(move || node_map_handler.handle(GetNodeMap));
    let physics_future = execute_in_thread(move || physics_handler.handle(GetPhysicsState));

    // Live physics settlement telemetry, fetched concurrently with the CQRS
    // queries. `None` ⇒ GPU actor not up / no tick yet ⇒ run-state fallback.
    let settlement_future = fetch_settlement(&state);

    let (graph_result, node_map_result, physics_result, settlement): (
        Result<Result<Arc<GraphData>, Hexserror>, String>,
        Result<Result<Arc<HashMap<u32, Node>>, Hexserror>, String>,
        Result<Result<PhysicsState, Hexserror>, String>,
        Option<SettlementSnapshot>,
    ) = tokio::join!(graph_future, node_map_future, physics_future, settlement_future);

    match (graph_result, node_map_result, physics_result) {
        (Ok(Ok(graph_data)), Ok(Ok(_node_map)), Ok(Ok(physics_state))) => {
            debug!(
                "Preparing enhanced graph response with {} nodes, {} edges, physics state: {:?}",
                graph_data.nodes.len(),
                graph_data.edges.len(),
                physics_state
            );


            let nodes_with_positions: Vec<NodeWithPosition> = graph_data
                .nodes
                .iter()
                .map(|node| {
                    // Use node's own data for position and velocity
                    // node_map contains HashMap<i32, Vec<i32>>, not physics nodes
                    let position: Vec3Data = node.data.position().into();
                    let velocity: Vec3Data = node.data.velocity().into();

                    NodeWithPosition {
                        id: node.id,
                        metadata_id: node.metadata_id.clone(),
                        label: node.label.clone(),
                        position,
                        velocity,
                        metadata: node.metadata.clone(),
                        node_type: node.node_type.clone(),
                        size: node.size,
                        color: node.color.clone(),
                        weight: node.weight,
                        group: node.group.clone(),
                    }
                })
                .collect();

            // Server-side population filtering (PRD-018 WS-4). `graph_type`
            // absent ⇒ all nodes (behaviour unchanged). Population membership is
            // defined by `PopulationFilter`, which mirrors the binary-protocol
            // flag bits so the REST and wire classifications agree.
            let population = PopulationFilter::parse(query.graph_type.as_deref());
            // linked_page stub gate (mirrors client nodeFilter.includeLinkedPages).
            // Authoritative origin is metadata["type"] (matches the client
            // `nodePopulationType` precedence), with node_type as the fallback.
            let exclude_linked_pages = query.exclude_linked_pages.unwrap_or(false);
            let filtered_nodes: Vec<NodeWithPosition> = nodes_with_positions
                .into_iter()
                .filter(|node| match population {
                    Some(p) => p.matches(node.node_type.as_deref(), &node.metadata),
                    None => true,
                })
                .filter(|node| {
                    if !exclude_linked_pages {
                        return true;
                    }
                    let origin = node
                        .metadata
                        .get("type")
                        .map(String::as_str)
                        .or(node.node_type.as_deref())
                        .unwrap_or("");
                    origin != "linked_page"
                })
                .collect();

            // Filter edges to only include those connecting filtered nodes
            let filtered_node_ids: std::collections::HashSet<u32> =
                filtered_nodes.iter().map(|n| n.id).collect();
            let filtered_edges: Vec<_> = graph_data
                .edges
                .iter()
                .filter(|e| {
                    filtered_node_ids.contains(&e.source)
                        && filtered_node_ids.contains(&e.target)
                })
                .cloned()
                .collect();

            let response = GraphResponseWithPositions {
                nodes: filtered_nodes,
                edges: filtered_edges,
                metadata: graph_data.metadata.clone(),
                // Honest settlement telemetry from the GPU force-compute actor;
                // falls back to run-state truth only when no telemetry exists.
                settlement_state: build_settlement_state(
                    settlement.as_ref(),
                    physics_state.is_running,
                ),
            };

            info!(
                "Sending graph data with {} nodes (CQRS query handlers)",
                response.nodes.len()
            );

            ok_json!(response)
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            error!("Thread execution error: {}", e);
            Ok::<HttpResponse, actix_web::Error>(HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Internal server error"})))
        }
        (Ok(Err(e)), _, _) | (_, Ok(Err(e)), _) | (_, _, Ok(Err(e))) => {
            error!("Failed to fetch graph data (CQRS): {}", e);
            Ok(HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Failed to retrieve graph data"})))
        }
    }
}

pub async fn get_paginated_graph_data(
    state: web::Data<AppState>,
    query: web::Query<GraphQuery>,
) -> impl Responder {
    info!(
        "Received request for paginated graph data (CQRS Phase 1D): {:?}",
        query
    );

    let page = query.page.map(|p| p.saturating_sub(1)).unwrap_or(0);
    let page_size = query.page_size.unwrap_or(100);

    if page_size == 0 {
        error!("Invalid page size: {}", page_size);
        return bad_request!("Page size must be greater than 0");
    }

    
    let graph_handler = state.graph_query_handlers.get_graph_data.clone();
    let graph_result = execute_in_thread(move || graph_handler.handle(GetGraphData)).await;

    let graph_data_owned = match graph_result {
        Ok(Ok(g_owned)) => g_owned,
        Ok(Err(e)) => {
            error!("Failed to get graph data for pagination (CQRS): {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Failed to retrieve graph data"})));
        }
        Err(e) => {
            error!("Thread execution error: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Internal server error"})));
        }
    };

    let total_items = graph_data_owned.nodes.len();

    if total_items == 0 {
        debug!("Graph is empty");
        return ok_json!(PaginatedGraphResponse {
            nodes: Vec::new(),
            edges: Vec::new(),
            metadata: HashMap::new(),
            total_pages: 0,
            current_page: 1,
            total_items: 0,
            page_size,
        });
    }

    let total_pages = (total_items + page_size - 1) / page_size;

    if page >= total_pages {
        warn!(
            "Requested page {} exceeds total pages {}",
            page + 1,
            total_pages
        );
        return bad_request!("Page {} exceeds total available pages {}", page + 1, total_pages);
    }

    let start = page * page_size;
    let end = std::cmp::min(start + page_size, total_items);

    debug!(
        "Calculating slice from {} to {} out of {} total items",
        start, end, total_items
    );

    let page_nodes = graph_data_owned.nodes[start..end].to_vec();

    let node_ids: std::collections::HashSet<_> = page_nodes.iter().map(|node| node.id).collect();

    let relevant_edges: Vec<_> = graph_data_owned
        .edges
        .iter()
        .filter(|edge| node_ids.contains(&edge.source) || node_ids.contains(&edge.target))
        .cloned()
        .collect();

    debug!(
        "Found {} relevant edges for {} nodes (CQRS)",
        relevant_edges.len(),
        page_nodes.len()
    );

    let response = PaginatedGraphResponse {
        nodes: page_nodes,
        edges: relevant_edges,
        metadata: graph_data_owned.metadata.clone(),
        total_pages,
        current_page: page + 1,
        total_items,
        page_size,
    };

    ok_json!(response)
}

pub async fn refresh_graph(state: web::Data<AppState>) -> impl Responder {
    info!("Received request to refresh graph (CQRS Phase 1D)");

    
    let graph_handler = state.graph_query_handlers.get_graph_data.clone();
    let graph_result = execute_in_thread(move || graph_handler.handle(GetGraphData)).await;

    match graph_result {
        Ok(Ok(graph_data_owned)) => {
            debug!(
                "Returning current graph state with {} nodes and {} edges (CQRS)",
                graph_data_owned.nodes.len(),
                graph_data_owned.edges.len()
            );

            let response = GraphResponse {
                nodes: graph_data_owned.nodes.clone(),
                edges: graph_data_owned.edges.clone(),
                metadata: graph_data_owned.metadata.clone(),
            };

            ok_json!(serde_json::json!({
                "success": true,
                "message": "Graph data retrieved successfully",
                "data": response
            }))
        }
        Ok(Err(e)) => {
            error!("Failed to get current graph data (CQRS): {}", e);
            error_json!("Failed to retrieve current graph data")
        }
        Err(e) => {
            error!("Thread execution error: {}", e);
            error_json!("Internal server error")
        }
    }
}

pub async fn update_graph(state: web::Data<AppState>) -> impl Responder {
    info!("Received request to update graph");

    let mut metadata = match FileService::load_or_create_metadata() {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to load metadata: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to load metadata: {}", e)
            })));
        }
    };

    let settings_result = state.settings_addr.send(GetSettings).await;
    let settings = match settings_result {
        Ok(Ok(s)) => Arc::new(tokio::sync::RwLock::new(s)),
        _ => {
            error!("Failed to retrieve settings for FileService in update_graph");
            return error_json!("Failed to retrieve application settings");
        }
    };

    let file_service = FileService::new(settings.clone());
    match file_service
        .fetch_and_process_files(state.content_api.clone(), settings.clone(), &mut metadata)
        .await
    {
        Ok(processed_files) => {
            if processed_files.is_empty() {
                debug!("No new files to process");
                return ok_json!(serde_json::json!({
                    "success": true,
                    "message": "No updates needed"
                }));
            }

            debug!("Processing {} new files", processed_files.len());

            {
                
                if let Err(e) = state
                    .metadata_addr
                    .send(crate::actors::messages::UpdateMetadata {
                        metadata: metadata.clone(),
                    })
                    .await
                {
                    error!("Failed to send UpdateMetadata to MetadataActor: {}", e);
                    
                }
            }

            
            match state
                .graph_service_addr
                .send(AddNodesFromMetadata { metadata })
                .await
            {
                Ok(Ok(())) => {
                    
                    debug!(
                        "Graph updated successfully via GraphServiceActor after file processing"
                    );
                    ok_json!(serde_json::json!({
                        "success": true,
                        "message": format!("Graph updated with {} new files", processed_files.len())
                    }))
                }
                Ok(Err(e)) => {
                    error!(
                        "GraphServiceActor failed to build graph from metadata: {}",
                        e
                    );
                    Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to build graph: {}", e)
                    })))
                }
                Err(e) => {
                    error!("Failed to build new graph: {}", e);
                    Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to build new graph: {}", e)
                    })))
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch and process files: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to fetch and process files: {}", e)
            })))
        }
    }
}

// Auto-balance notifications endpoint
pub async fn get_auto_balance_notifications(
    state: web::Data<AppState>,
    query: web::Query<serde_json::Value>,
) -> impl Responder {
    let since_timestamp = query.get("since").and_then(|v| v.as_i64());

    info!("Fetching auto-balance notifications (CQRS Phase 1D)");

    
    let handler = state
        .graph_query_handlers
        .get_auto_balance_notifications
        .clone();
    let query_obj = GetAutoBalanceNotifications { since_timestamp };

    let result = execute_in_thread(move || handler.handle(query_obj)).await;

    match result {
        Ok(Ok(notifications)) => ok_json!(serde_json::json!({
            "success": true,
            "notifications": notifications
        })),
        Ok(Err(e)) => {
            error!("Failed to get auto-balance notifications (CQRS): {}", e);
            error_json!("Failed to retrieve notifications")
        }
        Err(e) => {
            error!("Thread execution error: {}", e);
            error_json!("Internal server error")
        }
    }
}

/// Return the current GPU-computed node positions (not the initial loaded zeros).
///
/// `GET /api/graph/positions`
pub async fn get_graph_positions(
    state: web::Data<AppState>,
) -> impl Responder {
    // Acquire ForceComputeActor address
    let gpu_addr = match state.get_gpu_compute_addr().await {
        Some(addr) => addr,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "success": false,
                "error": "GPU compute actor not available"
            }));
        }
    };

    use crate::actors::messages::GetCurrentPositions;

    match gpu_addr.send(GetCurrentPositions).await {
        Ok(Ok(snapshot)) => {
            let positions: Vec<serde_json::Value> = snapshot
                .positions
                .iter()
                .map(|(id, x, y, z)| {
                    serde_json::json!({
                        "id": id,
                        "x": x,
                        "y": y,
                        "z": z
                    })
                })
                .collect();

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": {
                    "positions": positions,
                    "metadata": {
                        "numNodes": snapshot.num_nodes,
                        "settled": snapshot.settled,
                        "stableFrameCount": snapshot.stable_frame_count,
                        "kineticEnergy": snapshot.kinetic_energy,
                        "boundingBox": {
                            "min": {
                                "x": snapshot.bounding_box.min_x,
                                "y": snapshot.bounding_box.min_y,
                                "z": snapshot.bounding_box.min_z
                            },
                            "max": {
                                "x": snapshot.bounding_box.max_x,
                                "y": snapshot.bounding_box.max_y,
                                "z": snapshot.bounding_box.max_z
                            }
                        }
                    }
                }
            }))
        }
        Ok(Err(e)) => {
            warn!("GetCurrentPositions returned error: {}", e);
            HttpResponse::Ok().json(serde_json::json!({
                "success": false,
                "error": e
            }))
        }
        Err(e) => {
            error!("Mailbox error sending GetCurrentPositions: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Actor mailbox error: {}", e)
            }))
        }
    }
}

// Configure routes using snake_case
/// SECURITY: Graph mutation operations require authentication
pub fn config(cfg: &mut web::ServiceConfig) {
    use crate::middleware::{RateLimit, RequireAuth};

    // actix-web claims a route prefix for the FIRST registered `web::scope("/graph")`
    // and routes defined in later same-prefix scopes return 404. Mixed-auth on one
    // prefix must therefore live in a SINGLE scope with per-resource `.wrap()`.
    cfg.service(
        web::scope("/graph")
            .wrap(RateLimit::per_minute(600))  // Rate limit: 600 requests/min (10/sec) for public reads
            // Read operations - public with rate limiting
            .route("/data", web::get().to(get_graph_data))
            .route("/data/paginated", web::get().to(get_paginated_graph_data))
            .route("/positions", web::get().to(get_graph_positions))
            .route(
                "/auto-balance-notifications",
                web::get().to(get_auto_balance_notifications),
            )
            // S2: `/update` triggers a full bulk reload — it re-fetches and
            // re-processes the entire upstream content source and rebuilds the graph
            // from metadata (AddNodesFromMetadata). That is a destructive/expensive
            // privileged operation, not a routine per-node edit, so it is escalated
            // to power_user (Admin). A regular NIP-98 user (Authenticated) can no
            // longer trigger a global rebuild.
            .service(
                web::resource("/update")
                    .wrap(RequireAuth::power_user())  // Bulk reload requires power-user
                    .route(web::post().to(update_graph)),
            )
            // `/refresh` only reads the current graph state (GetGraphData) and returns
            // it; it mutates nothing, so any authenticated user may call it.
            .service(
                web::resource("/refresh")
                    .wrap(RequireAuth::authenticated())  // Read-back, any authed user
                    .route(web::post().to(refresh_graph)),
            ),
    );
}

#[cfg(test)]
mod population_filter_tests {
    use super::PopulationFilter;
    use std::collections::HashMap;

    fn md(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn absent_or_unknown_graph_type_yields_no_filter() {
        assert!(PopulationFilter::parse(None).is_none());
        assert!(PopulationFilter::parse(Some("bogus")).is_none());
    }

    #[test]
    fn knowledge_matches_pages_and_untyped() {
        let p = PopulationFilter::parse(Some("knowledge")).unwrap();
        assert!(p.matches(Some("page"), &md(&[])));
        assert!(p.matches(Some("linked_page"), &md(&[])));
        assert!(p.matches(None, &md(&[])));
        assert!(!p.matches(Some("owl_class"), &md(&[])));
        assert!(!p.matches(Some("agent"), &md(&[])));
    }

    #[test]
    fn ontology_matches_owl_and_class_iri_metadata() {
        let p = PopulationFilter::parse(Some("ontology")).unwrap();
        assert!(p.matches(Some("owl_class"), &md(&[])));
        assert!(p.matches(Some("ontology_node"), &md(&[])));
        assert!(p.matches(Some("page"), &md(&[("owl_class_iri", "urn:x")])));
        assert!(!p.matches(Some("page"), &md(&[])));
    }

    #[test]
    fn agent_matches_agent_bot_and_agenttype_metadata() {
        let p = PopulationFilter::parse(Some("agent")).unwrap();
        assert!(p.matches(Some("agent"), &md(&[])));
        assert!(p.matches(Some("bot"), &md(&[])));
        assert!(p.matches(Some("page"), &md(&[("agentType", "coder")])));
        assert!(!p.matches(Some("page"), &md(&[])));
    }
}
