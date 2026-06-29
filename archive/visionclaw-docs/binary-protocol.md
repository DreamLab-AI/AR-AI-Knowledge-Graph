---
title: The Binary Protocol
description: Single-source spec for the GPU position stream wire format (Protocol V3)
category: reference
tags: [websocket, binary, protocol, real-time]
updated-date: 2026-06-12
---

# The Binary Protocol (V3)

Single per-physics-tick wire format used by the GPU position stream on
`/wss`. The live wire has been **Protocol V3** since the ADR-031 analytics
extension: a leading version byte followed by fixed 52-byte node records
with the analytics inline.

> Authoritative encoder: `src/utils/binary_protocol.rs` (V3 always).
> Wire decision record: [ADR-102 §2](adr/ADR-102-xr-client-backend-transport-completion.md)
> (which also amends ADR-071's stale 28-byte claim).
> Historical: [ADR-061](adr/ADR-061-binary-protocol-unification.md) /
> [PRD-007](PRD-007-binary-protocol-unification.md) describe the pre-ADR-031
> 28-byte layout — that layout survives only as the server-internal
> `BinaryNodeData` struct, **not** the wire.

## Frame layout

```
[u8 version = 0x03]
[N × 52-byte node record]
```

Receivers MUST dispatch on the version byte and reject unknown versions;
record framing is length-checked (`payload % 52 == 0`).

## Node record (52 bytes, little-endian)

```
@0   u32   node id        (bits 0–25 = id, bits 26–31 = type flags)
@4   f32×3 position
@16  f32×3 velocity
@28  f32   sssp_distance  ─┐
@32  i32   sssp_parent     │
@36  u32   cluster_id      │  24-byte analytics tail (ADR-031)
@40  f32   anomaly         │
@44  u32   community_id    │
@48  f32   centrality     ─┘
```

Type flags: agent `0x8000_0000` (bit 31) · knowledge `0x4000_0000` (bit 30) ·
ontology mask `0x1C00_0000` (class bit 26, individual bit 27, property bit 28).
Node id range `0..2^26-1`; `NODE_ID_MASK = 0x03FF_FFFF`.

## Cadence

- **Position stream**: per broadcast interval (clients subscribe with
  `subscribe_position_updates`, typical interval 60–200 ms). Each record
  carries pos+vel **and** the analytics tail; there is no separate
  per-frame analytics message.
- Analytics values change at recompute cadence (~0.1–1 Hz); consumers should
  treat the tail as slowly-varying (the XR client quantises it into a
  visual key and reacts only to changes).

## Consumers

- **Desktop client**: `client/src/store/websocket/binaryProtocol.ts`
  (52-byte stride, analytics into `analyticsBuffer`).
- **XR client**: `xr-client/rust/src/binary_protocol.rs`
  (`PROTOCOL_V3`, `NODE_RECORD_BYTES = 52`; community/centrality/anomaly drive
  colour, size and importance-capped LOD).

## Privacy

Visibility is enforced at the broadcast boundary
(`ClientCoordinator::broadcast_with_filter`); positions for nodes the
caller cannot see are dropped from the frame.

## Forbidden patterns (regression guards)

- Adding or reordering record fields without an ADR superseding ADR-031/ADR-102.
- Encoding session-static state in id flag bits beyond the defined type flags.
- Shipping a decoder written to the historical 28-byte layout — it desyncs on
  the first live frame (this is exactly the bug ADR-102 records).
