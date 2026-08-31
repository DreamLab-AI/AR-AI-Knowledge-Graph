# ADR-099 — Reasoner Posture: Whelk-rs EL as Incremental Primary, DL Deep-Check Offline

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-05) |
| Drives | PRD-018 §5 WS-2, §6.2 |
| Companion ADRs | ADR-098 (constraint reuse), ADR-100 (canonical IRI), ADR-014 (semantic pipeline unification) |
| Affected paths | `crates/visionclaw-adapters/src/whelk_inference_engine.rs`, `src/services/ontology_pipeline_service.rs`, `src/services/github_sync_service.rs` (`run_post_sync_reasoning`), `urn:ngm:graph:ontology:inferred` named graph, `client/src/features/ontology/components/InferencePanel.tsx` |
| Evidence | `whelk_inference_engine.rs:86,260,290`, README (OWL 2 EL Whelk-rs), PRD-018 §3 |

## Context

VisionClaw runs the `jjohare/whelk-rs` fork as its reasoner. `lib.rs:10` claims EL++; the live extraction is narrower than even EL:

- `infer()` calls `reasoner_state.named_subsumptions()` and extracts **only** named class subsumptions (`whelk_inference_engine.rs:290`).
- `EquivalentClass` axioms are **downgraded to `SubClassOf`** with a warning (`:86-87`), losing the reverse direction.
- Disjointness, inverses, and equivalence inferences are never materialised back into the store.
- The `urn:ngm:graph:ontology:inferred` named graph exists but is under-populated, and the client `InferencePanel` that would surface inferences is built but never rendered.

Ontosphere, by contrast, runs a Konclude SROIQ(D) OWL 2 DL tableau in WASM and writes inferred triples to a separate named graph rendered as amber dashed edges with a reasoning report. That expressiveness is attractive, but Konclude is single-shot and requires a page reload to recover — incompatible with VisionClaw's incremental-at-scale sync of a 3,000+ page corpus.

The decision is which reasoner posture to commit to, and how much of the reasoning to materialise and surface, under the reuse directive (extend whelk + the existing inferred graph + the existing panel, do not adopt a new primary reasoner).

## Decision

### D1 — Whelk-rs EL stays the incremental primary

Whelk's value is incremental classification at corpus scale during sync. We keep it as the hot-path reasoner invoked by `run_post_sync_reasoning`. We do **not** adopt Konclude/OWL 2 DL as the primary reasoner: its single-shot, reload-to-recover model does not fit incremental GitHub sync, and the corpus is EL-shaped (subsumption hierarchies, no nominals/cardinality in the authored ontology).

### D2 — Stop downgrading `EquivalentClass`; materialise it bidirectionally

`EquivalentClass(A, B)` is preserved as an equivalence, not silently rewritten to a single `SubClassOf`. For the GPU mapper (ADR-098) it yields a colocate constraint; for the store it materialises both `A rdfs:subClassOf B` and `B rdfs:subClassOf A` (or `owl:equivalentClass`, preserved) into the inferred graph. The lossy `:86-87` path is removed.

### D3 — Materialise inferences into the existing inferred named graph with provenance

Extend extraction beyond `named_subsumptions` to also emit:
- inferred `rdfs:subClassOf` (transitive closure already available),
- preserved `owl:equivalentClass` (D2),
- entailed disjointness violations where derivable in EL.

All inferred quads land in `urn:ngm:graph:ontology:inferred` (reuse — the graph already exists), each tagged with provenance (`prov:wasGeneratedBy` whelk run id) and a confidence/derivation marker so the asserted vs inferred distinction is queryable and the "clear inferred" idempotent toggle is a single-graph `CLEAR`.

### D4 — Reasoning report + visual differentiation via the existing panel

Surface a reasoning report (list of inferred triples with their justification class) as data the **already-built** `InferencePanel` reads, and render inferred edges distinctly (dashed/amber, ontosphere's pattern) in the R3F/Babylon client. The client work mounts the orphaned panel (WS-2/WS-4) rather than building a new one.

### D5 — Optional offline OWL 2 DL deep-check, out of the hot path

A DL consistency audit (Konclude WASM or an `elk`/`horned-owl` path) is permitted as an **offline, on-demand** job for consistency checking and richer entailments. It is explicitly **not** wired into incremental sync and never blocks ingest. It produces a report only; it does not feed the GPU constraint path. This keeps DL expressiveness available for audits without compromising incremental scale. Scope and trigger are deferred to a follow-up; this ADR only fixes that DL is audit-only.

## Consequences

**Positive:**
- Reasoning becomes visible and trustworthy: equivalence preserved, inferences materialised, both queryable and rendered — closing the largest "parsed but not reasoned" gap.
- All reuse: extends whelk, fills the existing inferred graph, mounts the existing panel. No new reasoner on the hot path.
- The inferred/asserted split is a named-graph boundary, so it composes cleanly with ADR-101 migrations and ADR-098 constraint sourcing.

**Negative / risks:**
- Materialising inferences grows the store; mitigated by confining them to the inferred graph and making "clear inferred" a one-graph `CLEAR`.
- EL cannot derive everything DL can (no nominal/cardinality entailments); D5's offline audit is the escape hatch, and the corpus is EL-shaped so the gap is small in practice.
- Provenance tagging adds quads per inference; acceptable and bounded by inference count.

## Verification

- Unit: `EquivalentClass(A,B)` round-trips bidirectionally (no downgrade); inferred subsumptions land in `urn:ngm:graph:ontology:inferred` with provenance.
- Integration: post-sync reasoning materialises ≥ the previously-extracted subsumptions plus equivalences; "clear inferred" empties only the inferred graph.
- Client: `InferencePanel` renders the report; inferred edges are visually distinct with a toggle (browser-verified).
