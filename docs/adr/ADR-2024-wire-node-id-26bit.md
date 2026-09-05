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

## Closeout extension — 2026-09-04

CP-01/02/06/08. Owner remains jjohare with protocol/graph/client maintainers. Complete/live is retained for ephemeral 26-bit IDs and typed setter behaviour. The inspected server/XR/browser masks agree. Overflow logging is specific to the five typed setters: the untyped encoder branch only debug-asserts, then forwards the unchanged ID through the identity wire helper. This source finding does not establish a live overflow or collision.

**Acceptance condition:** Test allocator plus encoder/decoder boundaries for every class, including untyped nodes, in debug and release; specify reject/remap behaviour before capacity exhaustion. Bind compact maps to graph generations and test full/delta/reconnect and stale-map retirement across clients. Reopen on allocator/capacity, class-bit allocation or mapping persistence changes. See [review](../../../VisionFlow/docs/estate-review/rendered-state.md#wire-identifier-overflow-coverage) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/wire-force-boundaries.json). No runtime overflow frame was exercised.

## Acceptance progress — 2026-09-05

**Implemented — the release-build gap is closed.** The finding was that the five
typed setters logged and masked overflow in release while the untyped encoder
branch only `debug_assert!`ed and then forwarded the id *unchanged*. A release
build therefore shipped an over-range id straight onto the wire with no
diagnostic, where it aliases an existing id and picks up spurious class-flag bits.

All six branches now share one helper, `enforce_wire_id_bounds(id, WireIdClass)`,
with behaviour **identical in debug and release**: remap by masking to 26 bits and
log the overflow as an error naming the class and both ids. `debug_assert!` is
retained so development builds still fail loudly at the offending call site, but
release no longer depends on it for either the bound or the diagnostic.

Remap rather than reject is the deliberate choice, and is now documented as such:
a dropped node vanishes from the layout with no trace, whereas a masked one is
visibly wrong *and* leaves a log line. `WireIdClass::Untyped` exists precisely so
the previously-silent branch names itself in that log.

`to_wire_id_v2` stays an identity function, and now says why: it runs on an
already-flagged id whose bits 26..=31 legitimately carry the class, so masking
there would strip the class off every node. The bound is enforced upstream on the
bare id.

**Testability.** The overflow branch cannot be reached through
`enforce_wire_id_bounds` in a test profile — `debug_assert!` panics first, which
is exactly how the release-only gap went unnoticed. The pure remap is therefore
split out as `remap_wire_id(id) -> (masked, overflowed)`, which *is* the code a
release build runs, and the tests target it directly.

**Tests run.** `cargo test --lib --no-default-features adr_20` — 34 pass, 6 of
them in `adr_2024_wire_id_bounds`: in-range pass-through for all six classes;
release-path remap and overflow reporting for `2^26`, `2^26+7`, `0x80000000` and
`u32::MAX`; exact boundary (`NODE_ID_MASK` valid, `+1` overflows); a remapped id
leaking no flag bit (`get_node_type` returns `Unknown`); each typed setter
stamping only its own class with the id surviving; and an end-to-end encode
proving the untyped branch emits a bounded, class-free wire id.

**Governed paths changed.** `src/utils/binary_protocol.rs`.

**Open.** No over-range id was produced by a live allocator and no rendered
collision was observed — this is boundary coverage, not evidence of a deployed
overflow. Per-generation durable-to-wire mapping, full/delta/reconnect map
consistency and stale-map retirement across clients remain unaddressed; those are
allocator and session-lifecycle concerns rather than encoder-boundary ones.
`implementation_status` is unchanged.
