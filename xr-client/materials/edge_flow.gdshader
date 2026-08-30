shader_type spatial;
// XR-safe flowing edge. Unshaded additive cyan gradient with a pulse of light that
// travels along the cylinder's length (UV.y, 0 at one end to 1 at the other),
// driven purely by TIME so there are zero per-frame GDScript/material touches.
// Faint base alpha keeps dense edge bundles from blowing out to white. No
// post-process / screen-space anything — safe on gl_compatibility + multiview.
// Alpha (mix) blending: overlapping edges accumulate towards the edge colour
// instead of summing to white the way additive did in dense bundles.
render_mode blend_mix, unshaded, cull_disabled, depth_draw_never, shadows_disabled, fog_disabled;

uniform vec3 flow_color : source_color = vec3(0.30, 0.72, 1.0);
// Baseline alpha so the whole edge network is faintly visible.
uniform float base_alpha = 0.35;
// Pulse travel speed (cycles per second along the edge).
uniform float pulse_speed = 0.6;
// Half-width of the travelling pulse in UV space.
uniform float pulse_width = 0.22;
// Extra alpha at the pulse peak.
uniform float pulse_energy = 0.4;
// Brightness boost of the moving highlight colour (kept gentle so the pulse
// head reads as brightened cyan, not white).
uniform float pulse_gain = 1.2;

void fragment() {
	// UV.y runs along the unit cylinder's height; the per-instance transform
	// stretches the edge but leaves UV in [0,1], so the pulse always sweeps the
	// full length uniformly regardless of edge length.
	float t = UV.y;
	float head = fract(TIME * pulse_speed);
	float d = abs(t - head);
	d = min(d, 1.0 - d); // wrap so the pulse loops seamlessly end-to-end
	float pulse = smoothstep(pulse_width, 0.0, d);
	ALBEDO = flow_color * (1.0 + pulse * (pulse_gain - 1.0));
	ALPHA = clamp(base_alpha + pulse * pulse_energy, 0.0, 1.0);
}
