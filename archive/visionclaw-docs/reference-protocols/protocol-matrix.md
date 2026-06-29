# VisionClaw Protocol Matrix

> **Wire-format clauses superseded by [ADR-061](../../adr/ADR-061-binary-protocol-unification.md) (2026-04-30).**
> The current single-source spec is **[docs/binary-protocol.md](../../binary-protocol.md)**:
> one binary protocol, 24 bytes/node fixed, no versions, no flag bits.
> The matrix below is preserved as historical reference.

## Historical context

## Transport Protocols

| Transport | Protocol | Format | Use Case |
|-----------|----------|--------|----------|
| WebSocket | Binary protocol | 24 bytes/node + 9-byte header | Real-time physics updates (post-ADR-061) |
| QUIC | Postcard | Variable | High-throughput analytics batch |
| HTTP/LDP | JSON-LD | Semantic | Solid integration, crawler access |

## Binary wire format (historical, pre-ADR-061)

- Header: 1 byte (legacy version field; replaced by 0x42 preamble under ADR-061)
- Per-node payload: 48 bytes
  - node_id: u32 (4 bytes)
  - x, y, z: f32 (12 bytes)
  - vx, vy, vz: f32 (12 bytes)
  - sssp_distance: f32 (4 bytes)
  - sssp_parent: i32 (4 bytes)
  - cluster_id: u32 (4 bytes)
  - anomaly_score: f32 (4 bytes)
  - community_id: u32 (4 bytes)

## Node Type Flags (bits 26-31 of node_id)

| Bit | Flag | Description |
|-----|------|-------------|
| 31 | Agent | Agent node (0x80000000) |
| 30 | Knowledge | Knowledge node (0x40000000) |
| 28 | Property | Ontology property (0x10000000) |
| 27 | Individual | Ontology individual (0x08000000) |
| 26 | Class | Ontology class (0x04000000) |

## Deprecated

- **V1**: REMOVED (truncated IDs >16383, causing collisions)
- **V2**: Supported but not preferred (36 bytes/node, no analytics)

## Content Negotiation

JSS handles format conversion automatically:
- `Accept: application/ld+json` -> Native JSON-LD
- `Accept: text/turtle` -> Converted on demand
- `Accept: text/html` -> React SPA

## Wire-byte detection (historical, pre-ADR-061)

The first byte of every binary message historically indicated a wire variant:
- `0x01` = V1 (legacy, 34 bytes/node)
- `0x02` = V2 (stable, 36 bytes/node)
- `0x03` = V3 (48 bytes/node)

Under [ADR-061](../../adr/ADR-061-binary-protocol-unification.md) the byte is
replaced by a fixed `0x42` preamble — a sanity check, not a version dispatch.
There is one binary protocol; there are no versions. See
[docs/binary-protocol.md](../../binary-protocol.md).
