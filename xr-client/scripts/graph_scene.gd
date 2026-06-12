extends Node3D

const AVATAR_TEMPLATE_PATH := "res://scenes/Avatar.tscn"

# Reconnect with exponential backoff, unbounded: a Quest sleeping in its case
# must rejoin when it wakes, however long that takes.
const RECONNECT_BASE_DELAY_SEC: float = 2.0
const RECONNECT_MAX_DELAY_SEC: float = 60.0

# Quest render budgets. When the graph exceeds these, the most important
# nodes (by server-computed centrality) and the heaviest edges (by weight)
# are kept — same importance language as the desktop client.
const NODE_INSTANCE_CAP: int = 4000
const EDGE_INSTANCE_CAP: int = 3000

# Grab hysteresis: engagement is decided Rust-side (XrInteraction
# ACTIVATION_THRESHOLD = 0.7); release happens here when the trigger falls
# below this lower bound, so a half-pulled trigger can't flicker drag
# start/end at the boundary.
const GRAB_RELEASE: float = 0.4

# Backend endpoint resolution (env-overridable). The Rust hot-path crate owns the
# wire; GDScript only supplies URLs/credentials and pumps the inbox each frame.
const DEFAULT_BACKEND_WS := "ws://localhost:4000"
const GRAPH_STREAM_PATH := "/wss"
const PRESENCE_PATH := "/ws/presence"
const DEFAULT_ROOM_URN := "urn:visionclaw:room:sha256-12-deadbeefcafe"
const DEFAULT_DISPLAY_NAME := "Quest User"

var _binary_client: RefCounted = null
var _presence_client: RefCounted = null
var _interaction: RefCounted = null
var _lod_policy: RefCounted = null
var _voice_router: RefCounted = null

var _avatars: Dictionary = {}
var _node_positions: Dictionary = {}
# Server-computed visual identity (community colour / centrality size /
# anomaly tint), keyed by node id. Populated from node_visuals_updated.
var _node_colors: Dictionary = {}
var _node_sizes: Dictionary = {}
var _node_centrality: Dictionary = {}
# Edge topology from initialGraphLoad, pre-capped by weight:
# flat [src0, tgt0, src1, tgt1, ...].
var _edge_pairs: PackedInt32Array = PackedInt32Array()

# Rendered-subset cache rebuilt by _update_multimesh; reused by the
# interaction ray so candidate lists never allocate twice per frame.
var _render_ids: PackedInt32Array = PackedInt32Array()
var _render_positions: PackedVector3Array = PackedVector3Array()

var _graph_ws_url: String = ""
var _presence_ws_url: String = ""
var _room_urn: String = ""
var _display_name: String = ""
var _graph_token: String = ""
var _nostr_secret_hex: String = ""
var _reconnect_attempts: int = 0
var _reconnect_timer: float = -1.0

# Server-authoritative drag state.
var _grabbed_id: int = -1
var _grab_controller: XRController3D = null
var _last_targeted_id: int = -1

@onready var nodes_multi: MultiMeshInstance3D = $NodesMulti
@onready var edges_multi: MultiMeshInstance3D = $EdgesMulti
@onready var avatar_spawner: Node3D = $AvatarSpawner
@onready var left_controller: XRController3D = $XROrigin3D/LeftController
@onready var right_controller: XRController3D = $XROrigin3D/RightController
@onready var hud: Node3D = get_node_or_null("XROrigin3D/XRCamera3D/HUD")

signal node_targeted_in_scene(node_id: int)
signal avatar_count_changed(count: int)
signal connection_status_changed(connected: bool)


func _ready() -> void:
	# gdext classes are #[class(no_init)] in Rust — construct via their static create() factory, not ClassDB.instantiate() (which cannot build a no_init class).
	_binary_client = BinaryProtocolClient.create()
	_presence_client = PresenceClientNode.create()
	_interaction = XrInteraction.create()
	_lod_policy = LodPolicy.create()
	_voice_router = SpatialVoiceRouter.create()

	if _binary_client != null:
		_binary_client.connect("position_updated", Callable(self, "_on_position_updated"))
		_binary_client.connect("connection_changed", Callable(self, "_on_connection_changed"))
		_binary_client.connect("node_visuals_updated", Callable(self, "_on_node_visuals_updated"))
		_binary_client.connect("topology_updated", Callable(self, "_on_topology_updated"))
	if _presence_client != null:
		_presence_client.connect("avatar_joined", Callable(self, "_on_avatar_joined"))
		_presence_client.connect("avatar_left", Callable(self, "_on_avatar_left"))
		_presence_client.connect("avatar_pose_updated", Callable(self, "_on_avatar_pose_updated"))
		if _presence_client.has_signal("presence_kicked"):
			_presence_client.connect("presence_kicked", Callable(self, "_on_presence_kicked"))
	if _voice_router != null and _voice_router.has_signal("voice_activity"):
		_voice_router.connect("voice_activity", Callable(self, "_on_voice_activity"))
	if _interaction != null:
		_interaction.connect("node_targeted", Callable(self, "_on_node_targeted"))
		_interaction.connect("node_grabbed", Callable(self, "_on_node_grabbed"))

	_wire_hud()
	_connect_from_env()


func _wire_hud() -> void:
	if hud == null:
		return
	connection_status_changed.connect(hud._on_connection_status)
	avatar_count_changed.connect(hud.set_avatar_count)
	if hud.has_signal("join_requested"):
		hud.join_requested.connect(_on_hud_join_requested)


func _on_hud_join_requested(room_urn: String) -> void:
	_room_urn = room_urn
	_reconnect_attempts = 0
	_attempt_connect()


# Resolve graph/presence endpoints and credentials from the environment and open
# both sockets. `XR_BACKEND_WS` is the base (scheme+host+port); the two well-known
# paths are appended. Empty token/secret => anonymous graph stream / ephemeral
# Nostr identity.
func _connect_from_env() -> void:
	var base: String = _env_or("XR_BACKEND_WS", DEFAULT_BACKEND_WS).rstrip("/")
	connect_to_server(
		base + GRAPH_STREAM_PATH,
		base + PRESENCE_PATH,
		_env_or("XR_ROOM_URN", DEFAULT_ROOM_URN),
		_env_or("XR_DISPLAY_NAME", DEFAULT_DISPLAY_NAME),
		OS.get_environment("XR_GRAPH_TOKEN"),
		OS.get_environment("XR_NOSTR_SECRET")
	)


func _env_or(name: String, fallback: String) -> String:
	var value: String = OS.get_environment(name)
	return value if value != "" else fallback


func connect_to_server(
	graph_ws_url: String,
	presence_ws_url: String,
	room_urn: String,
	display_name: String,
	graph_token: String = "",
	nostr_secret_hex: String = ""
) -> void:
	_graph_ws_url = graph_ws_url
	_presence_ws_url = presence_ws_url
	_room_urn = room_urn
	_display_name = display_name
	_graph_token = graph_token
	_nostr_secret_hex = nostr_secret_hex
	_reconnect_attempts = 0
	_attempt_connect()


func _attempt_connect() -> void:
	# Tear down any prior sockets so a reconnect never leaks a detached tokio task.
	if _binary_client != null and _binary_client.has_method("close"):
		_binary_client.close()
	if _presence_client != null and _presence_client.has_method("close"):
		_presence_client.close()
	if _binary_client != null and _binary_client.has_method("connect_to_url"):
		# The Nostr secret also NIP-98-authenticates the graph socket so
		# server-authoritative drag/pin messages are accepted.
		_binary_client.connect_to_url(_graph_ws_url, _graph_token, _nostr_secret_hex)
	if _presence_client != null and _presence_client.has_method("join"):
		_presence_client.join(_presence_ws_url, _room_urn, _display_name, _nostr_secret_hex)


func _physics_process(delta: float) -> void:
	# Drain network inboxes on the scene-tree thread; both clients emit their
	# signals from inside poll().
	if _binary_client != null and _binary_client.has_method("poll"):
		_binary_client.poll()
	if _presence_client != null and _presence_client.has_method("poll"):
		_presence_client.poll()
	_update_lod()
	_update_multimesh()
	_update_edge_multimesh()
	_update_interaction()
	_update_voice_listener()
	_tick_reconnect(delta)


func _update_lod() -> void:
	if _lod_policy == null or not _lod_policy.has_method("should_recompute"):
		return
	if not _lod_policy.should_recompute():
		return
	var camera: XRCamera3D = _find_xr_camera()
	if camera == null:
		return
	var cam_pos: Vector3 = camera.global_position
	for avatar_id: String in _avatars:
		var av: Node3D = _avatars[avatar_id]
		var dist: float = cam_pos.distance_to(av.global_position)
		var level: int = _lod_policy.classify_distance(dist)
		if av.has_method("set_lod_level"):
			av.set_lod_level(level)


func _update_multimesh() -> void:
	if nodes_multi == null or nodes_multi.multimesh == null:
		return
	var mm: MultiMesh = nodes_multi.multimesh
	var ids: Array = _node_positions.keys()

	# Importance cap: when the graph exceeds the Quest instance budget, keep
	# the highest-centrality nodes (server analytics; nodes with no analytics
	# yet rank 0 and drop first).
	if ids.size() > NODE_INSTANCE_CAP and _lod_policy != null and _lod_policy.has_method("visible_subset"):
		var centrality := PackedFloat32Array()
		centrality.resize(ids.size())
		for i: int in range(ids.size()):
			centrality[i] = _node_centrality.get(ids[i], 0.0)
		var subset: PackedInt32Array = _lod_policy.visible_subset(centrality, NODE_INSTANCE_CAP)
		var capped: Array = []
		for idx: int in subset:
			capped.append(ids[idx])
		ids = capped

	var count: int = ids.size()
	if mm.instance_count != count:
		mm.instance_count = count
	_render_ids.resize(count)
	_render_positions.resize(count)
	for i: int in range(count):
		var node_id: int = ids[i]
		var pos: Vector3 = _node_positions[node_id]
		var size: float = _node_sizes.get(node_id, 1.0)
		var xf := Transform3D(Basis.IDENTITY.scaled(Vector3(size, size, size)), pos)
		mm.set_instance_transform(i, xf)
		mm.set_instance_color(i, _node_colors.get(node_id, Color(0.55, 0.65, 0.85)))
		_render_ids[i] = node_id
		_render_positions[i] = pos


func _update_edge_multimesh() -> void:
	if edges_multi == null or edges_multi.multimesh == null:
		return
	var mm: MultiMesh = edges_multi.multimesh
	var pair_count: int = _edge_pairs.size() / 2
	var written: int = 0
	if mm.instance_count != pair_count:
		mm.instance_count = pair_count
	for i: int in range(pair_count):
		var src: int = _edge_pairs[i * 2]
		var tgt: int = _edge_pairs[i * 2 + 1]
		if not (_node_positions.has(src) and _node_positions.has(tgt)):
			continue
		var a: Vector3 = _node_positions[src]
		var b: Vector3 = _node_positions[tgt]
		var d: Vector3 = b - a
		var length: float = d.length()
		if length < 0.001:
			continue
		# Unit cylinder is Y-aligned: rotate Y onto the edge direction and
		# stretch to the span, positioned at the midpoint.
		var basis := Basis(Quaternion(Vector3.UP, d / length)).scaled(Vector3(1.0, length, 1.0))
		mm.set_instance_transform(written, Transform3D(basis, a + d * 0.5))
		written += 1
	# Park any unfilled instances (endpoints not yet streamed) at zero scale.
	for i: int in range(written, pair_count):
		mm.set_instance_transform(i, Transform3D(Basis.IDENTITY.scaled(Vector3.ZERO), Vector3.ZERO))


# Feed controller rays into the Rust interaction policy and drive the
# server-authoritative drag protocol. Idle cost is two trigger reads; the
# candidate arrays are only consulted while a trigger is live or a grab is
# in flight.
func _update_interaction() -> void:
	if _interaction == null or _binary_client == null:
		return

	# Active drag: the grabbed node rides the grab controller's aim point.
	if _grabbed_id != -1:
		var trigger_now: float = _controller_trigger(_grab_controller)
		if trigger_now < GRAB_RELEASE or _grab_controller == null:
			if _binary_client.has_method("send_drag_end"):
				_binary_client.send_drag_end(_grabbed_id)
			_pulse(_grab_controller, 0.3, 0.05)
			_grabbed_id = -1
			_grab_controller = null
		else:
			var hand_pos: Vector3 = _grab_controller.global_position - _grab_controller.global_transform.basis.z * 0.6
			_node_positions[_grabbed_id] = hand_pos  # optimistic local echo
			if _binary_client.has_method("send_drag_update"):
				_binary_client.send_drag_update(_grabbed_id, hand_pos)
		return

	for controller: XRController3D in [left_controller, right_controller]:
		if controller == null or not controller.get_is_active():
			continue
		var pinch: float = _controller_trigger(controller)
		if pinch < 0.05:
			continue
		_grab_controller = controller
		_interaction.evaluate_ray(
			controller.global_position,
			-controller.global_transform.basis.z,
			pinch,
			_render_ids,
			_render_positions
		)
		break


func _controller_trigger(controller: XRController3D) -> float:
	if controller == null or not controller.get_is_active():
		return 0.0
	return controller.get_float("trigger")


func _pulse(controller: XRController3D, amplitude: float, duration_sec: float) -> void:
	if controller != null and controller.get_is_active():
		controller.trigger_haptic_pulse("haptic", 0.0, amplitude, duration_sec, 0.0)


func _update_voice_listener() -> void:
	if _voice_router == null or not _voice_router.has_method("update_listener"):
		return
	var camera: XRCamera3D = _find_xr_camera()
	if camera == null:
		return
	var cam_pos: Vector3 = camera.global_position
	var cam_fwd: Vector3 = -camera.global_transform.basis.z
	var cam_up: Vector3 = camera.global_transform.basis.y
	_voice_router.update_listener(cam_pos, cam_fwd, cam_up)


func _tick_reconnect(delta: float) -> void:
	if _reconnect_timer < 0.0:
		return
	_reconnect_timer -= delta
	if _reconnect_timer <= 0.0:
		_reconnect_timer = -1.0
		_attempt_connect()


func _schedule_reconnect() -> void:
	_reconnect_attempts += 1
	var delay: float = minf(
		RECONNECT_BASE_DELAY_SEC * pow(2.0, float(_reconnect_attempts - 1)),
		RECONNECT_MAX_DELAY_SEC
	)
	_reconnect_timer = delay
	push_warning("GraphScene: reconnect attempt %d in %.1fs" % [_reconnect_attempts, delay])


func _find_xr_camera() -> XRCamera3D:
	# During an active OpenXR session the XRCamera3D is the viewport's current
	# camera — that is the canonical lookup.
	var viewport_cam: Camera3D = get_viewport().get_camera_3d()
	if viewport_cam is XRCamera3D:
		return viewport_cam as XRCamera3D
	# Fallback before the session is current: locate the XROrigin3D in the tree
	# and return its XRCamera3D child.
	var origin: Node = get_tree().current_scene.find_child("XROrigin3D", true, false) if get_tree().current_scene != null else null
	if origin != null:
		for child: Node in origin.get_children():
			if child is XRCamera3D:
				return child as XRCamera3D
	return null


func _on_connection_changed(connected: bool) -> void:
	emit_signal("connection_status_changed", connected)
	if connected:
		_reconnect_attempts = 0
	else:
		_schedule_reconnect()


func _on_position_updated(node_id: int, position: Vector3, _velocity: Vector3) -> void:
	# Server is authoritative — but never fight the local hand while dragging.
	if node_id == _grabbed_id:
		return
	_node_positions[node_id] = position


func _on_node_visuals_updated(node_id: int, community_id: int, centrality: float, anomaly: float) -> void:
	_node_colors[node_id] = _community_color(community_id, anomaly)
	_node_sizes[node_id] = clampf(0.5 + centrality * 1.5, 0.5, 2.0)
	_node_centrality[node_id] = centrality


# Deterministic community palette: golden-ratio hue walk gives well-separated
# colours for any community count (same approach as the desktop renderer).
# Anomalous nodes blend toward warning red.
func _community_color(community_id: int, anomaly: float) -> Color:
	var hue: float = fmod(float(community_id) * 0.61803398875, 1.0)
	var base: Color = Color.from_hsv(hue, 0.65, 0.9)
	if anomaly > 0.5:
		return base.lerp(Color(1.0, 0.15, 0.1), clampf((anomaly - 0.5) * 2.0, 0.0, 0.85))
	return base


func _on_topology_updated(_edge_count: int) -> void:
	var pairs: PackedInt32Array = _binary_client.get_edges()
	var weights: PackedFloat32Array = _binary_client.get_edge_weights()
	var total: int = weights.size()
	if total <= EDGE_INSTANCE_CAP:
		_edge_pairs = pairs
		return
	# Over budget: keep the heaviest edges. One-time sort on topology arrival.
	var order: Array = range(total)
	order.sort_custom(func(a: int, b: int) -> bool: return weights[a] > weights[b])
	var capped := PackedInt32Array()
	capped.resize(EDGE_INSTANCE_CAP * 2)
	for i: int in range(EDGE_INSTANCE_CAP):
		var e: int = order[i]
		capped[i * 2] = pairs[e * 2]
		capped[i * 2 + 1] = pairs[e * 2 + 1]
	_edge_pairs = capped
	push_warning("GraphScene: %d edges exceed budget; rendering heaviest %d" % [total, EDGE_INSTANCE_CAP])


func _on_avatar_joined(did: String, display_name: String, avatar_id: String) -> void:
	var template: PackedScene = load(AVATAR_TEMPLATE_PATH)
	if template == null:
		push_warning("Avatar template missing")
		return
	var avatar: Node = template.instantiate()
	avatar_spawner.add_child(avatar)
	avatar.set_meta("avatar_id", avatar_id)
	avatar.set_meta("did", did)
	if avatar.has_method("set_display_name"):
		avatar.set_display_name(display_name)
	_avatars[avatar_id] = avatar

	if _voice_router != null and _voice_router.has_method("attach_track"):
		_voice_router.attach_track(did, avatar.global_position)

	emit_signal("avatar_count_changed", _avatars.size())


func _on_avatar_left(avatar_id: String) -> void:
	if not _avatars.has(avatar_id):
		return
	var av: Node = _avatars[avatar_id]
	var did: String = av.get_meta("did", "")
	if _voice_router != null and _voice_router.has_method("detach_track") and did != "":
		_voice_router.detach_track(did)
	_avatars.erase(avatar_id)
	av.queue_free()
	emit_signal("avatar_count_changed", _avatars.size())


func _on_avatar_pose_updated(
	avatar_id: String,
	head_pos: Vector3,
	head_rot: Quaternion,
	has_left: bool,
	has_right: bool
) -> void:
	if not _avatars.has(avatar_id):
		return
	var av: Node3D = _avatars[avatar_id]
	if av.has_method("apply_pose"):
		av.apply_pose(head_pos, head_rot, has_left, has_right)


func _on_voice_activity(avatar_id: String, active: bool) -> void:
	if not _avatars.has(avatar_id):
		return
	var av: Node3D = _avatars[avatar_id]
	if av.has_method("set_speaking"):
		av.set_speaking(active)


func _on_node_targeted(node_id: int, _distance: float) -> void:
	emit_signal("node_targeted_in_scene", node_id)
	if node_id != _last_targeted_id:
		_last_targeted_id = node_id
		_pulse(_grab_controller, 0.15, 0.02)


func _on_node_grabbed(node_id: int, _position: Vector3) -> void:
	if _grabbed_id == node_id:
		return
	_grabbed_id = node_id
	var pos: Vector3 = _node_positions.get(node_id, Vector3.ZERO)
	if _binary_client != null and _binary_client.has_method("send_drag_start"):
		_binary_client.send_drag_start(node_id, pos)
	_pulse(_grab_controller, 0.6, 0.08)


func _on_presence_kicked(reason: String) -> void:
	push_warning("GraphScene: kicked from presence -- %s" % reason)
	_schedule_reconnect()
