# PRD — Visual Query Builder with Semantic Planes (Vive XR client)

**Status:** Phase A shipped (pattern-match endpoint) · Phase B shipped (radial-menu integration + variable marking) · Phase C in build (pattern assembly + live count preview) · Phases D–E designed
**Index:** listed as a Subsystem PRD in [docs/prd/README.md](README.md); mining provenance in [ADR-139](../adr/ADR-139-immersive-interaction-adoption-programme.md)
**Author:** Opus flagship-feature lead
**Feature track:** Task #18 (flagship); builds on Wave 1 (#14) expansion API + radial menu, Wave 2 (#15) additive merge, and the CUDA settling-engine extensions (#16).
**Concept origin:** Graph2VR "in-place query variables → live count → materialised result subgraphs on stacked planes".

---

## 0. One-paragraph summary

The user marks nodes and edges of the *visible* graph as query variables in-place (a
node recolours and gains a `?v1` badge; the wand-joined edge between two marked nodes
becomes a typed or variable predicate). The marked pattern **is** a graph query. A live
count preview on the HUD tells the user how many bindings the pattern matches *before*
they commit. Executing the query asks the backend to enumerate bindings and spawns one
result-subgraph per binding, laid out on parallel "semantic planes" offset along +Z,
browsable and individually discardable. We execute the match **server-side over the
in-memory typed graph** (not via SPARQL translation) and render the planes **client-side
as a Z-offset in `RenderStore` for v1**, reserving the CUDA plane-snap force for a later
phase when planes hold live-simulated subgraphs.

---

## 1. Interaction design

### 1.1 State model (client)

A new autoload/service `query_builder.gd` owns the query being assembled, entirely
client-side until execute:

```
QueryState = {
  vars:   { node_id:int -> var_name:String },   # "?v1", "?v2", … assignment order
  triples:[ { src:VarOrId, edge_type:String|"?", tgt:VarOrId } ],
  active: bool                                    # builder mode on/off
}
```

- `VarOrId` is either a concrete wire id (`u32`) or a var name string (`"?v1"`).
- A node marked as a variable is recoloured and badged; an *unmarked* concrete node used
  in a triple stays itself (an anchor — "edges from *this specific* node to any `?v2`").

### 1.2 Marking a variable — wand + radial menu (not long-press)

We reuse the **existing selection + radial menu** rather than inventing a long-press,
because both already exist and long-press conflicts with the grab/drag trigger semantics
in `graph_scene.gd` (trigger is grab-engage; `ACTIVATION_THRESHOLD = 0.7`, release lower —
lines 54–55, 168–170).

Flow:
1. Wand ray hits a node; user pulls trigger → existing selection path fires
   (`_selection.selection_made` → `_on_selection_made`, `graph_scene.gd:321,1706`).
2. To open actions on that node the user presses the **A/X face button** (free — grip is
   HUD-grab, `graph_scene.gd:904`; trigger is grab/click). This calls
   `RadialMenu.open(items, world_pos)` at the node's render position
   (`_binary_client.node_position(id)`, exposed `binary_protocol.rs:728`).
   `RadialMenu` (`radial_menu.gd`) is **built and tested but not yet wired into the scene**
   — this feature is its first integrator.
3. Radial items are context-dependent dicts `{label, action, count?}` (the exact shape
   `radial_menu.gd:_build_buttons` consumes, lines 102–119):
   - `Mark as variable` → assigns next `?vN`, recolours node.
   - `Use as anchor` → node participates by concrete id (no recolour, subtle ring).
   - `Join to active` → creates a triple between this node and the *last* marked node
     (see 1.3); sub-menu picks the predicate, populated from the **existing
     `/relations` endpoint** (`graph/mod.rs:986`) so only real predicates between the
     endpoints are offered, each already carrying its `count` (rendered as
     `"references (12)"` by `radial_menu.gd:110-112`).
   - `Clear mark` → unmark.
   - `Execute query` / `Clear query` → terminal actions (also on the HUD, 1.5).

`RadialMenu.item_selected(action:String)` (signal, `radial_menu.gd:17`) routes to
`query_builder.gd`. Action strings encode payload, e.g. `"mark:12345"`,
`"join:12345:references"`, `"execute"`.

### 1.3 Visualising the active pattern

- **Variable recolour.** Node colour lives in `RenderStore.color[slot]` and is packed into
  the node MultiMesh buffer per instance (`render_store.rs:449`, `NODE_STRIDE = 20`,
  colour block at offset 12). We add a **variable-overlay set** to `RenderStore`
  (`HashSet<u32>` of variable node ids + a palette index) consulted inside
  `build_node_buffer`; marked nodes get a saturated query palette colour (cyan `?v1`,
  magenta `?v2`, …) and a raised `INSTANCE_CUSTOM.r` flag so the node shader can add a rim
  glow. This mirrors how `community_color`/anomaly already override colour there
  (`render_store.rs:117,449`). New Rust `#[func]`s on `BinaryProtocolClient`:
  `set_query_var(node_id, palette_idx)`, `clear_query_var(node_id)`, `clear_all_query_vars()`.
- **Variable badge.** A billboarded `?vN` label reuses the proximity-label system
  (`graph_scene.gd:394-446` builds a shown-list; the grabbed node is always labelled) —
  marked nodes are force-appended to the shown list with their var name as label text.
- **Active triple edges.** The joined pattern edges render as a distinct **query edge
  MultiMesh** (a second edge instance buffer, same `edge_transform12` packing,
  `render_store.rs:460`) tinted bright and animated (reuse the edge-flow pulse from the
  recent `edge flow` commit). This keeps the query pattern visually separate from the
  145k background edges.

### 1.4 Count-preview surface (HUD)

The HUD is a `SubViewport` Control tree (`hud.gd`) already carrying a control-state line
(`set_control_states(hierarchy_on, is_flat, pinned_count)`, `hud.gd:141`) and a document
panel. We add a **Query panel** to `HUD.tscn` with:

- one chip per variable (`?v1 · Page`, coloured to match the node palette),
- a predicate summary line (`?v1 —references→ ?v2`),
- a **live binding count** (`≈ 42 matches`) with a spinner while in-flight,
- `Execute` and `Clear` buttons (wand-clickable via the existing HUD pointer path
  `_update_hud_pointer`, `graph_scene.gd:936`).

New `hud.gd` API: `set_query_preview(vars:Array, predicate_summary:String, count:int,
pending:bool)` and `hide_query_preview()`. The count is refreshed by a debounced
(~300 ms) `countOnly` POST each time the pattern changes (1.6 wire), using a dedicated
`HTTPRequest` created at runtime exactly like `doc_http`/`_physics_http`
(`hud.gd:49`, `graph_scene.gd:353`).

### 1.5 Execute / dismiss

- **Execute** (radial `execute` or HUD button) → full `POST /api/graph/query/pattern`
  with `countOnly:false`, `limit` from settings (default 24 planes). On response,
  `plane_manager.gd` spawns result subgraphs (Section 3).
- **Dismiss a plane** — wand-point at a plane's header card and click Close (HUD-pointer
  pattern), or grip-grab a plane and fling it (reuse HUD-grab gesture `graph_scene.gd:904`).
- **Clear query** resets `QueryState`, calls `clear_all_query_vars()`, hides the query
  panel, frees query edges and all planes.

### 1.6 Failure / edge behaviour

- Empty pattern (no triples) → Execute disabled; count shows `—`.
- Count in-flight debounced; a newer request supersedes an older (cancel like
  `doc_http.cancel_request()`, `hud.gd:167`).
- `bindingCount == 0` → count reads `0 matches`, Execute disabled.
- `truncated:true` (more bindings than `limit`) → count shows `≈ N (showing first 24)`;
  never silently drop — surfaced, matching the project's "no silent caps" discipline.

---

## 2. Query execution design

### 2.1 Recommendation: **server-side pattern match over the in-memory typed graph**, not SPARQL translation

New endpoint `POST /api/graph/query/pattern`, sibling to `/relations` and `/expand`,
registered in the same `graph` scope (`graph/mod.rs:1089-1096`).

**Rationale (why not SPARQL/oxigraph):**

1. **The visible graph is the in-memory `GraphData`, not the oxigraph store.** `/relations`
   and `/expand` already match over the `Arc<GraphData>` snapshot fetched via
   `GetGraphStateActor → GetGraphData` (`graph/mod.rs:959-975`). The 13k nodes / 145k typed
   edges the user sees and marks are those structures. The oxigraph/Whelk store holds the
   **OWL ontology** (≈5,975 classes, subclass/disjoint axioms) — a different, smaller graph.
   A binding count from SPARQL would **not equal** what the user sees on screen, breaking the
   core "count preview matches the visible graph" promise.
2. **No instance data to translate to.** Making SPARQL correct would require mirroring every
   node/edge instance into oxigraph and keeping it in sync with live physics churn — a large
   new subsystem for zero interaction benefit.
3. **Pattern shape is tiny and bounded.** Real queries are a handful of triples over a
   handful of variables. A backtracking join over the in-memory adjacency is trivial and
   fast, and reuses the exact `fetch_graph_snapshot` + bounded-heap patterns already proven
   in `/expand` (`graph/mod.rs:849-906`) — including the DoS-bounded selection over 145k edges.
4. **`edge_group_key` already defines predicate identity** (`graph/mod.rs:684`, untyped →
   `"linked"`); the matcher reuses it verbatim so predicates mean the same thing as in
   `/relations`.

The ontology store remains available for a *future* semantic-variable extension (e.g.
`?v1 rdf:type ?Class` resolved through `mcp__ontology-bridge`), but that is out of scope
for the visible-graph query builder and would be an additive predicate resolver, not a
rewrite.

### 2.2 Wire shapes (exact)

Request (`camelCase`, matching the existing `#[serde(rename_all = "camelCase")]` convention
in this module, `graph/mod.rs:716,774,789`):

```jsonc
POST /api/graph/query/pattern
{
  "triples": [
    { "src": "?v1",   "edgeType": "references", "tgt": "?v2" },
    { "src": "?v2",   "edgeType": "?e",          "tgt": 8123 }   // ?e = variable predicate; 8123 = anchor id
  ],
  "limit": 24,          // max bindings returned; clamp [1, 500] like EXPAND_MAX_LIMIT
  "countOnly": false    // true = preview: return bindingCount only, skip binding materialisation
}
```

- A term is a **variable** iff it is a JSON string beginning `?`; otherwise it is a concrete
  `u32` wire id (masked with `NODE_ID_MASK` on entry, exactly as `get_node_relations` does,
  `graph/mod.rs:990`, so flagged XR ids don't spuriously miss).
- `edgeType` may be a concrete predicate string or a `?`-prefixed edge variable; a concrete
  empty/`"linked"` matches untyped edges via `edge_group_key`.

Response:

```jsonc
{
  "vars": ["?v1", "?v2"],          // node variables, assignment order
  "edgeVars": ["?e"],              // edge variables, if any
  "bindingCount": 42,              // total matches found (may exceed returned bindings)
  "truncated": true,               // bindingCount capped scan hit limit / true count ≥ limit
  "bindings": [                    // omitted/empty when countOnly
    { "?v1": 12, "?v2": 88, "?e": "authored" },
    { "?v1": 15, "?v2": 90, "?e": "references" }
  ]
}
```

Binding node vars carry `u32` ids; edge vars carry the matched predicate string. The client
already knows every node's label/pos locally (`label_of`, `node_position`), so bindings stay
lean — no node metadata echoed back.

### 2.3 Matcher algorithm (server)

Pure function `match_pattern(graph: &GraphData, triples, limit, count_only) -> PatternResult`,
unit-testable off-actor exactly like `aggregate_relations`/`expand_neighbours`:

1. **Pre-index** the edge slice once per request: `HashMap<(predicate), Vec<&Edge>>` plus
   `HashMap<src_id, Vec<&Edge>>`. O(E) build over 145k edges — same cost class as the
   existing single-pass scans.
2. **Order triples** by selectivity (concrete-endpoint triples first, then
   concrete-predicate, then fully-variable) so the backtracking join prunes early.
3. **Backtracking join** over a partial binding `HashMap<VarName, u32>`: for each triple,
   given already-bound endpoints, enumerate candidate edges from the index; bind free
   variables; recurse. On a complete binding, push (or, for `countOnly`, just increment).
4. **Bound the work**: stop enumerating once `bindingCount` reaches a scan cap
   (`QUERY_SCAN_CAP`, e.g. 5000) and set `truncated`. Returned `bindings` are separately
   capped at `limit`. This gives the same DoS posture as `/expand`'s bounded heap.

Complexity is O(E) index + O(candidates·depth); for realistic 1–3-triple patterns this is
sub-millisecond on the in-memory graph. Same public/unauth posture as the sibling reads
(`graph/mod.rs:1082-1088`).

### 2.4 Reused backend infrastructure

- `fetch_graph_snapshot` (`graph/mod.rs:959`) — no per-request Tokio runtime.
- `edge_group_key` / `prettify_edge_label` (`graph/mod.rs:684,694`).
- `NODE_ID_MASK` masking (`graph/mod.rs:990`).
- Serde camelCase structs pattern (`graph/mod.rs:715`).
- Route registration in the `graph` resource scope (`graph/mod.rs:1089`).

---

## 3. Semantic-plane rendering

### 3.1 Recommendation: **client-side Z-offset in `RenderStore` for v1**; CUDA plane-snap force reserved for live-simulated planes (later)

**v1 — client-side offset (ship this):**

Result subgraphs are **ephemeral, per-binding, browsable/discardable presentation** — they
are *not* part of the force simulation. Each binding's subgraph is small (the pattern nodes
plus, optionally, a one-hop `/expand` halo). Laying them out is pure presentation, so we do
it where presentation already lives: the `RenderStore` buffer builder.

Design:
- `plane_manager.gd` maps binding *i* → plane at world `z_offset = i * PLANE_GAP` (a chosen
  axis, default +Z; user-rotatable).
- A **new render path** packs a *plane buffer*: `build_plane_node_buffer(ids, z_offset,
  scale_comp, size_lo, size_hi)` — identical maths to `build_node_buffer`
  (`render_store.rs:431`) with `pos[2] += z_offset`, writing to a **separate MultiMesh**
  per plane (or one MultiMesh with a per-instance plane offset in `INSTANCE_CUSTOM.g`,
  which the node shader adds — avoids N MultiMesh nodes). Positions come from the client's
  already-known `render_positions` (`render_store.rs:485`); binding ids come from the query
  response. **No backend round-trip for layout, no physics coupling.**
- Each plane gets a translucent quad backboard + a header card (label = the binding's
  primary variable, e.g. `?v1 = "Attention Is All You Need"`), reusing the HUD card
  renderer style (`hud.gd:render_ng_card`, line 201).
- Dismiss = free that plane's MultiMesh/instances. Zero effect on the base graph or sim.

**Why not CUDA plane-snap for v1:** result-subgraph nodes are ephemeral and duplicated
across planes (the same node id can appear in many bindings). Feeding them into the sim
would mean synthetic node ids, a per-node `plane_index` buffer, and a full GPU re-upload on
every execute/dismiss — churn the 10 fps snapshot loop (`force_compute_actor.rs:346`) does
not want, for subgraphs that never need force layout. Presentation-only offset is strictly
simpler and correct.

### 3.2 Future — CUDA plane-snap force (when planes hold *live* subgraphs)

If a later phase lets users **edit / re-simulate** a plane's subgraph in place, that
subgraph's nodes become real sim participants and want a Z-plane constraint. That is a
direct clone of two existing GPU patterns:

- **Per-node plane-index buffer** — allocate/upload exactly like the DAG `node_rank` buffer
  and the `pinned_mask` (`force_compute_actor.rs:529-643`, `sync_pinned_mask` /
  `upload_pinned_mask`; kernel consumes `pinned_mask` at `visionclaw_unified.cu:803-817`).
- **Z-snap force term** — a device function `plane_snap(my_pos, plane_index, idx)` that
  Hooke-springs `pos.z` toward `plane_index * PLANE_GAP`, structurally identical to
  `dag_radial_bias` (`visionclaw_unified.cu:176-201`) but on a single axis. Add it to
  `total_force` next to the DAG term (`visionclaw_unified.cu:683,2342`).
- **Registry entry** — add `ForceChannel::PlaneSnap` to `force_channels.rs:52` with backing
  scalar `plane_snap_k` + `plane_gap` in `SimParams` (the module is explicitly built so
  "adding a force term means adding a variant here", `force_channels.rs:44`). The CPU-side
  `project_node_xy` dual-disc projector (`force_compute_actor.rs:126-158`) is the precedent
  for snap-to-plane geometry.

This split — presentation offset now, GPU constraint only when planes become live — keeps
v1 shippable in days and defers GPU work until a feature actually needs it.

---

## 4. Phased implementation plan (each phase independently shippable)

**Phase A — Backend pattern-match endpoint. [SHIPPED]** `POST /api/graph/query/pattern`
(`match_pattern` pure fn + handler + route in `graph/mod.rs`), full unit tests off-actor
(mirror `relations_expand_tests`, `graph/mod.rs:1167`). Ships value immediately: queryable
by curl/desktop before any XR UI. *No client changes.*

**Phase B — Radial menu integration + variable marking (no query yet). [SHIPPED]** Wire
`RadialMenu` into `graph_scene.gd` (A-button opens at node); add `RenderStore` query-var
overlay + `set_query_var`/`clear_query_var` `#[func]`s + shader rim; `query_builder.gd`
state. Deliverable: user can mark/unmark nodes as `?vN` and see them recolour. *No backend
dependency beyond Phase A being optional.*

**Phase C — Pattern assembly + live count preview. [IN BUILD]** `join` action + predicate sub-menu
from `/relations`; query edge MultiMesh; HUD query panel + `set_query_preview`; debounced
`countOnly` POST. Deliverable: user assembles a pattern and sees a live match count.
*Depends on A + B.*

**Phase D — Execute + semantic planes (client-side offset).** `plane_manager.gd`;
`build_plane_node_buffer` (or per-instance plane offset); plane backboards + header cards;
dismiss/clear. Deliverable: executing spawns browsable result planes. *Depends on A + C.*

**Phase E (optional, later) — Live-simulated planes via CUDA plane-snap.** `plane_index`
buffer + `plane_snap` kernel term + `ForceChannel::PlaneSnap`. Only if editable planes are
green-lit. *Depends on D + a product decision.*

Phases A–D deliver the full flagship concept; E is a capability upgrade, not a completion
requirement.

---

## 5. Desktop migration notes

The React/Three.js desktop client (`client/src/features/visualisation/`) shares the same
backend, so the split falls cleanly:

- **Backend (Phase A) is client-agnostic** — the desktop client consumes
  `POST /api/graph/query/pattern` unchanged. Zero XR coupling in the query engine is a
  deliberate migration lever.
- **Interaction** maps wand→mouse: mark-variable becomes right-click context menu (the
  desktop analogue of the radial menu) on a node; join becomes shift-click a second node;
  count preview is a panel in the existing control center
  (`client/src/features/control-center/`). The `query_builder` state model is
  UI-framework-neutral and ports as a Zustand slice.
- **Semantic planes** map directly to the desktop `RenderStore`/instanced-mesh path already
  documented in project memory (GraphManager instanced rendering): the same per-instance
  Z-offset approach applies — offset result-subgraph instances along a world axis, one
  `InstancedMesh` group (or plane offset attribute) per binding. The client-side-offset
  decision (3.1) is what makes desktop parity cheap: no GPU-specific code to port.
- **Command surface reuse:** the desktop `CommandInput.tsx` (modified in the working tree)
  could expose the same pattern as a typed query, sharing the wire contract with the XR
  visual builder — one endpoint, two authoring modalities.

---

## 6. Cited integration points (all read for this design)

| Concern | File · symbol |
|---|---|
| Radial menu component (built, unintegrated) | `xr-client/scripts/radial_menu.gd` — `open()`, `item_selected`, `_build_buttons` |
| Node/edge buffer packing, colour, positions | `xr-client/rust/src/render_store.rs` — `build_node_buffer:431`, `color:449`, `render_positions:485`, `NODE_STRIDE=20` |
| GDScript↔Rust surface | `xr-client/rust/src/binary_protocol.rs` — `build_node_buffer:684`, `node_position:728`, `search_labels:760`, `send_drag_*` |
| Selection / grab / HUD pointer wiring | `xr-client/scripts/graph_scene.gd` — `_on_selection_made:1706`, `_update_hud_pointer:936`, `_update_hud_grab:904`, label list:394 |
| HUD Control tree + cards + HTTP | `xr-client/scripts/hud.gd` — `set_control_states:141`, `render_ng_card:201`, `doc_http:49` |
| Predicate-count + expansion endpoints | `src/handlers/api_handler/graph/mod.rs` — `get_node_relations:986`, `expand_neighbours:831`, `fetch_graph_snapshot:959`, routes:1089, `edge_group_key:684`, `NODE_ID_MASK:990` |
| Force-channel registry (add-a-variant) | `src/models/force_channels.rs` — `ForceChannel:52`, `DagRadialBias`, snapshot:233 |
| DAG radial bias kernel (plane-snap template) | `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu` — `dag_radial_bias:176`, `pinned_mask:803`, force sum:683 |
| Per-node buffer upload / pinned mask / plane projection | `src/actors/gpu/force_compute_actor.rs` — `sync_pinned_mask:623`, `build_pinned_mask:532`, `project_node_xy:126` |

---

## 7. Open questions for queen review

1. **Plane axis default** — +Z (depth, into the scene) vs +Y (stacked upward)? Proposal: +Z,
   user-rotatable with the joined-hand rotate gesture from the pinch commit.
2. **Result subgraph scope per binding** — just the pattern nodes, or pattern + one-hop
   `/expand` halo? Proposal: pattern nodes by default, halo on a per-plane "expand" action.
3. **Edge variables in v1** — support `?e` predicate variables in Phase A, or defer to keep
   the count-preview join trivial? Proposal: support in the wire from the start (cheap in the
   matcher), expose in UI in a later polish pass.
4. **Default plane `limit`** — 24 proposed; large binding sets need a paging story.
