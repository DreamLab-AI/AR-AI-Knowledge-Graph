// ADR-110 — ACSP producer: agentic actors project control panels into the
// forum governance page and receive human decisions (kinds 31400-31405).
pub mod acsp;
pub mod agent_visualization_processor;
pub mod agent_visualization_protocol;
pub mod bots_client;
pub mod file_service;
pub mod github;
pub mod github_sync_service;
pub mod local_file_sync_service;
pub mod management_api_client;
pub mod multi_mcp_agent_discovery;
pub mod natural_language_query_service;
pub mod parsers;
pub mod graph_serialization;
pub mod mcp_relay_manager;
pub mod nostr_service;
pub mod owl_validator;
pub mod perplexity_service;
pub mod ragflow_service;
pub mod schema_service;
pub mod semantic_analyzer;
pub mod semantic_pathfinding_service;
pub mod audio_router;
pub mod speech_service;
pub mod speech_voice_integration;
pub mod voice_context_manager;
pub mod voice_tag_manager;
pub mod ontology_converter;
pub mod edge_classifier;
pub mod ontology_reasoner;
pub mod ontology_enrichment_service;
pub mod ontology_reasoning_service;
pub mod ontology_pipeline_service;
pub mod ontology_content_analyzer;
pub mod ontology_file_cache;
pub mod pathfinding;
pub mod semantic_type_registry;
pub mod ontology_query_service;
pub mod ontology_mutation_service;
pub mod github_pr_service;
pub mod briefing_service;
pub mod nostr_bead_publisher;
pub mod nostr_bridge;
// PRD-008 §5.3 — Schnorr identity verifier for the XR presence handshake
pub mod nostr_identity_verifier;

// RES-a: sprint-wide live-traffic observer + KG-backend watchdog (ADR-130 D3)
pub mod liveness_harness;

// JSON-LD validator (Data Sprint Phase D-2). Pure markdown + JSON-LD
// validation; does NOT depend on the persistence-oxigraph feature.
pub mod jsonld_validator;

// JSON-LD ingest pipeline (Migration Sprint Phase 2 M1). Parses Logseq
// markdown JSON-LD blocks → oxigraph::model::Quad sets routed to Phase 1
// repository ports.
pub mod jsonld_ingest;

// Re-export semantic type registry types for convenience
pub use semantic_type_registry::{
    DynamicForceConfigGPU, RelationshipForceConfig, SemanticTypeRegistry,
    SEMANTIC_TYPE_REGISTRY,
};
