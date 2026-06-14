# ADR-121 — Self-improving ontology via governed writeback (the elevation flywheel)

**Status:** Proposed
**Date:** 2026-06-14
**Decision-type:** Architecture + Agent-Orchestration + Security (write boundary)
**Parent:** ADR-112 (retrieval spine), PRD-020 (WS-9/10/11). **Relates:** ADR-113 (condensation mesh — the re-condense step), ADR-114 (RuVector index — the re-index step), ADR-119 (verifiable liveness — anti-ghost), ADR-120 (did:nostr agent identity — proposal attribution), ADR-041 / ADR-110 (Judgment Broker / ACSP — the human approval surface this reuses), `agentbox/management-api/lib/kg-proposal-extractor.js` + `agentbox/mcp/servers/ontology-propose.js` (the governed propose spine reused), PRD-018 (the silent-dead-wiring lesson), `docs/prd-insight-migration-loop.md` (sibling loop).

> The operator asked to "land the full ideal" of self-improvement through writeback and to "make the hard choices despite the complexity." This ADR does exactly that: it commits to the ambitious, loop-closing design **and** draws the non-negotiable governance line that keeps it safe. It also **kills a real ghost** — the `writeback_triggered` no-op in `enrichment_proposals_handler.rs` — replacing a flag that lies with a writeback that works.

---

## 1. Context — why close the loop

PRD-020/ADR-112 make the ontology *readable* by every agentbox AI call. That is a one-way street: the ontology informs agents, agents never improve the ontology. The richest property of the system is the **flywheel**: agent usage reveals what the ontology lacks (queries with no good seed, weak definitions, missing relations, emergent concepts), and that signal, fed back, makes the ontology better, which makes every future augmentation better, which generates richer usage. Closing this loop is what turns a retrieval layer into a *living* knowledge system.

The temptation is to let agents write the ontology directly. That is the trap. The whole architecture rests on **read-pervasive / write-governed** (ADR-112 §2.4): asserted truth is gated by Whelk EL consistency + human merge. Self-improvement must not erode that. The hard problem is therefore: *maximise autonomy of the loop while keeping a human/Whelk gate on asserted truth.*

**Verified ground truth (2026-06-14):**
- The governed propose spine exists and works: `kg-proposal-extractor.js` → `buildProposeRequest` → `POST /api/ontology-agent/propose` → `OntologyMutationService` → Whelk → PR / ACSP panel. **Reuse it.**
- The named-graph write pattern exists: `store_inference_results` writes `urn:ngm:graph:ontology:inferred` (`ontology_handler.rs:644`, route `/ontology/inference`). The graph constants `:assert`/`:inferred` are defined (`triple_emitter.rs:51-52`). **No `:summary`/`:usage` graphs exist** — net-new.
- **The ghost:** `enrichment_proposals_handler.rs` exposes `writeback_triggered: bool` backed by an in-memory `WRITEBACK_DECISIONS: Mutex<Vec>` (`:167`). Approval flips the flag but performs **no actual ontology write**; the doc-comment admits the durable store "not reachable on main yet." It is dead wiring — wired, flagged, inert.

---

## 2. Decision — a three-tier writeback loop with one hard line

### The hard line (non-negotiable)
**Asserted ontology axioms (`urn:ngm:graph:ontology:assert`) are NEVER written automatically.** Every change to asserted truth passes Whelk EL consistency **and** a human merge gate. The loop's autonomy lives in three places only — *generating evidence-backed proposals*, *materialising the augmentation layer's own derived output*, and *refreshing after a human merges* — never in *committing truth*.

### Tier W0 — Derived materialisation (direct write, namespace-fenced)

Write the augmentation layer's own derived output back into **derived named graphs the layer owns**:
- `urn:ngm:graph:ontology:summary` — the condensed Class Summaries (ADR-113 output) as annotation triples (`vc:classSummary`, `vc:summaryModel`, `prov:wasGeneratedBy` run IRI). Makes summaries SPARQL-queryable, version-controlled with the ontology, and re-embeddable by either system. **Supersedes ADR-114's "deferred" note — now adopted.**
- `urn:ngm:graph:ontology:usage` — usage telemetry as triples (`vc:accessCount`, `vc:lastUsefulAt`, `vc:relevanceEwma`, `vc:agentConfirmedUseful`), keyed by class IRI. Feeds maturity/quality scoring and retrieval ranking.

**Hard choice — a separate, fenced write surface.** A NEW endpoint `POST /api/ontology/derived` accepts writes **only** to `:summary`/`:usage` and is structurally incapable of touching `:assert`/`:inferred` (server-side: reject any quad whose graph ∉ {summary, usage} with 400). It is distinct from both the propose path and the `/load` backdoor. Derived graphs are **disposable**: a `POST /api/ontology/derived/regenerate` rebuilds them from scratch, so a corrupt writeback is never load-bearing.

### Tier W1 — Governed knowledge enrichment (reuse the spine)

Mine usage into **enrichment candidates** and route them through the *existing governed path*, unchanged:

`usage telemetry → enrichment-candidate extractor → buildProposeRequest → POST /api/ontology-agent/propose → Whelk EL consistency → GitHub PR / ACSP forum panel → human merge`

The extractor is a **generalisation of `kg-proposal-extractor.js`** to accept usage signals as a candidate source (alongside the existing personal-KG source):
- **No-seed queries** (a query whose top HNSW seed scores below the relevance floor repeatedly) → candidate *new class*.
- **Low-relevance hits** (a class repeatedly seeded but with weak definition match) → candidate *definition refinement*.
- **Manual bridges** (agents repeatedly traverse a k-hop dead-end the same way) → candidate *new relation*.
- **Emergent clusters** (a dense cluster of queries with no owning class) → candidate *new class + relations*.
- **Maturity signal** (a `draft` class with high confirmed-useful usage) → candidate *maturity upgrade*.

No new write authority is created: W1 produces *proposals*, the same artifact a human or personal-KG scan already produces.

### Tier W2 — Autonomous loop closure

Automate the two ends so the loop runs without a human in the *plumbing* (the human stays in the *decision*):
1. **Autonomous proposal generation + submission**, bounded (see §3): the extractor runs on a schedule/threshold and submits qualifying candidates to the governed queue, tagged machine-originated with a verified did:nostr (ADR-120).
2. **Autonomous post-merge refresh:** on PR merge / ACSP approval → re-ingest (GitHubSync) → re-condense changed classes (ADR-113 mesh) → re-index into RuVector (ADR-114) → update derived graphs (W0). The loop closes: usage → proposal → human/Whelk → merged truth → re-augmentation → richer usage.

**This is where the `writeback_triggered` ghost dies.** It is replaced by: (a) a durable `EnrichmentProposal` store (the aggregate the handler's doc-comment anticipates), and (b) on approval, an *actual* writeback — for W1 that means the PR-merged axioms land in `:assert` via the normal ingest path (not a flag); for W0/usage it means a real `/api/ontology/derived` write. A flag that lies is removed; a write that happens replaces it.

---

## 3. Bounded autonomy (the hard choices, made)

| # | Hard choice | Decision |
|---|---|---|
| HC1 | Can agents write asserted truth? | **No.** Whelk + human merge always gate `:assert`. Autonomy = proposal generation + derived materialisation + post-merge refresh only. |
| HC2 | One write surface or several? | **Three, sharply separated:** (i) `/api/ontology/derived` (direct, fenced to `:summary`/`:usage`), (ii) governed `/api/ontology-agent/propose` (Whelk→PR, for `:assert`), (iii) the existing `/inference` (Whelk-derived `:inferred`). The derived endpoint **cannot** write `:assert`; the propose endpoint **cannot** bypass Whelk. Enforced server-side. The ungoverned `/load` backdoor stays closed (ADR-118). |
| HC3 | What about `writeback_triggered`? | **Deleted.** Replaced by a durable `EnrichmentProposal` store + real writeback execution. No inert flag survives. |
| HC4 | How is a degrading loop contained? | **Reversibility + kill switch + outcome-gating.** Derived graphs fully regenerable (`/derived/regenerate`); the loop is gated by `agentbox.toml [sovereign_mesh].ontology_self_improvement` (default **off**); and the loop **auto-disables** if its Guidance-Control-Plane outcome metric (task success / cost-per-outcome with the loop on vs off) does not improve over a measurement window. A loop that doesn't help is switched off, not left running. |
| HC5 | How is the human queue protected from flooding? | **Confidence-gated + dedup'd + rate-limited + attributed.** Auto-proposals must be Whelk-consistent (hard), score ≥ a confidence threshold, dedup against existing classes (semantic + lexical), respect a per-window submission cap, and carry a verified machine did:nostr (ADR-120) so the broker can trust-tier and batch-review them. |
| HC6 | Is everything provenance-stamped and verifiable? | **Yes (anti-ghost, ADR-119).** Every writeback (W0/W1/W2) emits PROV-O (`prov:wasGeneratedBy` a loop-run IRI, `vc:derivation`, agent identity). A per-stage liveness counter proves each stage *fires and lands* (proposals_submitted, proposals_merged, classes_recondensed, derived_triples_written) — never a flag that merely claims success. |

---

## 4. Consequences

**Positive**
- Closes the flywheel: the ontology improves itself through usage, at bounded human cost (humans approve, don't author).
- Maximum reuse: the governed propose spine, the broker/ACSP approval surface, the condensation mesh, and the RuVector index are all reused; W1 is "feed usage into the extractor that already exists."
- **Removes a real ghost** (`writeback_triggered`) and gives the half-built enrichment/broker machinery a concrete reason to be completed rather than deleted.
- Derived materialisation (W0) makes the augmentation layer's output first-class and co-located with the source — better recall, SPARQL-queryable summaries, version-controlled.
- Governance integrity is *strengthened*, not weakened: three fenced write surfaces with server-side enforcement is clearer than today's ambiguous duplicate `/ontology` scope.

**Negative / costs (accepted, because the operator chose the full ideal)**
- Real new surface: the fenced `/derived` endpoint, the usage telemetry store + extractor generalisation, the durable `EnrichmentProposal` aggregate (the broker work ADR-041 anticipated), and the post-merge refresh orchestration. This is the largest single workstream in the family (PRD-020 WS-9/10/11).
- A self-modifying loop is the highest-risk component; contained by HC4 (reversibility + outcome-gating + default-off) and HC6 (provenance + per-stage liveness).
- Feedback risk (the loop reinforces its own biases — proposing classes that match what agents already ask, ossifying blind spots). Mitigation: the extractor weights *novelty* (no-seed/emergent clusters) above *reinforcement*; humans see the machine-origin tag and can down-weight.
- Latency between proposal and benefit is human-merge-bound (hours-to-days). Accepted: truth should not be fast.

**Neutral**
- Default-off; the loop is *installed* before it is *delivered* (activation milestone = PRD-020 WS-11 outcome sign-off), consistent with ADR-119.

---

## 5. Alternatives considered

1. **Agents write `:assert` directly (no Whelk/human gate).** Rejected outright: destroys the governance invariant, invites inconsistency and poisoning, repeats the ungoverned-`/load` mistake at scale.
2. **No writeback — read-only forever.** Rejected: this is what the operator explicitly asked to move beyond; leaves the flywheel unbuilt and the ontology static.
3. **Derived materialisation only (W0), no enrichment loop (W1/W2).** Rejected as the *whole* answer but adopted as the *first* tier: W0 improves recall but does not improve the *ontology's knowledge*. The operator asked for the full ideal.
4. **A brand-new enrichment/broker pipeline.** Rejected: ADR-041/110 + `kg-proposal-extractor.js` + `ontology-propose.js` already exist; building parallel machinery would itself create dead wiring. Complete and feed the existing spine.
5. **Keep `writeback_triggered` and bolt real writeback beside it.** Rejected: that is how ghosts breed. Delete the lying flag; replace with the durable aggregate + real write.

---

## 6. Verification (anti-PRD-018, anti-ghost)

Declared implemented only when:
1. `/api/ontology/derived` accepts a `:summary` write and **rejects** an `:assert` write (server-side fence test).
2. A usage signal produces a candidate that traverses extractor → propose → Whelk → PR → human-merge → re-ingest → re-condense → re-index, with PROV-O at each hop (end-to-end loop test against a running relay).
3. `writeback_triggered` and `WRITEBACK_DECISIONS` are **removed** from `enrichment_proposals_handler.rs`; a durable `EnrichmentProposal` store backs the decision path; grep proves the flag is gone.
4. Per-stage liveness counters are non-zero and the loop auto-disables when the outcome metric does not improve (kill-switch test).
5. `/api/ontology/derived/regenerate` rebuilds derived graphs from scratch (reversibility test).
6. Auto-proposals are confidence-gated, dedup'd, rate-limited, and carry a verified machine did:nostr (bounded-autonomy test).

Until then the loop is reported as **installed, not delivered**.
