# PRD-019: XR Transport Completion — Connecting the Native Client to the Live Backend

**Status:** Delivered and extended (transport wired; `local_id ↔ avatar` mapping (#28) shipped server+client; analytics-tail rendering, edge topology, server-authoritative drag, haptics, importance-capped LOD, HUD mount and unbounded-backoff reconnect added 2026-06-12; 141 headless tests green) — on-device validation and LiveKit AAR remain open
**Priority:** P0 — the native XR client (PRD-008 / ADR-071) was feature-complete but unconnected; the immersive path was still unshipped in practice
**Date:** 2026-06-08
**Author:** XR transport completion pass (build-with-quality)
**Decision record:** [ADR-102 — XR Client ↔ Backend Transport Completion](../adr/ADR-102-xr-client-backend-transport-completion.md)
**Related:**
- [PRD-008 — XR client native replacement](PRD-008-xr-godot-replacement.md) (§5 protocol surface; this PRD closes the §5 transport gap)
- [ADR-071 — Godot 4 + godot-rust + OpenXR native APK](../adr/ADR-071-godot-rust-xr-replacement.md) (runtime; wire-size claim amended by ADR-102 §6)
- PRD-QE-002 — XR Godot quality engineering (test strategy this pass extends)
- [`ddd-xr-godot-context.md`](../ddd/ddd-xr-godot-context.md) (BC22 bounded context — updated alongside)
- [`binary-protocol.md`](../reference/binary-protocol.md) (binary protocol reference — **reconciled to V3 52 B**, documents V2 36 B / V3 52 B position records; N2 docs-alignment effectively complete) / ADR-061 (original decision — still describes the 28 B/node layout, by design)

---

## 1. Problem Statement

PRD-008 and ADR-071 delivered a Godot 4.3 + gdext native XR client with five
registered gdext classes, a presence crate, and a 130-test headless suite — and
marked it "feature-complete minus LiveKit AAR". An audit on 2026-06-08 found the
client **never connected to the backend**:

| # | Finding | Impact |
|---|---------|--------|
| 1 | `graph_scene.gd::connect_to_server(...)` was orphaned — no caller — and `_physics_process` never drove `BinaryProtocolClient.poll()` / `PresenceClientNode.poll()`. | The scene instantiated classes and wired signals, then sat idle. Graph nodes and remote avatars never rendered. The immersive path was effectively unshipped. |
| 2 | The graph decoder targeted the **documented** 28-byte/`0x42` frame; the live server emits **Protocol V3** (`0x03` + 52-byte records, analytics inline) since ADR-031. | A decoder built to the doc would desync on the first frame. |
| 3 | No real transport sat behind the `WsTransport`/`Signer` ports — only test fakes. | Nothing could open a socket; the network layer was untested against any wire. |
| 4 | Presence client scaffolding assumed a client-initiated handshake; the live server is **server-initiated** (`challenge`→`auth`→`joined`). | The handshake would never complete against the real server. |
| 5 | The `0x43` presence sibling frame carries an opaque `local_id` and no avatar URN; no message maps `local_id ↔ avatar`. | Inbound remote poses cannot be attributed to a named avatar (carried forward as an open item). |

Summary: **the most expensive part of the XR work — protocol, presence, rendering,
interaction — was done, but the wire that makes it visible was never connected.**

## 2. Goals / Non-Goals

**Goals.**
- G1. Connect the native client to the live backend over two authenticated sockets:
  `/wss` (graph, binary V3) and `/ws/presence` (JSON handshake + binary pose).
- G2. Decode the **live** Protocol V3 graph wire correctly (version `0x03`, 52-byte
  records, 26-bit node ids, type flags).
- G3. Complete the server-initiated presence handshake with BIP-340 schnorr auth.
- G4. Keep all `Gd<T>` access on the scene-tree thread; run the network on tokio.
- G5. Auto-connect on device from environment variables (no CLI on a Quest APK).
- G6. Preserve the hexagonal port seam so the headless test suite runs with no live
  backend and no Godot runtime.
- G7. Remove legacy/dead/placeholder code and reach zero clippy errors.

**Non-Goals.**
- N1. LiveKit Android AAR media transport (PRD-008 §5.5) — voice stays in-memory.
- N2. Rewriting `binary-protocol.md`/ADR-061 to V3 (separate docs-alignment pass).
- N3. Editing the authoritative presence server for the `local_id` gap (needs
  explicit approval; see §6 / task #28).
- N4. On-device Quest 3 profiling and soak (PRD-QE-002 nightly path).

## 3. Requirements

### 3.1 Graph transport (`/wss`)
- R1. Open a tungstenite WebSocket to `${XR_BACKEND_WS}/wss`, auth via `?token=`.
- R2. On connect, send `{"type":"requestInitialData"}` then
  `{"type":"subscribe_position_updates","data":{"interval":60,"binary":true}}`.
- R3. Decode V3 frames (`0x03` + N×52 B); emit `position_updated` per node from
  `poll()`; node id = `raw & 0x03FF_FFFF`, type from flag bits 26–31.
- R4. Reject malformed frames (wrong version, mis-aligned length) without panicking.

### 3.2 Presence transport (`/ws/presence`)
- R5. Complete `challenge`→`auth`→`joined`; sign
  `SHA256(nonce[32] || timestamp_us.to_le_bytes())` with BIP-340 schnorr (128-hex).
- R6. Decode inbound `0x43` sibling frames and `avatar_joined`/`avatar_left` events;
  emit `avatar_joined`/`avatar_left`/`avatar_pose_updated`/`presence_kicked` from `poll()`.
- R7. Encode outbound poses via the single-frame `wire` codec (opcode `0x43`).
- R8. `send_pose` before a completed handshake returns `PresenceError::Protocol`;
  server close during init returns `PresenceError::Rejected`.

### 3.3 Runtime & wiring
- R9. Network pumps run on a shared tokio runtime; inbound events cross to the main
  thread as `Send` plain-data enums via `Arc<Mutex<VecDeque>>`; no lock held across `.await`.
- R10. `graph_scene.gd` auto-connects in `_ready()` from `XR_BACKEND_WS`,
  `XR_ROOM_URN`, `XR_DISPLAY_NAME`, `XR_GRAPH_TOKEN`, `XR_NOSTR_SECRET`; drives both
  `poll()` calls each `_physics_process`; reconnects with backoff on disconnect.
- R11. Every gdext class name, `#[func]`, and `#[signal]` referenced by `graph_scene.gd`
  must exist; the release cdylib must export `gdext_rust_init`.

### 3.4 Quality
- R12. `cargo test` green; `cargo clippy --all-targets` free of own-code errors
  (godot macro `result_large_err` warnings are macro-generated and excepted).
- R13. No placeholder/stub/mock language in shipped source.

## 4. Acceptance Criteria & Evidence (2026-06-08)

| Req | Criterion | Status | Evidence |
|-----|-----------|--------|----------|
| G2/R3 | Decoder matches authoritative server encoder | ✅ | `xr-client/.../binary_protocol.rs` constants (`0x03`, 52, `0x03FF_FFFF`) == `src/utils/binary_protocol.rs` V3 layout |
| G3/R5 | Server-initiated handshake completes | ✅ | `presence_handshake.rs` 4/4 async tests pass |
| G4/R9 | No `Gd<T>` off main thread; no lock-across-await | ✅ | `cargo clippy` clears `await_holding_lock`; pump/inbox/poll split |
| G1/R10/R11 | Scene wired, contract intact | ✅ | All 5 class names + every method + every signal in `graph_scene.gd` resolve; `gdext_rust_init` exported (`nm -D`) |
| G6/R12 | Headless suite green, clippy clean | ✅ | 130 tests pass (78 lib + 52 integration/property); only godot macro warnings remain |
| G7/R13 | Legacy removed | ✅ | `webrtc_audio` error/log/doc de-stubbed; broken `tick_wraps_u32_max` test rewritten; `FRAC_1_SQRT_2` replaces magic floats |
| — | On-device connect to live backend renders nodes+avatars | ⏳ | Requires sidecar rebuild + Quest session (task #27 host leg) |
| N3 | `local_id ↔ avatar` attribution | ❌ open | ADR-102 §5; needs server change (task #28) |
| N1 | LiveKit AAR voice media | ❌ open | PRD-008 §5.5 |

## 5. Architecture (delivered)

```text
 Godot scene-tree thread                         tokio runtime
 ───────────────────────                         ─────────────
 graph_scene.gd
   _ready()  ──► connect_to_server(env)
   _physics_process():
     BinaryProtocolClient.poll() ◄── drain ── Arc<Mutex<VecDeque<GraphInbound>>> ◄── /wss reader (tungstenite)
       └► emit position_updated / connection_changed
     PresenceClientNode.poll()   ◄── drain ── Arc<Mutex<VecDeque<PresenceInbound>>> ◄── /ws/presence reader
       └► emit avatar_joined/left/pose_updated/kicked
     send_pose ──► UnboundedSender<PoseFrame> ──────────────────────────────────► /ws/presence writer
 ports/ (WsTransport, Signer) ── real adapters: transport.rs, signer.rs, runtime.rs
                              └─ test fakes: FakeWsTransport, FakeSigner (headless suite)
```

## 6. Open Items / Follow-ups

- **#27 (host leg):** rebuild the xr-runtime sidecar, restart, confirm the extension
  loads and a live session renders nodes + a remote avatar. No commits/push until the
  user confirms.
- ~~**#28 (`local_id ↔ avatar`)**~~ — **DELIVERED** (approved 2026-06-12): eager
  `local_id` assignment at join, announced in `JoinAck.members` (`MemberSnapshot`)
  and the `AvatarJoined` event; client builds `local_to_avatar` from the JSON path
  and attributes binary sibling poses to named avatars.
- **LiveKit AAR:** PRD-008 §5.5 media transport.
- ~~**Bandwidth (analytics tail)**~~ — **RESOLVED differently** (2026-06-12): rather
  than negotiating a slim stream, the XR client now *uses* the tail — community
  drives node colour, centrality drives size and the importance-capped instance
  budget, anomaly tints toward warning red. The tail is no longer dead weight.
- ~~**Docs alignment**~~ — **DONE** (2026-06-12): `binary-protocol.md` rewritten to
  the live V3 52-byte wire.

### Feature expansion delivered 2026-06-12 (audit follow-up)

- **Analytics-aware rendering:** V3 tail decoded into `NodeUpdate`; quantised
  `VisualsKey` change-detection emits `node_visuals_updated` only when a node's
  visual identity changes (near-zero steady-state signal traffic).
- **Edge topology:** `/wss` `initialGraphLoad` text frames are parsed
  (`parse_initial_graph_load`); `topology_updated` + `get_edges()`/
  `get_edge_weights()` feed an instanced edge MultiMesh, weight-capped to the
  Quest budget (3,000 instances).
- **Server-authoritative drag (multi-client):** controller trigger feeds
  `XrInteraction::evaluate_ray`; grab sends the desktop's `node_drag_start/
  update/end` protocol over `/wss` so the server pins the node and every
  connected client sees the manipulation. The `/wss` session is NIP-98
  authenticated (kind-27235 event signed with the same Nostr identity as
  presence) to satisfy the server's drag auth gate.
- **Importance-capped node LOD:** `LodPolicy::visible_subset` keeps the
  highest-centrality 4,000 nodes when the graph exceeds the Quest budget.
- **Haptics:** target-acquire and grab pulses on the active controller.
- **HUD mounted:** `HUD.tscn` instanced under the XR camera; connection
  status, avatar count and room-join wired.
- **Reconnect:** unbounded with exponential backoff (2 s → 60 s cap) so a
  sleeping headset rejoins whenever it wakes.

## 7. Rollback

All work is local on `main`, uncommitted. The XR transport is additive (new adapter
modules + scene wiring); reverting the `graph_scene.gd` `_ready()` auto-connect and the
`poll()` drives returns the client to its prior idle-but-building state. No server or
desktop-client behaviour changes in this PRD.
