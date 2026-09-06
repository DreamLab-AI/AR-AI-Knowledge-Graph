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

## Closeout extension — 2026-09-04

CP-01/02/04/05. Owner remains jjohare with agentbox identifier maintainers. Five UTF-8 string fixtures match the agentbox BC20 hash helper. The Rust precomputed KG constructor/parser accepts malformed suffixes after the prefix; hash emission and input validation are separate guarantees.

**Acceptance condition:** bind bytes/serialisation, complete address grammar and kind/elevation support to a shared versioned fixture run in both repositories. Test malformed precomputed addresses and persisted round-trip recovery. Preserve explicit unmapped outcomes and existing decision status. Reopen on hash, parser, scope or kind-map changes. See the [identifier review](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/federation-identifiers.md) and [paired receipt](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/evidence/federation-identity-probe.json). These are helper-level results, not deployed-ingest certification.

## Acceptance progress — 2026-09-05

**Implemented.** `src/uri/mod.rs`. The reproduced defect — the Rust precomputed
KG constructor and parser accepted **any** suffix after the `sha256-12-` prefix,
so `sha256-12-`, `sha256-12-ZZZZ`, an upper-case digest and a 40-hex-character
body all round-tripped as valid addresses — is closed.

`is_content_address` defines the grammar: the prefix followed by exactly
`CONTENT_ADDR_HEX_LEN` (12) lowercase hex characters and nothing else. It is
enforced by `kg_with_address` (the precomputed constructor), by `parse` for
`Kg`/`Bead`, and by `parse` for `Execution` and `Room`, which previously checked
only the prefix plus an absent colon.

**Tests.** `cargo test --lib --no-default-features uri::` — 31 passed, 0 failed
(7 new for this record): every address the emitter produces satisfies the
grammar the parser enforces (hash emission and input validation tied together);
13 malformed precomputed addresses rejected by both `is_content_address` and
`kg_with_address`, each with its reason; malformed addresses rejected inside
full `kg`/`bead` URNs and inside `execution`/`room` URNs; a well-formed address
still round-tripping; persisted round-trip recovery (mint → serialise → parse →
re-mint yields the identical string); and every lowercase hex digit accepted, so
the grammar is not accidentally narrower than the emitter.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2021-2023-identifiers.txt`.

**Remains open.** The shared versioned fixture run in **both** repositories is
not done — these results are VisionClaw-side only, and the agentbox BC20 helper
was not re-run against them. Kind/elevation support and the complete address
grammar beyond the content address are unaddressed. Not deployed-ingest
certification.
