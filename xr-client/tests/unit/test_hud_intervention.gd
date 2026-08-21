extends "res://addons/gut/test.gd"

# M2 in-headset intervention affordance tests (PRD-023 WP-9 / COM-18). Exercises
# the case panel, the ambient ACSP indicator, the approve/deny decision intent,
# and the gaze-dwell reticle. The signed decide POST itself is Rust-signed and
# NIP-98-authenticated; its live server acceptance + CANARY-VC-COM18-INTERV fire
# are pending-live-session (no server + no godot binary in this container).


func _make_hud() -> Node3D:
	var packed: PackedScene = load("res://scenes/HUD.tscn")
	var hud: Node3D = packed.instantiate()
	add_child(hud)
	await get_tree().process_frame
	return hud


func test_intervention_panel_hidden_by_default():
	var hud: Node3D = await _make_hud()
	var panel: PanelContainer = hud.get_node("HudViewport/HudControl/InterventionPanel")
	assert_false(panel.visible, "no case → no intervention panel")
	hud.queue_free()
	await get_tree().process_frame


func test_show_case_reveals_panel_with_case_id():
	var hud: Node3D = await _make_hud()
	hud.show_case("case-42", "Merge ontology enrichment for Foo")
	var panel: PanelContainer = hud.get_node("HudViewport/HudControl/InterventionPanel")
	var title: Label = hud.get_node("HudViewport/HudControl/InterventionPanel/IVBox/CaseTitle")
	var summary: Label = hud.get_node("HudViewport/HudControl/InterventionPanel/IVBox/CaseSummary")
	assert_true(panel.visible, "a queued case reveals the panel")
	assert_true(title.text.contains("case-42"), "panel names the case")
	assert_true(summary.text.contains("Foo"), "panel shows the case summary")
	hud.queue_free()
	await get_tree().process_frame


func test_clear_case_hides_panel():
	var hud: Node3D = await _make_hud()
	hud.show_case("case-1", "x")
	hud.clear_case()
	var panel: PanelContainer = hud.get_node("HudViewport/HudControl/InterventionPanel")
	assert_false(panel.visible, "clearing a case hides the panel")
	hud.queue_free()
	await get_tree().process_frame


func test_acsp_indicator_tracks_open_case_count():
	var hud: Node3D = await _make_hud()
	hud.set_case_count(3)
	var label: Label = hud.get_node("HudViewport/HudControl/AcspIndicator/AcspLabel")
	assert_true(label.text.contains("3"), "ambient ACSP indicator shows the open-case count")
	hud.set_case_count(0)
	assert_true(label.text.contains("0"), "count clears back to zero")
	hud.queue_free()
	await get_tree().process_frame


func test_approve_emits_decision_intent():
	var hud: Node3D = await _make_hud()
	hud.show_case("case-7", "approve me")
	watch_signals(hud)
	hud.approve_selected_case()
	assert_signal_emitted_with_parameters(hud, "decision_submitted", ["case-7", "approve"])
	hud.queue_free()
	await get_tree().process_frame


func test_deny_emits_reject_intent():
	var hud: Node3D = await _make_hud()
	hud.show_case("case-8", "deny me")
	watch_signals(hud)
	hud.deny_selected_case()
	assert_signal_emitted_with_parameters(hud, "decision_submitted", ["case-8", "reject"])
	hud.queue_free()
	await get_tree().process_frame


func test_decision_without_case_is_a_noop():
	var hud: Node3D = await _make_hud()
	watch_signals(hud)
	hud.approve_selected_case()  # no case selected
	assert_signal_not_emitted(hud, "decision_submitted", "no case → no decision")
	hud.queue_free()
	await get_tree().process_frame


func test_dwell_reticle_reflects_charge():
	var hud: Node3D = await _make_hud()
	var reticle: Control = hud.get_node("HudViewport/HudControl/DwellReticle")
	assert_false(reticle.visible, "reticle hidden at zero charge")
	hud.set_dwell_charge(0.5)
	assert_true(reticle.visible, "reticle shows while charging")
	assert_almost_eq(reticle._charge, 0.5, 0.001, "reticle mirrors the dwell charge ratio")
	hud.set_dwell_charge(0.0)
	assert_false(reticle.visible, "reticle hides when the charge clears")
	hud.queue_free()
	await get_tree().process_frame
