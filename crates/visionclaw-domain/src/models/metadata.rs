use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub last_content_change: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_commit: Option<DateTime<Utc>>,
    #[serde(default)]
    pub change_count: Option<u32>,
    #[serde(default)]
    pub file_blob_sha: Option<String>,
    #[serde(default)]
    pub perplexity_link: String,
    #[serde(default)]
    pub last_perplexity_process: Option<DateTime<Utc>>,
    #[serde(default)]
    pub topic_counts: HashMap<String, usize>,
    // Ontology fields from new header format
    #[serde(default)]
    pub term_id: Option<String>,
    #[serde(default)]
    pub preferred_term: Option<String>,
    #[serde(default)]
    pub source_domain: Option<String>,
    #[serde(default)]
    pub ontology_status: Option<String>,
    #[serde(default)]
    pub owl_class: Option<String>,
    #[serde(default)]
    pub owl_physicality: Option<String>,
    #[serde(default)]
    pub owl_role: Option<String>,
    #[serde(default)]
    pub quality_score: Option<f64>,
    #[serde(default)]
    pub authority_score: Option<f64>,
    #[serde(default)]
    pub belongs_to_domain: Vec<String>,
    #[serde(default)]
    pub maturity: Option<String>,
    #[serde(default)]
    pub is_subclass_of: Vec<String>,
    #[serde(default)]
    pub definition: Option<String>,
}

// Default function for node_id to ensure backward compatibility
fn default_node_id() -> String {
    "0".to_string()
}

pub type MetadataStore = HashMap<String, Metadata>;

pub type FileMetadata = Metadata;

// Implement helper methods directly on HashMap<String, Metadata>
pub trait MetadataOps {
    fn validate_files(&self, markdown_dir: &str) -> bool;
    fn get_max_node_id(&self) -> u32;
}

impl MetadataOps for MetadataStore {
    fn get_max_node_id(&self) -> u32 {
        self.values()
            .map(|m| m.node_id.parse::<u32>().unwrap_or(0))
            .max()
            .unwrap_or(0)
    }

    fn validate_files(&self, markdown_dir: &str) -> bool {
        if self.is_empty() {
            return false;
        }

        for filename in self.keys() {
            let file_path = format!("{}/{}", markdown_dir, filename);
            if !std::path::Path::new(&file_path).exists() {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metadata(node_id: &str, file_name: &str) -> Metadata {
        Metadata {
            node_id: node_id.to_string(),
            file_name: file_name.to_string(),
            ..Metadata::default()
        }
    }

    #[test]
    fn metadata_default_has_sensible_values() {
        let m = Metadata::default();
        // Default::default() on a #[derive(Default)] String produces "".
        // The serde default ("0") only applies during JSON deserialization.
        assert_eq!(m.file_name, "");
        assert_eq!(m.file_size, 0);
        assert!(m.sha1.is_empty());
        // node_id default via derive is ""; only becomes "0" on deserialization
        assert_eq!(m.node_id, "");
    }

    #[test]
    fn metadata_serde_roundtrip() {
        let m = Metadata {
            file_name: "test.md".to_string(),
            file_size: 1024,
            node_id: "42".to_string(),
            sha1: "abc123".to_string(),
            ..Metadata::default()
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: Metadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.file_name, "test.md");
        assert_eq!(back.file_size, 1024);
        assert_eq!(back.node_id, "42");
        assert_eq!(back.sha1, "abc123");
    }

    #[test]
    fn metadata_serde_uses_camel_case() {
        let m = Metadata {
            hyperlink_count: 5,
            ..Metadata::default()
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(
            json.contains("hyperlinkCount"),
            "expected camelCase key, got: {}",
            json
        );
    }

    #[test]
    fn metadata_store_get_max_node_id_empty_returns_zero() {
        let store: MetadataStore = MetadataStore::new();
        assert_eq!(store.get_max_node_id(), 0);
    }

    #[test]
    fn metadata_store_get_max_node_id_finds_maximum() {
        let mut store = MetadataStore::new();
        store.insert("a.md".to_string(), make_metadata("10", "a.md"));
        store.insert("b.md".to_string(), make_metadata("3", "b.md"));
        store.insert("c.md".to_string(), make_metadata("100", "c.md"));
        assert_eq!(store.get_max_node_id(), 100);
    }

    #[test]
    fn metadata_store_get_max_node_id_ignores_non_numeric() {
        let mut store = MetadataStore::new();
        store.insert("a.md".to_string(), make_metadata("not_a_number", "a.md"));
        store.insert("b.md".to_string(), make_metadata("7", "b.md"));
        assert_eq!(store.get_max_node_id(), 7);
    }

    #[test]
    fn metadata_store_validate_files_empty_returns_false() {
        let store = MetadataStore::new();
        assert!(!store.validate_files("/any/path"));
    }

    #[test]
    fn metadata_store_validate_files_missing_file_returns_false() {
        let mut store = MetadataStore::new();
        store.insert(
            "nonexistent.md".to_string(),
            make_metadata("1", "nonexistent.md"),
        );
        assert!(!store.validate_files("/definitely/not/a/real/dir"));
    }
}
