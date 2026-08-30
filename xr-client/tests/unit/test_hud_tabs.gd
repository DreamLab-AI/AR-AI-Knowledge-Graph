extends "res://addons/gut/test.gd"

# HUD control-centre redesign (task #20): tab switching, hover-hint resolution,
# per-tab fit (no below-the-fold overflow), and preserved public API/signals.
# No godot binary in this container → these run in CI / a live session.

const TABS := "HudViewport/HudControl/Root/Tabs"
const TABBAR := "HudViewport/HudControl/Root/TabBar"


func _make_hud() -> Node3D:
	var packed: PackedScene = load("res://scenes/HUD.tscn")
	var hud: Node3D = packed.instantiate()
	add_child(hud)
	# Two frames so the SubViewport lays out the built Control tree.
	await get_tree().process_frame
	await get_tree().process_frame
	return hud


func test_all_pages_built_and_default_graph_visible() -> void:
	var hud: Node3D = await _make_hud()
	for page_name in ["GraphPage", "QueryPage", "PinsPage", "SessionPage", "HelpPage"]:
		assert_not_null(hud.get_node_or_null("%s/%s" % [TABS, page_name]), "%s exists" % page_name)
	var graph: Control = hud.get_node("%s/GraphPage" % TABS)
	assert_true(graph.visible, "Graph is the default visible tab")
	var session: Control = hud.get_node("%s/SessionPage" % TABS)
	assert_false(session.visible, "other tabs start hidden")
	hud.queue_free()
	await get_tree().process_frame


func test_show_tab_switches_exactly_one_page() -> void:
	var hud: Node3D = await _make_hud()
	hud._show_tab("session")
	assert_true((hud.get_node("%s/SessionPage" % TABS) as Control).visible, "session shown")
	assert_false((hud.get_node("%s/GraphPage" % TABS) as Control).visible, "graph hidden")
	var visible_count := 0
	for page_name in ["GraphPage", "QueryPage", "PinsPage", "SessionPage", "HelpPage"]:
		if (hud.get_node("%s/%s" % [TABS, page_name]) as Control).visible:
			visible_count += 1
	assert_eq(visible_count, 1, "exactly one page visible after a tab switch")
	hud.queue_free()
	await get_tree().process_frame


func test_every_button_and_tab_has_a_hint() -> void:
	var hud: Node3D = await _make_hud()
	var missing: Array[String] = []
	# Tabs + tab bar AND the overlay panels (Document / Intervention) — every
	# interactive control anywhere in the panel must carry a hint.
	for path in [TABS, TABBAR,
			"HudViewport/HudControl/DocumentPanel",
			"HudViewport/HudControl/InterventionPanel"]:
		var root: Node = hud.get_node_or_null(path)
		if root != null:
			_collect_hintless(root, missing)
	assert_eq(missing.size(), 0, "every interactive control carries a 'hint' meta; missing: %s" % str(missing))
	hud.queue_free()
	await get_tree().process_frame


# Recurse the WHOLE subtree (recursion is outside the interactive-check, so it
# descends plain containers too), flagging any Button/CheckButton/LineEdit that
# lacks a "hint" meta.
func _collect_hintless(root: Node, out: Array) -> void:
	for n in root.get_children():
		if (n is Button or n is CheckButton or n is LineEdit) and not n.has_meta("hint"):
			out.append(String(n.name))
		_collect_hintless(n, out)


func test_hint_resolution_defaults_when_nothing_hovered() -> void:
	var hud: Node3D = await _make_hud()
	# No synthetic pointer pushed → nothing hovered → default hint.
	assert_eq(hud._resolve_hint(), "Point at a control for help", "falls back to the default hint")
	hud.queue_free()
	await get_tree().process_frame


func test_no_page_overflows_its_host() -> void:
	var hud: Node3D = await _make_hud()
	var host: Control = hud.get_node(TABS)
	assert_gt(host.size.y, 1.0, "tab host has a real height")
	for page_name in ["GraphPage", "QueryPage", "PinsPage", "SessionPage", "HelpPage"]:
		var page: Control = hud.get_node("%s/%s" % [TABS, page_name])
		var need: float = page.get_combined_minimum_size().y
		assert_lte(need, host.size.y + 1.0,
			"%s min-height %.0f must fit host %.0f (no below-the-fold)" % [page_name, need, host.size.y])
	hud.queue_free()
	await get_tree().process_frame


func test_public_api_and_signals_preserved() -> void:
	var hud: Node3D = await _make_hud()
	for m in ["set_controls_status", "set_control_states", "set_fold_state", "set_query_preview",
			"hide_query_preview", "show_document", "hide_document", "show_case", "clear_case",
			"set_case_count", "set_dwell_charge", "set_avatar_count", "set_mtp_ms",
			"configure_intervention", "approve_selected_case", "deny_selected_case", "_on_connection_status"]:
		assert_true(hud.has_method(m), "public method preserved: %s" % m)
	for s in ["join_requested", "mute_toggled", "decision_submitted", "case_decided",
			"control_pressed", "query_execute_pressed", "query_clear_pressed"]:
		assert_true(hud.has_signal(s), "signal preserved: %s" % s)
	hud.queue_free()
	await get_tree().process_frame


func test_control_button_emits_named_action() -> void:
	var hud: Node3D = await _make_hud()
	watch_signals(hud)
	# The Reset Layout button lives on the Graph page; press it programmatically.
	var reset := _find_button(hud.get_node("%s/GraphPage" % TABS), "Reset Layout")
	assert_not_null(reset, "Reset Layout button exists on the Graph page")
	if reset != null:
		reset.pressed.emit()
		assert_signal_emitted_with_parameters(hud, "control_pressed", ["reset_layout"])
	hud.queue_free()
	await get_tree().process_frame


func _find_button(root: Node, text: String) -> Button:
	for n in root.get_children():
		if n is Button and (n as Button).text == text:
			return n
		var found := _find_button(n, text)
		if found != null:
			return found
	return null


# Wave 2, Feature 3 — type show/hide toggles live on the Graph page and, when
# pressed, flip their visible state and emit control_pressed
# "type_toggle:<class>:<1|0>".
func test_type_toggles_present_and_emit_on_press() -> void:
	var hud: Node3D = await _make_hud()
	var graph: Control = hud.get_node("%s/GraphPage" % TABS)
	# Starts visible → label shows "✓".
	var knowledge: Button = _find_button(graph, "Knowledge ✓")
	assert_not_null(knowledge, "Knowledge type toggle present, visible by default")
	assert_not_null(_find_button(graph, "Ontology ✓"), "Ontology toggle present")
	assert_not_null(_find_button(graph, "Agents ✓"), "Agents toggle present")
	# Pressing emits control_pressed with the now-hidden (0) state and relabels.
	watch_signals(hud)
	knowledge.pressed.emit()
	assert_signal_emitted_with_parameters(hud, "control_pressed", ["type_toggle:knowledge:0"])
	assert_eq(knowledge.text, "Knowledge ✕", "label flips to hidden marker")
	# Pressing again re-shows it.
	knowledge.pressed.emit()
	assert_signal_emitted_with_parameters(hud, "control_pressed", ["type_toggle:knowledge:1"])
	hud.queue_free()
	await get_tree().process_frame
