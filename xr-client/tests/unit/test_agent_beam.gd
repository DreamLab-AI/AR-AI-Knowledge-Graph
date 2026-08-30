extends "res://addons/gut/test.gd"

# Work-beam layer wiring (ADR-140, Pillar 2 / P3). The reserved AgentMulti MultiMesh
# is repurposed as the beam layer: one cylinder per active agent→target-node link,
# packed by the Rust render store (build_beam_buffer) and pushed as a single buffer
# each frame. These tests verify the scene-side surface — the MultiMesh, its mesh,
# custom-data format, and material — is present and correctly typed, so the Rust
# buffer (unit-tested on its own) lands somewhere that will render it. The beam
# content is driven by the live 0x23 agent-action wire feeding the registry.


func _make_scene() -> Node3D:
	var packed: PackedScene = load("res://scenes/GraphScene.tscn")
	var scene: Node3D = packed.instantiate()
	add_child(scene)
	await get_tree().process_frame
	return scene


func test_agent_multi_is_wired_as_the_beam_layer():
	var scene: Node3D = await _make_scene()
	var beam: MultiMeshInstance3D = scene.get_node("GraphRoot/AgentMulti")
	assert_not_null(beam, "AgentMulti node exists")
	assert_not_null(beam.multimesh, "AgentMulti has a MultiMesh (the beam buffer target)")
	assert_true(beam.multimesh.use_custom_data, "beam MultiMesh carries INSTANCE_CUSTOM (status in .a)")
	assert_not_null(beam.multimesh.mesh, "beam MultiMesh has a cylinder mesh to instance")
	assert_not_null(beam.material_override, "beam layer has the agent_beam material override")
	scene.queue_free()
	await get_tree().process_frame


func test_beam_multimesh_starts_empty_and_survives_a_frame():
	# With no agent-action wire in a headless test, the registry is empty, so the
	# beam buffer is empty and the per-frame update must be a safe no-op (instance
	# count stays 0, no error).
	var scene: Node3D = await _make_scene()
	var beam: MultiMeshInstance3D = scene.get_node("GraphRoot/AgentMulti")
	await get_tree().process_frame
	await get_tree().process_frame
	assert_eq(beam.multimesh.instance_count, 0, "no active agents ⇒ no beams drawn")
	scene.queue_free()
	await get_tree().process_frame
