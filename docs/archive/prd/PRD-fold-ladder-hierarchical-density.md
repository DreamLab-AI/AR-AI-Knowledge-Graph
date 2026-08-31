# PRD — Fold-Level Ladder (Hierarchical Density Management)

**Status:** Phase 1 shipped (server fold-plan endpoint) · Phase 2 in build (RenderStore application + HUD buttons) · Phases 3–4 designed
**Feature track:** Task #19 (flagship) / Task #17 (Wave 3); part of the immersive-interaction adoption programme ([ADR-139](../adr/ADR-139-immersive-interaction-adoption-programme.md))
**Concept origin:** Ontosphere's discrete hierarchical fold levels (`ontosphere-mining-2026-08-30`); synergises with OntoAir's barycenter/layered layout and the landed DAG radial rank-bias ([ADR-138](../adr/ADR-138-gpu-force-channel-registry.md))
**Promoted from:** Phase-0 design report (`scratchpad/fold-ladder-phase0-design.md`, 2026-08-30)

---

## 0. Problem & one-paragraph summary

The immersive graph is ~13k nodes / ~145k edges over a 5,975-class ontology —
too dense to read at room scale. The fold ladder is a discrete, steppable answer
to that density: a per-view control that walks **∅** (everything visible) →
**L1** (hide low-signal nodes) → **L2** (fold `rdfs:subClassOf` chains into their
root) → **L3** (fold Louvain communities into a medoid). The server computes a
*fold plan* — which nodes to hide, which groups collapse into a representative —
and returns it over `GET /api/graph/fold`; the client applies the plan as an
id→representative remap in its render store. The server never mutates the graph;
folding is a pure, per-view transform, so two viewers of one session can hold
different fold levels.

## 1. The ladder

| Level | Meaning | Computed from |
|-------|---------|---------------|
| **∅ (L0)** | Everything visible, no groups. | — |
| **L1** | Hide low-signal nodes (bottom-quartile PageRank centrality). | `centrality` from the shared per-node analytics map (GPU PageRank), `LOW_SIGNAL_QUANTILE = 0.25` |
| **L2** | L1 + fold each `rdfs:subClassOf` chain into its chain root. | Subclass edges via `is_subclass_relation` (`is_subclass_of`/`subclass_of`/`SUBCLASS_OF`), mirroring `force_compute_actor::is_directed_hierarchy_relation` |
| **L3** | L2 + fold each Louvain community (not already inside a subclass group) into its highest-centrality medoid. | `community_id` + `centrality` from the analytics map (GPU Louvain) |

`hidden` and `groups` are **disjoint and additive per level** — L2's response
includes L1's `hidden`; L3 includes L1+L2. The client applies one plan wholesale;
there is no client-side level composition.

## 2. Architecture — hybrid, server plans / client applies

**Server emits a fold plan; the client owns fold application.** The split is
forced by where the data lives:

| Concern | Owner | Why |
|---------|-------|-----|
| Which nodes are low-signal (L1) | Server | Holds node-type + centrality percentiles for the whole graph |
| Subclass chain → representative (L2) | Server | Only place subclass edges are identifiable (the accept set + DAG-rank BFS live server-side) |
| Community → meta-node (L3) | Server | Only place `community_id` is authoritative — **the client discards `community_id`** (`graph_scene.gd` retains only `_node_centrality`), so it cannot self-group |
| Applying a plan (remap ids, hide members, re-route edges, badge) | Client `RenderStore` | Hot path; must stay in Rust for the 13k-node frame budget |
| Transitions / animation | Client | Position hunt already lives client-side |

## 3. Wire contract — `GET /api/graph/fold`

Read-only and public, same posture and `RateLimit::per_minute(120)` wrap as
`/expand`. Registered in `src/handlers/api_handler/graph/mod.rs` `config()`;
handler in `src/handlers/api_handler/graph/fold.rs`.

Query: `?level=<0..3>` (clamped to `[0, MAX_FOLD_LEVEL]`), optional
`?graph_type=knowledge|ontology|agent` (same semantics as `/graph/data`),
optional `?pinned=<csv>` of node ids the caller has pinned in this view.

```jsonc
// GET /api/graph/fold?level=2  → 200
{
  "level": 2,
  "graphType": null,             // echoes ?graph_type=
  "generation": 47,              // topology version; client discards stale plans
  "hidden": [12, 88, 913],       // L1 suppressed node ids (masked u32, NODE_ID_MASK)
  "groups": [
    {
      "representativeId": 4021,  // an EXISTING node promoted to stand-in (chain root / community medoid)
      "memberIds": [4022, 4110, 4222],  // ids folded INTO the representative
      "badge": 3,                // "N collapsed" count on the representative
      "kind": "subclass"         // "subclass" | "community"
    }
  ]
}
```

Design invariants baked into the shape:
- **Representative is an existing node id, not synthetic.** Avoids minting ids
  outside `NODE_ID_MASK` and inherits the representative's streamed
  position/community colour for free.
- **`generation`** is an FNV-1a hash over node ids + edge (source, target, type)
  — O(n+e), sub-millisecond at 13k/145k. It changes whenever the fold-relevant
  topology changes, so a plan minted before a graph rebuild is detectable
  client-side and the memo self-invalidates. The client stamps its topology with
  the same counter and drops mismatched plans.
- **Pin-agnostic memoisation.** The base plan is memoised by
  `(level, graph_type, generation)`; per-view pinned-node promotion is a cheap
  post-step applied outside the memo so it never pollutes the cache key. A
  **pinned node is never folded away** — it is promoted to its group's
  representative, honouring the operator's explicit pin.

## 4. Client application — `RenderStore` touch points (Phase 2)

The plan is applied as an id→representative remap inside
`xr-client/rust/src/render_store.rs` — the layer that already owns the drawn
subset, edge filtering, and the packed MultiMesh buffers.

New store state: `fold_remap: HashMap<u32,u32>` (memberId → representativeId),
`fold_hidden: HashSet<u32>`, `fold_badge: HashMap<u32,u32>`, plus
`set_fold_plan(level, hidden, groups)` / `clear_fold_plan()`.

1. **Node buffer** (`build_node_buffer`) — skip an id in `fold_hidden` or a
   folded member (`fold_remap[id] != id`); members never enter the buffer.
2. **Badge** — rides the two spare floats of the existing `INSTANCE_CUSTOM`
   channel (`custom.g = badge count`); the node shader draws "N collapsed". No
   new stride (`NODE_STRIDE = 20`), **zero wire cost**.
3. **Edge re-routing** (`build_edge_buffer`) — remap each endpoint through
   `fold_remap` *before* the `drawn` filter; intra-group self-edges (`s == t`)
   drop; outside→member edges collapse onto the representative. Hidden members
   drag no dangling edges.
4. **Badge label** — a `fold_suffix(id) → " (+N)"` on the existing proximity-label
   detail line; reuses the Title+Detail anchor pool.

## 5. Interaction — HUD first, gesture later (Phase 2 / Phase 4)

**HUD (primary, always available).** A `[Fold +]` / `[Fold -]` pair on the HUD
control grid (`HUD.tscn` `ControlsGrid2`, beside Hierarchy / Unpin All / Shells /
Flat). `hud.gd` emits `control_pressed("fold_plus"|"fold_minus")`; `graph_scene.gd`
clamps `_fold_level` in `[0,3]`, fires `GET /api/graph/fold?level=`, and applies
the plan. Status readout shows `fold L2` and disables `[Fold +]` at L3 /
`[Fold -]` at ∅.

**Per-view state.** `_fold_level` is a plain scene var, sibling of `_dag_bias_on`
/ `_z_compression` / `_node_size_factor`. It is **not** sent to the server —
folding is a local view transform.

**Gesture (deferred, Phase 4).** The obvious "both-grips pull/push" is **unsafe as
specified** — the two-hand manipulation already claims both grips for
scale/rotate/translate, and a pull/push is indistinguishable from its scale. If a
gesture is wanted, the clean arbitration is a distinct modifier (e.g. both grips
**+ both triggers**, a combination no current path claims) stepping the ladder on
a discrete detent. The HUD pair stays primary.

## 6. Transitions (Phase 3)

Fold/unfold reuse the existing optimistic **position hunt** (`render_store.rs`
`hunt`, `POSITION_HUNT_EASE = 0.06`) — no new animation system. On fold, members
get a ~0.3 s countdown during which they stay drawable and hunt toward the
representative's position, then vanish cleanly rather than popping. On unfold,
members seed at the representative and ease back out toward their
server-authoritative targets (still streaming in V3 frames — zero round-trip).
Folding is suppressed while a grab is in flight; two-hand manip composes cleanly
(fold lerp is in server space, manip is in the GraphRoot world transform).

## 7. Phased plan

| Phase | Scope | Status |
|-------|-------|--------|
| **1** | Server fold endpoint (`GET /api/graph/fold`): L1/L2/L3, per-generation memo, pinned promotion; unit-tested; curl-testable. Server-only, no client change. | **Shipped** (`src/handlers/api_handler/graph/fold.rs`) |
| **2** | `RenderStore` `set_fold_plan` remap (node skip + edge remap + badge channel), `[Fold +]/[Fold -]` HUD buttons, `_fold_level` per-view state, request+apply glue. **Snap** transitions. | **In build** |
| **3** | Animated transitions — fold-anim countdown in `hunt`; pinned/grabbed interaction rules. | Designed |
| **4** | Badge polish (shader ring reading `custom.g`, `(+N)` label suffix) + optional grips+triggers detent gesture. | Designed |

Each phase leaves the system releasable; folding is inert until `level > 0` is
requested (mirrors "DAG bias inert until `dagBiasK > 0`").

## 8. Desktop migration

Server work is **100% shared** — the desktop React/Three client calls the same
`GET /api/graph/fold` route against the identical fold-plan contract. Client work
re-implements per renderer: the remap lives in the desktop graph-data layer
(`GraphManager.tsx` / `GraphViewport.tsx` instanced-mesh path, over the existing
`reverseNodeIdMap`), the badge uses the `InstancedLabels` channel rather than a
MultiMesh custom float, and `[Fold +]/[Fold -]` map onto the control panel as a
local view knob.

## 9. Acceptance criteria

- `GET /api/graph/fold?level=N` returns a valid plan for N ∈ {0,1,2,3}; level is
  clamped; `graph_type` and `pinned` are honoured. *(Phase 1 — met.)*
- Plan `hidden`/`groups` are disjoint; representatives are existing ids;
  `generation` invalidates a stale plan across a graph rebuild. *(Phase 1 — met.)*
- A pinned node is never a folded member — it is promoted to representative.
  *(Phase 1 — met.)*
- Stepping the ladder in-headset visibly reduces density (nodes hidden, chains
  and communities collapse to a badged representative), edges re-route onto
  representatives, no dangling edges. *(Phase 2 — in build.)*
- Fold/unfold animate via the position hunt; pinned/grabbed nodes obey the
  interaction rules. *(Phase 3.)*

## References

- Phase-0 design report: `scratchpad/fold-ladder-phase0-design.md`.
- [ADR-139](../adr/ADR-139-immersive-interaction-adoption-programme.md) — mining provenance & governance.
- [ADR-138](../adr/ADR-138-gpu-force-channel-registry.md) — DAG radial bias & pinned bitmask this builds on.
- Mining record: `ontosphere-mining-2026-08-30` (RuVector `patterns`).
- Sibling flagship: `prd-visual-query-builder-semantic-planes.md`.
