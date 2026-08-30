shader_type spatial;
// XR-safe agent WORK BEAM (VisionClaw ADR-140, Pillar 2). A fast, bright, directional
// particle-like stream travelling along the cylinder's length (UV.y, 0 at the AGENT
// end to 1 at the TARGET node) — the Rust render store orients each beam agent→target,
// so the flow always reads as "this agent is acting on that node". Driven purely by
// TIME: zero per-frame GDScript/material touches. Restyle of edge_flow.gdshader — same
// render_mode (safe on gl_compatibility + multiview), but INSTANCE_CUSTOM.a now carries
// the AGENT STATUS CODE (1 working / 2 blocked) rather than the edge relation grammar.
render_mode blend_add, unshaded, cull_disabled, depth_draw_never, shadows_disabled, fog_disabled;

// Working: a bright green-cyan energetic stream (the healthy "agent at work" look).
uniform vec3 working_color : source_color = vec3(0.30, 0.90, 0.72);
// Blocked: a stalled amber-red beam, slower and dimmer, so a stuck agent reads apart.
uniform vec3 blocked_color : source_color = vec3(1.0, 0.35, 0.20);
// Faint baseline so the beam's path is visible between pulse heads.
uniform float base_alpha = 0.12;
// Several pulse heads travel the beam at once for a particle-stream feel.
uniform float stream_heads = 4.0;
// Head travel speed (cycles/sec). Fast — this is the "bright fast stream" affordance.
uniform float stream_speed = 1.8;
// Half-width of each travelling head in per-head UV space.
uniform float head_width = 0.18;
// Extra alpha at a head's peak.
uniform float head_energy = 0.85;
// Brightness boost of a head (additive blend, so heads read as hot cores).
uniform float head_gain = 2.2;
// Blocked beams slow down and dim to signal a stall (multipliers on the above).
uniform float blocked_speed_scale = 0.35;
uniform float blocked_dim = 0.55;

varying flat float v_status;

void vertex() {
	// Latch the per-instance status code once; fragment reads the flat varying.
	v_status = INSTANCE_CUSTOM.a;
}

void fragment() {
	bool is_blocked = v_status > 1.5; // code 2 = blocked; 1 = working
	float speed = stream_speed * (is_blocked ? blocked_speed_scale : 1.0);

	// UV.y runs agent(0)→target(1). Tile it into `stream_heads` segments and run a
	// travelling pulse within each, so multiple hot cores flow toward the node.
	float seg = fract(UV.y * stream_heads);
	float head = fract(TIME * speed);
	float d = abs(seg - head);
	d = min(d, 1.0 - d); // wrap so heads loop seamlessly along the beam
	float pulse = smoothstep(head_width, 0.0, d);

	vec3 tint = is_blocked ? blocked_color : working_color;
	ALBEDO = tint * (1.0 + pulse * (head_gain - 1.0));

	float alpha = base_alpha + pulse * head_energy;
	if (is_blocked) {
		alpha *= blocked_dim;
	}
	ALPHA = clamp(alpha, 0.0, 1.0);
}
