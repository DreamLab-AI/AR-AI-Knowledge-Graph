# VIVE Pro Close-Out Session — Evidence Record (2026-08-20)

**Scope ruling:** [ADR-136](../adr/ADR-136-desktop-openxr-vive-validation-target.md) —
desktop OpenXR (SteamVR, VIVE Pro, HP-Desktop) accepted as the close-out
validation target for the PRD-008 §7.3 copresence layer; Quest 3 remains the
sole ship target. Executed by a Fable-queen / Opus mesh (WS-A backend
verification, WS-B HP bring-up, WS-C client fixes) under the
build-with-quality discipline: every claim below is command-receipted.

## 1. Environment receipts

| Fact | Evidence |
|---|---|
| HP-Desktop reachable | `ssh john@10.10.10.1` (BatchMode), CachyOS, kernel 7.1.5-1 |
| GPU | 2× Quadro RTX 6000, driver 610.43.03; Xorg glxinfo: OpenGL 4.6, direct rendering |
| VIVE Pro physically present | lsusb: HMD 0bb4:0309, audio 0bb4:030b, camera 0bb4:030c, BT hub 0bb4:0306, 2× Valve Watchman 28de:2101, LHR 28de:2300 |
| Godot 4.3-stable installed | `~/godot43/Godot_v4.3-stable_linux.x86_64 --version` → `4.3.stable.official.77dcf97d8` |
| gdext cdylib builds on HP | `cargo build --release -p visionclaw-xr-gdext` exit 0; `nm -D` exports `gdext_rust_init`; loads in Godot ("Initialize godot-rust API v4.3.stable") |

## 2. Test receipts

| Suite | Where | Result |
|---|---|---|
| `visionclaw-xr-gdext` cargo tests | agentbox container AND HP | **195 passed, 0 failed** (was 194; +1 `classify_graph_text` test) |
| `visionclaw-xr-presence` cargo tests | container | 24 passed, 0 failed |
| `visionclaw-protocol` (incl. new `did_nostr` tests) | container | 25 passed, 0 failed |
| `visionclaw-server --lib` | container | 931 passed, 0 failed, 6 ignored |
| GUT unit suite on real Godot 4.3 | HP | **6/6 scripts, 51/51 tests, 148 asserts, 0 failures** — first full GUT pass in project history (prior best: 1 script / 3 tests) |

Pre-existing, unrelated: `tests/analytics_correctness_test.rs` fails to
compile (E0499 double mutable borrow of `rng`, lines 396/416) — predates this
session, not touched.

## 3. Defects found and fixed (uncommitted, this working tree)

1. **`set_identity` native-method collision (production bug, found by real
   Godot only).** `Node3D` natively defines `set_identity()` (transform
   reset); Godot 4.3 rejects the 3-arg GDScript override as a parse error, and
   the `has_method("set_identity")` guards would have answered *true on every
   Node3D* and reset transforms instead of setting identity. Renamed the whole
   surface to `set_avatar_identity` (`agent_avatar.gd:79`, `avatar.gd:47`,
   `graph_scene.gd` spawn + join sites incl. guard strings, two test files).
   Collision sweep of every custom method vs Node3D/Node/Control natives:
   this was the only one. Neither headless `cargo test` nor static review
   could have caught this — the concrete justification for ADR-136.
2. **GDScript 4.3 `:=` inference from Variant** — `load()`, `instantiate()`
   and awaited-coroutine sites in 5 GUT test files; fixed with explicit types.
3. **`run_gut.gd` targeted a pre-9.x GUT API** (hang, no exit); replaced with
   a GUT 9.3 shim + `.gutconfig.json`; canonical invocation is
   `gut_cmdln.gd -gconfig=res://.gutconfig.json`.
4. **`did_nostr` absent from `initialGraphLoad`** (server): client parser read
   a top-level `node.did_nostr` the server never emitted. Added
   `InitialNodeData.did_nostr` + agent-metadata plumbing + both build sites
   (+2 tests). Verified compiled into the live dev backend (mounted source).
5. **Inbound Text frames dropped by the graph pump** (client): only
   `initialGraphLoad` was parsed; `broker:new_case` was lost. Added
   `GraphInbound::Text` + `classify_graph_text()` + `text_message` signal →
   `graph_scene.gd` routes `broker:new_case` to the HUD `show_case` entry.
6. **M4-RAY canary had no client emitter**: `POST /api/canary/observe/CANARY-VC-M4-RAY`
   (route verified unauthenticated) now fired by a one-shot latch in
   `graph_scene.gd:_on_selection_made` for resolved non-origin agent handles.

## 4. Live backend verification (WS-A)

- Dev stack brought up from agentbox via host Docker daemon
  (`HOST_PROJECT_ROOT=/mnt/mldata/githubs/AR-AI-Knowledge-Graph`).
- `/wss` and `/ws/presence`: plain GET → 400, WebSocket upgrade → **101**,
  both in-container and from the LAN at `ws://192.168.2.132:4000` (bogus-path
  404 control). nginx `:3001` does **not** proxy `/ws/presence` — clients must
  use `:4000` directly.
- Dev build: `dev-auth` features + `ALLOW_INSECURE_DEFAULTS` → anonymous
  read-only `/wss`; `/ws/presence` uses post-upgrade challenge/response, any
  BIP-340 key.
- Canary registry live and armed (`GET /api/canary`).
- Close-out keypair (scratchpad only, never committed):
  pubkey `5133637906a8e05fd260845bb6b40a65bdc0c203df7dbcd6b3224b717ef6e6ce`.
  For the COM18 decide POST it must be added to `POWER_USER_PUBKEYS`.

## 5. Doc-drift findings

- `agentbox/xr-runtime/README` "no production network transport" finding is
  **stale**: `TungsteniteWsTransport`, `NostrSigner`/`NostrAuth`, tokio
  runtime, and the `connect_to_url`/`XR_BACKEND_WS` wiring all exist and are
  test-covered.
- `.github/workflows/xr-godot-ci.yml` (PRD-008 §5.7's 10-job pipeline)
  **does not exist** on this branch.
- P2-M2's "broker:new_case egress unshipped" is stale server-side (the
  broadcast path exists via `broker_events::broadcast_new_case`); the missing
  half was client-side forwarding (fixed above, §3.5).

## 6. Pending — live headset session (canary fires)

Blocked on operator actions as of this writing:

- [ ] Steam login on HP (cached token expired; password + Steam Guard for the
      registered account) — VNC tunnel or attached monitor/dummy plug.
- [ ] SteamVR install (appid 250820) + `~/.config/openxr/1/active_runtime.json`.
- [ ] XRBoot smoke on the VIVE (lighthouse tracking, stereo render).
- [ ] `CANARY-VC-M1-HUD` — in-headset DID badge render (+ verified flip).
- [ ] `CANARY-VC-M4-RAY` — controller/head-gaze ray resolves non-origin agent
      node (auto-fires via the new observe latch).
- [ ] `CANARY-VC-COM18-INTERV` — signed decide accepted by
      `POST /api/broker/cases/{id}/decide` (needs the pubkey allowlisted).
- [ ] Tier promotions `standalone → integrated` in the P2-M* evidence files,
      citing this record + ADR-136.

HP session-state changes to revert/keep at close-out: SDDM autologin
`User=john` added (backup at `/etc/sddm.conf.bak.wsb`); `~/godot43/`,
`~/visionclaw-xr/`, GUT 9.3 in `addons/gut/` (not committed upstream);
gdext debug→release symlink in `rust/target/`.
