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

use std::collections::{HashMap, HashSet};

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
