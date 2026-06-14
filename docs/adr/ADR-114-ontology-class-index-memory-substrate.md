# ADR-114 — Memory substrate for the ontology Class-Summary index

**Status:** Proposed
**Date:** 2026-06-14
**Decision-type:** Feature Pipeline / Data
**Parent:** ADR-112 (retrieval spine), PRD-020 (WS-2). **Relates:** ADR-113 (condensation mesh produces what is indexed), ADR-115 (Turtle), agentbox ADR-015 (RuVector memory mandate), ADR-099/105 (ontology rigour / IRI).

> Written ahead of its siblings because the substrate choice was challenged directly ("do we index into RuVector? is that the right memory substrate?"). The answer is grounded in verified facts about what vector capability actually exists in each system today, not convention.

---

## 1. Context — the question

The augmentation binding needs a **semantic seed leg**: turn an agent's free-text query into the top-k relevant ontology class IRIs, cheaply. That requires embeddings + an ANN index over a Class-Summary per class (~14,718 incl. stubs). Where should that index live, and is RuVector the right store?

This is a genuine architecture fork, so it was settled against ground truth rather than the "always use RuVector" convention.

## 2. Verified facts (2026-06-14, working tree)

| Claim | Evidence | Verdict |
|---|---|---|
| VisionClaw has a Qdrant vector store | `qdrant_data/` is **7.9 GB** (collections, raft_state.json, last write Nov 2025) **but `grep -rin qdrant src crates Cargo.toml` → 0 matches** | **Orphaned.** A dead legacy blob with zero Rust wiring. Not a usable index; a trap. |
| VisionClaw `/api/ontology-agent/discover` is semantic | `ontology_query_service.rs:47` "Keyword match against OwlClass preferred_term/label"; `:98` `combined = keyword*0.4 + quality*0.3 + authority*0.2 + 0.1` | **Keyword only.** No embeddings, no ANN. Cheap structural filter, not semantic recall. |
| VisionClaw has a real embedding pipeline | `semantic_processor_actor.rs:402` `generate_content_embedding_static` → 256-dim **hash bag-of-words** (`embedding[hash] += 1.0/(i+1)`); `settings/models.rs:140` `ruvector_enabled = false // Off by default (requires integration)` | **No.** A toy hash vector, disabled integration. Not fit for semantic seeding. |
| agentbox has a real embedding pipeline + HNSW | `mcp/servers/ruvector-mcp.cjs:77-99` xinference `bge-small-en-v1.5` 384-dim → RuVector PostgreSQL HNSW; `memory_store/search` MCP tools live | **Yes.** The only live, real semantic substrate in the ecosystem. |

**Conclusion from facts:** VisionClaw's apparent vector options are all mirages — an orphaned 7.9 GB Qdrant with no code, a fake 256-dim hash embedding, a keyword-only discover, and `ruvector_enabled=false`. The **only** working embeddings + ANN in the ecosystem is agentbox's RuVector via xinference.

## 3. Decision

**Index the Class-Summary semantic seed leg into RuVector** (namespace `ontology-classes`), via `memory_store`/`memory_search` (xinference `bge-small-en-v1.5`, 384-dim, HNSW) — **never raw SQL** (raw INSERT bypasses the embedding pipeline; agentbox CLAUDE.md mandate). Built/refreshed by the ADR-113 condensation mesh.

**But RuVector is the substrate for the *seed leg only*, not for the binding as a whole.** The binding is intrinsically **three-store**, and conflating them is the error the question guards against:

| Concern | Substrate | Why |
|---|---|---|
| Source of truth + structural k-hop | **Oxigraph** (VisionClaw, SPARQL) | The ontology *is* Oxigraph; traversal is SPARQL. RuVector cannot do `subClassOf` closure. |
| Semantic seed (free-text → class IRIs) | **RuVector** (xinference HNSW) | Only live embeddings+ANN. This ADR. |
| Synchronous PUSH breadcrumb (<15 ms, no network) | **In-process pre-warmed cache** | RuVector is a Postgres round-trip — too slow for the hot hook path. The cache is *sourced* from RuVector async, not queried per turn. |

So "do we index into RuVector?" → **yes, for the seed leg**; "is it the right substrate?" → **yes, and currently the only viable one** — but it is *one of three*, and it holds a **derived projection** of VisionClaw, not the truth.

## 4. Consequences

**Positive**
- Reuses the only real embedding pipeline + HNSW; zero new vector infra; clean MCP (`memory_store/search`); honours the no-raw-SQL mandate; aligns with agentbox memory convention.
- Keeps Oxigraph as the single source of truth; RuVector is disposable and rebuildable.

**Negative / managed**
- **Derived-projection drift.** The index is a copy of VisionClaw ontology state. Mitigation (WS-2): refresh trigger on GitHubSync/elevation; incremental re-condense of changed classes only; `classes_seen` counter + a canary `memory_search` in the liveness self-test (ADR-119) to detect staleness.
- **Ownership smell.** Ideologically a semantic index over VisionClaw's ontology "belongs" co-located with the source. Accepted as pragmatic: VisionClaw has no real embedding capability and building one (revive/replace Qdrant + a Rust embedding pipeline) is a large detour duplicating what agentbox already does well. **Revisit trigger:** if VisionClaw gains a first-class embedding pipeline (real model, `ruvector_enabled` wired, or a live Qdrant), move the canonical index VisionClaw-side and let agentbox hold only the hot cache.
- **Multi-tenant store.** `ontology-classes` lives beside ~1.17M general memory entries. Mitigation: dedicated namespace; importance-weighted retrieval (build-with-quality v1.3.0) keeps class summaries from drowning in session noise.
- **Embedding model fit.** `bge-small` 384-dim is general-purpose, not ontology-tuned. Accepted for MVP; recall measured against the §"Outcomes" hypothesis (ADR-112 §3) before any model swap.

**Durable storage of the summary text — now ADOPTED via ADR-121 (W0), not deferred.** The Class-Summary *text* is written back to Oxigraph as annotation triples in the derived named graph `urn:ngm:graph:ontology:summary` (SPARQL-queryable, version-controlled with the ontology, re-embeddable by either system), with RuVector holding the *vectors* for seeding. This is the W0 tier of the self-improving writeback loop (ADR-121) through the **fenced** `POST /api/ontology/derived` endpoint that can write only `:summary`/`:usage`. RuVector remains the seed substrate; Oxigraph becomes the durable record of the summary text. (Originally deferred here for adding VisionClaw write surface; the operator chose to land the full ideal — ADR-121.)

## 5. Alternatives considered

1. **Revive the orphaned 7.9 GB Qdrant in VisionClaw.** Rejected: zero Rust wiring, stale since Nov 2025, unknown collection schema; reviving it is net-new integration work for no advantage over the live RuVector path.
2. **VisionClaw-side real embedding pipeline + vector search (upgrade `discover` from keyword to semantic).** Rejected for now (correct long-term, see revisit trigger): requires a real embedding model in Rust + ANN index + `ruvector_enabled` integration — a large detour duplicating agentbox's working pipeline. Keyword `discover` remains a useful cheap *structural* pre-filter, not the semantic leg.
3. **VisionClaw's existing 256-dim hash embedding.** Rejected: it is a bag-of-words hash (`semantic_processor_actor.rs:402`), not semantic; would give poor recall.
4. **A new standalone vector DB for agentbox.** Rejected: RuVector already provides HNSW + the embedding pipeline; new infra violates reuse and adds ops surface.
5. **No vector index — keyword/SPARQL only.** Rejected: defeats free-text "find the relevant class" and pushes the cost onto VisionClaw's in-process ~5000-class keyword scan per query.

## 6. Verification

- WS-2: `memory_search(ns:'ontology-classes')` returns IRIs for a canary query; index holds ≥ authored-class count; refresh re-condenses only changed classes (incremental proof).
- Liveness (ADR-119): the HNSW canary is one of the three backing assertions.
- Drift: `ClassSummaryIndexRefreshed{changed_count}` event fires on GitHubSync; staleness surfaced via `classes_seen`.
