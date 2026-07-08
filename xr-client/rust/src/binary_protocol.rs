//! gdext class decoding the live `/wss` graph position stream.
//!
//! The wire format is Protocol V3 (server `src/utils/binary_protocol.rs`,
//! ADR-031 Analytics Extension): a single version byte `0x03` followed by N
//! fixed 52-byte node records. Decoding is fallible (never `.unwrap()`) so a
//! malformed frame can never panic the Quest render loop.
//!
//! Record layout (little-endian):
//!   id@0 u32 (flag bits 26-31), pos@4 f32x3, vel@16 f32x3, sssp_dist@28 f32,
//!   sssp_parent@32 i32, cluster_id@36 u32, anomaly@40 f32, community@44 u32,
//!   centrality@48 f32.

use bytes::Bytes;
use thiserror::Error;
use tracing::{debug, error, warn};

#[cfg(not(test))]
use godot::prelude::*;

/// Protocol version byte that prefixes every V3 graph position frame.
pub const PROTOCOL_V3: u8 = 0x03;
/// Bytes per node record in a V3 frame.
pub const NODE_RECORD_BYTES: usize = 52;
const HEADER_BYTES: usize = 1;

/// Node ID occupies bits 0-25; bits 26-31 carry the node-type flags.
pub const NODE_ID_MASK: u32 = 0x03FF_FFFF;
const AGENT_NODE_FLAG: u32 = 0x8000_0000;
const KNOWLEDGE_NODE_FLAG: u32 = 0x4000_0000;
const ONTOLOGY_TYPE_MASK: u32 = 0x1C00_0000;
const ONTOLOGY_CLASS_FLAG: u32 = 0x0400_0000;
const ONTOLOGY_INDIVIDUAL_FLAG: u32 = 0x0800_0000;
const ONTOLOGY_PROPERTY_FLAG: u32 = 0x1000_0000;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("frame too short: need {need} bytes, got {got}")]
    TooShort { need: usize, got: usize },
    #[error("unexpected protocol version 0x{version:02x}, expected 0x{expected:02x}")]
    BadVersion { version: u8, expected: u8 },
    #[error("payload length {len} not aligned to {NODE_RECORD_BYTES}-byte node record")]
    Misaligned { len: usize },
}

/// Node-type encoded in the high flag bits of the wire id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Agent,
    Knowledge,
    OntologyClass,
    OntologyIndividual,
    OntologyProperty,
    Plain,
}

impl NodeKind {
    fn from_wire_id(raw: u32) -> Self {
        if raw & AGENT_NODE_FLAG != 0 {
            Self::Agent
        } else if raw & KNOWLEDGE_NODE_FLAG != 0 {
            Self::Knowledge
        } else {
            match raw & ONTOLOGY_TYPE_MASK {
                ONTOLOGY_CLASS_FLAG => Self::OntologyClass,
                ONTOLOGY_INDIVIDUAL_FLAG => Self::OntologyIndividual,
                ONTOLOGY_PROPERTY_FLAG => Self::OntologyProperty,
                _ => Self::Plain,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeUpdate {
    pub node_id: u32,
    pub kind: NodeKind,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    /// ADR-031 analytics tail (record bytes 28..52). The server computes these
    /// on the GPU; the XR renderer uses community for colour, centrality for
    /// size/importance and anomaly for highlighting — same visual language as
    /// the desktop client.
    pub sssp_distance: f32,
    pub sssp_parent: i32,
    pub cluster_id: u32,
    pub anomaly: f32,
    pub community_id: u32,
    pub centrality: f32,
}

/// Quantised visual identity of a node. Two updates with the same key render
/// identically, so `node_visuals_updated` only fires when the key changes —
/// per-frame signal traffic stays near zero once communities stabilise.
/// Centrality is bucketed to 1/64ths (visually sub-perceptual), anomaly to
/// 1/16ths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualsKey {
    pub community_id: u32,
    pub centrality_bucket: u8,
    pub anomaly_bucket: u8,
}

impl VisualsKey {
    pub fn of(u: &NodeUpdate) -> Self {
        Self {
            community_id: u.community_id,
            centrality_bucket: (u.centrality.clamp(0.0, 1.0) * 63.0) as u8,
            anomaly_bucket: (u.anomaly.clamp(0.0, 1.0) * 15.0) as u8,
        }
    }
}

/// One edge from the `initialGraphLoad` topology message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSpec {
    pub source: u32,
    pub target: u32,
    pub weight: f32,
}

/// Parse a `/wss` text frame; returns the edge topology when the frame is an
/// `initialGraphLoad` message, `None` for every other text frame. Wire shape
/// (server `visionclaw-protocol::socket_flow_messages`, snake_case fields):
/// `{"type":"initialGraphLoad","nodes":[...],"edges":[{"id":"..","source_id":u32,
/// "target_id":u32,"weight":f32?,..}],"timestamp":u64}`.
pub fn parse_initial_graph_load(text: &str) -> Option<Vec<EdgeSpec>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("initialGraphLoad") {
        return None;
    }
    let edges = v.get("edges")?.as_array()?;
    let mut out = Vec::with_capacity(edges.len());
    for e in edges {
        let source = e.get("source_id").and_then(|s| s.as_u64())?;
        let target = e.get("target_id").and_then(|t| t.as_u64())?;
        if source > u32::MAX as u64 || target > u32::MAX as u64 {
            continue;
        }
        let weight = e
            .get("weight")
            .and_then(|w| w.as_f64())
            .unwrap_or(1.0) as f32;
        out.push(EdgeSpec {
            source: source as u32 & NODE_ID_MASK,
            target: target as u32 & NODE_ID_MASK,
            weight,
        });
    }
    Some(out)
}

/// Extract `(node_id, did_nostr)` pairs for agent nodes from an
/// `initialGraphLoad` text frame — the additive DID-carrying channel for the
/// selection arbiter (COM-14 / M4, ADR-130 Decision 4/6).
///
/// The fixed 52-byte binary position record (0x03) has no room for a 32-byte
/// pubkey, so agent identity rides the `initialGraphLoad` node metadata instead.
/// This reads an optional `did_nostr` / `didNostr` string per node (keyed by the
/// node's `id` / `node_id`, masked with [`NODE_ID_MASK`] to drop the type-flag
/// bits) and returns only the nodes that carry one.
///
/// **Named server-side integration point:** the VisionClaw graph server must
/// include `did_nostr` on each agent node in the `initialGraphLoad` payload
/// (source: the agent record's `did_nostr` field landed by COM-14 P0 in
/// `src/services/bots_client.rs` / `agent_visualization_protocol.rs`). Until the
/// server emits it, this returns an empty map and selections carry `None` — an
/// honest absence, not a fabricated identity. GDScript feeds the result into
/// `SelectionArbiterNode::register_identity`.
pub fn parse_agent_identities(text: &str) -> Vec<(u32, String)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    if v.get("type").and_then(|t| t.as_str()) != Some("initialGraphLoad") {
        return Vec::new();
    }
    let Some(nodes) = v.get("nodes").and_then(|n| n.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for node in nodes {
        let did = node
            .get("did_nostr")
            .or_else(|| node.get("didNostr"))
            .and_then(|d| d.as_str());
        let Some(did) = did else { continue };
        if did.is_empty() {
            continue;
        }
        let raw_id = node
            .get("id")
            .or_else(|| node.get("node_id"))
            .and_then(json_u32);
        let Some(raw_id) = raw_id else { continue };
        out.push((raw_id & NODE_ID_MASK, did.to_owned()));
    }
    out
}

/// Read a u32 from a JSON value that may be a number or a numeric string.
fn json_u32(v: &serde_json::Value) -> Option<u32> {
    if let Some(n) = v.as_u64() {
        return (n <= u32::MAX as u64).then_some(n as u32);
    }
    v.as_str().and_then(|s| s.parse::<u32>().ok())
}

pub fn decode_position_frame(bytes: &[u8]) -> Result<Vec<NodeUpdate>, DecodeError> {
    if bytes.len() < HEADER_BYTES {
        return Err(DecodeError::TooShort {
            need: HEADER_BYTES,
            got: bytes.len(),
        });
    }
    let version = bytes[0];
    if version != PROTOCOL_V3 {
        return Err(DecodeError::BadVersion {
            version,
            expected: PROTOCOL_V3,
        });
    }
    let payload = &bytes[HEADER_BYTES..];
    if !payload.len().is_multiple_of(NODE_RECORD_BYTES) {
        return Err(DecodeError::Misaligned { len: payload.len() });
    }
    let count = payload.len() / NODE_RECORD_BYTES;
    let mut out = Vec::with_capacity(count);
    for chunk in payload.chunks_exact(NODE_RECORD_BYTES) {
        out.push(parse_node_record(chunk));
    }
    Ok(out)
}

#[inline]
fn read_f32(bytes: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

#[inline]
fn read_u32(bytes: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn parse_node_record(bytes: &[u8]) -> NodeUpdate {
    let raw_id = read_u32(bytes, 0);
    let kind = NodeKind::from_wire_id(raw_id);
    let node_id = raw_id & NODE_ID_MASK;
    let position = [read_f32(bytes, 4), read_f32(bytes, 8), read_f32(bytes, 12)];
    let velocity = [read_f32(bytes, 16), read_f32(bytes, 20), read_f32(bytes, 24)];
    NodeUpdate {
        node_id,
        kind,
        position,
        velocity,
        sssp_distance: read_f32(bytes, 28),
        sssp_parent: read_u32(bytes, 32) as i32,
        cluster_id: read_u32(bytes, 36),
        anomaly: read_f32(bytes, 40),
        community_id: read_u32(bytes, 44),
        centrality: read_f32(bytes, 48),
    }
}

/// Protocol version byte of a frame, or `None` if empty.
pub fn frame_version(bytes: &[u8]) -> Option<u8> {
    bytes.first().copied()
}

pub fn ingest_frame<F: FnMut(NodeUpdate)>(frame: Bytes, mut sink: F) {
    match decode_position_frame(&frame) {
        Ok(updates) => {
            debug!(count = updates.len(), "decoded position frame");
            for u in updates {
                sink(u);
            }
        }
        Err(DecodeError::BadVersion { version, .. }) => {
            warn!(version, "ignoring frame with unsupported protocol version");
        }
        Err(e) => {
            error!(err = %e, "graph position frame decode failed");
        }
    }
}

#[cfg(not(test))]
use std::collections::VecDeque;
#[cfg(not(test))]
use std::sync::{Arc, Mutex};

/// Network → main-thread events for the `/wss` graph stream. The recv pump runs
/// on the tokio runtime and can never touch Godot objects (not `Send`), so it
/// pushes events here; `poll()` drains them on the scene-tree thread. This is a
/// plain transport data type (no Godot deps) so it is available under `cfg(test)`
/// for the transport module's own tests.
pub enum GraphInbound {
    Connected,
    Disconnected,
    Frame(Vec<u8>),
    /// Edge topology from the `initialGraphLoad` text frame.
    Topology(Vec<EdgeSpec>),
}

#[cfg(not(test))]
#[derive(GodotClass)]
#[class(no_init, base = RefCounted)]
pub struct BinaryProtocolClient {
    inbox: Arc<Mutex<VecDeque<GraphInbound>>>,
    handle: Option<crate::transport::ConnHandle>,
    outbound: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Per-node visual identity cache: `node_visuals_updated` fires only when a
    /// node's quantised (community, centrality, anomaly) key changes, so signal
    /// traffic collapses to near-zero once analytics stabilise.
    visuals: std::collections::HashMap<u32, VisualsKey>,
    /// Latest edge topology from `initialGraphLoad`, flattened for Godot.
    edges_flat: Vec<i32>,
    edge_weights: Vec<f32>,
    base: Base<RefCounted>,
}

#[cfg(not(test))]
#[godot_api]
impl BinaryProtocolClient {
    #[signal]
    fn position_updated(node_id: u32, position: Vector3, velocity: Vector3);

    #[signal]
    fn connection_changed(connected: bool);

    /// Fired when a node's visual identity (community colour group, centrality
    /// size bucket, anomaly bucket) first arrives or changes.
    #[signal]
    fn node_visuals_updated(node_id: u32, community_id: u32, centrality: f32, anomaly: f32);

    /// Fired when an `initialGraphLoad` topology lands; read the edge list via
    /// `get_edges()` / `get_edge_weights()`.
    #[signal]
    fn topology_updated(edge_count: u32);

    #[func]
    fn create() -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            inbox: Arc::new(Mutex::new(VecDeque::new())),
            handle: None,
            outbound: None,
            visuals: std::collections::HashMap::new(),
            edges_flat: Vec::new(),
            edge_weights: Vec::new(),
            base,
        })
    }

    /// Connect to the `/wss` graph stream, authenticate via `?token=` query (if
    /// non-empty), subscribe to binary position updates, and queue every frame
    /// for `poll()` to decode on the main thread. Non-blocking: the connection
    /// is established on the tokio runtime. `nostr_secret_hex` (optional)
    /// additionally authenticates the session with a NIP-98 event so mutating
    /// messages (node drag/pin) are accepted by the server.
    #[func]
    fn connect_to_url(&mut self, url: GString, token: GString, nostr_secret_hex: GString) {
        self.visuals.clear();
        let (handle, outbound) = crate::transport::spawn_graph_stream(
            url.to_string(),
            token.to_string(),
            nostr_secret_hex.to_string(),
            self.inbox.clone(),
        );
        self.handle = Some(handle);
        self.outbound = Some(outbound);
    }

    #[func]
    fn close(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
        self.outbound = None;
    }

    /// Drain queued network events on the scene-tree thread, decoding frames and
    /// emitting `position_updated` / `connection_changed`. Call once per frame.
    #[func]
    fn poll(&mut self) {
        let drained: Vec<GraphInbound> = {
            let Ok(mut q) = self.inbox.lock() else {
                return;
            };
            q.drain(..).collect()
        };
        for ev in drained {
            match ev {
                GraphInbound::Connected => {
                    self.base_mut()
                        .emit_signal("connection_changed", &[Variant::from(true)]);
                }
                GraphInbound::Disconnected => {
                    self.base_mut()
                        .emit_signal("connection_changed", &[Variant::from(false)]);
                }
                GraphInbound::Frame(bytes) => self.emit_frame(&bytes),
                GraphInbound::Topology(edges) => {
                    self.edges_flat.clear();
                    self.edge_weights.clear();
                    for e in &edges {
                        self.edges_flat.push(e.source as i32);
                        self.edges_flat.push(e.target as i32);
                        self.edge_weights.push(e.weight);
                    }
                    let count = edges.len() as u32;
                    self.base_mut()
                        .emit_signal("topology_updated", &[Variant::from(count)]);
                }
            }
        }
    }

    /// Flattened edge topology `[src0, tgt0, src1, tgt1, ...]` from the most
    /// recent `initialGraphLoad`. Pair i = (edges[2i], edges[2i+1]).
    #[func]
    fn get_edges(&self) -> PackedInt32Array {
        PackedInt32Array::from(self.edges_flat.as_slice())
    }

    /// Edge weights parallel to the `get_edges()` pair list.
    #[func]
    fn get_edge_weights(&self) -> PackedFloat32Array {
        PackedFloat32Array::from(self.edge_weights.as_slice())
    }

    /// Begin a server-authoritative drag: the server pins the node and every
    /// connected client sees it move. Requires the NIP-98-authenticated
    /// connection (`nostr_secret_hex` in `connect_to_url`).
    #[func]
    fn send_drag_start(&mut self, node_id: u32, position: Vector3) {
        self.send_drag("node_drag_start", node_id, Some(position));
    }

    #[func]
    fn send_drag_update(&mut self, node_id: u32, position: Vector3) {
        self.send_drag("node_drag_update", node_id, Some(position));
    }

    /// End a drag: the server unpins the node and physics resumes ownership.
    #[func]
    fn send_drag_end(&mut self, node_id: u32) {
        self.send_drag("node_drag_end", node_id, None);
    }

    /// Decode an explicit frame (e.g. captured fixture) and emit signals.
    #[func]
    fn ingest(&mut self, payload: PackedByteArray) {
        self.emit_frame(payload.as_slice());
    }
}

#[cfg(not(test))]
impl BinaryProtocolClient {
    fn send_drag(&mut self, msg_type: &str, node_id: u32, position: Option<Vector3>) {
        let Some(tx) = self.outbound.as_ref() else {
            return;
        };
        let data = match position {
            Some(p) => serde_json::json!({
                "nodeId": node_id,
                "position": { "x": p.x, "y": p.y, "z": p.z },
            }),
            None => serde_json::json!({ "nodeId": node_id }),
        };
        let msg = serde_json::json!({ "type": msg_type, "data": data });
        let _ = tx.send(msg.to_string());
    }

    fn emit_frame(&mut self, bytes: &[u8]) {
        let mut emit_buf: Vec<NodeUpdate> = Vec::new();
        ingest_frame(Bytes::copy_from_slice(bytes), |u| emit_buf.push(u));
        for u in emit_buf {
            self.base_mut().emit_signal(
                "position_updated",
                &[
                    Variant::from(u.node_id),
                    Variant::from(Vector3::new(u.position[0], u.position[1], u.position[2])),
                    Variant::from(Vector3::new(u.velocity[0], u.velocity[1], u.velocity[2])),
                ],
            );
            let key = VisualsKey::of(&u);
            if self.visuals.insert(u.node_id, key) != Some(key) {
                self.base_mut().emit_signal(
                    "node_visuals_updated",
                    &[
                        Variant::from(u.node_id),
                        Variant::from(u.community_id),
                        Variant::from(u.centrality),
                        Variant::from(u.anomaly),
                    ],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_frame(records: &[(u32, [f32; 3], [f32; 3])]) -> Vec<u8> {
        let mut out = vec![PROTOCOL_V3];
        for (id, pos, vel) in records {
            out.extend_from_slice(&id.to_le_bytes());
            for v in pos {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for v in vel {
                out.extend_from_slice(&v.to_le_bytes());
            }
            // sssp_dist, sssp_parent, cluster_id, anomaly, community, centrality
            out.extend_from_slice(&f32::INFINITY.to_le_bytes());
            out.extend_from_slice(&(-1i32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&0f32.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&0f32.to_le_bytes());
        }
        out
    }

    #[test]
    fn record_is_52_bytes() {
        let frame = build_frame(&[(1, [0.0; 3], [0.0; 3])]);
        assert_eq!(frame.len(), HEADER_BYTES + NODE_RECORD_BYTES);
    }

    #[test]
    fn decodes_single_node_record() {
        let frame = build_frame(&[(7, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].node_id, 7);
        assert_eq!(decoded[0].kind, NodeKind::Plain);
        assert_eq!(decoded[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(decoded[0].velocity, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn strips_agent_flag_from_node_id() {
        let frame = build_frame(&[(AGENT_NODE_FLAG | 42, [0.0; 3], [0.0; 3])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded[0].node_id, 42);
        assert_eq!(decoded[0].kind, NodeKind::Agent);
    }

    #[test]
    fn strips_knowledge_flag_from_node_id() {
        let frame = build_frame(&[(KNOWLEDGE_NODE_FLAG | 99, [0.0; 3], [0.0; 3])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded[0].node_id, 99);
        assert_eq!(decoded[0].kind, NodeKind::Knowledge);
    }

    #[test]
    fn classifies_ontology_subtypes() {
        for (flag, want) in [
            (ONTOLOGY_CLASS_FLAG, NodeKind::OntologyClass),
            (ONTOLOGY_INDIVIDUAL_FLAG, NodeKind::OntologyIndividual),
            (ONTOLOGY_PROPERTY_FLAG, NodeKind::OntologyProperty),
        ] {
            let frame = build_frame(&[(flag | 5, [0.0; 3], [0.0; 3])]);
            let decoded = decode_position_frame(&frame).unwrap();
            assert_eq!(decoded[0].node_id, 5);
            assert_eq!(decoded[0].kind, want);
        }
    }

    #[test]
    fn rejects_bad_version() {
        let frame = vec![0x99, 0, 0, 0];
        assert!(matches!(
            decode_position_frame(&frame),
            Err(DecodeError::BadVersion { .. })
        ));
    }

    #[test]
    fn rejects_misaligned_payload() {
        let mut frame = vec![PROTOCOL_V3];
        frame.extend_from_slice(&[0u8; 51]);
        assert!(matches!(
            decode_position_frame(&frame),
            Err(DecodeError::Misaligned { .. })
        ));
    }

    #[test]
    fn rejects_too_short_for_header() {
        assert!(matches!(
            decode_position_frame(&[]),
            Err(DecodeError::TooShort { .. })
        ));
    }

    #[test]
    fn ingest_fires_callback_per_record() {
        let frame = build_frame(&[(1, [0.0; 3], [0.0; 3]), (2, [1.0, 0.0, 0.0], [0.0; 3])]);
        let mut count = 0usize;
        ingest_frame(Bytes::from(frame), |_| count += 1);
        assert_eq!(count, 2);
    }

    #[test]
    fn decodes_multi_node_frame() {
        let records = [
            (1u32, [1.0f32, 2.0, 3.0], [0.1f32, 0.2, 0.3]),
            (2, [4.0, 5.0, 6.0], [0.4, 0.5, 0.6]),
            (3, [7.0, 8.0, 9.0], [0.7, 0.8, 0.9]),
        ];
        let frame = build_frame(&records);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 3);
        for (i, (id, pos, vel)) in records.iter().enumerate() {
            assert_eq!(decoded[i].node_id, *id);
            assert_eq!(decoded[i].position, *pos);
            assert_eq!(decoded[i].velocity, *vel);
        }
    }

    #[test]
    fn zero_node_frame_is_valid() {
        let frame = vec![PROTOCOL_V3];
        let decoded = decode_position_frame(&frame).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn frame_version_returns_first_byte() {
        assert_eq!(frame_version(&[PROTOCOL_V3, 0x00, 0x01]), Some(PROTOCOL_V3));
        assert_eq!(frame_version(&[0x04]), Some(0x04));
        assert_eq!(frame_version(&[]), None);
    }

    #[test]
    fn ingest_frame_bad_version_is_silent() {
        let frame = vec![0x99, 0x00, 0x00, 0x00];
        let mut count = 0usize;
        ingest_frame(Bytes::from(frame), |_| count += 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn parse_preserves_negative_values() {
        let frame = build_frame(&[(42, [-1.5, -2.5, -3.5], [-0.1, -0.2, -0.3])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].node_id, 42);
        assert_eq!(decoded[0].position, [-1.5, -2.5, -3.5]);
        assert_eq!(decoded[0].velocity, [-0.1, -0.2, -0.3]);
    }

    #[test]
    fn max_node_id_within_mask() {
        let frame = build_frame(&[(NODE_ID_MASK, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].node_id, NODE_ID_MASK);
    }

    fn build_record_with_tail(
        id: u32,
        sssp_distance: f32,
        sssp_parent: i32,
        cluster_id: u32,
        anomaly: f32,
        community_id: u32,
        centrality: f32,
    ) -> Vec<u8> {
        let mut out = vec![PROTOCOL_V3];
        out.extend_from_slice(&id.to_le_bytes());
        for v in [1.0f32, 2.0, 3.0, 0.1, 0.2, 0.3] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&sssp_distance.to_le_bytes());
        out.extend_from_slice(&sssp_parent.to_le_bytes());
        out.extend_from_slice(&cluster_id.to_le_bytes());
        out.extend_from_slice(&anomaly.to_le_bytes());
        out.extend_from_slice(&community_id.to_le_bytes());
        out.extend_from_slice(&centrality.to_le_bytes());
        out
    }

    #[test]
    fn analytics_tail_is_decoded() {
        let frame = build_record_with_tail(42, 7.5, 41, 9, 0.25, 314, 0.875);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1);
        let u = decoded[0];
        assert_eq!(u.sssp_distance, 7.5);
        assert_eq!(u.sssp_parent, 41);
        assert_eq!(u.cluster_id, 9);
        assert_eq!(u.anomaly, 0.25);
        assert_eq!(u.community_id, 314);
        assert_eq!(u.centrality, 0.875);
    }

    #[test]
    fn visuals_key_quantises_and_detects_change() {
        let frame = build_record_with_tail(1, 0.0, -1, 0, 0.5, 6, 0.5);
        let u = decode_position_frame(&frame).unwrap()[0];
        let k1 = VisualsKey::of(&u);

        // Sub-bucket centrality wiggle (< 1/64) must NOT change the key.
        let frame2 = build_record_with_tail(1, 0.0, -1, 0, 0.5, 6, 0.505);
        let u2 = decode_position_frame(&frame2).unwrap()[0];
        assert_eq!(k1, VisualsKey::of(&u2));

        // Community change must change the key.
        let frame3 = build_record_with_tail(1, 0.0, -1, 0, 0.5, 7, 0.5);
        let u3 = decode_position_frame(&frame3).unwrap()[0];
        assert_ne!(k1, VisualsKey::of(&u3));

        // Out-of-range values clamp instead of overflowing the bucket byte.
        let frame4 = build_record_with_tail(1, 0.0, -1, 0, 9.0, 6, -3.0);
        let u4 = decode_position_frame(&frame4).unwrap()[0];
        let k4 = VisualsKey::of(&u4);
        assert_eq!(k4.centrality_bucket, 0);
        assert_eq!(k4.anomaly_bucket, 15);
    }

    #[test]
    fn parse_initial_graph_load_extracts_edges() {
        let text = r#"{"type":"initialGraphLoad","nodes":[],"edges":[
            {"id":"a","source_id":1,"target_id":2,"weight":2.5},
            {"id":"b","source_id":3,"target_id":4}
        ],"timestamp":1}"#;
        let edges = parse_initial_graph_load(text).unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(
            edges[0],
            EdgeSpec {
                source: 1,
                target: 2,
                weight: 2.5
            }
        );
        // Missing weight defaults to 1.0.
        assert_eq!(edges[1].weight, 1.0);
    }

    #[test]
    fn parse_initial_graph_load_masks_flag_bits() {
        // Wire ids can carry type flags in bits 26-31; topology must mask them
        // so edge endpoints match the masked ids from the binary stream.
        let raw = 0x4000_0005u32; // knowledge flag + id 5
        let text = format!(
            r#"{{"type":"initialGraphLoad","nodes":[],"edges":[{{"id":"x","source_id":{raw},"target_id":6}}],"timestamp":1}}"#
        );
        let edges = parse_initial_graph_load(&text).unwrap();
        assert_eq!(edges[0].source, 5);
    }

    #[test]
    fn parse_initial_graph_load_rejects_other_messages() {
        assert!(parse_initial_graph_load(r#"{"type":"pong"}"#).is_none());
        assert!(parse_initial_graph_load("not json").is_none());
        assert!(
            parse_initial_graph_load(r#"{"type":"initialDataInfo","message":"x"}"#).is_none()
        );
    }

    #[test]
    fn parse_agent_identities_extracts_did_and_masks_flags() {
        let did_a = format!("did:nostr:{}", "a".repeat(64));
        let did_b = format!("did:nostr:{}", "b".repeat(64));
        // node 1 has snake_case did; node with agent flag uses camelCase and a
        // numeric-string id; a third node has no DID and is dropped.
        let agent_flag_id = 0x8000_0000u32 | 42;
        let text = format!(
            r#"{{"type":"initialGraphLoad","nodes":[
                {{"id":1,"did_nostr":"{did_a}"}},
                {{"id":"{agent_flag_id}","didNostr":"{did_b}"}},
                {{"id":3}}
            ],"edges":[],"timestamp":1}}"#
        );
        let ids = parse_agent_identities(&text);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], (1, did_a));
        // flag bits masked off -> node id 42
        assert_eq!(ids[1], (42, did_b));
    }

    #[test]
    fn parse_agent_identities_ignores_empty_and_non_initial() {
        assert!(parse_agent_identities(r#"{"type":"pong"}"#).is_empty());
        assert!(parse_agent_identities("not json").is_empty());
        // present-but-empty DID is not an identity
        assert!(parse_agent_identities(
            r#"{"type":"initialGraphLoad","nodes":[{"id":1,"did_nostr":""}],"edges":[],"timestamp":1}"#
        )
        .is_empty());
    }
}
