---
id: ADR-2078
title: The TypeScript decoder fails closed on V2 and unknown versions, PROTOCOL_V4 names one thing, and the wire is fixture-pinned
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new position protocol version, or any change to NODE_RECORD_BYTES / NODE_ID_MASK in crates/visionclaw-protocol/src/wire_fixtures.rs — the pinned test must be updated in the same change
repo: visionclaw
domain: PROTOCOL-registry
lineage: implements the client half of ADR-2057's wire hygiene and amends that ADR — its Finding 1 premise about missing V5 support was wrong, the live TS decoder always had it; ownership passed to vc-clients when vc-gpu-wire was stood down
---

# ADR-2078 — The TypeScript decoder fails closed, `PROTOCOL_V4` names one thing, and the wire is fixture-pinned

## Context

vc-gpu-wire routed two findings for the TS wire decode boundary (their ADR-2057).
**One of them did not hold.** Finding 1 said the TS client has no V5 envelope support: it does,
on the live path — `client/src/types/binaryProtocol.ts:66` defines `PROTOCOL_V5`, `:198-201`
dispatches to `parseV5Nodes`, `:400-404` documents the exact
`[0x05][u64 seq LE][V3 body, no inner 0x03]` layout, `:453-459` surfaces `broadcastSequence`, and
`client/src/store/websocket/binaryProtocol.ts:383,416` uses that sequence for broadcast acks.
This matches diagram VC-32.2, drawn in Phase 1.

Finding 2 did hold, and was worse than reported: V2 was not merely *advertised* in
`services/binaryProtocol/frameTypes.ts`, it was **decoded** — `types/binaryProtocol.ts:186-189`
parsed 36-byte V2 records and `store/websocket/binaryProtocol.ts:476` routed V2 frames into the
legacy handler, while the server rejects V2 outright (`src/utils/binary_protocol.rs:565`).
Worse still, the `default` arm re-read an unrecognised frame from offset 0 as 36-byte records
whenever its length happened to divide by 36 — fabricating nodes at arbitrary positions out of
any unknown payload.

## Decision

The live decoder accepts exactly what the server emits — V3 (`0x03`) and the V5 envelope
(`0x05`) — and declines everything else instead of guessing.

- **V2 is declined with a diagnostic**, not decoded. `PROTOCOL_V2` survives only to *name* the
  rejected version in that message; the 36-byte `BINARY_NODE_SIZE_V2` stride and the
  V2/V3 size-swap heuristic are deleted, because nothing on the live wire is 36 bytes.
- **An unknown version is declined**, matching the existing `SIBLING_OPCODES` guard. Size-based
  auto-detection is removed entirely: with one stride left there is nothing to detect, and the
  fallback could only ever fabricate nodes.
- `store/websocket/binaryProtocol.ts` no longer routes a `0x02` lead byte into the decoder.
- **`PROTOCOL_V4` names exactly one thing.** The constant was declared twice with different
  meanings: `frameTypes.ts:17` (framed-header version) and `types/binaryProtocol.ts:65` (delta node
  encoding). The `frameTypes.ts` one is **deleted** — nothing ever wrote 4 into the header byte —
  and `SUPPORTED_PROTOCOLS` is renamed `SUPPORTED_HEADER_VERSIONS = [PROTOCOL_V3, PROTOCOL_V5]`,
  which is what it actually gates. `BinaryWebSocketProtocol.ts` no longer comments its header
  "V4 header" while writing `PROTOCOL_VERSION` (= V3 = 3); the comment now states that the version
  byte carries the *position* protocol version. `PROTOCOL_V4` survives only in
  `types/binaryProtocol.ts`, meaning delta node encoding.
- **The tag enum is annotated against the server's five.** `src/utils/binary_protocol.rs:1705-1722`
  defines exactly `BinaryPositions = 0`, `VoiceData = 0x02`, `ControlFrame = 0x03`,
  `AgentAction = 0x23`, `BroadcastAck = 0x34`. The TS `MessageType` members are marked
  server-tag-vs-client-outbound-only with that citation. They are **annotated, not deleted**: the
  framed-header path is a client-to-server surface whose server-side parsing lives outside
  `binary_protocol.rs`, so deleting a *referenced* outbound type on client-side evidence alone
  would be unverifiable overreach.
- **Five members with zero references are deleted**, not deferred: `VELOCITY_UPDATE` (0x12),
  `AGENT_STATE_DELTA` (0x21), `AGENT_HEALTH` (0x22), `HANDSHAKE` (0x32) and `HEARTBEAT` (0x33).
  Nothing encoded or decoded them, by name or by literal value. An earlier draft of this ADR named
  them in a doc comment as "deletion candidates pending confirmation" — that is deferred deletion,
  which this estate does not do, and zero references is itself the evidence the policy needs.
  Every member that remains has a live reference.
- **The decoder is fixture-pinned.** `client/src/types/__tests__/wireFixtures.test.ts` asserts
  the same constants the two Rust decoders pin against
  `crates/visionclaw-protocol/src/wire_fixtures.rs`, round-trips a synthetic V5 envelope, and
  asserts the V2, unknown-version and sibling-opcode rejections.

**No second decoder is introduced.** A parallel `nodePositionFrame.ts` module was written during
this work and then removed: the live decoder already did the job, and a duplicate is precisely
the defect class ADR-2074 had just eliminated. The 52-byte size and the node-id mask are
therefore *not* redeclared in `frameTypes.ts` — they live once in `types/binaryProtocol.ts`.

## Consequences

- A malformed or stale frame now yields zero nodes and a warning, instead of plausible-looking
  garbage positions. This is a behaviour change for any deployment still sending V2 — none
  exists, because the server rejects it.
- The TS decoder gains the fixture-backed cross-check the two Rust decoders already had. It was
  the only one without, which is why it drifted; the pin is what stops recurrence.
- `BINARY_NODE_SIZE_V2` is gone from the module's exports. It had no callers outside the file.
- `MessageType` shrinks from 20 members to 15, all of which are referenced. A reader can no longer
  mistake an unused tag for a live protocol surface.
- The `PROTOCOL_V4` collision and the misleading "V4 header" comment are **fixed here**, not
  deferred: vc-gpu-wire was stood down and ownership passed to this lead. A reader can no longer
  meet the name `PROTOCOL_V4` and have to work out which of two things it means.
- `SUPPORTED_HEADER_VERSIONS` is now strictly `[V3, V5]`, so a framed header claiming version 4 is
  rejected where it was previously accepted. Nothing writes 4 into that byte, so no live sender is
  affected — but it is a tightening, and worth knowing if a future sender wants to frame a V4
  delta payload.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit.

- `cd client && ./node_modules/.bin/tsc --noEmit` → exit 0, no output.
- `cd client && npx vitest run src/types/__tests__/wireFixtures.test.ts` → `12 passed (12)`,
  including the V2, unknown-version and sibling-opcode rejections, the V3-vs-V5 body-identity
  check, the V5 short-payload boundary (1/4/8-byte and header-only frames all decline, matching the
  server's `payload.len() < WIRE_V5_SEQ_SIZE` reject at `binary_protocol.rs:594`), and an assertion
  that `SUPPORTED_HEADER_VERSIONS` is exactly `[V3, V5]` and never contains 4.
- Server parity re-verified at the current line numbers: `src/utils/binary_protocol.rs:588-601`
  rejects V1 (`:589`) and V2 (`:590`), decodes V3 (`:591`), and for V5 (`:592`) rejects
  `payload.len() < WIRE_V5_SEQ_SIZE` (`:594`) before delegating (`:597`), with unknown versions
  rejected at `:600`. The TS live path mirrors every one of those arms.
- `cd client && npm test` (`vitest run`) → `Test Files 71 passed (71)`, `Tests 789 passed (789)`,
  unchanged by the five-member enum deletion. This file contributes 12 of those; the remainder of the delta from the 69/773 session baseline is
  ADR-2080's new suite, landed concurrently in the same tree.
- Premise re-check that corrected Finding 1:
  `grep -n "PROTOCOL_V5\|parseV5Nodes\|broadcastSequence" client/src/types/binaryProtocol.ts
  client/src/store/websocket/binaryProtocol.ts` → V5 constant, dispatch, parser, and
  ack-sequence use all pre-existing.
- `grep -rn "PROTOCOL_V2" client/src --include=*.ts` → the constant and its single
  decline-with-diagnostic site only; no decode path.
- Deletion evidence for the five enum members, run immediately before removing them:
  `grep -rn "MessageType\.<NAME>" client/src --include=*.ts --include=*.tsx` → `0` for each, and a
  sweep for their literal values (`0x12`, `0x21`, `0x22`, `0x32`, `0x33`) outside `frameTypes.ts`
  → no output. `tsc --noEmit` exit 0 after removal.
