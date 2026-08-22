extends Node3D

# Minimal XR bring-up probe: initialise OpenXR, enable the viewport for XR, and
# render three basic-material axis bars in front of the user. No graph, no HUD,
# no network — used to confirm the headset presents both eyes with the simplest
# possible scene before layering complexity back in.

func _ready() -> void:
	var xr: XRInterface = XRServer.find_interface("OpenXR")
	if xr == null:
		push_error("xr_min: OpenXR interface not found")
		return
	if not xr.is_initialized() and not xr.initialize():
		push_error("xr_min: OpenXR initialize() failed")
		return
	get_viewport().use_xr = true
	print("xr_min: OpenXR initialised, use_xr set, rendering axes")
