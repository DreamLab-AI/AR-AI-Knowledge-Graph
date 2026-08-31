---
id: ADR-2018
title: The graph frame is a frozen 52-byte inline-analytics V3 record wrapped additively by the V5 sequence envelope
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/utils/binary_protocol.rs, xr-client/rust/src/binary_protocol.rs]
owner: jjohare
review_trigger: a new GPU analytics field that cannot fit an existing slot, or any need to change the 52-byte node-record layout
repo: visionclaw
domain: PROTOCOL-registry
lineage: "Retires legacy ADR-061 D1/D2 (28B forever + separate JSON analytics channel); ADR-031 (36->52B tail); ADR-102 §2 (shipped 52B WireNodeDataItemV3); ADR-137 §5 (V5 wrapper)."
---

# ADR-2018 — The graph frame is a frozen 52-byte inline-analytics V3 record wrapped additively by the V5 sequence envelope

## Context

GPU physics produces sticky per-node analytics (sssp distance/parent, cluster,
anomaly, community, centrality) every frame. The legacy plan (ADR-061 D1/D2)
kept a 28-byte record forever and shipped analytics out-of-band as JSON,
splitting one truth across two channels that could disagree. V3 instead folded
the analytics inline; the record grew 36->52 bytes (ADR-031, ADR-102 §2). V5
(ADR-137 §5) then needed monotonic ordering for delta/full broadcast dedup
without disturbing the byte layout every client already parses.

## Decision

The node record is `WireNodeDataItemV3`: a fixed **52-byte** frame carrying
nine fields — `id`/`position`/`velocity` plus the six sticky analytics values
(sssp distance/parent, cluster, anomaly, community, centrality) — and
`WIRE_V3_ITEM_SIZE == 52` is a hard,
test-asserted invariant. V5 is purely additive: `[0x05][u64 broadcast_seq
LE][52B/node V3 body]` — the decoder skips the 8-byte sequence and delegates to
the unchanged V3 codec. The V3 body is therefore **frozen**: no field may be
resized, reordered, or repurposed. New analytics extend the registry only via a
**new tag with a new documented length**, never by mutating the 52-byte record.
This forecloses silent layout drift and the two-channel analytics split.

## Consequences

- One wire truth: analytics and positions arrive in the same record, so they
  cannot desynchronise, and the size assertion fails the build loudly if anyone
  edits the struct without updating the invariant.
- The 52-byte record is a fixed budget: a tenth analytic that will not fit an
  existing slot forces a new tag/codec rather than a cheap in-place edit — the
  intended cost of freezing.
- V5's 8 bytes/frame overhead buys ordering; encoders and decoders must agree
  the sequence is skipped, not interpreted, before the V3 body.

## Verification

At e0f8cd896: `src/utils/binary_protocol.rs` — `WireNodeDataItemV3` carries
`sssp_distance/sssp_parent/cluster_id/anomaly_score/community_id/centrality`
(layout comment lines 38-51); `test_wire_format_size` asserts
`WIRE_V3_ITEM_SIZE == 52`. The decoder `match protocol_version` routes `5 =>`
through an 8-byte skip then `decode_node_data_v3(&payload[8..])`. Client
mirror `xr-client/rust/src/binary_protocol.rs` fixes `NODE_RECORD_BYTES = 52`,
`PROTOCOL_V5 = 0x05`, `V5_SEQ_BYTES = 8`.
