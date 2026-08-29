# VisionClaw XR — Godot OpenXR client

Godot + godot-rust (gdext) + OpenXR client per
[PRD-008](../docs/PRD-008-xr-godot-replacement.md) and
[ADR-071](../docs/adr/ADR-071-godot-rust-xr-replacement.md).

**Two run targets, one codebase:**
- **Quest 3 native APK** — the sole *ship* target. Cross-build is currently
  **frozen** (no Android NDK provisioned in this environment; see below).
- **Desktop OpenXR (SteamVR / VIVE Pro)** — the close-out *validation* target
  per [ADR-136](../docs/adr/ADR-136-desktop-openxr-vive-validation-target.md).
  This is the path that has **actually rendered on a headset** — see
  "VIVE Pro desktop-OpenXR bring-up (2026-08-22)" below.

> **Version note.** The project was authored against Godot 4.3; the first
> working on-headset render (2026-08-22) was achieved on **Godot 4.6.1-stable**
> using the **Compatibility (OpenGL 3) renderer**. Where this README still says
> "4.3", read it as "the pinned editor of the day"; the *runtime* facts that
> matter for a headset session are in the VIVE bring-up section.

## Layout

```
xr-client/
├── project.godot                       Godot 4.3 project root
├── export_presets.cfg                  Quest 3 arm64 Android preset
├── visionclaw_xr_gdext.gdextension     Manifest binding the Rust .so
├── icon.svg                            Project icon
├── android-export-template-config.txt  OpenXR loader notes
├── permissions-required.md             Android permission justification
├── scenes/
│   ├── XRBoot.tscn                     Boot: OpenXR init + capability probe
│   ├── GraphScene.tscn                 Graph rendering + AvatarSpawner
│   ├── HUD.tscn                        Settings / room controls / debug
│   └── Avatar.tscn                     Per-remote-presence template
├── scripts/                            GDScript wiring (no business logic)
├── materials/                          gem / crystal_orb / agent_capsule
├── addons/                             Godot OpenXR Vendors (CI-installed)
└── rust/                               gdext crate (see ./rust/Cargo.toml)
```

## Build prerequisites

| Tool | Version | Notes |
|---|---|---|
| Godot 4.3 stable | 4.3 | Both editor and Android export templates |
| Godot OpenXR Vendors plugin | 3.0.x | Install via AssetLib at first project open |
| Rust toolchain | stable 1.82+ | `rust-toolchain.toml` not pinned in this dir; uses workspace default |
| Android target | `aarch64-linux-android` | `rustup target add aarch64-linux-android` |
| Android NDK | r26d | Pinned in `xr-client/android/local.properties.template` (created in W2) |
| `cargo-ndk` | latest | `cargo install cargo-ndk` |
| JDK | 17 | Android Gradle plugin requirement |

## Build steps (manual)

```bash
# 1. Build the gdext .so for the host OS (for editor preview)
cargo build -p visionclaw-xr-gdext --release

# 2. Build for Quest 3 (arm64 Android)
cd xr-client/rust
cargo ndk -t aarch64-linux-android -o ../addons/visionclaw_xr_gdext build --release
cd ../..

# 3. Open Godot, import project, install OpenXR Vendors via AssetLib

# 4. Export the APK
godot --headless --export-release "Quest 3 arm64" xr-client/export/visionclaw-xr.apk

# 5. Side-load to Quest 3 (developer mode + USB debugging)
adb install -r xr-client/export/visionclaw-xr.apk
adb shell am start -n uk.xrsystems.visionclaw.xr/com.godot.game.GodotApp
```

> **Quest APK cross-build is FROZEN.** Steps 2 and 4–5 above are the design
> path but do **not** run in the current environment: there is no Android NDK,
> no `aarch64-linux-android` Rust std component, and no `cargo-ndk`. All headset
> validation to date is via the **desktop OpenXR** path (see below); the arm64
> APK build is pending a provisioned Android toolchain.

## Desktop OpenXR run (VIVE Pro / SteamVR — the working headset path)

The canonical, verified way to get the client onto a real headset today is
**not** the APK — it is the Godot Compatibility renderer on native X11 driving
SteamVR:

```bash
# Prereqs on the render host (HP-Desktop, CachyOS):
#   - Godot 4.6.1-stable (Compatibility build)
#   - SteamVR running, VIVE Pro tracked (lighthouses up)
#   - NVIDIA 580 open driver (nvidia-580xx-open-dkms, PINNED — see constraints)
#   - ~/.config/openxr/1/active_runtime.json → SteamVR

XR_BACKEND_WS=ws://<backend-host>:4000 \
XR_NOSTR_SECRET=<hex-secret> \
godot --path xr-client \
      --rendering-driver opengl3 \
      --display-driver x11 \
      res://scenes/XRBoot.tscn
```

- `XR_BACKEND_WS` — presence/graph WebSocket. Point at the dev backend
  (`ws://192.168.2.132:4000` on the LAN). **Use `:4000` directly**; nginx `:3001`
  does not proxy `/ws/presence`.
- `XR_NOSTR_SECRET` — hex Nostr secret key for the presence challenge/response
  handshake (any BIP-340 key in dev).

### Render constraints (learned the hard way, 2026-08-22)

These are not preferences — each one is a workaround for a concrete failure
observed bringing up the VIVE Pro:

| Constraint | Why |
|---|---|
| **Compatibility (OpenGL 3) renderer is mandatory** (`--rendering-driver opengl3`) | The RenderingDevice / Vulkan **multiview tonemapper is broken on SteamVR + Linux + NVIDIA** — the stereo submission fails. Compatibility is the only renderer that submits both eyes. |
| **Glow OFF** | Godot's glow post-process breaks Compatibility XR multiview submission (blank/black second eye). Disabled in `WorldEnvironment`. |
| **NVIDIA 580, not 610** (`nvidia-580xx-open-dkms`, pinned) | The 610 driver fails to render the GL multiview **second eye**; 580 open renders both eyes correctly. Pin the driver. |
| **Native X11** (`--display-driver x11`) | SteamVR's Linux compositor path; Wayland was not brought up. |

## Test the gdext crate (headless)

```bash
cargo test -p visionclaw-xr-gdext
```

The headless suite — 9 integration files under `rust/tests/`
(`binary_protocol_edge`, `interaction_raycast`, `lod_thresholds`,
`pose_wire_round_trip`, `presence_handshake`, `property_binary_protocol`,
`property_interaction`, `property_lod`, `visual_fixture`) plus 100+ per-module
unit tests — runs in <1 s on a workstation. No Quest, no Godot runtime, no network
required. (PRD-019 tracks the canonical cross-crate total: 141 headless tests green.)

## gdext-registered classes

Read by `scripts/graph_scene.gd`; the registration lives in
`rust/src/lib.rs`. Each class is a `RefCounted` constructed via
`ClassName.create()` from GDScript:

| Class | Module | Signals |
|---|---|---|
| `BinaryProtocolClient` | `binary_protocol.rs` | `position_updated(node_id: u32, position: Vector3, velocity: Vector3)` |
| `PresenceClientNode` | `presence.rs` | `avatar_joined(did, display_name, avatar_id)`, `avatar_left(avatar_id)`, `avatar_pose_updated(avatar_id, head_pos, head_rot, has_left, has_right)`, `presence_kicked(reason)` |
| `XrInteraction` | `interaction.rs` | `node_targeted(node_id, distance)`, `node_grabbed(node_id, position)`, `haptic_pulse(controller, intensity)` |
| `LodPolicy` | `lod.rs` | (no signals; `should_recompute()` + `classify_distance()` getters) |
| `SpatialVoiceRouter` | `webrtc_audio.rs` | (no signals; `attach_track`, `detach_track`, `update_listener`) |
| `AgentAvatarNode` | `avatar_state.rs` | `activity_changed` — copresence activity state machine + gaze-attention |
| `ProxemicsSolver` | `proxemics.rs` | (no signals; Hall's-zones arc placement) |
| `GazeTracker` | `gaze.rs` | (no signals; unified head/eye gaze ray + one-euro smoothing) |
| `SelectionArbiterNode` | `selection.rs` | `selection_made(node_id: u32, did_nostr: GString, resolver: i32)` — controller-ray / pinch / gaze-dwell arbiter |
| `NostrAuth` | `signer.rs` | (no signals; BIP-340 presence challenge/response signer) |

All classes are `#[class(no_init, base = RefCounted)]` — constructed from
GDScript via `ClassName.create()`.

> **Interaction note (2026-08-22 bring-up).** `XrInteraction` grab now does a
> literal ray/sphere intersection with `TARGET_RADIUS` tightened `1.0 → 0.05`
> and a best-aligned pick, casting in **world space** (previously it tested a
> world-space ray against server coordinates, so grabs missed). Locomotion is
> trackpad-driven; the HUD is a world-anchored, wand-grabbable panel (grip to
> move). Controllers bind via `openxr_action_map.tres` to
> `htc/vive_controller` with `pose="aim"`.

## VIVE Pro desktop-OpenXR bring-up (2026-08-22)

First working **on-headset render** of the Godot XR client in project history —
branch `xr-vive-runtime`. Stack: **Godot 4.6.1-stable Compatibility renderer +
NVIDIA 580 open + native X11 + SteamVR** on HP-Desktop, VIVE Pro.

**What rendered / worked in the headset:**
- **Both eyes** — stereo submission via the Compatibility renderer (the RD/Vulkan
  multiview tonemapper is broken on SteamVR/Linux/NVIDIA; glow off; NVIDIA 580
  for the GL multiview second eye).
- **Dual-wand grab** — world-space raycast, literal ray/sphere pick
  (`TARGET_RADIUS` 1.0→0.05). **Right wand confirmed**; left wand still WIP.
- **Trackpad locomotion**.
- **Movable HUD** — world-anchored, grip-to-move on either wand.
- **Graph render** — top-5% node cap (**640 nodes**), per-node hue fallback,
  **adaptive fit** (GraphRoot scales/recentres the streamed graph into a
  room-sized volume, re-measured **each frame** so it tracks physics spread).
- **Client-side optimistic position hunting** — `_node_targets` eased into
  `_node_positions`, matching the desktop "GPU settles fast, clients hunt" model.
- **Edges** — the backend `initialGraphLoad` node limit was raised **200 → 3000**
  so edges actually reach the client (edge count **49 → 7558**); client renders
  edges only between displayed nodes, with robust src→tgt orientation.

**Backend changes that made the graph live:**
- **Continuous settle** default (was a FastSettle latch that froze the graph
  once converged → positions stopped streaming).
- **`PinNodePositions` GPU injection** on grab — a dragged node perturbs its
  neighbours' springs; grab re-arm on release.
- `wire.rs` decode-panic guard on truncated frames.
- Physics is live-tunable via dev-auth `PUT /api/settings/physics`
  (`Authorization: Bearer dev-session-token`).

**Still pending / WIP (honest):**
- **Canary fires NOT done** — `CANARY-VC-M1-HUD`, `CANARY-VC-M4-RAY`,
  `CANARY-VC-COM18-INTERV` remain **armed, unfired**. On-device render was
  achieved; the canary predicates have not yet been observed firing, so the
  P2-M* tiers are **not** promoted to `integrated`.
- **Edge tracking** — edges render but per-frame tracking of moving endpoints
  needs follow-up.
- **Adaptive-fit-on-grab** — fit interaction during an active grab is unfinished.
- **Left wand** — only the right wand grab is confirmed.
- **Glow parity** — glow is off (Compat XR constraint); visual parity with the
  desktop client is outstanding.
- Debug prints retained in the client (WIP branch).

## Cross-references

- **Wire format**: [`crates/visionclaw-xr-presence/src/wire.rs`](../crates/visionclaw-xr-presence/src/wire.rs) — opcode 0x43 single source of truth
- **Bounded context**: [`docs/ddd-xr-godot-context.md`](../docs/ddd-xr-godot-context.md) BC22
- **Threat model**: [`docs/xr-godot-threat-model.md`](../docs/xr-godot-threat-model.md)
- **Architecture**: [`docs/xr-godot-system-architecture.md`](../docs/xr-godot-system-architecture.md)
