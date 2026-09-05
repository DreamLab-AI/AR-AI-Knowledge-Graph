//! Postcard wire-format types shared with the fastwebsockets transport
//!
//! ADR-2066: the QUIC/WebTransport server (`QuicTransportServer`, its session
//! state, control-message protocol, and topology/delta types) was dead code —
//! constructed nowhere, routed nowhere, only re-exported at
//! `src/handlers/mod.rs`. It has been removed. `PostcardNodeUpdate` and
//! `PostcardBatchUpdate` are kept because `src/handlers/fastwebsockets_handler.rs`
//! imports them directly (`super::quic_transport_handler::{PostcardBatchUpdate,
//! PostcardNodeUpdate}`) for its own postcard-serialized position broadcasts.

use serde::{Deserialize, Serialize};

use crate::utils::socket_flow_messages::BinaryNodeData;

// ============================================================================
// POSTCARD-OPTIMIZED WIRE PROTOCOL
// ============================================================================

/// Postcard-serialized position update (compact binary format)
/// Achieves ~12 GB/s serialization vs ~2 GB/s JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostcardNodeUpdate {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    #[serde(default)]
    pub cluster_id: u32,
    #[serde(default)]
    pub anomaly_score: f32,
    #[serde(default)]
    pub community_id: u32,
}

impl From<&BinaryNodeData> for PostcardNodeUpdate {
    fn from(node: &BinaryNodeData) -> Self {
        Self {
            id: node.node_id,
            x: node.x,
            y: node.y,
            z: node.z,
            vx: node.vx,
            vy: node.vy,
            vz: node.vz,
            cluster_id: 0,
            anomaly_score: 0.0,
            community_id: 0,
        }
    }
}

impl From<PostcardNodeUpdate> for BinaryNodeData {
    fn from(update: PostcardNodeUpdate) -> Self {
        BinaryNodeData {
            node_id: update.id,
            x: update.x,
            y: update.y,
            z: update.z,
            vx: update.vx,
            vy: update.vy,
            vz: update.vz,
        }
    }
}

/// Batch position update for efficient transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostcardBatchUpdate {
    pub frame_id: u64,
    pub timestamp_ms: u64,
    pub nodes: Vec<PostcardNodeUpdate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postcard_node_update_roundtrip() {
        let node = BinaryNodeData {
            node_id: 42,
            x: 1.0,
            y: 2.0,
            z: 3.0,
            vx: 0.1,
            vy: 0.2,
            vz: 0.3,
        };

        let update = PostcardNodeUpdate::from(&node);
        assert_eq!(update.id, 42);
        assert_eq!(update.x, 1.0);

        let back: BinaryNodeData = update.into();
        assert_eq!(back.node_id, 42);
        assert_eq!(back.x, 1.0);
    }
}
