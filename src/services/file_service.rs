use crate::models::metadata::{Metadata, MetadataStore, MetadataOps};
use crate::models::graph::GraphData;
use crate::config::AppFullSettings; // Use AppFullSettings, ClientFacingSettings removed
use serde::{Deserialize, Serialize};
use log::{info, debug, error};
use std::sync::atomic::{AtomicU32, Ordering};
use regex::Regex;
use std::fs;
use std::path::Path;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::error::Error as StdError;
use std::time::Duration;
use tokio::time::sleep;
use actix_web::web;
use std::collections::HashMap;
use std::fs::File;
use std::io::Error;
use super::github::{GitHubClient, ContentAPI, GitHubConfig};

// Constants
const METADATA_PATH: &str = "/app/data/metadata/metadata.json";
pub const MARKDOWN_DIR: &str = "/app/data/markdown";
const GITHUB_API_DELAY: Duration = Duration::from_millis(500);

#[derive(Serialize, Deserialize, Clone)]
pub struct ProcessedFile {
    pub file_name: String,
    pub content: String,
    pub is_public: bool,
    pub metadata: Metadata,
}

pub struct FileService {
    _settings: Arc<RwLock<AppFullSettings>>, // Changed to AppFullSettings, prefixed with underscore
    // Counter for assigning node IDs, initialized based on existing metadata
    node_id_counter: AtomicU32,
}

impl FileService {
    pub fn new(_settings: Arc<RwLock<AppFullSettings>>) -> Self { // Changed to AppFullSettings, parameter prefixed
        // Initialize with a default counter
        let service = Self {
            _settings, // Prefixed with underscore
            node_id_counter: AtomicU32::new(1),
        };
        
        // Try to initialize the counter based on existing metadata
        if let Ok(metadata) = Self::load_or_create_metadata() {
            let max_id = metadata.get_max_node_id();
            if max_id > 0 {
                // Start from the next ID after the maximum
                service.node_id_counter.store(max_id + 1, Ordering::SeqCst);
                info!("Initialized node ID counter to {} based on existing metadata", max_id + 1);
            }
        }
        
        service
    }
    
    /// Get the next unique node ID
    fn get_next_node_id(&self) -> u32 {
        self.node_id_counter.fetch_add(1, Ordering::SeqCst)
    }
    
    /// Update node IDs for processed files
    fn update_node_ids(&self, processed_files: &mut Vec<ProcessedFile>) {
        for processed_file in processed_files {
            if processed_file.metadata.node_id == "0" {
                processed_file.metadata.node_id = self.get_next_node_id().to_string();
            }
        }
    }

    /// Process uploaded file and return graph data
    pub async fn process_file_upload(&self, payload: web::Bytes) -> Result<GraphData, Error> {
        let content = String::from_utf8(payload.to_vec())
            .map_err(|e| Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        let metadata = Self::load_or_create_metadata()
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e))?;
        let mut graph_data = GraphData::new();
        
        // Create a temporary file to process
        let temp_filename = format!("temp_{}.md", Utc::now().timestamp());
        let temp_path = format!("{}/{}", MARKDOWN_DIR, temp_filename);
        if let Err(e) = fs::write(&temp_path, &content) {
            return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
        }

        // Extract references and create metadata
        let valid_nodes: Vec<String> = metadata.keys()
            .map(|name| name.trim_end_matches(".md").to_string())
            .collect();

        let references = Self::extract_references(&content, &valid_nodes);
        let topic_counts = Self::convert_references_to_topic_counts(references);

        // Create metadata for the uploaded file
        let file_size = content.len();
        let node_size = Self::calculate_node_size(file_size);
        let file_metadata = Metadata {
            file_name: temp_filename.clone(),
            file_size,
            node_size,
            node_id: "0".to_string(),
            hyperlink_count: Self::count_hyperlinks(&content),
            sha1: Self::calculate_sha1(&content),
            last_modified: Utc::now(),
            last_content_change: Some(Utc::now()), // Uploaded files have content change same as upload time
            last_commit: Some(Utc::now()),
            change_count: Some(1), // First upload counts as 1 change
            file_blob_sha: None, // No GitHub blob SHA for uploaded files
            perplexity_link: String::new(),
            last_perplexity_process: None,
            topic_counts,
        };

        // Assign a unique node ID
        let mut file_metadata = file_metadata;
        file_metadata.node_id = self.get_next_node_id().to_string();

        // Update graph data
        graph_data.metadata.insert(temp_filename.clone(), file_metadata);

        // Clean up temporary file
        if let Err(e) = fs::remove_file(&temp_path) {
            error!("Failed to remove temporary file: {}", e);
        }

        Ok(graph_data)
    }

    /// List available files
    pub async fn list_files(&self) -> Result<Vec<String>, Error> {
        let metadata = Self::load_or_create_metadata()
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e))?;
        Ok(metadata.keys().cloned().collect())
    }

    /// Load a specific file and return graph data
    pub async fn load_file(&self, filename: &str) -> Result<GraphData, Error> {
        let file_path = format!("{}/{}", MARKDOWN_DIR, filename);
        if !Path::new(&file_path).exists() {
            return Err(Error::new(std::io::ErrorKind::NotFound, format!("File not found: {}", filename)));
        }

        let content = fs::read_to_string(&file_path)
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let metadata = Self::load_or_create_metadata()
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e))?;
        let mut graph_data = GraphData::new();

        // Extract references and update metadata
        let valid_nodes: Vec<String> = metadata.keys()
            .map(|name| name.trim_end_matches(".md").to_string())
            .collect();

        let references = Self::extract_references(&content, &valid_nodes);
        let topic_counts = Self::convert_references_to_topic_counts(references);

        // Update or create metadata for the file
        let file_size = content.len();
        let node_size = Self::calculate_node_size(file_size);
        let file_metadata = Metadata {
            file_name: filename.to_string(),
            file_size,
            node_size,
            node_id: "0".to_string(),
            hyperlink_count: Self::count_hyperlinks(&content),
            sha1: Self::calculate_sha1(&content),
            last_modified: Utc::now(),
            last_content_change: Some(Utc::now()),
            last_commit: Some(Utc::now()),
            change_count: None,
            file_blob_sha: None,
            perplexity_link: String::new(),
            last_perplexity_process: None,
            topic_counts,
        };

        // Assign a unique node ID
        let mut file_metadata = file_metadata;
        file_metadata.node_id = self.get_next_node_id().to_string();

        // Update graph data
        graph_data.metadata.insert(filename.to_string(), file_metadata);
        
        Ok(graph_data)
    }

    /// Load metadata from file or create new if not exists
    pub fn load_or_create_metadata() -> Result<MetadataStore, String> {
        // Ensure metadata directory exists
        std::fs::create_dir_all("/app/data/metadata")
            .map_err(|e| format!("Failed to create metadata directory: {}", e))?;
        
        let metadata_path = "/app/data/metadata/metadata.json";
        
        if let Ok(file) = File::open(metadata_path) {
            info!("Loading existing metadata from {}", metadata_path);
            serde_json::from_reader(file)
                .map_err(|e| format!("Failed to parse metadata: {}", e))
        } else {
            info!("Creating new metadata file at {}", metadata_path);
            let empty_store = MetadataStore::default();
            let file = File::create(metadata_path)
                .map_err(|e| format!("Failed to create metadata file: {}", e))?;
                
            serde_json::to_writer_pretty(file, &empty_store)
                .map_err(|e| format!("Failed to write metadata: {}", e))?;
                
            // Verify file was created with correct permissions
            let metadata = std::fs::metadata(metadata_path)
                .map_err(|e| format!("Failed to verify metadata file: {}", e))?;
            
            if !metadata.is_file() {
                return Err("Metadata file was not created properly".to_string());
            }
            
            Ok(empty_store)
        }
    }

    /// Calculate node size based on file size
    fn calculate_node_size(file_size: usize) -> f64 {
        const BASE_SIZE: f64 = 1000.0; // Base file size for scaling
        const MIN_SIZE: f64 = 5.0;  // Minimum node size
        const MAX_SIZE: f64 = 50.0; // Maximum node size

        let size = (file_size as f64 / BASE_SIZE).min(5.0);
        MIN_SIZE + (size * (MAX_SIZE - MIN_SIZE) / 5.0)
    }

    /// Extract references to other files based on their names (case insensitive)
    fn extract_references(content: &str, valid_nodes: &[String]) -> Vec<String> {
        let mut references = Vec::new();
        let content_lower = content.to_lowercase();
        
        for node_name in valid_nodes {
            let node_name_lower = node_name.to_lowercase();
            
            // Create a regex pattern with word boundaries
            let pattern = format!(r"\b{}\b", regex::escape(&node_name_lower));
            if let Ok(re) = Regex::new(&pattern) {
                // Count case-insensitive matches of the filename
                let count = re.find_iter(&content_lower).count();
                
                // If we found any references, add them to the map
                if count > 0 {
                    debug!("Found {} references to {} in content", count, node_name);
                    // Add the reference multiple times based on count
                    for _ in 0..count {
                        references.push(node_name.clone());
                    }
                }
            }
        }
        
        references
    }

    fn convert_references_to_topic_counts(references: Vec<String>) -> HashMap<String, usize> {
        let mut topic_counts = HashMap::new();
        for reference in references {
            *topic_counts.entry(reference).or_insert(0) += 1;
        }
        topic_counts
    }

    /// Initialize local storage with files from GitHub
    pub async fn initialize_local_storage(
        settings: Arc<RwLock<AppFullSettings>>, // Changed to AppFullSettings
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        // Create GitHub client using environment variables
        let github_config = GitHubConfig::from_env()
            .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;
            
        let github = GitHubClient::new(github_config, Arc::clone(&settings)).await?;
        let content_api = ContentAPI::new(Arc::new(github));

        // Check if we already have a valid local setup
        if Self::has_valid_local_setup() {
            info!("Valid local setup found, skipping initialization");
            return Ok(());
        }

        info!("Initializing local storage with files from GitHub");

        // Ensure directories exist and have proper permissions
        Self::ensure_directories()?;

        // Get all markdown files from GitHub
        let github_files = content_api.list_markdown_files("").await?;
        info!("Found {} markdown files in GitHub", github_files.len());

        let mut metadata_store = MetadataStore::new();

        // Process files in batches to prevent timeouts
        const BATCH_SIZE: usize = 5;
        for chunk in github_files.chunks(BATCH_SIZE) {
            let mut futures = Vec::new();
            
            for file_meta in chunk {
                let file_meta = file_meta.clone();
                let content_api = content_api.clone();
                
                futures.push(async move {
                    // First check if file is public
                    match content_api.check_file_public(&file_meta.download_url).await {
                        Ok(is_public) => {
                            if !is_public {
                                debug!("Skipping non-public file: {}", file_meta.name);
                                return Ok(None);
                            }

                            // Only fetch full content for public files
                            match content_api.fetch_file_content(&file_meta.download_url).await {
                                Ok(content) => {
                                    let file_path = format!("{}/{}", MARKDOWN_DIR, file_meta.name);
                                    if let Err(e) = fs::write(&file_path, &content) {
                                        error!("Failed to write file {}: {}", file_path, e);
                                        return Err(e.into());
                                    }

                                    Ok(Some((file_meta, content)))
                                }
                                Err(e) => {
                                    error!("Failed to fetch content for {}: {}", file_meta.name, e);
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to check public status for {}: {}", file_meta.name, e);
                            Err(e)
                        }
                    }
                });
            }

            // Wait for batch to complete
            let results = futures::future::join_all(futures).await;
            
            for result in results {
                match result {
                    Ok(Some((file_meta, content))) => {
                        let _node_name = file_meta.name.trim_end_matches(".md").to_string();
                        let file_size = content.len();
                        let node_size = Self::calculate_node_size(file_size);

                        // Create metadata entry
                        let metadata = Metadata {
                            file_name: file_meta.name.clone(),
                            file_size,
                            node_size,
                            node_id: "0".to_string(), // Will be assigned properly later
                            hyperlink_count: Self::count_hyperlinks(&content),
                            sha1: Self::calculate_sha1(&content),
                            last_modified: file_meta.last_modified.unwrap_or_else(|| Utc::now()),
                            last_content_change: file_meta.last_content_change,
                            last_commit: file_meta.last_modified,
                            change_count: None, // Could be populated with API calls to get commit count
                            file_blob_sha: file_meta.file_blob_sha.clone(),
                            perplexity_link: String::new(),
                            last_perplexity_process: None,
                            topic_counts: HashMap::new(), // Will be updated later
                        };

                        metadata_store.insert(file_meta.name, metadata);
                    }
                    Ok(None) => continue, // Skipped non-public file
                    Err(e) => {
                        error!("Failed to process file in batch: {}", e);
                    }
                }
            }

            sleep(GITHUB_API_DELAY).await;
        }

        // Update topic counts after all files are processed
        Self::update_topic_counts(&mut metadata_store)?;

        // Save metadata
        info!("Saving metadata for {} public files", metadata_store.len());
        Self::save_metadata(&metadata_store)?;

        info!("Initialization complete. Processed {} public files", metadata_store.len());
        Ok(())
    }

    /// Update topic counts for all files
    fn update_topic_counts(metadata_store: &mut MetadataStore) -> Result<(), Error> {
        let valid_nodes: Vec<String> = metadata_store.keys()
            .map(|name| name.trim_end_matches(".md").to_string())
            .collect();

        for file_name in metadata_store.keys().cloned().collect::<Vec<_>>() {
            let file_path = format!("{}/{}", MARKDOWN_DIR, file_name);
            if let Ok(content) = fs::read_to_string(&file_path) {
                let references = Self::extract_references(&content, &valid_nodes);
                let topic_counts = Self::convert_references_to_topic_counts(references);
                
                if let Some(metadata) = metadata_store.get_mut(&file_name) {
                    metadata.topic_counts = topic_counts;
                }
            }
        }

        Ok(())
    }

    /// Check if we have a valid local setup
    fn has_valid_local_setup() -> bool {
        if let Ok(metadata_content) = fs::read_to_string(METADATA_PATH) {
            if metadata_content.trim().is_empty() {
                return false;
            }
            
            if let Ok(metadata) = serde_json::from_str::<MetadataStore>(&metadata_content) {
                return metadata.validate_files(MARKDOWN_DIR);
            }
        }
        false
    }

    /// Ensures all required directories exist with proper permissions
    fn ensure_directories() -> Result<(), Error> {
        // Create markdown directory
        let markdown_dir = Path::new(MARKDOWN_DIR);
        if !markdown_dir.exists() {
            info!("Creating markdown directory at {:?}", markdown_dir);
            fs::create_dir_all(markdown_dir)
                .map_err(|e| Error::new(std::io::ErrorKind::Other, format!("Failed to create markdown directory: {}", e)))?;
            // Set permissions to allow writing
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(markdown_dir, fs::Permissions::from_mode(0o777))
                    .map_err(|e| Error::new(std::io::ErrorKind::Other, format!("Failed to set markdown directory permissions: {}", e)))?;
            }
        }

        // Create metadata directory if it doesn't exist
        let metadata_dir = Path::new(METADATA_PATH).parent().unwrap();
        if !metadata_dir.exists() {
            info!("Creating metadata directory at {:?}", metadata_dir);
            fs::create_dir_all(metadata_dir)
                .map_err(|e| Error::new(std::io::ErrorKind::Other, format!("Failed to create metadata directory: {}", e)))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(metadata_dir, fs::Permissions::from_mode(0o777))
                    .map_err(|e| Error::new(std::io::ErrorKind::Other, format!("Failed to set metadata directory permissions: {}", e)))?;
            }
        }

        // Verify permissions by attempting to create a test file
        let test_file = format!("{}/test_permissions", MARKDOWN_DIR);
        match fs::write(&test_file, "test") {
            Ok(_) => {
                info!("Successfully wrote test file to {}", test_file);
                fs::remove_file(&test_file)
                    .map_err(|e| Error::new(std::io::ErrorKind::Other, format!("Failed to remove test file: {}", e)))?;
                info!("Successfully removed test file");
                info!("Directory permissions verified");
                Ok(())
            },
            Err(e) => {
                error!("Failed to verify directory permissions: {}", e);
                if let Ok(current_dir) = std::env::current_dir() {
                    error!("Current directory: {:?}", current_dir);
                }
                if let Ok(dir_contents) = fs::read_dir(MARKDOWN_DIR) {
                    error!("Directory contents: {:?}", dir_contents);
                }
                Err(Error::new(std::io::ErrorKind::PermissionDenied, format!("Failed to verify directory permissions: {}", e)))
            }
        }
    }

    /// Save metadata to file
    pub fn save_metadata(metadata: &MetadataStore) -> Result<(), Error> {
        let json = serde_json::to_string_pretty(metadata)
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        fs::write(METADATA_PATH, json)
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(())
    }

    /// Calculate SHA1 hash of content
    fn calculate_sha1(content: &str) -> String {
        use sha1::{Sha1, Digest};
        let mut hasher = Sha1::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Count hyperlinks in content
    fn count_hyperlinks(content: &str) -> usize {
        let re = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
        re.find_iter(content).count()
    }

    /// Fetch and process files from GitHub
    pub async fn fetch_and_process_files(
        &self,
        content_api: Arc<ContentAPI>,
        _settings: Arc<RwLock<AppFullSettings>>, // Changed to AppFullSettings (though unused)
        metadata_store: &mut MetadataStore,
    ) -> Result<Vec<ProcessedFile>, Box<dyn StdError + Send + Sync>> {
        let mut processed_files = Vec::new();

        // Get all markdown files from GitHub
        let github_files = content_api.list_markdown_files("").await?;
        info!("Found {} markdown files in GitHub", github_files.len());

        // Process files in batches to prevent timeouts
        const BATCH_SIZE: usize = 5;
        for chunk in github_files.chunks(BATCH_SIZE) {
            let mut futures = Vec::new();
            
            for file_meta in chunk {
                let file_meta = file_meta.clone();
                let content_api = content_api.clone();
                
                futures.push(async move {
                    // First check if file is public
                    match content_api.check_file_public(&file_meta.download_url).await {
                        Ok(is_public) => {
                            if !is_public {
                                debug!("Skipping non-public file: {}", file_meta.name);
                                return Ok(None);
                            }

                            // Only fetch full content for public files
                            match content_api.fetch_file_content(&file_meta.download_url).await {
                                Ok(content) => {
                                    let file_path = format!("{}/{}", MARKDOWN_DIR, file_meta.name);
                                    if let Err(e) = fs::write(&file_path, &content) {
                                        error!("Failed to write file {}: {}", file_path, e);
                                        return Err(e.into());
                                    }

                                    let file_size = content.len();
                                    let node_size = Self::calculate_node_size(file_size);

                                    let metadata = Metadata {
                                        file_name: file_meta.name.clone(),
                                        file_size,
                                        node_size,
                                        node_id: "0".to_string(), // Will be assigned properly later
                                        hyperlink_count: Self::count_hyperlinks(&content),
                                        sha1: Self::calculate_sha1(&content),
                                        last_modified: file_meta.last_modified.unwrap_or_else(|| Utc::now()),
                                        last_content_change: file_meta.last_content_change,
                                        last_commit: file_meta.last_modified,
                                        change_count: None, // Could be populated with API calls to get commit count
                                        file_blob_sha: file_meta.file_blob_sha.clone(),
                                        perplexity_link: String::new(),
                                        last_perplexity_process: None,
                                        topic_counts: HashMap::new(), // Will be updated later
                                    };

                                    Ok(Some(ProcessedFile {
                                        file_name: file_meta.name.clone(),
                                        content,
                                        is_public: true,
                                        metadata,
                                    }))
                                }
                                Err(e) => {
                                    error!("Failed to fetch content for {}: {}", file_meta.name, e);
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to check public status for {}: {}", file_meta.name, e);
                            Err(e)
                        }
                    }
                });
            }

            // Wait for batch to complete
            let results = futures::future::join_all(futures).await;
            
            for result in results {
                match result {
                    Ok(Some(processed_file)) => {
                        processed_files.push(processed_file);
                    }
                    Ok(None) => continue, // Skipped non-public file
                    Err(e) => {
                        error!("Failed to process file in batch: {}", e);
                    }
                }
            }

            sleep(GITHUB_API_DELAY).await;
        }

        // Assign node IDs to any new files
        self.update_node_ids(&mut processed_files);

        // Update topic counts after all files are processed
        Self::update_topic_counts(metadata_store)?;

        Ok(processed_files)
    }
}