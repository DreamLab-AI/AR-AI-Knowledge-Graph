// src/bin/sync_local.rs
//! Local file sync binary - syncs local baseline with GitHub delta updates
//!
//! Uses Oxigraph + SQLite persistence (ADR-11).

use std::sync::Arc;
use tokio::sync::RwLock;
use visionclaw_server::adapters::{OxigraphGraphRepository, OxigraphOntologyRepository};
use visionclaw_server::config::AppFullSettings;
use visionclaw_server::services::github::api::GitHubClient;
use visionclaw_server::services::github::config::GitHubConfig;
use visionclaw_server::services::github::content_enhanced::EnhancedContentAPI;
use visionclaw_server::services::local_file_sync_service::LocalFileSyncService;
use visionclaw_server::services::ontology_enrichment_service::OntologyEnrichmentService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting local file sync with GitHub delta updates (Oxigraph backend, ADR-11)");

    // Load environment variables
    dotenvy::dotenv().ok();

    // Open Oxigraph store
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let oxigraph_path = std::path::Path::new(&data_dir).join("oxigraph");
    log::info!("Opening Oxigraph store at: {}", oxigraph_path.display());

    let onto_repo = Arc::new(OxigraphOntologyRepository::open(&oxigraph_path).await?);
    let oxigraph_store = onto_repo.store().clone();
    let kg_repo = Arc::new(OxigraphGraphRepository::from_store(oxigraph_store));

    log::info!("Oxigraph store opened successfully");

    // Initialize GitHub client
    let github_config = GitHubConfig::from_env()?;
    let settings = Arc::new(RwLock::new(AppFullSettings::default()));
    let github_client = Arc::new(GitHubClient::new(github_config, settings).await?);

    let content_api = Arc::new(EnhancedContentAPI::new(github_client));

    // Initialize whelk inference engine for ontology reasoning
    let whelk_engine =
        Arc::new(visionclaw_server::adapters::whelk_inference_engine::WhelkInferenceEngine::new());
    let reasoner = Arc::new(
        visionclaw_server::services::ontology_reasoner::OntologyReasoner::new(
            whelk_engine,
            onto_repo.clone()
                as Arc<dyn visionclaw_server::ports::ontology_repository::OntologyRepository>,
        ),
    );

    // Initialize edge classifier (no arguments needed)
    let edge_classifier =
        Arc::new(visionclaw_server::services::edge_classifier::EdgeClassifier::new());

    let enrichment_service = Arc::new(OntologyEnrichmentService::new(reasoner, edge_classifier));

    // Create sync service
    let sync_service = LocalFileSyncService::new(
        content_api,
        kg_repo
            as Arc<
                dyn visionclaw_server::ports::knowledge_graph_repository::KnowledgeGraphRepository,
            >,
        onto_repo,
        enrichment_service,
    );

    log::info!("Starting sync operation...");

    // Run sync
    let stats = sync_service.sync_with_github_delta().await?;

    // Display results
    println!("\nSync complete!");
    println!("{}", "=".repeat(43));
    println!("Statistics:");
    println!("  Total files scanned:      {}", stats.total_files);
    println!(
        "  Files synced from local:  {}",
        stats.files_synced_from_local
    );
    println!(
        "  Files updated from GitHub: {}",
        stats.files_updated_from_github
    );
    println!("  Knowledge graph files:    {}", stats.kg_files_processed);
    println!(
        "  Ontology files processed: {}",
        stats.ontology_files_processed
    );
    println!("  Skipped files:            {}", stats.skipped_files);
    println!("  Duration:                 {:?}", stats.duration);
    println!("{}", "=".repeat(43));

    if !stats.errors.is_empty() {
        println!("\nErrors encountered ({}):", stats.errors.len());
        for (i, error) in stats.errors.iter().enumerate().take(10) {
            println!("  {}. {}", i + 1, error);
        }
        if stats.errors.len() > 10 {
            println!("  ... and {} more errors", stats.errors.len() - 10);
        }
    }

    log::info!("Sync binary completed successfully");

    Ok(())
}
