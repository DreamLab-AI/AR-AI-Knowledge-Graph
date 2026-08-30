shader_type spatial;
// XR-safe agent WORK BEAM (VisionClaw ADR-140, Pillar 2). A fast, bright, directional
// particle-like stream travelling the cylinder's length from the AGENT end to the
// TARGET node — the Rust render store orients each beam's local +Y onto agent→target,
// and the fragment derives a longitudinal L (0 at agent, 1 at target) from the side
// UVs (see fragment()) so the flow always reads as "this agent is acting on that
// node" regardless of Godot's cylinder UV convention. Driven purely by
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

	// Godot CylinderMesh UV: the SIDE surface runs UV.y = 0 at the +Y (top) rim to
	// UV.y = 0.5 at the −Y (bottom) rim; the two end caps occupy UV.y ∈ (0.5, 1.0].
	// edge_transform12 maps the mesh's local +Y onto the AGENT→TARGET direction, so
	// the +Y rim (UV.y = 0) sits at the TARGET and the −Y rim (UV.y = 0.5) at the
	// AGENT. Define a longitudinal coordinate L that is 0 at the agent and 1 at the
	// target: L = 1 − UV.y*2  ⇒  agent(UV.y=0.5)→0, target(UV.y=0)→1. A head at
	// parameter `head` (0→1 over time) then travels agent→target, the correct flow.
	// Only the side (UV.y ≤ 0.5) streams; caps render base tint so their squashed
	// UVs never inject spurious pulses.
	bool is_side = UV.y <= 0.5;
	float uL = clamp(1.0 - UV.y * 2.0, 0.0, 1.0);

	float seg = fract(uL * stream_heads);
	float head = fract(TIME * speed);
	float d = abs(seg - head);
	d = min(d, 1.0 - d); // wrap so heads loop seamlessly along the beam
	float pulse = is_side ? smoothstep(head_width, 0.0, d) : 0.0;

	vec3 tint = is_blocked ? blocked_color : working_color;
	ALBEDO = tint * (1.0 + pulse * (head_gain - 1.0));

	float alpha = base_alpha + pulse * head_energy;
	if (is_blocked) {
		alpha *= blocked_dim;
	}
	ALPHA = clamp(alpha, 0.0, 1.0);
}
