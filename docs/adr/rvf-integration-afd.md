# AFD: RVF Integration Architecture Fitness Document

**Status**: Draft
**Date**: 2026-02-14
**Scope**: VisionClaw server + client + agent platform

> **Correction (2026-07-22 doc-drift audit).** "Neo4j remains the source of
> truth" is false (see lines 24-25, 93-94 below: `Neo4jAdapter` /
> `Neo4jOntology Repository` do not exist on `main`). Ontology/graph source of
> truth is Oxigraph (`oxigraph_ontology_repository.rs`). RVF supplements
> Oxigraph, not Neo4j. Status is still Draft, ~5mo stale — see
> `KNOWN_ISSUES.md` AGENT-001 and ADR-131.

---

## 1. Architecture Context

### 1.1 Current System Architecture

```mermaid
graph TD
    subgraph Server["VisionClaw Server (Actix-web 4.11)"]
        subgraph Ports["Ports"]
            KGR["KnowledgeGraph Repository"]
            OR["Ontology Repository"]
            IE["Inference Engine"]
            SA["Semantic Analyzer"]
            SR["Settings Repository"]
        end
        subgraph Adapters["Adapters"]
            NA["Neo4jAdapter"]
            NO["Neo4jOntology Repository"]
            WI["WhelkInference Engine"]
            GS["GPUSemantic Analyzer (O(N^2) keyword)"]
            NS["Neo4jSettings Repository"]
        end
        subgraph GPU["GPU Actor System (CUDA)"]
            FC["ForceCompute"]
            CL["Clustering"]
            AN["Anomaly"]
            PR["PageRank"]
            SSSP["SSSP"]
            SF["SemanticForces"]
            COM["Communities"]
        end
        KGR --> NA
        OR --> NO
        IE --> WI
        SA --> GS
        SR --> NS
    end

    subgraph Transport["Transport Layer"]
        REST["REST: GET /graph/data (JSON, full graph)"]
        WS["WS: Binary V3 (48 bytes/node @ 60Hz)"]
    end

    subgraph Client["VisionClaw Client (React 19 + Three.js r182)"]
        GDM["graphDataManager -> graph.worker (Web Worker)"]
        SAB["SharedArrayBuffer (positions)"]
        RENDER["GemNodes (InstancedMesh) + GlassEdges<br/>WasmSceneEffects (WASM particles/wisps)<br/>ClusterHulls"]
        GDM --> SAB --> RENDER
    end

    subgraph RuVector["External: RuVector PostgreSQL"]
        RV["ruvector-postgres:5432<br/>1.17M+ memory_entries (384-dim HNSW)<br/>Agent memory, reasoning patterns, SONA<br/>NOT connected to VisionClaw Rust backend"]
    end

    Server --> Transport --> Client
```

### 1.2 Key Architectural Properties

| Property | Current State | Fitness |
|----------|--------------|---------|
| **Hexagonal ports** | 5 port traits with adapter implementations | Excellent -- new adapters drop in |
| **WASM bridge pattern** | Proven via scene-effects-bridge.ts | Excellent -- reusable for RVF |
| **Binary wire protocol** | V3 at 48 bytes/node, V4 delta ready | Good -- can carry embedding refs |
| **Server-authoritative** | All layout on GPU, client only interpolates | Good -- RVF supplements, doesn't replace |
| **Feature gating** | `ruvectorEnabled` plumbed through full stack | Excellent -- integration point exists |
| **Embedding generation** | None (keyword counting only) | Poor -- must add embedding pipeline |

---

## 2. Target Architecture

### 2.1 RVF Integration Points

```mermaid
graph TD
    subgraph Server2["VisionClaw Server (Target)"]
        subgraph Ports2["Ports"]
            KGR2["KnowledgeGraph Repository"]
            OR2["Ontology Repository"]
            IE2["Inference Engine"]
            SA2["Semantic Analyzer (port)"]
            VS2["+NEW: VectorStore (port)"]
        end
        subgraph Adapters2["Adapters"]
            NA2["Neo4jAdapter"]
            NO2["Neo4jOntology Repository"]
            WI2["WhelkInference Engine"]
            RSA["RvfSemantic Analyzer (HNSW k-NN, rvf-index)"]
            RVS["RvfVector Store (rvf-runtime)"]
        end
        subgraph Pipeline["RVF Export Pipeline"]
            EXP["Neo4j nodes -> embed -> .rvf file<br/>Serves: /api/graph/vectors.rvf"]
        end
        KGR2 --> NA2
        OR2 --> NO2
        IE2 --> WI2
        SA2 --> RSA
        VS2 --> RVS
    end

    subgraph Transport2["Transport Layer"]
        REST2["REST: /graph/data (JSON) + /graph/vectors.rvf (binary)"]
        WS2["WS: Binary V3 (48 bytes/node @ 60Hz)"]
    end

    subgraph Client2["VisionClaw Client"]
        subgraph WASM["RvfVectorBridge (WASM)"]
            LOAD["loads vectors.rvf"]
            QUERY["WasmRvfStore.query(embedding, k) -> similar node IDs -> highlight"]
            PAT["Pattern: scene-effects-bridge.ts"]
        end
        GDM2["graphDataManager -> graph.worker"]
        RENDER2["GemNodes + GlassEdges + ClusterHulls"]
    end

    Server2 --> Transport2 --> Client2
```

### 2.2 New Port: VectorStore

```rust
// src/ports/vector_store.rs

#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert embeddings for nodes
    async fn upsert(&self, ids: &[u32], embeddings: &[Vec<f32>]) -> Result<()>;

    /// Find k nearest neighbors by embedding
    async fn query(&self, embedding: &[f32], k: usize) -> Result<Vec<(u32, f32)>>;

    /// Find k nearest neighbors for a node by its ID
    async fn query_by_id(&self, node_id: u32, k: usize) -> Result<Vec<(u32, f32)>>;

    /// Export current state to .rvf file
    async fn export(&self, path: &Path) -> Result<()>;

    /// Load from .rvf file
    async fn load(&self, path: &Path) -> Result<()>;

    /// Number of indexed vectors
    fn count(&self) -> usize;

    /// Vector dimension
    fn dimension(&self) -> usize;
}
```

### 2.3 Adapter: RvfVectorStore

```rust
// src/adapters/rvf_vector_store.rs

use rvf_index::HnswIndex;
use rvf_runtime::RvfStore;

pub struct RvfVectorStore {
    index: HnswIndex,
    store: Option<RvfStore>,  // None until first export/load
    dimension: usize,
    embeddings: Vec<Vec<f32>>,  // In-memory backing
}

impl VectorStore for RvfVectorStore {
    // HNSW-backed implementation
    // Falls back to brute-force if index build fails
}
```

---

## 3. Component Fitness Assessment

### 3.1 Server-Side Integration

| Component | Integration Approach | Risk | Effort |
|-----------|---------------------|------|--------|
| `SemanticAnalyzer` port | New `RvfSemanticAnalyzer` adapter using `rvf-index` HNSW | Low | 3-5 days |
| `SemanticForcesActor` | Replace O(N^2) similarity with k-NN lookup from VectorStore | Low | 2-3 days |
| `OntologyQueryService::discover()` | Augment keyword search with embedding similarity ranking | Medium | 3-5 days |
| `SemanticPathfindingService` | Use embedding distance as edge weight heuristic | Low | 1-2 days |
| `FileService` import pipeline | Generate embeddings during markdown import, store in .rvf | Medium | 5-7 days |
| Export endpoint | New `/api/graph/vectors.rvf` serving pre-built .rvf file | Low | 2-3 days |

### 3.2 Client-Side Integration

| Component | Integration Approach | Risk | Effort |
|-----------|---------------------|------|--------|
| WASM bridge | `RvfVectorBridge` class following `scene-effects-bridge.ts` pattern | Low | 3-5 days |
| File loading | Fetch `/api/graph/vectors.rvf`, pass to WASM store | Low | 1 day |
| Node similarity highlight | Click node → query WASM → highlight k-NN results | Medium | 3-5 days |
| Semantic search UI | Search box → embed query → WASM k-NN → filter/highlight | Medium | 5-7 days |
| Progressive loading | Start queries before full index built | Medium | 2-3 days |

### 3.3 Embedding Pipeline (New Component)

Currently no embeddings exist in VisionClaw. The embedding pipeline must be built:

| Approach | Method | Latency | Quality |
|----------|--------|---------|---------|
| **Server-side Rust** | `rust-bert` or `candle` with all-MiniLM-L6-v2 | 10-50ms/node | High |
| **External API** | OpenAI embeddings API (already configured) | 100-500ms/batch | High |
| **Lightweight hash** | Extend existing topic_counts to fixed-dim via SimHash | <1ms/node | Low |
| **Hybrid** | SimHash for initial layout, API for refined embeddings | Varies | Medium→High |

**Recommendation**: Start with SimHash (already partially implemented in `semantic_constraints.rs` LSH) for Phase 1 prototype, upgrade to learned embeddings in Phase 2.

---

## 4. Data Flow: RVF Export Pipeline

```
Neo4j GraphData
    │
    ├── nodes: Vec<Node> with metadata
    │       │
    │       ▼
    │   Embedding Generator
    │       │  (SimHash or API)
    │       ▼
    │   Vec<(u32, Vec<f32>)>  ── node_id → 384-dim embedding
    │       │
    │       ▼
    │   rvf-index::HnswIndex::build()
    │       │
    │       ▼
    │   rvf-runtime::RvfStore::create()
    │       │
    │       ├── VEC_SEG:   embeddings (f32 × dim × N)
    │       ├── INDEX_SEG: HNSW graph (layers + neighbors)
    │       ├── META:      node metadata (quality, authority, type)
    │       └── GRAPH:     adjacency list (edges)
    │
    ▼
  vectors.rvf  ──→ served at /api/graph/vectors.rvf
                    │
                    ▼ (browser fetch)
                    │
              Client WASM runtime
                    │
                    ├── WasmRvfStore.query(embedding, k=10)
                    │       → [(node_id, distance), ...]
                    │
                    └── Feed into selection/highlight pipeline
```

---

## 5. Dependency Management

### 5.1 Rust Dependencies (Cargo.toml additions)

```toml
[dependencies]
# RVF core -- pinned to git commit for stability
rvf-types = { git = "https://github.com/ruvnet/ruvector", rev = "PINNED_COMMIT" }
rvf-index = { git = "https://github.com/ruvnet/ruvector", rev = "PINNED_COMMIT" }
rvf-runtime = { git = "https://github.com/ruvnet/ruvector", rev = "PINNED_COMMIT", optional = true }

[features]
default = []
rvf = ["rvf-runtime"]  # Full RVF support behind feature flag
```

### 5.2 Client Dependencies

No npm packages. WASM compiled from source:

```bash
# Build RVF WASM module from Rust source
cd crates/rvf/rvf-runtime
wasm-pack build --target web --features wasm
# Output: pkg/rvf_runtime_bg.wasm + rvf_runtime.js
```

Integrated into Vite build via existing `vite-plugin-wasm` configuration.

### 5.3 Why Not npm Packages

- `@ruvector/rvf-wasm`: 1.1KB stub, no actual WASM binary
- `@ruvector/rvf-node`: References N-API binaries not included in package
- `@ruvector/rvf-mcp-server`: Broken `workspace:*` dependency
- All published today with zero downloads

---

## 6. Quality Gates

### 6.1 Phase 1 Gate (Server HNSW)

| Criterion | Threshold | Test |
|-----------|-----------|------|
| HNSW build time (10k vectors, 384-dim) | < 2s | Benchmark test |
| HNSW query time (k=10) | < 1ms | Benchmark test |
| Memory overhead vs brute force | < 3x | Memory profiling |
| No segfaults at test scale | 0 crashes in 1000 queries | Stress test |
| Recall@10 vs brute force | > 95% | Accuracy test |
| `ruvectorEnabled=false` regression | Zero behavior change | E2E test |
| `cargo test` passes | 100% | CI |

### 6.2 Phase 2 Gate (Graph Snapshot)

| Criterion | Threshold | Test |
|-----------|-----------|------|
| .rvf file size vs raw data | < 2x | Size comparison |
| Export time (10k nodes) | < 5s | Benchmark |
| Import/load time (10k nodes) | < 1s | Benchmark |
| Round-trip fidelity | Lossless for embeddings + metadata | Comparison test |
| Crash safety | File readable after interrupted write | Kill-during-write test |

### 6.3 Phase 3 Gate (Client WASM)

| Criterion | Threshold | Test |
|-----------|-----------|------|
| WASM module size | < 200KB gzipped | Build output |
| Browser query latency | < 5ms for k=10 on 50k vectors | Performance API |
| Memory usage in browser | < 100MB for 50k × 384 | DevTools profiling |
| No main-thread blocking | Loads in worker or async | Lighthouse audit |
| Works in Chrome, Firefox, Safari | All three | Cross-browser test |

---

## 7. Failure Modes and Fallbacks

| Failure | Detection | Fallback |
|---------|-----------|----------|
| rvf-index segfault (#164) | Process crash signal | Restart with `ruvectorEnabled=false`; log + alert |
| .rvf file corrupt | CRC check on load | Re-export from Neo4j; serve stale cached version |
| Embedding generation fails | Error in embedding pipeline | Use zero vectors; disable semantic forces |
| WASM module fails to load | Browser error handler | Client works without similarity search (current behavior) |
| .rvf file too large for browser | Fetch timeout / memory error | Serve truncated file (top-N by authority); progressive load |

---

## 8. Migration Path

### 8.1 From Current State to Phase 1

1. Add `rvf-index` as git dependency (zero other deps)
2. Create `VectorStore` port trait
3. Create `RvfVectorStore` adapter
4. Generate SimHash embeddings during graph load
5. Wire `SemanticForcesActor` to use VectorStore for k-NN
6. Gate behind `ruvectorEnabled` quality gate
7. Benchmark and validate

### 8.2 Data Migration for Agent Memory (Phase 4)

```sql
-- Export from PostgreSQL
COPY (
  SELECT key, namespace, value, embedding::text
  FROM memory_entries
  WHERE embedding IS NOT NULL
) TO '/tmp/memory_export.csv' WITH CSV HEADER;
```

```bash
# Import to RVF
rvf-import --input /tmp/memory_export.csv \
           --output agent_memory.rvf \
           --dimension 384 \
           --id-column key \
           --vector-column embedding
```
