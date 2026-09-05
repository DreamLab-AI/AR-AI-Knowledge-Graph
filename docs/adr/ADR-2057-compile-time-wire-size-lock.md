---
id: ADR-2057
title: Lock the V3 record and V5 envelope widths at compile time
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: Any change to a WIRE_*_SIZE constant, the WireNodeDataItemV3 field set, or the V5 envelope layout
repo: visionclaw
---

# ADR-2057 — Lock the V3 record and V5 envelope widths at compile time

## Context

The 52-byte V3 record is the wire contract four shipping clients speak. Its width was
enforced only by `assert_eq!`s inside `#[cfg(test)] mod tests`
(`src/utils/binary_protocol.rs:1052,1149`), so the invariant held only when the suite ran —
a change to any `WIRE_*_SIZE` constant could reach a release build unnoticed. The sibling
`SimParams` ABI already had the stronger form: a `const _: () = assert!(...)` at
`src/models/simulation_params.rs:228` plus a CUDA `static_assert` (ADR-2028).
The V5 envelope `[0x05][u64 broadcast_seq LE][V3 body]` had no owning ADR at all
(PROTOCOL-registry.md declared itself owner) and its 8-byte sequence width was an
unnamed literal `8` in the decode branch. Diagram VC-14.1 recorded the weak assertion as
a DIVERGENCE; VC-14.4 recorded the V5 dispatch.

## Decision

The V3 record width and the V5 sequence-prefix width are compile-time constants, asserted
with `const _: () = assert!(...)` in `src/utils/binary_protocol.rs`. The test-time
assertions remain as a second, redundant check; they are not the primary guard.
The V5 tag gains a named constant `PROTOCOL_V5` alongside `PROTOCOL_V3`, and the decode
branch matches on it rather than on a bare literal. The sequence-prefix width is
`WIRE_V5_SEQ_SIZE` and is used by the decode branch, so the constant is load-bearing
rather than decorative.

This ADR is the owning record for the V5 broadcast envelope. Its normative layout is
`[0x05][u64 broadcast_seq LE][V3 body]`, where the body carries no inner `0x03` byte and a
receiver distinguishes the two frames by the lead byte alone. `broadcast_seq` exists to
give clients a monotonic ordering and drop-detection handle; a decoder that discards it is
conformant but forfeits drop detection.

## Consequences

Changing the record width now fails the build rather than a test run, which is the point:
the failure is unmissable and arrives before any artefact is produced. The cost is that a
deliberate future width change must edit the assertion as well as the constants — that is
the intended friction, and the assertion message names the consequence.

Follow-on work: the TypeScript web client has no V5 handling at all and still advertises
V2 in `SUPPORTED_PROTOCOLS`, while the XR Rust client parses V5 correctly. That parity gap
lives in `client/src/services/binaryProtocol/` and was routed to the vc-clients lead with
an exact spec citing this ADR. Until it lands, a V5 envelope is unrecognised on web.
`crates/visionclaw-protocol/src/wire_fixtures.rs` pins the same constants for both Rust
decoders (asserted at `binary_protocol.rs:916-918`); the TS decoder remains the only one
with no fixture-backed cross-check, which is why it was the one that drifted.

## Verification

`cargo check -p visionclaw-server` — clean, zero errors, at the working tree above
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`.

The const assertions were confirmed to be real guards, not no-ops: they are evaluated in
`const` context, so `WIRE_V3_ITEM_SIZE` is required to equal 52 at compile time.

Stale in-code citation corrected in the same change: the `MessageType::BinaryPositions`
doc comment described Protocol V3 as "48 bytes/node"; the wire is 52. The 48-byte figure
was an interim analytics count retired by ADR-2018.

**Tracking caveat:** `crates/visionclaw-protocol/src/wire_fixtures.rs`, which this ADR relies on, is UNTRACKED in the owner's working
tree and absent from `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` — a clean checkout of that SHA
will not contain it. The landing commit must add it, or this ADR describes a file that does not
exist in the repository.

Verification ran on the uncommitted working tree above the recorded SHA and must be re-run
at the landing commit; `verified_paths` is empty for that reason.

## Amendment — 2026-09-05 (vc-clients, on ADR-2078)

This ADR's two routed client findings were reviewed against the code before implementation.
**Finding 1 was wrong and is withdrawn.** It stated the TypeScript client has no V5 envelope
support. The live TS decode path always had it:

- `client/src/types/binaryProtocol.ts:66` defines `PROTOCOL_V5`; `:198-201` dispatches to
  `parseV5Nodes`; `:410-424` reads the u64 LE sequence and decodes the body from offset 9, with a
  `byteLength < 9` guard that mirrors this ADR's own server-side `payload.len() < WIRE_V5_SEQ_SIZE`
  reject at `src/utils/binary_protocol.rs:594`; `:453-459` surfaces `broadcastSequence`.
- `client/src/store/websocket/binaryProtocol.ts:383` accepts V5 and `:416` already uses that
  sequence as the broadcast-ack sequence — the drop/reorder purpose was wired.

The module the finding pointed at, `client/src/services/binaryProtocol/`, is **not** the live
position path: it carries the outbound framed header and the 21-byte agent-position format, and has
no 52-byte decoding at all. A V5 branch was written there to satisfy the spec and was **reverted
before landing**, because a second decoder is the duplication class ADR-2074 had just removed.

**Finding 2 held, and was worse than reported.** V2 was not merely advertised in
`SUPPORTED_PROTOCOLS`; it was decoded — `types/binaryProtocol.ts:186-189` parsed 36-byte records and
`store/websocket/binaryProtocol.ts:476` routed a `0x02` lead byte into the handler. Worse, the
`default` arm re-read an unrecognised frame from offset 0 as 36-byte records whenever its length
divided by 36, fabricating nodes at arbitrary positions out of any unknown payload. That
fabrication hazard, not the absent V5 support, was the real defect.

Both are closed by **ADR-2078**, which also gives the TS decoder the fixture-backed cross-check the
two Rust decoders already had against `crates/visionclaw-protocol/src/wire_fixtures.rs` — the
absence of which is why this decoder was the one that drifted.
