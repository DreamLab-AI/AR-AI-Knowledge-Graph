// src/services/parsers/knowledge_graph_parser.rs
//! Knowledge Graph Parser
//!
//! Parses markdown files marked with `public:: true` to extract:
//! - Nodes (pages, concepts)
//! - Edges (links, relationships)
//! - Metadata (properties, tags)

use crate::utils::socket_flow_messages::BinaryNodeData;
use log::{debug, info};
use std::collections::HashMap;
use visionclaw_domain::models::edge::Edge;
use visionclaw_domain::models::graph::GraphData;
use visionclaw_domain::models::metadata::MetadataStore;
use visionclaw_domain::models::node::Node;

/// Knowledge graph parser with position preservation support
pub struct KnowledgeGraphParser {
    /// Existing positions from database (node_id -> (x, y, z))
    existing_positions: Option<HashMap<u32, (f32, f32, f32)>>,
}

impl KnowledgeGraphParser {
    pub fn new() -> Self {
        Self {
            existing_positions: None,
        }
    }

    /// Create parser with existing positions from database
    /// These positions will be used instead of generating random ones
    pub fn with_positions(existing_positions: HashMap<u32, (f32, f32, f32)>) -> Self {
        Self {
            existing_positions: Some(existing_positions),
        }
    }

    /// Set existing positions for position preservation
    pub fn set_positions(&mut self, positions: HashMap<u32, (f32, f32, f32)>) {
        self.existing_positions = Some(positions);
    }

    /// Get position for a node ID, using existing position or generating random.
    /// Public alias `get_position_public` provided for cross-module callers
    /// that build nodes directly from canonical entities (see ADR-090 Phase B).
    pub fn get_position_public(&self, node_id: u32) -> (f32, f32, f32) {
        self.get_position(node_id)
    }

    /// Get position for a node ID, using existing position or generating random
    fn get_position(&self, node_id: u32) -> (f32, f32, f32) {
        if let Some(ref positions) = self.existing_positions {
            if let Some(&(x, y, z)) = positions.get(&node_id) {
                return (x, y, z);
            }
        }
        // Generate random position only if no existing position found
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (
            rng.gen_range(-100.0..100.0),
            rng.gen_range(-100.0..100.0),
            rng.gen_range(-100.0..100.0),
        )
    }

    pub fn parse(&self, content: &str, filename: &str) -> Result<GraphData, String> {
        info!("Parsing knowledge graph file: {}", filename);

        let page_name = filename.strip_suffix(".md").unwrap_or(filename).to_string();

        let mut nodes = vec![self.create_page_node(&page_name, content)];
        let mut id_to_metadata = HashMap::new();
        id_to_metadata.insert(nodes[0].id.to_string(), page_name.clone());

        // Wikilink edges-only: create Edge objects for [[WikiLinks]] without
        // inflating the node count. Only edges are emitted; target nodes are NOT
        // created here. Edges whose target doesn't exist as a page node will
        // still be stored — the Oxigraph SPARQL INSERT will create stubs or the edge will
        // dangle harmlessly until the target page is synced.
        let wikilink_edges = self.extract_wikilink_edges(content, &nodes[0].id);

        let metadata = self.extract_metadata_store(content);

        debug!(
            "Parsed {}: {} nodes, {} wikilink edges",
            filename,
            nodes.len(),
            wikilink_edges.len(),
        );

        Ok(GraphData {
            nodes,
            edges: wikilink_edges,
            metadata,
            id_to_metadata,
        })
    }

    /// Create a page node, preserving existing position if available
    fn create_page_node(&self, page_name: &str, content: &str) -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "page".to_string());
        metadata.insert("source_file".to_string(), format!("{}.md", page_name));
        metadata.insert("public".to_string(), "true".to_string());

        // Real markdown byte-size, surfaced so the client can size nodes by content volume.
        let content_size = content.len();
        metadata.insert("file_size".to_string(), content_size.to_string());

        let tags = self.extract_tags(content);
        if !tags.is_empty() {
            metadata.insert("tags".to_string(), tags.join(", "));
        }

        // Extract owl:class IRI from logseq-style "owl:class:: prefix:Local" lines.
        // Pages carrying this metadata are reclassified as ontology nodes downstream
        // (graph_state_actor.classify_node treats owl_class_iri.is_some() as ontology).
        let owl_class_iri = Self::extract_owl_class(content);
        let source_domain = Self::extract_source_domain(content);
        if let Some(ref dom) = source_domain {
            metadata.insert("source_domain".to_string(), dom.clone());
        }

        let id = self.page_name_to_id(page_name);

        // Use existing position or generate random (position preservation)
        let (x, y, z) = self.get_position(id);
        let data: visionclaw_domain::BinaryNodeData = BinaryNodeData {
            node_id: id,
            x,
            y,
            z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
        }
        .into();

        // Pages with owl:class metadata are surfaced as ontology nodes so the
        // dual-graph (knowledge ↔ ontology) X-axis separation control has something
        // to separate. Pages without it remain "page" (knowledge population).
        let (node_type, color) = if owl_class_iri.is_some() {
            (
                Some("ontology_node".to_string()),
                Some("#B91C7B".to_string()),
            )
        } else {
            (Some("page".to_string()), Some("#4A90E2".to_string()))
        };

        Node {
            id,
            metadata_id: page_name.to_string(),
            label: page_name.to_string(),
            data,
            metadata,
            file_size: content_size as u64,
            node_type,
            color,
            // Monotonic ln(bytes) mapping: long-tailed file sizes -> small visual range
            // (~0 for tiny, ~9 for 8KB, ~11 for 64KB). No magic multipliers.
            size: Some((content_size as f32).max(1.0).ln()),
            weight: Some(1.0),
            group: None,
            user_data: None,
            mass: Some(1.0),
            x: Some(data.x),
            y: Some(data.y),
            z: Some(data.z),
            vx: Some(0.0),
            vy: Some(0.0),
            vz: Some(0.0),
            owl_class_iri,
        }
    }

    /// Extract `owl:class:: prefix:LocalName` from a logseq markdown ontology block.
    /// Returns the full IRI value (e.g. "mv:ArbitrationDecisionEngine").
    fn extract_owl_class(content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim_start_matches(|c: char| c.is_whitespace() || c == '-');
            if let Some(rest) = trimmed.strip_prefix("owl:class::") {
                let value = rest.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extract `source-domain:: <value>` from logseq markdown front-matter style metadata.
    fn extract_source_domain(content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim_start_matches(|c: char| c.is_whitespace() || c == '-');
            if let Some(rest) = trimmed.strip_prefix("source-domain::") {
                let value = rest.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extract wikilink edges only — no new nodes created.
    /// Returns Edge objects for each [[WikiLink]] found in content.
    /// Deduplicates by target to avoid multiple edges to the same page.
    fn extract_wikilink_edges(&self, content: &str, source_id: &u32) -> Vec<Edge> {
        let mut edges = Vec::new();
        let mut seen_targets = std::collections::HashSet::new();

        let link_pattern =
            regex::Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").expect("Invalid regex pattern");

        for cap in link_pattern.captures_iter(content) {
            if let Some(link_match) = cap.get(1) {
                let target_page = link_match.as_str().trim().to_string();
                let target_id = self.page_name_to_id(&target_page);

                // Skip self-loops and duplicates
                if target_id == *source_id || !seen_targets.insert(target_id) {
                    continue;
                }

                edges.push(Edge {
                    id: format!("{}_{}", source_id, target_id),
                    source: *source_id,
                    target: target_id,
                    weight: 1.0,
                    edge_type: Some("explicit_link".to_string()),
                    metadata: None,
                    owl_property_iri: None,
                });
            }
        }

        edges
    }

    /// Extract links from content, preserving existing positions (legacy — creates nodes)
    #[allow(dead_code)]
    fn extract_links(&self, content: &str, source_id: &u32) -> (Vec<Node>, Vec<Edge>) {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let link_pattern =
            regex::Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").expect("Invalid regex pattern");

        for cap in link_pattern.captures_iter(content) {
            if let Some(link_match) = cap.get(1) {
                let target_page = link_match.as_str().trim().to_string();
                let target_id = self.page_name_to_id(&target_page);

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), "linked_page".to_string());

                // Use existing position or generate random (position preservation)
                let (x, y, z) = self.get_position(target_id);
                let data: visionclaw_domain::BinaryNodeData = BinaryNodeData {
                    node_id: target_id,
                    x,
                    y,
                    z,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                }
                .into();

                nodes.push(Node {
                    id: target_id,
                    metadata_id: target_page.clone(),
                    label: target_page.clone(),
                    data,
                    metadata,
                    file_size: 0,
                    node_type: Some("linked_page".to_string()),
                    color: Some("#7C3AED".to_string()),
                    size: Some(0.8),
                    weight: Some(0.8),
                    group: None,
                    user_data: None,
                    mass: Some(1.0),
                    x: Some(data.x),
                    y: Some(data.y),
                    z: Some(data.z),
                    vx: Some(0.0),
                    vy: Some(0.0),
                    vz: Some(0.0),
                    owl_class_iri: None,
                });

                edges.push(Edge {
                    id: format!("{}_{}", source_id, target_id),
                    source: *source_id,
                    target: target_id,
                    weight: 1.0,
                    edge_type: Some("link".to_string()),
                    metadata: Some(HashMap::new()),
                    owl_property_iri: None,
                });
            }
        }

        (nodes, edges)
    }

    fn extract_metadata_store(&self, content: &str) -> MetadataStore {
        let store = MetadataStore::new();

        let prop_pattern =
            regex::Regex::new(r"([a-zA-Z_]+)::\s*(.+)").expect("Invalid regex pattern");

        let mut properties = HashMap::new();
        for cap in prop_pattern.captures_iter(content) {
            if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                let key_str = key.as_str().to_string();
                let value_str = value.as_str().trim().to_string();

                properties.insert(key_str, value_str);
            }
        }

        store
    }

    fn extract_tags(&self, content: &str) -> Vec<String> {
        let mut tags = Vec::new();

        let tag_pattern = regex::Regex::new(r"#([a-zA-Z0-9_-]+)|tag::\s*#?([a-zA-Z0-9_-]+)")
            .expect("Invalid regex pattern");

        for cap in tag_pattern.captures_iter(content) {
            if let Some(tag) = cap.get(1).or_else(|| cap.get(2)) {
                tags.push(tag.as_str().to_string());
            }
        }

        tags.dedup();
        tags
    }

    pub fn page_name_to_id(&self, page_name: &str) -> u32 {
        // Deterministic, seeded SHA-256 derivation (ADR-100 D2 / PRD-018 WS-0),
        // replacing the previous `DefaultHasher` whose output is explicitly NOT
        // stable across Rust releases and machines — the root cause of the
        // historical node-ID collisions and broken IRI→node resolution.
        //
        // The canonical derivation slugifies internally with the same lower /
        // collapse-non-alnum rule (now diacritic-preserving via NFKD), so
        // "Camera", "camera", and the IRI local-name "camera" (from
        // urn:ngm:class:camera) all resolve to the same id — preserving the
        // cross-graph join the `@type: Page` / `@type: Class` blocks rely on.
        visionclaw_ontology::services::canonical_iri::NodeIdHasher::derive_id(
            &visionclaw_ontology::services::canonical_iri::slugify(page_name),
        )
    }

    /// Canonical slug: lowercase, collapse non-alphanumeric runs to `-`,
    /// strip leading/trailing dashes. Matches `logseq/pipeline/validate.py`.
    pub fn slugify(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut prev_dash = true; // suppresses leading dash
        for c in s.chars() {
            if c.is_ascii_alphanumeric() {
                out.extend(c.to_lowercase());
                prev_dash = false;
            } else if !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        }
        // Strip trailing dash
        if out.ends_with('-') {
            out.pop();
        }
        out
    }
}

impl Default for KnowledgeGraphParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_preservation() {
        let mut positions = HashMap::new();
        positions.insert(12345u32, (10.0f32, 20.0f32, 30.0f32));

        let parser = KnowledgeGraphParser::with_positions(positions);
        let pos = parser.get_position(12345);

        assert_eq!(pos, (10.0, 20.0, 30.0));
    }

    #[test]
    fn test_fallback_to_random() {
        let parser = KnowledgeGraphParser::new();
        let pos = parser.get_position(99999);

        // Should be within random range
        assert!(pos.0 >= -100.0 && pos.0 <= 100.0);
        assert!(pos.1 >= -100.0 && pos.1 <= 100.0);
        assert!(pos.2 >= -100.0 && pos.2 <= 100.0);
    }
}
