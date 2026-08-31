# ADR-102: XR Client ↔ Backend Transport Completion (Graph V3 + Presence Handshake)

**Status:** Accepted
**Date:** 2026-06-08
**Deciders:** jjohare, VisionClaw XR/platform team
**Supersedes:** None
**Amends:** ADR-071 (corrects its "28 B/node" wire claim; see §6)
**Related:**
- ADR-071 (Godot 4 + godot-rust + OpenXR native APK — the runtime this transport plugs into)
- PRD-008 (XR client native replacement — §5 protocol surface)
- PRD-019 (XR transport completion — the product record this ADR decides)
- DDD `ddd-xr-godot-context.md` (BC22 — bounded-context model, updated alongside this ADR)
- `docs/reference/binary-protocol.md` (binary protocol reference — **now reconciled to V3 52 B**) / ADR-061 (original decision — still at 28 B/node by design; live wire is V3 52 B, see §2)
- ADR-031 (analytics extension — the change that took the graph wire from 36 B to 52 B/node)
- `src/utils/binary_protocol.rs` (authoritative server encoder), `src/actors/presence_actor.rs` (authoritative presence server)

## TL;DR

The Godot XR client (ADR-071) shipped with decoder, presence, interaction, LOD,
and avatar modules feature-complete **but never connected to the backend** — the
`connect_to_server` GDScript entry point was orphaned and the gdext clients' `poll()`
pumps were never driven, so the immersive scene rendered an empty world. This ADR
records the completion of the client↔backend transport: a tungstenite WebSocket
adapter + Nostr (BIP-340 schnorr) signer + tokio pump pushing `Send`-safe events
into an `Arc<Mutex<VecDeque>>` inbox drained on the scene-tree thread, env-driven
auto-connect in `_ready()`, and conformance to the **live Protocol V3 graph wire
(version byte `0x03` + N×52-byte records)** rather than the stale 28-byte spec in
ADR-061/`binary-protocol.md`. It also records that the presence sibling-broadcast
frame carries an opaque `local_id` with **no avatar URN**, leaving a `local_id ↔
avatar` mapping gap (§5) whose resolution requires an authoritative-server change
deferred to a follow-up under explicit approval.

## Context

ADR-071 chose Godot + gdext + OpenXR and PRD-008 tracked the client to
"feature-complete minus LiveKit AAR". The gdext crate registered five classes
(`BinaryProtocolClient`, `PresenceClientNode`, `XrInteraction`, `LodPolicy`,
`SpatialVoiceRouter`), each with comprehensive headless test coverage (the copresence layer later added
five more — `AgentAvatarNode`, `ProxemicsSolver`, `GazeTracker`,
`SelectionArbiterNode`, `NostrAuth` — per ADR-130 D4; ten register today). The gap
discovered on audit (2026-06-08):

1. **Transport was never wired.** `graph_scene.gd::connect_to_server(...)` existed
   but no caller invoked it, and `_physics_process` never called `BinaryProtocolClient.poll()`
   or `PresenceClientNode.poll()`. The network clients had no transport implementation
   behind their ports — only the test fakes. Result: the scene instantiated the
   classes, connected signals, and then sat idle. No user ever saw graph nodes or
   a remote avatar in the native client.

2. **The graph wire spec was stale.** ADR-071 and `binary-protocol.md` describe a
   28-byte/node frame with an `0x42` preamble and analytics carried out-of-band as
   a separate `analytics_update` JSON message. The live server
   (`src/utils/binary_protocol.rs::encode_node_data`) has emitted **Protocol V3**
   since the ADR-031 analytics extension: a leading version byte `0x03` followed by
   N fixed 52-byte records with the analytics inline (sssp distance/parent, cluster,
   anomaly, community, centrality). A decoder written to the documented 28-byte
   spec would desync on the first frame.

3. **The presence handshake direction was undocumented in client code.** The
   authoritative server (`presence_actor.rs`) is **server-initiated**: it sends a
   `challenge` and expects a signed `auth` before `joined`. Earlier client scaffolding
   assumed a client-initiated NIP-98 `join`.

4. **`Gd<T>` is not `Send`.** Any transport pump must run on tokio threads, but
   Godot objects and signal emission must stay on the scene-tree thread. The two
   cannot share the gdext class directly.

## Decision Drivers

- **Single authenticated socket per stream, matching the rest of the substrate.**
  Graph positions ride `/wss` (binary V3); presence rides `/ws/presence` (JSON
  handshake + binary pose). No third-party world server, no parallel identity model.
- **Thread-safety without `unsafe`.** The pump/inbox/poll split keeps every `Gd<T>`
  touch on the main thread; only `Send` plain-data events cross the boundary.
- **Conform to the live wire, not the documented one.** The client is a consumer;
  it must decode exactly what the server emits today.
- **Zero-config on device.** A Quest APK has no CLI; connection parameters come from
  the environment so the boot scene self-connects.
- **Preserve the hexagonal port seam.** Real transport is an adapter behind the same
  `WsTransport`/`Signer` ports the fakes implement, so the 130-test headless suite
  keeps running without a live backend or a Godot runtime.

## Considered Options

### Graph wire conformance

- **(chosen) Decode live Protocol V3.** Read version byte `0x03`, then `chunks_exact(52)`;
  consume the first 28 bytes of each record (id+pos+vel), ignore the 24-byte analytics
  tail (the XR renderer does not yet use sssp/cluster/centrality). Node id =
  `raw & 0x03FF_FFFF`; type flags in bits 26–31.
- **Rejected: decode the documented 28-byte frame.** Matches ADR-061/`binary-protocol.md`
  but not the live server — guaranteed desync.
- **Rejected: ask the server to emit a slim XR-only 28-byte stream.** Adds a server
  code path and a second wire format to maintain for a client that can cheaply skip
  24 bytes/record. Reconsider only if Quest bandwidth profiling shows the tail matters.

### Threading model for the gdext network clients

- **(chosen) tokio pump → `Arc<Mutex<VecDeque<Inbound>>>` inbox → `poll()` on main thread.**
  Inbound frames decoded off-thread into `Send` plain-data enums; `poll()` drains the
  queue each `_physics_process` tick and emits Godot signals. Outbound poses via
  `tokio::sync::mpsc::UnboundedSender`.
- **Rejected: block the main thread on socket reads.** Stalls the 90 Hz render loop.
- **Rejected: emit signals directly from the tokio task.** `Gd<T>` is not `Send`;
  unsound and rejected by gdext at compile time.

### Connection trigger

- **(chosen) env-driven auto-connect in `_ready()`.** `_connect_from_env()` reads
  `XR_BACKEND_WS` (default `ws://localhost:4000`, paths `/wss` and `/ws/presence`
  appended), `XR_ROOM_URN`, `XR_DISPLAY_NAME`, `XR_GRAPH_TOKEN`, `XR_NOSTR_SECRET`.
- **Rejected: HUD "Join" button only.** Leaves the default boot path empty; useful as
  an override, retained, but not the primary trigger.

## Decision

Implement the transport as adapters behind the existing ports and wire them into
the scene. Concrete deliverables in this repository (all under `xr-client/`):

**Graph stream (`/wss`, Protocol V3).** `src/binary_protocol.rs` decodes the live
wire: `PROTOCOL_V3 = 0x03`, `NODE_RECORD_BYTES = 52`, `NODE_ID_MASK = 0x03FF_FFFF`.
`BinaryProtocolClient` (gdext) drives a tungstenite reader on tokio, decodes frames
into `GraphInbound` events, and emits `position_updated` / `connection_changed` from
`poll()`. Control messages sent on connect: `{"type":"requestInitialData"}` then
`{"type":"subscribe_position_updates","data":{"interval":60,"binary":true}}`; auth via
`?token=` query.

**Presence (`/ws/presence`, server-initiated).** `src/presence.rs` implements the
challenge→auth→joined state machine. The signed `auth` is
`schnorr_sign(SHA256(nonce[32] || timestamp_us.to_le_bytes()[8]))` (secp256k1 BIP-340,
128-hex). `PresenceClientNode` (gdext) decodes inbound `0x43` **sibling** broadcast
frames (multi-avatar, see §5) and text room events (`avatar_joined`, `avatar_left`)
into `PresenceInbound`, emitting `avatar_joined` / `avatar_left` / `avatar_pose_updated`
/ `presence_kicked` from `poll()`. Outbound poses encode through the single-frame
`visionclaw-xr-presence::wire` codec (opcode `0x43`).

**Transport/signer adapters.** `src/transport.rs` (tungstenite `WsTransport`),
`src/signer.rs` (Nostr BIP-340 `Signer`), `src/runtime.rs` (shared tokio runtime).
The fakes in `ports/` remain the test substrate.

**Scene wiring.** `scripts/graph_scene.gd` connects `connection_changed`, calls
`_connect_from_env()` in `_ready()`, drives both clients' `poll()` first in
`_physics_process`, closes existing sockets before reconnect, and schedules
backoff reconnect on disconnect via `_on_connection_changed`.

**Class/method/signal contract verified** against `graph_scene.gd`: all five
registered class names, every called `#[func]`, and every connected `#[signal]`
match; the release cdylib exports `gdext_rust_init`.

## Consequences

### Positive

- The native XR client connects to the live backend and renders graph nodes and
  remote avatars for the first time. The immersive path is no longer silently empty.
- The graph decoder conforms to the wire the server actually emits; analytics-tail
  drift cannot desync it because record framing is length-checked (`chunks_exact(52)`).
- All `Gd<T>` access stays on the scene-tree thread; the network runs on tokio with
  no `unsafe` and no lock held across `.await`.
- The hexagonal seam is preserved: 130 headless tests (78 lib + 52 integration/property)
  pass with zero clippy errors and no live backend or Godot runtime required.
- Connection is zero-config on device via environment variables; the HUD Join path
  remains as an override.

### Negative

- The 24-byte analytics tail is decoded-and-discarded each frame. Negligible CPU,
  but ~46% of graph-stream bytes are unused by the XR renderer today. Acceptable;
  revisit if on-device bandwidth profiling flags it (see PRD-019 open items).
- ~~The `local_id ↔ avatar` mapping gap (§5) means inbound sibling poses cannot yet be
  attributed to a named avatar without a follow-up server change.~~ **Closed 2026-06-12
  (PRD-019 task #28); see the §5 update.**
- LiveKit Android AAR media transport remains unimplemented (PRD-008 §5.5); voice
  routing is in-memory only (`SpatialVoiceRouter`).

### Neutral

- `binary-protocol.md` and ADR-061 are not rewritten by this ADR; §2 records that the
  XR client conforms to the **live V3 wire**, and §6 amends ADR-071's stale claim.
  A separate docs-alignment pass should reconcile `binary-protocol.md` to V3.
  **Update: that pass has since landed — `docs/reference/binary-protocol.md` now
  documents the V2 36 B / V3 52 B position records (PRD-019 N2). ADR-061 itself is
  left at its original 28 B framing by design, as the historical decision record.**

## §2. Live graph wire (authoritative, as emitted by the server today)

Source of truth: `src/utils/binary_protocol.rs::encode_node_data` (V3 always).

```text
Frame:  [u8 version = 0x03] [ N × 52-byte node record ]

Node record (52 bytes, little-endian):
  @0   u32   node id      (bits 0–25 = id, bits 26–31 = type flags)
  @4   f32×3 position
  @16  f32×3 velocity
  @28  f32   sssp_distance   ─┐
  @32  i32   sssp_parent      │
  @36  u32   cluster_id       │ 24-byte analytics tail
  @40  f32   anomaly          │ (ADR-031; XR client decodes 0..28, skips tail)
  @44  u32   community_id     │
  @48  f32   centrality      ─┘

Type flags:  agent 0x8000_0000 (bit31) · knowledge 0x4000_0000 (bit30)
             ontology mask 0x1C00_0000 (class 0x0400_0000 bit26,
             individual 0x0800_0000 bit27, property 0x1000_0000 bit28)
Node id range: 0 .. 2^26-1 (67,108,863).  NODE_ID_MASK = 0x03FF_FFFF.
```

The 28-byte/`0x42` layout in `binary-protocol.md` describes the **server-internal
`BinaryNodeData` struct**, not the wire. The wire has been V3/52-byte since ADR-031.

> **Amendment (2026-08-30, ADR-137).** The position stream now also emits a
> **Protocol V5** wrapper: `[u8 version = 0x05][u64 broadcast_seq LE][ V3 body ]`,
> i.e. a V3 frame (`0x03` + N×52-byte records) prefixed with an 8-byte broadcast
> sequence number. The client decodes it by skipping the 8-byte sequence and
> decoding the V3 body unchanged (`xr-client/rust/src/binary_protocol.rs:23-26,
> 292-309`; `PROTOCOL_V5 = 0x05`, `V5_SEQ_BYTES = 8`). V5 is additive — a V3-only
> decoder still works against a V3 stream. See `docs/reference/binary-protocol.md`.

## §5. Presence sibling-broadcast frame and the `local_id ↔ avatar` gap

> **Update (2026-06-12 — PRD-019 task #28 shipped).** The *eager assignment +
> announce* resolution recommended at the foot of this section was implemented.
> `handle_join` now calls `local_id_for(&avatar_id)` and includes the resulting
> `local_id` in both the `JoinAck` member snapshots (`presence_actor.rs:429-441`)
> and the `AvatarJoined` event (`presence_actor.rs:59,126`); the client builds
> `local_to_avatar` from the JSON path (`xr-client/rust/src/presence.rs`) and applies
> it to binary frames. **This §5 gap and its Negative consequence are closed.** The
> description below is retained as the record of the gap as found on 2026-06-08.

Server→client multi-avatar pose frame (`presence_actor.rs:31`, `broadcast`):

```text
[u8 0x43][u64 broadcast_seq LE][u32 room_id_u32 LE][u16 user_count LE]
  per user: [u32 local_id][u64 timestamp_us][u8 mask][ 28 × popcount(mask) transforms ]
mask: bit0 head (always) · bit1 left hand · bit2 right hand
```

The frame carries a server-assigned **opaque `local_id`** per user and **no avatar
URN**. `local_id` is minted lazily by `local_id_for()` on a user's *first* pose
broadcast (`next_local_id` starts at 1, `wrapping_add`), stored only in the server's
`avatar_id_to_local` map, and **never announced** to clients. The identity-bearing
messages — the `joined` ack `members` (`AvatarMetadata`: did, display_name, model_uri)
and the `AvatarJoined` event (avatar_id, did, display_name) — **omit `local_id`**.

Consequence: the client receives avatar identities over JSON and pose data keyed by
`local_id` over binary, with **no message linking the two**. `MemberDescriptor.local_id`
is modelled as `Option<u32>` (always `None` today) and `PresenceClientNode`'s
`local_to_avatar` map stays empty. Inbound sibling poses cannot be attributed to a
named avatar.

**Recommended resolution (deferred — requires authoritative-server change, explicit
approval):** *eager assignment + announce.* In `handle_join`, call
`local_id_for(&avatar_id)` and include the resulting `local_id` in both the `JoinAck`
member descriptors and the `AvatarJoined` event. The client then builds
`local_to_avatar` from the JSON path and applies it to binary frames. This is cheaper
on the wire than embedding the ~50-byte URN in every per-user pose record and keeps the
4-byte `local_id` as the hot-path key. Tracked as task #28; **not implemented in this
ADR** because it edits `src/actors/presence_actor.rs`.

## §6. Amendment to ADR-071

ADR-071 states the XR client "reus[es] the existing **28 B/node** binary protocol
(ADR-061)" and cites "28 B/node" in its Decision Drivers and Consequences. This is
**superseded by §2**: the live graph wire is **Protocol V3, a `0x03` version byte
followed by 52-byte records** (position+velocity in the first 28 bytes, analytics in
the trailing 24). The XR client conforms to the live V3 wire and decodes the first 28
bytes of each record. ADR-071's decision (Godot + gdext + OpenXR) is unchanged; only
its wire-size claim is corrected here.

## References

- Authoritative server: `src/utils/binary_protocol.rs`, `src/actors/presence_actor.rs`
- XR client: `xr-client/rust/src/{binary_protocol,presence,transport,signer,runtime}.rs`,
  `xr-client/scripts/graph_scene.gd`
- Presence crate: `crates/visionclaw-xr-presence/` (single-frame `wire` codec, opcode `0x43`)
- ADR-071 (runtime decision), ADR-061 (binary protocol — stale at 28 B), ADR-031
  (analytics extension — V3 52 B), PRD-008 (§5 protocol), PRD-019 (transport completion)
