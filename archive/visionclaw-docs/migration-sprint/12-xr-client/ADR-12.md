# ADR-12 — XR Client (Godot + gdext)

Status      : Proposed
Date        : 2026-05-16
Related     : ADR-02 (Binary Protocol — XR consumes same V3 stream),
              ADR-04 (Web rendering — visual parity reference),
              ADR-06 (Auth — Nostr challenge-response),
              ADR-07 (Bots & Telemetry — agent panel contract),
              PRD-12 (capability statement)

## Context

The web client (PRD-04 / ADR-04) is desktop-only. WebXR on Quest browsers
is not viable for graphs of 5k+ nodes — frame budget, hand-tracking
fidelity, and update cadence are all bottlenecked by the host browser.
A native XR client is required, but a port of the web client's React +
three.js stack to a native engine is not the right shape: it would either
duplicate the data path (settings, auth, position state) or fork the
rendering layer (R3F is web-only).

Instead, the XR client is treated as a *second consumer* of the same
VisionClaw server, written natively in Godot 4.x with a Rust gdext bridge.
The data path (V3 protocol, Nostr auth, agent telemetry endpoints) is
unchanged. The XR client implements its own rendering, interaction, and
comfort layers but matches the visual primitives exactly (Icosahedron
r=0.5, Sphere r=0.5, Capsule r=0.3 h=0.6).

The hardware target is Meta Quest 3 standalone (Android, Vulkan, ARMv8).
Tethered OpenXR (Linux / Windows) is supported but secondary.

## Decision

### D1. Godot 4.x as the engine — not Unity, not Unreal, not custom

Godot is chosen for four reasons. First, MIT licence means no royalty
overhead and no per-platform store gates. Second, native OpenXR support
since Godot 4.0 with Quest XRTools as a community-maintained interaction
layer. Third, GDScript + gdext gives us a thin scripting surface for
scene logic and a compiled Rust path for performance-critical work
(protocol decode, signing) without a C++ build pipeline. Fourth, Godot's
Android export targets Quest cleanly via the official Android export
template.

Unity is rejected: per-seat licensing, recent licence churn, and a C#
runtime that would force a duplicate codebase from the existing Rust
backend stack. Unreal is rejected: C++ ergonomics, weight (Quest builds
are large), and the Unreal-specific rendering pipeline would not match
the simple instanced-mesh model we need. A custom OpenXR client in raw
Rust + wgpu was considered and rejected — too much engine-level work
(scene graph, asset pipeline, UI) for a feature with a single ship target.

### D2. Same V3 protocol, byte-for-byte — no XR-specific wire format

The XR client consumes the exact V3 full-sync binary protocol defined in
ADR-02. Header (`magic = 0xV3F0`, `frame_id`), 28-byte per-node payload,
trailer (`node_count`). The gdext bridge at `xr-client/rust/src/protocol.rs`
mirrors `src/utils/binary_protocol.rs` byte-for-byte.

This is non-negotiable. A separate XR wire format would:

- Double the server's broadcast work (two encoders, two cadence
  managers).
- Create drift risk: every protocol change would need two
  implementations, and there is no way to keep them in sync without
  a shared crate.
- Add an XR-specific settlement gate that would mismatch the web client's
  view of the graph state.

The settlement-gated cadence (ADR-02 D2) applies unchanged: ACTIVE up to
10 Hz, SETTLED on heartbeat (5s). At 90Hz XR rendering, this means the
XR client is interpolating between 10 Hz sample frames — which is fine
because at SETTLED state the positions don't move at all and at ACTIVE
state the eye tolerates extrapolation more than the desktop does.

### D3. gdext (Rust → Godot binding) for the bridge layer

The protocol decoder, the Nostr signer, the WebSocket client, and the
XR presence encoder are all in Rust, compiled as a single gdext shared
library (`xr-client/rust/target/release/libvisionclaw_xr.so` on Linux,
`.dylib` on macOS, `.dll` on Windows, `.so` on Android). Godot loads
this via `xr-client/visionclaw_xr.gdextension`.

The bridge exposes three Godot classes:

- `VisionclawProtocol` — `decode_frame(bytes: PackedByteArray) ->
  XRGraphState`, where `XRGraphState` is a Godot Resource carrying
  PackedFloat32Array positions, PackedInt32Array node_ids,
  PackedInt32Array type_flags.
- `VisionclawAuth` — `request_challenge(url) -> Signal`,
  `sign_and_verify(challenge) -> String` (returns JWT).
- `VisionclawPresence` — `start(url) -> Signal`, called by the XR
  scene every frame with the head/hand/gaze pose; the bridge batches
  to 30Hz internally.

GDScript drives scene logic, instancing, comfort UI, and tests. The
discipline is **bridge only does what GDScript cannot do efficiently**.
Reading WebSocket frames in GDScript would allocate per frame; in Rust
it does not.

### D4. Visual primitive parity is a hard constraint

The XR client renders the same three node classes as PRD-04 F1: gem
(Icosahedron r=0.5), crystal orb (Sphere r=0.5), agent capsule (Capsule
r=0.3 h=0.6). Materials are Godot `StandardMaterial3D` resources at
`xr-client/materials/{gem,crystal_orb,agent_capsule}.tres`, tuned so
the visual outcome (PBR transmission, IOR, roughness, emissive tint)
matches the web client's `GemNodeMaterial` / `CrystalOrbMaterial` /
`AgentCapsuleMaterial`.

Parity means: a user wearing the headset and looking at a node should
recognise it as the same node they would see on the web client. The web
client is the visual specification; the XR client implements it. Where
Godot's PBR differs from three.js's PBR (it does in subtle ways —
transmission falloff, schlick term), the XR client tunes to look
*similar enough* and documents the divergence.

Class membership is computed in the gdext bridge from the V3 payload's
node_id type-flag bits. See ADR-08 §D6 for the canonical class-bit
allocation. The XR client does not re-classify nodes; it reads the bits.

### D5. Hand-tracking-first interaction, controllers as fallback

The Quest 3 supports both hand tracking and Touch controllers. The XR
client treats hand tracking as the primary modality, controllers as a
parity fallback that maps each pinch / drag / select to the equivalent
trigger / grip / stick action. This is the opposite of most VR apps,
which lead with controllers.

The reasoning: a knowledge graph is read more than it is manipulated.
Users pick up the headset, sit on the sofa, and want to *look*. Hands
are always present; controllers require finding them, charging them,
and pairing. Hand tracking with the gesture set defined in PRD-04 F7
(pinch on node, pinch-and-drag pan, gaze-and-pinch select, two-handed
pinch zoom) is enough for the core experience.

Controller mapping (when controllers are detected):

- Trigger press = pinch
- Thumbstick = snap-turn (left/right) + dolly (forward/back)
- Grip = same as pinch (for users who find the trigger awkward)
- Menu button = open in-VR settings panel

### D6. Comfort policy is enforced, not configurable

PRD-12 F8 lists three comfort affordances: snap-turn, vignette on
translation, IPD-aware near-plane. None of these are exposed as
settings in v1. The reasoning: motion sickness is the worst outcome
for an XR app; a user who toggles comfort off and feels sick associates
the app, not the comfort setting, with the discomfort. Ship safe
defaults, gather telemetry, iterate.

Snap-turn at exactly 30° is the Quest comfort guideline. The vignette
threshold at 0.5 m/s is empirically chosen — slow enough that
deliberate panning doesn't trigger it, fast enough that an inadvertent
push-against-the-edge does. The IPD-driven near plane prevents the
nasal-bridge clipping artefact when interpupillary distance is below
~58mm.

### D7. Performance ceiling is enforced by CI, not by hope

The performance fixture at `xr-client/perf/fixtures/perf_graph_1k.json`
is a 1,000-node graph that is loaded at 5× scale (5,000 nodes) by
`xr-client/perf/run_benchmark.gd`. The benchmark records:

- Median GPU frame time
- 95th percentile GPU frame time
- Heap allocations per second
- Settle time from connect to SETTLED snapshot

`xr-client/perf/regression_check.py` parses the benchmark output, reads
the baseline from `xr-client/perf/baselines/quest3.json`, and fails the
build if any metric regresses by more than 5%. CI runs the headless
half (allocations, settle time) on every PR; the Quest 3 GPU half runs
on a connected device in a nightly job.

This is a "fail loud" stance: a performance regression that ships to
users is much worse than a CI failure that blocks merge. The 5%
threshold tolerates noise; anything beyond is a real regression that
must be investigated.

### D8. Nostr auth — same flow as web client, secure local secret

The Nostr challenge-response flow defined in ADR-06 applies unchanged:

1. Client: `GET /auth/challenge` → returns challenge (random nonce + ts).
2. Client: signs challenge with nsec using `nostr-sdk` (BIP-340 Schnorr).
3. Client: `POST /auth/verify` with signature + npub → returns JWT.
4. Client: connects `wss://<host>/wss?token=<jwt>` (canonical path per
   ADR-06 §D12).

The XR client stores the nsec in the platform secret store:

- **Android (Quest)**: Android Keystore via `AndroidJNI` from Godot.
  Keystore-backed nsec is encrypted at rest, decrypted only in-process.
- **Linux**: libsecret (via `secret-service`); falls back to a
  GPG-encrypted file in `~/.local/share/visionclaw/nsec.gpg` if no
  Secret Service is available.
- **Windows**: DPAPI via `CryptProtectData`.

The nsec never leaves the secret store; signing happens in the gdext
Rust bridge by passing the cleartext nsec to `nostr-sdk` for the
duration of one signing operation and zeroing the buffer immediately
after. The JWT is held in RAM only.

First-launch flow: if no nsec exists, the XR client offers to generate
one and prompts the user to register the resulting npub on the web
client out-of-band. This is friction; it is also the price of the
Nostr-only auth posture (ADR-06).

### D9. XR presence as a separate WebSocket endpoint

PRD-12 F11's presence stream goes to `/ws/xr-presence`, distinct from
`/wss` (the canonical V3 position broadcast endpoint per ADR-06 §D12).
Reasons:

- **Backpressure isolation**: if the presence path is dropping frames
  (the network is congested), the graph data path must not be affected.
  Two endpoints, two independent backpressure regimes.
- **Cadence mismatch**: graph data is event-driven (settlement-gated);
  presence is fixed-cadence 30Hz. Different timing characteristics belong
  on different connections.
- **Future routing**: when shared XR sessions ship (v2, out of scope),
  the relay will be presence-only and benefit from being on its own
  endpoint that the server can scale independently.

The presence frame format is defined in `crates/visionclaw-xr-presence`
as a public stable type. Server-side, the endpoint is a *sink* in v1
that logs to metrics and discards; v2 will add the relay. Adding a
second WebSocket endpoint is a server change — but a trivial one,
documented in ADR-06 §D12's canonical WebSocket enumeration as an
auth-protected route, no protocol changes.

### D10. Build target: Android (Quest) primary, desktop OpenXR secondary

The shipped build is the Quest 3 Android APK. Exported from Godot
using `xr-client/export_presets.cfg` (preset `Meta Quest`) with the
Android export template configured by
`xr-client/android-export-template-config.txt`.

Required permissions (declared in `xr-client/permissions-required.md`,
generated into the AndroidManifest):

- `com.oculus.permission.HAND_TRACKING` — for F7 hand interactions.
- `com.oculus.permission.EYE_TRACKING_CALIBRATION` — for F7 gaze on
  Quest Pro (no-op on Quest 3, declared anyway for forward compat).
- `android.permission.INTERNET` — for WebSocket connection.
- *Not requested*: microphone, camera (passthrough only requested
  via the Quest passthrough API, no raw camera access), file
  storage beyond the app's private dir.

Desktop OpenXR (Linux SteamVR, Windows OpenXR runtime) is a secondary
build target — same Godot project, different export preset. Useful for
debugging and for users with PCVR setups. Validated by the same test
suite under headless mode.

## Options considered

### O1. WebXR (browser-side XR)

Rejected. The web client described in PRD-04 is React + three.js +
R3F; WebXR-enabling it is non-trivial (R3F's XR support is
community-maintained, lags behind three.js releases, and has known
performance issues on Quest browsers). Beyond the engineering effort,
the browser's frame budget on Quest is too constrained for 5k-node
graphs — the same reason this section exists.

### O2. Port the web client to a native engine (Unity, Unreal)

Rejected. Either forces a C# (Unity) or C++ (Unreal) duplicate of the
data path, or requires a foreign-function bridge into the existing
Rust stack that is heavier than gdext.

### O3. Custom OpenXR client in raw Rust + wgpu

Rejected. Too much engine work — scene graph, asset pipeline, UI,
materials. The gain over Godot (smaller binary, more direct control)
does not justify the build-out cost given a single XR ship target.

### O4. Godot + gdext bridge, XR-specific server endpoints

Rejected — the XR-specific endpoints variant. The wire format would
fork, and the maintenance burden of keeping two protocols in sync
across the server, web client, and XR client is high. Adopted: Godot
+ gdext, same server endpoints (this ADR).

## Risks

- **R1**: gdext is a relatively young binding (Godot 4.2+). API stability
  is good but not guaranteed. Mitigation: pin the gdext version in
  `xr-client/rust/Cargo.toml`; upgrades happen deliberately, not via
  `cargo update`.
- **R2**: Quest XRTools is community-maintained and may lag Godot
  releases. Mitigation: forking points are well-known
  (interaction polish, controller mapping); we can fork if upstream
  stalls. The comfort policy (D6) is implemented in-tree, not via
  XRTools, so it survives an XRTools fork.
- **R3**: Material parity (D4) between three.js PBR and Godot's
  StandardMaterial3D is *approximate*, not pixel-exact. Mitigation:
  document the divergence; do not promise pixel parity in the PRD;
  iterate on materials based on user feedback.
- **R4**: The 90Hz / 5k-node ceiling (PRD-12 F9 / A2) is aggressive.
  If we miss it, the comfort policy (D6) becomes more important —
  motion-to-photon latency at lower frame rates is uncomfortable.
  Mitigation: D7's CI gate catches regressions; if the initial
  implementation cannot hit 90Hz at 5k, we negotiate a documented
  reduction (e.g. 72Hz at 5k, 90Hz at 3k) with the product owner
  rather than ship a quietly slow client.
- **R5**: Nostr secret-store integration is platform-specific (D8)
  and adds three platform-specific code paths. Mitigation: encapsulate
  in `xr-client/rust/src/secret_store.rs` with one trait and three
  impls; tests use an in-memory impl.
- **R6**: Hand tracking on Quest occasionally drops out (occluded
  hands, low light). Mitigation: controller fallback (D5) is always
  available; the in-VR settings panel offers a "controllers only"
  toggle for users who find hand tracking unreliable in their
  environment.

## Rejected from main as buggy / unjustified

This is a greenfield section; there is no `main` history to reject.

## Bugs and smells at the reset point (41979d33e)

No XR client exists at baseline. The relevant smell is in the wider
codebase: there is no documented contract for *non-web consumers* of
the V3 binary protocol. The web client is implicitly assumed.

Migration must surface this:

- The V3 protocol description in ADR-02 is updated to read
  "consumed by the web client (PRD-04) and the XR client (PRD-12)"
  so future protocol changes account for both.
- Server changes to the WebSocket auth flow (ADR-06) are likewise
  noted as affecting both consumers.
- The agent telemetry contract (ADR-07) is similarly noted as having
  two readers.

The XR client itself, being new, has no legacy bugs to flag. The first
implementation should resist the temptation to start with a Godot
WebXR plugin or a community-maintained Quest template; start from the
empty `project.godot` and grow only what PRD-12 specifies.
