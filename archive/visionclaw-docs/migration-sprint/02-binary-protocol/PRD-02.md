# PRD-02 — Binary Protocol & Broadcast Pipeline

## 1. Capability statement

VisionClaw transmits node positions from server to client over a WebSocket
binary channel at a cadence governed by GPU layout settlement, with
guaranteed eventual full-state delivery so that a client connecting at any
time sees the current positions within a known maximum latency.

## 2. Why this exists

The freeze regression that triggered the sprint was traced primarily to
the broadcast layer:

- Server emitting position frames at 60Hz saturated the Comlink IPC at the
  client (4.77 MB/s for 4,519 nodes × 28 bytes × 60 frames/s).
- `BroadcastOptimizer` delta-filter withheld frames once nodes converged,
  starving polling-based catch-up paths of any positions at all.
- V4 delta protocol was added then rejected by the client (`8a7610a00`,
  `1d0b29468`) — the client only understood V3 full-sync.
- Auth-gated binary endpoints (`?skipAuth=true`) interacted with
  `nostrAuth.isAuthenticated()` in ways that masked silent failures.

The capability we want is *settlement-aware, full-sync, backpressure-friendly*
position transport. We do not want deltas, and we do not want fixed-FPS.

## 3. Users and use cases

- **Client connecting cold**: receives full-state position frame within
  500ms of WebSocket handshake.
- **Client connecting late** (after layout settle): receives a full
  position frame within the heartbeat window (≤5s).
- **Client during active layout**: receives position frames at the rate
  the GPU produces meaningful change, up to a max of 10 Hz.
- **Client through pan / zoom / settings**: receives no extra position
  traffic; client-side camera state never reaches the server.

## 4. Acceptance criteria

A1. **V3 full-sync only.** The protocol carries one wire format: V3 full
    position frames. Each frame contains `(u32 node_id, f32 x, f32 y, f32 z,
    f32 vx, f32 vy, f32 vz)` per included node. V4 delta encoding is removed.

A2. **Settlement-gated cadence.** While `LayoutDestabilised` is the most
    recent physics event, frames emit at up to 10 Hz. Once `LayoutSettled`
    fires, frames drop to the broadcast-owned heartbeat cadence
    (`broadcast_heartbeat_secs`, default 5 s; see ADR-02 D2 and A8).

A3. **Backpressure-honouring.** If the WebSocket buffer is above 64KB on a
    given client, the broadcast layer drops the *current* frame for that
    client (never queues). The next frame attempt re-evaluates.

A4. **Full-state guarantee.** Every emitted frame is full-state (all
    nodes). A client never has to maintain a delta history. Clients that
    request "current positions" via REST receive the same wire format
    payload as the WebSocket broadcast.

A5. **Single broadcast path.** The polling REST endpoint
    (`GET /graph/positions`) reads from the same `GraphStateActor` snapshot
    as the WebSocket broadcast. They cannot diverge.

A6. **Bounded latency.** Time from the broadcast actor's heartbeat
    `tokio::time::interval` tick firing in SETTLED state to the
    corresponding position frame appearing on the WebSocket wire is
    ≤50ms p99.

A7. **Auth interaction**: in `?skipAuth=true` mode the WebSocket accepts
    without Nostr token. Production mode requires Nostr token in the
    upgrade handshake. There is no third mode.

A8. **Heartbeat is wall-clock, not iteration-driven.** In the SETTLED
    state, full-position frames emit every `broadcast_heartbeat_secs` of
    wall-clock time (default 5 s), independent of physics tick rate.
    Verified by a test that pauses physics entirely and observes a
    heartbeat within 5.5 s.

## 5. Non-goals

- Delta encoding. Settled outright: rejected as a complication that has
  caused two regressions already.
- Quic / WebTransport. WebSocket remains the transport.
- Per-edge or per-label updates. This protocol is positions only. Edge
  topology changes go through the REST API and trigger a fresh layout.

## 6. Wire format

```
Frame header (8 bytes):
    u32 magic       = 0xV3F0  (V3 Full)
    u32 frame_id    (monotonic per connection, wraps at u32::MAX)

Per node (28 bytes):
    u32 node_id     (full id including class flag bits)
    f32 pos_x
    f32 pos_y
    f32 pos_z
    f32 vel_x
    f32 vel_y
    f32 vel_z

Frame trailer (4 bytes):
    u32 node_count  (count of nodes in this frame)
```

For 5k nodes: header + 5000*28 + trailer = 140,012 bytes per frame.
At 10 Hz settled = 0 frames (heartbeat is 0.2 Hz = 1 frame / 5s).
At 10 Hz destabilised = 1.4 MB/s peak — well within capacity.

## 7. Out-of-scope smells flagged for ADR review

- `c09f8725a fix: eliminate client freeze on 4.5k-node graphs via
  comms-rate fix` — the underlying bug was the absence of A2's settlement
  gating. ADR must adopt A2, not the constant-1-FPS workaround.
- Vestigial `broadcast_interval` fields in `client_coordinator_actor.rs`
  that handlers don't consult. Remove during migration.
- `8a7610a00 fix: add V4 to valid binary protocol versions` and the
  subsequent `1d0b29468 fix: force V3 full sync every frame` — V4 was
  never functional and is dropped entirely.
