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

## Closeout extension — 2026-09-04

Work package: **CP-01/06**. Owner remains `jjohare`, with protocol, identity and XR maintainers responsible for their respective boundaries.

The XR decoder skips V5 sequence bytes; a sequence envelope alone does not establish consumer ordering. All 218 XR Rust library tests pass, including the sequence-skip test.

**Acceptance condition:** Define and test consumer freshness across full/delta production and reconnects, using decreasing/duplicate sequences and a shared server/client fixture. Preserve the 52-byte record.

Dependencies: CP-01 release identity and CP-04 authority where authenticated actions cross the wire. Reopen on the existing review trigger, a changed opcode or a failing freshness/visibility probe. Existing verification and activation fields retain their historical scope; this annex records source/local tests at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`, not a new live certification.

See [rendered-state review](../../../VisionFlow/docs/estate-review/rendered-state.md) and [receipt](../../../VisionFlow/docs/estate-review/evidence/xr-render-snapshot.json).

## Acceptance progress — 2026-09-05

**Implemented.** Consumer freshness now exists as an enforced contract rather than
an envelope field the consumer discarded.

- `decode_position_frame_with_sequence` returns the V5 broadcast sequence
  alongside the records; `decode_position_frame` is retained as a
  sequence-discarding wrapper. A V3 frame yields `None` — it makes no ordering
  claim at all, and that is now explicit in the type rather than implicit in a
  skipped read.
- `FreshnessGate` implements the ordering contract: increasing sequence accepted;
  equal rejected as duplicate; lower rejected as stale; unsequenced V3 accepted
  but never moving the watermark; after `reconnect()` only a `Full` frame may
  re-baseline (a producer restart legitimately moves the sequence backwards),
  and a `Delta` arriving before that snapshot is refused for want of a baseline.
  `admit_frame` decodes and gates in one step, withholding records on rejection
  so a caller cannot apply a frame the gate refused.
- Wired live: `BinaryProtocolClient` gates every inbound position frame,
  arms a resync on `Disconnected`, and exposes `freshness_counters()`
  (`[stale, duplicate, awaiting_resync, resyncs]`) so the ordering problem is
  observable on a deployment rather than only in tests.
- The 52-byte record is untouched; a fixture test asserts the frozen size and
  that the V5 envelope is purely additive over the V3 body.

**Shared server/client fixture.** `crates/visionclaw-protocol/src/wire_fixtures.rs`
is the single source of the frame bytes. It is dependency-free, so the
deliberately isolated `xr-client/rust` workspace includes the *same source file*
by `#[path]` rather than taking on the domain crate's tree. A server-side test
pins those fixtures to the real encoder byte for byte, so the fixture is a pinned
rendering of the producer, not a second implementation of the protocol.

**Tests run.**

- `cargo test --lib` and `cargo test` in `xr-client/rust` — 227 lib + 75
  integration pass, including 23 in `tests/wire_freshness_and_frame_policy.rs`
  (decreasing, duplicate, full/delta interleaving, reconnect resync, delta before
  resync, unsequenced V3, frozen record size).
- `cargo test --lib --no-default-features adr_20` in the server crate — 34 pass,
  including the fixture/encoder equivalence and V5-additivity checks.

**Governed paths changed.** `xr-client/rust/src/binary_protocol.rs`,
`xr-client/rust/tests/wire_freshness_and_frame_policy.rs`,
`crates/visionclaw-protocol/src/wire_fixtures.rs`,
`crates/visionclaw-protocol/src/lib.rs`, `src/utils/binary_protocol.rs` (tests).

**Open.** The gate is exercised against fixtures and in unit tests, not against a
live concurrent full/delta producer pair or a real reconnect to a restarted
server. `implementation_status` is unchanged: the ordering contract is
implemented and tested at the consumer, but end-to-end certification against live
production remains outstanding.
