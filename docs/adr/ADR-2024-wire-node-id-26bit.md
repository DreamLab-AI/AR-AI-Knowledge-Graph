---
id: ADR-2024
title: Wire node-ID is an ephemeral 26-bit u32 with reserved type-flag bits, not a URN
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/utils/binary_protocol.rs]
owner: jjohare
review_trigger: node count approaching 2^26, or promotion of the debug_assert ceiling to a runtime guard
repo: visionclaw
domain: IDENTIFIER-taxonomy
lineage: legacy protocol V1/V2 records (V1 truncation bug); distils the render-plane-vs-durable-store split
---

# ADR-2024 — Wire node-ID is an ephemeral 26-bit u32 with reserved type-flag bits, not a URN

## Context

The render plane pushes node positions to clients every frame; carrying a full
`urn:visionclaw` string per node per frame is untenable. The durable identifier
(the URN) and the wire identifier are therefore distinct concerns. Protocol V1's
narrow field silently truncated IDs above its ceiling and corrupted them; V3
must reserve enough width and encode node type inline for the client. See
`docs/IDENTIFIER-taxonomy.md` for the render-plane vs durable-store split.

## Decision

On Protocol V3 a node travels as a compact sequential `u32`: bits 0-25 are the
ID (`NODE_ID_MASK = 0x03FFFFFF`, ceiling 2^26-1), bits 26-31 are type flags
(bit 31 agent, bit 30 knowledge, bits 26-28 ontology subtypes). This wire ID is
ephemeral and render-only — it is never a durable identifier and never persisted
as one. Flag setters mask the ID and OR the flag; decode strips flags via
`& NODE_ID_MASK`. The 26-bit ceiling is asserted with `debug_assert!` only, so
release builds silently truncate an over-range ID to its low 26 bits. This
forecloses shipping a URN on the hot path; it does **not** yet foreclose the V1
truncation failure mode in release.

## Consequences

- Per-frame payloads stay compact and the client reads node type without a
  lookup.
- Open hardening: the ceiling guard is debug-only, so a >2^26 ID in a release
  build reintroduces the historical silent-truncation corruption. Promote the
  `debug_assert` to a runtime guard (reject/log) before node counts approach the
  ceiling.
- Only three type classes fit the reserved bits; a fourth top-level class needs a
  new bit and a protocol bump.

## Verification

Re-checked at `e0f8cd896`: `src/utils/binary_protocol.rs:14-26` — flag masks and
`NODE_ID_MASK = 0x03FFFFFF`; `set_agent_flag`/`set_knowledge_flag` at `:117-137`
`debug_assert!` the ceiling then unconditionally `(id & NODE_ID_MASK) | FLAG`;
decode strips via `& NODE_ID_MASK` (`clear_all_flags` `:143-145`,
`get_actual_node_id` `:155-157`). The release-truncation gap is confirmed: no
runtime guard exists, hence `implementation_status: complete` with the hardening
tracked by `review_trigger`.
