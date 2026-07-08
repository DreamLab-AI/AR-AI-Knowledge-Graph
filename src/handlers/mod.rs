pub mod admin_sync_handler;
pub mod api_handler;
pub mod bots_handler;
pub mod bots_visualization_handler;
pub mod client_log_handler;
pub mod client_messages_handler;
pub mod consolidated_health_handler;
pub mod metrics_handler;
pub mod constraints_handler;
pub mod graph_export_handler;
pub mod graph_state_handler;
pub mod mcp_relay_handler;
pub mod multi_mcp_websocket_handler;
pub mod natural_language_query_handler;
pub mod nostr_handler;
pub mod ontology_handler;
pub mod ontology_agent_handler;
pub use ontology_agent_handler::configure_ontology_agent_routes;
pub mod pages_handler;
pub mod ragflow_handler;
pub mod settings_handler;
pub mod settings_validation_fix;
pub mod socket_flow_handler;
pub mod speech_socket_handler;
pub mod utils;
pub mod validation_handler;
pub mod websocket_utils;
pub mod workspace_handler;

// Phase 5: Hexagonal architecture handlers
pub mod physics_handler;
pub mod schema_handler;
pub mod semantic_handler;

pub use natural_language_query_handler::configure_nl_query_routes;
pub use physics_handler::configure_routes as configure_physics_routes;
pub use schema_handler::configure_schema_routes;
pub use semantic_handler::configure_routes as configure_semantic_routes;

// Phase 7: Inference handler
pub mod inference_handler;

pub use inference_handler::configure_routes as configure_inference_routes;

#[cfg(test)]
pub mod tests;
pub mod semantic_pathfinding_handler;
pub use semantic_pathfinding_handler::configure_pathfinding_routes;

// Briefing workflow handler
pub mod briefing_handler;
pub use briefing_handler::configure_routes as configure_briefing_routes;

// Memory flash handler (RuVector access → WS broadcast)
pub mod memory_flash_handler;
pub use memory_flash_handler::configure_routes as configure_memory_flash_routes;

// Enrichment-proposals governance decide endpoint (broker write-back loop)
pub mod enrichment_proposals_handler;
pub use enrichment_proposals_handler::configure_routes as configure_enrichment_proposals_routes;

// Derived ontology graphs (WS-9 fenced write: :summary/:observed only)
pub mod ontology_derived_handler;
pub use ontology_derived_handler::configure_routes as configure_ontology_derived_routes;

// Broker inbox read surface (WS-12) — serves the agentbox broker-bridge
pub mod broker_inbox_handler;
pub use broker_inbox_handler::configure_routes as configure_broker_inbox_routes;

// RES-a: LivenessHarness HTTP surface (register / observe / status) — ADR-130 D3
pub mod liveness_harness_handler;
pub use liveness_harness_handler::configure_routes as configure_liveness_routes;

// REC-4 (PRD-023 WP-8): four-KPI dashboard surface — /api/kpi/{summary,lineage}
pub mod kpi_handler;
pub use kpi_handler::configure_routes as configure_kpi_routes;

// REC-10 (PRD-023 WP-12): Insight Ingestion Loop v1 —
// /api/insight-loop/trace[/{case_id}]
pub mod insight_loop_handler;
pub use insight_loop_handler::configure_routes as configure_insight_loop_routes;

// REC-11 (PRD-023 WP-12): data-moat unified provenance trace — /api/trace
pub mod trace_handler;
pub use trace_handler::configure_routes as configure_trace_routes;

// RES-d (PRD-023 WP-12): script-queryable ontology class-count source for the
// canon DriftCounter — /api/ontology/class-count
pub mod ontology_class_count_handler;
pub use ontology_class_count_handler::configure_routes as configure_ontology_class_count_routes;

// Layout mode system (ADR-031)
pub mod layout_handler;
pub use layout_handler::configure_layout_routes;

// High-Performance Networking (QUIC/WebTransport + fastwebsockets)
pub mod quic_transport_handler;
pub mod fastwebsockets_handler;

// Solid Pod (embedded solid-pod-rs)
pub mod solid_proxy_handler;
pub use solid_proxy_handler::configure_routes as configure_solid_routes;
#[cfg(feature = "solid-pod-embed")]
pub use solid_proxy_handler::init_solid_state;

// Image generation (ComfyUI Flux2)
pub mod image_gen_handler;
pub use image_gen_handler::configure_routes as configure_image_gen_routes;

// HTTP 402 payments / exchange (Web Ledgers + AMM) — gated on solid-pod-rs
#[cfg(feature = "solid-pod-embed")]
pub mod pay_handler;
#[cfg(feature = "solid-pod-embed")]
pub use pay_handler::configure_pay_routes;

// PRD-008: XR presence WebSocket (`/ws/presence`)
pub mod presence_handler;

pub use quic_transport_handler::{
    QuicTransportServer, QuicServerConfig,
    PostcardNodeUpdate, PostcardBatchUpdate, PostcardDeltaUpdate,
    ControlMessage, TopologyNode, TopologyEdge,
    encode_postcard_batch, decode_postcard_batch, calculate_deltas,
};

pub use fastwebsockets_handler::{
    FastWebSocketServer, FastWebSocketConfig,
    StandaloneFastWsHandler,
    TransportProtocol, SerializationFormat, NegotiatedProtocol,
    negotiate_protocol,
};
