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
The four ontology graph roles relevant to this fence are `:assert`, `:inferred`, `:summary`, `:observed`.

## Decision

The derived write path (`append_derived_quads`) accepts quads targeting
**only** `:summary` and `:observed`. Any quad naming a `DERIVED_FENCE` graph
(`:assert` or `:inferred`) is rejected inside the repository method itself, and
any quad naming a graph other than `:summary`/`:observed` is rejected as well.
This is defence-in-depth: the fence holds even if the handler check is removed
or bypassed. No caller can write authoritative or reasoner-derived triples
through this path; the asserted graph also has governed runtime writers, and full-sync rebuild
can replace their content unless it has reached the corpus (ADR-2017). The
inferred graph is a separately managed reasoner projection.

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

## Closeout extension — 2026-09-04

**Work package:** CP-02 / CP-04. **Owner:** existing owner above. Dependencies are
CP-01 revision/ownership mapping and the relevant corpus or authority contract.

**Current evidence:** The repository method rejects forbidden graph names and unsafe IRIs before submitting its combined update. The fence is scoped to this method. Other governed runtime writers can update asserted ontology, as the sync source explicitly records.

See [runtime analysis](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/visionclaw-data-runtime.md),
[source hashes](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/evidence/visionclaw-data-snapshot.json)
and [backup receipt](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/evidence/visionclaw-backup-probe.json).
Source was inspected at `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`. Earlier verification at `e0f8cd896`
remains historical evidence; this annex does not claim a new deployed activation
or complete verification of every older assertion.

**Acceptance still required:** Test mixed permitted/forbidden batches, unsafe IRIs and direct repository calls without the HTTP handler. Document each asserted/inferred writer and preserve the fence when adding routes.

## Acceptance progress — 2026-09-05

**Implemented (tests only; the fence itself was already correct).**
`crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs`. The acceptance
asked for mixed permitted/forbidden batches, unsafe IRIs and direct repository
calls without the HTTP handler; all three are now executable.

The mixed-batch test is the load-bearing one: a batch containing a legitimate
`:summary` quad, a fenced-graph quad and a legitimate `:observed` quad is
rejected **whole**, and the permitted quads are not written — so a caller cannot
smuggle a write past the fence by burying it among valid ones. The allow-list is
asserted to be an allow-list, not a deny-list: `:knowledge`, `:agent`,
`:shapes`, `:provenance` and an invented graph name are all refused, so a graph
added later is refused by default rather than accepted by omission.

**Tests.** `cargo test -p visionclaw-adapters --lib` — 76 passed, 0 failed
(9 new): permitted graphs accept; mixed batch rejected whole for each fenced
graph; each fenced graph refused alone; unlisted graphs refused; unsafe
subject/predicate/object IRIs rejected with the batch writing nothing; a hostile
literal escaped rather than executed, with the asserted graph verified empty;
the fence holding on a direct repository call with no handler in the picture;
empty batch a no-op; and the fence constant covering exactly `:assert` and
`:inferred`.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2015-2016-adapters.txt`.

**Remains open.** The other governed runtime writers that can update asserted
ontology are still not individually documented, and preserving the fence when
adding routes is a review obligation, not a test.
