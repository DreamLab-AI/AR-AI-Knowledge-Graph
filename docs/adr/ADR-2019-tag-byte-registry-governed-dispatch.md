---
id: ADR-2019
title: Tag byte 0 is registry-governed — it selects the codec, rejects unknown tags, and is allocated across disjoint per-socket spaces
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 9a2c8087385bf6db08b1aeb91004e1a60203965b
verified_paths: [src/utils/binary_protocol.rs, xr-client/rust/src/binary_protocol.rs, src/protocols/binary_settings_protocol.rs, crates/visionclaw-xr-presence/src/wire.rs, crates/visionclaw-xr-presence/src/agent_presence.rs]
owner: jjohare
review_trigger: allocation of a new opcode/version tag on any binary socket, or a proposal to share one demultiplexer across sockets
repo: visionclaw
domain: PROTOCOL-registry
lineage: "Reverses legacy ADR-061 D4 (versioning-vocabulary-removed); distils ADR-059 (0x23 agent channel) + ADR-102 (presence handshake) tag fragmentation into a single allocation authority."
---

# ADR-2019 — Tag byte 0 is registry-governed — it selects the codec, rejects unknown tags, and is allocated across disjoint per-socket spaces

## Context

Every binary frame leads with a tag byte. ADR-061 D4 had removed the versioning
vocabulary, leaving no authority over what the leading byte means or who may
allocate one; tags accreted ad hoc across channels (ADR-059 0x23 agent,
ADR-102 presence handshake). Without a rule, a receiver could reinterpret an
unknown tag as a known one and silently misparse, and there was no defined
relationship between the graph socket's tag space and the settings/presence
sockets' tag spaces, which happen to reuse the same numeric values.

## Decision

The leading tag byte **selects the codec** and receivers MUST branch on it:
graph `0x03` decodes a bare V3 body, `0x05` a V5 envelope. Removed versions
(V1/V2) **fail loud** with an explicit upgrade error, and any unknown tag is
**rejected, never reinterpreted**. Tag allocation happens only in the
PROTOCOL-registry and is **scoped per socket**: the graph space, the settings
space, and the presence space are independent, so numeric overlap between them
(e.g. graph `0x05` vs settings `0x05`) is legitimate because each socket is
demultiplexed on its own. This forecloses cross-socket tag collisions being
treated as errors and forecloses lenient reinterpretation of unknown bytes.

## Consequences

- A corrupt or downgraded frame surfaces as a named error at the boundary
  rather than a misparsed record; clients on retired versions get a clear
  upgrade signal instead of garbage.
- The same byte value carries different meanings on different sockets by
  design; anyone reading the registry must read the socket column too — reuse
  is a feature, not a clash.
- Every new tag is a registry transaction: adding one out-of-band (a raw magic
  byte in a codec) is now a defect, which is the intended governance cost.

## Verification

At e0f8cd896: `src/utils/binary_protocol.rs` `match protocol_version` returns
explicit "no longer supported / please upgrade" errors for `1` and `2`, routes
`0x03`/`5`, and ends `v => Err(format!("Unknown protocol version: {}", v))`.
Client `xr-client/rust/src/binary_protocol.rs` carries a typed
`DecodeError::BadVersion`. Disjoint reuse confirmed: graph `0x05` (the V5
branch) coexists with settings `0x05` in
`src/protocols/binary_settings_protocol.rs`, and presence uses `0x43`
(`crates/visionclaw-xr-presence/src/wire.rs`) and `0x44`
(`.../agent_presence.rs`).

## Closeout extension — 2026-09-04

Work package: **CP-01/06**. Owner remains `jjohare`, with protocol, identity and XR maintainers responsible for their respective boundaries.

Position frames reject unknown versions and misalignment. The separately dispatched 0x23 visual batch accepts complete events before truncation, so rejection guarantees must be scoped by codec.

**Acceptance condition:** Document and test each opcode's malformed/truncated-frame policy; use fixtures from the real server encoder in both browser and XR decoders and verify unknown-tag handling at each demultiplexer.

Dependencies: CP-01 release identity and CP-04 authority where authenticated actions cross the wire. Reopen on the existing review trigger, a changed opcode or a failing freshness/visibility probe. Existing verification and activation fields retain their historical scope; this annex records source/local tests at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`, not a new live certification.

See [rendered-state review](../../../VisionFlow/docs/estate-review/rendered-state.md) and [receipt](../../../VisionFlow/docs/estate-review/evidence/xr-render-snapshot.json).

## Acceptance progress — 2026-09-05

**Policy documented.** The per-opcode malformed/truncated policy is now written
down where each decoder lives, and — importantly — the policy is *not uniform*,
which was the substance of the closeout finding. Recorded matrix:

| Opcode | Consumer | Malformed / truncated policy |
|---|---|---|
| `0x03`/`0x05` position | XR (Rust) | all-or-nothing: unknown version, misaligned body and truncated V5 sequence all reject the whole frame |
| `0x03`/`0x05` position | server decoder | all-or-nothing |
| `0x03`/`0x05` position | browser | tolerant: complete records kept, torn tail dropped (a partial record must not blank the view) |
| `0x23` agent action | XR (Rust) | tolerant prefix parse: complete events before a truncation are accepted |
| `0x23` agent action | server decoder | strict: a truncated or over-counted batch is refused whole |
| `0x23`/`0x43`/`0x44` | browser position decoder | declined outright, never size-auto-detected |

The XR/server asymmetry on `0x23` is deliberate and now stated as such: the
ingest side must not accept a partial batch into the event stream, while the
render side should not discard a whole burst of visual activity over one torn
event.

**Implemented.** One behavioural change, in the browser demultiplexer. Its
`default:` branch previously fell through to size-based auto-detection for *any*
unrecognised first byte, so a `0x23`/`0x43`/`0x44` frame whose length happened to
be a multiple of the 36-byte V2 stride would be reinterpreted as node records —
fabricating nodes at arbitrary positions from another codec's payload. A
`SIBLING_OPCODES` guard now declines known sibling opcodes before auto-detection;
genuinely unknown, non-sibling versions still auto-detect as before.

**Fixtures from the real encoder.** `crates/visionclaw-protocol/src/wire_fixtures.rs`
supplies both well-formed and malformed frames, and is pinned byte for byte to
`encode_node_data_extended_with_sssp` and `encode_agent_actions` by
`adr_2019_shared_fixture_equivalence`. The XR decoder consumes that same source
file via `#[path]`; the browser test mirrors its layout with a comment binding it
to the Rust module.

**Tests run.**

- `xr-client/rust`: `cargo test` — 227 lib + 75 integration pass. Frame-policy
  cases cover unknown versions (9 values), misaligned bodies, truncated V5
  sequence (8 lengths), empty frames, the `0x23` tolerant prefix parse,
  overstated batch counts and mutual opcode refusal between the two codecs.
- Server: `cargo test --lib --no-default-features adr_20` — 34 pass, including
  strict server-side batch rejection and unknown-version refusal.
- Browser: `./node_modules/.bin/vitest run
  src/types/__tests__/binaryProtocol.framePolicy.test.ts` — 15 pass, covering a
  256-value version sweep, exhaustive truncation sweeps asserting the decoder
  never throws, sibling-opcode refusal and V5 envelope additivity.

**Governed paths changed.** `client/src/types/binaryProtocol.ts`,
`client/src/types/__tests__/binaryProtocol.framePolicy.test.ts`,
`xr-client/rust/tests/wire_freshness_and_frame_policy.rs`,
`crates/visionclaw-protocol/src/wire_fixtures.rs`, `src/utils/binary_protocol.rs`
(tests).

**Open.** Fixtures are byte-equal to the encoder's output, but no frame was
carried over a live socket in this pass, and the browser decoder's behaviour is
verified in jsdom rather than in a running client.
