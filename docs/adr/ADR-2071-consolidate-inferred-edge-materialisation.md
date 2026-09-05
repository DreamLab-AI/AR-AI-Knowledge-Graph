---
id: ADR-2071
title: Consolidate inferred-edge materialisation onto the shared set-logic module
date: 2026-09-05
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: the next change to Whelk post-sync reasoning in src/services/github_sync_service.rs, or any report of long-range hierarchical edges in the client graph
repo: visionclaw
---

# ADR-2071 — Consolidate inferred-edge materialisation onto the shared set-logic module

## Context
- Two independent implementations materialise inferred hierarchical edges (Phase 1 diagram VC-20.4).
- `src/services/inferred_edge_materialiser.rs` is the shared module: constants (`:30-37`), the
  predicates `edge_is_inferred` (`:59`) and `build_inferred_edge` (`:68`), and the set-logic
  `asserted_pairs` (`:78`), `immediate_inferred_parents` (`:97`), `select_inferred_edges` (`:126`),
  `materialise` (`:158`). It is **not** dead: the live broadcast path consumes `edge_is_inferred`
  (`client_coordinator_actor.rs:2016`, `socket_flow_handler/types.rs:415`).
- `OntologyPipelineService::materialise_inferred_edges_from_axioms`
  (`ontology_pipeline_service.rs:473-495`) uses that module, and reduces the reasoner's **transitive**
  ancestors to **immediate** parents first, with the stated rationale "otherwise deep hierarchies
  would materialise long-range grandparent edges".
- The live Whelk path does not. `GitHubSyncService::run_post_sync_reasoning` (`~:1178-1239`) hand-rolls
  its own loop over `results.inferred_axioms`, with its own `owl#Nothing`/`owl#Thing`/self-loop
  filters and its own `IriNodeResolver`, and never calls `select_inferred_edges`.
- The two therefore differ in *behaviour*, not merely in style, and the weaker logic is on the live path.

## Decision
**Proposed.** `GitHubSyncService::run_post_sync_reasoning` stops hand-rolling edge selection and calls
the shared module: it maps Whelk `InferredAxiom`s to the module's input shape, applies
`immediate_inferred_parents` to reduce transitive ancestors, and emits through `select_inferred_edges`
/ `build_inferred_edge`, so asserted-pair suppression, the per-child parent cap
(`DEFAULT_MAX_INFERRED_PARENTS_PER_CHILD = 8`) and the `inferred` metadata key are applied identically
on both paths. `inferred_edge_materialiser` becomes the single definition of what an inferred edge is;
no caller re-implements the selection rules.

This is proposed rather than accepted because it changes materialised edge counts on the live corpus,
so it must land with the acceptance evidence below rather than on inspection alone.

## Consequences
- One definition of inferred-edge selection; the `inferred` flag the client renders means the same
  thing regardless of which path produced the edge.
- **Edge counts will drop** on deep hierarchies, because long-range grandparent edges currently
  emitted by the sync path are suppressed by the immediate-parent reduction. That is the intended
  correction, but it is a visible graph change and needs the shadow-sync comparison below.
- The per-child cap begins applying to the sync path, which today has no cap.
- Until this lands, `docs/BASELINE-architecture.md`'s data-pipeline description and diagram VC-20.4
  carry `PROPOSED ADR-2071:` notes recording that two implementations exist and differ.

## Acceptance test (required before `decision_status: accepted`)
1. `cargo test -p webxr inferred_edge` — existing module unit tests stay green, plus a new test
   asserting that a three-level hierarchy A→B→C yields exactly one inferred edge per child
   (A→B, B→C) and **no** A→C long-range edge from the sync path.
2. Shadow sync on the 2026-09-02 corpus, before and after, recording node/edge/`inferred`-edge counts
   (the EXP-V08 method already used in `docs/VAULT-corpus-format.md`): node count identical, inferred
   edge count lower, and every removed edge demonstrably a transitive ancestor of a retained one.
3. No change to `edge_is_inferred` semantics — `client_coordinator_actor.rs:2016` and
   `socket_flow_handler/types.rs:415` continue to classify the retained edges as inferred.

## Verification
Ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; must be re-run
at the landing commit. No code changed by this ADR — it records the decision and its entry criteria.

```
$ grep -rn "inferred_edge_materialiser" src/ --include=*.rs | grep -v "^src/services/inferred_edge_materialiser.rs"
src/actors/client_coordinator_actor.rs:2016:  ... inferred_edge_materialiser::edge_is_inferred(edge),
src/handlers/socket_flow_handler/types.rs:415: use crate::services::inferred_edge_materialiser::edge_is_inferred;
src/services/ontology_pipeline_service.rs:65:  ... DEFAULT_MAX_INFERRED_PARENTS_PER_CHILD,
src/services/ontology_pipeline_service.rs:478: use crate::services::inferred_edge_materialiser as mat;
src/services/mod.rs:34:pub mod inferred_edge_materialiser;

# the live Whelk path does NOT appear above — it hand-rolls selection instead:
$ sed -n '1186,1200p' src/services/github_sync_service.rs
        let mut inferred_edges = Vec::new();
        for axiom in &results.inferred_axioms {
            if axiom.axiom_type == AxiomType::SubClassOf
                && !axiom.subject.contains("owl#Nothing") ...
```

Corrects the Phase 1 report, which described this module as a dead duplicate: it is live, and the
duplication is on the other side.
