// src/bin/sync_github.rs
//! GitHub sync binary — pulls all markdown files from GitHub and ingests to Oxigraph.
//! Uses GitHubSyncService::sync_graphs() for a full remote pull.

use std::sync::Arc;
use tokio::sync::RwLock;
use visionclaw_server::adapters::{
    OxigraphGraphRepository, OxigraphOntologyRepository, SqliteSettingsRepository,
};
use visionclaw_server::config::AppFullSettings;
use visionclaw_server::services::github::api::GitHubClient;
use visionclaw_server::services::github::config::GitHubConfig;
use visionclaw_server::services::github::content_enhanced::EnhancedContentAPI;
use visionclaw_server::services::github_sync_service::GitHubSyncService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting full GitHub sync (Oxigraph backend)");

    dotenvy::dotenv().ok();

    // Open Oxigraph store for both KG and ontology (ADR-11 — single embedded store)
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let oxigraph_path = std::path::Path::new(&data_dir).join("oxigraph");
    log::info!("Opening Oxigraph store at {}", oxigraph_path.display());
    let onto_repo = Arc::new(
        OxigraphOntologyRepository::open(&oxigraph_path)
            .await
            .map_err(|e| format!("Failed to open Oxigraph store: {}", e))?,
    );
    let kg_repo = Arc::new(OxigraphGraphRepository::from_store(
        onto_repo.store().clone(),
    ));
    log::info!("Oxigraph store opened successfully");

    // SQLite settings repository (shared with sync metadata)
    let settings_db_path = std::path::Path::new(&data_dir).join("settings.sqlite3");
    let sqlite_settings_repo = Arc::new(
        SqliteSettingsRepository::open(&settings_db_path)
            .await
            .map_err(|e| format!("Failed to open SQLite settings: {}", e))?,
    );

    // GitHub client
    let github_config = GitHubConfig::from_env()?;
    let settings = Arc::new(RwLock::new(AppFullSettings::default()));
    let github_client = Arc::new(GitHubClient::new(github_config, settings).await?);
    let content_api = Arc::new(EnhancedContentAPI::new(github_client));

    // Create sync service
    let sync_service = GitHubSyncService::new(
        content_api,
        kg_repo
            as Arc<
                dyn visionclaw_server::ports::knowledge_graph_repository::KnowledgeGraphRepository,
            >,
        onto_repo,
        sqlite_settings_repo,
    );

    log::info!("Starting sync from GitHub...");

    let stats = sync_service.sync_graphs().await?;

    println!("\nSync complete!");
    println!("{}", "=".repeat(50));
    println!("  Total files found:        {}", stats.total_files);
    println!("  KG files processed:       {}", stats.kg_files_processed);
    println!(
        "  Ontology files processed: {}",
        stats.ontology_files_processed
    );
    println!("  Skipped (unchanged):      {}", stats.skipped_files);
    println!("  Total nodes:              {}", stats.total_nodes);
    println!("  Total edges:              {}", stats.total_edges);
    println!("  Duration:                 {:?}", stats.duration);
    println!("{}", "=".repeat(50));

    if !stats.errors.is_empty() {
        println!("\nErrors ({}):", stats.errors.len());
        for (i, error) in stats.errors.iter().enumerate().take(10) {
            println!("  {}. {}", i + 1, error);
        }
        if stats.errors.len() > 10 {
            println!("  ... and {} more", stats.errors.len() - 10);
        }
    }

    Ok(())
}
