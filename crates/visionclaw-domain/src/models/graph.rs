use super::edge::Edge;
use super::metadata::MetadataStore;
use super::node::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<Node>,

    pub edges: Vec<Edge>,

    pub metadata: MetadataStore,

    #[serde(skip)]
    pub id_to_metadata: HashMap<String, String>,
}

impl GraphData {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            metadata: MetadataStore::new(),
            id_to_metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_data_new_is_empty() {
        let g = GraphData::new();
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
        assert!(g.metadata.is_empty());
        assert!(g.id_to_metadata.is_empty());
    }

    #[test]
    fn graph_data_default_equals_new() {
        let g = GraphData::default();
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn graph_data_serde_roundtrip() {
        let mut g = GraphData::new();
        g.nodes
            .push(super::super::node::Node::new("meta-1".to_string()));
        let json = serde_json::to_string(&g).unwrap();
        let back: GraphData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.nodes.len(), 1);
        assert_eq!(back.edges.len(), 0);
    }

    #[test]
    fn graph_data_id_to_metadata_is_skipped_in_serde() {
        let mut g = GraphData::new();
        g.id_to_metadata
            .insert("1".to_string(), "doc.md".to_string());
        let json = serde_json::to_string(&g).unwrap();
        // id_to_metadata has #[serde(skip)] so it must not appear in output
        assert!(!json.contains("id_to_metadata"));
        assert!(!json.contains("idToMetadata"));
    }
}
