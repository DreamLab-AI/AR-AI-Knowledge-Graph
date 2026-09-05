---
id: ADR-2035
title: DAG-rank detection accepts the collapsed 'hierarchical' edge label
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
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
- The current function doc-comment agrees with acceptance. The existing predicate
  test still rejects the collapsed label; see the closeout extension for this
  unresolved test/producer-contract conflict.
- Governing-doc Invariant 7. See `docs/XR-client.md`.

## Verification

Re-verified at `eac01130`: `src/actors/gpu/force_compute_actor.rs:586` the `matches!`
set includes `"hierarchical" | "HIERARCHICAL"`; the doc-comment at `:576–579` —
previously stale, asserting the label was EXCLUDED — was corrected in this change to
state it IS accepted, so code and comment now agree.

## Closeout extension — 2026-09-04

CP-01/02/06/08. Owner remains jjohare with ingest/layout/XR maintainers. Complete/live is retained for the scoped label-accept implementation. The source comment now agrees with acceptance, but the existing predicate test still expects hierarchical to be rejected. The unchanged extracted predicate/test fails on that label. This reveals a contract/test conflict, not evidence that current deployed ingest has the wrong edges.

**Acceptance condition:** Ratify the collapsed label's producer semantics with subclass and domain-membership fixtures, reconcile the stale test, and verify orientation, mixed cyclic/disconnected inputs, rank upload and displayed layout. Reopen on ingest labels, edge provenance, ranking or folding changes. See [review](../../../VisionFlow/docs/estate-review/rendered-state.md#xr-control-coverage-and-hierarchy-semantics) and [extracted test receipt](../../../VisionFlow/docs/estate-review/evidence/xr-decision-probe.json). No full actor suite or GPU layout ran.

## Acceptance progress — 2026-09-05

**The stale test is reconciled.** The conflict was in the test, not the predicate:
`directed_hierarchy_relation_accepts_only_class_subsumption` asserted that
`hierarchical` must be rejected, contradicting both the implementation and this
ADR's accepted decision. The test is renamed
`directed_hierarchy_accepts_subsumption_and_the_collapsed_label` and now asserts
the ratified contract — explicit subclass labels **and** the collapsed
`hierarchical`/`HIERARCHICAL` label are accepted; symmetric relations
(`equivalent_class`, `same_as`), the separate property hierarchy
(`sub_property_of`) and membership-flavoured labels (`member_of`, `belongs_to`)
are not.

The reason the collapsed label is accepted is recorded in the test itself: it is
what this deployment's ingest writes for a subclass edge, matching the fold
endpoint, and without it DAG ranks stay unranked and the Radial: DAG / Hierarchy
layouts go silently inert. Rejecting the label its own ingest emits would restore
the very failure the accept was introduced to fix.

**The cost is recorded, not hidden.** Ratifying the collapsed label means a
producer reusing it for domain membership contributes edges ranked as if they were
subsumption. New fixtures make that explicit rather than leaving it as a caveat in
prose:

- `a_subclass_fixture_ranks_by_depth_from_its_root` — Entity → Animal → {Dog, Cat}
  under explicit subclass provenance; siblings share a layer.
- `a_domain_membership_fixture_ranks_identically_under_the_collapsed_label` —
  repo → dir → {file, file} under `hierarchical`, asserted to produce a rank vector
  **identical** to the subclass fixture. The predicate cannot separate them,
  because the label carries no provenance to separate them by. Distinguishing the
  two requires a producer-side label change; no consumer predicate can recover it.
- `mixed_subclass_and_membership_edges_share_one_rank_space` — when both producers
  write the collapsed label into one graph the ranker sees a single hierarchy, and
  shortest-depth multi-source BFS means a membership shortcut lifts a
  deeply-subsumed class up a layer.
- `nodes_outside_any_hierarchy_edge_stay_unranked` — rank `-1.0` is the opt-out
  from the radial bias; an empty hierarchy seeds nothing.
- `a_wholly_cyclic_hierarchy_is_seeded_deterministically` — a pure cycle has no
  natural root, so the lowest participating index is seeded. Rank is a layout
  projection, not a proof that the input is a DAG.

**Tests run.** `cargo test --lib --no-default-features dag_rank_tests` — 14 pass
(9 pre-existing, 5 new/renamed). The previously-failing extracted predicate case
now passes against the unchanged implementation.

**Governed paths changed.** `src/actors/gpu/force_compute_actor.rs` (test module
only; `is_directed_hierarchy_relation` and `compute_dag_ranks` are unmodified).

**Open.** The producer-side provenance question stands: a predicate match still
cannot establish that every collapsed edge represents subclass. Ratifying the
label is a decision about what this ingest emits, not evidence about it — actual
ingest fixtures from the live producer, rank-buffer upload and displayed layout
were not exercised, and no GPU layout ran. Complete/live is retained for the
scoped label-accept implementation.

## Re-verification — 2026-09-05 at b0bc275f6501aae7751b85a72ce15fe1e730e7e8


**Range note.** `bed6b617d..b0bc275f6` is `cargo fmt --all` plus the test-side
fixes that made `--all-targets` build; **no production logic changed**. Verified,
not assumed: comparing every changed file with all whitespace stripped leaves
only rustfmt artefacts — struct-literal reflow, import/module reordering and
added trailing commas. The largest single case,
`src/models/simulation_params.rs` (+303/-70 raw), is the `SIMPARAMS_MANIFEST`
literal reflowed one-field-per-line: its field names and byte offsets hash
identically on both sides. Citations below are
therefore re-derived line numbers over unchanged code, not new findings.

**Governed change since `eac011303`:** `src/actors/gpu/force_compute_actor.rs`,
carried in the `346fff7af` actor trim and the `da2f5cac7` GPU consolidation. The
predicate itself was not part of either refactor.

**The accept still stands.** `is_directed_hierarchy_relation` is at
`src/actors/gpu/force_compute_actor.rs:581`, and its `matches!` arm at `:587`
still reads
`"is_subclass_of" | "subclass_of" | "SUBCLASS_OF" | "hierarchical" | "HIERARCHICAL"`
(cited `:586` — the line moved by one). The doc-comment at `:576-580` still
states the generic label **IS** accepted, so code and comment continue to agree,
and the rationale comment naming this deployment's ingest is at `:582`. The
predicate's only consumer is unchanged at `:1247`.

**The test/producer-contract conflict recorded in Consequences is resolved.** The
Consequences bullet says "the existing predicate test still rejects the collapsed
label; see the closeout extension for this unresolved test/producer-contract
conflict." At HEAD that test no longer exists under its old name: it is
`directed_hierarchy_accepts_subsumption_and_the_collapsed_label` at `:4563`, and
it asserts the ratified contract rather than contradicting it. The bullet is now
historical; the conflict is closed, not merely re-described.

**Still open:** ratifying producer semantics with subclass *and*
domain-membership fixtures, and verifying rank upload and displayed layout, still
need ingest fixtures and a GPU layout run. Neither ran here. The risk the
Decision knowingly accepts — domain-membership edges reusing `"hierarchical"`
would fabricate ranks — is unchanged and remains the `review_trigger`.

**Commands run:** `git diff --stat eac011303..HEAD --
src/actors/gpu/force_compute_actor.rs`; `grep -n
'hierarchical|HIERARCHICAL|is_directed_hierarchy_relation'
src/actors/gpu/force_compute_actor.rs`; `cargo test --lib --no-default-features
hierarchy` → **8 passed, 0 failed** (1259 filtered out).
