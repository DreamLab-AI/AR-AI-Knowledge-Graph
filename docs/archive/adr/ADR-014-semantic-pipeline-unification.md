# ADR-014: Semantic Pipeline Unification

**Date**: 2026-03-25
**Status**: ACCEPTED
**Decision**: Wire all existing semantic pipeline components into a single data flow. No new systems. No backward compatibility. No fallbacks.

## Context

The VisionClaw codebase contains a complete semantic pipeline — parsers, Neo4j adapters, GPU actors, CUDA kernels, constraint translators, binary protocol fields — built over months but never connected end-to-end. 7 data loss points cause 8/9 relationship types to be dropped, 110K axioms to sit isolated, and GPU analytics to return zeros.

## Decision

**Single pass, single sprint. Delete all fallback paths. Wire source→sink directly.**

### Principles
1. **Markdown is truth** — every relationship in an OntologyBlock becomes a Neo4j edge
2. **No fallback edge generation** — delete `generate_edges_from_metadata()` and `generate_edges_from_labels()`
3. **No dual-path loading** — `load_graph()` loads ONE unified graph (KGNode + OwlClass edges combined)
4. **Edge type flows to GPU** — CSR carries edge_type buffer alongside col_indices
5. **Analytics flow back** — ClusteringActor writes results, binary protocol carries them, client reads them

> **Correction (2026-07-22 doc-drift audit).** This ADR's Context ("Neo4j
> adapters") and Principle 1 ("every relationship in an OntologyBlock becomes
> a Neo4j edge") describe Neo4j as the existing/target persistence. Neo4j is
> 100% removed (ADR-132); the sole store is Oxigraph named graphs + SQLite.
> Read "Neo4j adapters" / "Neo4j edge" above as "Oxigraph adapters" /
> "Oxigraph triple".
6. **Edge colour = relationship power** — gradient from source domain colour to target domain colour, weighted by relationship strength

### What We Delete
- `generate_edges_from_metadata()` (dead code, never called)
- `generate_edges_from_labels()` (fallback that produces low-quality namespace-only edges)
- `if iri_to_id.is_empty()` branch in `load_graph()` (either/or path selection)
- `app_state.node_analytics` empty HashMap pattern (replace with write path)

### What We Wire
- `neo4j_ontology_repository::add_owl_class()` → store ALL relationship types as `:RELATES` edges
- `neo4j_adapter::load_graph()` → single query: EDGE + SUBCLASS_OF + RELATES
- `force_compute_actor` → CSR with `edge_types: DeviceBuffer<u8>`
- `clustering_actor` → `ClientCoordinatorActor` → `analytics_update` JSON message (post-[ADR-061](ADR-061-binary-protocol-unification.md); historically rode the per-frame binary)
- `ontology_constraint_actor` → `apply_ontology_constraints()` (remove dead_code annotation)
- `semantic_forces_actor` → receive `source_domain` as type_id, activate type clustering

### Edge Colour Model
Edges render as gradient tubes:
- **Source end**: domain colour of source node (AI=#4FC3F7, BC=#81C784, MV=#CE93D8, etc.)
- **Target end**: domain colour of target node
- **Width**: `edge.weight` (hierarchical=2.5, structural=1.5, dependency=1.5, associative=1.0, bridge=1.0)
- **Opacity**: `relationship_power = weight * (1 + log(edge_count_between_pair))`
- Cross-domain edges create visible colour gradients between domain clusters

## Consequences

- Breaking change: edge count jumps from ~490 to ~1,500+
- Physics will need re-settle with new edge topology
- Cluster hulls will naturally separate into domain groups
- Client edge rendering needs gradient material (already partially in `GlassEdges` with `useGradient: true`)

## Alternatives Considered

- **Keep fallback paths**: Rejected — they mask data quality issues and add code complexity
- **New edge storage system**: Rejected — existing `:EDGE`/`:RELATES`/`:SUBCLASS_OF` pattern is sufficient
- **Client-side edge synthesis**: Rejected — server is source of truth, client should only render
