// src/services/local_file_sync_service.rs
//! Local File Sync Service with GitHub SHA1 Delta Updates
//!
//! Strategy:
//! 1. Primary source: Local filesystem at /app/data/pages (mounted from host)
//! 2. GitHub API: Only for SHA1 hash comparison to detect changed files
//! 3. Incremental updates: Download only files with different SHA1 from GitHub
//!
//! This avoids pagination issues with 250k+ files by using local baseline.

use crate::adapters::OxigraphOntologyRepository;
use crate::ports::knowledge_graph_repository::KnowledgeGraphRepository;
use crate::services::github::content_enhanced::EnhancedContentAPI;
use crate::services::github::types::{OntologyFileMetadata, OntologyPriority};
use crate::services::ontology_content_analyzer::OntologyContentAnalyzer;
use crate::services::ontology_enrichment_service::OntologyEnrichmentService;
use crate::services::ontology_file_cache::{
    CachedOntologyFile, OntologyCacheConfig, OntologyFileCache,
};
use crate::services::parsers::{KnowledgeGraphParser, OntologyParser};
use log::{debug, error, info, warn};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use visionclaw_domain::ports::ontology_repository::OntologyRepository;

const BATCH_SIZE: usize = 50;
const LOCAL_PAGES_DIR: &str = "/app/data/pages";

#[derive(Clone)]
pub struct LocalFileSyncService {
    content_api: Arc<EnhancedContentAPI>,
    kg_parser: Arc<KnowledgeGraphParser>,
    onto_parser: Arc<OntologyParser>,
    kg_repo: Arc<dyn KnowledgeGraphRepository>,
    onto_repo: Arc<OxigraphOntologyRepository>,
    enrichment_service: Arc<OntologyEnrichmentService>,
    content_analyzer: Arc<OntologyContentAnalyzer>,
    ontology_cache: Arc<OntologyFileCache>,
}

#[derive(Debug, Clone)]
pub struct SyncStatistics {
    pub total_files: usize,
    pub files_synced_from_local: usize,
    pub files_updated_from_github: usize,
    pub kg_files_processed: usize,
    pub ontology_files_processed: usize,
    pub skipped_files: usize,
    pub errors: Vec<String>,
    pub duration: Duration,

    // Ontology-specific statistics
    pub priority1_files: usize, // public:: true AND OntologyBlock
    pub priority2_files: usize, // OntologyBlock only
    pub priority3_files: usize, // public:: true only
    pub total_classes: usize,
    pub total_properties: usize,
    pub total_relationships: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub files_with_commit_dates: usize,
}

impl LocalFileSyncService {
    pub fn new(
        content_api: Arc<EnhancedContentAPI>,
        kg_repo: Arc<dyn KnowledgeGraphRepository>,
        onto_repo: Arc<OxigraphOntologyRepository>,
        enrichment_service: Arc<OntologyEnrichmentService>,
    ) -> Self {
        Self {
            content_api,
            kg_parser: Arc::new(KnowledgeGraphParser::new()),
            onto_parser: Arc::new(OntologyParser::new()),
            kg_repo,
            onto_repo,
            enrichment_service,
            content_analyzer: Arc::new(OntologyContentAnalyzer::new()),
            ontology_cache: Arc::new(OntologyFileCache::new(OntologyCacheConfig::default())),
        }
    }

    /// Main sync operation: Use local files as baseline, update from GitHub if SHA1 differs
    pub async fn sync_with_github_delta(&self) -> Result<SyncStatistics, String> {
        info!("Starting local file sync with GitHub SHA1 delta check");
        let start_time = Instant::now();

        let mut stats = SyncStatistics {
            total_files: 0,
            files_synced_from_local: 0,
            files_updated_from_github: 0,
            kg_files_processed: 0,
            ontology_files_processed: 0,
            skipped_files: 0,
            errors: Vec::new(),
            duration: Duration::from_secs(0),
            priority1_files: 0,
            priority2_files: 0,
            priority3_files: 0,
            total_classes: 0,
            total_properties: 0,
            total_relationships: 0,
            cache_hits: 0,
            cache_misses: 0,
            files_with_commit_dates: 0,
        };

        // Step 1: Read all local markdown files
        let local_files = self.scan_local_pages()?;
        stats.total_files = local_files.len();
        info!(
            "📂 Found {} local markdown files in {}",
            local_files.len(),
            LOCAL_PAGES_DIR
        );

        // Step 2: Get SHA1 hashes from GitHub (lightweight API call - only metadata, not content)
        info!("🔍 Fetching GitHub SHA1 hashes for comparison...");
        let github_sha_map = match self.fetch_github_sha_map().await {
            Ok(map) => {
                info!("Retrieved SHA1 hashes for {} files from GitHub", map.len());
                map
            }
            Err(e) => {
                warn!(
                    "⚠️  Failed to fetch GitHub SHA1 map: {}. Proceeding with local files only.",
                    e
                );
                HashMap::new()
            }
        };

        // Step 3: Process local files in batches
        let mut nodes = HashMap::new();
        let mut edges = HashMap::new();
        let mut public_pages = std::collections::HashSet::new();
        let mut batch_count = 0;

        for (index, local_file) in local_files.iter().enumerate() {
            let file_name = local_file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Calculate local SHA1
            let local_sha = match self.calculate_file_sha1(&local_file) {
                Ok(sha) => sha,
                Err(e) => {
                    error!("Failed to calculate SHA1 for {:?}: {}", local_file, e);
                    stats
                        .errors
                        .push(format!("SHA1 calculation failed: {}", file_name));
                    continue;
                }
            };

            // Check if file needs update from GitHub
            let needs_github_update = github_sha_map
                .get(file_name)
                .map(|github_sha| github_sha != &local_sha)
                .unwrap_or(false);

            let content = if needs_github_update {
                // Download updated content from GitHub
                info!("Updating {} from GitHub (SHA1 mismatch)", file_name);
                match self.fetch_and_update_file(&local_file, file_name).await {
                    Ok(content) => {
                        stats.files_updated_from_github += 1;
                        content
                    }
                    Err(e) => {
                        error!("Failed to update {} from GitHub: {}", file_name, e);
                        stats
                            .errors
                            .push(format!("GitHub update failed: {}", file_name));
                        // Fallback to local file
                        match fs::read_to_string(&local_file) {
                            Ok(c) => c,
                            Err(e) => {
                                error!("Failed to read local file {:?}: {}", local_file, e);
                                continue;
                            }
                        }
                    }
                }
            } else {
                // Use local file (already up-to-date or GitHub unavailable)
                match fs::read_to_string(&local_file) {
                    Ok(content) => {
                        stats.files_synced_from_local += 1;
                        content
                    }
                    Err(e) => {
                        error!("Failed to read local file {:?}: {}", local_file, e);
                        stats.errors.push(format!("Read error: {}", file_name));
                        continue;
                    }
                }
            };

            // Process file content with ontology-aware filtering
            if let Err(e) = self
                .process_file_content(
                    file_name,
                    &content,
                    &local_sha,
                    &mut nodes,
                    &mut edges,
                    &mut public_pages,
                    &mut stats,
                )
                .await
            {
                error!("Failed to process {}: {}", file_name, e);
                stats
                    .errors
                    .push(format!("Processing error: {}", file_name));
            }

            // Batch save every BATCH_SIZE files
            if (index + 1) % BATCH_SIZE == 0 || index == local_files.len() - 1 {
                batch_count += 1;
                info!(
                    "💾 Saving batch {} ({}/{} files processed)",
                    batch_count,
                    index + 1,
                    local_files.len()
                );

                if let Err(e) = self.save_batch(&nodes, &edges).await {
                    error!("Failed to save batch {}: {}", batch_count, e);
                    stats.errors.push(format!("Batch save error: {}", e));
                } else {
                    nodes.clear();
                    edges.clear();
                }
            }

            if (index + 1) % 100 == 0 {
                info!(
                    "Progress: {}/{} files processed",
                    index + 1,
                    local_files.len()
                );
            }
        }

        stats.duration = start_time.elapsed();

        // Get cache statistics
        let cache_stats = self.ontology_cache.get_stats().await;
        stats.cache_hits = cache_stats.hits;
        stats.cache_misses = cache_stats.misses;

        info!(
            "Sync complete! {} files from local, {} updated from GitHub in {:?}",
            stats.files_synced_from_local, stats.files_updated_from_github, stats.duration
        );

        self.log_ontology_statistics(&stats);

        Ok(stats)
    }

    /// Log detailed ontology statistics
    fn log_ontology_statistics(&self, stats: &SyncStatistics) {
        info!("Ontology Sync Statistics:");
        info!(
            "   Priority 1 files (public + ontology): {}",
            stats.priority1_files
        );
        info!(
            "   Priority 2 files (ontology only): {}",
            stats.priority2_files
        );
        info!(
            "   Priority 3 files (public only): {}",
            stats.priority3_files
        );
        info!("   Total classes extracted: {}", stats.total_classes);
        info!("   Total properties extracted: {}", stats.total_properties);
        info!("   Total relationships: {}", stats.total_relationships);
        info!(
            "   Cache performance: {} hits, {} misses ({:.2}% hit rate)",
            stats.cache_hits,
            stats.cache_misses,
            if stats.cache_hits + stats.cache_misses > 0 {
                (stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64) * 100.0
            } else {
                0.0
            }
        );
        info!(
            "   Files with commit dates: {}",
            stats.files_with_commit_dates
        );
    }

    /// Scan local pages directory for markdown files
    fn scan_local_pages(&self) -> Result<Vec<PathBuf>, String> {
        let pages_dir = Path::new(LOCAL_PAGES_DIR);

        if !pages_dir.exists() {
            return Err(format!(
                "Local pages directory does not exist: {}",
                LOCAL_PAGES_DIR
            ));
        }

        let mut md_files = Vec::new();

        for entry in fs::read_dir(pages_dir)
            .map_err(|e| format!("Failed to read directory {}: {}", LOCAL_PAGES_DIR, e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                md_files.push(path);
            }
        }

        md_files.sort();
        Ok(md_files)
    }

    /// Fetch SHA1 hash map from GitHub API (lightweight - only metadata)
    async fn fetch_github_sha_map(&self) -> Result<HashMap<String, String>, String> {
        // Use GitHub tree API for efficient metadata retrieval
        // This avoids pagination issues by getting all file metadata in one call

        // For now, use the existing list_markdown_files (with pagination fix)
        // Future: Implement git tree API for better efficiency
        let github_files = self
            .content_api
            .list_markdown_files("")
            .await
            .map_err(|e| format!("GitHub API error: {}", e))?;

        let mut sha_map = HashMap::new();
        for file in github_files {
            sha_map.insert(file.name, file.sha);
        }

        Ok(sha_map)
    }

    /// Calculate SHA1 hash of local file
    fn calculate_file_sha1(&self, file_path: &Path) -> Result<String, String> {
        let content = fs::read(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

        let mut hasher = Sha1::new();
        hasher.update(&content);
        let result = hasher.finalize();

        Ok(format!("{:x}", result))
    }

    /// Fetch updated file from GitHub and save to local filesystem
    async fn fetch_and_update_file(
        &self,
        local_path: &Path,
        file_name: &str,
    ) -> Result<String, String> {
        // Construct GitHub download URL from env vars (no hardcoded fallbacks)
        let owner = std::env::var("GITHUB_OWNER")
            .map_err(|_| "GITHUB_OWNER not set in .env".to_string())?;
        let repo =
            std::env::var("GITHUB_REPO").map_err(|_| "GITHUB_REPO not set in .env".to_string())?;
        let branch = std::env::var("GITHUB_BRANCH")
            .map_err(|_| "GITHUB_BRANCH not set in .env".to_string())?;
        let base_path = std::env::var("GITHUB_BASE_PATH")
            .map_err(|_| "GITHUB_BASE_PATH not set in .env".to_string())?;
        let download_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}/{}",
            owner, repo, branch, base_path, file_name
        );

        // Fetch content from GitHub
        let content = self
            .content_api
            .fetch_file_content(&download_url)
            .await
            .map_err(|e| format!("Failed to fetch from GitHub: {}", e))?;

        // Write updated content to local file
        fs::write(local_path, &content)
            .map_err(|e| format!("Failed to write local file: {}", e))?;

        info!("Updated local file: {:?}", local_path);
        Ok(content)
    }

    /// Process file content with ontology-aware filtering and caching
    async fn process_file_content(
        &self,
        file_name: &str,
        content: &str,
        content_sha: &str,
        nodes: &mut HashMap<u32, visionclaw_domain::models::node::Node>,
        edges: &mut HashMap<String, visionclaw_domain::models::edge::Edge>,
        public_pages: &mut std::collections::HashSet<String>,
        stats: &mut SyncStatistics,
    ) -> Result<(), String> {
        // Check cache first
        if let Some(cached) = self.ontology_cache.get(file_name, content_sha).await {
            stats.cache_hits += 1;

            // Use cached analysis
            let _analysis = &cached.analysis;
            let metadata = &cached.metadata;

            // Update statistics from cache
            match metadata.priority {
                OntologyPriority::Priority1 => stats.priority1_files += 1,
                OntologyPriority::Priority2 => stats.priority2_files += 1,
                OntologyPriority::Priority3 => stats.priority3_files += 1,
                OntologyPriority::None => {}
            }

            stats.total_classes += metadata.class_count;
            stats.total_properties += metadata.property_count;
            stats.total_relationships += metadata.relationship_count;

            debug!(
                "Cache hit for {}: priority={:?}",
                file_name, metadata.priority
            );
        } else {
            stats.cache_misses += 1;

            // Analyze content
            let analysis = self.content_analyzer.analyze_content(content, file_name);

            // Create ontology metadata
            let mut metadata = OntologyFileMetadata {
                name: file_name.to_string(),
                path: format!("pages/{}", file_name),
                sha: content_sha.to_string(),
                size: content.len() as u64,
                download_url: String::new(),
                git_commit_date: None,
                has_public_flag: analysis.has_public_flag,
                has_ontology_block: analysis.has_ontology_block,
                priority: OntologyPriority::None,
                source_domain: analysis.source_domain.clone(),
                topics: analysis.topics.clone(),
                relationship_count: analysis.relationship_count,
                class_count: analysis.class_count,
                property_count: analysis.property_count,
            };

            metadata.calculate_priority();

            // Update statistics
            match metadata.priority {
                OntologyPriority::Priority1 => stats.priority1_files += 1,
                OntologyPriority::Priority2 => stats.priority2_files += 1,
                OntologyPriority::Priority3 => stats.priority3_files += 1,
                OntologyPriority::None => {}
            }

            stats.total_classes += metadata.class_count;
            stats.total_properties += metadata.property_count;
            stats.total_relationships += metadata.relationship_count;

            // Cache the analysis
            let cached_entry = CachedOntologyFile::new(
                metadata.clone(),
                analysis.clone(),
                content_sha.to_string(),
            );
            self.ontology_cache
                .put(file_name.to_string(), cached_entry)
                .await;

            debug!(
                "Analyzed {}: priority={:?}, domain={:?}",
                file_name, metadata.priority, metadata.source_domain
            );
        }

        // Process based on content type (Priority 1, 2, or 3)
        // Priority 1 & 3: Knowledge graph files (public:: true)
        if content.lines().take(20).any(|line| {
            let trimmed = line.trim().to_lowercase();
            trimmed == "public:: true" || trimmed == "public::true"
        }) {
            let mut parsed = self
                .kg_parser
                .parse(content, file_name)
                .map_err(|e| format!("Parse error: {}", e))?;

            // Enrich with ontology
            match self
                .enrichment_service
                .enrich_graph(&mut parsed, file_name, content)
                .await
            {
                Ok((nodes_enriched, edges_enriched)) => {
                    debug!(
                        "Enriched {}: {} nodes, {} edges",
                        file_name, nodes_enriched, edges_enriched
                    );
                }
                Err(e) => {
                    warn!("Failed to enrich {}: {}", file_name, e);
                }
            }

            // Add to collections
            let page_name = file_name.trim_end_matches(".md");
            public_pages.insert(page_name.to_string());

            for node in parsed.nodes {
                nodes.insert(node.id, node);
            }

            for edge in parsed.edges {
                edges.insert(edge.id.clone(), edge);
            }

            stats.kg_files_processed += 1;
        }

        // Priority 1 & 2: Ontology files (OntologyBlock)
        if content.contains("### OntologyBlock") {
            match self.onto_parser.parse(content, file_name) {
                Ok(onto_data) => {
                    info!(
                        "🦉 Extracted ontology from {}: {} classes, {} properties",
                        file_name,
                        onto_data.classes.len(),
                        onto_data.properties.len()
                    );

                    // Persist the parsed ontology to the Oxigraph store (ADR-11).
                    // OxigraphOntologyRepository::save_ontology writes classes,
                    // properties and axioms via SPARQL INSERT, so local-file
                    // OntologyBlock extraction reaches the OWL store on the same
                    // path as the GitHub sync instead of being dropped out of scope.
                    match self
                        .onto_repo
                        .save_ontology(&onto_data.classes, &onto_data.properties, &onto_data.axioms)
                        .await
                    {
                        Ok(()) => {
                            debug!(
                                "Persisted ontology from {} to Oxigraph: {} classes, {} properties, {} axioms",
                                file_name,
                                onto_data.classes.len(),
                                onto_data.properties.len(),
                                onto_data.axioms.len()
                            );
                            stats.ontology_files_processed += 1;
                        }
                        Err(e) => {
                            warn!(
                                "Failed to persist ontology from {} to Oxigraph: {}",
                                file_name, e
                            );
                            stats
                                .errors
                                .push(format!("ontology persist {}: {}", file_name, e));
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse ontology from {}: {}", file_name, e);
                }
            }
        }

        // Skip files with no special markers
        if !content.contains("public::") && !content.contains("### OntologyBlock") {
            stats.skipped_files += 1;
        }

        Ok(())
    }

    /// Fetch GitHub commit dates for ontology files (Priority 1 and 2)
    /// This can be called separately to enrich metadata with git history
    pub async fn enrich_with_commit_dates(&self) -> Result<usize, String> {
        info!("🕐 Enriching ontology files with GitHub commit dates...");

        let cached_files = self.ontology_cache.get_by_priority().await;
        let mut enriched_count = 0;

        for (file_path, mut cached_entry) in cached_files {
            // Only enrich Priority 1 and Priority 2 files
            if cached_entry.metadata.priority == OntologyPriority::Priority1
                || cached_entry.metadata.priority == OntologyPriority::Priority2
            {
                // Skip if already has commit date
                if cached_entry.metadata.git_commit_date.is_some() {
                    continue;
                }

                // Extract just the filename from the full path
                let file_name = file_path.split('/').last().unwrap_or(&file_path);

                // Fetch commit date from GitHub API
                match self
                    .content_api
                    .get_file_content_last_modified(file_name, true)
                    .await
                {
                    Ok(commit_date) => {
                        cached_entry.metadata.git_commit_date = Some(commit_date);
                        enriched_count += 1;

                        // Update cache with enriched metadata
                        self.ontology_cache
                            .put(file_path.clone(), cached_entry.clone())
                            .await;

                        debug!("Enriched {} with commit date: {}", file_name, commit_date);
                    }
                    Err(e) => {
                        warn!("Failed to get commit date for {}: {}", file_name, e);
                    }
                }

                // Rate limiting: sleep briefly between requests
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        info!(
            "Enriched {} ontology files with commit dates",
            enriched_count
        );
        Ok(enriched_count)
    }

    /// Get ontology files by priority for selective processing
    pub async fn get_ontology_files_by_priority(
        &self,
        priority: OntologyPriority,
    ) -> Vec<(String, CachedOntologyFile)> {
        let all_files = self.ontology_cache.get_by_priority().await;

        all_files
            .into_iter()
            .filter(|(_, entry)| entry.metadata.priority == priority)
            .collect()
    }

    /// Get cache statistics for monitoring
    pub async fn get_cache_statistics(
        &self,
    ) -> crate::services::ontology_file_cache::OntologyCacheStats {
        self.ontology_cache.get_stats().await
    }

    /// Clear ontology cache (useful for testing or forcing re-analysis)
    pub async fn clear_cache(&self) {
        self.ontology_cache.clear().await;
        info!("🗑️  Ontology cache cleared");
    }

    /// Save batch to Oxigraph store (ADR-11)
    async fn save_batch(
        &self,
        nodes: &HashMap<u32, visionclaw_domain::models::node::Node>,
        edges: &HashMap<String, visionclaw_domain::models::edge::Edge>,
    ) -> Result<(), String> {
        if nodes.is_empty() && edges.is_empty() {
            return Ok(());
        }

        let graph = visionclaw_domain::models::graph::GraphData {
            nodes: nodes.values().cloned().collect(),
            edges: edges.values().cloned().collect(),
            metadata: visionclaw_domain::models::metadata::MetadataStore::new(),
            id_to_metadata: std::collections::HashMap::new(),
        };

        self.kg_repo
            .save_graph(&graph)
            .await
            .map_err(|e| format!("Failed to save graph: {}", e))?;

        Ok(())
    }
}
