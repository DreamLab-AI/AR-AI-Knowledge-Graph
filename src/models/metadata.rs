use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stores metadata about a processed file.
/// All fields use camelCase serialization for client compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub file_size: usize,
    #[serde(default)]
    pub node_size: f64,
    #[serde(default)]
    pub hyperlink_count: usize,
    #[serde(default)]
    pub sha1: String,
    #[serde(default = "default_node_id")]
    pub node_id: String,
    #[serde(default = "Utc::now")]
    pub last_modified: DateTime<Utc>,
    #[serde(default)]
    pub last_content_change: Option<DateTime<Utc>>,  // When content actually changed
    #[serde(default)]
    pub last_commit: Option<DateTime<Utc>>,  // When file appeared in any commit
    #[serde(default)]
    pub change_count: Option<u32>,  // Number of actual content changes
    #[serde(default)]
    pub file_blob_sha: Option<String>,  // GitHub file blob SHA
    #[serde(default)]
    pub perplexity_link: String,
    #[serde(default)]
    pub last_perplexity_process: Option<DateTime<Utc>>,
    #[serde(default)]
    pub topic_counts: HashMap<String, usize>,
}

// Default function for node_id to ensure backward compatibility
fn default_node_id() -> String {
    // Will be replaced with actual ID during processing
    "0".to_string()
}

/// Type alias for metadata storage with camelCase keys
pub type MetadataStore = HashMap<String, Metadata>;

// Implement helper methods directly on HashMap<String, Metadata>
pub trait MetadataOps {
    fn validate_files(&self, markdown_dir: &str) -> bool;
    fn get_max_node_id(&self) -> u32;
}

impl MetadataOps for MetadataStore {
    fn get_max_node_id(&self) -> u32 {
        // Find the maximum node_id in the metadata store
        self.values()
            .map(|m| m.node_id.parse::<u32>().unwrap_or(0))
            .max()
            .unwrap_or(0)
    }
    
    fn validate_files(&self, markdown_dir: &str) -> bool {
        if self.is_empty() {
            return false;
        }

        // Check if the markdown files referenced in metadata actually exist
        for filename in self.keys() {
            let file_path = format!("{}/{}", markdown_dir, filename);
            if !std::path::Path::new(&file_path).exists() {
                return false;
            }
        }
        
        true
    }
}
