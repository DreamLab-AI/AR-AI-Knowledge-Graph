extends Node3D

## In-headset HUD. The visible half of M2 (PRD-023 WP-9 / COM-18): a per-agent
## intervention panel bound to the same broker decide path the desktop uses, an
## ambient ACSP open-case indicator, and the gaze-dwell charging reticle. All
## decision logic and signing stay in Rust (NostrAuth); this script only wires
## signals to scene nodes and issues the cold decide POST.

@onready var room_label: Label = $HudViewport/HudControl/VBox/RoomLabel
@onready var room_entry: LineEdit = $HudViewport/HudControl/VBox/RoomEntry
@onready var join_button: Button = $HudViewport/HudControl/VBox/JoinButton
@onready var mute_toggle: CheckButton = $HudViewport/HudControl/VBox/MuteToggle
@onready var debug_stats: Label = $HudViewport/HudControl/VBox/DebugStats

@onready var acsp_label: Label = $HudViewport/HudControl/AcspIndicator/AcspLabel
@onready var acsp_glow: ColorRect = $HudViewport/HudControl/AcspIndicator/AcspGlow
@onready var intervention_panel: PanelContainer = $HudViewport/HudControl/InterventionPanel
@onready var case_title: Label = $HudViewport/HudControl/InterventionPanel/IVBox/CaseTitle
@onready var case_summary: Label = $HudViewport/HudControl/InterventionPanel/IVBox/CaseSummary
@onready var approve_button: Button = $HudViewport/HudControl/InterventionPanel/IVBox/Buttons/ApproveButton
@onready var deny_button: Button = $HudViewport/HudControl/InterventionPanel/IVBox/Buttons/DenyButton
@onready var dwell_reticle: Control = $HudViewport/HudControl/DwellReticle
@onready var decide_http: HTTPRequest = $DecideHttp

# Operator control-panel buttons + live status line.
@onready var reset_layout_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/ResetLayoutButton
@onready var spread_plus_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/SpreadPlusButton
@onready var spread_minus_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/SpreadMinusButton
@onready var edges_plus_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/EdgesPlusButton
@onready var edges_minus_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/EdgesMinusButton
@onready var node_size_plus_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/NodeSizePlusButton
@onready var node_size_minus_button: Button = $HudViewport/HudControl/VBox/ControlsGrid/NodeSizeMinusButton
@onready var controls_status: Label = $HudViewport/HudControl/VBox/ControlsStatus

# Double-click document view (narrativegoldmine page card).
@onready var document_panel: PanelContainer = $HudViewport/HudControl/DocumentPanel
@onready var doc_title_label: Label = $HudViewport/HudControl/DocumentPanel/DVBox/DHeader/DocTitle
@onready var close_doc_button: Button = $HudViewport/HudControl/DocumentPanel/DVBox/DHeader/CloseDocButton
@onready var doc_scroll: ScrollContainer = $HudViewport/HudControl/DocumentPanel/DVBox/DocScroll
@onready var doc_text: RichTextLabel = $HudViewport/HudControl/DocumentPanel/DVBox/DocScroll/DocText
@onready var scroll_up_button: Button = $HudViewport/HudControl/DocumentPanel/DVBox/DScroll/ScrollUpButton
@onready var scroll_down_button: Button = $HudViewport/HudControl/DocumentPanel/DVBox/DScroll/ScrollDownButton
@onready var doc_http: HTTPRequest = $DocHttp

const NG_PAGE_BASE: String = "https://narrativegoldmine.com/api/pages/"
const DOC_TIMEOUT_SEC: float = 10.0
const DOC_SCROLL_STEP: int = 140
var _doc_title: String = ""

signal join_requested(room_urn: String)
signal mute_toggled(muted: bool)
## Emitted the instant the operator approves/denies — the M2 intervention intent,
## independent of the HTTP round-trip.
signal decision_submitted(case_id: String, outcome: String)
## Emitted when the decide POST resolves; `accepted` is the server verdict.
signal case_decided(case_id: String, outcome: String, accepted: bool)
## Emitted when an operator control-panel button is pressed. GraphScene owns the
## effect (physics POST/PUT, edge/node runtime factors); the HUD only reports the
## intent so all state lives in one place. `action` is one of: reset_layout,
## spread_plus, spread_minus, edges_plus, edges_minus, node_size_plus,
## node_size_minus.
signal control_pressed(action: String)

const ACSP_GLOW_COLOR: Color = Color(1.0, 0.62, 0.12, 1.0)
# Hard cap on a single decide POST. A stalled request fires request_completed with
# RESULT_TIMEOUT at this point, releasing the single-in-flight decision gate.
const DECIDE_TIMEOUT_SEC: float = 10.0

var _avatar_count: int = 0
var _mtp_ms: float = 0.0
var _connected: bool = false

# M2 intervention state.
var _http_base: String = ""
var _nostr_auth: RefCounted = null  # Rust NostrAuth
var _current_case_id: String = ""
var _last_outcome: String = ""
# Case id captured at POST time. show_case() can swap _current_case_id between the
# request and its response; the completion handler must attribute the verdict to
# the case that was actually submitted, not whatever is on screen now.
var _pending_case_id: String = ""
var _open_case_count: int = 0
var _pulse_time: float = 0.0


func _ready() -> void:
	join_button.pressed.connect(_on_join_pressed)
	mute_toggle.toggled.connect(_on_mute_toggled)
	approve_button.pressed.connect(_on_approve_pressed)
	deny_button.pressed.connect(_on_deny_pressed)
	if decide_http != null:
		decide_http.request_completed.connect(_on_decide_completed)
		# Bound every decide POST: without a finite timeout a stalled connection
		# never fires request_completed, so _pending_case_id would block the
		# decision gate forever. On timeout Godot still fires request_completed
		# with result == RESULT_TIMEOUT, which clears the gate below.
		decide_http.timeout = DECIDE_TIMEOUT_SEC
	# Operator controls: each button just re-broadcasts a named intent; GraphScene
	# owns the effect. Bound with the action name so there's no per-button handler.
	reset_layout_button.pressed.connect(func() -> void: emit_signal("control_pressed", "reset_layout"))
	spread_plus_button.pressed.connect(func() -> void: emit_signal("control_pressed", "spread_plus"))
	spread_minus_button.pressed.connect(func() -> void: emit_signal("control_pressed", "spread_minus"))
	edges_plus_button.pressed.connect(func() -> void: emit_signal("control_pressed", "edges_plus"))
	edges_minus_button.pressed.connect(func() -> void: emit_signal("control_pressed", "edges_minus"))
	node_size_plus_button.pressed.connect(func() -> void: emit_signal("control_pressed", "node_size_plus"))
	node_size_minus_button.pressed.connect(func() -> void: emit_signal("control_pressed", "node_size_minus"))
	# Document view (double-click node → narrativegoldmine page card).
	close_doc_button.pressed.connect(hide_document)
	scroll_up_button.pressed.connect(func() -> void: doc_scroll.scroll_vertical -= DOC_SCROLL_STEP)
	scroll_down_button.pressed.connect(func() -> void: doc_scroll.scroll_vertical += DOC_SCROLL_STEP)
	if doc_http != null:
		doc_http.request_completed.connect(_on_doc_completed)
		doc_http.timeout = DOC_TIMEOUT_SEC
	set_process(true)


## Set by GraphScene after each control press: the current physics params, edge
## count shown, and node-size factor. Called on press only — no per-frame cost.
func set_controls_status(text: String) -> void:
	if controls_status != null:
		controls_status.text = text


# --- Document view (double-click node → narrativegoldmine page card) ----------

## Open the document view for `slug`, fetching its narrativegoldmine page JSON.
## `title` is the node label shown while loading. Wand-clickable Close/scroll.
func show_document(title: String, slug: String) -> void:
	_doc_title = title
	if document_panel != null:
		document_panel.visible = true
	if doc_title_label != null:
		doc_title_label.text = title
	if doc_text != null:
		doc_text.text = "[i]Loading…[/i]"
	if doc_scroll != null:
		doc_scroll.scroll_vertical = 0
	if doc_http == null:
		return
	# One request at a time; a new open cancels the previous fetch.
	doc_http.cancel_request()
	var url := "%s%s.json" % [NG_PAGE_BASE, slug.uri_encode()]
	var err := doc_http.request(url)
	if err != OK:
		if doc_text != null:
			doc_text.text = "[i]Could not reach the page service.[/i]"


func hide_document() -> void:
	if document_panel != null:
		document_panel.visible = false


func _on_doc_completed(
	result: int,
	response_code: int,
	_headers: PackedStringArray,
	body: PackedByteArray
) -> void:
	if doc_text == null:
		return
	var ok := result == HTTPRequest.RESULT_SUCCESS and response_code >= 200 and response_code < 300
	if not ok:
		doc_text.text = "[i]No linked page.[/i]"
		return
	var parsed: Variant = JSON.parse_string(body.get_string_from_utf8())
	if typeof(parsed) != TYPE_DICTIONARY:
		doc_text.text = "[i]No linked page.[/i]"
		return
	doc_text.text = render_ng_card(_doc_title, parsed)


## Pure builder: narrativegoldmine page Dictionary → BBCode card. Total — never
## errors on missing/oddly-typed fields; unknown sections are skipped.
func render_ng_card(fallback_title: String, data: Dictionary) -> String:
	var lines: Array = []
	var title: String = _dict_str(data, "title", fallback_title)
	lines.append("[font_size=34][b]%s[/b][/font_size]" % _bb(title))
	# Subtitle: domain · entityType · maturity · quality score (present fields only).
	var sub: Array = []
	var domain := _dict_str(data, "domain", "")
	if domain != "":
		sub.append(domain)
	var entity := _dict_str(data, "entityType", "")
	if entity != "":
		sub.append(entity)
	var maturity := _dict_str(data, "maturity", "")
	if maturity != "":
		sub.append(maturity)
	if data.has("qualityScore") and (data["qualityScore"] is float or data["qualityScore"] is int):
		sub.append("quality %.2f" % float(data["qualityScore"]))
	if not sub.is_empty():
		lines.append("[color=#8fa6c8]%s[/color]" % _bb(" · ".join(sub)))
	# Definition.
	var definition := _dict_str(data, "definition", "")
	if definition != "":
		lines.append("")
		lines.append(_bb(definition))
	# Relationships (defensive shape: dicts predicate→target, or plain strings).
	var rels: Array = _rel_lines(data.get("relationships", []))
	if not rels.is_empty():
		lines.append("")
		lines.append("[b]Relationships[/b]")
		for r: String in rels:
			lines.append("  • %s" % _bb(r))
	# Links: wikilinks + backlinks, capped.
	var wl: Array = _link_chips(data.get("wikilinks", []), 10)
	var bl: Array = _link_chips(data.get("backlinks", []), 10)
	if not wl.is_empty() or not bl.is_empty():
		lines.append("")
		lines.append("[b]Links[/b]")
		if not wl.is_empty():
			lines.append("[color=#7fd0ff]→[/color] " + " ".join(wl))
		if not bl.is_empty():
			lines.append("[color=#c8a0ff]←[/color] " + " ".join(bl))
	return "\n".join(lines)


# Coloured, escaped chips from a link array (strings or {title/label/slug} dicts),
# capped at `cap` with a "+N more" tail.
func _link_chips(arr: Variant, cap: int) -> Array:
	var out: Array = []
	if not (arr is Array):
		return out
	var items: Array = arr
	var n: int = items.size()
	var shown: int = mini(n, cap)
	for i: int in range(shown):
		var name := _any_name(items[i])
		if name != "":
			out.append("[color=#cfe3ff]%s[/color]" % _bb(name))
	if n > cap:
		out.append("[i]+%d more[/i]" % (n - cap))
	return out


# Relationship lines: "predicate → target" from dicts, or the raw string.
func _rel_lines(arr: Variant) -> Array:
	var out: Array = []
	if not (arr is Array):
		return out
	var items: Array = arr
	for i: int in range(mini(items.size(), 15)):
		var it: Variant = items[i]
		if it is String:
			if it != "":
				out.append(it)
		elif it is Dictionary:
			var pred := _dict_str(it, "predicate", _dict_str(it, "relation", _dict_str(it, "type", "")))
			var tgt := _dict_str(it, "target", _dict_str(it, "object", _dict_str(it, "value", "")))
			if pred != "" and tgt != "":
				out.append("%s → %s" % [pred, tgt])
			elif tgt != "":
				out.append(tgt)
			elif pred != "":
				out.append(pred)
	return out


# Best-effort display name from a string or a dict with title/label/slug.
func _any_name(v: Variant) -> String:
	if v is String:
		return v
	if v is Dictionary:
		return _dict_str(v, "title", _dict_str(v, "label", _dict_str(v, "slug", "")))
	return ""


func _dict_str(d: Dictionary, key: String, fallback: String) -> String:
	if d.has(key) and d[key] is String:
		return d[key]
	return fallback


# Escape BBCode-significant '[' so page content can't inject tags.
func _bb(s: String) -> String:
	return s.replace("[", "[lb]")


func _process(delta: float) -> void:
	var conn_str: String = "OK" if _connected else "OFF"
	debug_stats.text = "FPS: %d  MTP: %.1fms  Avatars: %d  Net: %s" % [
		Engine.get_frames_per_second(),
		_mtp_ms,
		_avatar_count,
		conn_str,
	]
	# Ambient ACSP glow: pulse amber while cases are open, transparent when clear.
	if acsp_glow != null:
		if _open_case_count > 0:
			_pulse_time += delta
			var a: float = 0.2 + 0.25 * (0.5 + 0.5 * sin(_pulse_time * 3.0))
			acsp_glow.color = Color(ACSP_GLOW_COLOR.r, ACSP_GLOW_COLOR.g, ACSP_GLOW_COLOR.b, a)
		else:
			acsp_glow.color = Color(ACSP_GLOW_COLOR.r, ACSP_GLOW_COLOR.g, ACSP_GLOW_COLOR.b, 0.0)


func set_avatar_count(count: int) -> void:
	_avatar_count = count


func set_mtp_ms(ms: float) -> void:
	_mtp_ms = ms


func _on_connection_status(connected: bool) -> void:
	_connected = connected


func _on_join_pressed() -> void:
	var urn := room_entry.text.strip_edges()
	if urn.is_empty():
		push_warning("Empty room URN")
		return
	emit_signal("join_requested", urn)
	room_label.text = "Room: %s" % urn


func _on_mute_toggled(state: bool) -> void:
	emit_signal("mute_toggled", state)


# --- M2 intervention ---------------------------------------------------------

## Wire the decide path. `http_base` is the visionclaw-server origin (scheme +
## host + port, no trailing slash); `nostr_auth` is the Rust NostrAuth that mints
## the NIP-98 header so the POST authenticates against the same power-user gate
## the desktop control centre uses.
func configure_intervention(http_base: String, nostr_auth: RefCounted) -> void:
	_http_base = http_base.rstrip("/")
	_nostr_auth = nostr_auth


## Show the intervention panel for a broker case awaiting the operator's
## approval (driven by a broker:new_case event on the selected agent).
func show_case(case_id: String, summary: String) -> void:
	_current_case_id = case_id
	if case_title != null:
		case_title.text = "Awaiting approval — %s" % case_id
	if case_summary != null:
		case_summary.text = summary
	if intervention_panel != null:
		intervention_panel.visible = true


func clear_case() -> void:
	_current_case_id = ""
	if intervention_panel != null:
		intervention_panel.visible = false


## Ambient ACSP indicator: the count of open broker cases (from broker:new_case /
## broker:case_decided events).
func set_case_count(count: int) -> void:
	_open_case_count = maxi(count, 0)
	if acsp_label != null:
		acsp_label.text = "ACSP: %d open" % _open_case_count


## Drive the gaze-dwell charging reticle (Rust SelectionArbiterNode.charge_ratio).
func set_dwell_charge(ratio: float) -> void:
	if dwell_reticle != null and dwell_reticle.has_method("set_charge"):
		dwell_reticle.set_charge(ratio)


func _on_approve_pressed() -> void:
	_submit_decision("approve")


func _on_deny_pressed() -> void:
	_submit_decision("reject")


func approve_selected_case() -> void:
	_submit_decision("approve")


func deny_selected_case() -> void:
	_submit_decision("reject")


# POST the decision to the shared broker decide core through the same route the
# desktop operator uses (enrichment_proposals_handler::decide_as_operator,
# power-user-gated). Signing happens Rust-side; the header carries a single-use
# NIP-98 event. The decision intent is emitted immediately so the loop is
# observable even before the round-trip resolves.
func _submit_decision(outcome: String) -> void:
	if _current_case_id.is_empty():
		push_warning("HUD: no case selected for decision")
		return
	# Reject overlapping submissions: the single DecideHttp is one-in-flight, so a
	# second dispatch would return ERR_BUSY *after* we'd already overwritten the
	# pending case id — the first (still live) request would then complete under
	# the second case's identity while the second never went out. Only one decide
	# may be in flight; a new one is ignored until the current resolves.
	if not _pending_case_id.is_empty():
		push_warning("HUD: a decision is already in flight; ignoring new submission")
		return
	var case_id := _current_case_id
	# Intent is observable immediately, independent of the HTTP round-trip.
	emit_signal("decision_submitted", case_id, outcome)

	if _nostr_auth == null or _http_base.is_empty() or decide_http == null:
		push_warning("HUD: intervention not configured; decision not dispatched")
		return

	var url := "%s/api/broker/cases/%s/decide" % [_http_base, case_id]
	var auth: String = _nostr_auth.nip98_header(url, "POST")
	var pubkey: String = _nostr_auth.pubkey_hex()
	var body := {
		"outcome": outcome,
		"broker_pubkey": pubkey,
		"reasoning": "in-headset operator decision",
	}
	var headers := PackedStringArray([
		"Authorization: %s" % auth,
		"Content-Type: application/json",
	])
	var err := decide_http.request(url, headers, HTTPClient.METHOD_POST, JSON.stringify(body))
	if err != OK:
		push_warning("HUD: decide request failed to start (%d)" % err)
		return
	# Record the in-flight attribution ONLY once the request has actually
	# launched, so a failed/busy dispatch never leaves stale pending state that a
	# later completion would mis-attribute.
	_last_outcome = outcome
	_pending_case_id = case_id


func _on_decide_completed(
	result: int,
	response_code: int,
	_headers: PackedStringArray,
	_body: PackedByteArray
) -> void:
	# A verdict counts only when the transport itself succeeded (result ==
	# RESULT_SUCCESS) AND the server returned 2xx. A timeout / connection failure
	# reports a non-SUCCESS result with response_code 0 — treat as not accepted,
	# but still release the gate so the operator can retry.
	var transport_ok := result == HTTPRequest.RESULT_SUCCESS
	var accepted := transport_ok and response_code >= 200 and response_code < 300
	# Attribute the verdict to the in-flight case, not the (possibly swapped)
	# on-screen case. Clearing _pending_case_id here (on ANY completion, including
	# timeout/failure) is what reopens the decision gate.
	var decided_case := _pending_case_id
	_pending_case_id = ""
	if not accepted:
		push_warning("HUD: decide failed for %s (result=%d code=%d)" % [
			decided_case, result, response_code])
	emit_signal("case_decided", decided_case, _last_outcome, accepted)
	# Only dismiss the panel if it is still showing the case we just decided;
	# a newer case shown mid-flight must stay visible.
	if accepted and _current_case_id == decided_case:
		clear_case()
