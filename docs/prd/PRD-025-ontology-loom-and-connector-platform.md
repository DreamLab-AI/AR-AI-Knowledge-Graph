---
id: PRD-025
title: "Ontology Loom & Connector Platform: A Portable Reasoning Node, One Corpus Authority, and the Deferred Distillation Loop"
status: proposed
date: 2026-08-11
authors: VisionFlow operator (did:nostr:jjohare) + opus exploration mesh
linked_adrs: [ADR-135 (loom façade + deferred distillation decisions)]
linked_ddd: docs/ddd/ddd-ontology-connector-context.md
relates: [
  PRD-020 (pervasive ontology augmentation),
  VisionClaw PRD-022 (semantic-trust-layer),
  PRD-023 (gap-close),
  PRD-024 (final-mile closeout),
  ADR-112 (retrieval spine / one brain),
  ADR-113 (condensation mesh),
  ADR-116 (tiered token budgets),
  ADR-119 (verifiable liveness telemetry),
  ADR-121 (self-improving writeback loop),
  ADR-075 (IS-Envelope contract),
  ADR-090 (hexagonal ring order),
  ADR-050 VisionClaw (pod-backed-kgnode-schema),
  agentbox ADR-013 (canonical URI grammar),
  agentbox ADR-049 (bitemporal facts / runtime provenance),
  agentbox ADR-050 (decision-elevation inverse-corpus-path),
  agentbox ADR-051 (harness-side loom-client decisions — proposed),
  agentbox PRD-022 (semantic-integrity-provenance-decisions)
]
supersedes: "PRD-025 connector-platform draft (pre-Loom reframe); this is the operator-reframed capstone"
---

# PRD-025 — Ontology Loom & Connector Platform

**Status:** Proposed (adversarially reviewed; operator-reframed 2026-08-11 — see §12)
**Date:** 2026-08-11
**Owner:** VisionFlow operator (Dr J. O'Hare, did:nostr:jjohare)
**Companion decisions:** VisionClaw [ADR-135](../adr/ADR-135-ontology-loom-facade-and-deferred-distillation.md); agentbox ADR-051 (harness-side loom-client decisions, proposed)
**Bounded context:** [ddd-ontology-connector-context.md](../ddd/ddd-ontology-connector-context.md) (BC24 — OntologyConnector)

> **EXECUTION NOTE (read first).** This is a **design + workstream plan**. It changes no
> VisionClaw / agentbox / logseq / HP code by itself; implementation is the WS-A…WS-J build
> order in §6, each gated by the evidence bars in §10. This is a **dev/test estate**, so we
> build the **target end-state directly** (§6) rather than staging a live migration — the
> adversarial correctness and security must-fixes still all apply to that end-state; they are
> not staging scaffolding and are not waived by going direct.

> **Citation discipline (mandatory).** Two `PRD-022`s and two `ADR-050`s exist across repos.
> This document repo-qualifies every cross-repo citation:
> **VisionClaw PRD-022** = *semantic-trust-layer*; **agentbox PRD-022** = *semantic-integrity-
> provenance-decisions*. **VisionClaw ADR-050** = *pod-backed-kgnode-schema*; **agentbox
> ADR-050** = *decision-elevation-inverse-corpus-path*. An unqualified `PRD-022` / `ADR-050`
> in this document is a defect.

---

## 1. Problem — the loop was designed but never closed

VisionFlow was conceived to enhance LLM responses by fusing five capabilities — agentic
intelligence, agentic search, reasoning, local data, and ontology/knowledge-graph traversal
rendered into semi-structured markdown. Every one of those capabilities now ships. None of
them close the loop, for two structural reasons.

**(a) The ontology intelligence is scattered across four codebases — and duplicated.** The
same capability is implemented more than once, in more than one repo, and the copies have
already drifted (operator memory: an 8152-vs-5975 class-count divergence; a stale `:assert`
graph). The audited duplication:

| Capability | Copy 1 | Copy 2 | Copy 3 | Drift risk |
|---|---|---|---|---|
| **Reasoning** | VisionClaw Whelk EL++ → `:inferred` | logseq `pipeline/reason.py` transitive closure → `ontology-inferred.ttl` | — | Two closures, two answers |
| **Retrieval** | agentbox `@agentbox/ontology-retrieval` "one brain" (ADR-112) | HP `ontology_scaffold.py` | `ontology-mcp` | Three index builders |
| **Parsing** | Rust `canonical-entity` / `knowledge_graph_parser` | Python `jsonld_parser` | — | Two corpus→entity parses |
| **Conflict/consistency** | Whelk consistency | logseq `conflicts.py` (typed `ConflictReport`) | — | Two gates, uncomposed |

Nothing carries a **corpus identity** through the pipeline, so no two copies can even be
proven to be looking at the same generation. Drift is not a risk; it is the observed default.

**(b) VisionClaw's GitHub sync is clunky, and it is clunky by construction.** The
`github_sync_service` `force_full` path does a **CLEAR+INSERT of the entire `:assert` graph**.
That wipes runtime decision-class triples and needs the agentbox ADR-050 (decision-elevation
inverse-corpus-path) trick merely to survive its own reload. The root cause is a
category error: a real-time render/physics engine is also being asked to play
**corpus-lifecycle-manager**. Those are different jobs at different clocks.

**Consequence:** the "slow external reasoner" leg was never built at all. An agent that wants
a deeply-distilled, corpus-grounded summary from a local GPU model (10 s–10 min) has no way to
request one without blocking a turn — which the ADR-112 one-brain / no-hot-path-LLM law
forbids. So the capstone loop — *agent asks a hard question → a slow reasoner distils the
corpus → the distillate recombines into a parallel agent workflow, provenance-provable* —
does not exist end-to-end.

This PRD closes both gaps with **one** architectural move: name the missing owner.

---

## 2. Vision — the Ontology Loom

> **NAME (operator decision, 2026-08-11): the component is `Loom`** (the Ontology Loom /
> VisionLoom). It weaves the corpus into a reasoned ontology, holds it as the canonical source
> of truth, and serves that intelligence behind a **stable, model-swappable façade**. The node
> role is *the Loom*; the endpoint is *the Loom façade*; a containerised deployment is *a Loom
> sidecar*; the corpus snapshots it publishes are *Loom generations*.

The **Loom is a first-class VisionFlow node** — a peer of VisionClaw, agentbox, and the
published site — defined by a **role with a stable façade contract**, of which HP-Desktop is
the *reference deployment* and which is **host-portable**. It has two deliberately separable
facets:

- **Lifecycle + façade facet** (lightweight, always-on-ish; needs git credentials + outbound
  internet; stdlib-portable). Owns the canonical ontology lifecycle end-to-end: **sync** the
  corpus from GitHub → compute atomic **generations** (build-manifest + sha) → run
  **reasoning** (EL closure) → serve **retrieval/scaffold/distillation** → **publish** the
  cloud replica → **manage the GitHub write-back** (enrichment/elevation commits). Exposes
  **ONE** stable façade endpoint.
- **Distillation facet** (GPU; the *swappable model*). The LLM behind the façade. The
  guarantee is **"swap models only on the remote side"** — Gemma → Muse-Glimmer → next — with
  **zero consumer change**, because the façade contract is stable and model identity is
  carried *in results*, never in the endpoint. This is the "no technical debt on upgrade"
  property, and it holds at two levels: swap the model behind the façade today; swap the Loom
  host tomorrow, without consumers noticing.

### 2.1 Deployment topologies (portable module, pluggable seam)

Because the two facets separate cleanly, the Loom is a portable module with a stable contract
and **pluggable deployment**:

- **Topology A — HP host (reference):** façade + distillation co-located on HP, GPU-local.
  Distillation backend = HP `llama.cpp` `:8084`/`:8085`.
- **Topology B — Docker sidecar (on `visionclaw_network`):** the lightweight façade+lifecycle
  facet as a container; distillation **delegated** to a configured backend via
  `DISTILL_BACKEND_URL` (→ HP `:8084`, a cloud model, or a local model container). No GPU in
  the sidecar — the model is an OpenAI-compatible URL behind the façade.

Both expose the **identical** façade contract; consumers never learn which is running. The
distillation backend is a **config line, not an architecture change**. ADR-135 specifies the
façade contract as deployment-agnostic and names `DISTILL_BACKEND_URL` (plus model-swap-behind)
as the pluggable seam. The lifecycle+façade facet is stdlib-portable (HP, container, or ml);
GPU is only needed by whichever backend the façade points at.

### 2.2 The clean node boundary

| Node | Owns | Stops owning |
|---|---|---|
| **Loom** | Corpus lifecycle (sync, generations, reason, publish, GitHub write-back) **and** serving ontology intelligence (retrieval, scaffold, distillation, model-swap) behind the façade | — |
| **VisionClaw** | Visualisation, GPU physics, live-linkage (`graphUpdated`), the governance **write DOOR** (`propose`) | **Corpus sync.** It consumes Loom **generations** (atomic "load generation N"), never CLEAR+INSERT. Governance *outcomes* flow to the Loom, which manages the GitHub merge. |
| **agentbox** | The ADR-112 in-process "one brain" hot path | Independent index building — it resolves authoritative state **through the Loom façade** |
| **Published site** | A **generation the Loom publishes** (cloud read replica) | — It is the always-available fallback when the Loom host is down: the corpus changes slowly, stale reads are fine, only *fresh distillation* pauses. |

---

## 3. Connector model — three latency classes over one Loom-managed corpus

**Thesis:** *one corpus, one Loom, one contract, three latency classes, one provenance
grammar.* A **connector** = `(backend × transport × latency-class)` exposing a uniform JSON
contract, all resolving to **Loom-managed generations**. The three historical "connector
classes" are not three systems — they are the **three ways a consumer reaches the same
Loom-managed corpus** at three latency classes. Coverage is an **audited matrix** (PRD-020
binding constraint — never the word "ALL"):

| # | Class | Backend | Transport | Latency | Freshness | Auth | Coverage | Status |
|---|---|---|---|---|---|---|---|---|
| 1 | `static-cloud` | narrativegoldmine.com artifacts (page JSON incl. `inferredSuperClasses`, scaffold-index v1, prose-index v1, `ontology.ttl` + `ontology-inferred.ttl`, NGG1 tiers) — a Loom-published generation | HTTPS GET, CORS-open | ms reads | batch-CI (per corpus push) | anonymous | any HTTP consumer, anywhere | ✅ (contract formalised by WS-A/WS-B) |
| 2 | `live-lan` | VisionClaw HTTP (`discover` anon / `sparql` clamped power_user / `inferred` / `state-at`) + agentbox ontology-bridge MCP + in-process `@agentbox/ontology-retrieval` | HTTP JSON / MCP / in-process | fast-lan | live (`graphUpdated` push) | anon reads / NIP-98 power surfaces | in-container agents + LAN | ✅ |
| 3 | `external-reasoner` (slow-llm) | HP-Desktop Loom deployment: distillation provider producing signed distilled summaries | pull-worker job queue (this PRD) | slow-llm, 10 s–10 min | Loom generation (sha-pinned) | strict NIP-98 + signed envelopes | **N=1 provider today (HP)** | ◐ (retrieval stack deployed; job layer = this PRD) |

**"Platform" is earned, not declared.** Class 3 generalises to any second provider via the
distillation backend seam, but the **platform claim is deferred until a second provider
lands**. Until then this is honestly *a deferred distillation channel with a provider-plugin
seam* — stated, not oversold.

### 3.1 Empirical grounding (HP uplift benchmark, 2026-08-11)

The Loom's retrieval facet is not speculative. A held-out 37-question, seed-42 benchmark
(objective graph-derived gold; paired raw/scaffold/prose/tools axes with bootstrap 95% CIs —
`bench_ontology_uplift.py`, deployed on HP) produced, for Muse-Glimmer-30B:

| Axis | Mean recall | vs raw |
|---|---|---|
| raw (no grounding) | 0.27 | — |
| **scaffold** (static structured injection) | **0.94** | **~3.5×** (CI-significant) |
| prose-enriched scaffold | 0.95 | ~nothing over structured |
| tools (agentic graph traversal) | 0.65 | *below* static injection |

Three findings, all load-bearing for this design (Gemma cross-check in flight):

1. **Static structured scaffold is the dominant win (~3.5×).** This validates the Loom's
   *scaffold/index generation as the primary grounding product* — the structured ontology
   (taxonomy + typed relations + definitions) does the work.
2. **Prose adds ~nothing over structured.** The prose-index is a cheap complement, not the
   value driver — the Loom prioritises the structured generation; prose stays optional.
3. **Agentic tool-traversal underperforms static injection (0.65 < 0.94).** For this
   substrate, letting a model traverse the graph itself is *worse* than feeding it a
   pre-computed scaffold. **Design consequence:** the Loom serves pre-computed scaffold
   generations as the primary grounding path, and its distillation facet (§5) is
   **retrieval-fed map-reduce (scaffold → N LLM calls → merge), never tool-driven
   exploration** — the benchmark says the model should be *fed*, not sent traversing. The
   live-lan traversal tools (`kg_neighbors`/`kg_pathfind`) remain available but are
   *secondary* to scaffold injection for grounding.

---

## 4. Consolidation map — one authority each

The Loom is the **single owner** of each duplicated capability; the existing fast in-process
pieces become **thin clients of the Loom's authoritative generation**. This *preserves*
ADR-112 (no LLM / no network on the hot path): the Loom owns the **slow/authoritative** path
(lifecycle, reasoning, distillation, index generation); the in-process libraries keep their
**fast local** paths but resolve authoritative state **from** the Loom instead of re-deriving
it.

| Capability | Loom owns (authority) | Thin clients | Invariant |
|---|---|---|---|
| **Reasoning** | The authoritative EL closure over the current generation | Published `ontology-inferred.ttl` **and** VisionClaw `:inferred` both *derive from that one generation* | ONE authority, **conformance-tested** so the two never drift again |
| **Retrieval** | The **INDEX generation** (scaffold-index / prose-index / RuVector-condense) derived **once**, by the Loom | agentbox one-brain (ADR-112), HP scaffold, `ontology-mcp` become index *consumers*; the ADR-112 in-process library stays the hot-path client, it just stops building its own index | ONE index; hot path unchanged |
| **Parsing + sync** | Corpus→canonical-entity parse lives with the Loom (which owns sync) | VisionClaw consumes **generations**, never re-parse+CLEAR+INSERT | ONE parser, ONE sync |
| **Conflict/consistency** | `conflicts.py` (typed `ConflictReport`) = Loom **pre-publish** gate; Whelk consistency = Loom **pre-assert** gate | — | Composed, not duplicated |

**Which reasoning engine is canonical is an OPEN DECISION (§12):** promote Whelk into the Loom
vs. make `reason.py` authoritative and have VisionClaw load its output. ADR-135 records the
choice; the invariant here is *one authority, conformance-tested*, regardless of which engine
wins. The DDD context (BC24) shows the ownership migration explicitly (see §11).

---

## 5. The deferred distillation loop — the flagship capability

This is the only genuinely **new machinery** (with the Loom node itself). Everything else is
alignment + contract formalisation over shipped surfaces. It is **one capability of the Loom**,
not the whole product.

### 5.1 Shape of the loop

```
turn N     : agent → ontology_distill_submit(scope, question, budget) → {jobUrn, beadId}
             agent CONTINUES working (search, reasoning) IN PARALLEL — never blocks
[async]    : HP jobd PULLS the job → scaffold retrieval → 1..N LLM calls (map-reduce) →
             signs a sha-pinned result envelope → delivers (RuVector) → CAS-closes the bead
turn N+k   : agent → ontology_distill_fetch(jobUrn) → sig-verified, budget-clamped distillate
             → recombine with the search it did in parallel
```

**No-synchronous-await LAW (binding).** Distillation is *submit-in-turn-N,
recombine-in-a-later-turn-or-worker*. **No consumer ever holds a turn open on a distill job.**
Fast-wake only *schedules* the recombine; it never gates correctness. (The 600 s consultant
cap makes synchronous await structurally impossible anyway.)

### 5.2 Identity — a job URN, NOT a plain bead

The shipped bead mint (`local-sqlite.js:85,121`) injects `nonce: crypto.randomUUID()` so
same-title beads within one millisecond get **unique** ids — the *opposite* of
content-addressed. Therefore we do **not** overload the `bead` kind. Instead:

- Mint a **distinct `job` URN kind** through the agentbox ADR-013 §6 URI-grammar extension API
  (KINDS entry + resolver case + contract test), via `uris.js` — the URI/DID grammar stays
  **closed**, no ad-hoc string building:
  `urn:agentbox:job:<pubkey>:<sha256-12>`, `contentAddressed:true`, `ownerScope:true`,
  content-addressed **over the identity core only**.
- The distill **bead** (the work-ledger row) is minted **normally** (nonce-carrying) and
  *carries* the job URN in a typed field. The job URN is the **idempotency + provenance
  anchor**; resubmitting an identical request resolves to the **same** job URN →
  **dedupe-on-create** at the submit tool.

**Content-address identity core (RFC 8785 JCS).** Hash **only**:

```json
{ "kind": "ontology.distill",
  "corpusSha": "<generation sha>",
  "scope": { "slugs": ["<sorted+deduped>"] | "domain": "<name>" | "question": "<normalised>" },
  "budget_tokens": 4096 }
```

`budget_tokens` **is** content (it changes the answer) — decided, documented. **Execution
fields are NOT hashed:** `deadline`, `requester`, `sig`, result rendezvous, `model_policy`.
Canonicalisation is **RFC 8785 JCS** (named so two implementations agree); arrays sorted where
semantically unordered (slugs), preserved where ordered.

### 5.3 Provider-side lifecycle (HP `jobd`, stdlib pull-worker)

State machine:

```
submitted → admitted → queued → running → distilled → stamped → delivered → acked
                │                                                              │
                ├─ waiting-for-model ──(max_wait)──► expired(model-unavailable) │
                ├─ preempted ──► queued                                         │
                └─ (deadline)  ──► expired(cause: gpu-contended | corpus-unavailable | timeout)
```

Laws (all binding):

- **Concurrency 1** (ADR-113 §2.2, cited exact); the queue sits **in front of** the model,
  never inside it.
- **Claim-time admission** replaces "429 past cap" (a pull worker has nobody to 429): `jobd`
  claims only when `depth < cap` **AND** estimated completion fits the deadline; otherwise it
  leaves the job unclaimed for the harness reaper.
- **`admitted`** probes `/v1/models`, records the **exact model id + file metadata**, and
  takes the GPU flock **non-blocking**.
- **GPU flock must be CREATED, not referenced:** ship `~/githubs/llm-server/.gpu.lock`, patch
  the bench harnesses + protocol to take it (shared for benches), `jobd` tries it
  non-blocking. Arbitration: **benches always win**; `jobd` never preempts/kills a bench; jobs
  blocked past deadline expire `cause=gpu-contended`.
- **Re-probe model identity per LLM call**; abort on mid-job change. Record the model file
  path/metadata (guards a same-alias gguf swap; the residual risk is noted, not eliminated).
- **`model_policy`**: `any-serving | pinned:<id>`; `pinned` + not loaded → `waiting-for-model`
  with `max_wait` → `expired(model-unavailable)`.
- **FAIL-LABELLED, not fail-open:** a `scaffold_engaged=false` output is **never** delivered as
  ontology-grounded — it is quarantined. Cause-split telemetry throughout.
- `jobd` is **stateless-by-design**: queue durability = re-pull on restart; nothing is
  persisted on HP.

### 5.4 Corpus identity & the mirror (WS-A — the root fix)

No envelope rule below is even *expressible* until the corpus carries an identity.

- The logseq pipeline emits `api/build-manifest.json` **written LAST**:
  `{ commitSha (GITHUB_SHA), buildId, generatedAt, pipelineVersion, artifacts: { <path>: {sha256, bytes, count} } }`.
  Thread **one shared timestamp + commitSha** through every emitter; add `owl:versionIRI` +
  `prov:wasDerivedFrom <repo@sha>` to **both** TTL headers.
- `mirror.sh` fetches build-manifest **first**, verifies each artifact `sha256`, then
  **atomically** publishes `data/` as one generation (write `data.new/`, `rename`) — this
  kills the mixed-build window.
- **corpusSha resolution** (`corpusSha_match: exact | at_least | latest`): on admit, if
  `job.corpusSha != mirror generation`, `jobd` runs **one** on-demand mirror refresh (cron
  cadence is irrelevant); `exact` + still-mismatched → terminal `corpus-unavailable`; the
  result envelope **always** carries `corpus_generation_used` + a mismatch flag.

### 5.5 Result envelope (extends the bench-row schema)

```json
{
  "jobUrn": "urn:agentbox:job:<pubkey>:<sha256-12>",
  "summary": "<distilled markdown>",
  "corpusSha_used": "<sha>",
  "corpus_generation": "<buildId>",
  "model_id_probed": "gemma-2-27b-it-Q5_K_M",
  "model_file_meta": { "path": "...", "size": 0, "mtime": "..." },
  "toolkit_rev": "<git sha>",
  "llama_build": "<build id>",
  "engaged_class_slugs": ["..."],
  "scaffold_engaged": true,
  "injected_tokens": 0,
  "tokens_used": 0,
  "latency_ms": 0,
  "derivation_labels": { "<slug>": "asserted | inferred | summary" },
  "corpusNature": "synthetic-ai-generated-human-directed",
  "generatedAt": "2026-08-11T...Z",
  "lease_epoch": 3,
  "retrieval_transcript_sha": "<sha256>",
  "sig": "<BIP-340 over the canonicalised envelope, HP machine did:nostr>"
}
```

`corpusNature: "synthetic-ai-generated-human-directed"` is **mandatory corpus-honesty** — the
corpus is AI-generated under human direction and every distillate says so. `scaffold_engaged`
/ `engaged_class_slugs` **SHOULD** be bound to a `retrieval_transcript_sha` the harness can
reproduce (self-assertion hardening — the provider cannot claim grounding it did not perform).

### 5.6 Delivery — Phase-1 is TWO paths, not five

Phase-1 load-bearing sinks: **(a) RuVector payload** + **(c) bead close**. Strict ordering;
**close is the linearisation point**; invariant **"closed-done ⇒ payload retrievable at
jobUrn."**

- **(a) RuVector write** (harness-side adapter — HP has no MCP access):
  `memory_store(key=jobUrn, namespace='ontology-distilled')` on receipt, via the RuVector
  **MCP tools only** (`mcp__claude-flow__memory_*` — CLI / raw SQL bypass the embedding
  pipeline and are invisible to search). Typed-metadata gate REQUIRED for this namespace.
  **First-write-wins on content:** an existing key with a *different* result sha is
  **rejected + logged** as divergence, never upserted.
  **TTL law:** `TTL ≥ consumer_deadline + lease_TTL × max_redeliveries + sweep_period + slack`,
  clock starting at delivery.
- **(c) Bead close is a CAS** (WS-C):
  `UPDATE ... SET status='closed' WHERE id=? AND status='claimed' AND actor=? AND lease_epoch=?`
  returning changes-count; a failed CAS is a **no-op**.
- **Lease fencing** (WS-C): a monotonic `lease_epoch` is incremented on **every**
  claim/reclaim and carried in the envelope; **both sinks reject stale-epoch deliveries**.
  `claim()` is a conditional `UPDATE ... WHERE actor IS NULL` (changes-count).
- **getReady is outcome-aware** (not outcome-blind): the recombine worker MUST read the
  blocker's `outcome` **before** dereferencing `result_ref` — `done` → fetch;
  `expired | failed` → propagate a **labelled failure** into search-only recombination.

**Phase-2 delivery sinks** (build-order later): **(b)** VisionClaw fenced `:summary` durable
copy + provenance emit; **(d)** agent-events fast-wake (fire-and-forget accelerator ONLY,
strictly-after-close); **(e)** kind-30840 operator digest.

### 5.7 Consumer-side tools — the missing surface (WS-J)

The single biggest gap: without these, a real agent cannot close the loop. Three
`ontology-bridge` MCP tools so **no agent ever hand-rolls the six steps** (canonicalise, mint,
sign, create beads, poll, clamp):

- `ontology_distill_submit({ scope, question, budget_tokens, deadline, model_policy, corpusSha_match })`
  → mints the job URN (via management-api `uris`), **signs with the HARNESS machine key**
  (individual agents never touch keys), creates the distill + recombine beads, returns
  `{ jobUrn, beadId }`. **Dedupe-on-create.**
- `ontology_distill_fetch({ jobUrn })` → **budget-clamped** (ADR-116 tiers), **sig-verified**
  retrieval of the delivered result; **fail-labelled** on miss/unverified.
- `ontology_distill_await({ jobUrn, deadline })` → **deadline-bounded** poll for mid-workflow
  use. It never breaches the no-synchronous-await law: it polls **across the deadline** and
  returns whatever landed (or a labelled timeout); it does **not** hold a turn open on the LLM.

Two consumption modes, stated explicitly: **fire-and-collect-later** (cross-session — the
default) and **deadline-bounded await** (mid-workflow, bounded). **Recombine-worker ownership:**
Phase-1 consumption is **tool-side** (`await`/`fetch`); the recombine bead is optional. The
**autonomous** recombine-bead worker (a claude-flow daemon consumer polling `getReady`) is a
Phase-2 workstream with a named owner.

### 5.8 Provenance routing (portable-reification is a MAPPING target, not "canonical")

Neither provenance graph is declared canonical — both have binding owners: VisionClaw PRD-022
(semantic-trust-layer) constraint 3 owns `urn:ngm:graph:provenance`; agentbox ADR-049
(bitemporal facts / runtime provenance) owns `urn:agentbox:graph:provenance`. This PRD
specifies a **reconciliation MAPPING** (an alignment vocabulary; each graph keeps its owner's
invariants), **not** a merge.

- The distillate's PROV-O triples route through the **VisionClaw PRD-022 (semantic-trust-layer)
  ProvenanceEmitter** into `urn:ngm:graph:provenance` — **NOT** into `:summary` (the `/derived`
  fence writes summary payload ONLY; PROV-O in `:summary` would violate VisionClaw PRD-022
  constraint 3).
- `POST /api/ontology/derived` receives **only** the summary quads (`:summary` / `:usage`),
  fenced at two layers (verified in code: `ontology_derived_handler.rs:30-40`).

---

## 6. Direct-to-target — the honest build order (NOT live phases)

**Operator decision (2026-08-11): NOT staged.** This is a dev/test estate, not a live system,
so the phased live-migration is dropped — it would only add transitional shims, dual code
paths, and confusion. **Build the target end-state directly** and debug the integrated system
from there. The target IS the design:

- The Loom OWNS the full loop: sync FROM GitHub → generations + reason → serve the façade
  (retrieval/scaffold/distillation, model-swap-behind) → publish the cloud replica → manage the
  GitHub **write** side (enrichment/elevation commits).
- VisionClaw `github_sync_service` CLEAR+INSERT is **RETIRED**; VisionClaw loads Loom
  generations (clean atomic feed) and is a **pure consumer** (viz + physics + live-linkage +
  propose door; governance outcomes flow to the Loom for the GitHub merge).
- Duplicated intelligence collapses to **one authority each** (§4); the fast in-process pieces
  become **thin clients immediately**, not after a migration.

The workstreams below are a **build order toward the one end-state**, not live-migration
phases. "Phase-2" on WS-F/H/I means "sequence after the core loop stands up," **not** "protect
a running system" — cut over and debug.

### Build order — close the loop (WS-A · B · C · D · E · G · J)

| WS | Owner (repo) | Content | Order |
|---|---|---|---|
| **WS-A** | logseq pipeline | `api/build-manifest.json` (written last) + one shared `GITHUB_SHA`/timestamp through all emitters + `owl:versionIRI` + `prov:wasDerivedFrom <repo@sha>` in both TTL headers; `mirror.sh` manifest-first, per-artifact sha256 verify, **atomic** `data.new/`→rename. **Root fix — no envelope rule exists without it.** | 1 |
| **WS-B** | logseq | JSON Schemas for the **two new envelopes** (job envelope + result envelope) under `api/schema/`. (Page/scaffold/prose retro-schemas: later polish.) | 1 |
| **WS-C** | agentbox | Beads: **`job` URN kind** (deterministic, via ADR-013 §6), typed `result_ref {namespace,key,content_sha}`, `lease_epoch` + claim-lease + reclaim-TTL, **CAS close**, conditional claim (`WHERE actor IS NULL`). **Schema migration + contract tests — NOT "~20 lines."** | 1 |
| **WS-D** | agentbox | management-api **hp-ontology provider door**: NEW verbs `pending-per-provider` / `claim` / `result-upload` (NOT the existing local-subprocess `/v1/tasks`); **strict-nip98** (drop Bearer); claim lease bound to claimant's verified pubkey. | 1 |
| **WS-E** | HP toolkit | HP `jobd` (stdlib pull-worker): state machine (§5.3), claim-time admission, per-call model probe, **GPU flock creation** + bench-harness patch, manifest-first atomic mirror + on-demand refresh, **heartbeat**, `Restart=on-failure` unit + `enable --now` runbook step with a verify command. | 2 |
| **WS-G** | agentbox + HP | Liveness (ADR-119 extension): jobd **heartbeat every poll**; periodic **cap-exempt canary** with a harness-side landing deadline; **deadline reaper** + **reconciliation janitor** (§9). | 2 |
| **WS-J** | agentbox | **Consumer MCP tools** `ontology_distill_submit` / `_fetch` / `_await` (§5.7) + recombine ownership. **Same weight as WS-E — without it the loop does not close for a real agent.** | 2 |

### Build order — generalise & durability (WS-F · H · I — sequence after the loop stands up)

| WS | Owner (repo) | Content |
|---|---|---|
| **WS-F** | VisionClaw | Derived landing → fenced `:summary` durable copy **with signed-envelope verification**; ProvenanceEmitter routing into `urn:ngm:graph:provenance`; `NotifyGraphUpdated{reason:"derived_summary_written"}` wake. |
| **WS-H** | agentbox + logseq | **Elevation** of significant distillates → `EnrichmentCandidate` → **ElevationActor + KnowledgeEnrichment broker case (ACSP 31402/31403)** composed with the PRD-020 / ADR-121 propose spine → corpus → CI conflict gate → published site. (agentbox ADR-050 (decision-elevation) is a **pattern precedent only**, not the mechanism.) Replay-before-accept is a **hard gate** on this path or the replay claim is dropped from the verification story. |
| **WS-I** | agentbox relay | Nostr relay federation for cloud consumers: **NEW ACSP kinds 31408 `DistillJobRequest` / 31409 `DistillJobResult`** + a new IS-Envelope kind `ontology.distill` via ADR-075 D1. **Leave 31406/31407 (SPARQL `semantic_query`) untouched.** **NIP-59 gift-wrap** scope/question to the provider key; retire NIP-26 delegation (deprecated, unrevocable) → per-consumer NIP-59 capabilities. |

**Smallest honest loop-close:** WS-A + WS-B(2 schemas) + WS-C + WS-D + WS-E + WS-J +
WS-G(reaper/janitor/heartbeat) + the §8 security floor. An agent calls
`ontology_distill_submit` mid-workflow, keeps working, and a later turn (or
`ontology_distill_await` within a bounded deadline) `fetch`es a signed, sha-pinned,
budget-clamped distilled summary to recombine with search. Provenance is provable from the
signed envelope + build-manifest sha. That is the capstone loop, closed.

---

## 7. Binding constraints (verified — honour verbatim)

1. **ADR-112 one-brain / no hot-path LLM.** No LLM call and no network hop on the in-process
   retrieval hot path. The Loom owns the slow/authoritative path; in-process libraries stay
   fast-local and resolve authoritative state *from* the Loom. Distillation is **always**
   deferred/off-turn.
2. **ADR-116 tier budgets.** Every distillate crossing into an agent context passes
   `clampToBudget` at the consuming tier; `ontology_distill_fetch` is budget-clamped.
3. **Write-path never widened.** The governed write path (`propose → Whelk → PR → human merge`)
   is unchanged. Distillates land on read/derived surfaces; elevation (WS-H) re-enters the
   *same* propose spine — it does not open a new write door.
4. **Fail-labelled, not fail-open.** `scaffold_engaged=false` or unverifiable-signature results
   are **quarantined**, never delivered as ontology-grounded.
5. **URI/DID grammar closed.** Bead and job URNs are minted via `uris.js` (agentbox ADR-013)
   only — `job` URN = `urn:agentbox:job:<pubkey>:<sha256-12>`, `sha256-12` truncation; `did:nostr`
   identities are Schnorr-wrapped. No ad-hoc URN string building anywhere.
6. **Corpus-honesty.** Every distillate carries
   `corpusNature: "synthetic-ai-generated-human-directed"`.
7. **SPARQL SERVICE stays forbidden.** ADR-011 S1 blocks `SERVICE` at the handler boundary;
   this PRD does not unblock it. Class-3 federation (WS-I) is relay-mediated, never outbound
   SPARQL.
8. **ADR-090 ring order.** All durable state and outbound calls respect the hexagonal ring
   order; the Loom façade is an adapter-boundary surface, not a domain shortcut.
9. **RuVector MCP-only.** All durable memory flows through `mcp__claude-flow__memory_*`; the
   `claude-flow memory` CLI and raw SQL bypass the embedding pipeline and are invisible to
   search — forbidden for the `ontology-distilled` namespace.

---

## 8. Security posture — verify signatures where trust is consumed

- **Verify the BIP-340 envelope signature at BOTH doors:** the derived write door (WS-F) AND
  the RuVector read (recombine), against a **distiller-provider allowlist** — **NOT** generic
  `power_user`. Generic `power_user` must **not** suffice to write `:summary`.
  Unsigned/unverifiable → **400** (write) / **fail-labelled quarantine** (read); it never
  crosses `clampToBudget`.
- **Reconcile envelope fields** (`jobUrn` / `corpusSha` / `scaffold_engaged`) against the
  submitted job **server-side**; mismatch → reject.
- **Provider door = strict-nip98, drop Bearer.** The shared `MANAGEMENT_API_KEY` is a broadly
  held LAN dev secret with no replay defence. Bind the claim lease to the claimant's **verified
  pubkey**; the delivering pubkey MUST equal the job's designated provider; bind bead close to
  job identity.
- **HP key at rest:** age/agenix-encrypted or hardware-held — **never** a plaintext dotfile.
  Use a short-lived capability grant rather than long-lived `power_user` membership, and a
  **per-request revocation list** (not env-at-boot) so a stolen key is cut without a redeploy.
- **Release-build CI gate:** CI proves the LAN-facing VisionClaw binary is a **release** build
  (the `dev-session-token → power_user` path is debug/dev-auth only; a debug build on the LAN
  reopens the fence).
- **Phase-2 nostr (WS-I): NIP-59 gift-wrap** the scope/question to the provider key (a
  content-addressed URN hides nothing; plaintext scope leaks to the relay). Same for the
  kind-30840 digest. Retire NIP-26 delegation → per-consumer NIP-59 capabilities.

---

## 9. Correctness posture — the reconciliation janitor & deadline reaper

**Content-address identity is CORE** (RFC 8785 JCS over the identity core; execution fields
excluded — §5.2). **CAS bead close + lease_epoch fencing** make double-delivery and stale
claims no-ops (§5.6). Two harness-side sweepers guarantee liveness of the ledger:

**Reconciliation janitor** — one idempotent sweep, period `P < min(lease_TTL, memory_TTL)/2`:

1. `claimed` beads past `lease_TTL` → **increment epoch**, clear actor → re-eligible;
2. **OPEN** distill beads whose `jobUrn` key exists in RuVector → **complete the tail** from
   the stored **signed** envelope (Phase-2: ensure `:summary` write; CAS-close `done`) — the
   envelope is self-contained + signed, so the sweep is *courier-of-record*;
3. open beads past consumer deadline with **no** payload → **CAS-close `expired`**
   (outcome-aware recombine handles it);
4. divergence audit: RuVector entries missing from `:summary` are re-posted before TTL.

*"closed-done-with-no-payload" is structurally impossible once close-last + CAS holds.*

**Deadline reaper** — at a job's deadline it **CAS-closes the distill bead `expired`**
(`cause: unclaimed | claimed-not-delivered`), unblocking recombine to proceed **search-only**.
This is precisely what makes **"HP absence never blocks a turn" TRUE**.

**No-synchronous-await LAW** (restated as correctness, not just style): submit-in-turn-N,
recombine-in-a-later-turn/worker; fast-wake only *schedules* the recombine worker.

---

## 10. Success criteria & verifiability (ADR-119 liveness — heartbeat, not boot canary)

Liveness is proven by observation, not by "wired". Boot completion is **NOT** liveness.

| # | Criterion | Verification |
|---|---|---|
| 1 | **Loop closes end-to-end** | An agent `submit`s, keeps working, and a later turn `fetch`es a **signed, sha-pinned, budget-clamped** distillate; provenance reconstructs from `sig` + build-manifest sha |
| 2 | **jobd heartbeat** | `jobd` heartbeats **every poll** (short-TTL RuVector key or management-api provider-status); WS-G liveness = **heartbeat-staleness threshold** (kills the green-but-zero / '.48-is-dead' class) |
| 3 | **Periodic canary lands** | Submitted at `T`, not landed by `T+deadline` → **alert**. Canary is a **cap-exempt lane**, `canary:true` → lands in `ontology-canary` ns, never writes `:summary`, never digests, excluded from elevation + queue-depth metrics |
| 4 | **Fail-labelled honesty** | Induced `scaffold_engaged=false` is quarantined, never returned as grounded; induced bad signature → 400 (write) / quarantine (read) |
| 5 | **Reaper unblocks** | With HP offline, a submitted job's deadline reaper CAS-closes `expired`; recombine proceeds **search-only** in a bounded turn |
| 6 | **Corpus identity threads** | `build-manifest.json` present, written last; both TTL headers carry `owl:versionIRI` + `prov:wasDerivedFrom`; `mirror.sh` verifies per-artifact sha256 before publishing |
| 7 | **One authority, conformance-tested** | Published `ontology-inferred.ttl` and VisionClaw `:inferred` derive from the same generation; a conformance test asserts they agree |
| 8 | **Zero regression on governed write** | `propose → Whelk → PR → merge` unchanged |
| 9 | **`jobd.service` durable** | `Restart=on-failure`; `systemctl enable --now jobd` is an explicit operator runbook step with a verify command |

---

## 11. DDD & cross-references

**BC24 — OntologyConnector** (`docs/ddd/ddd-ontology-connector-context.md`), **honestly
scoped**: a *deferred-distillation subdomain extending BC21*, not a sprawling new context.

- **Aggregates:** `DistillationJob` (job-URN-anchored lifecycle), `ResultEnvelope` (signed,
  sha-pinned), `CorpusGeneration` (build-manifest identity). `ConnectorDescriptor`
  (capability-discovery) has **no Phase-1 consumer** → deferred/noted.
- **Neighbours (all four):** **BC21** read/augment (Customer/Supplier — BC24 hands distillates
  to BC21's fenced L3 write and does **not** own the fence); **BC22** SemanticTrustLayer
  (Published-Language — consumes the ProvenanceEmitter + the reconciliation mapping; **resolve
  the BC22 numbering collision** between `ddd-xr-godot` and `ddd-semantic-trust-layer` in the
  BC catalogue); **BC23** semantic-integrity/provenance (grammar); **BC20** write door
  (elevation only).
- The DDD context **must show the ownership migration explicitly** (Loom becomes the single
  owner; in-process pieces become thin clients).

**Companion decisions:** VisionClaw **ADR-135** records the Loom façade contract
(deployment-agnostic; `DISTILL_BACKEND_URL` seam), the canonical-reasoner choice, and the
deferred-distillation lifecycle laws. **agentbox ADR-051** (proposed; next-free — ADR-048–050
landed 2026-08, re-check at merge) records the harness-side loom-client decisions
(deterministic job-mint discipline, the RuVector adapter, the janitor/reaper). **agentbox
ADR-050** (decision-elevation-inverse-corpus-path) is cited in WS-H as a **pattern precedent
only**.

---

## 12. Open decisions (explicit — for the operator)

| # | Decision | Options | Owner | Default if undecided |
|---|---|---|---|---|
| **OD-1** | **Canonical reasoner engine** | (a) promote Whelk EL++ into the Loom; (b) make logseq `reason.py` transitive closure authoritative and have VisionClaw load its output | Operator + ADR-135 | Invariant holds either way (one authority, conformance-tested); **default (a)** if VisionClaw's EL++ closure is richer than the pipeline's transitive closure |
| **OD-2** | **Aggressive full consolidation confirmed?** | Retire *all* duplicate impls to thin clients now (direct-to-target), vs. keep any copy as a documented fallback | Operator | **Confirmed aggressive** (dev/test estate; the whole point of direct-to-target is to stop paying for drift) |
| **OD-3** | **Reference reasoning-host topology at merge** | Topology A (HP GPU-local) as the only reference, vs. also standing up a Topology-B sidecar on `visionclaw_network` in the same sprint | Operator | Topology A reference; B is a config-line follow-on |
| **OD-4** | **Recombine-worker owner** | claude-flow daemon consumer polling `getReady` (Phase-2), vs. leave tool-side `await`/`fetch` only | Operator | Tool-side only for loop-close; name the daemon owner before WS-H |
| **OD-5** | **RuVector TTL vs. non-expiring for Phase-1** | Apply the TTL law immediately, vs. non-expiring memory type until Phase-2 durable copy exists | Operator | Non-expiring for loop-close; switch to the TTL law when `:summary` durable copy (WS-F) lands |

---

## 13. Risk matrix

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Direct-to-target cutover breaks live VisionClaw viz during debug | Medium | Medium | Dev/test estate — no production SLA; generation feed is atomic (load-generation-N) so a bad generation rolls back to the last good one |
| gguf swapped under the same alias mid-benchmark window | Low | Medium | Per-call model re-probe + file metadata record; residual risk noted, not eliminated |
| RuVector `ontology-distilled` namespace low-recall (per project RuVector index-law) | Medium | Low | Fetch is **keyed** (`memory_retrieve(key=jobUrn)`), not semantic search — recall band does not apply to keyed reads |
| Elevation (WS-H) replay non-determinism weakens the verification story | Medium | Low | Verification rests on `sig` + sha-pins; deterministic replay is best-effort same-binary/same-weights only, and is a **hard gate** on the elevation path or the claim is dropped |
| Two provenance graphs diverge under the mapping | Medium | Medium | Reconciliation **mapping** (alignment vocabulary), each graph keeps its owner's invariants; no merge attempted |
| Provider-door shared-secret replay | Was High | High | strict-nip98 (drop Bearer) + per-request revocation + release-build CI gate |

---

## 14. Relationship to the capstone thesis

This PRD is the VisionFlow capstone: it **closes the loop** the platform was designed for —
agentic intelligence + search + reasoning + local data + ontology traversal into
semi-structured markdown, *recombined to enhance an LLM response* — by naming and building the
one missing owner, the **Loom**. It demonstrates the **self-sovereign data + provenance**
pillar end-to-end: every distilled answer is attributable to a **signed identity**, a **pinned
corpus generation**, and a **probed model identity** — verifiable from the signed envelope +
build-manifest sha, without trusting any intermediary. One corpus, one Loom, one contract,
three latency classes, one provenance grammar.
