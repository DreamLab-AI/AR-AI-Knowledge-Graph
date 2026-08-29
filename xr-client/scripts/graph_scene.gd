extends Node3D

const AVATAR_TEMPLATE_PATH := "res://scenes/Avatar.tscn"
const AGENT_TEMPLATE_PATH := "res://scenes/AgentAvatar.tscn"

# Radius the selection arbiter uses for an agent core (mesh radius + a small
# acquisition margin). The Rust dwell resolver applies its own target-size floor.
const AGENT_SELECT_RADIUS: float = 0.2

# The dot(camera_forward, dir_to_agent) above which the user is deemed to be
# gazing at an agent — a ~25° head-gaze cone (cos 25° ≈ 0.906), feeding the
# agent's mutual-gaze model.
const MUTUAL_GAZE_DOT: float = 0.9

# HTTP origin for the intervention decide POST, derived from the ws:// backend
# base by scheme swap (ws→http, wss→https) unless XR_BACKEND_HTTP overrides.

# Reconnect with exponential backoff, unbounded: a Quest sleeping in its case
# must rejoin when it wakes, however long that takes.
const RECONNECT_BASE_DELAY_SEC: float = 2.0
const RECONNECT_MAX_DELAY_SEC: float = 60.0

# Quest render budgets. When the graph exceeds these, the most important
# nodes (by server-computed centrality) and the heaviest edges (by weight)
# are kept — same importance language as the desktop client.
const NODE_INSTANCE_CAP: int = 640
const EDGE_INSTANCE_CAP: int = 3000

# XR fit-to-view: the server streams graph coordinates spanning hundreds of
# metres centred nowhere near the user. A flat client frames its camera to the
# graph AABB; XR cannot move the user's physical floor, so instead we scale and
# recentre the whole graph (GraphRoot) into a room-sized volume anchored in
# front of the standing user. Recomputed while the layout is still settling,
# then latched so it does not jitter once stable.
const GRAPH_TARGET_SPAN: float = 2.4          # metres — longest AABB axis maps to this
const GRAPH_ANCHOR := Vector3(0.0, 1.3, -1.6) # in front of XROrigin, ~chest height
const GRAPH_REFIT_SETTLE_SEC: float = 6.0     # keep refitting for this long after first nodes

# GraphRoot's fit-scale shrinks node/edge geometry along with positions, so a
# raw 0.5 m node sphere becomes sub-pixel. Nodes and edges compensate by the
# inverse of the fit scale to hold a fixed apparent world size. Mesh radii come
# from GraphScene.tscn (SphereMesh r=0.5, CylinderMesh r=0.03).
const NODE_MESH_RADIUS: float = 0.5
const NODE_WORLD_RADIUS: float = 0.03         # ~3 cm node in the room
const EDGE_MESH_RADIUS: float = 0.03
const EDGE_WORLD_RADIUS: float = 0.0015       # ~1.5 mm edge tube

# Grab hysteresis: engagement is decided Rust-side (XrInteraction
# ACTIVATION_THRESHOLD = 0.7); release happens here when the trigger falls
# below this lower bound, so a half-pulled trigger can't flicker drag
# start/end at the boundary.
const GRAB_RELEASE: float = 0.4

# Controller locomotion: trackpad/stick slides the XR rig through the graph
# volume in the head-facing horizontal plane.
const LOCOMOTION_SPEED: float = 1.5   # metres/second at full deflection
const LOCOMOTION_DEADZONE: float = 0.15

# Backend endpoint resolution (env-overridable). The Rust hot-path crate owns the
# wire; GDScript only supplies URLs/credentials and pumps the inbox each frame.
const DEFAULT_BACKEND_WS := "ws://localhost:4000"
const GRAPH_STREAM_PATH := "/wss"
const PRESENCE_PATH := "/ws/presence"
const DEFAULT_ROOM_URN := "urn:visionclaw:room:sha256-12-deadbeefcafe"
const DEFAULT_DISPLAY_NAME := "Quest User"

# RES-a / ADR-130 D3 liveness canary the on-device selection loop fires once,
# the first time the selection arbiter resolves a non-origin agent-node
# selection. Recorded via POST /api/canary/observe/{id} (unauthenticated JSON).
const CANARY_M4_RAY := "CANARY-VC-M4-RAY"

var _binary_client: RefCounted = null
var _presence_client: RefCounted = null
var _interaction: RefCounted = null
var _lod_policy: RefCounted = null
var _voice_router: RefCounted = null
var _proxemics: RefCounted = null
var _selection: RefCounted = null
var _gaze_tracker: RefCounted = null
var _nostr_auth: RefCounted = null

# Agent embodiments (M3), keyed by agent id, distinct from human peer _avatars.
var _agents: Dictionary = {}
var _agent_order: PackedStringArray = PackedStringArray()
var _agent_cases: Dictionary = {}  # agent_id -> {case_id, summary}
# Integer handles bridge String agent ids to the Rust arbiter's u32 candidate
# ids. Handles start at 1 so 0 stays reserved for "no selection".
var _agent_by_handle: Dictionary = {}  # int handle -> agent_id
var _next_handle: int = 1
# Proxemics re-solve memo: last user pose + agent count the arc was solved for.
var _last_solve_pos: Vector3 = Vector3.ZERO
var _last_solve_forward: Vector3 = Vector3.FORWARD
var _last_solve_count: int = -1
var _eye_gaze_supported: bool = false
# Once-per-frame LOD recompute decision, set by _update_lod, read by _update_agents.
var _lod_recompute: bool = false

var _avatars: Dictionary = {}
var _node_positions: Dictionary = {}
# Authoritative server positions treated as optimistic targets; the rendered
# _node_positions hunt toward these each frame (see _hunt_positions).
var _node_targets: Dictionary = {}
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
# Per-socket reconnect state. The graph (/wss) and presence (/ws/presence)
# sockets fail and recover independently, so each owns its own backoff counter
# and pending timer. Sharing one counter+timer (the old design) let a pending
# graph reconnect tear down a live presence socket and vice versa.
var _graph_reconnect_attempts: int = 0
var _graph_reconnect_timer: float = -1.0
var _presence_reconnect_attempts: int = 0
var _presence_reconnect_timer: float = -1.0

# One-shot latch for the M4-RAY canary (fires on the first resolved agent
# selection). The HTTPRequest is created in _ready so the POST never blocks the
# scene-tree thread.
var _m4_ray_observed: bool = false
var _observe_http: HTTPRequest = null

# Server-authoritative drag state.
var _grabbed_id: int = -1
var _grab_controller: XRController3D = null
var _grab_distance: float = 1.0
var _last_targeted_id: int = -1

# Movable world-anchored HUD state.
var _hud_grab_controller: XRController3D = null
var _hud_grab_offset: Transform3D = Transform3D.IDENTITY

@onready var graph_root: Node3D = $GraphRoot
@onready var nodes_multi: MultiMeshInstance3D = $GraphRoot/NodesMulti
@onready var edges_multi: MultiMeshInstance3D = $GraphRoot/EdgesMulti
@onready var avatar_spawner: Node3D = $GraphRoot/AvatarSpawner
@onready var agent_spawner: Node3D = $GraphRoot/AgentSpawner
@onready var left_controller: XRController3D = $XROrigin3D/LeftController
@onready var right_controller: XRController3D = $XROrigin3D/RightController
@onready var hud: Node3D = get_node_or_null("XROrigin3D/XRCamera3D/HUD")

signal node_targeted_in_scene(node_id: int)
signal avatar_count_changed(count: int)
signal agent_selected(agent_id: String, did_nostr: String)
signal connection_status_changed(connected: bool)


func _ready() -> void:
	# The flat fallback camera is only for non-XR (desktop diagnostic / no-HMD
	# fallback). In an XR session the XRCamera3D must drive both eyes, so release
	# the flat camera's `current` flag to avoid it stealing the viewport.
	var flat_cam: Camera3D = get_node_or_null("FlatFallbackCamera")
	if flat_cam != null:
		# XR session drives both eyes via XRCamera3D — the flat camera must NOT be
		# current or it steals the viewport and the HMD layer goes empty (SteamVR
		# falls back to its home environment). Only make it current for a non-XR
		# desktop/diagnostic run.
		flat_cam.current = not get_viewport().use_xr
	# Detach the HUD from the camera so it stops following the gaze; re-anchor it
	# in the play space as a world-fixed panel the user can grab and reposition
	# with a wand (see _update_hud_grab). Deferred so we don't reparent while the
	# XR node tree is still being built.
	if hud != null:
		_reparent_hud_to_world.call_deferred()
	# gdext classes are #[class(no_init)] in Rust — construct via their static create() factory, not ClassDB.instantiate() (which cannot build a no_init class).
	_binary_client = BinaryProtocolClient.create()
	_presence_client = PresenceClientNode.create()
	_interaction = XrInteraction.create()
	_lod_policy = LodPolicy.create()
	_voice_router = SpatialVoiceRouter.create()
	_proxemics = ProxemicsSolver.create()
	_selection = SelectionArbiterNode.create()
	_gaze_tracker = GazeTracker.create()
	# One signing identity for both the graph socket and the intervention POST.
	_nostr_auth = NostrAuth.create(OS.get_environment("XR_NOSTR_SECRET"))

	if _selection != null and _selection.has_signal("selection_made"):
		_selection.connect("selection_made", Callable(self, "_on_selection_made"))

	if _binary_client != null:
		_binary_client.connect("position_updated", Callable(self, "_on_position_updated"))
		_binary_client.connect("connection_changed", Callable(self, "_on_connection_changed"))
		_binary_client.connect("node_visuals_updated", Callable(self, "_on_node_visuals_updated"))
		_binary_client.connect("topology_updated", Callable(self, "_on_topology_updated"))
		if _binary_client.has_signal("text_message"):
			_binary_client.connect("text_message", Callable(self, "_on_graph_text"))
	if _presence_client != null:
		_presence_client.connect("avatar_joined", Callable(self, "_on_avatar_joined"))
		_presence_client.connect("avatar_left", Callable(self, "_on_avatar_left"))
		_presence_client.connect("avatar_pose_updated", Callable(self, "_on_avatar_pose_updated"))
		if _presence_client.has_signal("connection_changed"):
			_presence_client.connect("connection_changed", Callable(self, "_on_presence_connection_changed"))
		if _presence_client.has_signal("presence_kicked"):
			_presence_client.connect("presence_kicked", Callable(self, "_on_presence_kicked"))
	if _voice_router != null and _voice_router.has_signal("voice_activity"):
		_voice_router.connect("voice_activity", Callable(self, "_on_voice_activity"))
	if _interaction != null:
		_interaction.connect("node_targeted", Callable(self, "_on_node_targeted"))
		_interaction.connect("node_grabbed", Callable(self, "_on_node_grabbed"))

	# HTTPRequest for the M4-RAY liveness observe POST (created at runtime so the
	# scene file stays script-only; the request is async and non-blocking).
	_observe_http = HTTPRequest.new()
	add_child(_observe_http)

	_probe_eye_gaze()
	_wire_hud()
	_connect_from_env()


# Eye-gaze capability probe (copresence brief §Godot API availability; the
# godot #113717 hazard). Query support ONLY here — after OpenXR init in
# XRBoot — and feed the flag to the Rust gaze resolver, which keeps head-gaze
# primary and degrades eye-gaze to head unless supported. We never enable the
# XR_EXT_eye_gaze_interaction action-map binding blindly: the extension stays
# off on Quest 3 (which returns false), so the action-map error can never fire.
func _probe_eye_gaze() -> void:
	var xr: XRInterface = XRServer.find_interface("OpenXR")
	if xr != null and xr.has_method("is_eye_gaze_interaction_supported"):
		_eye_gaze_supported = xr.is_eye_gaze_interaction_supported()
	else:
		_eye_gaze_supported = false
	if _gaze_tracker != null and _gaze_tracker.has_method("set_eye_gaze_supported"):
		_gaze_tracker.set_eye_gaze_supported(_eye_gaze_supported)
	if not _eye_gaze_supported:
		print("GraphScene: eye-gaze unsupported (Quest 3 floor device) -- head-gaze primary")


func _wire_hud() -> void:
	if hud == null:
		return
	connection_status_changed.connect(hud._on_connection_status)
	avatar_count_changed.connect(hud.set_avatar_count)
	if hud.has_signal("join_requested"):
		hud.join_requested.connect(_on_hud_join_requested)
	if hud.has_method("configure_intervention"):
		hud.configure_intervention(_http_base(), _nostr_auth)


func _on_hud_join_requested(room_urn: String) -> void:
	_room_urn = room_urn
	_reset_reconnect_state()
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


# HTTP origin for the intervention decide POST. Prefer an explicit override,
# else swap the ws:// backend scheme to http:// (wss → https).
func _http_base() -> String:
	var override: String = OS.get_environment("XR_BACKEND_HTTP")
	if override != "":
		return override.rstrip("/")
	var ws: String = _env_or("XR_BACKEND_WS", DEFAULT_BACKEND_WS).rstrip("/")
	if ws.begins_with("wss://"):
		return "https://" + ws.substr(6)
	if ws.begins_with("ws://"):
		return "http://" + ws.substr(5)
	return ws


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
	_reset_reconnect_state()
	_attempt_connect()


func _reset_reconnect_state() -> void:
	_graph_reconnect_attempts = 0
	_graph_reconnect_timer = -1.0
	_presence_reconnect_attempts = 0
	_presence_reconnect_timer = -1.0


func _attempt_connect() -> void:
	# Initial connect (and full re-join): bring up both sockets. Reconnects are
	# per-socket via _connect_graph / _connect_presence so one socket's backoff
	# never disturbs the other.
	_connect_graph()
	_connect_presence()


func _connect_graph() -> void:
	# Tear down any prior graph socket so a reconnect never leaks a detached
	# tokio task, then re-open. The Nostr secret also NIP-98-authenticates the
	# graph socket so server-authoritative drag/pin messages are accepted.
	if _binary_client == null:
		return
	if _binary_client.has_method("close"):
		_binary_client.close()
	if _binary_client.has_method("connect_to_url"):
		_binary_client.connect_to_url(_graph_ws_url, _graph_token, _nostr_secret_hex)


func _connect_presence() -> void:
	if _presence_client == null:
		return
	if _presence_client.has_method("close"):
		_presence_client.close()
	if _presence_client.has_method("join"):
		_presence_client.join(_presence_ws_url, _room_urn, _display_name, _nostr_secret_hex)


func _physics_process(delta: float) -> void:
	# Drain network inboxes on the scene-tree thread; both clients emit their
	# signals from inside poll().
	if _binary_client != null and _binary_client.has_method("poll"):
		_binary_client.poll()
	if _presence_client != null and _presence_client.has_method("poll"):
		_presence_client.poll()
	_update_lod()
	_update_locomotion(delta)
	_update_hud_grab()
	_hunt_positions()
	_fit_graph_to_view(delta)
	_update_multimesh()
	_update_edge_multimesh()
	_update_interaction()
	_update_agents(delta)
	_update_selection(delta)
	_update_voice_listener()
	_tick_reconnect(delta)


# Trackpad/stick locomotion: slide the XR rig through the graph. Either wand's
# primary axis drives movement in the camera's horizontal facing plane (push
# up = move the way you look, sideways = strafe). Vertical fly is on the same
# stick's push when the trigger is not held, kept simple and comfortable.
func _update_locomotion(delta: float) -> void:
	var origin: XROrigin3D = get_node_or_null("XROrigin3D")
	var camera: XRCamera3D = _find_xr_camera()
	if origin == null or camera == null:
		return
	var move := Vector2.ZERO
	for controller: XRController3D in [left_controller, right_controller]:
		if controller == null or not controller.get_is_active():
			continue
		var v: Vector2 = controller.get_vector2("primary")
		if v.length() > move.length():
			move = v
	if move.length() < LOCOMOTION_DEADZONE:
		return
	var fwd: Vector3 = -camera.global_transform.basis.z
	fwd.y = 0.0
	fwd = fwd.normalized()
	var right: Vector3 = camera.global_transform.basis.x
	right.y = 0.0
	right = right.normalized()
	var step: Vector3 = (fwd * move.y + right * move.x) * LOCOMOTION_SPEED * delta
	origin.global_position += step


# Move the HUD out of the camera (gaze-locked) and anchor it in the play space
# as a world-fixed, wand-movable panel.
func _reparent_hud_to_world() -> void:
	if hud == null:
		return
	var origin: XROrigin3D = get_node_or_null("XROrigin3D")
	if origin == null:
		return
	var old_parent: Node = hud.get_parent()
	if old_parent != null and old_parent != origin:
		old_parent.remove_child(hud)
		origin.add_child(hud)
	# Comfortable default: down and to the left, angled toward the user.
	hud.transform = Transform3D(Basis(Vector3.UP, deg_to_rad(25.0)), Vector3(-0.6, 1.0, -0.9))
	hud.visible = true


# Grab the HUD with a wand: hold the grip button while the wand is near the panel
# to pick it up; it then rides the controller until grip releases.
func _update_hud_grab() -> void:
	if hud == null:
		return
	if _hud_grab_controller != null:
		var grip_now: float = _hud_grab_controller.get_float("grip")
		if grip_now < 0.5:
			_hud_grab_controller = null
		else:
			hud.global_transform = _hud_grab_controller.global_transform * _hud_grab_offset
		return
	for controller: XRController3D in [left_controller, right_controller]:
		if controller == null or not controller.get_is_active():
			continue
		if controller.get_float("grip") < 0.7:
			continue
		if controller.global_position.distance_to(hud.global_position) < 0.4:
			_hud_grab_controller = controller
			_hud_grab_offset = controller.global_transform.affine_inverse() * hud.global_transform
			_pulse(controller, 0.4, 0.05)
			break


func _update_lod() -> void:
	# should_recompute() ticks a frame counter, so it must be called EXACTLY once
	# per frame. Cache the decision here for _update_agents to reuse — a second
	# call would double-tick and desynchronise both LOD passes.
	_lod_recompute = false
	if _lod_policy == null or not _lod_policy.has_method("should_recompute"):
		return
	if not _lod_policy.should_recompute():
		return
	_lod_recompute = true
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


var _graph_scale: float = 1.0
var _fit_frame: int = 0
var _fit_target_scale: float = 0.0
var _fit_target_centre: Vector3 = Vector3.ZERO
# Frames between AABB recomputes (a full pass over _node_positions is O(N)).
const FIT_RECOMPUTE_FRAMES: int = 15
# Per-frame easing toward the fit target — small enough that a moving/re-settling
# graph rescales smoothly rather than snapping.
const FIT_EASE: float = 0.06


# Scale + recentre GraphRoot so the streamed node cloud always fills a room-sized
# volume in front of the user. Adaptive (not latched): the AABB is re-measured
# periodically and the transform eased toward the new target, so the graph stays
# room-sized as the force layout spreads, re-settles, or reheats after a physics
# change. Node/edge geometry compensates for `_graph_scale` so it never shrinks
# to specks.
func _fit_graph_to_view(_delta: float) -> void:
	if graph_root == null or _node_positions.is_empty():
		return
	_fit_frame += 1
	if _fit_frame % FIT_RECOMPUTE_FRAMES == 0 or _fit_target_scale == 0.0:
		# Collect positions per axis, EXCLUDING the grabbed node: a node held in
		# hand is dragged far from the settled cloud, and letting it into the AABB
		# inflates the fit so the whole graph rescales as you move ("edges scaling
		# off when moved"). Robust 5th–95th percentile bounds per axis then stop a
		# single stray outlier from driving the fit.
		var xs: PackedFloat32Array = PackedFloat32Array()
		var ys: PackedFloat32Array = PackedFloat32Array()
		var zs: PackedFloat32Array = PackedFloat32Array()
		for node_id: int in _node_positions:
			if node_id == _grabbed_id:
				continue
			var pos: Vector3 = _node_positions[node_id]
			xs.append(pos.x)
			ys.append(pos.y)
			zs.append(pos.z)
		if xs.size() > 0:
			var mn := Vector3(
				_percentile(xs, 0.05), _percentile(ys, 0.05), _percentile(zs, 0.05)
			)
			var mx := Vector3(
				_percentile(xs, 0.95), _percentile(ys, 0.95), _percentile(zs, 0.95)
			)
			var span: Vector3 = mx - mn
			var longest: float = maxf(span.x, maxf(span.y, span.z))
			if longest >= 0.001:
				_fit_target_scale = GRAPH_TARGET_SPAN / longest
				_fit_target_centre = (mn + mx) * 0.5
	if _fit_target_scale <= 0.0:
		return
	# Ease current scale/centre toward the target, then rebuild the transform:
	# world = anchor + scale * (server_pos - centre) → graph centre sits at anchor.
	var ease: float = FIT_EASE if _graph_scale > 0.0 else 1.0
	_graph_scale = lerpf(_graph_scale, _fit_target_scale, ease)
	graph_root.transform = Transform3D(
		Basis.IDENTITY.scaled(Vector3(_graph_scale, _graph_scale, _graph_scale)),
		GRAPH_ANCHOR - _fit_target_centre * _graph_scale
	)


# Linear-interpolated percentile (q in [0,1]) of an unsorted float array. Used to
# derive robust AABB bounds for the fit so a single outlier node can't drive the
# whole-graph rescale. Copies before sorting so caller data is untouched.
func _percentile(values: PackedFloat32Array, q: float) -> float:
	var n: int = values.size()
	if n == 0:
		return 0.0
	if n == 1:
		return values[0]
	var sorted: PackedFloat32Array = values.duplicate()
	sorted.sort()
	var rank: float = clampf(q, 0.0, 1.0) * float(n - 1)
	var lo: int = int(floor(rank))
	var hi: int = int(ceil(rank))
	if lo == hi:
		return sorted[lo]
	var frac: float = rank - float(lo)
	return lerpf(sorted[lo], sorted[hi], frac)


# Client-side optimistic position hunting: the server streams authoritative
# positions as targets; each frame the rendered position eases toward its target
# so motion stays smooth between (and independent of) update ticks — the same
# pattern the desktop client uses. The grabbed node is exempt (it tracks the
# hand locally and the server echoes it back).
const POSITION_HUNT_EASE: float = 0.06


func _hunt_positions() -> void:
	for node_id: int in _node_targets:
		if node_id == _grabbed_id:
			continue
		var cur: Vector3 = _node_positions.get(node_id, _node_targets[node_id])
		_node_positions[node_id] = cur.lerp(_node_targets[node_id], POSITION_HUNT_EASE)


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
		print("GraphScene DEBUG: multimesh instance_count %d -> %d" % [mm.instance_count, count])
		mm.instance_count = count
	_render_ids.resize(count)
	_render_positions.resize(count)
	for i: int in range(count):
		var node_id: int = ids[i]
		var pos: Vector3 = _node_positions[node_id]
		# Compensate the GraphRoot fit-scale so the node holds ~NODE_WORLD_RADIUS
		# in the room; _node_sizes (centrality, ~0.5–2.0) still modulates it.
		var comp: float = NODE_WORLD_RADIUS / (NODE_MESH_RADIUS * _graph_scale)
		var size: float = _node_sizes.get(node_id, 1.0) * comp
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
	# Only draw an edge when BOTH endpoints are actually rendered (in the top-N
	# displayed subset). Testing _node_positions instead would draw edges to the
	# ~12k streamed-but-unrendered nodes, producing long streaks to invisible
	# endpoints.
	var shown := {}
	for rid: int in _render_ids:
		shown[rid] = true
	for i: int in range(pair_count):
		var src: int = _edge_pairs[i * 2]
		var tgt: int = _edge_pairs[i * 2 + 1]
		if not (shown.has(src) and shown.has(tgt)):
			continue
		var a: Vector3 = _node_positions[src]
		var b: Vector3 = _node_positions[tgt]
		var d: Vector3 = b - a
		var length: float = d.length()
		if length < 0.001:
			continue
		# Unit cylinder is Y-aligned: rotate Y onto the edge direction and stretch
		# to the span, positioned at the midpoint. Build the rotation robustly —
		# Quaternion(UP, dir) is degenerate when dir is (anti)parallel to UP, which
		# for a Y-tall/thin layout is most edges, producing wrong/vertical tubes.
		var dir: Vector3 = d / length
		var q: Quaternion
		var dp: float = Vector3.UP.dot(dir)
		if dp > 0.9999:
			q = Quaternion.IDENTITY
		elif dp < -0.9999:
			q = Quaternion(Vector3.RIGHT, PI)  # flip Y→-Y about X
		else:
			var axis: Vector3 = Vector3.UP.cross(dir).normalized()
			q = Quaternion(axis, acos(clampf(dp, -1.0, 1.0)))
		var er: float = EDGE_WORLD_RADIUS / (EDGE_MESH_RADIUS * _graph_scale)
		var basis := Basis(q).scaled(Vector3(er, length, er))
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
	_ensure_controller_rays()
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
			# Keep the node at the distance it was grabbed at (ride the ray), rather
			# than snapping it to a fixed point in front of the wand. World→server
			# because the server and _node_positions work in GraphRoot-local space.
			var ray_origin: Vector3 = _grab_controller.global_position
			var ray_dir: Vector3 = -_grab_controller.global_transform.basis.z
			var hand_world: Vector3 = ray_origin + ray_dir * _grab_distance
			var hand_server: Vector3 = graph_root.global_transform.affine_inverse() * hand_world
			_node_positions[_grabbed_id] = hand_server  # optimistic local echo
			_node_targets[_grabbed_id] = hand_server
			if _binary_client.has_method("send_drag_update"):
				_binary_client.send_drag_update(_grabbed_id, hand_server)
		return

	for controller: XRController3D in [left_controller, right_controller]:
		if controller == null or not controller.get_is_active():
			continue
		var pinch: float = _controller_trigger(controller)
		if pinch < 0.05:
			continue
		_grab_controller = controller
		# Node render positions are in GraphRoot's scaled space; transform to world
		# so the world-space wand ray and the interaction's metre-space thresholds
		# actually intersect the nodes.
		var gxf: Transform3D = graph_root.global_transform
		var world_positions := PackedVector3Array()
		world_positions.resize(_render_positions.size())
		for i: int in range(_render_positions.size()):
			world_positions[i] = gxf * _render_positions[i]
		_interaction.evaluate_ray(
			controller.global_position,
			-controller.global_transform.basis.z,
			pinch,
			_render_ids,
			world_positions
		)
		break


func _controller_trigger(controller: XRController3D) -> float:
	if controller == null or not controller.get_is_active():
		return 0.0
	return controller.get_float("trigger")


# Desktop-parity laser pointers. A thin emissive beam is parented to each wand,
# extending down the aim pose's -Z (the same ray fed to the Rust interaction
# policy). Created once, then shown only while the controller is tracking and
# tinted green as the trigger engages so the user sees the selection ray.
const RAY_LENGTH: float = 5.0
const RAY_IDLE_COLOR := Color(0.35, 0.7, 1.0)   # cyan when tracking, idle
const RAY_ACTIVE_COLOR := Color(0.3, 1.0, 0.4)  # green as the trigger pulls


func _ensure_controller_rays() -> void:
	for controller: XRController3D in [left_controller, right_controller]:
		if controller == null:
			continue
		var ray: MeshInstance3D = controller.get_node_or_null("AimRay") as MeshInstance3D
		if ray == null:
			var mesh := BoxMesh.new()
			# Thin beam down -Z; the box is centred so offset it forward by half.
			mesh.size = Vector3(0.006, 0.006, RAY_LENGTH)
			var mat := StandardMaterial3D.new()
			mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
			mat.emission_enabled = true
			mat.emission = RAY_IDLE_COLOR
			mat.albedo_color = RAY_IDLE_COLOR
			ray = MeshInstance3D.new()
			ray.name = "AimRay"
			ray.mesh = mesh
			ray.material_override = mat
			ray.position = Vector3(0.0, 0.0, -RAY_LENGTH * 0.5)
			controller.add_child(ray)
		var active: bool = controller.get_is_active()
		ray.visible = active
		if active:
			var cur_mat := ray.material_override as StandardMaterial3D
			if cur_mat != null:
				var col: Color = RAY_ACTIVE_COLOR if _controller_trigger(controller) > 0.05 else RAY_IDLE_COLOR
				cur_mat.emission = col
				cur_mat.albedo_color = col


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


# --- Agent embodiment (M3) + selection (M4) ---------------------------------

## Spawn a geometric agent embodiment. Distinct from a human presence peer: an
## agent is a utility actor placed on the proxemics arc, keyed by did:nostr.
func spawn_agent(agent_id: String, display_name: String, did: String, verified: bool) -> void:
	if _agents.has(agent_id):
		return
	var template: PackedScene = load(AGENT_TEMPLATE_PATH)
	if template == null:
		push_warning("AgentAvatar template missing")
		return
	var agent: Node3D = template.instantiate()
	agent_spawner.add_child(agent)
	var handle: int = _next_handle
	_next_handle += 1
	agent.set_meta("agent_id", agent_id)
	agent.set_meta("handle", handle)
	agent.set_meta("did", did)
	if agent.has_method("set_avatar_identity"):
		agent.set_avatar_identity(display_name, did, verified)
	_agents[agent_id] = agent
	_agent_by_handle[handle] = agent_id
	_agent_order.append(agent_id)
	if _selection != null and _selection.has_method("register_identity"):
		_selection.register_identity(handle, did)
	_last_solve_count = -1  # force an arc re-solve on the next frame
	_resolve_agent_arc()


func despawn_agent(agent_id: String) -> void:
	if not _agents.has(agent_id):
		return
	var agent: Node3D = _agents[agent_id]
	var handle: int = agent.get_meta("handle", 0)
	_agent_by_handle.erase(handle)
	_agents.erase(agent_id)
	_agent_cases.erase(agent_id)
	var idx: int = _agent_order.find(agent_id)
	if idx != -1:
		_agent_order.remove_at(idx)
	agent.queue_free()
	_last_solve_count = -1
	_resolve_agent_arc()
	_publish_case_count()


## An agent raised a broker case awaiting the operator's approval. Drives the
## agent's own state (saturated colour + attention motion) and, if selected,
## the HUD intervention panel.
func set_agent_case(agent_id: String, case_id: String, summary: String) -> void:
	if not _agents.has(agent_id):
		return
	_agent_cases[agent_id] = {"case_id": case_id, "summary": summary}
	var agent: Node3D = _agents[agent_id]
	if agent.has_method("apply_signal"):
		agent.apply_signal(agent_avatar_signal_approval_requested())
	_publish_case_count()


func clear_agent_case(agent_id: String) -> void:
	if not _agent_cases.has(agent_id):
		return
	_agent_cases.erase(agent_id)
	if _agents.has(agent_id):
		var agent: Node3D = _agents[agent_id]
		if agent.has_method("apply_signal"):
			agent.apply_signal(agent_avatar_signal_approval_granted())
	_publish_case_count()


static func agent_avatar_signal_approval_requested() -> int:
	return 2  # AgentSignal::ApprovalRequested (avatar_state.rs:418)


static func agent_avatar_signal_approval_granted() -> int:
	return 3  # AgentSignal::ApprovalGranted


func _publish_case_count() -> void:
	if hud != null and hud.has_method("set_case_count"):
		hud.set_case_count(_agent_cases.size())


# Place agents on a forward arc in the social band via the Rust proxemics solver
# (Hall's zones, ±60°, 1.5–2.5 m). Re-solve only on a membership or user-pose
# change (the solver's own gate keeps it off the per-frame path).
func _resolve_agent_arc() -> void:
	if _proxemics == null or _agent_order.is_empty():
		return
	var camera: XRCamera3D = _find_xr_camera()
	var user_pos: Vector3 = camera.global_position if camera != null else Vector3.ZERO
	var user_fwd: Vector3 = -camera.global_transform.basis.z if camera != null else Vector3.FORWARD
	var count: int = _agent_order.size()

	if count == _last_solve_count and _proxemics.has_method("should_resolve"):
		if not _proxemics.should_resolve(
			_last_solve_pos, _last_solve_forward, _last_solve_count,
			user_pos, user_fwd, count
		):
			return

	var positions: PackedVector3Array = _proxemics.solve(user_pos, user_fwd, count)
	for i: int in range(mini(positions.size(), count)):
		var agent_id: String = _agent_order[i]
		if _agents.has(agent_id):
			(_agents[agent_id] as Node3D).global_position = positions[i]
	_last_solve_pos = user_pos
	_last_solve_forward = user_fwd
	_last_solve_count = count


func _update_agents(delta: float) -> void:
	if _agents.is_empty():
		return
	_resolve_agent_arc()
	var camera: XRCamera3D = _find_xr_camera()
	if camera == null:
		return
	var cam_pos: Vector3 = camera.global_position
	var cam_fwd: Vector3 = -camera.global_transform.basis.z
	var dt_us: int = int(delta * 1_000_000.0)
	# Reuse _update_lod's once-per-frame recompute decision (never re-tick here).
	var recompute_lod: bool = _lod_recompute and _lod_policy != null \
		and _lod_policy.has_method("agent_feature_mask")

	for agent_id: String in _agents:
		var agent: Node3D = _agents[agent_id]
		var to_agent: Vector3 = agent.global_position - cam_pos
		var dist: float = to_agent.length()
		# Mutual-gaze test: is the user's head-gaze pointed at this agent?
		var gazing: bool = dist > 0.001 and cam_fwd.dot(to_agent / dist) > MUTUAL_GAZE_DOT
		if agent.has_method("tick_attention"):
			agent.tick_attention(gazing, cam_pos - agent.global_position, false, 0, Vector3.ZERO, dt_us)
		# LOD feature mask (badge drops first, then cone, core billboards).
		if recompute_lod and agent.has_method("set_feature_mask"):
			var level: int = _lod_policy.classify_distance(dist)
			agent.set_feature_mask(_lod_policy.agent_feature_mask(level))


# Feed the three-resolver arbiter this frame's controller rays, smoothed gaze,
# and agent candidates; drive the dwell reticle from the charge ratio.
func _update_selection(delta: float) -> void:
	if _selection == null or _agents.is_empty():
		return
	var camera: XRCamera3D = _find_xr_camera()
	if camera == null:
		return

	_selection.begin_frame()
	var any_controller: bool = false
	var hand: int = 0
	for controller: XRController3D in [left_controller, right_controller]:
		if controller != null and controller.get_is_active():
			any_controller = true
			var trig: float = controller.get_float("trigger")
			_selection.push_controller(
				hand,
				controller.global_position,
				-controller.global_transform.basis.z,
				trig,
				true,
				trig > 0.7
			)
		hand += 1

	# Smoothed head-gaze ray (eye-gaze only if the probe found hardware support).
	var cam_pos: Vector3 = camera.global_position
	var cam_fwd: Vector3 = -camera.global_transform.basis.z
	if _gaze_tracker != null and _gaze_tracker.has_method("resolve"):
		var gdir: Vector3 = _gaze_tracker.resolve(cam_pos, cam_fwd, delta, _eye_gaze_supported)
		_selection.set_gaze(cam_pos, gdir)
	else:
		_selection.set_gaze(cam_pos, cam_fwd)

	# Candidates = the agents (handles), with the core radius + margin.
	var ids := PackedInt32Array()
	var positions := PackedVector3Array()
	var radii := PackedFloat32Array()
	for agent_id: String in _agents:
		var agent: Node3D = _agents[agent_id]
		ids.append(agent.get_meta("handle", 0))
		positions.append(agent.global_position)
		radii.append(AGENT_SELECT_RADIUS)
	_selection.set_candidates(ids, positions, radii)

	_selection.tick(not any_controller, Time.get_ticks_usec(), int(delta * 1_000_000.0))

	if hud != null and hud.has_method("set_dwell_charge") and _selection.has_method("charge_ratio"):
		hud.set_dwell_charge(_selection.charge_ratio())


func _on_selection_made(handle: int, did_nostr: String, _resolver: int) -> void:
	if not _agent_by_handle.has(handle):
		return
	var agent_id: String = _agent_by_handle[handle]
	emit_signal("agent_selected", agent_id, did_nostr)
	# M4-RAY: the arbiter resolved a non-origin agent-node selection — fire the
	# liveness canary once (RES-a / ADR-130 D3).
	_fire_m4_ray_canary(agent_id, did_nostr)
	# If the selected agent is awaiting approval, open the intervention panel.
	if _agent_cases.has(agent_id) and hud != null and hud.has_method("show_case"):
		var c: Dictionary = _agent_cases[agent_id]
		hud.show_case(c.get("case_id", ""), c.get("summary", ""))


# One-shot POST /api/canary/observe/CANARY-VC-M4-RAY, latched so it fires exactly
# once per session — on the first resolved agent selection. The route is
# unauthenticated (a thin JSON adapter over the LivenessHarness), so no NIP-98
# header is needed. Fail-open: a failed dispatch un-latches so a later selection
# can retry, and never blocks the scene.
func _fire_m4_ray_canary(agent_id: String, did_nostr: String) -> void:
	if _m4_ray_observed or _observe_http == null:
		return
	_m4_ray_observed = true
	var url: String = "%s/api/canary/observe/%s" % [_http_base(), CANARY_M4_RAY]
	var body: Dictionary = {
		"evidence": "xr-client selection arbiter resolved agent %s (%s)" % [agent_id, did_nostr],
	}
	var headers := PackedStringArray(["Content-Type: application/json"])
	var err: int = _observe_http.request(url, headers, HTTPClient.METHOD_POST, JSON.stringify(body))
	if err != OK:
		push_warning("GraphScene: M4-RAY observe POST failed to start (%d)" % err)
		_m4_ray_observed = false


# Inbound non-topology JSON on the multiplexed /wss graph socket (M2). The Rust
# BinaryProtocolClient forwards these verbatim via text_message; we parse the
# envelope and route broker:new_case to the HUD intervention panel's entry point.
func _on_graph_text(json: String) -> void:
	var parsed: Variant = JSON.parse_string(json)
	if typeof(parsed) != TYPE_DICTIONARY:
		return
	var msg: Dictionary = parsed
	match str(msg.get("type", "")):
		"broker:new_case":
			# A malformed frame can carry a non-Dictionary payload (string, null,
			# array); passing that to a Dictionary-typed param crashes
			# _physics_process. Guard the type before routing.
			var p: Variant = msg.get("payload", {})
			if typeof(p) == TYPE_DICTIONARY:
				_handle_broker_new_case(p)


func _handle_broker_new_case(payload: Dictionary) -> void:
	var case_id: String = str(payload.get("caseId", ""))
	if case_id.is_empty():
		return
	var summary: String = str(payload.get("title", ""))
	# Surface the case through the HUD's existing intervention entry point.
	if hud != null and hud.has_method("show_case"):
		hud.show_case(case_id, summary)


func _tick_reconnect(delta: float) -> void:
	# Each socket owns its own countdown; firing one reconnects only that socket
	# so a pending graph reconnect can't tear down a live presence socket.
	if _graph_reconnect_timer >= 0.0:
		_graph_reconnect_timer -= delta
		if _graph_reconnect_timer <= 0.0:
			_graph_reconnect_timer = -1.0
			_connect_graph()
	if _presence_reconnect_timer >= 0.0:
		_presence_reconnect_timer -= delta
		if _presence_reconnect_timer <= 0.0:
			_presence_reconnect_timer = -1.0
			_connect_presence()


func _backoff_delay(attempts: int) -> float:
	return minf(
		RECONNECT_BASE_DELAY_SEC * pow(2.0, float(attempts - 1)),
		RECONNECT_MAX_DELAY_SEC
	)


func _schedule_graph_reconnect() -> void:
	_graph_reconnect_attempts += 1
	_graph_reconnect_timer = _backoff_delay(_graph_reconnect_attempts)
	push_warning("GraphScene: graph reconnect attempt %d in %.1fs" % [
		_graph_reconnect_attempts, _graph_reconnect_timer])


func _schedule_presence_reconnect() -> void:
	_presence_reconnect_attempts += 1
	_presence_reconnect_timer = _backoff_delay(_presence_reconnect_attempts)
	push_warning("GraphScene: presence reconnect attempt %d in %.1fs" % [
		_presence_reconnect_attempts, _presence_reconnect_timer])


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


# Graph (/wss) socket state. `connected` is emitted only once the subscribe
# handshake completes (transport.rs), so a live signal always means a usable
# stream. Clear any pending graph reconnect timer on connect, else a queued
# _tick_reconnect would tear the fresh socket back down.
func _on_connection_changed(connected: bool) -> void:
	emit_signal("connection_status_changed", connected)
	if connected:
		_graph_reconnect_attempts = 0
		_graph_reconnect_timer = -1.0
	else:
		_schedule_graph_reconnect()


# Presence (/ws/presence) socket state, on its own independent backoff.
func _on_presence_connection_changed(connected: bool) -> void:
	if connected:
		_presence_reconnect_attempts = 0
		_presence_reconnect_timer = -1.0
	else:
		_schedule_presence_reconnect()


var _dbg_pos_frames: int = 0


func _on_position_updated(node_id: int, position: Vector3, _velocity: Vector3) -> void:
	# Server is authoritative — but never fight the local hand while dragging.
	_dbg_pos_frames += 1
	if _dbg_pos_frames == 1 or _dbg_pos_frames % 5000 == 0:
		print("GraphScene DEBUG: position update #%d node=%d pos=%s" % [_dbg_pos_frames, node_id, position])
	if node_id == _grabbed_id:
		return
	# Store as an optimistic target; the render position hunts toward it each
	# frame (_hunt_positions). Seed the render position on first appearance so a
	# new node doesn't ease in from the origin.
	if not _node_positions.has(node_id):
		_node_positions[node_id] = position
	_node_targets[node_id] = position


func _on_node_visuals_updated(node_id: int, community_id: int, centrality: float, anomaly: float) -> void:
	_node_colors[node_id] = _community_color(community_id, anomaly, node_id)
	_node_sizes[node_id] = clampf(0.5 + centrality * 1.5, 0.5, 2.0)
	_node_centrality[node_id] = centrality


# Deterministic community palette: golden-ratio hue walk gives well-separated
# colours for any community count (same approach as the desktop renderer).
# When the server has not computed communities (all community_id == 0, which
# collapses every node to hue 0 / red), fall back to a per-node golden-ratio hue
# so the graph still reads as varied rather than a uniform mass. Anomalous nodes
# blend toward warning red regardless.
func _community_color(community_id: int, anomaly: float, node_id: int = 0) -> Color:
	var key: int = community_id if community_id != 0 else node_id
	var hue: float = fmod(float(key) * 0.61803398875, 1.0)
	var base: Color = Color.from_hsv(hue, 0.6, 0.95)
	if anomaly > 0.5:
		return base.lerp(Color(1.0, 0.15, 0.1), clampf((anomaly - 0.5) * 2.0, 0.0, 0.85))
	return base


func _on_topology_updated(_edge_count: int) -> void:
	var pairs: PackedInt32Array = _binary_client.get_edges()
	var weights: PackedFloat32Array = _binary_client.get_edge_weights()
	var total: int = weights.size()
	print("GraphScene DEBUG: topology arrived edges=%d positions_known=%d" % [total, _node_positions.size()])
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
	# M1 (PRD-023 WP-9): render the did:nostr the presence join carried. The
	# badge reads unverified until the client runs the Schnorr-challenge check
	# (ADR-130 Decision 6) — the server-vouched roster DID is not client-trusted
	# on presentation (invariant 2).
	if avatar.has_method("set_avatar_identity"):
		avatar.set_avatar_identity(display_name, did, false)
	elif avatar.has_method("set_display_name"):
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
	has_right: bool,
	left_pos: Vector3,
	left_rot: Quaternion,
	right_pos: Vector3,
	right_rot: Quaternion
) -> void:
	if not _avatars.has(avatar_id):
		return
	var av: Node3D = _avatars[avatar_id]
	if av.has_method("apply_pose"):
		# Forward articulated hand transforms so remote avatars get real hands;
		# has_left/has_right gate whether the pos/rot are meaningful.
		av.apply_pose(
			head_pos, head_rot, has_left, has_right,
			left_pos, left_rot, right_pos, right_rot
		)


func _on_voice_activity(avatar_id: String, active: bool) -> void:
	if not _avatars.has(avatar_id):
		return
	var av: Node3D = _avatars[avatar_id]
	if av.has_method("set_speaking"):
		av.set_speaking(active)


func _on_node_targeted(node_id: int, _distance: float) -> void:
	emit_signal("node_targeted_in_scene", node_id)
	if node_id != _last_targeted_id:
		print("GraphScene DEBUG: node_targeted id=%d dist=%.2f" % [node_id, _distance])
		_last_targeted_id = node_id
		_pulse(_grab_controller, 0.15, 0.02)


func _on_node_grabbed(node_id: int, _position: Vector3) -> void:
	if _grabbed_id == node_id:
		return
	_grabbed_id = node_id
	print("GraphScene DEBUG: node_grabbed id=%d" % node_id)
	# Remember how far along the ray the node was so the drag preserves depth.
	if _grab_controller != null:
		var node_world: Vector3 = graph_root.global_transform * _node_positions.get(node_id, Vector3.ZERO)
		_grab_distance = clampf(_grab_controller.global_position.distance_to(node_world), 0.2, 6.0)
	var pos: Vector3 = _node_positions.get(node_id, Vector3.ZERO)
	if _binary_client != null and _binary_client.has_method("send_drag_start"):
		_binary_client.send_drag_start(node_id, pos)
	_pulse(_grab_controller, 0.6, 0.08)


func _on_presence_kicked(reason: String) -> void:
	push_warning("GraphScene: kicked from presence -- %s" % reason)
	_schedule_presence_reconnect()
