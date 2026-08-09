//! ADR-090 Phase A6 — webxr shim for `socket_flow_messages`.
//!
//! All pure-data wire-format types have been promoted to
//! `visionclaw_protocol::socket_flow_messages`.  This file re-exports them so
//! that the 13 existing adapter/actor callers continue to compile unchanged.
//!
//! # Items that REMAIN here (not promoteable)
//!
//! * `BinaryNodeDataGPU` — carries `#[cfg(feature = "gpu")]` cudarc impls
//!   (`DeviceRepr`, `ValidAsZeroBits`).  These are GPU-server concerns; cudarc
//!   is not a protocol-layer dep.
//!
//! * `vec3data_to_glam` / `glam_to_vec3data` — require `glam::Vec3`.  Adding
//!   glam to visionclaw-protocol's dep graph is unwarranted for two small
//!   helpers.
//!
//! # TODO (Phase A6 follow-up)
//!
//! Once all 13 adapter/actor callers have been migrated to import directly from
//! `visionclaw_protocol::socket_flow_messages`, delete this file and remove the
//! `pub use` lines.  The glam helpers can be moved to a `visionclaw-server-utils`
//! helper module at that point.

// ── Re-exports from visionclaw-protocol ──────────────────────────────────────
pub use visionclaw_protocol::socket_flow_messages::{
    array_to_vec3data, vec3data_to_array, BinaryNodeData, BinaryNodeDataClient, InitialEdgeData,
    InitialNodeData, Message, PingMessage, PongMessage,
};

// ── GPU-only types (stay in webxr) ───────────────────────────────────────────
use crate::types::vec3::Vec3Data;
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "gpu")]
use cudarc::driver::{DeviceRepr, ValidAsZeroBits};
use glam::Vec3;

/// Extended node record for server-side GPU computations (48 bytes).
///
/// Use [`BinaryNodeDataClient`] (28 bytes) for the network wire format.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, serde::Serialize, serde::Deserialize)]
pub struct BinaryNodeDataGPU {
    pub node_id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    pub sssp_distance: f32,
    pub sssp_parent: i32,
    pub cluster_id: i32,
    pub centrality: f32,
    pub mass: f32,
}

static_assertions::const_assert_eq!(std::mem::size_of::<BinaryNodeDataGPU>(), 48);

#[cfg(feature = "gpu")]
unsafe impl DeviceRepr for BinaryNodeDataGPU {}
#[cfg(feature = "gpu")]
unsafe impl ValidAsZeroBits for BinaryNodeDataGPU {}

impl BinaryNodeDataGPU {
    pub fn to_client(&self) -> BinaryNodeDataClient {
        BinaryNodeDataClient {
            node_id: self.node_id,
            x: self.x,
            y: self.y,
            z: self.z,
            vx: self.vx,
            vy: self.vy,
            vz: self.vz,
        }
    }

    pub fn from_client(client: &BinaryNodeDataClient) -> Self {
        Self {
            node_id: client.node_id,
            x: client.x,
            y: client.y,
            z: client.z,
            vx: client.vx,
            vy: client.vy,
            vz: client.vz,
            sssp_distance: f32::INFINITY,
            sssp_parent: -1,
            cluster_id: -1,
            centrality: 0.0,
            mass: 1.0,
        }
    }
}

// ── glam helpers (stay in webxr — glam not a protocol dep) ───────────────────
//
// Note: `glam_to_vec3data` returns `visionclaw_domain::Vec3Data` so that
// callers passing the result directly to `BinaryNodeDataClient::new` continue
// to compile unchanged (that method now takes the domain type after the
// ADR-090 Phase A6 promotion).  The webxr-local `Vec3Data` is kept for
// `vec3data_to_glam` which only reads x/y/z fields.

#[inline]
pub fn vec3data_to_glam(vec: &Vec3Data) -> Vec3 {
    Vec3::new(vec.x, vec.y, vec.z)
}

#[inline]
pub fn glam_to_vec3data(vec: Vec3) -> visionclaw_domain::Vec3Data {
    visionclaw_domain::Vec3Data::new(vec.x, vec.y, vec.z)
}
