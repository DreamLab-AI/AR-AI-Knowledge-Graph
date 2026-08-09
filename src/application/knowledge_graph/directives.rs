// src/application/knowledge_graph/directives.rs
//! Knowledge Graph Domain - Write Operations (Directives)
//!
//! All directives for modifying graph state following CQRS patterns.

use hexser::{Directive, DirectiveHandler, HexResult, Hexserror};
use std::sync::Arc;

use crate::ports::knowledge_graph_repository::KnowledgeGraphRepository;
use visionclaw_domain::models::edge::Edge;
use visionclaw_domain::models::graph::GraphData;
use visionclaw_domain::models::node::Node;

// ============================================================================
// ADD NODE
// ============================================================================

#[derive(Debug, Clone)]
pub struct AddNode {
    pub node: Node,
}

impl Directive for AddNode {
    fn validate(&self) -> HexResult<()> {
        if self.node.metadata_id.is_empty() {
            return Err(Hexserror::validation("Node metadata_id cannot be empty"));
        }
        Ok(())
    }
}

pub struct AddNodeHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> AddNodeHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<AddNode>
    for AddNodeHandler<R>
{
    fn handle(&self, directive: AddNode) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!(
                "Executing AddNode directive: metadata_id={}",
                directive.node.metadata_id
            );

            let node_id = repository.add_node(&directive.node).await.map_err(|e| {
                Hexserror::adapter("E_KG_ADD_NODE", &format!("Failed to add node: {}", e))
            })?;

            log::info!("Node added successfully: id={}", node_id);
            Ok(())
        })
    }
}

// ============================================================================
// UPDATE NODE
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdateNode {
    pub node: Node,
}

impl Directive for UpdateNode {
    fn validate(&self) -> HexResult<()> {
        if self.node.id == 0 {
            return Err(Hexserror::validation("Node id cannot be 0"));
        }
        Ok(())
    }
}

pub struct UpdateNodeHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> UpdateNodeHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<UpdateNode>
    for UpdateNodeHandler<R>
{
    fn handle(&self, directive: UpdateNode) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!("Executing UpdateNode directive: id={}", directive.node.id);

            repository.update_node(&directive.node).await.map_err(|e| {
                Hexserror::adapter("E_KG_UPDATE_NODE", &format!("Failed to update node: {}", e))
            })?;

            log::info!("Node updated successfully: id={}", directive.node.id);
            Ok(())
        })
    }
}

// ============================================================================
// REMOVE NODE
// ============================================================================

#[derive(Debug, Clone)]
pub struct RemoveNode {
    pub node_id: u32,
}

impl Directive for RemoveNode {
    fn validate(&self) -> HexResult<()> {
        if self.node_id == 0 {
            return Err(Hexserror::validation("Node id cannot be 0"));
        }
        Ok(())
    }
}

pub struct RemoveNodeHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> RemoveNodeHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<RemoveNode>
    for RemoveNodeHandler<R>
{
    fn handle(&self, directive: RemoveNode) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!("Executing RemoveNode directive: id={}", directive.node_id);

            repository
                .remove_node(directive.node_id)
                .await
                .map_err(|e| {
                    Hexserror::adapter("E_KG_REMOVE_NODE", &format!("Failed to remove node: {}", e))
                })?;

            log::info!("Node removed successfully: id={}", directive.node_id);
            Ok(())
        })
    }
}

// ============================================================================
// ADD EDGE
// ============================================================================

#[derive(Debug, Clone)]
pub struct AddEdge {
    pub edge: Edge,
}

impl Directive for AddEdge {
    fn validate(&self) -> HexResult<()> {
        if self.edge.source == 0 || self.edge.target == 0 {
            return Err(Hexserror::validation(
                "Edge source and target must be non-zero",
            ));
        }
        Ok(())
    }
}

pub struct AddEdgeHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> AddEdgeHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<AddEdge>
    for AddEdgeHandler<R>
{
    fn handle(&self, directive: AddEdge) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!(
                "Executing AddEdge directive: source={}, target={}",
                directive.edge.source,
                directive.edge.target
            );

            let edge_id = repository.add_edge(&directive.edge).await.map_err(|e| {
                Hexserror::adapter("E_KG_ADD_EDGE", &format!("Failed to add edge: {}", e))
            })?;

            log::info!("Edge added successfully: id={}", edge_id);
            Ok(())
        })
    }
}

// ============================================================================
// UPDATE EDGE
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdateEdge {
    pub edge: Edge,
}

impl Directive for UpdateEdge {
    fn validate(&self) -> HexResult<()> {
        if self.edge.id.is_empty() {
            return Err(Hexserror::validation("Edge id cannot be empty"));
        }
        Ok(())
    }
}

pub struct UpdateEdgeHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> UpdateEdgeHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<UpdateEdge>
    for UpdateEdgeHandler<R>
{
    fn handle(&self, directive: UpdateEdge) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!("Executing UpdateEdge directive: id={}", directive.edge.id);

            repository.update_edge(&directive.edge).await.map_err(|e| {
                Hexserror::adapter("E_KG_UPDATE_EDGE", &format!("Failed to update edge: {}", e))
            })?;

            log::info!("Edge updated successfully: id={}", directive.edge.id);
            Ok(())
        })
    }
}

// ============================================================================
// REMOVE EDGE
// ============================================================================

#[derive(Debug, Clone)]
pub struct RemoveEdge {
    pub edge_id: String,
}

impl Directive for RemoveEdge {
    fn validate(&self) -> HexResult<()> {
        if self.edge_id.is_empty() {
            return Err(Hexserror::validation("Edge id cannot be empty"));
        }
        Ok(())
    }
}

pub struct RemoveEdgeHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> RemoveEdgeHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<RemoveEdge>
    for RemoveEdgeHandler<R>
{
    fn handle(&self, directive: RemoveEdge) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!("Executing RemoveEdge directive: id={}", directive.edge_id);

            repository
                .remove_edge(&directive.edge_id)
                .await
                .map_err(|e| {
                    Hexserror::adapter("E_KG_REMOVE_EDGE", &format!("Failed to remove edge: {}", e))
                })?;

            log::info!("Edge removed successfully: id={}", directive.edge_id);
            Ok(())
        })
    }
}

// ============================================================================
// SAVE GRAPH
// ============================================================================

#[derive(Debug, Clone)]
pub struct SaveGraph {
    pub graph: GraphData,
}

impl Directive for SaveGraph {
    fn validate(&self) -> HexResult<()> {
        Ok(())
    }
}

pub struct SaveGraphHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> SaveGraphHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<SaveGraph>
    for SaveGraphHandler<R>
{
    fn handle(&self, directive: SaveGraph) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!(
                "Executing SaveGraph directive: {} nodes, {} edges",
                directive.graph.nodes.len(),
                directive.graph.edges.len()
            );

            repository.save_graph(&directive.graph).await.map_err(|e| {
                Hexserror::adapter("E_KG_SAVE_GRAPH", &format!("Failed to save graph: {}", e))
            })?;

            log::info!("Graph saved successfully");
            Ok(())
        })
    }
}

// ============================================================================
// BATCH UPDATE POSITIONS
// ============================================================================

#[derive(Debug, Clone)]
pub struct BatchUpdatePositions {
    pub positions: Vec<(u32, f32, f32, f32)>,
}

impl Directive for BatchUpdatePositions {
    fn validate(&self) -> HexResult<()> {
        if self.positions.is_empty() {
            return Err(Hexserror::validation("Positions list cannot be empty"));
        }
        Ok(())
    }
}

pub struct BatchUpdatePositionsHandler<R: KnowledgeGraphRepository> {
    repository: Arc<R>,
}

impl<R: KnowledgeGraphRepository> BatchUpdatePositionsHandler<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: KnowledgeGraphRepository + Send + Sync + 'static> DirectiveHandler<BatchUpdatePositions>
    for BatchUpdatePositionsHandler<R>
{
    fn handle(&self, directive: BatchUpdatePositions) -> HexResult<()> {
        let repository = self.repository.clone();
        tokio::runtime::Handle::current().block_on(async move {
            log::info!(
                "Executing BatchUpdatePositions directive: {} positions",
                directive.positions.len()
            );

            repository
                .batch_update_positions(directive.positions)
                .await
                .map_err(|e| {
                    Hexserror::adapter(
                        "E_KG_BATCH_UPDATE",
                        &format!("Failed to batch update positions: {}", e),
                    )
                })?;

            log::info!("Positions updated successfully");
            Ok(())
        })
    }
}
