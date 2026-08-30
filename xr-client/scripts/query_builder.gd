class_name QueryBuilder
extends RefCounted

## Visual query builder state (flagship — Graph2VR "mark variables in-place").
##
## Holds the client-side query being assembled before it is executed server-side
## against POST /api/graph/query/pattern (Phase A). This RefCounted owns ONLY the
## pure state + the radial-menu item model, so it is unit-testable without the XR
## scene: graph_scene.gd drives it and applies the visual side effects (recolour
## via BinaryProtocolClient.set_query_var, ?vN proximity badge).
##
## Marked nodes become query variables "?v1","?v2",…; each carries a palette index
## (0..QUERY_PALETTE_LEN-1, cycling) that the render store maps to a highlight
## colour. v1 marks variables only — triple assembly + count preview land in
## Phase C, so this class already exposes `triples` as an (empty in v1) forward
## seam without wiring the join UI yet.

## Must match render_store.rs::QUERY_PALETTE_LEN so palette indices agree.
const QUERY_PALETTE_LEN: int = 8

## Whether Execute is wired to a real effect. False until Phase D (execute +
## semantic planes) lands — while false the radial omits the Execute item and the
## HUD button is disabled/relabelled, so there is no silent no-op.
const EXECUTE_ENABLED: bool = false

# node_id:int -> var_name:String ("?v1", …). Insertion order = assignment order.
var _vars: Dictionary = {}
# node_id:int -> palette index:int, parallel to _vars.
var _palette: Dictionary = {}
# Monotonic counter for the next ?vN name; never reused within a query so names
# stay stable even after unmarking an earlier variable.
var _next_index: int = 1
# Assembled triples (Phase C). Each: {src, edge_type:String, tgt} where src/tgt
# are a node id (int) or a var name (String). Empty in v1.
var triples: Array = []


## Mark `node_id` as a fresh variable. Returns its var name (e.g. "?v3"). If the
## node is already marked, its existing name is returned unchanged (idempotent).
func mark(node_id: int) -> String:
	if _vars.has(node_id):
		return _vars[node_id]
	var name := "?v%d" % _next_index
	_next_index += 1
	_vars[node_id] = name
	# Palette index cycles by the ordinal of this variable so ?v1..?vN get
	# distinct, repeating highlight colours.
	_palette[node_id] = (_vars.size() - 1) % QUERY_PALETTE_LEN
	return name


## Unmark `node_id`. Returns true if it was marked (caller then restores its
## colour), false if it was not a variable.
func unmark(node_id: int) -> bool:
	if not _vars.has(node_id):
		return false
	_vars.erase(node_id)
	_palette.erase(node_id)
	# In v1 `triples` is always empty; Phase C will prune triples that referenced
	# the removed variable here.
	return true


## True when `node_id` is currently a query variable.
func is_marked(node_id: int) -> bool:
	return _vars.has(node_id)


## Variable name for `node_id`, or "" when unmarked.
func var_name(node_id: int) -> String:
	return _vars.get(node_id, "")


## Palette index for `node_id` (0 when unmarked — callers should gate on
## is_marked). Feeds BinaryProtocolClient.set_query_var.
func palette_index(node_id: int) -> int:
	return _palette.get(node_id, 0)


## All currently marked node ids, in assignment order.
func marked_ids() -> Array:
	return _vars.keys()


## Number of marked variables.
func var_count() -> int:
	return _vars.size()


## Whether any query state exists (variables or triples) — gates "Clear query".
func is_active() -> bool:
	return not _vars.is_empty() or not triples.is_empty()


## Reset the whole query (Clear query). The caller separately clears the render
## overlay via BinaryProtocolClient.clear_query_vars.
func clear() -> void:
	_vars.clear()
	_palette.clear()
	triples.clear()
	_next_index = 1


## Build the flat radial-menu item model for a right-click/A-button on `node_id`.
##
## This is the SHARED node context menu: it returns sections in a stable order so
## the query-builder actions coexist with future Wave-2 expansion items. Each item
## is a dict {label, action[, count]} exactly as radial_menu.gd consumes.
##
## Sections, in ring order:
##   1. Variable section — "Mark as ?vN" or "Clear variable ?vN".
##   2. `extra_items` — caller-supplied (e.g. Wave-2 "Expand: references (12)");
##      passed through verbatim so this class never needs to know about them.
##   3. Query section — "Execute query" / "Clear query" when a query is active.
##
## Action strings (parsed by graph_scene.gd::_on_radial_item_selected):
##   "qb_mark:<id>", "qb_unmark:<id>", "qb_execute", "qb_clear".
func build_node_menu_items(node_id: int, extra_items: Array = []) -> Array:
	var items: Array = []
	# 1. Variable section.
	if is_marked(node_id):
		items.append({
			"label": "Clear variable %s" % var_name(node_id),
			"action": "qb_unmark:%d" % node_id,
		})
	else:
		items.append({
			"label": "Mark as ?v%d" % _next_index,
			"action": "qb_mark:%d" % node_id,
		})
	# 2. Future expansion / Wave-2 items (verbatim pass-through).
	for it: Dictionary in extra_items:
		items.append(it)
	# 3. Query section (only once a query exists). Execute is omitted until Phase D
	# wires it (EXECUTE_ENABLED) — never a menu item that no-ops.
	if is_active():
		if EXECUTE_ENABLED:
			items.append({
				"label": "Execute query (%d var)" % var_count(),
				"action": "qb_execute",
			})
		items.append({
			"label": "Clear query",
			"action": "qb_clear",
		})
	return items


## Derive the active pattern's triples from the visible-graph edge list.
##
## `edge_pairs` is the flat [src0,tgt0,src1,tgt1,…] directed pair list from
## BinaryProtocolClient.get_edges(). A directed edge whose BOTH endpoints are
## marked becomes a triple {src:?vSrc, edgeType:"any", tgt:?vTgt}: the pattern is
## exactly the set of visible edges connecting marked nodes, with the marked nodes
## as query variables.
##
## Edges are WILDCARD ("any") in v1 — the XR render store carries only edge
## (source,target,weight), NOT the predicate, so a concrete edgeType cannot be
## derived client-side without a round-trip. The concrete-type toggle is deferred
## to Phase D (state in the report). Deduped by (src,tgt): parallel edges collapse
## since the wildcard makes their triples identical. Stores and returns `triples`.
func derive_triples(edge_pairs: PackedInt32Array) -> Array:
	triples.clear()
	if _vars.is_empty():
		return triples
	var seen: Dictionary = {}
	var n := edge_pairs.size() / 2
	for i in range(n):
		var s: int = edge_pairs[i * 2]
		var t: int = edge_pairs[i * 2 + 1]
		if not _vars.has(s) or not _vars.has(t):
			continue
		var key := "%d>%d" % [s, t]
		if seen.has(key):
			continue
		seen[key] = true
		triples.append({
			"src": _vars[s],
			"edgeType": "any",
			"tgt": _vars[t],
		})
	return triples


## Build the POST /api/graph/query/pattern request body from the current pattern.
## `count_only` drives the debounced live-count preview; the full form is used at
## execute. Derives triples from `edge_pairs` first.
func build_pattern_payload(edge_pairs: PackedInt32Array, count_only: bool, limit: int) -> Dictionary:
	return {
		"triples": derive_triples(edge_pairs),
		"limit": limit,
		"countOnly": count_only,
	}


## One-line pattern summary for the HUD (call after derive_triples). E.g.
## "3 vars · 2 edges".
func pattern_summary() -> String:
	var vc := var_count()
	var ec := triples.size()
	return "%d var%s · %d edge%s" % [vc, "" if vc == 1 else "s", ec, "" if ec == 1 else "s"]
