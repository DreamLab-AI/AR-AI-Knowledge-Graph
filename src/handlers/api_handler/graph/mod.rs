use crate::services::file_service::FileService;
use crate::types::vec3::Vec3Data;
use crate::AppState;
use crate::{bad_request, error_json, ok_json};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use visionclaw_domain::models::metadata::Metadata;
use visionclaw_domain::models::node::Node;
// GraphService direct import is no longer needed as we use actors
// use crate::services::graph_service::GraphService;
use crate::actors::graph_actor::PhysicsState;
use crate::actors::messages::{
    AddNodesFromMetadata, GetSettings, GetSettlementState, SettlementSnapshot,
};
use crate::application::graph::queries::{
    GetAutoBalanceNotifications, GetGraphData, GetNodeMap, GetPhysicsState,
};
use crate::handlers::utils::execute_in_thread;
use hexser::{Hexserror, QueryHandler};
use visionclaw_domain::models::graph::GraphData;

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
    info!(
        "Received request for graph data (CQRS Phase 1D), graph_type={:?}",
        query.graph_type
    );

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
    ) = tokio::join!(
        graph_future,
        node_map_future,
        physics_future,
        settlement_future
    );

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
                    filtered_node_ids.contains(&e.source) && filtered_node_ids.contains(&e.target)
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
            Ok::<HttpResponse, actix_web::Error>(
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({"error": "Internal server error"})),
            )
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
        return bad_request!(
            "Page {} exceeds total available pages {}",
            page + 1,
            total_pages
        );
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
pub async fn get_graph_positions(state: web::Data<AppState>) -> impl Responder {
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

// ---------------------------------------------------------------------------
// Graph2VR "predicate-count-first expansion" (relations + expand)
// ---------------------------------------------------------------------------
//
// Two read-only endpoints that let a client browse the graph one predicate at a
// time (the Graph2VR interaction model): first ask "what relations does this
// node have, and how many of each?" (`/relations`), then pull just the
// neighbours along one chosen predicate/direction (`/expand`). Both operate over
// the in-memory `GraphData` obtained via the same `GetGraphData` CQRS query the
// other read endpoints use, so they see exactly the live graph.

/// Untyped edges (no `edge_type`) are grouped under this synthetic predicate.
const UNTYPED_EDGE_GROUP: &str = "linked";

/// Group key for an edge: its `edge_type` predicate, or `linked` when untyped.
fn edge_group_key(edge: &visionclaw_domain::models::edge::Edge) -> &str {
    match edge.edge_type.as_deref() {
        Some(t) if !t.is_empty() => t,
        _ => UNTYPED_EDGE_GROUP,
    }
}

/// Human-readable form of an edge-type key. Snake/kebab tokens become
/// space-separated Capitalised words (`is_subclass_of` -> `Is Subclass Of`);
/// anything already spaced is title-cased word-by-word.
fn prettify_edge_label(key: &str) -> String {
    let words: Vec<String> = key
        .split(|c| c == '_' || c == '-' || c == ' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        key.to_string()
    } else {
        words.join(" ")
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelationCount {
    pub edge_type: String,
    pub label: String,
    pub count: u32,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct RelationsResponse {
    pub outgoing: Vec<RelationCount>,
    pub incoming: Vec<RelationCount>,
}

/// Aggregate the edges incident to `node_id`, grouped by predicate and split by
/// direction. Outgoing = edge.source == node_id, incoming = edge.target ==
/// node_id (a self-loop contributes to both). Within each direction the counts
/// are returned heaviest-first (highest count), ties broken alphabetically by
/// edge type for deterministic output — this is the "predicate-count-first"
/// ordering Graph2VR presents to the user.
fn aggregate_relations(
    edges: &[visionclaw_domain::models::edge::Edge],
    node_id: u32,
) -> RelationsResponse {
    use std::collections::BTreeMap;
    let mut outgoing: BTreeMap<String, u32> = BTreeMap::new();
    let mut incoming: BTreeMap<String, u32> = BTreeMap::new();

    for edge in edges {
        let key = edge_group_key(edge);
        if edge.source == node_id {
            *outgoing.entry(key.to_string()).or_insert(0) += 1;
        }
        if edge.target == node_id {
            *incoming.entry(key.to_string()).or_insert(0) += 1;
        }
    }

    fn to_sorted(map: BTreeMap<String, u32>) -> Vec<RelationCount> {
        let mut v: Vec<RelationCount> = map
            .into_iter()
            .map(|(edge_type, count)| RelationCount {
                label: prettify_edge_label(&edge_type),
                edge_type,
                count,
            })
            .collect();
        // Heaviest predicate-count first; alphabetical by type on ties.
        v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.edge_type.cmp(&b.edge_type)));
        v
    }

    RelationsResponse {
        outgoing: to_sorted(outgoing),
        incoming: to_sorted(incoming),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandRequest {
    pub edge_type: String,
    pub direction: ExpandDirection,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExpandDirection {
    Outgoing,
    Incoming,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExpandNode {
    pub id: u32,
    pub metadata_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_type: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExpandEdge {
    pub source: u32,
    pub target: u32,
    pub edge_type: String,
    pub weight: f32,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ExpandResponse {
    pub nodes: Vec<ExpandNode>,
    pub edges: Vec<ExpandEdge>,
}

const EXPAND_DEFAULT_LIMIT: u32 = 25;
const EXPAND_MAX_LIMIT: u32 = 500;

/// Clamp an optional caller limit to `[1, EXPAND_MAX_LIMIT]`, defaulting to
/// `EXPAND_DEFAULT_LIMIT` when absent. A supplied `0` is treated as the default
/// rather than an empty page (defensive: avoids a silently useless response).
fn clamp_expand_limit(limit: Option<u32>) -> u32 {
    match limit {
        None | Some(0) => EXPAND_DEFAULT_LIMIT,
        Some(n) => n.min(EXPAND_MAX_LIMIT),
    }
}

/// Pure expansion core: neighbours of `node_id` along edges whose group key
/// equals `edge_type`, in `direction`, heaviest-weight first, capped at `limit`.
/// `node_lookup` resolves a neighbour id to its `(metadata_id, label,
/// node_type)`; unknown neighbours are skipped (a dangling edge target must not
/// fabricate a node). Read-only — never mutates the graph.
fn expand_neighbours<'a, F>(
    edges: &[visionclaw_domain::models::edge::Edge],
    node_id: u32,
    edge_type: &str,
    direction: ExpandDirection,
    limit: u32,
    mut node_lookup: F,
) -> ExpandResponse
where
    F: FnMut(u32) -> Option<(&'a str, &'a str, Option<String>)>,
{
    if limit == 0 {
        return ExpandResponse {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    }

    // Bounded top-k selection: the limit is pushed *into* the scan so memory
    // stays O(limit), not O(matches). A single O(edges) pass feeds a
    // `limit`-sized min-heap whose top is always the current worst-ranked edge
    // (lowest weight, ties broken by later position); once full, each new match
    // evicts that worst element. This caps allocation regardless of how many
    // edges of the requested predicate exist (DoS surface with 145k+ edges).
    struct HeapItem<'e> {
        weight: f32,
        seq: u32,
        edge: &'e visionclaw_domain::models::edge::Edge,
    }
    impl<'e> PartialEq for HeapItem<'e> {
        fn eq(&self, other: &Self) -> bool {
            self.weight.to_bits() == other.weight.to_bits() && self.seq == other.seq
        }
    }
    impl<'e> Eq for HeapItem<'e> {}
    impl<'e> Ord for HeapItem<'e> {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            // Max-heap where the "greatest" (top) is the WORST-ranked edge:
            // lower weight is worse; on a weight tie the later (higher seq) is
            // worse, so equal-weight ties keep earlier edges (stable ordering).
            other
                .weight
                .total_cmp(&self.weight)
                .then_with(|| self.seq.cmp(&other.seq))
        }
    }
    impl<'e> PartialOrd for HeapItem<'e> {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut heap: std::collections::BinaryHeap<HeapItem> = std::collections::BinaryHeap::new();
    let limit_usize = limit as usize;
    let mut seq: u32 = 0;
    for edge in edges.iter() {
        if edge_group_key(edge) != edge_type {
            continue;
        }
        let matches_dir = match direction {
            ExpandDirection::Outgoing => edge.source == node_id,
            ExpandDirection::Incoming => edge.target == node_id,
        };
        if !matches_dir {
            continue;
        }
        heap.push(HeapItem {
            weight: edge.weight,
            seq,
            edge,
        });
        seq = seq.wrapping_add(1);
        if heap.len() > limit_usize {
            heap.pop(); // evict current worst
        }
    }

    // Drain and order heaviest-weight first (ties keep earlier position).
    let mut matched: Vec<HeapItem> = heap.into_vec();
    matched.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then_with(|| a.seq.cmp(&b.seq))
    });

    let mut nodes: Vec<ExpandNode> = Vec::new();
    let mut out_edges: Vec<ExpandEdge> = Vec::new();
    let mut seen_nodes: std::collections::HashSet<u32> = std::collections::HashSet::new();

    for item in matched.into_iter() {
        let edge = item.edge;
        let neighbour = match direction {
            ExpandDirection::Outgoing => edge.target,
            ExpandDirection::Incoming => edge.source,
        };
        if let Some((metadata_id, label, node_type)) = node_lookup(neighbour) {
            if seen_nodes.insert(neighbour) {
                nodes.push(ExpandNode {
                    id: neighbour,
                    metadata_id: metadata_id.to_string(),
                    label: label.to_string(),
                    node_type,
                });
            }
            out_edges.push(ExpandEdge {
                source: edge.source,
                target: edge.target,
                edge_type: edge_group_key(edge).to_string(),
                weight: edge.weight,
            });
        }
    }

    ExpandResponse {
        nodes,
        edges: out_edges,
    }
}

/// Fetch the live in-memory graph snapshot directly from the `GraphStateActor`
/// in the handler's own async context — no per-request Tokio runtime.
///
/// The CQRS `GetGraphData` *query handler* spins up a fresh `tokio::runtime::
/// Runtime` on every call (it is a sync `QueryHandler`); for a public,
/// unauthenticated route that scans every edge that is a needless per-request
/// cost and a DoS lever. We instead resolve the actor address from the
/// supervisor and `send` the `GetGraphData` actor message asynchronously. The
/// result is an `Arc<GraphData>` clone — cheap, no graph copy.
async fn fetch_graph_snapshot(
    state: &web::Data<AppState>,
) -> Result<Arc<GraphData>, String> {
    use crate::actors::messages::{GetGraphData as ActorGetGraphData, GetGraphStateActor};

    let actor = state
        .graph_service_addr
        .send(GetGraphStateActor)
        .await
        .map_err(|e| format!("supervisor mailbox error: {}", e))?
        .ok_or_else(|| "GraphStateActor not initialised in supervisor".to_string())?;

    actor
        .send(ActorGetGraphData)
        .await
        .map_err(|e| format!("graph actor mailbox error: {}", e))?
}

/// `GET /api/graph/node/{id}/relations`
///
/// Predicate-count summary for one node: for each edge-type incident to the
/// node, how many outgoing and incoming edges of that type exist. 404 if the
/// node id is not present in the graph.
///
/// The `{id}` may arrive as a *flagged wire id* from an XR/binary client (bits
/// 26-31 carry node-type flags); it is masked with `NODE_ID_MASK` back to the
/// bare 26-bit id before lookup so a flagged id does not spuriously 404.
pub async fn get_node_relations(
    state: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let node_id = path.into_inner() & crate::utils::binary_protocol::NODE_ID_MASK;

    let graph_data = match fetch_graph_snapshot(&state).await {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to get graph data for relations: {}", e);
            return Ok::<HttpResponse, actix_web::Error>(
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({"error": "Failed to retrieve graph data"})),
            );
        }
    };

    if !graph_data.nodes.iter().any(|n| n.id == node_id) {
        return Ok(HttpResponse::NotFound()
            .json(serde_json::json!({"error": format!("Node {} not found", node_id)})));
    }

    let response = aggregate_relations(&graph_data.edges, node_id);
    Ok(HttpResponse::Ok().json(response))
}

/// `POST /api/graph/node/{id}/expand`
///
/// Neighbours of a node along one predicate/direction, capped and heaviest-
/// weight first. Read-only. 404 if the node id is unknown. `{id}` is masked with
/// `NODE_ID_MASK` (see `get_node_relations`) to accept flagged wire ids.
pub async fn expand_node(
    state: web::Data<AppState>,
    path: web::Path<u32>,
    body: web::Json<ExpandRequest>,
) -> impl Responder {
    let node_id = path.into_inner() & crate::utils::binary_protocol::NODE_ID_MASK;
    let req = body.into_inner();
    let limit = clamp_expand_limit(req.limit);

    let graph_data = match fetch_graph_snapshot(&state).await {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to get graph data for expand: {}", e);
            return Ok::<HttpResponse, actix_web::Error>(
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({"error": "Failed to retrieve graph data"})),
            );
        }
    };

    if !graph_data.nodes.iter().any(|n| n.id == node_id) {
        return Ok(HttpResponse::NotFound()
            .json(serde_json::json!({"error": format!("Node {} not found", node_id)})));
    }

    // Index nodes by id for neighbour resolution.
    let node_index: HashMap<u32, &Node> =
        graph_data.nodes.iter().map(|n| (n.id, n)).collect();

    let response = expand_neighbours(
        &graph_data.edges,
        node_id,
        &req.edge_type,
        req.direction,
        limit,
        |nid| {
            node_index
                .get(&nid)
                .map(|n| (n.metadata_id.as_str(), n.label.as_str(), n.node_type.clone()))
        },
    );

    Ok(HttpResponse::Ok().json(response))
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
            .wrap(RateLimit::per_minute(600)) // Rate limit: 600 requests/min (10/sec) for public reads
            // Read operations - public with rate limiting
            .route("/data", web::get().to(get_graph_data))
            .route("/data/paginated", web::get().to(get_paginated_graph_data))
            .route("/positions", web::get().to(get_graph_positions))
            .route(
                "/auto-balance-notifications",
                web::get().to(get_auto_balance_notifications),
            )
            // Graph2VR predicate-count-first expansion — read-only browsing of
            // one node's relations, then neighbours along a chosen predicate.
            // Same auth posture as the other reads (public); the `/expand` POST
            // mutates nothing. Each scans every edge, so they carry a TIGHTER
            // per-resource limit (120/min) than the 600/min scope default —
            // wrapped on the resource, stacking under the scope limiter so the
            // stricter ceiling gates these two routes.
            .service(
                web::resource("/node/{id}/relations")
                    .wrap(RateLimit::per_minute(120))
                    .route(web::get().to(get_node_relations)),
            )
            .service(
                web::resource("/node/{id}/expand")
                    .wrap(RateLimit::per_minute(120))
                    .route(web::post().to(expand_node)),
            )
            // S2: `/update` triggers a full bulk reload — it re-fetches and
            // re-processes the entire upstream content source and rebuilds the graph
            // from metadata (AddNodesFromMetadata). That is a destructive/expensive
            // privileged operation, not a routine per-node edit, so it is escalated
            // to power_user (Admin). A regular NIP-98 user (Authenticated) can no
            // longer trigger a global rebuild.
            .service(
                web::resource("/update")
                    .wrap(RequireAuth::power_user()) // Bulk reload requires power-user
                    .route(web::post().to(update_graph)),
            )
            // `/refresh` only reads the current graph state (GetGraphData) and returns
            // it; it mutates nothing, so any authenticated user may call it.
            .service(
                web::resource("/refresh")
                    .wrap(RequireAuth::authenticated()) // Read-back, any authed user
                    .route(web::post().to(refresh_graph)),
            ),
    );
}

#[cfg(test)]
mod population_filter_tests {
    use super::PopulationFilter;
    use std::collections::HashMap;

    fn md(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
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

#[cfg(test)]
mod relations_expand_tests {
    use super::*;
    use visionclaw_domain::models::edge::Edge;

    // Small fixture: node 1 is the hub.
    //   1 -[implements]-> 2   (w 0.9)
    //   1 -[implements]-> 3   (w 0.4)
    //   1 -[requires]--->  4  (w 0.7)
    //   1 -[<untyped>]---> 5  (w 0.2)
    //   6 -[implements]-> 1   (incoming, w 0.5)
    //   7 -[requires]---> 1   (incoming, w 0.8)
    fn fixture_edges() -> Vec<Edge> {
        vec![
            Edge::new(1, 2, 0.9).with_edge_type("implements".to_string()),
            Edge::new(1, 3, 0.4).with_edge_type("implements".to_string()),
            Edge::new(1, 4, 0.7).with_edge_type("requires".to_string()),
            Edge::new(1, 5, 0.2), // untyped -> "linked"
            Edge::new(6, 1, 0.5).with_edge_type("implements".to_string()),
            Edge::new(7, 1, 0.8).with_edge_type("requires".to_string()),
        ]
    }

    fn lookup(nid: u32) -> Option<(&'static str, &'static str, Option<String>)> {
        // Deliberately omit node 3 to exercise the "unknown neighbour skipped" path.
        match nid {
            2 => Some(("meta-2", "Node Two", Some("page".to_string()))),
            4 => Some(("meta-4", "Node Four", None)),
            5 => Some(("meta-5", "Node Five", Some("linked_page".to_string()))),
            6 => Some(("meta-6", "Node Six", Some("page".to_string()))),
            7 => Some(("meta-7", "Node Seven", None)),
            _ => None,
        }
    }

    // --- edge_group_key / prettify ---

    #[test]
    fn untyped_edges_group_under_linked() {
        assert_eq!(edge_group_key(&Edge::new(1, 2, 1.0)), UNTYPED_EDGE_GROUP);
        assert_eq!(
            edge_group_key(&Edge::new(1, 2, 1.0).with_edge_type(String::new())),
            UNTYPED_EDGE_GROUP
        );
        assert_eq!(
            edge_group_key(&Edge::new(1, 2, 1.0).with_edge_type("implements".to_string())),
            "implements"
        );
    }

    #[test]
    fn prettify_edge_label_title_cases_tokens() {
        assert_eq!(prettify_edge_label("is_subclass_of"), "Is Subclass Of");
        assert_eq!(prettify_edge_label("implements"), "Implements");
        assert_eq!(prettify_edge_label("bridges-to"), "Bridges To");
        assert_eq!(prettify_edge_label("linked"), "Linked");
    }

    // --- aggregate_relations ---

    #[test]
    fn aggregate_counts_group_by_type_and_direction() {
        let r = aggregate_relations(&fixture_edges(), 1);

        // Outgoing: implements x2 (heaviest count first), requires x1, linked x1.
        assert_eq!(r.outgoing[0].edge_type, "implements");
        assert_eq!(r.outgoing[0].count, 2);
        assert_eq!(r.outgoing[0].label, "Implements");

        let requires = r
            .outgoing
            .iter()
            .find(|c| c.edge_type == "requires")
            .unwrap();
        assert_eq!(requires.count, 1);
        let linked = r
            .outgoing
            .iter()
            .find(|c| c.edge_type == UNTYPED_EDGE_GROUP)
            .unwrap();
        assert_eq!(linked.count, 1);

        // Incoming: implements x1, requires x1.
        assert_eq!(r.incoming.len(), 2);
        assert_eq!(
            r.incoming
                .iter()
                .find(|c| c.edge_type == "implements")
                .unwrap()
                .count,
            1
        );
        assert_eq!(
            r.incoming
                .iter()
                .find(|c| c.edge_type == "requires")
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn aggregate_orders_heaviest_count_first() {
        let r = aggregate_relations(&fixture_edges(), 1);
        // First outgoing entry is the most frequent predicate.
        for w in r.outgoing.windows(2) {
            assert!(w[0].count >= w[1].count);
        }
    }

    #[test]
    fn aggregate_self_loop_counts_both_directions() {
        let edges = vec![Edge::new(1, 1, 1.0).with_edge_type("relates_to".to_string())];
        let r = aggregate_relations(&edges, 1);
        assert_eq!(r.outgoing[0].count, 1);
        assert_eq!(r.incoming[0].count, 1);
    }

    #[test]
    fn aggregate_unknown_node_yields_empty() {
        let r = aggregate_relations(&fixture_edges(), 999);
        assert!(r.outgoing.is_empty());
        assert!(r.incoming.is_empty());
    }

    // --- clamp_expand_limit ---

    #[test]
    fn clamp_defaults_and_caps() {
        assert_eq!(clamp_expand_limit(None), EXPAND_DEFAULT_LIMIT);
        assert_eq!(clamp_expand_limit(Some(0)), EXPAND_DEFAULT_LIMIT);
        assert_eq!(clamp_expand_limit(Some(10)), 10);
        assert_eq!(clamp_expand_limit(Some(1000)), EXPAND_MAX_LIMIT);
        assert_eq!(clamp_expand_limit(Some(EXPAND_MAX_LIMIT)), EXPAND_MAX_LIMIT);
    }

    // --- expand_neighbours ---

    #[test]
    fn expand_outgoing_filters_by_type_and_orders_by_weight() {
        // implements outgoing: edges to 2 (0.9) and 3 (0.4). Node 3 is unknown
        // so it is skipped from `nodes`, but node 2 comes first (heaviest).
        let resp = expand_neighbours(
            &fixture_edges(),
            1,
            "implements",
            ExpandDirection::Outgoing,
            25,
            lookup,
        );
        assert_eq!(resp.nodes.len(), 1);
        assert_eq!(resp.nodes[0].id, 2);
        assert_eq!(resp.nodes[0].metadata_id, "meta-2");
        assert_eq!(resp.nodes[0].node_type.as_deref(), Some("page"));
        // Only the resolvable edge (to node 2) is emitted.
        assert_eq!(resp.edges.len(), 1);
        assert_eq!(resp.edges[0].source, 1);
        assert_eq!(resp.edges[0].target, 2);
        assert_eq!(resp.edges[0].edge_type, "implements");
        assert!((resp.edges[0].weight - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn expand_incoming_reads_source_side() {
        // requires incoming: edge 7 -> 1 (w 0.8).
        let resp = expand_neighbours(
            &fixture_edges(),
            1,
            "requires",
            ExpandDirection::Incoming,
            25,
            lookup,
        );
        assert_eq!(resp.nodes.len(), 1);
        assert_eq!(resp.nodes[0].id, 7);
        assert_eq!(resp.edges[0].source, 7);
        assert_eq!(resp.edges[0].target, 1);
    }

    #[test]
    fn expand_direction_isolates_matches() {
        // implements incoming: only edge 6 -> 1.
        let resp = expand_neighbours(
            &fixture_edges(),
            1,
            "implements",
            ExpandDirection::Incoming,
            25,
            lookup,
        );
        assert_eq!(resp.nodes.len(), 1);
        assert_eq!(resp.nodes[0].id, 6);
    }

    #[test]
    fn expand_untyped_edges_addressable_as_linked() {
        let resp = expand_neighbours(
            &fixture_edges(),
            1,
            UNTYPED_EDGE_GROUP,
            ExpandDirection::Outgoing,
            25,
            lookup,
        );
        assert_eq!(resp.nodes.len(), 1);
        assert_eq!(resp.nodes[0].id, 5);
        assert_eq!(resp.edges[0].edge_type, UNTYPED_EDGE_GROUP);
    }

    #[test]
    fn expand_respects_limit_heaviest_first() {
        // Three implements-outgoing edges from node 1 with distinct weights;
        // limit 2 must keep the two heaviest (targets 20 then 10), all resolvable.
        let edges = vec![
            Edge::new(1, 10, 0.3).with_edge_type("implements".to_string()),
            Edge::new(1, 20, 0.9).with_edge_type("implements".to_string()),
            Edge::new(1, 30, 0.6).with_edge_type("implements".to_string()),
        ];
        let resp = expand_neighbours(&edges, 1, "implements", ExpandDirection::Outgoing, 2, |nid| {
            match nid {
                10 => Some(("m10", "Ten", None)),
                20 => Some(("m20", "Twenty", None)),
                30 => Some(("m30", "Thirty", None)),
                _ => None,
            }
        });
        assert_eq!(resp.edges.len(), 2);
        assert_eq!(resp.edges[0].target, 20); // 0.9
        assert_eq!(resp.edges[1].target, 30); // 0.6
    }

    #[test]
    fn expand_no_matching_type_is_empty() {
        let resp = expand_neighbours(
            &fixture_edges(),
            1,
            "nonexistent",
            ExpandDirection::Outgoing,
            25,
            lookup,
        );
        assert!(resp.nodes.is_empty());
        assert!(resp.edges.is_empty());
    }

    // --- flagged wire-id masking (bits 26-31 carry type flags) ---

    #[test]
    fn node_id_mask_strips_wire_flags() {
        use crate::utils::binary_protocol::NODE_ID_MASK;
        // AGENT_NODE_FLAG 0x80000000, KNOWLEDGE 0x40000000, ontology bits 26-28.
        let bare: u32 = 42;
        for flag in [0x8000_0000u32, 0x4000_0000, 0x1C00_0000, 0x0400_0000] {
            let flagged = bare | flag;
            assert_eq!(flagged & NODE_ID_MASK, bare, "flag {:#x}", flag);
        }
        // A bare id round-trips unchanged.
        assert_eq!(bare & NODE_ID_MASK, bare);
    }

    // --- camelCase wire contract: relation output must feed expansion input ---

    #[test]
    fn relation_count_serialises_camel_case() {
        let rc = RelationCount {
            edge_type: "is_subclass_of".to_string(),
            label: "Is Subclass Of".to_string(),
            count: 3,
        };
        let json = serde_json::to_string(&rc).unwrap();
        assert!(json.contains("\"edgeType\""), "got {}", json);
        assert!(!json.contains("edge_type"));
    }

    #[test]
    fn expand_request_deserialises_camel_case() {
        // The exact shape a client builds from a RelationCount entry.
        let body = r#"{"edgeType":"implements","direction":"outgoing","limit":10}"#;
        let req: ExpandRequest = serde_json::from_str(body).unwrap();
        assert_eq!(req.edge_type, "implements");
        assert_eq!(req.direction, ExpandDirection::Outgoing);
        assert_eq!(req.limit, Some(10));
    }

    #[test]
    fn expand_node_and_edge_serialise_camel_case() {
        let n = ExpandNode {
            id: 7,
            metadata_id: "meta-7".to_string(),
            label: "Seven".to_string(),
            node_type: Some("page".to_string()),
        };
        let nj = serde_json::to_string(&n).unwrap();
        assert!(nj.contains("\"metadataId\""));
        assert!(nj.contains("\"nodeType\""));

        let e = ExpandEdge {
            source: 1,
            target: 7,
            edge_type: "implements".to_string(),
            weight: 0.5,
        };
        let ej = serde_json::to_string(&e).unwrap();
        assert!(ej.contains("\"edgeType\""));
        assert!(!ej.contains("edge_type"));
    }
}
