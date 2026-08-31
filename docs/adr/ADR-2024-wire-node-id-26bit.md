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
`& NODE_ID_MASK`. The 26-bit ceiling is asserted with `debug_assert!` (fail-fast
in debug); release builds additionally emit a `log::error!` before masking an
over-range ID to its low 26 bits, so the historical V1 truncation is now **loud,
not silent**. This forecloses shipping a URN on the hot path.

## Consequences

- Per-frame payloads stay compact and the client reads node type without a
  lookup.
- The release guard logs (`error!`) but still truncates: an over-range ID is now
  loud, not silently corrupt, though truncation remains the failure mode above
  2^26. A hard reject (Result-returning setters) is the next step if node counts
  ever approach the ceiling.
- Only three type classes fit the reserved bits; a fourth top-level class needs a
  new bit and a protocol bump.

## Verification

Re-verified at `eac01130`: `src/utils/binary_protocol.rs` — flag masks and
`NODE_ID_MASK = 0x03FFFFFF`; the five `set_*_flag` setters (agent, knowledge, and
three ontology subtypes) `debug_assert!` the ceiling, then — in every build —
`if node_id > NODE_ID_MASK { error!(…) }` before `(id & NODE_ID_MASK) | FLAG`, so
the release path now logs the overflow instead of truncating silently; decode
strips via `& NODE_ID_MASK` (`clear_all_flags`, `get_actual_node_id`). The
silent-truncation gap is closed (loud, not silent); a hard reject remains the
open hardening tracked by `review_trigger`.
