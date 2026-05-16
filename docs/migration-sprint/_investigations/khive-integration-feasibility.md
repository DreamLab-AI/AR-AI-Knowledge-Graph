# Feasibility — Bringing `khive` In-Tree as a VisionFlow Dependency

Status      : Investigation (no decision yet)
Date        : 2026-05-16
Author      : architecture-investigator (sub-agent)
Worktree    : `visionflow-worktrees/khive-investigation` (branch
              `impl/khive-investigation`, off `radical-rollback`@`d260a6158`)
Upstream    : `https://github.com/ohdearquant/khive`
              (cloned at `.khive-source/` — gitignored, not committed)
Scope       : Decide whether VisionFlow should adopt khive as a workspace
              member, a git dep, a library subset, an external service,
              or replace it entirely with Oxigraph.

## Executive summary

The upstream `khive` Rust crate (Apache-2.0, single author, six in-tree
crates, ~20,700 LOC of Rust) is a **clean hexagonal SQLite-and-vector
knowledge-graph runtime** with a stdio MCP front end. It is *not* what
the user's MCP tool description suggests: the upstream 11-tool surface
(`create / get / list / update / delete / merge / search / link /
neighbors / traverse / query`) is *substrate-level*. The `remember /
recall / inbox / thread / next / assign / send / complete / update /
request`-style 15-verb GTD/messaging surface visible to the agent
runtime is from a **downstream wrapper** — almost certainly the
`khive.ai` daemon — that layers tasks, messages, episodic memory, and
the `request` batch DSL on top of the substrate. That distinction is
load-bearing for this decision: the *substrate* is portable, the
*wrapper* is not in this repository and we cannot evaluate it directly.

Against VisionFlow's destination architecture (ADR-11 Oxigraph + SQLite,
ADR-10 WebSocket-only external integration, ADR-07 single
`GraphStateActor` for telemetry), khive is **architecturally orthogonal
on the storage axis** (it stores into its own SQLite-with-`sqlite-vec`
file) and **redundant on the embedding axis** (it ships
`lattice-embed` + MiniLM-L6-v2, which is the same model RuVector uses).
The right move depends almost entirely on whether VisionFlow needs a
*semantic memory store* at all — a question this ADR cannot answer
because PRD-11 explicitly scopes persistence to ontology + settings
and explicitly defers semantic search to "later".

**Recommendation: Path D (keep external MCP) — short-circuited by
Path E (replace with Oxigraph) if and when VisionFlow needs in-process
semantic memory.** Justification follows in §5. The recommendation is
defended on architectural grounds, not on effort.

| Path | Verdict        | One-line reason                                                                 |
|------|----------------|---------------------------------------------------------------------------------|
| A    | Reject         | Two embedded stores (Oxigraph + SQLite-vec) in one binary is incoherent.        |
| B    | Reject         | Same as A; git-dep doesn't fix the architectural duplication.                   |
| C    | **Conditional**| Take only `lattice-embed` if/when VisionFlow gets in-process semantic search.   |
| D    | **Recommend**  | khive is a substrate VisionFlow doesn't own — keep it external, MCP-shaped.     |
| E    | **Future**     | If we ever need in-process semantic memory, build it on Oxigraph, not on khive. |

The three sections most likely to revisit this decision are §5 (Path
evaluation), §7 (Spike plan — what we'd actually do if Path C is
adopted), and §9 (Decision-makers' checklist — questions the project
lead must answer before any of this matters).

## 1. khive architecture inventory

### 1.1 Workspace layout

`crates/Cargo.toml` declares a 7-member workspace
(`khive-types`, `khive-score`, `khive-storage`, `khive-db`,
`khive-query`, `khive-runtime`, `khive-mcp`), edition 2021, MSRV not
declared (workspace package metadata sets only version, edition,
authors, licence). The workspace dependencies pin:

| Dep                 | Version | Used by               |
|---------------------|---------|-----------------------|
| `serde` / `serde_json` | 1.0  | all                   |
| `tokio`             | 1.40    | db/runtime/mcp        |
| `anyhow`            | 1.0     | mcp                   |
| `thiserror`         | 2.0     | storage/db/runtime    |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | runtime/mcp |
| `uuid`              | 1.10    | storage/db/runtime    |
| `chrono`            | 0.4     | storage/db/runtime    |
| `async-trait`       | 0.1     | storage/db/runtime    |
| `axum`              | 0.7     | declared, *not used in any member crate* |
| `tower` / `tower-http` | 0.5/0.6 | declared, unused    |
| `reqwest`           | 0.12    | runtime               |
| `clap`              | 4.5     | mcp                   |
| `lattice-embed`     | 0.1.2   | runtime               |

The presence of `axum`/`tower` in the workspace dependency table
without any crate consuming them is a smell — they appear to be
forward-declared for a future HTTP MCP transport that is not yet
shipped. The only network-facing binary is `khive-mcp`, which uses
**rmcp 1.7 stdio transport only**.

`khive-mcp/Cargo.toml` declares no HTTP server, no axum binding, no
TCP listener — `serve_stdio()` in `khive-mcp/src/server.rs` literally
attaches the rmcp service to `std::io::stdin`/`stdout`. The daemon
mentioned in `khive-runtime`'s public docs ("Composable Service API
used by daemon, MCP server, and CLI") is not in this crate.

### 1.2 Crate roles

| Crate         | LOC*  | Owns                                                                       |
|---------------|-------|----------------------------------------------------------------------------|
| `khive-types` | ~300  | `Id128`, `Timestamp`, `Namespace`, substrate enums (`Entity`, `Note`, `Event`) |
| `khive-score` | ~250  | Deterministic fixed-point scoring (RRF, sum/avg/max/min), with cross-platform ordering |
| `khive-storage` | ~500 | Capability **port** traits: `SqlAccess`, `VectorStore`, `TextSearch`, `GraphStore`, `NoteStore`, `EventStore`. Zero implementations. |
| `khive-db`    | ~5,000 | SQLite **adapter** for the 6 storage capabilities. Uses `rusqlite 0.33 bundled`, `sqlite-vec 0.1.9` (feature-gated), FTS5 with trigram tokenizer. |
| `khive-query` | ~600  | GQL and SPARQL parsers compiling to SQL. (SPARQL→SQL is a self-contained mini-engine, *not* a third-party reasoner.) |
| `khive-runtime` | ~4,400 | High-level "operations" (create_entity, link, hybrid_search, traverse, merge, etc.) composed over the port traits. Holds the lazy embedder. |
| `khive-mcp`   | ~700  | rmcp-based stdio MCP server. Wraps `KhiveRuntime`. Exposes **11 tools**. |

\* approximate; from `wc -l` across each `src/`.

### 1.3 Persistence

SQLite-only, with three virtual-table extensions used through the
`rusqlite` bundled feature:

- **FTS5** for full-text (CJK-safe `trigram` tokeniser by default;
  `unicode61` available).
- **sqlite-vec** for cosine-distance vector search (feature-gated as
  `khive-db/features = ["vectors"]`; the workspace doesn't enable it
  by default but `khive-mcp` pulls it transitively through `runtime →
  db`).
- Standard SQLite WAL pool: 1 writer + N readers, in `khive-db/pool.rs`.

Database lives at `~/.khive/khive-graph.db` by default (`khive-mcp/src/
main.rs`), or `:memory:` for tests. Schema is **applied
per-capability on demand**: calling `backend.notes()` runs the notes
DDL and returns a `NoteStore`. There is no global migration runner that
applies all schemas upfront. The `schema_migrations` table tracks
per-service migration IDs (ADR-022). Five canonical schemas:
`entities`, `graph_edges`, `notes`, `events`, `vec_<model_key>`,
`fts_<table_key>`.

### 1.4 Embeddings

`khive-runtime` depends on `lattice-embed = "0.1.2"`. `cargo info`
shows this crate is also by `ohdearquant`, Apache-2.0, and depends on
`lattice-inference = "0.1.2"` — described as a "pure Rust transformer
inference engine — safetensors loading, SIMD matmul, BGE/Qwen3
embeddings". Notably it has **no** `ort` (ONNX runtime), no `candle`,
no `tch`, no `tract` in its dependency tree. It is a self-contained
inference stack with optional `metal-gpu` and `wgpu-gpu` features.

`RuntimeConfig::default()` selects `EmbeddingModel::AllMiniLmL6V2`
unless `KHIVE_EMBEDDING_MODEL` overrides. The runtime wraps the model
in `CachedEmbeddingService` (LRU over `NativeEmbeddingService`),
loaded lazily on first call. Vectors are 384-dim cosine; the SQL
schema embeds the dimension into the vec0 virtual table at creation
(`embedding float[384] distance_metric=cosine`).

**This is exact format-parity with RuVector's MiniLM-L6-v2 384-dim
HNSW table**, which is more than coincidence — it's the same model
shipped by an unrelated codebase. The implication is that *if*
VisionFlow ever needs semantic embeddings, the cheapest way is to
adopt `lattice-embed` directly (Path C subset), regardless of whether
the rest of khive comes along.

### 1.5 Vector search

`khive-storage::VectorStore` is the port:

```rust
async fn insert(&self, subject_id: Uuid, kind: SubstrateKind, namespace: &str, embedding: Vec<f32>) -> StorageResult<()>;
async fn search(&self, request: VectorSearchRequest) -> StorageResult<Vec<VectorSearchHit>>;
async fn rebuild(&self, scope: IndexRebuildScope) -> StorageResult<VectorStoreInfo>;
```

The SQLite adapter (`khive-db/src/stores/vectors.rs`,
`SqliteVecStore`) translates `search` into a `MATCH ? AND k=?` query
against the `vec0` virtual table. It does **not** use HNSW —
`sqlite-vec` uses brute-force kNN with optional metadata partitioning.
At RuVector's 1.17M-entry scale this would not scale; at khive's
single-user GTD scale (thousands of notes), brute force is fine.

This is one of the most important findings: **khive's vector path
does NOT replace RuVector**. They are not in the same league.

### 1.6 The 11 (not 15) MCP verbs

`khive-mcp/src/server.rs` (the `tool_router` block) defines exactly
these tools, all dispatched to `KhiveRuntime` methods:

| MCP tool    | Runtime method(s)                                  | Notes                                                                                  |
|-------------|----------------------------------------------------|----------------------------------------------------------------------------------------|
| `create`    | `create_entity` / `create_note`                    | `kind=entity|note`. Auto-embeds on insert if model configured.                         |
| `get`       | `get_entity` / `notes().get_note` / `get_edge`     | UUID-only; substrate auto-detected.                                                    |
| `list`      | `list_entities` / `list_edges` / `list_notes`      | `kind=entity|edge|note`.                                                               |
| `update`    | `update_entity` / `update_edge`                    | UUID-only.                                                                             |
| `delete`    | `delete_entity` / `delete_edge` / `delete_note`    | UUID-only; soft by default.                                                            |
| `merge`     | `merge_entity`                                     | Entity-only; not atomic in v0.1.                                                       |
| `search`    | `hybrid_search` / `search_notes`                   | FTS5 + vector, RRF-fused (`khive-score`).                                              |
| `link`      | `link`                                             | Directed edge, 13 canonical relations enumerated in `EdgeRelation`.                    |
| `neighbors` | `neighbors`                                        | One-hop with direction/relation filter.                                                |
| `traverse`  | `traverse`                                         | Multi-hop, BFS.                                                                        |
| `query`     | `query`                                            | GQL or SPARQL, compiled to SQL.                                                        |

The `remember / recall / inbox / thread / next / assign / send /
complete / link / delete / orient / request` 15-verb surface visible
in the agent's MCP tool list is **not present in this repository**.
It is a downstream wrapper — likely a Deno or Python daemon hosted at
`khive.ai` — that maps GTD-style ergonomics onto the substrate's 11
verbs. We cannot evaluate the wrapper from this checkout. The
deno.json in the root only configures `deno fmt` for the docs
folder; there is no TypeScript source for the wrapper here.

### 1.7 Concurrency model

khive uses **no actor system**. It uses raw `async-trait` on storage
traits, `tokio::sync::OnceCell` for the lazy embedder, and
`Arc<dyn Trait>` for handle sharing. The MCP server is a single
struct cloned per call (the `tool_router` macro implements `Clone`
and `ServerHandler`). This is the opposite of VisionFlow, which is
heavily actix-based (`actix 0.13`, `actix-web 4.11`, supervisors,
mailboxes, async handlers).

### 1.8 Tests

There is exactly one integration test surface: `tests/smoke_test.py`,
a Python script that spawns the binary over stdio and exercises all
11 tools end-to-end. Unit tests are inline (`#[cfg(test)] mod tests`)
in each crate — `khive-db/src/backend.rs` alone has 8 test functions
covering vectors/text/sql round-trip and idempotency. Rough estimate:
~150 inline tests, all in-process, mostly memory-backed.

There are no fuzzers, no property tests, no benchmarks committed.

### 1.9 Licence

Apache-2.0 (root LICENSE, all crate manifests). Compatible with
VisionFlow's MIT licence by absorption. The `lattice-embed` and
`lattice-inference` transitive crates are also Apache-2.0.
**No licence blockers.**

### 1.10 Maintenance posture

Single author (`Ocean <ocean@lionagi.ai>`), one repo, v0.1.0 across
all crates, no semver freeze yet — the README and ADR-022 explicitly
state the schema can break without notice in pre-1.0. There is no
release cadence yet to point at; the repo is < 6 months old by
content.

## 2. VisionFlow integration surface analysis

### 2.1 Where would khive slot in?

VisionFlow's hexagonal spine (`src/ports/`) defines seven port
traits: `GraphRepository`, `KnowledgeGraphRepository`,
`OntologyRepository`, `SettingsRepository`, `InferenceEngine`,
`PhysicsSimulator`, `SemanticAnalyzer`, plus two GPU adapter ports.
There is **no `MemoryRepository`, `KnowledgeBaseRepository`,
`SemanticIndex`, or equivalent port** today. Adopting khive in any
form requires defining one.

If we created `MemoryRepository`, it would have to deliberately
*avoid* overlapping with `KnowledgeGraphRepository`. The latter
already owns nodes and edges; adding khive's `create_entity` and
`link` to a parallel port duplicates the surface.

The cleanest integration shape would be a single new port,
`SemanticMemoryRepository`, with five methods:

```rust
#[async_trait]
pub trait SemanticMemoryRepository: Send + Sync {
    async fn upsert_note(&self, note: Note) -> Result<Uuid, MemoryError>;
    async fn search_notes(&self, query: &str, k: usize) -> Result<Vec<Hit>, MemoryError>;
    async fn delete_note(&self, id: Uuid) -> Result<bool, MemoryError>;
    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoryError>;
    async fn stats(&self) -> Result<MemoryStats, MemoryError>;
}
```

Notably absent: entity CRUD, graph operations, traversal. Those are
already owned by `KnowledgeGraphRepository` (Neo4j today, Oxigraph
per ADR-11). The semantic-memory port is *additive* on top of the
graph repository, not a replacement.

If VisionFlow adopted khive in-tree (Path A), `SemanticMemoryRepository`
would be implemented as a thin wrapper over `KhiveRuntime::create_note
/ search_notes`. The entity/edge methods of `KhiveRuntime` would go
unused, since the graph data already lives in Oxigraph.

This is the **architectural duplication smell** that disqualifies
Paths A and B in §5: half of khive's surface area is graph storage
we don't need.

### 2.2 Dependency conflict map

VisionFlow `Cargo.toml` vs khive `crates/Cargo.toml`:

| Dep             | VisionFlow              | khive          | Compatible?                                            |
|-----------------|-------------------------|----------------|--------------------------------------------------------|
| `tokio`         | 1.47.1                  | 1.40           | ✅ semver compatible                                    |
| `serde`         | 1.0.219                 | 1.0            | ✅                                                      |
| `serde_json`    | 1.0                     | 1.0            | ✅                                                      |
| `uuid`          | 1.18.0                  | 1.10           | ✅                                                      |
| `chrono`        | 0.4.41                  | 0.4            | ✅                                                      |
| `tracing`       | 0.1                     | 0.1            | ✅                                                      |
| `thiserror`     | 2.0.16                  | 2.0            | ✅                                                      |
| `async-trait`   | 0.1                     | 0.1            | ✅                                                      |
| `reqwest`       | 0.12.23                 | 0.12           | ✅                                                      |
| `clap`          | 4.5                     | 4.5            | ✅                                                      |
| `rusqlite`      | (none)                  | 0.33 (bundled) | New transitive — adds bundled SQLite C lib.            |
| `sqlite-vec`    | (none)                  | 0.1.9          | New transitive.                                        |
| `lattice-embed` | (none)                  | 0.1.2          | New transitive — adds `lattice-inference` + safetensors.|
| `oxigraph`      | (planned per ADR-11)    | (none)         | No collision, but both store data.                     |
| `actix-web`     | 4.11.0                  | (none)         | No collision, but worlds-apart concurrency styles.     |
| `axum`          | (none)                  | declared, unused| If we adopt khive in-tree, this becomes a phantom dep.|

**No version collisions.** `lattice-embed` would, however, double
the binary size: `lattice-inference` carries safetensors loading and
SIMD matmul kernels, and likely a bundled MiniLM model file. A rough
estimate from `cargo info` and the dependency tree: an additional
**20–35 MB** of `.text` once linked, plus whatever model bytes the
crate ships (need to verify — see §9 Q4).

### 2.3 Concurrency-model collision

VisionFlow is actix-heavy:

```
GraphStateActor → PhysicsOrchestrator → ClientCoordinatorActor →
WebSocket frames
```

…with actor-to-actor messaging via Actix `Message` trait. khive is
plain `async fn` returning `Result`. The two compose fine — we
already do this with `neo4rs`, which is also `async fn`-shaped — but
the integration point has to be inside an actor wrapper. The shape
would be:

```rust
// src/actors/semantic_memory_actor.rs
pub struct SemanticMemoryActor {
    runtime: khive_runtime::KhiveRuntime,
}
impl Actor for SemanticMemoryActor { type Context = Context<Self>; }
impl Handler<UpsertNote> for SemanticMemoryActor { /* ... */ }
```

This is uncontroversial — we wrap async libraries inside actors all
the time. It just adds one more actor with no clear consumer (because
nothing in VisionFlow's current handler set wants semantic memory).

### 2.4 HTTP/MCP surface — does khive want to be a server?

khive-mcp serves *only* over stdio. It does not bind a TCP socket, it
does not expose HTTP. The `axum` and `tower` deps in the workspace
table are unused. So adopting khive in-tree does **not** add an HTTP
server to VisionFlow's actix-web tree. The only way to keep an MCP
surface alive after in-tree adoption is for VisionFlow to *itself*
become an MCP server — which actix-web is not a natural fit for
(rmcp expects stdio or a future axum HTTP transport).

In other words, Path A/B/C all force the question: **do external
agents lose access to khive's verbs, or do we add a stdio MCP
listener subprocess inside the VisionFlow container?** Neither answer
is appealing. Path D side-steps the question entirely.

### 2.5 Where would the data live?

ADR-11 commits to:

- Oxigraph (RocksDB-backed) for graph data (knowledge + ontology +
  agent telemetry quads in 4 named graphs).
- SQLite (`settings.sqlite3`) for settings + audit.

khive would add a **third store**: SQLite-with-vec0/FTS5 at, e.g.,
`<data-dir>/semantic-memory.sqlite3`. Three on-disk stores in one
binary, three backup procedures, three corruption-recovery stories,
three pragma sets. ADR-11 §O7 explicitly rejected adding Oxigraph for
settings on the grounds of "Two stores is the right cost for the
right shape." Adding a third store to host semantic memory we don't
currently consume violates that decision's spirit.

The technically clean alternative is to retarget khive's
`VectorStore` and `TextSearch` ports onto Oxigraph — but Oxigraph
has no built-in vector index, no FTS5 equivalent, and no analogue to
`sqlite-vec`. The hot retrieval path (`hybrid_search` in
`khive-runtime/src/retrieval.rs`) would need a complete rewrite
against a different backing store. This is not a trait swap; it is a
rewrite of the only feature khive is actually useful for.

### 2.6 Agent telemetry intersection (Section 7)

ADR-07 D7 specifies that *all* live agent state comes from telemetry
events ingested over a WebSocket from agentbox (ADR-10 D1). The
agent graph lives in the *same* `GraphStateActor` as the knowledge
graph, discriminated by class-flag bits on node IDs. Agent state has
no persistence requirement beyond a TTL — the in-RAM telemetry feed
is the source of truth, and there is no "remember this agent for
later" affordance.

khive's `task` and `message` substrate kinds (per the wrapper's MCP
description) have no consumer in VisionFlow. We do not run a GTD
inbox, we do not assign tasks, we do not maintain agent message
threads. **The wrapper's verbs solve a problem VisionFlow does not
have.**

This is the strongest single argument against bringing khive in-tree:
the only part of khive's *user-visible* feature set that VisionFlow
could plausibly want — semantic search over notes — is the part that
overlaps least with the substrate's reason for existing.

## 3. Path evaluation

| Path | Description                                | Pros                                                              | Cons                                                                                                                | Effort (engineer-days) | Risk         |
|------|--------------------------------------------|-------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------|------------------------|--------------|
| **A**| Vendor (`crates/khive/` as workspace member) | Hermetic; patchable; one binary; type-level integration possible. | Adds SQLite + sqlite-vec + lattice-embed + lattice-inference + safetensors stack. Three on-disk stores. Maintenance owns. | 8–12 days              | High         |
| **B**| Git dep with pinned commit                 | Upstream tracking; lower merge cost than vendoring.               | Same architectural duplication as A. Cannot local-patch easily. Single-author upstream is a bus-factor risk.        | 4–6 days               | Medium-high  |
| **C**| Library subset — take only `lattice-embed` + a `SemanticMemoryRepository` adapter | Clean port boundary; no SQLite-vec; embeddings live in Oxigraph. | Need to build Oxigraph-backed vector search ourselves (no HNSW today; brute-force kNN is acceptable at small scale). Re-implements the bits of khive-db we actually want. | 6–10 days              | Medium       |
| **D**| Keep external MCP (status quo)             | Zero work; clean failure-domain separation; khive ages without us.| External auth surface; cannot share types; cannot drive khive from VisionFlow events.                               | 0 days                 | Low          |
| **E**| Replace with Oxigraph                      | One store; no embeddings dep at all; falls within ADR-11.         | Loses semantic search until we add an HNSW crate (e.g., `hnsw_rs`) on top of Oxigraph. Punts the question.          | 0 days now, 5–8 days when needed | Low |

### 3.1 Effort estimates explained

**Path A** (8–12 days): copy `.khive-source/crates/{types,score,storage,db,query,runtime}`
into `crates/khive-*` (skip `khive-mcp`); rewrite top-level
`Cargo.toml` to declare them as workspace members; resolve the
implicit `axum`/`tower` workspace deps; thread the data-dir convention
from ADR-11 (`<data-dir>/semantic-memory.sqlite3`); add
`SemanticMemoryActor` wrapper around `KhiveRuntime`; ship a
`SemanticMemoryRepository` port; write 3–4 integration tests against
the actor; update Dockerfile to add `libsqlite3-dev` (or rely on
bundled feature); add backup procedure entry to
`docs/operations/backup-restore.md`.

**Path B** (4–6 days): same as A minus the vendoring mechanics, plus a
`khive = { git = "...", rev = "..." }` entry and a CI job that
periodically tries `cargo update -p khive*` and opens a PR. Same
actor wrapper, same port, same tests.

**Path C** (6–10 days): take only `lattice-embed = "0.1.2"` from
crates.io. Build `SemanticMemoryRepository` and its Oxigraph adapter
ourselves. The adapter stores `(uuid, embedding_iri, embedding_blob_iri)`
triples in Oxigraph and pulls all embeddings into RAM for query (~1.5
KB per entry × 10k entries = 15 MB, acceptable). Brute-force cosine
in Rust with `rayon` is plenty for that scale. Add an HNSW crate
later if scale demands it. Net code added: ~1,200 LOC.

**Path D** (0 days): document the boundary. Add the `khive` MCP
endpoint to ADR-10 §"External MCP boundaries" (does not exist today —
this would be an additive change). Specify that VisionFlow does not
own khive's data, auth, or schema. This is essentially status quo.

**Path E** (0 days now): no decision required until VisionFlow grows
a semantic-search consumer. When that happens, allocate 5–8 days for
a brute-force-kNN-over-Oxigraph implementation as in Path C minus
the embedding library (because we'd then need to choose an embedder
of our own — `lattice-embed` is the obvious candidate, but at that
point the choice is small and isolated).

### 3.2 Risk explained

**Path A / B risks**: single-author upstream (Ocean) is a bus-factor
of one. Repo is pre-1.0; ADR-022 promises no schema stability. We
would inherit migrations from upstream and our own. The
`lattice-inference` crate is also pre-1.0 and same author — if either
crate goes unmaintained, we own both.

**Path C risk**: `lattice-embed` is the only piece we'd take, but we'd
own integrating it into Oxigraph and would need to verify the model
download story (does it download MiniLM on first use? where to?
container-friendly?). See §9 Q4.

**Path D risk**: external MCP servers have their own failure modes
(auth surface, container, network partition). But the *VisionFlow*
deployment is unaffected if khive fails: it just loses access to
semantic recall verbs that nothing in VisionFlow currently
consumes.

**Path E risk**: the lowest. We're deferring a decision rather than
making the wrong one.

## 4. Recommended path

### Primary: Path D — keep khive as an external MCP server

The single sharpest argument: **VisionFlow has no consumer of khive's
verbs today.** ADR-07, ADR-10, ADR-11 do not reference semantic
recall. Agent telemetry is in-RAM with TTL. Ontology is in Oxigraph
with named-graph semantics and Whelk inference. Settings are in
SQLite. Knowledge-graph nodes and edges are quads.

Bringing khive in-tree would:

1. Add a third on-disk store (SQLite-with-vec0) to a binary that
   ADR-11 explicitly designed to have two.
2. Pull in `sqlite-vec`, `rusqlite-bundled`, `lattice-embed`,
   `lattice-inference`, `safetensors` — none of which any current
   VisionFlow feature uses.
3. Double the embedded ML/NLP surface area (we already plan no
   on-binary inference; Whelk is the only reasoner in-process).
4. Inherit a pre-1.0 dependency tree with a single upstream
   maintainer.

…all to provide a feature (semantic memory) for which no consumer is
specified. The honest answer is "we don't need this yet". Path D
acknowledges that.

### Conditional fallback: Path E — replace with Oxigraph when needed

If, after this sprint, a new ADR specifies a "semantic memory" feature
(e.g. "agents should be able to recall their own past observations
through VisionFlow's API"), the right move is to build it on the
existing Oxigraph store, not adopt khive. The semantic-memory store
would be:

- Named graph `<urn:visionflow:graph:memory>` in the same Oxigraph
  dataset.
- Each memory entry is one IRI (`vc:memory/<sha256-12>`) with triples
  for `vc:body` (literal), `vc:embedded_at` (xsd:dateTime), and
  `vc:author` (npub).
- The embedding vector is stored as a parallel SQLite table in
  `settings.sqlite3` (or a sibling file), keyed by the same IRI,
  containing `(iri TEXT PRIMARY KEY, embedding BLOB)`.
- Query path: brute-force cosine over the BLOB table, joined with
  Oxigraph SPARQL for filtering — single SQL prepared statement
  with `unsafe_load_extension` to add `sqlite-vec` only if the
  result count tips above ~10k entries.

This keeps the ADR-11 two-store posture, adds no new dependencies
beyond an optional `sqlite-vec` activation, and gives us the
*useful* part of khive (semantic recall) without the *unused* part
(GTD substrate, RRF scoring, GQL/SPARQL parsing, edge ontology).

### Why not Path C now?

Path C is tempting because `lattice-embed` is genuinely well-designed
— pure Rust, no ONNX, SIMD-accelerated, MiniLM-default — and we *will*
eventually need embeddings for *something*. But adopting it now means
either (a) sitting on a configured-but-unused embedder (which is a
liability — code rot accumulates around code with no callers), or
(b) inventing a consumer to justify the dep. Neither is sound.

Path C becomes the right answer the moment a PRD specifies a
semantic-memory consumer. Until then, Path D + Path E is the honest
posture.

## 5. Spike plan (only relevant if we choose Path C in the future)

If a future ADR commits VisionFlow to in-binary semantic memory,
the spike sequence is:

1. **Day 1** — Add `lattice-embed = "0.1.2"` to `Cargo.toml`. Write
   a single integration test under `tests/embedder_smoke.rs` that
   embeds three strings and asserts cosine similarity ordering
   (`"cat sat on mat"`, `"dog ran in park"`, `"feline rested on
   rug"` — third should be nearest first). Confirm the model
   download story (where does it pull MiniLM weights from? is the
   download network-fenced for the production container?). This
   answers §9 Q4 empirically before commitment.
2. **Day 2** — Define `src/ports/semantic_memory_repository.rs` with
   the 5-method trait above. Add `MemoryError` enum.
3. **Day 3** — Build `src/adapters/oxigraph_semantic_memory.rs`
   storing memory triples in `<urn:visionflow:graph:memory>` and
   embeddings as `(iri, blob)` rows in an extra SQLite table
   `memory_embeddings`. Brute-force kNN search with `rayon`.
4. **Day 4** — `src/actors/semantic_memory_actor.rs` wrapping the
   adapter. Mailbox messages: `UpsertNote`, `SearchNotes`,
   `DeleteNote`, `EmbedText`, `Stats`.
5. **Day 5** — REST endpoint at `POST /api/memory/search` if PRD
   requires; otherwise leave actor private.
6. **Day 6** — Acceptance tests against a temp directory, including
   10k-row scale test (cold-start memory footprint, query latency
   p50/p99).
7. **Day 7** — Operational handover: backup procedure for the new
   sibling SQLite file, monitoring of memory size, retention policy.

Each day produces a commit that passes `cargo test`. The spike's
abort criterion is **Day 1**: if `lattice-embed`'s model-download
behaviour is incompatible with the production container (e.g.
downloads at startup, requires internet at runtime, or pulls
unbounded model bytes), we abort and switch to either
`fastembed-rs` or an external embedding service.

## 6. Licence and dependency hygiene

- khive itself: **Apache-2.0** — compatible with VisionFlow's MIT.
  No CLA, no patent clause issues for our deployment posture
  (commercial use permitted, sublicensing permitted).
- `lattice-embed` 0.1.2: **Apache-2.0**.
- `lattice-inference` 0.1.2: **Apache-2.0**.
- `rusqlite` 0.33: **MIT** (statically linked to SQLite, also
  public-domain-equivalent).
- `sqlite-vec` 0.1.9: **Apache-2.0 OR MIT**.

No copyleft (no GPL, no AGPL, no MPL) in the chain. **No licence
blockers.**

The dependency-hygiene concerns are bus-factor and pre-1.0 churn,
not licensing:

- All four critical-path crates (`khive`, `lattice-embed`,
  `lattice-inference`, plus `oxigraph` once we adopt it) are by
  small teams or solo maintainers.
- Pre-1.0 semver means any minor version bump can break us.
- The model-download story for `lattice-inference` is unverified
  in this investigation — see §9 Q4.

## 7. Decision-makers' checklist

To approve any of Paths A/B/C/D/E, the project lead needs yes-or-no
answers to these seven questions. If even one answer points away
from the recommended path, the recommendation flips.

1. **Does any VisionFlow feature today need semantic recall over
   notes or memory entries?**
   - If yes → Path C/E becomes urgent.
   - If no → Path D stands.

2. **Is the agent control plane (memory + tasks + messages)
   considered "VisionFlow's responsibility" or "agentbox's
   responsibility"?**
   - ADR-10 D1 places it firmly in agentbox.
   - If the answer changes → re-evaluate everything; this whole
     investigation is moot.

3. **Are we willing to add a third on-disk store (SQLite-with-vec0)
   to the `webxr` binary in addition to Oxigraph and
   `settings.sqlite3`?**
   - ADR-11 §O7 said no to a third store.
   - Path A/B says yes; Path C/D/E say no.

4. **What is `lattice-embed`'s model-download behaviour, and is it
   compatible with our production container's network policy?**
   - Empirically determined by Spike Day 1.
   - If incompatible → Path A/B/C all become harder; Path D/E
     unaffected.

5. **Do we expect external (non-VisionFlow) agents to reach khive's
   verbs in 6–12 months?**
   - If yes → Path D is structurally correct (preserves the
     boundary).
   - If no → Path E is the long-term endpoint.

6. **Are we comfortable inheriting bus-factor-1 from `khive` +
   `lattice-embed` + `lattice-inference` (all by `ohdearquant`)?**
   - If no → Path D until upstream consolidates or VisionFlow
     forks.

7. **Does the operations team have capacity to own a fourth backup
   procedure (Oxigraph dir, settings.sqlite3, memory.sqlite3, and
   whatever the audit-log retention thing rotates) without a
   procedural overhaul?**
   - If no → Path E is the only acceptable choice when the time
     comes.

A "yes" on Q1 and Q3, with a "compatible" on Q4, is the only state
that justifies Path A or B. The current evidence is "no" on Q1,
"no" on Q3 (per ADR-11 §O7), and "unknown" on Q4. So the current
state mandates Path D, with Path E reserved for future need.

## 8. Open questions and unknowns

These items could not be determined inside the 20-minute investigation
window and are flagged for follow-up if a Path C spike is approved.

- **`lattice-embed` model bytes**: where does the MiniLM weights
  blob come from? Crates.io artefact? Hugging Face on first use?
  Bundled in `lattice-inference` `data/`? Run `cargo run --example
  embed` in a fenced network sandbox to find out. Affects Q4 above
  and Spike Day 1.
- **The 15-verb wrapper**: the GTD-flavoured surface (`remember`,
  `recall`, `inbox`, `thread`, `next`, `assign`, `send`, `complete`,
  `update`, `request`, `orient`) is not in this repository. It is
  almost certainly a Deno or Python daemon on the `khive.ai`
  property. Without seeing that wrapper's source, we cannot
  evaluate it. If the user's intent is to in-tree that wrapper as
  well, the investigation is incomplete.
- **`khive-runtime`'s portability module** (`portability.rs`, 436
  lines): names suggest an export/import format for KG archives.
  If VisionFlow grows a corpus interchange feature with another
  khive-based system, this becomes relevant; otherwise it's dead
  surface.
- **GQL parser** (`khive-query`): a Cypher-like surface compiled
  to SQL. Has no consumer in any plausible VisionFlow path
  because we use SPARQL via Oxigraph. Would be dead code if
  vendored.
- **The `axum`/`tower` workspace deps with no consumers**: are
  these forward declarations for an HTTP MCP transport not yet
  written? If so, an in-tree adoption inherits the obligation to
  finish that transport, or strip the unused deps from our copy.
- **`whelk-rs` in this worktree**: VisionFlow already has whelk-rs
  vendored (`./whelk-rs`) for OWL reasoning. khive does not use
  it. No interaction.

## 9. What we explicitly chose not to do in this investigation

- We did not benchmark khive's hybrid_search latency. The spike
  plan (§7) calls for that on Day 6, after architectural sign-off.
- We did not attempt to compile khive in this worktree. The
  workspace is throwaway; compilation would test crate hygiene
  but not architectural fit, which is the question this report
  answers.
- We did not contact the upstream maintainer about roadmap, semver
  intent, or 1.0 timeline. That is a project-lead conversation,
  not a sub-agent one.
- We did not survey the wrapper at `khive.ai`. It is not in this
  repo; assuming anything about it would be speculation.
- We did not evaluate `fastembed-rs`, `model2vec`, `ort` direct, or
  any other embedding-library alternative. If Spike Day 1 disqualifies
  `lattice-embed`, that survey becomes a separate investigation.

## 10. References

- Upstream: `https://github.com/ohdearquant/khive` (Apache-2.0,
  commit explored: HEAD as of clone date 2026-05-16)
- VisionFlow ADRs consulted:
  - `docs/migration-sprint/11-persistence-migration/ADR-11.md` (D1,
    O5, O7 directly cited)
  - `docs/migration-sprint/07-bots-telemetry/ADR-07.md` (D3, D7
    directly cited)
  - `docs/migration-sprint/10-external-integrations/ADR-10.md` (D1,
    D6 directly cited)
- VisionFlow ports: `src/ports/{mod,knowledge_graph_repository,
  ontology_repository,settings_repository,inference_engine}.rs`
- khive ADRs consulted: ADR-005 (storage capability traits), ADR-022
  (schema migrations), ADR-023 (verb-consolidated surface), ADR-024
  (note search), all under `.khive-source/docs/adr/`
- khive source paths cited:
  - `.khive-source/crates/khive-mcp/src/{main,server}.rs`
  - `.khive-source/crates/khive-runtime/src/{lib,runtime,operations}.rs`
  - `.khive-source/crates/khive-storage/src/{lib,vectors}.rs`
  - `.khive-source/crates/khive-db/src/backend.rs`
  - `.khive-source/tests/smoke_test.py`

— end —
