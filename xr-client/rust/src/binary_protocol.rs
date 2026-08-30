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

/// Protocol V5 wraps a V3 body with an 8-byte broadcast sequence number:
/// `[0x05][u64 broadcast_seq][V3 node records]`.
pub const PROTOCOL_V5: u8 = 0x05;
const V5_SEQ_BYTES: usize = 8;
/// Bytes per node record in a V3 frame.
pub const NODE_RECORD_BYTES: usize = 52;
const HEADER_BYTES: usize = 1;

/// Hard-reject bound (metres): a position component beyond this is treated as a
/// corrupt/hostile frame and the record is dropped. Server physics bounds are
/// ±400 m, so 10 km leaves ample slack for legitimate overshoot.
const WORLD_LIMIT_M: f32 = 10_000.0;

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
    // V5 = [version][8-byte broadcast_seq][V3 node records]; the sequence number
    // is a broadcast-ordering aid we don't need, so skip it and parse as V3.
    let body_offset = match version {
        PROTOCOL_V3 => HEADER_BYTES,
        PROTOCOL_V5 => {
            let need = HEADER_BYTES + V5_SEQ_BYTES;
            if bytes.len() < need {
                return Err(DecodeError::TooShort {
                    need,
                    got: bytes.len(),
                });
            }
            need
        }
        _ => {
            return Err(DecodeError::BadVersion {
                version,
                expected: PROTOCOL_V3,
            });
        }
    };
    let payload = &bytes[body_offset..];
    if !payload.len().is_multiple_of(NODE_RECORD_BYTES) {
        return Err(DecodeError::Misaligned { len: payload.len() });
    }
    let count = payload.len() / NODE_RECORD_BYTES;
    let mut out = Vec::with_capacity(count);
    for chunk in payload.chunks_exact(NODE_RECORD_BYTES) {
        // Drop records the server should never emit (NaN/Inf, absurd magnitude):
        // a single poisoned position propagates through the fit AABB and edge
        // layout, so reject at the decode boundary rather than downstream.
        if let Some(rec) = sanitize_node_update(parse_node_record(chunk)) {
            out.push(rec);
        }
    }
    Ok(out)
}

/// Validate one decoded record. Returns `None` (drop it) when a position or
/// velocity component is non-finite or a position exceeds [`WORLD_LIMIT_M`];
/// otherwise clamps positions to [`WORLD_CLAMP_M`] and neutralises any
/// non-finite analytics-tail scalar so colour/size mapping stays well-defined.
fn sanitize_node_update(mut u: NodeUpdate) -> Option<NodeUpdate> {
    for c in u.position.iter().chain(u.velocity.iter()) {
        if !c.is_finite() {
            return None;
        }
    }
    // Hard-reject corrupt magnitudes only. Do NOT clamp survivors: the live
    // layout legitimately overshoots the physics volume (observed r_max ~3200),
    // and clamping collapses distinct outliers onto the bounds-cube faces —
    // which renders as an edge fan converging on the clamped face.
    for c in u.position.iter() {
        if c.abs() > WORLD_LIMIT_M {
            return None;
        }
    }
    if !u.anomaly.is_finite() {
        u.anomaly = 0.0;
    }
    if !u.centrality.is_finite() {
        u.centrality = 0.0;
    }
    if !u.sssp_distance.is_finite() {
        u.sssp_distance = 0.0;
    }
    Some(u)
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
    /// Any other JSON text frame on the multiplexed `/wss` socket (e.g.
    /// `broker:new_case`, `broker:case_decided`). Forwarded verbatim to GDScript
    /// via `text_message`; the scene layer routes by the envelope `type`.
    Text(String),
}

/// Classify an inbound text frame from the multiplexed `/wss` graph socket.
/// The `initialGraphLoad` topology is decoded into [`GraphInbound::Topology`];
/// every other JSON envelope (broker events, acks, info frames) is forwarded
/// verbatim as [`GraphInbound::Text`] for the scene layer to route by `type`.
/// Pure (no Godot deps) so it is unit-testable under `cfg(test)`.
pub fn classify_graph_text(text: &str) -> GraphInbound {
    match parse_initial_graph_load(text) {
        Some(edges) => GraphInbound::Topology(edges),
        None => GraphInbound::Text(text.to_owned()),
    }
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
    /// Hot-path render store: owns node targets/positions and packs the MultiMesh
    /// instance buffers so GDScript never loops per-instance (PRD-008 perf).
    store: crate::render_store::RenderStore,
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

    /// Fired for every non-topology JSON text frame on the multiplexed `/wss`
    /// socket (e.g. `broker:new_case`). The scene layer parses the envelope and
    /// routes by its `type`; the Rust side stays transport-only.
    #[signal]
    fn text_message(json: GString);

    #[func]
    fn create() -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            inbox: Arc::new(Mutex::new(VecDeque::new())),
            handle: None,
            outbound: None,
            visuals: std::collections::HashMap::new(),
            edges_flat: Vec::new(),
            edge_weights: Vec::new(),
            store: crate::render_store::RenderStore::new(),
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
        self.store.clear();
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
                GraphInbound::Text(json) => {
                    self.base_mut()
                        .emit_signal("text_message", &[Variant::from(GString::from(json))]);
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

    // --- hot-path render API (PRD-008): the store owns positions; GDScript calls
    // these per frame instead of looping per instance. ------------------------

    /// Ease every render position toward its streamed target. `grab_id` < 0 means
    /// no grab; otherwise that node is pinned to `grab_pos` (server space) so it
    /// tracks the hand. Call once per poll (per frame).
    #[func]
    fn hunt(&mut self, ease: f32, grab_id: i64, grab_pos: Vector3) {
        let gid = if grab_id < 0 { None } else { Some(grab_id as u32) };
        self.store.hunt(ease, gid, [grab_pos.x, grab_pos.y, grab_pos.z]);
    }

    /// Pack the node MultiMesh buffer for the drawn `ids` (20 floats/instance:
    /// transform + colour + custom). `scale_comp` folds GraphRoot scale + the HUD
    /// node-size factor; size eases `size_lo..size_hi` by sqrt(centrality-norm).
    #[func]
    fn build_node_buffer(&mut self, ids: PackedInt32Array, scale_comp: f32, size_lo: f32, size_hi: f32) -> PackedFloat32Array {
        let v = self.store.build_node_buffer(ids.as_slice(), scale_comp, size_lo, size_hi);
        PackedFloat32Array::from(v.as_slice())
    }

    /// Pack the edge MultiMesh buffer for the ranked `pairs` (12 floats/instance).
    /// Only edges with both endpoints in the last node buffer's drawn set survive.
    #[func]
    fn build_edge_buffer(&mut self, pairs: PackedInt32Array, radius_comp: f32) -> PackedFloat32Array {
        let v = self.store.build_edge_buffer(pairs.as_slice(), radius_comp);
        PackedFloat32Array::from(v.as_slice())
    }

    /// Drawn node ids from the last `build_node_buffer`, for the interaction ray.
    #[func]
    fn get_render_ids(&self) -> PackedInt32Array {
        let ids: Vec<i32> = self.store.render_ids().iter().map(|&id| id as i32).collect();
        PackedInt32Array::from(ids.as_slice())
    }

    /// Drawn render positions (server space) parallel to `get_render_ids()`.
    #[func]
    fn get_render_positions(&self) -> PackedVector3Array {
        let mut out = PackedVector3Array::new();
        for p in self.store.render_positions() {
            out.push(Vector3::new(p[0], p[1], p[2]));
        }
        out
    }

    /// All node ids currently in the store (for the LOD/topology selection).
    #[func]
    fn get_node_ids(&self) -> PackedInt32Array {
        let ids: Vec<i32> = self.store.all_ids().iter().map(|&id| id as i32).collect();
        PackedInt32Array::from(ids.as_slice())
    }

    /// Node count in the store.
    #[func]
    fn node_count(&self) -> i64 {
        self.store.len() as i64
    }

    /// Current render position of a node (ZERO if unknown) — used at grab start.
    #[func]
    fn node_position(&self, node_id: u32) -> Vector3 {
        let p = self.store.position_of(node_id);
        Vector3::new(p[0], p[1], p[2])
    }

    /// Per-axis percentile AABB `[minx,miny,minz,maxx,maxy,maxz]` over render
    /// positions, excluding `exclude_id` (< 0 = none). Empty array when no nodes.
    #[func]
    fn render_aabb(&self, lo_q: f32, hi_q: f32, exclude_id: i64) -> PackedFloat32Array {
        let excl = if exclude_id < 0 { None } else { Some(exclude_id as u32) };
        match self.store.aabb_percentile(lo_q, hi_q, excl) {
            Some(bb) => PackedFloat32Array::from(bb.as_slice()),
            None => PackedFloat32Array::new(),
        }
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
            // The Rust render store now owns positions (hunted per poll, packed into
            // MultiMesh buffers) — no per-node position_updated signal, which at 13k
            // nodes was a ~13k-emit-per-frame storm across the gdext boundary.
            self.store
                .upsert(u.node_id, u.position, u.community_id, u.anomaly, u.centrality);
            // Visuals still surface to GDScript (throttled to quantised-key changes)
            // so the scene keeps its centrality mirror for the LOD/edge selection.
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
    fn decodes_v5_frame_by_skipping_broadcast_seq() {
        // Take a valid single-record V3 frame and rewrap it as V5.
        let v3 = build_frame(&[(7, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3])]);
        let mut v5 = vec![PROTOCOL_V5];
        v5.extend_from_slice(&42u64.to_le_bytes());
        v5.extend_from_slice(&v3[HEADER_BYTES..]);
        let v3_decoded = decode_position_frame(&v3).unwrap();
        let v5_decoded = decode_position_frame(&v5).unwrap();
        assert_eq!(v3_decoded.len(), v5_decoded.len());
        assert_eq!(v3_decoded[0].node_id, v5_decoded[0].node_id);
    }

    #[test]
    fn rejects_truncated_v5_header() {
        let frame = vec![PROTOCOL_V5, 1, 2, 3];
        assert!(matches!(
            decode_position_frame(&frame),
            Err(DecodeError::TooShort { .. })
        ));
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
    fn drops_record_with_nan_position() {
        let frame = build_frame(&[
            (1, [f32::NAN, 0.0, 0.0], [0.0; 3]),
            (2, [1.0, 2.0, 3.0], [0.0; 3]),
        ]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1, "NaN record must be dropped");
        assert_eq!(decoded[0].node_id, 2);
    }

    #[test]
    fn drops_record_with_infinite_velocity() {
        let frame = build_frame(&[(1, [0.0; 3], [f32::INFINITY, 0.0, 0.0])]);
        assert!(decode_position_frame(&frame).unwrap().is_empty());
    }

    #[test]
    fn drops_record_beyond_world_limit() {
        let frame = build_frame(&[(1, [WORLD_LIMIT_M * 2.0, 0.0, 0.0], [0.0; 3])]);
        assert!(decode_position_frame(&frame).unwrap().is_empty());
    }

    #[test]
    fn preserves_legitimate_outlier_positions() {
        // Layout overshoot beyond the physics volume is legitimate; clamping it
        // used to collapse outliers onto the bounds cube (rendered as an edge
        // fan converging on the clamped face).
        let frame = build_frame(&[(1, [900.0, -9000.0, 0.0], [0.0; 3])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].position[0], 900.0);
        assert_eq!(decoded[0].position[1], -9000.0);
    }

    #[test]
    fn neutralises_nonfinite_analytics_tail() {
        // build_frame writes sssp_distance = +Inf; sanitize must zero it, not drop.
        let frame = build_frame(&[(1, [0.0; 3], [0.0; 3])]);
        let decoded = decode_position_frame(&frame).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].sssp_distance, 0.0);
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
    fn classify_graph_text_topology_vs_text() {
        // initialGraphLoad → Topology (decoded edges).
        let topo = r#"{"type":"initialGraphLoad","nodes":[],"edges":[
            {"id":"a","source_id":1,"target_id":2,"weight":2.5}
        ],"timestamp":1}"#;
        match classify_graph_text(topo) {
            GraphInbound::Topology(edges) => assert_eq!(edges.len(), 1),
            other => panic!("expected Topology, got {:?}", std::mem::discriminant(&other)),
        }

        // broker:new_case → forwarded verbatim as Text for the scene layer.
        let broker = r#"{"type":"broker:new_case","channel":"inbox","payload":{"caseId":"case-9","title":"Merge","category":"ontology"}}"#;
        match classify_graph_text(broker) {
            GraphInbound::Text(json) => {
                assert_eq!(json, broker);
                assert!(json.contains("broker:new_case"));
            }
            other => panic!("expected Text, got {:?}", std::mem::discriminant(&other)),
        }

        // An ack/info frame is also forwarded as Text (never silently dropped).
        match classify_graph_text(r#"{"type":"pong"}"#) {
            GraphInbound::Text(json) => assert_eq!(json, r#"{"type":"pong"}"#),
            other => panic!("expected Text, got {:?}", std::mem::discriminant(&other)),
        }
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
