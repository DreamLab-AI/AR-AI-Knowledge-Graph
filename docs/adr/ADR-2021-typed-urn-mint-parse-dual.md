---
id: ADR-2021
title: Durable urn:visionclaw IDs are minted only through typed fail-closed constructors, with mint split from resolve
date: 2026-08-31
decision_status: accepted
implementation_status: partial
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
diverge: `parse()` rejects the legacy `urn:ngm:*` namespace (`NotVisionclaw`) while other legacy mint sites still exist (see closeout below), while `parse_dual()` additionally accepts a
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

## Closeout extension — 2026-09-04

CP-01/04/05/08. Owner remains jjohare with identifier/identity/storage maintainers. Implementation is partial: ontology_mutation_service directly formats a converged execution activity identifier, and the graph repository still formats legacy node/edge identifiers. A strict parser cannot prohibit minting elsewhere. Existing legacy read compatibility and the no-rewrite named-graph policy remain; do not infer a migration authorisation from this finding.

**Acceptance condition:** Inventory durable mint, compatibility-write, lookup and display sites; use typed validation or record deliberate exceptions. Verify canonical bytes across signed identity and persistence boundaries, with challenge freshness/reuse/audience policy and separate role admission. Test old/new lookup, duplicate joins and rollback before retiring legacy data. Reopen on identifier grammar, proof verification, new persistence sites or migration decisions. See the [review](../../../VisionFlow/docs/estate-review/federation-identifiers.md#mint-site-coverage-and-proof-of-identity) and [receipt](../../../VisionFlow/docs/estate-review/evidence/identifier-mint-sites.json). Prior paired-helper source hashes match; no new live proof or persistence test ran.

## Acceptance progress — 2026-09-05

**Implemented.** `src/uri/mod.rs`, `src/adapters/oxigraph_graph_repository.rs`,
`src/services/ontology_mutation_service.rs`. The mint-site inventory the
acceptance asked for was carried out, and both named sites are resolved — one by
routing through typed constructors, one by recording a deliberate exception with
its parser.

* *`ontology_mutation_service`* minted
  `format!("urn:visionclaw:execution:{kind}-{proposal_id}")`, which is not a
  `sha256-12-` content address and therefore does not satisfy the execution
  grammar `uri::parse` enforces — the record it wrote could never be parsed
  back. It now calls `uri::execution(...)` and `uri::did_nostr(agent_id)`; an
  invalid agent id skips the emission with a warning (provenance is fail-open by
  contract) rather than writing an unattributable record.
* *Graph repository legacy IRIs* are the recorded exception. ADR-105's
  no-rewrite policy forbids re-minting the on-disk identity, so rather than
  leave inline `format!`s free to drift, the scheme moves into `uri::ngm` —
  `node_iri`/`parse_node_iri`, `edge_iri`/`parse_edge_iri`, `is_edge_iri` and
  `edge_lookup` — beside its parser, documented as minting legacy identifiers on
  purpose and closed to new use.

*A defect the inventory surfaced.* `remove_edge` built a **one**-segment
`urn:ngm:edge:<id>` while `edge_iri` writes the **three**-segment
`urn:ngm:edge:<source>:<target>:<id>`. No subject could ever match, so every
delete silently removed nothing. `uri::ngm::edge_lookup` distinguishes the two
forms a caller can hold — the full IRI a read returns in `Edge::id`, and the
bare id `Edge::new` leaves there — and `remove_edge` now issues an exact-subject
delete or a trailing-component filter accordingly.

**Tests.** `cargo test --lib --no-default-features uri::` — 31 passed, 0 failed
(6 new for this record): node and edge IRIs round-tripping across the `u32`
range and with a colon-bearing edge id; foreign input rejected, including the
one- and two-segment shapes; `edge_lookup` distinguishing the two forms; a
minted edge IRI ending with its bare id (the delete filter depends on it);
legacy identifiers resolving as legacy and never as converged URNs; and the
mutation service's new execution URN parsing where the old shape does not.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2021-2023-identifiers.txt`.

**Remains open.** Display and compatibility-write sites beyond the two named
were inventoried but not migrated (`decision_service`, `github_sync_service`,
`nostr_identity_verifier` each mint directly). Challenge freshness/reuse/audience
policy, old/new lookup and duplicate joins, and rollback before retiring legacy
data are untouched. No live proof or persistence test ran.
