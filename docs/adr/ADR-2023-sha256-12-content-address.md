---
id: ADR-2023
title: Content address is sha256-12, byte-identical to the agentbox sha12() contract
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: any change to the truncation length or hash function, or a divergence from the agentbox sha12() helper
repo: visionclaw
domain: IDENTIFIER-taxonomy
lineage: legacy ADR-105 (convergence); distils the agentbox bc20-provenance-bridge sha12 contract
---

# ADR-2023 — Content address is sha256-12, byte-identical to the agentbox sha12() contract

## Context

Content-addressed IDs (`kg`, `bead`, `execution`, `room`) must be identical on
both sides of the VisionClaw/agentbox federation so cross-substrate joins compare
strings byte-for-byte. A truncation length or hex casing that drifts from the
agentbox `sha12()` helper would silently break every join without erroring. See
`docs/IDENTIFIER-taxonomy.md` for the address grammar.

## Decision

A content address is `sha256-12-<12 lowercase hex chars>`: the SHA-256 digest
truncated to its first 6 bytes, each rendered as two lowercase hex characters.
The truncation length is fixed at 6 bytes specifically to match agentbox exactly;
it is not a tunable. The `sha256-12-` prefix is a named constant
(`CONTENT_ADDR_PREFIX`) and is enforced at the BC20 ingest boundary by
`kg_with_address`, which rejects any pre-computed address lacking it. This
forecloses a locally chosen digest length and forecloses an unprefixed address
crossing the boundary.

## Consequences

- Cross-substrate joins are a plain string equality — no re-hash, no
  normalisation step.
- The 6-byte truncation is a deliberate collision-vs-brevity trade fixed by the
  cross-repo contract; widening it for collision headroom is a coordinated
  two-repo change, not a local tweak.
- Any agentbox-side change to `sha12()` is a breaking change here and must be
  co-ordinated.

## Verification

Re-checked at `e0f8cd896`: `src/uri/mod.rs:144-153` — `content_address()` does
`digest.iter().take(6)` emitting 12 lowercase hex chars behind
`CONTENT_ADDR_PREFIX` (`:48-49`); `kg_with_address` at `:220-228` enforces the
prefix at the BC20 boundary (`Malformed` otherwise). The doc comment at `:143`
records the byte-for-byte agentbox `sha12()` equivalence.
