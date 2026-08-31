---
id: ADR-2015
title: Derived-writeback fence — only `:summary`/`:observed` are writable
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: addition of a new derived named graph, or any caller needing to write `:assert`/`:inferred` through a non-sync path
repo: visionclaw
domain: DATA-authority-erasure
lineage: legacy WS-9 derived-graph work-stream, ADR-099 (inferred-graph lifecycle / clear-inferred)
---

# ADR-2015 — Derived-writeback fence — only `:summary`/`:observed` are writable

## Context

Enrichment and observation flows need a write path into the triple store, but
must never be able to forge authoritative asserted axioms (`:assert`) or
reasoner output (`:inferred`). A handler-level check alone is insufficient:
any future caller bypassing the handler could corrupt the authoritative graphs.
Four named graphs exist — `:assert`, `:inferred`, `:summary`, `:observed`.

## Decision

The derived write path (`append_derived_quads`) accepts quads targeting
**only** `:summary` and `:observed`. Any quad naming a `DERIVED_FENCE` graph
(`:assert` or `:inferred`) is rejected inside the repository method itself, and
any quad naming a graph other than `:summary`/`:observed` is rejected as well.
This is defence-in-depth: the fence holds even if the handler check is removed
or bypassed. No caller can write authoritative or reasoner-derived triples
through this path; `:assert` is only rebuilt by the GitHub sync (ADR-2017) and
`:inferred` only by the reasoner.

## Consequences

- Enrichment cannot escalate into asserted knowledge; a compromised or buggy
  derived caller can at worst pollute the two disposable derived graphs.
- Two enforcement points (handler + repo) must stay in sync conceptually, a
  small duplication cost accepted for the safety guarantee.
- Adding a new writable derived graph requires editing the allow-list in the
  repo method, not just the handler — deliberate friction.

## Verification

Re-checked at `e0f8cd896`: `oxigraph_ontology_repository.rs` defines
`const DERIVED_FENCE: [&str; 2] = [GRAPH_ONTOLOGY, GRAPH_ONTOLOGY_INFERRED]`
(`:assert`, `:inferred`). In `append_derived_quads`, each quad is rejected if
its graph is in `DERIVED_FENCE`, and again rejected if it is neither
`GRAPH_ONTOLOGY_SUMMARY` nor `GRAPH_ONTOLOGY_OBSERVED`; only then does it build
the `INSERT DATA` into `:summary`/`:observed`.
