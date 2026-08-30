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
## colour. `derive_triples` turns the marked-marked visible edges into the query
## pattern; `build_pattern_payload` produces the POST body for the count preview
## and execute.

## Must match render_store.rs::QUERY_PALETTE_LEN so palette indices agree.
const QUERY_PALETTE_LEN: int = 8

## Whether Execute is wired to a real effect. Phase D lands execute + semantic
## planes end-to-end, so this is now TRUE (radial offers the Execute item; the HUD
## button is enabled).
const EXECUTE_ENABLED: bool = true

## When true, marked-marked edges contribute their concrete predicate to the
## pattern; when false, every pattern edge is the "any" wildcard. Toggled from the
## radial ("Edges: concrete/any"). Defaults to concrete now that the edge-type wire
## carries predicates (Phase D).
var use_concrete_edges: bool = true

# node_id:int -> var_name:String ("?v1", …). Insertion order = assignment order.
var _vars: Dictionary = {}
# node_id:int -> palette index:int, parallel to _vars.
var _palette: Dictionary = {}
# Monotonic counter for the next ?vN name; never reused within a query so names
# stay stable even after unmarking an earlier variable.
var _next_index: int = 1
# Pattern triples, rebuilt by derive_triples on each pattern change. Each:
# {src:String var, edgeType:String, tgt:String var}. Endpoints are ?vN names;
# edgeType is a concrete predicate or "any".
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
	# No triple pruning needed: derive_triples rebuilds `triples` from scratch on
	# the next pattern change, so edges that referenced the removed variable drop
	# out automatically.
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
	# 3. Query section (only once a query exists). Execute is gated on
	# EXECUTE_ENABLED — never a menu item that no-ops.
	if is_active():
		items.append({
			"label": "Edges: %s" % ("concrete" if use_concrete_edges else "any"),
			"action": "qb_toggle_edges",
		})
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
## `edge_types` (parallel to the pairs, from BinaryProtocolClient.get_edge_types(),
## empty = untyped) supplies the concrete predicate when `use_concrete_edges` is on;
## an edge with no type, or the toggle off, falls back to the "any" wildcard. Deduped
## by (src,tgt,edgeType) so two DIFFERENT concrete predicates between the same pair
## produce two triples, while wildcards collapse. Stores and returns `triples`.
func derive_triples(edge_pairs: PackedInt32Array, edge_types: PackedStringArray = PackedStringArray()) -> Array:
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
		var etype := "any"
		if use_concrete_edges and i < edge_types.size():
			var raw := edge_types[i]
			if raw != "":
				etype = raw
		var key := "%d>%d>%s" % [s, t, etype]
		if seen.has(key):
			continue
		seen[key] = true
		triples.append({
			"src": _vars[s],
			"edgeType": etype,
			"tgt": _vars[t],
		})
	return triples


## Build the POST /api/graph/query/pattern request body from the current pattern.
## `count_only` drives the debounced live-count preview; the full form is used at
## execute. Derives triples from `edge_pairs` (+ optional `edge_types`) first.
func build_pattern_payload(edge_pairs: PackedInt32Array, edge_types: PackedStringArray, count_only: bool, limit: int) -> Dictionary:
	return {
		"triples": derive_triples(edge_pairs, edge_types),
		"limit": limit,
		"countOnly": count_only,
	}


## One-line pattern summary for the HUD (call after derive_triples). E.g.
## "3 vars · 2 edges".
func pattern_summary() -> String:
	var vc := var_count()
	var ec := triples.size()
	return "%d var%s · %d edge%s" % [vc, "" if vc == 1 else "s", ec, "" if ec == 1 else "s"]
