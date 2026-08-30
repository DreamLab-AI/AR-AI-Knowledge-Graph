//! Layout types for graph visualization — domain representation.
//!
//! Mirrors `src/layout/types.rs` without specta/validator dependencies.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutMode {
    /// ForceAtlas2 with LinLog (default)
    ForceDirected,
    /// Sugiyama DAG layers
    Hierarchical,
    /// Centrality rings
    Radial,
    /// Graph Laplacian eigenvectors
    Spectral,
    /// Z-axis = timestamp
    Temporal,
    /// ForceAtlas2 + Louvain metanodes
    Clustered,
}

impl Default for LayoutMode {
    fn default() -> Self {
        LayoutMode::ForceDirected
    }
}

impl std::fmt::Display for LayoutMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutMode::ForceDirected => write!(f, "forceDirected"),
            LayoutMode::Hierarchical => write!(f, "hierarchical"),
            LayoutMode::Radial => write!(f, "radial"),
            LayoutMode::Spectral => write!(f, "spectral"),
            LayoutMode::Temporal => write!(f, "temporal"),
            LayoutMode::Clustered => write!(f, "clustered"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutModeConfig {
    pub mode: LayoutMode,
    pub transition_duration_ms: u32,
    // ForceAtlas2 specific
    pub scaling_ratio: f32,
    pub gravity: f32,
    pub lin_log_mode: bool,
    pub dissuade_hubs: bool,
    pub barnes_hut_theta: f32,
    pub strong_gravity: bool,
    // Hierarchical specific
    pub layer_spacing: f32,
    pub node_spacing: f32,
    pub hierarchy_direction: String, // "top_down", "left_right", "radial"
    // Radial specific
    pub centrality_measure: String, // "pagerank", "degree", "betweenness"
    pub ring_count: u32,
    // Zone constraints
    pub zones: Vec<ConstraintZone>,
    // Graph separation
    pub graph_separation_x: f32,
}

impl Default for LayoutModeConfig {
    fn default() -> Self {
        Self {
            mode: LayoutMode::ForceDirected,
            transition_duration_ms: 500,
            scaling_ratio: 10.0,
            gravity: 1.0,
            lin_log_mode: true,
            dissuade_hubs: true,
            barnes_hut_theta: 0.5,
            strong_gravity: false,
            layer_spacing: 150.0,
            node_spacing: 80.0,
            hierarchy_direction: "top_down".to_string(),
            centrality_measure: "pagerank".to_string(),
            ring_count: 8,
            zones: vec![],
            graph_separation_x: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintZone {
    pub id: String,
    pub center: [f32; 3],
    pub radius: f32,
    pub strength: f32,
    pub node_types: Vec<String>, // e.g. ["owl_class", "ontology_node"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutStatus {
    pub current_mode: LayoutMode,
    pub transitioning: bool,
    pub transition_progress: f32,
    pub iterations: u64,
    pub converged: bool,
    pub kinetic_energy: f64,
    pub available_modes: Vec<LayoutMode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(LayoutMode::default(), LayoutMode::ForceDirected);
    }

    #[test]
    fn test_serde_round_trip() {
        let mode = LayoutMode::Hierarchical;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"hierarchical\"");
        let back: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, mode);
    }

    #[test]
    fn test_display() {
        assert_eq!(LayoutMode::ForceDirected.to_string(), "forceDirected");
        assert_eq!(LayoutMode::Clustered.to_string(), "clustered");
    }

    #[test]
    fn test_layout_mode_config_default() {
        let cfg = LayoutModeConfig::default();
        assert_eq!(cfg.mode, LayoutMode::ForceDirected);
        assert_eq!(cfg.transition_duration_ms, 500);
        assert!(cfg.zones.is_empty());
    }
}
