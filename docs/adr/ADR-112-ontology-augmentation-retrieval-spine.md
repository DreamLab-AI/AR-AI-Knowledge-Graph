# ADR-112 — Ontology Augmentation: one shared-library retrieval brain, two channels, condense offline, govern the write

**Status:** Implemented — updated 2026-07-03. The shared-library retrieval brain
ships in agentbox (`agentbox/mcp/servers/ontology-bridge.js`,
`agentbox/mcp/servers/lib/ontology-retrieval.js`,
`agentbox/mcp/servers/lib/ontology-push.js`) and is treated as a live binding —
the "pervasive ontology binding (PRD-020/ADR-112)" referenced in `CLAUDE.md` and
the `ontology-augment` skill. Originally proposed 2026-06-14.
**Date:** 2026-06-14
**Decision-type:** Architecture (keystone) + Agent-Orchestration (§ Condensation mesh)
**Relates:** PRD-020 (parent), `docs/ddd-ontology-augmentation-context.md` (bounded context), ADR-099/100/105/106 (ontology rigour, IRI, convergence, SPARQL patch), ADR-110/041 (ACSP / Judgment Broker — governed write), ADR-026 (3-tier model routing), agentbox ADR-015 (RuVector memory), agentbox ADR-005 (pluggable adapters), ADR-111 §4.6 (the diagram this decision finalises), PRD-018 (the silent-dead-wiring lesson this ADR is built to avoid)

> Keystone ADR for the PRD-020 family. The sibling decisions (ADR-113…ADR-120) are summarised in the **Decision register** below and split into individual ADRs during implementation. Authored under `build-with-quality`; the design survived a 5-lens adversarial review only after the corrections recorded here (the first synthesis failed all five, `holds=false`, conf 4–5).

---

## 1. Context

VisionClaw owns a large formal ontology/KG (Oxigraph/RocksDB, Whelk EL++, ~4,952 authored `owl:Class` + ~9,766 stubs, ~123k triples, materialised inference, a logseq corpus whose conceptual reach is "~40M words"). Agentbox owns the active intelligence (5 LLM consultants, the claude-flow/ruv-swarm Claude substrate, ~114 skills, autonomous agents, a per-turn injection hook). The requirement: **every agentbox AI call gains the option to query that ontology, structurally and pervasively, without overpowering context windows.**

A 16-agent opus mesh explored both surfaces and proposed a hybrid "Ontology Augmentation Service" as a new `POST /v1/ontology/ask` HTTP route inside `management-api`. Adversarial review demolished the *form* (not the spirit) of that proposal:

- `management-api` is **Fastify, not Express** (`server.js:8,69`); the synthesis's "Express router" would not mount, would not receive the `fastify.adapters.memory` / `fastify.linkedData` decorators, and would bypass the auth chain.
- A new HTTP brain forces a **second secret domain** (`MANAGEMENT_API_KEY`, hard-required, binds `:9090`, `server.js:36-40`) on top of VisionClaw's power_user Bearer+X-Nostr — across *isolated* consultant subprocesses and the hook env.
- A per-turn HTTP→HNSW→embedding round-trip on `UserPromptSubmit` is categorically heavier than the **synchronous <15 ms local** lookup the hook budget (`intelligence.cjs:getContext`) assumes.
- `ontology-bridge.js` **already implements most of the substance** (read-only SPARQL + denylist, domain-filtered class list, bounded k-hop, search) — a new service is largely redundant.
- The proposed "governed write anchor" `POST /api/ontology-agent/propose` is **wholly unauthenticated today** (`ontology_agent_handler.rs:317-329`), so the governance story rested on a forge/flood hole.
- The PUSH channel **does not reach swarm subagents or spawned agentic CLIs** (`spawn-cli.js:30` `inherit_env=false`), so an unqualified "ALL AI calls" claim was false.

PRD-018 is the cautionary precedent: ontology *forces* were compiled but silently inert for months because nobody proved the consumer invoked them. "Wired ≠ working."

---

## 2. Decision

### 2.1 One retrieval brain — as a **shared in-process library**, not a service

Build exactly one retrieval module, **`@agentbox/ontology-retrieval`**, that owns entity-linking, hybrid seed→expand retrieval, terse Turtle serialisation, tiered summarisation, central token budgeting, TTL caching, and PROV-O scoping. It is **imported in-process** by each channel — the bridge's `ontology_ask` tool, the consultant seam, and the hook's `getOntologyContext`. "One brain" = one shared module + the shared backing stores (RuVector + VisionClaw), **not** one HTTP process.

*Consequence:* no second auth wall, no Fastify/Express mismatch, no per-turn network hop. Each process runs its own instance of the same code; the only shared *state* is RuVector (the class index) and VisionClaw (the source), both already shared services.

### 2.2 Two thin channels (PUSH + PULL), per an audited coverage matrix

- **PUSH** — a **synchronous** breadcrumb on the live `UserPromptSubmit` hook (`hook-handler.cjs route`). It does a local trigram match over a **pre-warmed in-process Class-Summary cache** (mirroring `getContext`), applies a relevance null-gate, and emits **one locally-clamped** `[ONTOLOGY]` line (≤80 tok). It never touches the network on the hot path. Opt-out, default-off until WS-6 telemetry.
- **PULL-A** — a seam at `BaseConsultant._handleConsult` (one edit reaches deepseek + perplexity cleanly, and the first turn of the three CLI-spawning consultants).
- **PULL-B** — an `ontology_ask` MCP tool on `ontology-bridge.js`, promoted into canonical `mcp/mcp.json` (gated) so every agent can pull on demand.

"Pervasive" is the **coverage matrix in PRD-020 §2.3**, not the word "ALL". CLI-spawn subtrees, swarm subagents, 9 direct-SDK skill clients, and the junkiejarvis backend agent are reached by neither channel as-is and are closed in WS-7 (shared `callLlm` wrapper + per-CLI `$HOME` grounding) after a WS-5 spike *proves* whether the hook fires for swarm subagents.

### 2.3 Condense offline — Haiku mesh under a Sonnet lead (ADR-113)

The 40M-word→Class-Summary compression is done by an **offline orchestrator-worker mesh**, not on any request path. A **Sonnet lead** batches the ~14,718 classes, assembles each class's v2 JSON-LD `Class` block + depth-1 neighbourhood, dispatches to **N Haiku workers** that condense each to a ~100–150-tok Class Summary, then dedupes/normalises/validates and writes via `memory_store(ns:'ontology-classes', upsert:true)`. Triggered incrementally on GitHubSync/elevation. Optionally reused as a bounded on-demand map-reduce for opus-tier `mode=expand depth=2` queries whose raw subgraph exceeds budget — **never on the PUSH path**.

### 2.4 Read pervasive, write governed and untouched

The brain is **read-only**. The sole mutation path is unchanged: `ontology_propose → Whelk EL consistency gate → GitHub PR / ACSP forum panel (Nostr 31400-31405) → human merge`. No ungoverned `/api/ontology/load`; `AGENTBOX_ONTOLOGY_DIRECT_LOAD` stays off. The unauthenticated `propose` hole and the duplicate `/ontology` scope are closed as P0 (ADR-120 / ADR-118).

### 2.5 Structural overflow safety + verifiable liveness

Token budgets are tier-aware and enforced in the library, with a **local clamp in the hook** (PUSH must not trust a network response) and a **server-side SPARQL LIMIT/row/byte clamp in VisionClaw** as a hard prerequisite (ADR-117, WS-0). `full:true` page bodies are forbidden below sonnet and chunked to ≤ budget where allowed. Liveness is proven by a **per-channel matrix + startup canary** (ADR-119), not by wiring.

---

## 3. Recall economics — why condense, and does it let us recall *more*? (design opinion)

The operator asked whether a Haiku-mesh condensation tier "might allow us to recall more data." **My assessment: yes — materially, and in the right place — with one honest caveat.**

- **It is a genuine recall multiplier *offline*.** The binding constraint on recall is not how much the ontology *holds*, it is how much can be *distilled into a fixed context budget*. A Haiku mesh reads large volumes of raw subgraph in parallel and emits compact, structured facts; the lead assembles only the distillate. So a fixed ≤2,000-tok sonnet budget can be backed by a class index that surveyed *all* ~14.7k classes, not just the handful that fit raw. Effective corpus coverage rises by ~1–2 orders of magnitude at constant per-call context cost. This is the correct economics because the index is **built once and reused across every call** — the Haiku cost amortises.
- **For a knowledge graph the lossy part is small.** The structured backbone (IRIs, `subClassOf`/`enables`/`requires`, domain, maturity) compresses essentially losslessly; only the free-text definition nuance is summarised. Because we **bind structure pervasively and fetch prose only on explicit `full:true`**, condensation throws away little that the augmentation path actually uses.
- **The caveat — and its mitigation.** Generative condensation can normalise away real distinctions or hallucinate. Mitigation is baked into ADR-113: the Sonnet lead validates each summary's structured claims against the source triples (SPARQL-verifiable), and workers are constrained to **extract, not infer**. The generative latitude is confined to the prose `definition` field; the facts are ground-truthable.
- **On-demand deep-expand condensation is situational, not default.** It raises recall on a single deep query but adds LLM latency, so it is opus-tier, opt-in, latency-gated, and fail-open to raw-truncated Turtle. The offline index is where the recall win is real and cheap; the on-demand path is a power-user convenience.

**Net opinion:** adopt the condensation mesh as the *offline index builder* (high-value, low-risk, amortised) and as an *optional* opus-tier deep-expand step (situational). Do **not** put any LLM call on the synchronous PUSH path. This is recorded as ADR-113 with the model IDs pinned for reproducible summaries (open question PRD-020 §10.7).

---

## 4. Consequences

**Positive**
- Satisfies the requirement (option to query, pervasively per the matrix) with **two seam edits + one tool + one library**, not 15 bespoke integrations.
- The overflow guarantee is **structural** (one budget governor + server clamp), not per-caller discretionary.
- Maximum reuse: extends `ontology-bridge.js`, the consultant chokepoint, the live hook, RuVector, and the governed propose loop; creates only the library, the class index + condensation mesh, and the telemetry.
- Avoids the new-service tax (second auth domain, Fastify plugin + manifest gate + 3-class adapter-contract tests, an HTTP hop on the hot path).
- Closes two live governance holes (unauthenticated `propose`, `/load` Whelk-bypass) as a side effect.
- Recall scales with the offline index, not with per-call context budget.

**Negative / costs**
- A shared library imported across process boundaries means **version discipline** (each process must run a compatible version); mitigated by keeping the library small and pinning it in agentbox's dependency set.
- The Haiku-mesh index build is a real one-time/refresh compute cost (~14.7k classes; ~3× the naïve 4,952 estimate); mitigated by incremental refresh on sync.
- The PUSH channel's synchronous-local design means it cannot do embedding-based seeding on the hot path — it uses trigram matching over the cache, which is lower-precision than HNSW. Accepted: PUSH is a *breadcrumb* ("here is the relevant seed, expand via `ontology_ask`"), not a full retrieval.
- Verifiable-liveness telemetry + per-channel canary is extra build surface (WS-6) — but it is the explicit price of not repeating PRD-018.

**Neutral**
- Default-off at ship: the capability is *installed* before it is *delivered*; WS-6 telemetry sign-off is the activation milestone.

---

## 5. Decision register (sibling ADRs, to be split out)

| ADR | Category | Decision | Status |
|---|---|---|---|
| **ADR-113** | Agent Orchestration | Haiku condensation mesh under a Sonnet lead (offline build/refresh + optional opus deep-expand; excluded from PUSH) | proposed |
| **ADR-114** | Feature Pipeline | Memory substrate: RuVector (xinference HNSW) for the **seed leg only** — verified as the only live embeddings+ANN (VisionClaw's Qdrant is orphaned, its embeddings fake); binding is 3-store (Oxigraph truth + RuVector seed + in-process PUSH cache). *(written — standalone file)* | proposed |
| **ADR-115** | Architecture | Terse Turtle over SPARQL-Results JSON (2–9× token reduction) | proposed |
| **ADR-116** | Architecture | Model-tier budgets (booster≤80/haiku≤500/sonnet≤2,000/opus≤6,000); `full:true` capped+tier-gated; local hook clamp | proposed |
| **ADR-117** | Architecture | Server-side SPARQL clamp (default LIMIT + row/byte cap) as hard invariant; forbid `SERVICE` | proposed |
| **ADR-118** | Security | Read-pervasive/write-untouched; harden `/load`; resolve duplicate `/ontology` scope | proposed |
| **ADR-119** | Operations | Fail-open + per-channel verifiable liveness (anti-PRD-018); cause-split `fail_open_count` | proposed |
| **ADR-120** | Security (P0) | Authenticate `/api/ontology-agent/propose`; bind `agent_id` to verified did:nostr (NIP-98); rate-limit | proposed |
| **ADR-121** | Architecture + Orchestration + Security | Self-improving ontology via governed writeback (elevation flywheel): W0 derived materialisation, W1 governed enrichment, W2 autonomous closure; deletes the `writeback_triggered` ghost | **written** |
| **ADR-122** | Security / Governance | Two-speed writeback — governance routing by epistemic class: L1 structural→forum gate, L2 volatile ABox→fenced `:observed` auto (never auto-promoted), L3 derived→auto | **written** |
| **ADR-123** | Architecture + Security + UX | Voice-mediated governance sign-off: immersive voice agent as authenticated L1 decision-queue client; closes the `/api/broker/inbox` ghost | **written** |

---

## 6. Alternatives considered

1. **New `POST /v1/ontology/ask` Fastify service (the raw synthesis).** Rejected: redundant with the bridge, adds a second auth domain and an HTTP hop on the hot path, and the synthesis mis-specified it as Express. The shared-library form delivers the same "one brain" property without the tax.
2. **Pull-only (Approach A — universal MCP tool).** Rejected as the whole answer: opt-in; cannot reach the opaque claude-flow substrate; but **retained as PULL-B** because universal availability is genuinely useful.
3. **Push-only (Approach B — hook breadcrumb).** Rejected as the whole answer: structurally cannot reach the consultant tier (the place agentbox itself originates external LLM calls); but **retained as PUSH** for the interactive/opaque tier.
4. **Embedding-seeded PUSH on the hot path.** Rejected: an xinference round-trip violates the <15 ms synchronous hook budget; degenerates to fire-and-forget that emits nothing. PUSH uses local trigram matching; HNSW seeding lives on the async PULL path.
5. **LLM condensation on every call.** Rejected: latency + cost on the hot path. Condensation is offline (amortised) + optional opus deep-expand only.

---

## 7. Verification (how we will know it is real — anti-PRD-018)

This ADR is declared **implemented** only when:
1. WS-0 route-dump proves a single `/load` gating and a `SELECT ?s ?p ?o` returns ≤ cap; `SERVICE` is rejected.
2. The bridge-start self-test passes one **authed** SELECT on boot and fails loudly otherwise.
3. An unauthenticated `propose` is rejected and a forged `agent_id` is overridden by the verified did:nostr.
4. A consult with augment on **actually changed** `context_excerpt` (contract test).
5. The PUSH integration test shows a locally-clamped `[ONTOLOGY]` line in stdout within budget on a relevant turn and nothing on an off-topic turn.
6. The per-channel liveness matrix + startup canary drive a non-zero injection through each enabled channel and assert downstream receipt; `fail_open_count{cause=auth}` == 0 in steady state.

Until then the binding is reported as **installed, not delivered**.
