use crate::events::types::DomainEvent;
use crate::utils::json::to_json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Macro to simplify DomainEvent implementations
macro_rules! impl_domain_event {
    ($type:ty, $event_type:expr, $aggregate_type:expr, $id_field:ident) => {
        impl DomainEvent for $type {
            fn event_type(&self) -> &'static str {
                $event_type
            }
            fn aggregate_id(&self) -> &str {
                &self.$id_field
            }
            fn timestamp(&self) -> DateTime<Utc> {
                self.timestamp
            }
            fn aggregate_type(&self) -> &'static str {
                $aggregate_type
            }
            fn to_json_string(&self) -> Result<String, serde_json::Error> {
                to_json(self).map_err(|e| {
                    let msg = format!("JSON serialization error: {}", e);
                    serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::Other, msg))
                })
            }
        }
    };
}

// ==================== Graph Events ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAddedEvent {
    pub node_id: String,
    pub label: String,
    pub node_type: String,
    pub properties: std::collections::HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(NodeAddedEvent, "NodeAdded", "Node", node_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeUpdatedEvent {
    pub node_id: String,
    pub label: Option<String>,
    pub properties: Option<std::collections::HashMap<String, String>>,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(NodeUpdatedEvent, "NodeUpdated", "Node", node_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRemovedEvent {
    pub node_id: String,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(NodeRemovedEvent, "NodeRemoved", "Node", node_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeAddedEvent {
    pub edge_id: String,
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(EdgeAddedEvent, "EdgeAdded", "Edge", edge_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRemovedEvent {
    pub edge_id: String,
    pub source_id: String,
    pub target_id: String,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(EdgeRemovedEvent, "EdgeRemoved", "Edge", edge_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSavedEvent {
    pub graph_id: String,
    pub file_path: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(GraphSavedEvent, "GraphSaved", "Graph", graph_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphClearedEvent {
    pub graph_id: String,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(GraphClearedEvent, "GraphCleared", "Graph", graph_id);

// ==================== Ontology Events ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassAddedEvent {
    pub class_id: String,
    pub class_iri: String,
    pub label: Option<String>,
    pub parent_classes: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(ClassAddedEvent, "ClassAdded", "OntologyClass", class_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyAddedEvent {
    pub property_id: String,
    pub property_iri: String,
    pub property_type: String,
    pub domain: Option<String>,
    pub range: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    PropertyAddedEvent,
    "PropertyAdded",
    "OntologyProperty",
    property_id
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxiomAddedEvent {
    pub axiom_id: String,
    pub axiom_type: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(AxiomAddedEvent, "AxiomAdded", "Axiom", axiom_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyImportedEvent {
    pub ontology_id: String,
    pub file_path: String,
    pub format: String,
    pub class_count: usize,
    pub property_count: usize,
    pub individual_count: usize,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    OntologyImportedEvent,
    "OntologyImported",
    "Ontology",
    ontology_id
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceCompletedEvent {
    pub ontology_id: String,
    pub reasoner_type: String,
    pub inferred_axioms: usize,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    InferenceCompletedEvent,
    "InferenceCompleted",
    "Ontology",
    ontology_id
);

// ==================== Physics Events ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStartedEvent {
    pub simulation_id: String,
    pub physics_profile: String,
    pub node_count: usize,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    SimulationStartedEvent,
    "SimulationStarted",
    "Simulation",
    simulation_id
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStoppedEvent {
    pub simulation_id: String,
    pub iterations: u32,
    pub final_energy: f64,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    SimulationStoppedEvent,
    "SimulationStopped",
    "Simulation",
    simulation_id
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutOptimizedEvent {
    pub layout_id: String,
    pub algorithm: String,
    pub node_count: usize,
    pub optimization_score: f64,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(LayoutOptimizedEvent, "LayoutOptimized", "Layout", layout_id);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionsUpdatedEvent {
    pub graph_id: String,
    pub updated_nodes: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(PositionsUpdatedEvent, "PositionsUpdated", "Graph", graph_id);

// ==================== Settings Events ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingUpdatedEvent {
    pub setting_key: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub category: String,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    SettingUpdatedEvent,
    "SettingUpdated",
    "Setting",
    setting_key
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsProfileSavedEvent {
    pub profile_id: String,
    pub profile_name: String,
    pub parameters: std::collections::HashMap<String, f64>,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    PhysicsProfileSavedEvent,
    "PhysicsProfileSaved",
    "PhysicsProfile",
    profile_id
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsImportedEvent {
    pub settings_id: String,
    pub file_path: String,
    pub imported_count: usize,
    pub timestamp: DateTime<Utc>,
}

impl_domain_event!(
    SettingsImportedEvent,
    "SettingsImported",
    "Settings",
    settings_id
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::time;

    #[test]
    fn test_node_added_event() {
        let event = NodeAddedEvent {
            node_id: "node-1".to_string(),
            label: "Test Node".to_string(),
            node_type: "Person".to_string(),
            properties: std::collections::HashMap::new(),
            timestamp: time::now(),
        };

        assert_eq!(event.event_type(), "NodeAdded");
        assert_eq!(event.aggregate_id(), "node-1");
        assert_eq!(event.aggregate_type(), "Node");
    }

    #[test]
    fn test_ontology_imported_event() {
        let event = OntologyImportedEvent {
            ontology_id: "onto-1".to_string(),
            file_path: "/test.owl".to_string(),
            format: "RDF/XML".to_string(),
            class_count: 100,
            property_count: 50,
            individual_count: 200,
            timestamp: time::now(),
        };

        assert_eq!(event.event_type(), "OntologyImported");
        assert_eq!(event.aggregate_type(), "Ontology");
    }

    #[test]
    fn test_simulation_events() {
        let start = SimulationStartedEvent {
            simulation_id: "sim-1".to_string(),
            physics_profile: "force-directed".to_string(),
            node_count: 100,
            timestamp: time::now(),
        };

        let stop = SimulationStoppedEvent {
            simulation_id: "sim-1".to_string(),
            iterations: 1000,
            final_energy: 0.05,
            timestamp: time::now(),
        };

        assert_eq!(start.aggregate_id(), stop.aggregate_id());
    }
}
