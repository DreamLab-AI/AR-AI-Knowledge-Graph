shader_type spatial;
// XR-safe fake-bloom halo (no post-process / WorldEnvironment glow — this runs on
// Godot 4.6 gl_compatibility + OpenXR multiview). It is a second pass over the node
// mesh: the shell is pushed out along the normal, front faces are culled so we see
// its far side around the silhouette, and a fresnel falloff makes a soft additive
// rim that reads as a glow. Additive + depth_draw_never so it never occludes.
render_mode blend_add, unshaded, cull_front, depth_draw_never, shadows_disabled, fog_disabled;

// Shell expansion in local mesh units (node sphere radius is 0.5). Scales with the
// per-instance transform, so the halo stays proportional to node size.
uniform float halo_width = 0.16;
// Fresnel exponent — higher = tighter rim.
uniform float fresnel_power = 2.5;
// Additive brightness multiplier for the halo tint.
uniform float halo_energy = 1.35;

// How much node centrality (INSTANCE_CUSTOM.r, 0..1) lifts halo intensity. The
// halo brightness is multiplied by mix(1-cen_boost, 1+cen_boost, centrality), so
// central nodes glow a little harder without low-centrality nodes going dark.
uniform float cen_boost = 0.4;

// Per-instance colour arrives via MultiMesh use_colors (set_instance_color in
// graph_scene.gd) as vertex COLOR — the halo tint follows the node's community
// colour exactly like the base pass. Normalised centrality rides INSTANCE_CUSTOM.r
// (set_instance_custom_data) to modulate glow strength.
varying vec3 halo_tint;
varying float halo_cen;

void vertex() {
	halo_tint = COLOR.rgb;
	halo_cen = INSTANCE_CUSTOM.r;
	VERTEX += normalize(NORMAL) * halo_width;
}

void fragment() {
	// VIEW points from the fragment toward the camera (view space); NORMAL is the
	// view-space geometric normal. Rim-lit fresnel: bright at grazing angles.
	float ndotv = clamp(dot(normalize(NORMAL), normalize(VIEW)), 0.0, 1.0);
	float fres = pow(1.0 - ndotv, fresnel_power);
	float cen_mul = mix(1.0 - cen_boost, 1.0 + cen_boost, clamp(halo_cen, 0.0, 1.0));
	// Unshaded + additive: ALBEDO is the emitted colour, ALPHA (with SRC_ALPHA,ONE)
	// scales how much is added, so a thin bright rim fades to nothing face-on.
	ALBEDO = halo_tint * halo_energy * cen_mul;
	ALPHA = fres * cen_mul;
}
