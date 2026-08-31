---
id: ADR-2019
title: Tag byte 0 is registry-governed — it selects the codec, rejects unknown tags, and is allocated across disjoint per-socket spaces
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
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
