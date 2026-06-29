# PRD-12 — XR Client (Godot + gdext)

Status   : Proposed
Date     : 2026-05-16
Owner    : anthropic@xrsystems.uk
Related  : ADR-12 (this section), ADR-02 (Binary Protocol — XR consumes the
           same V3 stream), ADR-04 / PRD-04 (Web rendering — visual primitive
           parity), ADR-06 (Auth — Nostr challenge), ADR-07 (Bots & Telemetry
           — XR agent panels), ADR-03 (Client State — model reuse intent).

## Capability statement

The XR client is a standalone Godot 4.x application that renders the same
VisionClaw knowledge graph as the web client, in room-scale stereoscopic 3D
on standalone HMDs (primary target: Meta Quest 3) and tethered OpenXR
devices. It connects to the existing VisionClaw server over WebSocket using
the V3 full-sync binary protocol (ADR-02) — *the same wire format the web
client uses, byte-for-byte* — authenticates via Nostr challenge-response
on connect (ADR-06), and renders nodes / edges / labels with visual
parity to the web client's three primitive classes (gem, crystal orb,
agent capsule). The user navigates with hand tracking (pinch-and-drag pan,
gaze-and-pinch select, pinch-on-node highlight) and snap-turn for comfort.

The XR client is **not** a port of the web client. It is a second consumer
of the same server, written from scratch in Godot + GDScript, with a Rust
gdext bridge (`xr-client/rust/`) handling the binary protocol decode and a
separate `crates/visionclaw-xr-presence` crate handling XR presence (head
pose, hand pose, gaze) routed to the server for future shared-session
features. The Godot project lives at `xr-client/` and is independently
buildable for Android (Quest) and Linux/Windows (tethered OpenXR).

## Why this matters

The web client is the authoritative interactive surface for desktop. It is
not adequate for VR/AR: WebXR on Quest browsers is unreliable for graphs
of 5k+ nodes, lacks hand-tracking fidelity for fine manipulation, and is
gated by browser update cycles. The native Godot client closes that gap
without forking the data path.

The critical design constraint is **no XR-specific server changes**. The
server is unchanged: same V3 protocol, same Nostr auth, same agent
telemetry contracts. The XR client is purely a new consumer. This means
all of ADR-02's settlement-gated cadence, drop-on-pressure backpressure,
and `frame_id` drop detection apply unchanged. The XR client experiences
the same SETTLED-state heartbeat (5s) as the web client, which is fine
because the graph stops moving — there is nothing to render new.

A second-class XR experience — laggy, low-FPS, motion-sick — is worse than
no XR at all. The performance ceiling (90Hz on Quest 3 with 5k nodes)
and the comfort policy (snap-turn, vignette on translation, IPD-aware
rig) are non-negotiable functional requirements, not aspirations.

## Functional requirements

### F1. WebSocket binary protocol — identical to web client

The XR client connects to `wss://<host>/ws` and consumes the V3 full-sync
binary frames defined in ADR-02:

- Header: `magic = 0xV3F0`, `frame_id` (u32 monotonic per connection).
- Per-node payload: 28 bytes (per ADR-02's wire format).
- Trailer: `node_count`.

The decoder is a Rust function in `xr-client/rust/src/protocol.rs`,
compiled to a `.so` / `.dylib` / `.dll` and loaded by Godot via gdext.
It writes positions into a `PackedFloat32Array` buffer shared with the
GDScript scene through a `XRGraphState` Godot resource. Decoding is
zero-copy from the WebSocket frame into the PackedArray — no intermediate
`Vec<f32>`.

The XR client implements ADR-02 D7's `frame_id` drop-detection counter
and surfaces it on the in-VR diagnostics overlay. Lost frames are not
retransmitted; the next frame is full state anyway.

### F2. Nostr challenge-response on launch

On WebSocket upgrade, the server (per ADR-06) requires a Nostr-signed
JWT in the `?token=` query parameter. The XR client:

- Reads the user's Nostr nsec from a local secret store on first launch
  (Android Keystore on Quest; libsecret on Linux; DPAPI on Windows).
- First-launch onboarding: a Godot UI scene generates a fresh nsec if the
  user has none, displays the npub, prompts the user to register the npub
  on the web client (out-of-band) before connecting.
- For each connection, requests a challenge from `GET /auth/challenge`,
  signs the challenge with the local nsec, exchanges the signature for a
  JWT via `POST /auth/verify`, attaches the JWT to the WebSocket URL.

The JWT is held in memory only — never written to disk. The nsec never
leaves the secret store; signing happens in the gdext Rust bridge with
`nostr-sdk`.

### F3. Visual primitive parity with web client

Three node visual classes, geometries identical to PRD-04 F1:

- **Gem** (knowledge). Geometry: Icosahedron, base radius `0.5`. Material:
  `xr-client/materials/gem.tres` — Godot `StandardMaterial3D` configured
  for transmission, low roughness, IOR ≈ 1.45, tinted by per-instance
  `INSTANCE_CUSTOM`.
- **Crystal Orb** (ontology). Geometry: Sphere, radius `0.5`. Material:
  `xr-client/materials/crystal_orb.tres` — higher transmission, lower
  roughness than gem.
- **Agent Capsule** (agents). Geometry: Capsule, radius `0.3`, height
  `0.6`. Material: `xr-client/materials/agent_capsule.tres` — emissive
  tint, opaque.

Each class is a `MultiMeshInstance3D` (Godot's equivalent of three.js
`InstancedMesh`). Class membership is derived from the type-flag bits in
the V3 payload's `node_id` field. See ADR-08 §D6 for the canonical
class-bit allocation. The XR client must not duplicate the classification
logic; it reads the flag bits from the `node_id` field exactly as the
web client does.

### F4. Edges with surface-to-surface offset

Single edge visual class, mirroring PRD-04 F2 / F9:

- `MultiMeshInstance3D` of unit-height cylinders (Godot `CylinderMesh`,
  radius `0.03`, height `1`).
- Each instance placed at midpoint, scaled along local Y to inter-node
  distance, rotated to align Y axis with src→tgt.
- Surface-to-surface offset: cylinder shortened by `(srcR + tgtR)` along
  the connection axis where `srcR / tgtR` come from the class radius
  (`0.5` for gem / orb, `0.3` for capsule, matching PRD-04 F9).
- Per-instance colour buffer encodes edge category (knowledge edge,
  namespace edge, ontology relation, agent communication).

No edge-flow shaders, no ribbons, no animated edges in v1. Static
cylinders with category tint.

### F5. Edge capacity is configured, not magic

Same rule as PRD-04 F3: no top-level `MAX_EDGES` constant. The
`MultiMeshInstance3D`'s instance count starts at
`min(currentEdgeCount * 1.5, ceiling)` on first non-empty frame, grows
to `max(currentEdgeCount, current_capacity * 2)` when exceeded, ceiling
read from a project setting (`xr/rendering/max_edges_ceiling`, default
`64_000`).

### F6. Labels via Godot Label3D pool

Labels are pooled `Label3D` nodes, one per visible node, billboard-mode
`BILLBOARD_ENABLED` so they always face the camera. The pool is sized to
`max_visible_labels` (default `512`) and recycled per frustum-cull pass.
Layout rebuild runs every 3 frames (matching PRD-04 F5); position patch
runs every frame from the shared `PackedFloat32Array`.

Text content comes from the V3 payload's optional label slot (or is
fetched lazily from `GET /graph/node/:id/label` and cached). No
in-VR text editing.

### F7. Hand-tracking interactions

The XR client uses OpenXR hand tracking (Godot 4.3+ `XRHandTracker`):

- **Pinch on a node** (thumb-index distance < 0.025m, ray-casting from
  index fingertip): the node highlights (emissive boost, scale 1.1x).
  Highlight persists while pinch held; releases on un-pinch.
- **Pinch-and-drag in empty space**: pans the camera rig laterally. Drag
  delta is the controller pose delta in world space.
- **Gaze + pinch**: when the user gazes at a node (eye-tracking on Quest
  Pro, head-forward fallback on Quest 3) for >250ms and pinches, the
  node is *selected*. Selection state opens a floating info panel
  (Label3D + Sprite3D) at the node showing the node's metadata fetched
  from the same REST endpoints the web client uses.
- **Two-handed pinch** (both hands pinched simultaneously): pinch-zoom.
  The world scales between the hands' midpoint, clamped to
  [0.1, 10.0] scale factor.

Interaction state lives in `xr-client/scripts/interaction_state.gd`.
Selection is single — selecting a new node deselects the previous.

### F8. Comfort policy — non-negotiable

The XR client enforces three comfort affordances:

- **Snap-turn**: thumbstick-left / thumbstick-right (or pinch-flick on
  hand-only) rotates the camera rig in discrete 30° increments. No
  smooth turning. The snap is instantaneous with a 100ms vignette to
  hide the discontinuity.
- **Vignette on translation**: any pinch-and-drag pan or pinch-zoom
  movement above 0.5 m/s triggers a radial darkening vignette
  (post-process shader at `xr-client/shaders/comfort_vignette.gdshader`).
  Vignette inner radius `0.4`, outer `0.9`, opacity ramping with speed.
- **IPD-aware camera rig**: the XRCamera3D's IPD is read from OpenXR at
  scene start; node and edge scale (`nodeSize`, `edgeRadius`) are *not*
  IPD-adjusted, but the camera near-plane is set to
  `max(0.05, ipd * 0.4)` to avoid sub-IPD clipping artefacts.

These are not configurable. A user who turns them off on a different
project is welcome to fork; in this project they are part of the
shipped experience.

### F9. Performance ceiling — 90Hz on Quest 3 with 5k nodes

The XR client targets:

- **Frame rate**: 90Hz on Quest 3 standalone with a 5,000-node /
  20,000-edge graph in the SETTLED state. No frame may exceed 11.1ms
  on the GPU.
- **Cold-start**: from app launch to first rendered frame ≤ 4 seconds
  (Quest 3 cold-start budget).
- **Settle-time**: from WebSocket connect to first SETTLED snapshot
  applied ≤ 6 seconds for a 5k-node graph.
- **Allocation budget**: zero allocations per frame in the render path.
  Re-uses `PackedFloat32Array` buffers and pre-allocated
  `MultiMesh.transform` storage. The allocator pressure target matches
  PRD-04 F10.
- **Battery**: foveated rendering enabled (Vulkan multiview + Quest fixed
  foveation level 2); aim for ≥1 hour continuous use on a fully charged
  Quest 3 with screen-on time as the dominant cost.

These targets are validated by `xr-client/perf/run_benchmark.gd` against
the fixture `xr-client/perf/fixtures/perf_graph_1k.json` (scaled 5×) and
checked by `xr-client/perf/regression_check.py` in CI.

### F10. Agent telemetry display (XR mirror of Section 7)

The XR client subscribes to the same agent telemetry endpoints as the
web client (per ADR-07). Each agent node carries a small floating
panel above it (Sprite3D + Label3D) showing:

- Agent name / type
- Current task (truncated to 40 chars)
- Health indicator (green / amber / red)

Panels are only rendered for agent-class nodes within 5m of the camera
(distance gate) and within the frustum. The panel pool is pre-allocated
at `max_visible_agent_panels` (default `32`).

### F11. XR presence routed via separate crate

`crates/visionclaw-xr-presence` is a standalone Rust crate that:

- Reads head pose (HMD position + orientation), hand poses (left/right
  joint poses, pinch state), and gaze direction (eye-tracking where
  available) from the OpenXR runtime.
- Encodes them into a compact `XRPresenceFrame` struct (≤ 256 bytes).
- Publishes them at 30Hz over a *separate* WebSocket endpoint
  (`wss://<host>/ws/xr-presence`) — distinct from the graph data
  WebSocket (F1) so that presence backpressure does not affect graph
  state and vice versa.

The presence stream is *unidirectional* (client → server). In v1 the
server logs presence for analytics only; multi-user shared XR sessions
(where peer presence would be relayed back) is out of scope. The
contract is forward-compatible: the server's `/ws/xr-presence` accepts
presence frames silently in v1 and is wired to a future relay in v2.

### F12. Tests via GUT

GDScript tests run under GUT (Godot Unit Test). The minimum suites:

- `xr-client/tests/unit/test_scene_load.gd` — boot scene loads, all
  three material `.tres` files resolve, the `XRGraphState` resource is
  registered.
- `xr-client/tests/unit/test_protocol_decode.gd` — feed a synthetic V3
  frame into the gdext bridge, assert PackedFloat32Array length and
  per-node positions match expected.
- `xr-client/tests/unit/test_comfort_policy.gd` — verify snap-turn
  produces exactly 30° rotation, vignette activates above 0.5 m/s.
- `xr-client/tests/unit/test_capacity_growth.gd` — verify edge
  MultiMesh capacity doubles on overflow, never shrinks, never exceeds
  ceiling.

Tests are run by `xr-client/tests/run_gut.gd` in headless Godot. CI runs
them on a Linux runner without OpenXR; XR-specific runtime checks
(hand-tracking, OpenXR pose) are gated by `OS.has_feature("xr")` and
mocked under headless.

## Acceptance criteria

- **A1**: The XR client connects to the existing VisionClaw server with
  no server-side code changes. The same `wss://<host>/ws` URL serves
  both web and XR clients.
- **A2**: A 5,000-node / 20,000-edge graph renders at 90Hz on Quest 3
  in SETTLED state, validated by `xr-client/perf/run_benchmark.gd`.
- **A3**: Nostr challenge-response completes within 1 second on a warm
  start; the JWT is in memory only and is never written to disk.
- **A4**: All three node geometries (Icosahedron r=0.5, Sphere r=0.5,
  Capsule r=0.3 h=0.6) render with materials specified in F3. Per-class
  instance counts match the type-flag bits in the V3 payload.
- **A5**: Edges visually touch node surfaces (F4 / PRD-04 F9 parity).
  Per-instance edge colours encode category.
- **A6**: `MAX_EDGES` is not present as a top-level constant in any
  GDScript or gdext source file. Capacity is read from project settings.
- **A7**: Pinch-on-node highlights, pinch-and-drag pans, gaze-and-pinch
  selects. Two-handed pinch zooms. All four are usable without a tutorial
  by a first-time tester (informal usability gate, not measured).
- **A8**: Snap-turn rotates exactly 30°. Vignette activates at speeds
  above 0.5 m/s. IPD-driven near plane is set at scene start.
- **A9**: Cold-start app-launch to first frame is ≤ 4s on Quest 3;
  graph settle-time is ≤ 6s for a 5k-node graph.
- **A10**: Zero per-frame allocations measured by Godot's profiler over
  a 60s steady-state recording on Quest 3.
- **A11**: The XR client builds for Android (Quest) using
  `xr-client/android-export-template-config.txt` and
  `xr-client/export_presets.cfg` without manual config edits.
- **A12**: XR presence frames are published at 30Hz to
  `wss://<host>/ws/xr-presence`; presence backpressure does not impact
  graph data WebSocket receipt rate.

## Non-goals

- **N1**: WebXR (browser-side XR). The web client described in PRD-04
  is desktop / 2D only. WebXR is a separate consumer-class — handled
  by neither this section nor PRD-04.
- **N2**: Port to Unity or Unreal. Godot is fixed for this section. A
  Unity port would be a separate workstream with its own ADR.
- **N3**: Multi-user shared XR sessions. Presence is published one-way
  in v1 (F11); peer relay, avatars, voice chat, shared selection are
  deferred. The wire format is forward-compatible.
- **N4**: In-VR text editing of node content. Knowledge graph editing
  remains a desktop / web operation.
- **N5**: In-VR settings panel parity with the web Control Panel. The
  XR client exposes a minimal in-headset settings menu
  (connect / disconnect / comfort sliders / disconnect-on-low-battery).
  Full settings management stays on the web client.
- **N6**: Custom Godot fork / engine modifications. The XR client uses
  stock Godot 4.3+ from the official release channel.
- **N7**: Per-class physics in the XR client. Physics is computed
  server-side (Section 1); the XR client only renders positions.
- **N8**: Spatial audio for graph navigation. A future workstream may
  add audio cues; v1 is silent.

## Dependencies

- **Section 2 (Binary protocol)** — V3 wire format is the input
  contract. Any change to the V3 frame layout breaks the XR client.
  Coordination point: F1's `xr-client/rust/src/protocol.rs` must be
  updated in lockstep with `src/utils/binary_protocol.rs` on the server.
- **Section 6 (Auth)** — Nostr challenge-response endpoints
  (`GET /auth/challenge`, `POST /auth/verify`) are the auth contract.
  The XR client uses these unchanged.
- **Section 7 (Bots & Telemetry)** — F10's agent panels consume the
  same REST endpoints the web client uses for agent metadata. No
  XR-specific telemetry contract.
- **Section 8 (Ontology / KG data)** — type-flag bits in the V3 payload
  are the input for F3's class classification. The XR client does not
  duplicate the classifier.
- **Section 5 (Settings)** — `rendering.maxEdgesCeiling` and a small
  number of comfort settings are mirrored under the project setting
  namespace `xr/rendering/*` and `xr/comfort/*`. The XR client reads
  these locally; it does not sync them with the server's settings store.

## Migration approach

Baseline `41979d33e3` does not contain an XR client. This is a
greenfield section. The migration is a project creation:

1. **Scaffold** the Godot project at `xr-client/` with `project.godot`,
   `xr_boot.gd` boot scene, materials `.tres` files (F3).
2. **Bridge** `xr-client/rust/` as a gdext crate. Implement the V3
   protocol decoder (F1) and Nostr signing (F2). Confirm `.so` loads
   in headless Godot.
3. **Carry the visual primitives forward** from PRD-04 F1–F2 / F9 as
   MultiMesh-based renderers (F3 / F4 / F5).
4. **Add hand tracking and comfort** (F7 / F8) using Godot's XRTools
   plugin (Quest XRTools, MIT-licensed) as the starting point; replace
   bespoke locomotion with the comfort policy.
5. **Wire performance benchmark** (F9 / `xr-client/perf/`) before
   merging. Regressions block merge.
6. **Define `crates/visionclaw-xr-presence`** (F11) as a standalone
   crate so the Godot client and any future native non-Godot consumers
   share a presence format.

The web client (PRD-04) does not need to change to support the XR
client. The server (ADR-02 / ADR-06) does not need to change. This is
a pure consumer addition.

## Open questions resolved here

- *Should the XR client reuse the web client's binary protocol decoder
  by compiling it to WASM and loading it in Godot?* No. Godot has no
  ergonomic WASM host; we re-implement the V3 decoder natively in
  gdext Rust. The protocol is small (28 bytes per node, fixed
  layout) — duplication cost is low.
- *Should we render edges as `ImmediateMesh` for true line primitives?*
  No. MultiMesh of cylinders matches PRD-04 F2 visual exactly; lines
  in VR alias badly on Quest's foveated regions.
- *Should snap-turn angle be configurable?* No. 30° is the standard
  Quest comfort guideline; the project ships with the safe default
  and does not surface this as a knob.
- *Should we ship a non-XR fallback (flat 2D Godot)?* No. The Godot
  build is XR-only. Users without XR hardware use the web client.
