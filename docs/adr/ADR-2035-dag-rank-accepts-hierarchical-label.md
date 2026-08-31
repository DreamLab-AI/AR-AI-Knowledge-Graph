---
id: ADR-2035
title: DAG-rank detection accepts the collapsed 'hierarchical' edge label
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/actors/gpu/force_compute_actor.rs]
owner: jjohare
review_trigger: an ingest change that stops collapsing subclass provenance to the generic 'hierarchical' label, or reintroduces domain-membership edges under that same label
repo: visionclaw
domain: XR-client
lineage: distils legacy ADR-141 (constrained-layout engine) and ADR-138 (GPU force-channel registry); label-accept landed 73540faa0, stale doc-comment corrected eac01130
---

# ADR-2035 — DAG-rank detection accepts the collapsed 'hierarchical' edge label

## Context

`compute_dag_ranks` ranks nodes only along edges that
`is_directed_hierarchy_relation` accepts. This deployment's ingest collapses subclass
provenance to a generic `"hierarchical"` label rather than emitting explicit
`subclass_of`. With that label rejected, no edge qualifies, every node stays unranked,
and Radial: DAG plus the Hierarchy toggle are silently inert. The same collapsed label
also feeds the fold endpoint (fold.rs).

## Decision

`is_directed_hierarchy_relation` accepts `"hierarchical"` / `"HIERARCHICAL"` alongside
the explicit `is_subclass_of` / `subclass_of` / `SUBCLASS_OF` provenance. This is a
deployment-specific accept keyed to how our ingest writes edges; it forecloses treating
the collapsed label as non-hierarchical. The risk it accepts: if domain-membership
edges ever reuse `"hierarchical"`, ranks would be fabricated from non-subclass
structure — that is the trade this deployment takes because its ingest does not do so.

## Consequences

- Radial: DAG and the Hierarchy toggle rank correctly on the deployed graph.
- The accept is coupled to ingest behaviour; a change to how ingest labels edges can
  silently over- or under-rank.
- The function's doc-comment (`:576–579`) still asserts `"hierarchical"` is EXCLUDED —
  stale and contradicted by the accept at `:586`. The inline comment at `:581–583` is
  authoritative; the doc-comment should be corrected on next touch.
- Governing-doc Invariant 7. See `docs/XR-client.md`.

## Verification

Re-verified at `eac01130`: `src/actors/gpu/force_compute_actor.rs:586` the `matches!`
set includes `"hierarchical" | "HIERARCHICAL"`; the doc-comment at `:576–579` —
previously stale, asserting the label was EXCLUDED — was corrected in this change to
state it IS accepted, so code and comment now agree.
