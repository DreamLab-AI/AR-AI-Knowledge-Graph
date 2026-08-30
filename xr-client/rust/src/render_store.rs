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
//!   4 custom (INSTANCE_CUSTOM: cen_norm, fold_badge, query_var_flag, 1).
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
    // Fold-level ladder (Wave 3, Phase 2). A server-computed fold plan applied as
    // an id→representative remap. `fold_hidden`: L1 low-signal ids suppressed from
    // the draw. `fold_remap`: memberId→representativeId (members hide, their edges
    // re-route to the representative). `fold_badge`: representativeId→collapsed
    // count, surfaced via the INSTANCE_CUSTOM.g channel. Empty maps ⇒ ∅ (no fold).
    // `fold_hidden`/`fold_remap`/`fold_badge` are the EFFECTIVE state actually
    // rendered; they are derived from the raw plan below minus the current
    // query-var lift-outs, and recomputed whenever either input changes.
    fold_hidden: HashSet<u32>,
    fold_remap: HashMap<u32, u32>,
    fold_badge: HashMap<u32, u32>,
    // The RAW server fold plan, retained verbatim so the effective state can be
    // re-derived non-destructively when the query-var set changes (clearing a
    // query var must re-fold the node it had lifted out, with no server refetch).
    raw_fold_hidden: Vec<u32>,
    raw_fold_members: Vec<u32>,
    raw_fold_reps: Vec<u32>,
    // Visual-query-builder overlay (flagship): node id → variable palette index.
    // Marked nodes are recoloured to a saturated query-palette colour in
    // `build_node_buffer` and flagged in INSTANCE_CUSTOM.b so the node shader can
    // rim-glow them. Kept separate from `color` (community colour) so unmarking a
    // node restores its original colour with no reload.
    query_vars: HashMap<u32, u8>,
}

/// Distinct query-variable palette colours before they cycle — matches the client
/// marker's `?v1`,`?v2`,… palette length.
pub const QUERY_PALETTE_LEN: u8 = 8;

/// Saturated overlay colour for a query variable, keyed by palette index. A
/// golden-ratio hue walk (like [`community_color`]) at full saturation/value so a
/// marked `?vN` node reads as deliberately highlighted, not organically coloured.
pub fn query_var_color(palette_idx: u8) -> [f32; 4] {
    let slot = (palette_idx % QUERY_PALETTE_LEN) as f32;
    let hue = ((slot as f64) * 0.618_033_988_75).fract() as f32;
    let rgb = hsv_to_rgb(hue, 0.85, 1.0);
    [rgb[0], rgb[1], rgb[2], 1.0]
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
        self.fold_hidden.clear();
        self.fold_remap.clear();
        self.fold_badge.clear();
        self.raw_fold_hidden.clear();
        self.raw_fold_members.clear();
        self.raw_fold_reps.clear();
        self.query_vars.clear();
    }

    /// Mark a node as query variable `palette_idx` (recoloured + rim-flagged on the
    /// next `build_node_buffer`). Re-marking updates the palette slot.
    pub fn set_query_var(&mut self, node_id: u32, palette_idx: u8) {
        self.query_vars.insert(node_id, palette_idx);
        self.recompute_effective_fold(); // marking lifts the node out of any fold
    }

    /// Unmark a node (restores its community colour on the next build). No-op when
    /// the node was not marked.
    pub fn clear_query_var(&mut self, node_id: u32) {
        self.query_vars.remove(&node_id);
        self.recompute_effective_fold(); // unmarking re-folds it if the raw plan had it
    }

    /// Clear every query-variable mark (Clear Query).
    pub fn clear_query_vars(&mut self) {
        self.query_vars.clear();
        self.recompute_effective_fold(); // all previously-lifted nodes re-fold
    }

    /// Whether a node is currently marked as a query variable.
    pub fn is_query_var(&self, node_id: u32) -> bool {
        self.query_vars.contains_key(&node_id)
    }

    /// Apply a server fold plan. `hidden` are ids to suppress (L1); `members[i]`
    /// folds into `reps[i]` (L2/L3). The per-representative badge count is derived
    /// from the remap. Mismatched-length inputs are zipped to the shorter. Passing
    /// all-empty is equivalent to [`clear_fold_plan`](Self::clear_fold_plan).
    pub fn set_fold_plan(&mut self, hidden: &[u32], members: &[u32], reps: &[u32]) {
        // Retain the raw plan verbatim, then derive the effective state. Keeping the
        // raw plan lets a later query-var change re-derive without a refetch.
        self.raw_fold_hidden = hidden.to_vec();
        self.raw_fold_members = members.to_vec();
        self.raw_fold_reps = reps.to_vec();
        self.recompute_effective_fold();
    }

    /// Rebuild the effective fold state (`fold_hidden`/`fold_remap`/`fold_badge`)
    /// from the raw plan, applying the query-var lift-out: a query-marked node must
    /// never be folded away. The server's `?pinned=` promotion can't see the
    /// client-only `query_vars` marking, so reconcile it here. Idempotent and cheap
    /// (O(plan)); called on every fold-plan set AND every query-var change so
    /// clearing a mark re-folds its node with no server round-trip.
    fn recompute_effective_fold(&mut self) {
        self.fold_hidden.clear();
        for i in 0..self.raw_fold_hidden.len() {
            let id = self.raw_fold_hidden[i];
            if !self.query_vars.contains_key(&id) {
                self.fold_hidden.insert(id);
            }
        }
        self.fold_remap.clear();
        self.fold_badge.clear();
        let n = self.raw_fold_members.len().min(self.raw_fold_reps.len());
        for i in 0..n {
            let m = self.raw_fold_members[i];
            let r = self.raw_fold_reps[i];
            // A representative must never be its own member (defensive: skip); a
            // query-marked member is lifted out of the fold, not collapsed.
            if m == r || self.query_vars.contains_key(&m) {
                continue;
            }
            self.fold_remap.insert(m, r);
            *self.fold_badge.entry(r).or_insert(0) += 1;
        }
    }

    /// Fold badge count for a node — the number of members collapsed into it as a
    /// representative (0 for a plain node or when no fold is active). Drives the
    /// "(+N)" proximity-label suffix.
    pub fn badge_of(&self, node_id: u32) -> u32 {
        self.fold_badge.get(&node_id).copied().unwrap_or(0)
    }

    /// Clear any active fold plan (return to full density ∅), raw plan included.
    pub fn clear_fold_plan(&mut self) {
        self.raw_fold_hidden.clear();
        self.raw_fold_members.clear();
        self.raw_fold_reps.clear();
        self.fold_hidden.clear();
        self.fold_remap.clear();
        self.fold_badge.clear();
    }

    /// Representative a node id renders as under the active fold plan — itself when
    /// not a folded member.
    #[inline]
    fn fold_target(&self, id: u32) -> u32 {
        self.fold_remap.get(&id).copied().unwrap_or(id)
    }

    /// Map a ranked id list to its visible representatives under the active fold
    /// plan: drop L1-hidden ids, replace a folded member with its representative,
    /// and dedup while preserving order. Identity (and allocation-free clone of
    /// order) when no fold is active. Used by `search_labels` / `nodes_near` so a
    /// folded member can never be a search-teleport or proximity-label target at an
    /// invisible position — the caller lands on the visible representative instead.
    fn canonical_visible(&self, ids: Vec<u32>) -> Vec<u32> {
        if self.fold_hidden.is_empty() && self.fold_remap.is_empty() {
            return ids;
        }
        let mut out = Vec::with_capacity(ids.len());
        let mut seen: HashSet<u32> = HashSet::new();
        for id in ids {
            if self.fold_hidden.contains(&id) {
                continue;
            }
            let rep = self.fold_target(id);
            if self.fold_hidden.contains(&rep) {
                continue;
            }
            if seen.insert(rep) {
                out.push(rep);
            }
        }
        out
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
        // Canonicalise + dedup DURING selection so `max` bounds unique VISIBLE
        // results, not raw member matches: several folded members of one group must
        // not eat the quota and starve other reps. Each matching id is resolved to
        // its visible representative (hidden dropped); the best-ranked hit per
        // representative is kept in a map, then a bounded top-k selects `max` of the
        // deduped reps. When no fold is active the canonical id is the id itself, so
        // this degrades to the previous per-node behaviour.
        let mut best: HashMap<u32, LabelHit> = HashMap::new();
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
            // Resolve to the visible representative; drop a hidden match (or a match
            // whose representative is hidden).
            if self.fold_hidden.contains(&id) {
                continue;
            }
            let canon = self.fold_target(id);
            if self.fold_hidden.contains(&canon) {
                continue;
            }
            // Rank/centrality reflect the matching node; the returned id is `canon`
            // so tie-breaks stay stable on the representative id.
            let hit = LabelHit {
                rank,
                centrality: self.centrality_of(id),
                id: canon,
            };
            best.entry(canon)
                .and_modify(|cur| {
                    if hit.worseness(cur) == Ordering::Less {
                        *cur = hit;
                    }
                })
                .or_insert(hit);
        }
        // Bounded top-k over the deduped representatives (worst on top).
        let mut heap: BinaryHeap<LabelHit> = BinaryHeap::with_capacity(max + 1);
        for hit in best.into_values() {
            if heap.len() < max {
                heap.push(hit);
            } else if let Some(worst) = heap.peek() {
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
        // Canonicalise through the fold plan (member → visible representative,
        // hidden dropped, deduped) BEFORE the cap, so a folded cluster contributes
        // its representative once and the cap isn't spent on invisible members.
        let ranked: Vec<u32> = hits.into_iter().map(|(_, id)| id).collect();
        let mut vis = self.canonical_visible(ranked);
        vis.truncate(max);
        vis
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
        let has_fold = !self.fold_hidden.is_empty() || !self.fold_remap.is_empty();
        for &raw in ids {
            let id0 = raw as u32;
            // Fold plan is applied to the (already budgeted) draw list here: an
            // L1-hidden id is dropped, and a folded member is PROMOTED to its
            // representative rather than skipped. Promotion is what injects a
            // representative into the drawn set even when the LOD budget picked
            // only its members — without it a drawn member mapping to an
            // un-budgeted rep would make both the node and its remapped edges
            // vanish. Dedup (via the drawn set) keeps a rep with several drawn
            // members to a single instance.
            if self.fold_hidden.contains(&id0) {
                continue;
            }
            let id = self.fold_target(id0);
            if self.fold_hidden.contains(&id) {
                continue; // defensive: a representative marked hidden
            }
            if has_fold && self.drawn.contains(&id) {
                continue; // already drawn this build (another member promoted here)
            }
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
            // INSTANCE_CUSTOM: r = centrality halo tell, g = fold badge count
            // ("N collapsed" on a representative; 0 for a plain node), b/a reserved.
            let badge = self.fold_badge.get(&id).copied().unwrap_or(0) as f32;
            // Query-variable overlay (flagship): a marked node draws in its
            // saturated palette colour with INSTANCE_CUSTOM.b flagged so the shader
            // rim-glows it; unmarked nodes keep their community colour.
            let (col, query_flag) = match self.query_vars.get(&id) {
                Some(&palette_idx) => (query_var_color(palette_idx), 1.0),
                None => (self.color[slot], 0.0),
            };
            buf.extend_from_slice(&node_transform12(size, pos));
            buf.extend_from_slice(&col);
            buf.extend_from_slice(&[cen_norm, badge, query_flag, 1.0]);
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
        // Fold plan: many member→member edges collapse onto the same
        // representative→representative pair. Dedup so a folded group draws one
        // cylinder per distinct representative pair, not one per hidden member edge.
        let has_fold = !self.fold_remap.is_empty();
        let mut seen: HashSet<(u32, u32)> = HashSet::new();
        for i in 0..n {
            // Re-route each endpoint through the fold remap BEFORE the drawn test,
            // so edges of hidden members attach to their representative instead of
            // vanishing.
            let s = self.fold_target(pairs[i * 2] as u32);
            let t = self.fold_target(pairs[i * 2 + 1] as u32);
            // Intra-group edge (both endpoints fold to the same representative)
            // has no length — drop it.
            if s == t {
                continue;
            }
            if has_fold {
                let key = if s < t { (s, t) } else { (t, s) };
                if !seen.insert(key) {
                    continue;
                }
            }
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
    fn fold_plan_hides_members_and_badges_representative() {
        let mut s = RenderStore::new();
        for i in 1..=4u32 {
            s.upsert(i, [i as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        // Fold 2,3 into representative 1; hide node 4 (L1 low-signal).
        s.set_fold_plan(&[4], &[2, 3], &[1, 1]);
        let buf = s.build_node_buffer(&[1, 2, 3, 4], 1.0, 0.7, 1.9);
        // Only the representative (1) draws: members 2,3 folded, 4 hidden.
        assert_eq!(buf.len(), NODE_STRIDE, "only the representative renders");
        // Its custom.g (index 17) carries the badge count = 2 folded members.
        assert!(approx(buf[17], 2.0), "badge count in custom.g");
        // Drawn set reflects the fold — the interaction ray only sees the rep.
        assert_eq!(s.render_ids(), &[1]);
    }

    #[test]
    fn fold_plan_reroutes_edges_to_representative() {
        let mut s = RenderStore::new();
        // 1 = rep; 2,3 fold into 1; 5 is an outside node.
        for id in [1u32, 2, 3, 5] {
            s.upsert(id, [id as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        s.set_fold_plan(&[], &[2, 3], &[1, 1]);
        let _ = s.build_node_buffer(&[1, 2, 3, 5], 1.0, 0.7, 1.9); // drawn = {1,5}
        // Edges: (5→2) outside→member should re-route to (5→1); (2→3) intra-group
        // collapses to self and drops; a second outside edge (5→3) also maps to
        // (5→1) and must dedup against the first.
        let buf = s.build_edge_buffer(&[5, 2, 2, 3, 5, 3], 0.1);
        assert_eq!(
            buf.len(),
            EDGE_STRIDE,
            "one representative edge after remap+dedup, intra-group dropped"
        );
    }

    #[test]
    fn clear_fold_plan_restores_full_density() {
        let mut s = RenderStore::new();
        for i in 1..=3u32 {
            s.upsert(i, [i as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        s.set_fold_plan(&[3], &[2], &[1]);
        assert_eq!(s.build_node_buffer(&[1, 2, 3], 1.0, 0.7, 1.9).len(), NODE_STRIDE);
        s.clear_fold_plan();
        // All three draw again; badge channel back to 0.
        let buf = s.build_node_buffer(&[1, 2, 3], 1.0, 0.7, 1.9);
        assert_eq!(buf.len(), 3 * NODE_STRIDE);
        assert!(approx(buf[17], 0.0), "no badge after clear");
    }

    #[test]
    fn fold_promotes_representative_into_drawn_set_when_budget_picks_only_members() {
        // Regression: a budgeted draw list containing only folded MEMBERS must
        // still draw their representative (promotion) — otherwise both the node
        // and its remapped edges vanish.
        let mut s = RenderStore::new();
        for id in [1u32, 2, 3, 5] {
            s.upsert(id, [id as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        s.set_fold_plan(&[], &[2, 3], &[1, 1]); // 2,3 → rep 1
        // Budget selected only the members (rep 1 absent from the list).
        let buf = s.build_node_buffer(&[2, 3], 1.0, 0.7, 1.9);
        assert_eq!(buf.len(), NODE_STRIDE, "rep drawn once (promoted + deduped)");
        assert_eq!(s.render_ids(), &[1], "representative injected into drawn set");
        // And an edge from an outside node to a member now finds the rep on-screen.
        let ebuf = s.build_edge_buffer(&[5, 2], 0.1);
        // 5 wasn't drawn (not in the node list), so this still filters — but the
        // rep IS drawn, proving the endpoint remap resolves to a drawn node.
        let _ = ebuf; // (edge needs both endpoints drawn; asserted separately below)
        let buf2 = s.build_node_buffer(&[2, 3, 5], 1.0, 0.7, 1.9); // now 5 drawn too
        assert_eq!(buf2.len(), 2 * NODE_STRIDE, "rep(1) + outside(5)");
        let ebuf2 = s.build_edge_buffer(&[5, 2], 0.1);
        assert_eq!(ebuf2.len(), EDGE_STRIDE, "edge 5→member reroutes to drawn rep");
    }

    #[test]
    fn search_and_nodes_near_canonicalise_through_fold() {
        let mut s = RenderStore::new();
        for id in [1u32, 2, 3] {
            s.upsert(id, [id as f32, 0.0, 0.0], 0, 0.0, 0.0);
            s.set_meta(id, "".into(), format!("Node {id}"), "page".into(), "".into());
        }
        // Fold 2 → rep 1; hide 3.
        s.set_fold_plan(&[3], &[2], &[1]);
        // Searching a folded member's label resolves to its visible representative.
        assert_eq!(s.search_labels("Node 2", 10), vec![1], "member → representative");
        // A hidden node is never a search target.
        assert!(s.search_labels("Node 3", 10).is_empty(), "hidden excluded from search");
        // The representative itself still matches directly.
        assert_eq!(s.search_labels("Node 1", 10), vec![1]);
        // nodes_near canonicalises the same way: a query at the member's position
        // returns the representative, and the hidden node never appears.
        let near = s.nodes_near([2.0, 0.0, 0.0], 5.0, 10);
        assert!(near.contains(&1), "member's neighbourhood resolves to rep");
        assert!(!near.contains(&2), "folded member not returned directly");
        assert!(!near.contains(&3), "hidden node excluded from proximity");
    }

    #[test]
    fn query_var_member_is_never_folded_away() {
        let mut s = RenderStore::new();
        for id in [1u32, 2, 3] {
            s.upsert(id, [id as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        // Mark node 2 as a query variable, THEN apply a plan that would fold 2,3→1.
        s.set_query_var(2, 0);
        s.set_fold_plan(&[2], &[2, 3], &[1, 1]); // 2 also (wrongly) listed as hidden
        // Node 2 must be lifted out of both hide and fold — it draws as itself, so
        // build draws rep 1 (folding only member 3) plus the lifted query var 2.
        let buf = s.build_node_buffer(&[1, 2, 3], 1.0, 0.7, 1.9);
        assert_eq!(buf.len(), 2 * NODE_STRIDE, "rep + lifted query var draw");
        assert_eq!(s.render_ids(), &[1, 2], "query var 2 not hidden, not folded");
        assert_eq!(s.badge_of(1), 1, "only member 3 folded into rep 1");
    }

    #[test]
    fn search_cap_applies_to_unique_visible_representatives() {
        // Regression: several folded members of one group, all matching, must not
        // eat the search quota and starve other visible representatives.
        let mut s = RenderStore::new();
        for id in [1u32, 2, 3, 4, 10, 11] {
            s.upsert(id, [0.0, 0.0, 0.0], 0, 0.0, 0.0);
        }
        // Members 2,3 carry the HIGHEST centralities — under the old cap-then-fold
        // path they'd win both raw slots and collapse to a single rep, returning
        // just [1] and starving nodes 10/11.
        s.upsert(1, [0.0; 3], 0, 0.0, 0.50); // rep, modest centrality
        s.upsert(2, [0.0; 3], 0, 0.0, 0.99);
        s.upsert(3, [0.0; 3], 0, 0.0, 0.98);
        s.upsert(10, [0.0; 3], 0, 0.0, 0.70);
        s.upsert(11, [0.0; 3], 0, 0.0, 0.60);
        for (id, name) in [
            (1u32, "Match Rep"),
            (2, "Match A"),
            (3, "Match B"),
            (4, "Match C"),
            (10, "Match X"),
            (11, "Match Y"),
        ] {
            s.set_meta(id, "".into(), name.into(), "page".into(), "".into());
        }
        s.set_fold_plan(&[], &[2, 3, 4], &[1, 1, 1]); // 2,3,4 → rep 1
        // max=2 must return TWO distinct visible reps: rep 1 (best member centrality
        // 0.99) and node 10 (0.70) — NOT two members of the same fold.
        let hits = s.search_labels("match", 2);
        assert_eq!(hits, vec![1, 10], "cap bounds unique visible reps, not raw members");
    }

    #[test]
    fn clearing_query_var_refolds_without_refetch() {
        // Regression: the raw plan is retained, so a query-var lift-out is reversible
        // — clearing the mark re-folds the node with no server refetch.
        let mut s = RenderStore::new();
        for id in [1u32, 2, 3] {
            s.upsert(id, [id as f32, 0.0, 0.0], 0, 0.0, 0.0);
        }
        s.set_fold_plan(&[], &[2, 3], &[1, 1]); // 2,3 → rep 1
        assert_eq!(s.build_node_buffer(&[1, 2, 3], 1.0, 0.7, 1.9).len(), NODE_STRIDE);
        assert_eq!(s.badge_of(1), 2);
        // Mark node 2 → lifted out: rep 1 (folding only 3) + node 2 draw.
        s.set_query_var(2, 0);
        assert_eq!(s.build_node_buffer(&[1, 2, 3], 1.0, 0.7, 1.9).len(), 2 * NODE_STRIDE);
        assert_eq!(s.badge_of(1), 1);
        // Clear the mark → node 2 RE-FOLDS from the retained raw plan, no refetch.
        s.clear_query_var(2);
        assert_eq!(s.build_node_buffer(&[1, 2, 3], 1.0, 0.7, 1.9).len(), NODE_STRIDE);
        assert_eq!(s.badge_of(1), 2, "cleared query var re-folds into rep");
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

    #[test]
    fn query_var_overlay_recolours_and_flags_then_restores() {
        let mut s = RenderStore::new();
        s.upsert(1, [0.0, 0.0, 0.0], 3, 0.0, 1.0); // community colour, max centrality
        s.upsert(2, [1.0, 0.0, 0.0], 3, 0.0, 1.0);
        let base = s.build_node_buffer(&[1, 2], 1.0, 0.7, 1.9);
        // Unmarked: colour == community colour, query flag (custom.b, offset 18) == 0.
        let community = community_color(3, 0.0, 1);
        assert!(approx(base[12], community[0]) && approx(base[13], community[1]));
        assert!(approx(base[18], 0.0), "unmarked node has no query flag");

        // Mark node 1 as palette 0.
        s.set_query_var(1, 0);
        assert!(s.is_query_var(1) && !s.is_query_var(2));
        let marked = s.build_node_buffer(&[1, 2], 1.0, 0.7, 1.9);
        let qv = query_var_color(0);
        // Node 1 now the query colour with custom.b flagged; node 2 untouched.
        assert!(approx(marked[12], qv[0]) && approx(marked[13], qv[1]) && approx(marked[14], qv[2]));
        assert!(approx(marked[18], 1.0), "marked node sets query flag");
        assert!(approx(marked[NODE_STRIDE + 18], 0.0), "other node unflagged");
        // Fold badge channel (custom.g, offset 17) is untouched by the overlay.
        assert!(approx(marked[17], 0.0));

        // Unmark restores the community colour and clears the flag.
        s.clear_query_var(1);
        let restored = s.build_node_buffer(&[1, 2], 1.0, 0.7, 1.9);
        assert!(approx(restored[12], community[0]) && approx(restored[18], 0.0));
    }

    #[test]
    fn query_var_color_cycles_and_is_opaque() {
        // Palette index wraps at QUERY_PALETTE_LEN and is always opaque.
        assert_eq!(query_var_color(0), query_var_color(QUERY_PALETTE_LEN));
        assert_eq!(query_var_color(3)[3], 1.0);
        assert_ne!(query_var_color(0), query_var_color(1));
    }

    #[test]
    fn clear_query_vars_unmarks_all() {
        let mut s = RenderStore::new();
        s.set_query_var(1, 0);
        s.set_query_var(2, 1);
        assert!(s.is_query_var(1) && s.is_query_var(2));
        s.clear_query_vars();
        assert!(!s.is_query_var(1) && !s.is_query_var(2));
    }
}
