// tests/ports/mocks.rs
//! Mock implementations of all ports for testing

use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use tokio::sync::RwLock;

use visionclaw_server::config::{AppFullSettings, PhysicsSettings};
use visionclaw_server::models::{Edge, GraphData, Node};
use visionclaw_server::models::constraints::ConstraintSet;
use visionclaw_server::ports::*;

// ============================================================================
// MockSettingsRepository
// ============================================================================

pub struct MockSettingsRepository {
    data: Arc<RwLock<HashMap<String, SettingValue>>>,
}

impl MockSettingsRepository {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl SettingsRepository for MockSettingsRepository {
    async fn get_setting(&self, key: &str) -> settings_repository::Result<Option<SettingValue>> {
        Ok(self.data.read().await.get(key).cloned())
    }

    async fn set_setting(
        &self,
        key: &str,
        value: SettingValue,
        _description: Option<&str>,
    ) -> settings_repository::Result<()> {
        self.data.write().await.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete_setting(&self, key: &str) -> settings_repository::Result<()> {
        self.data.write().await.remove(key);
        Ok(())
    }

    async fn has_setting(&self, key: &str) -> settings_repository::Result<bool> {
        Ok(self.data.read().await.contains_key(key))
    }

    async fn get_settings_batch(
        &self,
        keys: &[String],
    ) -> settings_repository::Result<HashMap<String, SettingValue>> {
        let data = self.data.read().await;
        Ok(keys
            .iter()
            .filter_map(|k| data.get(k).map(|v| (k.clone(), v.clone())))
            .collect())
    }

    async fn set_settings_batch(
        &self,
        updates: HashMap<String, SettingValue>,
    ) -> settings_repository::Result<()> {
        let mut data = self.data.write().await;
        for (key, value) in updates {
            data.insert(key, value);
        }
        Ok(())
    }

    async fn list_settings(
        &self,
        prefix: Option<&str>,
    ) -> settings_repository::Result<Vec<String>> {
        let data = self.data.read().await;
        Ok(data
            .keys()
            .filter(|k| prefix.map_or(true, |p| k.starts_with(p)))
            .cloned()
            .collect())
    }

    async fn load_all_settings(&self) -> settings_repository::Result<Option<AppFullSettings>> {
        // Return mock settings
        Ok(Some(AppFullSettings::default()))
    }

    async fn save_all_settings(
        &self,
        _settings: &AppFullSettings,
    ) -> settings_repository::Result<()> {
        Ok(())
    }

    async fn get_physics_settings(
        &self,
        profile_name: &str,
    ) -> settings_repository::Result<PhysicsSettings> {
        if let Some(SettingValue::Json(value)) = self
            .data
            .read()
            .await
            .get(&format!("physics_profile_{}", profile_name))
        {
            Ok(serde_json::from_value(value.clone()).unwrap_or_default())
        } else {
            Ok(PhysicsSettings::default())
        }
    }

    async fn save_physics_settings(
        &self,
        profile_name: &str,
        settings: &PhysicsSettings,
    ) -> settings_repository::Result<()> {
        let value = serde_json::to_value(settings).unwrap();
        self.data
            .write()
            .await
            .insert(
                format!("physics_profile_{}", profile_name),
                SettingValue::Json(value),
            );
        Ok(())
    }

    async fn list_physics_profiles(&self) -> settings_repository::Result<Vec<String>> {
        let data = self.data.read().await;
        Ok(data
            .keys()
            .filter(|k| k.starts_with("physics_profile_"))
            .map(|k| k.strip_prefix("physics_profile_").unwrap().to_string())
            .collect())
    }

    async fn delete_physics_profile(&self, profile_name: &str) -> settings_repository::Result<()> {
        self.data
            .write()
            .await
            .remove(&format!("physics_profile_{}", profile_name));
        Ok(())
    }

    async fn export_settings(&self) -> settings_repository::Result<serde_json::Value> {
        let data = self.data.read().await;
        Ok(serde_json::to_value(&*data).unwrap())
    }

    async fn import_settings(
        &self,
        settings_json: &serde_json::Value,
    ) -> settings_repository::Result<()> {
        let imported: HashMap<String, SettingValue> =
            serde_json::from_value(settings_json.clone()).unwrap();
        let mut data = self.data.write().await;
        for (key, value) in imported {
            data.insert(key, value);
        }
        Ok(())
    }

    async fn clear_cache(&self) -> settings_repository::Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> settings_repository::Result<bool> {
        Ok(true)
    }
}

// ============================================================================
// MockKnowledgeGraphRepository
// ============================================================================

pub struct MockKnowledgeGraphRepository {
    graph: Arc<RwLock<GraphData>>,
    next_node_id: Arc<AtomicU32>,
}

impl MockKnowledgeGraphRepository {
    pub fn new() -> Self {
        Self {
            graph: Arc::new(RwLock::new(GraphData::new())),
            next_node_id: Arc::new(AtomicU32::new(1)),
        }
    }

    pub fn with_graph(graph: GraphData) -> Self {
        let max_id = graph.nodes.iter().map(|n| n.id).max().unwrap_or(0);
        Self {
            graph: Arc::new(RwLock::new(graph)),
            next_node_id: Arc::new(AtomicU32::new(max_id + 1)),
        }
    }
}

#[async_trait]
impl KnowledgeGraphRepository for MockKnowledgeGraphRepository {
    async fn load_graph(&self) -> knowledge_graph_repository::Result<Arc<GraphData>> {
        Ok(Arc::new(self.graph.read().await.clone()))
    }

    async fn save_graph(&self, graph: &GraphData) -> knowledge_graph_repository::Result<()> {
        *self.graph.write().await = graph.clone();
        Ok(())
    }

    async fn add_node(&self, node: &Node) -> knowledge_graph_repository::Result<u32> {
        let id = self.next_node_id.fetch_add(1, Ordering::SeqCst);
        let mut new_node = node.clone();
        new_node.id = id;
        self.graph.write().await.nodes.push(new_node);
        Ok(id)
    }

    async fn batch_add_nodes(
        &self,
        nodes: Vec<Node>,
    ) -> knowledge_graph_repository::Result<Vec<u32>> {
        let mut ids = Vec::new();
        for node in nodes {
            ids.push(self.add_node(&node).await?);
        }
        Ok(ids)
    }

    async fn batch_add_nodes_if_absent(
        &self,
        nodes: Vec<Node>,
    ) -> knowledge_graph_repository::Result<Vec<u32>> {
        let mut ids = Vec::new();
        for node in nodes {
            let exists = self.graph.read().await.nodes.iter().any(|n| n.id == node.id);
            if !exists {
                ids.push(self.add_node(&node).await?);
            }
        }
        Ok(ids)
    }

    async fn update_node(&self, node: &Node) -> knowledge_graph_repository::Result<()> {
        let mut graph = self.graph.write().await;
        if let Some(existing) = graph.nodes.iter_mut().find(|n| n.id == node.id) {
            *existing = node.clone();
            Ok(())
        } else {
            Err(knowledge_graph_repository::KnowledgeGraphRepositoryError::NodeNotFound(node.id))
        }
    }

    async fn batch_update_nodes(
        &self,
        nodes: Vec<Node>,
    ) -> knowledge_graph_repository::Result<()> {
        for node in nodes {
            self.update_node(&node).await?;
        }
        Ok(())
    }

    async fn remove_node(&self, node_id: u32) -> knowledge_graph_repository::Result<()> {
        let mut graph = self.graph.write().await;
        graph.nodes.retain(|n| n.id != node_id);
        graph.edges.retain(|e| e.source != node_id && e.target != node_id);
        Ok(())
    }

    async fn batch_remove_nodes(
        &self,
        node_ids: Vec<u32>,
    ) -> knowledge_graph_repository::Result<()> {
        let mut graph = self.graph.write().await;
        let ids_set: HashSet<u32> = node_ids.into_iter().collect();
        graph.nodes.retain(|n| !ids_set.contains(&n.id));
        graph.edges.retain(|e| !ids_set.contains(&e.source) && !ids_set.contains(&e.target));
        Ok(())
    }

    async fn get_node(&self, node_id: u32) -> knowledge_graph_repository::Result<Option<Node>> {
        let graph = self.graph.read().await;
        Ok(graph.nodes.iter().find(|n| n.id == node_id).cloned())
    }

    async fn get_nodes(&self, node_ids: Vec<u32>) -> knowledge_graph_repository::Result<Vec<Node>> {
        let graph = self.graph.read().await;
        let ids_set: HashSet<u32> = node_ids.into_iter().collect();
        Ok(graph.nodes.iter().filter(|n| ids_set.contains(&n.id)).cloned().collect())
    }

    async fn get_nodes_by_metadata_id(
        &self,
        metadata_id: &str,
    ) -> knowledge_graph_repository::Result<Vec<Node>> {
        let graph = self.graph.read().await;
        Ok(graph.nodes.iter().filter(|n| n.metadata_id == metadata_id).cloned().collect())
    }

    async fn search_nodes_by_label(
        &self,
        label: &str,
    ) -> knowledge_graph_repository::Result<Vec<Node>> {
        let graph = self.graph.read().await;
        let label_lower = label.to_lowercase();
        Ok(graph
            .nodes
            .iter()
            .filter(|n| n.label.to_lowercase().contains(&label_lower))
            .cloned()
            .collect())
    }

    async fn add_edge(&self, edge: &Edge) -> knowledge_graph_repository::Result<String> {
        let id = format!("edge_{}", uuid::Uuid::new_v4());
        let mut new_edge = edge.clone();
        new_edge.id = id.clone();
        self.graph.write().await.edges.push(new_edge);
        Ok(id)
    }

    async fn batch_add_edges(
        &self,
        edges: Vec<Edge>,
    ) -> knowledge_graph_repository::Result<Vec<String>> {
        let mut ids = Vec::new();
        for edge in edges {
            ids.push(self.add_edge(&edge).await?);
        }
        Ok(ids)
    }

    async fn update_edge(&self, edge: &Edge) -> knowledge_graph_repository::Result<()> {
        let mut graph = self.graph.write().await;
        if let Some(existing) = graph.edges.iter_mut().find(|e| e.id == edge.id) {
            *existing = edge.clone();
            Ok(())
        } else {
            Err(knowledge_graph_repository::KnowledgeGraphRepositoryError::EdgeNotFound(edge.id.clone()))
        }
    }

    async fn remove_edge(&self, edge_id: &str) -> knowledge_graph_repository::Result<()> {
        self.graph.write().await.edges.retain(|e| e.id != edge_id);
        Ok(())
    }

    async fn batch_remove_edges(
        &self,
        edge_ids: Vec<String>,
    ) -> knowledge_graph_repository::Result<()> {
        let mut graph = self.graph.write().await;
        let ids_set: HashSet<String> = edge_ids.into_iter().collect();
        graph.edges.retain(|e| !ids_set.contains(&e.id));
        Ok(())
    }

    async fn get_node_edges(&self, node_id: u32) -> knowledge_graph_repository::Result<Vec<Edge>> {
        let graph = self.graph.read().await;
        Ok(graph
            .edges
            .iter()
            .filter(|e| e.source == node_id || e.target == node_id)
            .cloned()
            .collect())
    }

    async fn get_edges_between(
        &self,
        source_id: u32,
        target_id: u32,
    ) -> knowledge_graph_repository::Result<Vec<Edge>> {
        let graph = self.graph.read().await;
        Ok(graph
            .edges
            .iter()
            .filter(|e| e.source == source_id && e.target == target_id)
            .cloned()
            .collect())
    }

    async fn batch_update_positions(
        &self,
        positions: Vec<(u32, f32, f32, f32)>,
    ) -> knowledge_graph_repository::Result<()> {
        let mut graph = self.graph.write().await;
        for (node_id, x, y, z) in positions {
            if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == node_id) {
                node.position.x = x;
                node.position.y = y;
                node.position.z = z;
            }
        }
        Ok(())
    }

    async fn query_nodes(&self, _query: &str) -> knowledge_graph_repository::Result<Vec<Node>> {
        // Simplified mock: return all nodes
        let graph = self.graph.read().await;
        Ok(graph.nodes.clone())
    }

    async fn get_neighbors(&self, node_id: u32) -> knowledge_graph_repository::Result<Vec<Node>> {
        let graph = self.graph.read().await;
        let neighbor_ids: HashSet<u32> = graph
            .edges
            .iter()
            .filter_map(|e| {
                if e.source == node_id {
                    Some(e.target)
                } else if e.target == node_id {
                    Some(e.source)
                } else {
                    None
                }
            })
            .collect();

        Ok(graph.nodes.iter().filter(|n| neighbor_ids.contains(&n.id)).cloned().collect())
    }

    async fn get_statistics(
        &self,
    ) -> knowledge_graph_repository::Result<knowledge_graph_repository::GraphStatistics> {
        let graph = self.graph.read().await;
        let node_count = graph.nodes.len();
        let edge_count = graph.edges.len();

        Ok(knowledge_graph_repository::GraphStatistics {
            node_count,
            edge_count,
            average_degree: if node_count > 0 {
                (edge_count * 2) as f32 / node_count as f32
            } else {
                0.0
            },
            connected_components: 1, // Simplified
            last_updated: Utc::now(),
        })
    }

    async fn clear_graph(&self) -> knowledge_graph_repository::Result<()> {
        *self.graph.write().await = GraphData::new();
        Ok(())
    }

    async fn begin_transaction(&self) -> knowledge_graph_repository::Result<()> {
        Ok(())
    }

    async fn commit_transaction(&self) -> knowledge_graph_repository::Result<()> {
        Ok(())
    }

    async fn rollback_transaction(&self) -> knowledge_graph_repository::Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> knowledge_graph_repository::Result<bool> {
        Ok(true)
    }
}

// ============================================================================
// MockOntologyRepository
// ============================================================================

pub struct MockOntologyRepository {
    classes: Arc<RwLock<Vec<ontology_repository::OwlClass>>>,
    properties: Arc<RwLock<Vec<ontology_repository::OwlProperty>>>,
    axioms: Arc<RwLock<Vec<ontology_repository::OwlAxiom>>>,
    next_axiom_id: Arc<AtomicU32>,
}

impl MockOntologyRepository {
    pub fn new() -> Self {
        Self {
            classes: Arc::new(RwLock::new(Vec::new())),
            properties: Arc::new(RwLock::new(Vec::new())),
            axioms: Arc::new(RwLock::new(Vec::new())),
            next_axiom_id: Arc::new(AtomicU32::new(1)),
        }
    }
}

#[async_trait]
impl OntologyRepository for MockOntologyRepository {
    async fn load_ontology_graph(&self) -> ontology_repository::Result<Arc<GraphData>> {
        Ok(Arc::new(GraphData::new()))
    }

    async fn save_ontology_graph(&self, _graph: &GraphData) -> ontology_repository::Result<()> {
        Ok(())
    }

    async fn save_ontology(
        &self,
        classes: &[ontology_repository::OwlClass],
        properties: &[ontology_repository::OwlProperty],
        axioms: &[ontology_repository::OwlAxiom],
    ) -> ontology_repository::Result<()> {
        *self.classes.write().await = classes.to_vec();
        *self.properties.write().await = properties.to_vec();
        *self.axioms.write().await = axioms.to_vec();
        Ok(())
    }

    async fn add_owl_class(
        &self,
        class: &ontology_repository::OwlClass,
    ) -> ontology_repository::Result<String> {
        self.classes.write().await.push(class.clone());
        Ok(class.iri.clone())
    }

    async fn get_owl_class(
        &self,
        iri: &str,
    ) -> ontology_repository::Result<Option<ontology_repository::OwlClass>> {
        let classes = self.classes.read().await;
        Ok(classes.iter().find(|c| c.iri == iri).cloned())
    }

    async fn list_owl_classes(
        &self,
    ) -> ontology_repository::Result<Vec<ontology_repository::OwlClass>> {
        Ok(self.classes.read().await.clone())
    }

    async fn add_owl_property(
        &self,
        property: &ontology_repository::OwlProperty,
    ) -> ontology_repository::Result<String> {
        self.properties.write().await.push(property.clone());
        Ok(property.iri.clone())
    }

    async fn get_owl_property(
        &self,
        iri: &str,
    ) -> ontology_repository::Result<Option<ontology_repository::OwlProperty>> {
        let properties = self.properties.read().await;
        Ok(properties.iter().find(|p| p.iri == iri).cloned())
    }

    async fn list_owl_properties(
        &self,
    ) -> ontology_repository::Result<Vec<ontology_repository::OwlProperty>> {
        Ok(self.properties.read().await.clone())
    }

    async fn get_classes(
        &self,
    ) -> ontology_repository::Result<Vec<ontology_repository::OwlClass>> {
        // Alias for list_owl_classes
        self.list_owl_classes().await
    }

    async fn get_axioms(
        &self,
    ) -> ontology_repository::Result<Vec<ontology_repository::OwlAxiom>> {
        Ok(self.axioms.read().await.clone())
    }

    async fn add_axiom(
        &self,
        axiom: &ontology_repository::OwlAxiom,
    ) -> ontology_repository::Result<u64> {
        let id = self.next_axiom_id.fetch_add(1, Ordering::SeqCst) as u64;
        let mut axiom = axiom.clone();
        axiom.id = Some(id);
        self.axioms.write().await.push(axiom);
        Ok(id)
    }

    async fn get_class_axioms(
        &self,
        class_iri: &str,
    ) -> ontology_repository::Result<Vec<ontology_repository::OwlAxiom>> {
        let axioms = self.axioms.read().await;
        Ok(axioms
            .iter()
            .filter(|a| a.subject == class_iri || a.object == class_iri)
            .cloned()
            .collect())
    }

    async fn store_inference_results(
        &self,
        _results: &ontology_repository::InferenceResults,
    ) -> ontology_repository::Result<()> {
        Ok(())
    }

    async fn get_inference_results(
        &self,
    ) -> ontology_repository::Result<Option<ontology_repository::InferenceResults>> {
        Ok(None)
    }

    async fn validate_ontology(
        &self,
    ) -> ontology_repository::Result<ontology_repository::ValidationReport> {
        Ok(ontology_repository::ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            timestamp: Utc::now(),
        })
    }

    async fn query_ontology(
        &self,
        _query: &str,
    ) -> ontology_repository::Result<Vec<HashMap<String, String>>> {
        Ok(Vec::new())
    }

    async fn get_metrics(&self) -> ontology_repository::Result<ontology_repository::OntologyMetrics> {
        let classes = self.classes.read().await;
        let properties = self.properties.read().await;
        let axioms = self.axioms.read().await;

        Ok(ontology_repository::OntologyMetrics {
            class_count: classes.len(),
            property_count: properties.len(),
            axiom_count: axioms.len(),
            max_depth: 3,
            average_branching_factor: 2.0,
        })
    }

    async fn cache_sssp_result(
        &self,
        _entry: &ontology_repository::PathfindingCacheEntry,
    ) -> ontology_repository::Result<()> {
        Ok(())
    }

    async fn get_cached_sssp(
        &self,
        _source_node_id: u32,
    ) -> ontology_repository::Result<Option<ontology_repository::PathfindingCacheEntry>> {
        Ok(None)
    }

    async fn cache_apsp_result(
        &self,
        _distance_matrix: &Vec<Vec<f32>>,
    ) -> ontology_repository::Result<()> {
        Ok(())
    }

    async fn get_cached_apsp(&self) -> ontology_repository::Result<Option<Vec<Vec<f32>>>> {
        Ok(None)
    }

    async fn invalidate_pathfinding_caches(&self) -> ontology_repository::Result<()> {
        Ok(())
    }
}

// Additional mock implementations will be added in separate files
// to keep this file manageable
