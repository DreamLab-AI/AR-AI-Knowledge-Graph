---
id: ADR-2016
title: GRAPH_PROVENANCE is append-only (INSERT DATA only)
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: 9a2c8087385bf6db08b1aeb91004e1a60203965b
verified_paths: [crates/visionclaw-adapters/src/provenance_emitter.rs, crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs, src/services/ontology_mutation_service.rs]
owner: jjohare
review_trigger: a GDPR/right-to-erasure obligation landing on provenance-recorded subjects, or introduction of a redaction/crypto-shred mechanism
repo: visionclaw
domain: DATA-authority-erasure
lineage: legacy ADR-033 (git-bead provenance), ADR-034 (needle-bead), ADR-124/ADR-128 (web-contract / gitmark blocktrails)
---

# ADR-2016 — GRAPH_PROVENANCE is append-only (INSERT DATA only)

## Context

Provenance must be a tamper-evident record: a governed event that could be
silently rewritten or deleted afterwards is not provenance. The PROV-O triad
(Entity/Activity/Agent) reifies the same identifiers used on the decision
paths, so the RDF is a queryable projection, never a fork. The constraint is
in tension with data-erasure duties: an append-only log cannot honour a delete.

## Decision

Every governed event writes a full PROV-O `prov:Entity` / `prov:Activity` /
`prov:Agent` triad (plus `wasGeneratedBy` / `wasAttributedTo`) into
`GRAPH_PROVENANCE` via **insert-only quad writes**. `DELETE`, `DROP`, and
`CLEAR` are never issued against this graph. All emission funnels through the
single `reify_activity` primitive (called via the async `emit_activity` /
`emit_activity_nonfatal` wrappers), whether invoked through
`OxigraphOntologyRepository::emit_provenance` or directly by a caller such as
`ontology_mutation_service.rs`. This forecloses in-place mutation and
destructive compaction: any future erasure of a provenance-recorded subject
must be an explicit redaction or crypto-shred mechanism — which does not yet
exist.

## Consequences

- The provenance graph grows monotonically; there is no compaction and no
  built-in retention trim. Storage cost is unbounded over time.
- There is a real erasure gap: a right-to-be-forgotten request against a
  subject recorded here cannot be satisfied today. This interacts with the
  backup / write-master posture in ADR-2017 (no PITR, no consistent restore).
- Tamper-evidence and agent-scoped SPARQL (`?a a prov:Agent`) are guaranteed
  because the triad is always complete and never edited.

## Verification

Re-checked at `e0f8cd896`: `provenance_emitter.rs` header states the
append-only rationale ("only `INSERT DATA` is permitted. No `DELETE`, `DROP`,
or `CLEAR`"); `reify_activity` (line 97) issues only `store.insert(QuadRef::new(...))`
calls for the Activity/Agent/Entity triad, and a grep of the file for
`remove`/`clear`/`DELETE`/`DROP`/`CLEAR` returns nothing but the header comment.
Two call sites reach `reify_activity`: `oxigraph_ontology_repository.rs:646`
(`emit_provenance`, wrapping `emit_activity`) and
`src/services/ontology_mutation_service.rs:127` (`emit_activity_nonfatal`,
called directly, bypassing `emit_provenance`) — both resolve to the same
insert-only primitive, so the append-only property holds across both paths
even though `emit_provenance` is not the sole entry point.
Re-verified at `542d63d1d` after the ADR-141 formatting sweep (test-only line
wrapping in `ontology_mutation_service.rs` `provenance_wiring_tests`) — the
append-only invariant is unchanged.
