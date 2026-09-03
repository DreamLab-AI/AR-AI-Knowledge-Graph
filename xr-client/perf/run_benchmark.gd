extends SceneTree

# Headless entry point. Invoked as:
#   godot --headless --path xr-client --script perf/run_benchmark.gd
# Loads the benchmark scene, lets it run for 30s real time, then quits with the
# scene's pass/fail exit code (0 pass, 1 fail). The scene itself prints the
# "[XR_PERF_RESULT]={...}" JSON line that the xr-perf-regression bin consumes
# (xr-client/rust/perf-regression).

const SCENE_PATH := "res://perf/benchmark_scene.tscn"

func _initialize() -> void:
	var packed: PackedScene = load(SCENE_PATH)
	if packed == null:
		push_error("benchmark scene missing at %s" % SCENE_PATH)
		quit(2)
		return
	var inst := packed.instantiate()
	get_root().add_child(inst)
