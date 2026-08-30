//! Hot-path graph render store (PRD-008 perf endgame).
//!
//! At full density (13,164 nodes / 145,692 edges) the previous GDScript per-frame
//! path — `_hunt_positions` over a 13k Dictionary plus `_update_multimesh` /
//! `_update_edge_multimesh` issuing ~100k `set_instance_*` calls — cost ~90 ms and
//! held the client at 11 fps. This module owns the position store in Rust, runs the
//! per-poll hunt (lerp render→target) in tight `Vec` loops, and packs the two
//! `MultiMesh` instance buffers as flat `PackedFloat32Array`s so GDScript does a
//! single `.buffer =` assignment per phase instead of one API call per instance.
//!
//! Pure (no Godot deps) so the buffer layout, hunt convergence, grabbed-node
//! override and AABB maths are unit-tested directly. `BinaryProtocolClient` wraps
//! it with thin `#[func]` adapters that convert to/from Godot packed arrays.
//!
//! ## MultiMesh buffer layout (must match GraphScene.tscn format flags)
//! * Nodes: `transform_format = TRANSFORM_3D` + `use_colors` + `use_custom_data`
//!   → **20 floats/instance**: 12 transform (row-major 3×4) + 4 colour (RGBA) +
//!   4 custom (INSTANCE_CUSTOM: cen_norm,0,0,1).
//! * Edges: `TRANSFORM_3D` only → **12 floats/instance** (no colour/custom; the
//!   edge shader is uniform-tinted).
//!
//! The 12-float transform block is Godot's row-major 3×4: for basis columns
//! `c0,c1,c2` and origin `o` it is `[c0.x,c1.x,c2.x,o.x, c0.y,c1.y,c2.y,o.y,
//! c0.z,c1.z,c2.z,o.z]`.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// One `search_labels` match. `Ord` is defined so that a GREATER value is a WORSE
/// result (higher rank tier, then lower centrality, then larger id) — a max-heap
/// of these therefore has the worst-so-far on top, which is exactly what a bounded
/// top-k keep-the-best-`max` selection pops. `into_sorted_vec()` then yields the
/// survivors best-first.
#[derive(Clone, Copy)]
struct LabelHit {
    /// 0 = prefix match, 1 = substring match (lower is better).
    rank: u8,
    centrality: f32,
    id: u32,
}

impl LabelHit {
    /// Worse-than ordering: bigger rank is worse; for equal rank, lower centrality
    /// is worse; for equal centrality, larger id is worse (stable tie-break).
    fn worseness(&self, other: &Self) -> Ordering {
        self.rank
            .cmp(&other.rank)
            .then_with(|| {
                // lower centrality = worse = "greater" in this ordering.
                other
                    .centrality
                    .partial_cmp(&self.centrality)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| self.id.cmp(&other.id))
    }
}

impl Ord for LabelHit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.worseness(other)
    }
}
impl PartialOrd for LabelHit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for LabelHit {
    fn eq(&self, other: &Self) -> bool {
        self.worseness(other) == Ordering::Equal
    }
}
impl Eq for LabelHit {}

/// Proximity-label metadata for one node (kept small — no full metadata map).
#[derive(Default, Clone)]
struct NodeMeta {
    /// Stable id used as the narrativegoldmine page slug source (falls back to the
    /// slugified label when empty).
    meta_id: String,
    label: String,
    /// Lowercased `label`, precomputed at `set_meta` time so keystroke-driven
    /// `search_labels` never re-lowercases every label per call.
    label_lower: String,
    node_type: String,
    detail: String,
}

/// Floats per node instance in the MultiMesh buffer (12 transform + 4 colour + 4 custom).
pub const NODE_STRIDE: usize = 20;
/// Floats per edge instance (12 transform only).
pub const EDGE_STRIDE: usize = 12;

/// HSV→RGB (all components in `[0,1]`), matching Godot `Color.from_hsv`.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h6 = (h - h.floor()) * 6.0;
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i.rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Deterministic per-node colour — a direct port of `graph_scene.gd::_community_color`.
/// Golden-ratio hue walk keyed by community (or node id when community is 0), warm
/// red blend for anomalies. The hue product is done in f64 to match GDScript's
/// double-precision `float` so colours are bit-for-bit consistent with the old path.
pub fn community_color(community_id: u32, anomaly: f32, node_id: u32) -> [f32; 4] {
    let key = if community_id != 0 { community_id } else { node_id };
    let hue = ((key as f64) * 0.618_033_988_75).fract() as f32;
    let mut rgb = hsv_to_rgb(hue, 0.6, 0.95);
    if anomaly > 0.5 {
        let t = ((anomaly - 0.5) * 2.0).clamp(0.0, 0.85);
        let warn = [1.0, 0.15, 0.1];
        for c in 0..3 {
            rgb[c] = rgb[c] + (warn[c] - rgb[c]) * t;
        }
    }
    [rgb[0], rgb[1], rgb[2], 1.0]
}

/// Row-major 3×4 transform for a uniform-scaled, axis-aligned node at `pos`.
fn node_transform12(size: f32, pos: [f32; 3]) -> [f32; 12] {
    [
        size, 0.0, 0.0, pos[0],
        0.0, size, 0.0, pos[1],
        0.0, 0.0, size, pos[2],
    ]
}

/// Row-major 3×4 transform for a unit Y-cylinder rotated onto the edge `a→b`,
/// scaled `radius` across and to the span along Y, centred at the midpoint. Returns
/// `None` for a degenerate (near-zero length) edge. Ports the `scaled_local` basis
/// and the (anti)parallel-to-UP degenerate handling from `_update_edge_multimesh`.
fn edge_transform12(a: [f32; 3], b: [f32; 3], radius: f32) -> Option<[f32; 12]> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len < 0.001 {
        return None;
    }
    let dir = [d[0] / len, d[1] / len, d[2] / len];
    let dp = dir[1]; // UP·dir, UP = (0,1,0)
    // Rotation basis columns c0,c1,c2 mapping local Y onto `dir`.
    let (mut c0, mut c1, mut c2): ([f32; 3], [f32; 3], [f32; 3]);
    if dp > 0.9999 {
        c0 = [1.0, 0.0, 0.0];
        c1 = [0.0, 1.0, 0.0];
        c2 = [0.0, 0.0, 1.0];
    } else if dp < -0.9999 {
        // 180° about X: Y→-Y, Z→-Z.
        c0 = [1.0, 0.0, 0.0];
        c1 = [0.0, -1.0, 0.0];
        c2 = [0.0, 0.0, -1.0];
    } else {
        // axis = normalize(UP × dir) = normalize((dir.z, 0, -dir.x)); angle = acos(dp).
        let ax = [dir[2], 0.0, -dir[0]];
        let al = (ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2]).sqrt();
        let x = ax[0] / al;
        let y = ax[1] / al;
        let z = ax[2] / al;
        let c = dp.clamp(-1.0, 1.0);
        let s = (1.0 - c * c).max(0.0).sqrt();
        let omc = 1.0 - c;
        // Rodrigues rotation matrix, columns.
        c0 = [c + x * x * omc, y * x * omc + z * s, z * x * omc - y * s];
        c1 = [x * y * omc - z * s, c + y * y * omc, z * y * omc + x * s];
        c2 = [x * z * omc + y * s, y * z * omc - x * s, c + z * z * omc];
    }
    // scaled_local: scale each basis column along the cylinder's own axes.
    for k in 0..3 {
        c0[k] *= radius;
        c1[k] *= len;
        c2[k] *= radius;
    }
    let o = [a[0] + d[0] * 0.5, a[1] + d[1] * 0.5, a[2] + d[2] * 0.5];
    Some([
        c0[0], c1[0], c2[0], o[0],
        c0[1], c1[1], c2[1], o[1],
        c0[2], c1[2], c2[2], o[2],
    ])
}

/// Linear-interpolated percentile of an unsorted slice (q in `[0,1]`); mirrors
/// `graph_scene.gd::_percentile`. Copies before sorting.
fn percentile(values: &[f32], q: f32) -> f32 {
    let n = values.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return values[0];
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = q.clamp(0.0, 1.0) * (n as f32 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = rank - lo as f32;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}

/// Dense, compact-indexed node store. Ids are assigned a slot on first sight; all
/// per-node arrays are parallel-indexed by slot for cache-friendly frame loops.
#[derive(Default)]
pub struct RenderStore {
    id_index: HashMap<u32, usize>,
    ids: Vec<u32>,
    targets: Vec<[f32; 3]>,
    positions: Vec<[f32; 3]>,
    centrality: Vec<f32>,
    color: Vec<[f32; 4]>,
    centrality_max: f32,
    // Drawn subset from the most recent build_node_buffer — the edge builder filters
    // against this so an edge only renders when BOTH endpoints are on screen.
    drawn: HashSet<u32>,
    render_ids: Vec<u32>,
    render_positions: Vec<[f32; 3]>,
    // Node label metadata (from initialGraphLoad), keyed by id — independent of the
    // position store so it survives whichever arrives first.
    meta: HashMap<u32, NodeMeta>,
}

impl RenderStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.id_index.clear();
        self.ids.clear();
        self.targets.clear();
        self.positions.clear();
        self.centrality.clear();
        self.color.clear();
        self.centrality_max = 0.0;
        self.drawn.clear();
        self.render_ids.clear();
        self.render_positions.clear();
        self.meta.clear();
    }

    /// Store a node's label metadata (from initialGraphLoad).
    pub fn set_meta(&mut self, node_id: u32, meta_id: String, label: String, node_type: String, detail: String) {
        let label_lower = label.to_lowercase();
        self.meta.insert(
            node_id,
            NodeMeta {
                meta_id,
                label,
                label_lower,
                node_type,
                detail,
            },
        );
    }

    /// Primary label for a node (empty string if unknown).
    pub fn label_of(&self, node_id: u32) -> String {
        self.meta.get(&node_id).map(|m| m.label.clone()).unwrap_or_default()
    }

    /// Slug source (metadata_id) for a node (empty if unknown); the double-click
    /// document view slugifies this, falling back to the label.
    pub fn meta_id_of(&self, node_id: u32) -> String {
        self.meta.get(&node_id).map(|m| m.meta_id.clone()).unwrap_or_default()
    }

    /// Secondary detail line: node type and one metadata value, joined by " · ".
    /// Empty when neither is known.
    pub fn detail_of(&self, node_id: u32) -> String {
        match self.meta.get(&node_id) {
            None => String::new(),
            Some(m) => {
                let mut parts: Vec<&str> = Vec::new();
                if !m.node_type.is_empty() {
                    parts.push(&m.node_type);
                }
                if !m.detail.is_empty() {
                    parts.push(&m.detail);
                }
                parts.join(" · ")
            }
        }
    }

    /// Centrality for a node id (0.0 if unknown) — used to rank label search hits.
    fn centrality_of(&self, node_id: u32) -> f32 {
        self.id_index
            .get(&node_id)
            .map(|&s| self.centrality[s])
            .unwrap_or(0.0)
    }

    /// Case-insensitive label search over the node metadata. Ranking:
    /// prefix matches (label starts with `query`) rank before substring matches;
    /// within each tier, higher centrality ranks first (stable id order breaks
    /// remaining ties). Empty `query` or `max == 0` returns an empty vec.
    /// Returns at most `max` node ids.
    pub fn search_labels(&self, query: &str, max: usize) -> Vec<u32> {
        if max == 0 {
            return Vec::new();
        }
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        // Bounded top-k: keep at most `max` best hits in a max-heap (worst on top),
        // so the working set never exceeds `max` and no full sort of all matches is
        // needed. Labels are matched against the cached lowercase copy — no
        // per-call allocation per label.
        let mut heap: BinaryHeap<LabelHit> = BinaryHeap::with_capacity(max + 1);
        for (&id, meta) in &self.meta {
            let hay = &meta.label_lower;
            if hay.is_empty() {
                continue;
            }
            let rank = if hay.starts_with(&needle) {
                0u8
            } else if hay.contains(&needle) {
                1u8
            } else {
                continue;
            };
            let hit = LabelHit {
                rank,
                centrality: self.centrality_of(id),
                id,
            };
            if heap.len() < max {
                heap.push(hit);
            } else if let Some(worst) = heap.peek() {
                // Replace the current worst only if this hit is strictly better.
                if hit.worseness(worst) == Ordering::Less {
                    heap.pop();
                    heap.push(hit);
                }
            }
        }
        // into_sorted_vec yields ascending by Ord (= best-first here).
        heap.into_sorted_vec().into_iter().map(|h| h.id).collect()
    }

    /// Ids of nodes within `radius` (server space) of `center`, nearest first,
    /// capped at `max`. A plain O(N) scan — fine at 13k for a few-Hz query.
    pub fn nodes_near(&self, center: [f32; 3], radius: f32, max: usize) -> Vec<u32> {
        let r2 = radius * radius;
        let mut hits: Vec<(f32, u32)> = Vec::new();
        for slot in 0..self.ids.len() {
            let p = self.positions[slot];
            let dx = p[0] - center[0];
            let dy = p[1] - center[1];
            let dz = p[2] - center[2];
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 <= r2 {
                hits.push((d2, self.ids[slot]));
            }
        }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(max);
        hits.into_iter().map(|(_, id)| id).collect()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Insert or update a node from a decoded frame. New ids seed their render
    /// position at the target (no ease-in from origin); existing ids only move
    /// their target — the hunt eases the render position toward it.
    pub fn upsert(&mut self, node_id: u32, position: [f32; 3], community_id: u32, anomaly: f32, centrality: f32) {
        let color = community_color(community_id, anomaly, node_id);
        match self.id_index.get(&node_id).copied() {
            Some(slot) => {
                self.targets[slot] = position;
                self.centrality[slot] = centrality;
                self.color[slot] = color;
            }
            None => {
                let slot = self.ids.len();
                self.id_index.insert(node_id, slot);
                self.ids.push(node_id);
                self.targets.push(position);
                self.positions.push(position);
                self.centrality.push(centrality);
                self.color.push(color);
            }
        }
        if centrality > self.centrality_max {
            self.centrality_max = centrality;
        }
    }

    /// Ease every render position toward its target. The grabbed node (if any) is
    /// pinned to `grab_pos` (server space) so it tracks the hand exactly.
    pub fn hunt(&mut self, ease: f32, grab_id: Option<u32>, grab_pos: [f32; 3]) {
        for slot in 0..self.ids.len() {
            if Some(self.ids[slot]) == grab_id {
                self.positions[slot] = grab_pos;
                self.targets[slot] = grab_pos;
                continue;
            }
            let p = self.positions[slot];
            let t = self.targets[slot];
            self.positions[slot] = [
                p[0] + (t[0] - p[0]) * ease,
                p[1] + (t[1] - p[1]) * ease,
                p[2] + (t[2] - p[2]) * ease,
            ];
        }
    }

    /// Pack the node MultiMesh buffer for the drawn `ids` (in order). Records the
    /// drawn set + render positions for the edge builder and the interaction ray.
    /// Ids not present in the store are skipped (buffer shrinks accordingly).
    pub fn build_node_buffer(&mut self, ids: &[i32], scale_comp: f32, size_lo: f32, size_hi: f32) -> Vec<f32> {
        self.drawn.clear();
        self.render_ids.clear();
        self.render_positions.clear();
        let mut buf = Vec::with_capacity(ids.len() * NODE_STRIDE);
        for &raw in ids {
            let id = raw as u32;
            let Some(&slot) = self.id_index.get(&id) else {
                continue;
            };
            let pos = self.positions[slot];
            let cen_norm = if self.centrality_max > 0.0 {
                (self.centrality[slot] / self.centrality_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let size = scale_comp * (size_lo + (size_hi - size_lo) * cen_norm.sqrt());
            buf.extend_from_slice(&node_transform12(size, pos));
            buf.extend_from_slice(&self.color[slot]);
            buf.extend_from_slice(&[cen_norm, 0.0, 0.0, 1.0]);
            self.drawn.insert(id);
            self.render_ids.push(id);
            self.render_positions.push(pos);
        }
        buf
    }

    /// Pack the edge MultiMesh buffer for the ranked `pairs`. An edge is emitted
    /// only when both endpoints are in the drawn set and non-degenerate.
    pub fn build_edge_buffer(&self, pairs: &[i32], radius_comp: f32) -> Vec<f32> {
        let mut buf = Vec::new();
        let n = pairs.len() / 2;
        for i in 0..n {
            let s = pairs[i * 2] as u32;
            let t = pairs[i * 2 + 1] as u32;
            if !self.drawn.contains(&s) || !self.drawn.contains(&t) {
                continue;
            }
            let (Some(&ss), Some(&ts)) = (self.id_index.get(&s), self.id_index.get(&t)) else {
                continue;
            };
            if let Some(tf) = edge_transform12(self.positions[ss], self.positions[ts], radius_comp) {
                buf.extend_from_slice(&tf);
            }
        }
        buf
    }

    /// Ids drawn by the last `build_node_buffer`, for the interaction ray.
    pub fn render_ids(&self) -> &[u32] {
        &self.render_ids
    }

    /// Render (server-space) positions parallel to [`render_ids`](Self::render_ids).
    pub fn render_positions(&self) -> &[[f32; 3]] {
        &self.render_positions
    }

    /// All node ids currently in the store (slot order), for the LOD selection.
    pub fn all_ids(&self) -> &[u32] {
        &self.ids
    }

    /// Current render position of a node (zeros if unknown) — used at grab start.
    pub fn position_of(&self, node_id: u32) -> [f32; 3] {
        self.id_index
            .get(&node_id)
            .map(|&s| self.positions[s])
            .unwrap_or([0.0, 0.0, 0.0])
    }

    /// Per-axis `[lo,hi]` percentile AABB over render positions, excluding the
    /// grabbed node so a dragged outlier can't inflate the adaptive fit. Returns
    /// `[minx,miny,minz,maxx,maxy,maxz]`, or `None` when empty.
    pub fn aabb_percentile(&self, lo_q: f32, hi_q: f32, exclude_id: Option<u32>) -> Option<[f32; 6]> {
        let mut xs = Vec::with_capacity(self.ids.len());
        let mut ys = Vec::with_capacity(self.ids.len());
        let mut zs = Vec::with_capacity(self.ids.len());
        for slot in 0..self.ids.len() {
            if Some(self.ids[slot]) == exclude_id {
                continue;
            }
            let p = self.positions[slot];
            xs.push(p[0]);
            ys.push(p[1]);
            zs.push(p[2]);
        }
        if xs.is_empty() {
            return None;
        }
        Some([
            percentile(&xs, lo_q),
            percentile(&ys, lo_q),
            percentile(&zs, lo_q),
            percentile(&xs, hi_q),
            percentile(&ys, hi_q),
            percentile(&zs, hi_q),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn node_transform_places_scale_and_origin() {
        let t = node_transform12(2.0, [5.0, 6.0, 7.0]);
        // row-major 3x4: diag scale, origin in the 4th column of each row.
        assert_eq!(t, [2.0, 0.0, 0.0, 5.0, 0.0, 2.0, 0.0, 6.0, 0.0, 0.0, 2.0, 7.0]);
    }

    #[test]
    fn edge_transform_vertical_is_identity_rotation() {
        // a→b straight up: dir == UP, rotation identity, Y scaled to length 4.
        let tf = edge_transform12([0.0, 0.0, 0.0], [0.0, 4.0, 0.0], 0.1).unwrap();
        // origin = midpoint (0,2,0)
        assert!(approx(tf[3], 0.0) && approx(tf[7], 2.0) && approx(tf[11], 0.0));
        // c1 (local Y column) = (0, length, 0) → row1col1 == 4
        assert!(approx(tf[5], 4.0));
        // c0,c2 scaled by radius on the diagonal
        assert!(approx(tf[0], 0.1) && approx(tf[10], 0.1));
    }

    #[test]
    fn edge_transform_maps_local_y_onto_direction() {
        // For any direction, column 1 (local Y) must equal dir*length.
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 2.0, 3.0]; // +X direction, length 3
        let tf = edge_transform12(a, b, 0.5).unwrap();
        // c1 = (row0col1,row1col1,row2col1) = tf[1],tf[5],tf[9]
        let c1 = [tf[1], tf[5], tf[9]];
        assert!(approx(c1[0], 3.0) && approx(c1[1], 0.0) && approx(c1[2], 0.0));
        // origin = midpoint
        assert!(approx(tf[3], 2.5) && approx(tf[7], 2.0) && approx(tf[11], 3.0));
    }

    #[test]
    fn edge_transform_antiparallel_flips_y() {
        let tf = edge_transform12([0.0, 0.0, 0.0], [0.0, -2.0, 0.0], 0.1).unwrap();
        // c1 = (0,-length,0) → tf[5] == -2
        assert!(approx(tf[5], -2.0));
    }

    #[test]
    fn degenerate_edge_is_none() {
        assert!(edge_transform12([1.0, 1.0, 1.0], [1.0, 1.0, 1.0], 0.1).is_none());
    }

    #[test]
    fn hunt_converges_toward_target() {
        let mut s = RenderStore::new();
        s.upsert(7, [0.0, 0.0, 0.0], 0, 0.0, 0.0);
        // Move the target; render position should ease toward it, not jump.
        s.upsert(7, [10.0, 0.0, 0.0], 0, 0.0, 0.0);
        s.hunt(0.5, None, [0.0, 0.0, 0.0]);
        assert!(approx(s.position_of(7)[0], 5.0));
        for _ in 0..40 {
            s.hunt(0.5, None, [0.0, 0.0, 0.0]);
        }
        assert!(approx(s.position_of(7)[0], 10.0));
    }

    #[test]
    fn hunt_pins_grabbed_node() {
        let mut s = RenderStore::new();
        s.upsert(3, [0.0, 0.0, 0.0], 0, 0.0, 0.0);
        s.upsert(3, [100.0, 0.0, 0.0], 0, 0.0, 0.0); // target far away
        s.hunt(0.1, Some(3), [1.0, 2.0, 3.0]);
        // Grabbed → pinned exactly at grab position, ignoring the target.
        assert_eq!(s.position_of(3), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn node_buffer_layout_and_size_from_centrality() {
        let mut s = RenderStore::new();
        s.upsert(1, [1.0, 2.0, 3.0], 0, 0.0, 1.0); // max centrality
        s.upsert(2, [4.0, 5.0, 6.0], 0, 0.0, 0.0); // zero centrality
        let buf = s.build_node_buffer(&[1, 2], 2.0, 0.7, 1.9);
        assert_eq!(buf.len(), 2 * NODE_STRIDE);
        // Node 1: cen_norm=1 → size = 2.0*lerp(0.7,1.9,1)=3.8; origin (1,2,3).
        assert!(approx(buf[0], 3.8) && approx(buf[3], 1.0) && approx(buf[7], 2.0) && approx(buf[11], 3.0));
        // custom.r == cen_norm == 1 (index 16..20 block, r at 16).
        assert!(approx(buf[16], 1.0));
        // Node 2: cen_norm=0 → size = 2.0*0.7 = 1.4; custom.r == 0.
        assert!(approx(buf[NODE_STRIDE], 1.4));
        assert!(approx(buf[NODE_STRIDE + 16], 0.0));
    }

    #[test]
    fn edge_buffer_filters_undrawn_endpoints() {
        let mut s = RenderStore::new();
        s.upsert(1, [0.0, 0.0, 0.0], 0, 0.0, 0.0);
        s.upsert(2, [0.0, 2.0, 0.0], 0, 0.0, 0.0);
        s.upsert(3, [0.0, 4.0, 0.0], 0, 0.0, 0.0);
        // Draw only 1 and 2.
        let _ = s.build_node_buffer(&[1, 2], 1.0, 0.7, 1.9);
        // Edge (1,2) both drawn → kept; edge (2,3) has undrawn endpoint → dropped.
        let buf = s.build_edge_buffer(&[1, 2, 2, 3], 0.1);
        assert_eq!(buf.len(), EDGE_STRIDE, "only the fully-drawn edge is emitted");
    }

    #[test]
    fn aabb_excludes_grabbed_and_uses_percentiles() {
        let mut s = RenderStore::new();
        for i in 0..100u32 {
            s.upsert(i, [i as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        // An outlier that would blow the AABB if included.
        s.upsert(999, [100000.0, 0.0, 0.0], 0, 0.0, 0.0);
        let bb = s.aabb_percentile(0.05, 0.95, Some(999)).unwrap();
        // 5th–95th percentile of 0..99 stays well within range; outlier excluded.
        assert!(bb[3] < 100.0, "outlier excluded, x-max stays bounded");
        assert!(bb[0] >= 0.0);
    }

    #[test]
    fn nodes_near_orders_by_distance_and_caps() {
        let mut s = RenderStore::new();
        // Nodes strung along +X at 1,2,3,4,5 m.
        for i in 1..=5u32 {
            s.upsert(i, [i as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        // From the origin, radius 3.5 covers ids 1,2,3; cap to 2 → nearest two.
        let near = s.nodes_near([0.0, 0.0, 0.0], 3.5, 2);
        assert_eq!(near, vec![1, 2], "nearest-first, capped at max");
        // Radius 3.5, no cap pressure → 1,2,3 in order.
        let near_all = s.nodes_near([0.0, 0.0, 0.0], 3.5, 10);
        assert_eq!(near_all, vec![1, 2, 3]);
        // Empty when nothing is in range.
        assert!(s.nodes_near([100.0, 0.0, 0.0], 0.5, 10).is_empty());
    }

    #[test]
    fn meta_label_and_detail() {
        let mut s = RenderStore::new();
        s.set_meta(1, "alpha-page".into(), "Alpha".into(), "page".into(), "logseq".into());
        assert_eq!(s.meta_id_of(1), "alpha-page");
        assert_eq!(s.label_of(1), "Alpha");
        assert_eq!(s.detail_of(1), "page · logseq");
        // Unknown node → empty, not a panic.
        assert_eq!(s.meta_id_of(99), "");
        assert_eq!(s.label_of(99), "");
        assert_eq!(s.detail_of(99), "");
        // Only node_type present.
        s.set_meta(2, "".into(), "Beta".into(), "agent".into(), "".into());
        assert_eq!(s.detail_of(2), "agent");
    }

    #[test]
    fn search_labels_ranks_prefix_over_substring() {
        let mut s = RenderStore::new();
        // "Graph" is a prefix match; "Knowledge Graph" only a substring match.
        s.set_meta(1, "".into(), "Knowledge Graph".into(), "page".into(), "".into());
        s.set_meta(2, "".into(), "Graph Theory".into(), "page".into(), "".into());
        let hits = s.search_labels("graph", 10);
        assert_eq!(hits, vec![2, 1], "prefix match ranks before substring match");
    }

    #[test]
    fn search_labels_is_case_insensitive() {
        let mut s = RenderStore::new();
        s.set_meta(1, "".into(), "OntoLogy".into(), "page".into(), "".into());
        assert_eq!(s.search_labels("ONTOLOGY", 10), vec![1]);
        assert_eq!(s.search_labels("onto", 10), vec![1]);
        assert_eq!(s.search_labels("LOG", 10), vec![1]); // substring, mixed case
    }

    #[test]
    fn search_labels_ties_break_by_centrality_desc() {
        let mut s = RenderStore::new();
        // Both prefix matches; centrality (from position upserts) orders them.
        s.set_meta(1, "".into(), "Node Alpha".into(), "page".into(), "".into());
        s.set_meta(2, "".into(), "Node Beta".into(), "page".into(), "".into());
        s.set_meta(3, "".into(), "Node Gamma".into(), "page".into(), "".into());
        s.upsert(1, [0.0; 3], 0, 0.0, 0.2);
        s.upsert(2, [0.0; 3], 0, 0.0, 0.9);
        s.upsert(3, [0.0; 3], 0, 0.0, 0.5);
        assert_eq!(s.search_labels("node", 10), vec![2, 3, 1]);
    }

    #[test]
    fn search_labels_respects_cap_and_empty_query() {
        let mut s = RenderStore::new();
        for i in 1..=5u32 {
            s.set_meta(i, "".into(), format!("Item {i}"), "page".into(), "".into());
        }
        assert_eq!(s.search_labels("item", 2).len(), 2, "capped at max");
        assert!(s.search_labels("", 10).is_empty(), "empty query → no hits");
        assert!(s.search_labels("   ", 10).is_empty(), "whitespace query → no hits");
        assert!(s.search_labels("item", 0).is_empty(), "max 0 → no hits");
        assert!(s.search_labels("nonexistent", 10).is_empty());
    }

    #[test]
    fn search_labels_bounded_topk_keeps_the_best_not_arbitrary() {
        // Many matches, small cap: the bounded heap must return the globally best
        // `max`, independent of HashMap iteration order. Two prefix matches (best
        // tier) plus several substring matches; max=2 must return exactly the two
        // prefix hits, ranked by centrality desc.
        let mut s = RenderStore::new();
        s.set_meta(1, "".into(), "Graph Alpha".into(), "page".into(), "".into()); // prefix
        s.set_meta(2, "".into(), "Graph Beta".into(), "page".into(), "".into());  // prefix
        for i in 3..=12u32 {
            s.set_meta(i, "".into(), format!("A Knowledge Graph {i}"), "page".into(), "".into()); // substring
        }
        s.upsert(1, [0.0; 3], 0, 0.0, 0.3);
        s.upsert(2, [0.0; 3], 0, 0.0, 0.9);
        // give a substring match high centrality — must STILL lose to prefix tier.
        s.upsert(5, [0.0; 3], 0, 0.0, 1.0);
        let hits = s.search_labels("graph", 2);
        assert_eq!(hits, vec![2, 1], "top-2 = the two prefix matches, centrality desc");
    }

    #[test]
    fn search_labels_uses_cached_lowercase() {
        // set_meta caches label_lower; a match on mixed case proves the cache is
        // populated and used (not the raw label).
        let mut s = RenderStore::new();
        s.set_meta(1, "".into(), "MixedCaseLabel".into(), "page".into(), "".into());
        assert_eq!(s.search_labels("mixedcase", 5), vec![1]);
        assert_eq!(s.search_labels("CASELABEL", 5), vec![1]);
    }

    #[test]
    fn community_color_is_deterministic_and_opaque() {
        let a = community_color(5, 0.0, 1);
        let b = community_color(5, 0.0, 1);
        assert_eq!(a, b);
        assert_eq!(a[3], 1.0);
        // Anomaly pushes toward warning red.
        let warn = community_color(5, 1.0, 1);
        assert!(warn[0] > a[0], "anomalous node reddens");
    }
}
