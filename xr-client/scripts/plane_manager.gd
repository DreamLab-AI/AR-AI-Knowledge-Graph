class_name PlaneManager
extends Node3D

## Semantic planes (flagship Phase D). Executing a visual query spawns one result
## subgraph per binding, each the EXACT matched pattern (its nodes + the pattern's
## edges) lifted onto a parallel horizontal layer stacked along +Y above the main
## graph. Layout is pure presentation — the subgraph nodes are NOT simulated; each
## plane is a clean client-side copy built by RenderStore.build_plane_* (positions
## reused from the store, offset in Y), so spawning/dismissing never touches the
## physics or the base graph.
##
## Parented under GraphRoot so the +Y offset (server space) rides the same fit
## transform as the graph; `plane_gap` is chosen server-side so layers land ~0.5 m
## apart after the fit scale. Each plane is two MultiMeshInstance3D children (nodes
## + edges) plus a Label3D header; dismiss/clear frees them.

## Hard cap on spawned planes (matches the query limit).
const PLANE_CAP: int = 24

var _client: Object = null          # BinaryProtocolClient
var _node_mesh: Mesh = null
var _edge_mesh: Mesh = null
var _node_material: Material = null
var _edge_material: Material = null

var _plane_count: int = 0


## Wire the meshes/materials to copy from the main graph multimeshes.
func configure(client: Object, node_mesh: Mesh, edge_mesh: Mesh, node_material: Material, edge_material: Material) -> void:
	_client = client
	_node_mesh = node_mesh
	_edge_mesh = edge_mesh
	_node_material = node_material
	_edge_material = edge_material


## How many planes are currently shown.
func plane_count() -> int:
	return _plane_count


## Build result planes from `bindings` (each a var→id Dictionary) and the query
## `triples` (each {src,edgeType,tgt} with var-name endpoints). `plane_gap` is the
## per-layer +Y offset in server space. Node/edge sizing mirrors the main graph's
## compensation factors. Returns the number of planes spawned (capped at PLANE_CAP).
func build(
	bindings: Array,
	triples: Array,
	plane_gap: float,
	scale_comp: float,
	size_lo: float,
	size_hi: float,
	edge_radius_comp: float,
	label_fn: Callable
) -> int:
	clear()
	if _client == null:
		return 0
	var count: int = mini(bindings.size(), PLANE_CAP)
	for i in range(count):
		if typeof(bindings[i]) != TYPE_DICTIONARY:
			continue
		var b: Dictionary = bindings[i]
		# Node ids = the binding's variable assignments.
		var ids := PackedInt32Array()
		for k in b.keys():
			ids.append(int(b[k]))
		# Edges = each triple's endpoints resolved through this binding.
		var pairs := PackedInt32Array()
		for tr: Variant in triples:
			if typeof(tr) != TYPE_DICTIONARY:
				continue
			var sv := str((tr as Dictionary).get("src", ""))
			var tv := str((tr as Dictionary).get("tgt", ""))
			if b.has(sv) and b.has(tv):
				pairs.append(int(b[sv]))
				pairs.append(int(b[tv]))
		var y_offset: float = float(i + 1) * plane_gap
		_spawn_plane(ids, pairs, y_offset, scale_comp, size_lo, size_hi, edge_radius_comp, i, b, label_fn)
	_plane_count = count
	return count


func _spawn_plane(
	ids: PackedInt32Array,
	pairs: PackedInt32Array,
	y_offset: float,
	scale_comp: float,
	size_lo: float,
	size_hi: float,
	edge_radius_comp: float,
	index: int,
	binding: Dictionary,
	label_fn: Callable
) -> void:
	# Nodes.
	if _client.has_method("build_plane_node_buffer"):
		var node_buf: PackedFloat32Array = _client.build_plane_node_buffer(ids, y_offset, scale_comp, size_lo, size_hi)
		var nmm := MultiMesh.new()
		nmm.transform_format = MultiMesh.TRANSFORM_3D
		nmm.use_colors = true
		nmm.use_custom_data = true
		nmm.mesh = _node_mesh
		nmm.instance_count = node_buf.size() / 20
		if nmm.instance_count > 0:
			nmm.buffer = node_buf
		var nmi := MultiMeshInstance3D.new()
		nmi.multimesh = nmm
		if _node_material != null:
			nmi.material_override = _node_material
		add_child(nmi)
	# Edges.
	if _client.has_method("build_plane_edge_buffer") and pairs.size() >= 2:
		var edge_buf: PackedFloat32Array = _client.build_plane_edge_buffer(pairs, y_offset, edge_radius_comp)
		var emm := MultiMesh.new()
		emm.transform_format = MultiMesh.TRANSFORM_3D
		emm.mesh = _edge_mesh
		emm.instance_count = edge_buf.size() / 12
		if emm.instance_count > 0:
			emm.buffer = edge_buf
		var emi := MultiMeshInstance3D.new()
		emi.multimesh = emm
		if _edge_material != null:
			emi.material_override = _edge_material
		add_child(emi)
	# Header label at the plane's first node, lifted with it.
	if label_fn.is_valid() and ids.size() > 0 and _client.has_method("node_position"):
		var anchor_id := int(ids[0])
		var pos: Vector3 = _client.node_position(anchor_id)
		pos.y += y_offset
		var lbl := Label3D.new()
		lbl.text = "#%d  %s" % [index + 1, str(label_fn.call(binding))]
		lbl.billboard = BaseMaterial3D.BILLBOARD_ENABLED
		lbl.no_depth_test = true
		lbl.pixel_size = 0.002
		lbl.modulate = Color(0.85, 0.95, 1.0)
		lbl.position = pos
		add_child(lbl)


## Free every plane.
func clear() -> void:
	for c in get_children():
		c.queue_free()
	_plane_count = 0
