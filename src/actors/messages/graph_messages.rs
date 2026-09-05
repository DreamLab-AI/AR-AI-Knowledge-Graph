//! Graph-domain messages: node/edge CRUD, graph data queries, metadata operations,
//! workspace management, and graph supervision.
//!
//! ## Split (ADR-090 Phase A3+)
//! Domain-safe types live in `visionclaw_actors::messages::graph_messages` and
//! are re-exported here so that `use crate::actors::messages::*` continues to
//! compile unchanged across all webxr actor files.
//!
//! ## Webxr-local definitions (types referencing webxr-internal types)
//! The following messages are defined locally rather than re-exported, because
//! their fields reference webxr-internal types:
//! - `UpdateNodePositions` — `positions: Vec<(u32, BinaryNodeData)>` where
//!   `BinaryNodeData` is `socket_flow_messages::BinaryNodeDataClient` (webxr alias)
//! - `UpdateNodePosition` — `position`/`velocity` are `glam::Vec3`
//! - `GetGraphStateActor` — return type names `graph_state_actor::GraphStateActor`
//! - `RequestGraphUpdate` — accepts `crate::models::graph_types::GraphType`

use actix::prelude::*;

use crate::actors::messaging::MessageId;
use crate::utils::socket_flow_messages::BinaryNodeData;

// ---------------------------------------------------------------------------
// Re-export domain-safe graph messages from visionclaw-actors
// (excludes UpdateNodePositions and UpdateNodePosition — defined locally below)
// ---------------------------------------------------------------------------

pub use visionclaw_actors::messages::graph_messages::{
    AddEdge, AddNode, AddNodesFromMetadata, ArchiveWorkspace, AutoBalanceNotification,
    BuildGraphFromMetadata, CreateWorkspace, DeleteWorkspace, GetAutoBalanceNotifications,
    GetGraphData, GetMetadata, GetNodeIdMapping, GetNodeMap, GetNodePositions, GetNodeTypeArrays,
    GetPositionFrameSnapshot, GetWorkspace, GetWorkspaceCount, GetWorkspaces, InitializeActor,
    LoadWorkspaces, NodeIdMapping, NodeTypeArrays, PositionFrameSnapshot, PositionRow,
    ReloadGraphFromDatabase, RemoveEdge, RemoveNode, RemoveNodeByMetadata, SaveWorkspaces,
    ToggleFavoriteWorkspace, UpdateGraphData, UpdateMetadata, UpdateNodeFromMetadata,
    UpdateNodeTypeArrays, UpdateWorkspace, WorkspaceChangeType, WorkspaceStateChanged,
};

// ---------------------------------------------------------------------------
// Webxr-local: position mutation messages (use webxr-native types)
// ---------------------------------------------------------------------------

/// Update a single node's position and velocity.
/// Uses `glam::Vec3` which is not in visionclaw-domain — stays in webxr.
#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateNodePosition {
    pub node_id: u32,
    pub position: glam::Vec3,
    pub velocity: glam::Vec3,
}

/// Batch node position update from GPU.
/// Uses `BinaryNodeData` (webxr alias for `BinaryNodeDataClient`) and
/// `MessageId` from the webxr messaging module.
#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct UpdateNodePositions {
    pub positions: Vec<(u32, BinaryNodeData)>,
    /// Optional correlation ID for message tracking (H4)
    pub correlation_id: Option<MessageId>,
}

// ---------------------------------------------------------------------------
// Webxr-internal messages (reference concrete webxr actor/model types)
// ---------------------------------------------------------------------------

/// Get the `GraphStateActor` address from `GraphServiceSupervisor`.
/// Stays in webxr because the return type names a concrete webxr actor.
#[derive(Message)]
#[rtype(result = "Option<actix::Addr<crate::actors::graph_state_actor::GraphStateActor>>")]
pub struct GetGraphStateActor;

/// Graph update request for supervision system.
/// Stays in webxr because `GraphType` is defined in `crate::models::graph_types`.
#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct RequestGraphUpdate {
    pub graph_type: crate::models::graph_types::GraphType,
    pub force_refresh: bool,
}
