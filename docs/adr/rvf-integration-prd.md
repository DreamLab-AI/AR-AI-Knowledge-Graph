# PRD: RVF (RuVector Format) Integration into VisionClaw

**Status**: Draft
**Author**: Architecture Team
**Date**: 2026-02-14
**Stakeholders**: VisionClaw Core, Agent Platform, DevOps

---

## 1. Problem Statement

VisionClaw's knowledge graph visualization lacks vector-based semantic intelligence. Three specific deficiencies exist:

1. **No vector similarity search**: The `SemanticAnalyzer` (`src/services/semantic_analyzer.rs`) uses O(N^2) keyword-based cosine similarity over `HashMap<String, usize>` topic counts. This is not learned embeddings -- it is word counting. At 10k+ nodes, this is the computational bottleneck in semantic force computation (`src/physics/semantic_constraints.rs` falls back to LSH at 500+ nodes).

2. **No client-side semantic capability**: Embeddings never cross the wire. The binary protocol carries position+velocity only (24 bytes/node, see [docs/binary-protocol.md](../reference/binary-protocol.md)); analytics (`cluster_id`, `anomaly_score`, `community_id`) ride the separate `analytics_update` JSON channel at recompute cadence. Neither channel carries embedding data, so the client cannot perform similarity search, semantic filtering, or embedding-based clustering without a server round-trip.

3. **Agent memory is infrastructure-coupled**: 1.17M+ memory entries in an external PostgreSQL container (`ruvector-postgres:5432`) with pgvector + HNSW. This requires a running database server, has connection reliability issues (30-retry loop in entrypoint), and creates a data fragmentation risk between external and local fallback schemas.

## 2. Proposed Solution

Integrate the RVF (RuVector Format) file-based vector storage into VisionClaw at three levels:

| Level | Component | RVF Crate | Replaces |
|-------|-----------|-----------|----------|
| **Server-side HNSW** | Rust backend semantic analysis | `rvf-index` + `rvf-runtime` | Manual cosine similarity in SemanticAnalyzer |
| **Client-side search** | Browser WASM vector queries | `rvf-runtime` (wasm target) | Nothing (new capability) |
| **Agent memory** | File-based agent knowledge | `rvf-runtime` + `rvf-import` | PostgreSQL dependency for agent memory |

## 3. User Stories

### 3.1 Server-Side Semantic Search (P0)

**As a** VisionClaw user exploring a knowledge graph
**I want** semantically similar nodes to cluster together and be discoverable by meaning
**So that** I can find related concepts even when they don't share keywords

**Acceptance Criteria:**
- `OntologyQueryService::discover()` returns results ranked by embedding similarity, not keyword match
- Semantic forces in physics layout use HNSW nearest-neighbor lookups instead of O(N^2) all-pairs
- HNSW query latency < 1ms for 50k vectors (vs current ~500ms for 5k nodes with brute force)
- The `ruvectorEnabled` quality gate toggle activates the new HNSW path
- Graceful fallback to keyword matching when RVF index is unavailable

### 3.2 Client-Side Similarity Search (P1)

**As a** user interacting with a rendered graph
**I want** to click a node and instantly see its most similar neighbors highlighted
**So that** I can explore semantic relationships without waiting for server responses

**Acceptance Criteria:**
- An `.rvf` file containing node embeddings + HNSW index is served to the client
- WASM runtime loads the file and exposes `query(embedding, k)` API
- Query latency < 5ms in browser for 50k vectors
- Results feed into the existing selection highlight pipeline (selectionHighlightColor, selectionEdgeFlow)
- Works offline once the `.rvf` file is cached

### 3.3 Graph Snapshot Export/Import (P1)

**As a** user or system operator
**I want** to export and import complete graph state as a single `.rvf` file
**So that** graphs can be shared, versioned, and loaded without a running backend

**Acceptance Criteria:**
- Export produces a single `.rvf` file containing: node embeddings (VEC_SEG), graph topology (GRAPH), node metadata (META), HNSW index (INDEX_SEG)
- Import reconstructs the full graph state from the file
- File includes lineage hash for version tracking
- File size < 2x the raw data size (compression + index overhead)

### 3.4 Decoupled Agent Memory (P2)

**As a** platform operator
**I want** agent memory to work without an external PostgreSQL server
**So that** the system is self-contained and deployable as a single artifact

**Acceptance Criteria:**
- Agent memory entries (1.17M+) stored in `.rvf` files with 384-dim HNSW index
- Read/write latency comparable to PostgreSQL (<10ms for similarity search)
- Concurrent agent access supported (append-only crash safety)
- Migration path from existing PostgreSQL data to `.rvf` files
- Backward-compatible: PostgreSQL backend remains available as an option

## 4. Non-Goals

- **Replace Neo4j for graph storage**: Neo4j remains the source of truth for ontology structure, OWL class hierarchy, and Cypher queries. RVF supplements it with vector operations.
- **Replace CUDA GPU physics**: The force-directed layout engine stays on GPU. RVF provides pre-computed embeddings that feed into semantic forces, not the force computation itself.
- **Build a custom embedding model**: Embeddings will be generated by external models (all-MiniLM-L6-v2 for 384-dim, or domain-specific models). RVF stores and indexes them.
- **Production deployment of npm packages**: The `@ruvector/rvf-wasm` npm package is a stub (1.1KB, no WASM binary). Client-side integration will compile from Rust source via `wasm-pack`, not use the npm package.

## 5. Technical Constraints

### 5.1 Ecosystem Maturity
- All RVF crates published 2026-02-14 (v0.1.0, 27 total downloads)
- Known HNSW segfault on >100K rows (GitHub issue #164)
- npm packages are non-functional stubs
- Single maintainer (`ruvnet`)
- API explicitly unstable at 0.1.0

### 5.2 Mitigation Strategy
- **Feature-flagged**: All RVF integration behind `ruvectorEnabled` quality gate (already plumbed)
- **rvf-index only first**: Start with the zero-dependency HNSW crate (2,045 lines), not the full runtime
- **Compile from source**: Build WASM from Rust source, don't depend on npm artifacts
- **Fallback paths**: Every RVF-powered feature falls back to the existing implementation
- **Pin to commit hash**: Don't depend on crates.io versions; use git dependency with pinned rev

### 5.3 Compatibility
- Rust: VisionClaw toolchain 1.93.0 > RVF MSRV 1.87 (compatible)
- Edition: Both 2021
- WASM: VisionClaw already uses `vite-plugin-wasm` + `wasm-bindgen` (proven pipeline)
- Serialization: Both use `serde 1.x`

## 6. Success Metrics

| Metric | Current | Target | Method |
|--------|---------|--------|--------|
| Semantic discovery latency (5k nodes) | ~500ms (brute force) | <5ms (HNSW) | Server-side benchmark |
| Semantic force computation (10k nodes) | ~2s per tick (O(N^2)) | <10ms per tick (k-NN) | Physics actor profiling |
| Client-side similarity query | N/A (not possible) | <5ms (WASM HNSW) | Browser performance API |
| Graph snapshot load time | ~3s (REST JSON parse) | <500ms (.rvf binary load) | Client timing |
| Agent memory without PostgreSQL | Not possible | Functional | Integration test |
| `ruvectorEnabled` toggle | Non-functional | Gates real HNSW path | E2E test |

## 7. Phased Delivery

### Phase 1: Server-Side HNSW (2-3 weeks)
- Add `rvf-index` to Cargo.toml (git dep, pinned rev)
- Implement `RvfSemanticAnalyzer` behind `SemanticAnalyzer` port
- Wire to `ruvectorEnabled` quality gate
- Benchmark against current brute-force at 1k, 5k, 10k, 50k nodes

### Phase 2: Graph Snapshot Format (1-2 weeks)
- Define VisionClaw `.rvf` profile (which segments, schema)
- Export endpoint: `/api/graph/export.rvf`
- Import endpoint: `/api/graph/import`
- CLI tool for offline conversion

### Phase 3: Client-Side WASM Search (2-3 weeks)
- `wasm-pack build` rvf-runtime for browser target
- Bridge class following `scene-effects-bridge.ts` pattern
- Wire to node click → similarity highlight
- Progressive loading (query at 70% accuracy while indexing)

### Phase 4: Agent Memory Migration (3-4 weeks)
- `rvf-import` for PostgreSQL → .rvf migration
- MCP memory tools backed by .rvf store
- Concurrent access protocol (append-only + compaction)
- Backward compatibility with PostgreSQL path

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| HNSW segfault (#164) at >100K vectors | Medium | High | Start with <50K; monitor issue; implement bounds checking |
| API breaking changes at 0.1.0 | High | Medium | Pin to git commit; vendor if needed |
| WASM compilation fails for rvf-runtime | Medium | Medium | Already proven with scene-effects WASM; same toolchain |
| Performance worse than PostgreSQL HNSW | Low | High | Benchmark before migrating agent memory; keep PG as fallback |
| Single maintainer abandons project | Low | High | Vendor critical crates; rvf-index is 2K lines, maintainable |

## 9. Dependencies

- Embedding model for generating 384-dim vectors (all-MiniLM-L6-v2 or equivalent)
- `wasm-pack` for client-side WASM compilation
- Resolution of RVF issue #164 (HNSW segfault) before Phase 4
