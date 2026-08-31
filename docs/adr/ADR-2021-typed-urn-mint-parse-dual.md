---
id: ADR-2021
title: Durable urn:visionclaw IDs are minted only through typed fail-closed constructors, with mint split from resolve
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: any new persisted identifier namespace, or a decision to rewrite/retire the legacy urn:ngm:* scheme
repo: visionclaw
domain: IDENTIFIER-taxonomy
lineage: legacy ADR-105 (urn convergence + ngm cutover), ADR-100 (named-graph IRIs retained); distils the agentbox uris.js mint-only mandate
---

# ADR-2021 — Durable urn:visionclaw IDs are minted only through typed fail-closed constructors, with mint split from resolve

## Context

VisionClaw `main` carries a legacy `urn:ngm:*` scheme while the converged
`urn:visionclaw` grammar lands alongside it (no rip-out). Two forces collide:
new durable IDs must be converged-only and validated, yet pre-cutover IDs
already persisted (nodes, edges, named graphs) must keep resolving un-rewritten.
Ad-hoc `format!()` construction anywhere would let an unvalidated string become a
durable ID, bypassing the grammar. See `docs/IDENTIFIER-taxonomy.md` for the
governing living-doc grammar.

## Decision

Every durable `urn:visionclaw` identifier is minted through a typed constructor
in `src/uri/mod.rs` (`concept`, `kg`, `bead`, `execution`, `group_members`,
`room`, `avatar`, `did_nostr`); ad-hoc `format!()` minting is prohibited so
validation cannot be bypassed. The mint and resolve surfaces deliberately
diverge: `parse()` rejects the legacy `urn:ngm:*` namespace (`NotVisionclaw`) so
no new legacy ID is ever created, while `parse_dual()` additionally accepts a
persisted `urn:ngm:*` opaquely as `ParsedUri::LegacyNgm` so pre-cutover IDs keep
resolving. `urn:ngm:graph:*` named graphs are recognised but never rewritten
(ADR-100). This forecloses minting a legacy ID and forecloses a resolve path
that silently drops legacy data.

## Consequences

- Strict validation lives in one module; every mint site inherits it.
- Two read primitives exist and must not be confused: mint/validate call
  `parse()`, resolve/lookup call `parse_dual()`. A resolver that mistakenly calls
  `parse()` would 404 legacy IDs — a latent trap until ngm is fully retired.
- The legacy namespace persists indefinitely until a separate migration ADR
  retires it; carrying both grammars is an ongoing maintenance cost.

## Verification

Re-checked at `e0f8cd896`: `src/uri/mod.rs:33-35` states the ad-hoc `format!()`
ban; `:184-268` are the typed mint fns (`kg()` at `:207-216` rejects a non-hex
owner via `is_pubkey_hex`); `parse()` at `:357-369` returns `NotVisionclaw` for
`urn:ngm:*`; `parse_dual()` at `:467-483` wraps it as `LegacyNgm`; the doc
comment `:464-466` records that `urn:ngm:graph:*` is not rewritten.
