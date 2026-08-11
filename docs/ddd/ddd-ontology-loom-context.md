# DDD: Ontology Loom Bounded Context

**Context name:** `OntologyLoom` (BC24 — provisional, pending BC catalogue update; see §9)
**Date:** 2026-08-11
**Author:** VisionClaw platform team (OCP capstone mesh — operator reframe 2026-08-11)
**Related:** VisionClaw PRD-025 (Ontology Loom — parent, next-free at draft), VisionClaw ADR-135 (Loom façade contract + `DISTILL_BACKEND_URL` pluggable seam), `docs/ddd-ontology-augmentation-context.md` (BC21 — read/augment + L3 writeback, Loom's downstream customer), `docs/ddd-semantic-trust-layer-context.md` (BC22 — trust trinity, publishes ProvenanceEmitter + `:provenance` reconciliation), `docs/ddd-agentbox-integration-context.md` (BC20 — governed write door, elevation only), `docs/ddd-bead-provenance-context.md` (BC-BP — bead lifecycle Loom leases), `docs/ddd-mesh-federation-context.md` (BC-MF — relay transport for Phase-2 job events). Cross-repo: agentbox ADR-112 (one-brain retrieval spine / no hot-path LLM), agentbox ADR-113 §2.2 (condensation mesh; distillation concurrency=1), agentbox ADR-116 (tier budgets), agentbox ADR-013 §6 (URN extension API), agentbox ADR-075 D1 (IS-Envelope), agentbox ADR-090 (ring order), agentbox ADR-049 (`urn:agentbox:graph:provenance`), agentbox ADR-050 (decision-elevation — *pattern precedent only*), agentbox PRD-022 (semantic-integrity-provenance-decisions), agentbox ADR-051 (Loom job/result kinds — re-check next-free at merge), VisionClaw PRD-022 (semantic-trust-layer) constraint 3 (`urn:ngm:graph:provenance`), VisionClaw ADR-050 (pod-backed-kgnode), VisionClaw ADR-121 WS-10 (propose spine), logseq `pipeline/` (build/reason/conflicts).

## 1. Purpose

Define the bounded context that **owns the canonical ontology corpus lifecycle end-to-end and serves ontology intelligence behind one stable, model-swappable façade.** The Loom is a first-class VisionFlow node — a peer of VisionClaw, agentbox, and the published site, not a peripheral "connector." It weaves the corpus (semi-structured Logseq markdown + JSON-LD blocks) into a reasoned ontology, holds that as the single source of truth, and exposes it at three latency classes (static-cloud replica, live-lan façade, slow-llm distillation) through a uniform JSON contract.

The Loom exists to end two structural problems the operator observed (OCP Revised Design §0, §0b):

1. **A real-time render/physics engine was moonlighting as a corpus-lifecycle manager.** VisionClaw's `github_sync_service` performed a `force_full` CLEAR+INSERT of the whole `:assert` graph on every sync — wiping runtime decision-class triples, and needing agentbox ADR-050 (decision-elevation) inverse-corpus-path just to survive its own reload. The Loom takes ownership of sync/generation/reason/publish/write-back so VisionClaw can be a pure consumer.
2. **Ontology intelligence was scattered across four codebases with duplication that caused real drift** (the 8152-vs-5975 class-count divergence; the stale `:assert` graph). Two reasoners, three retrieval implementations, two parsers, two conflict checkers. The Loom becomes the **single owner of each capability**; the fast in-process pieces become thin clients of its authoritative generation (see §8, the ownership migration).

The critical constraint, inherited verbatim and never relaxed: **agentbox ADR-112 — no LLM and no network call on the hot retrieval path.** The Loom owns the *slow/authoritative* path (lifecycle, reasoning, index generation, distillation); the in-process libraries keep their *fast local* paths but resolve authoritative state **from** the Loom instead of re-deriving it. Consolidation preserves the one-brain hot path; it does not put the Loom in it.

This context is **not** a sprawling new domain. It is honestly scoped as the **corpus-lifecycle + deferred-distillation owner**. Retrieval algorithms, budget economics, and the L2/L3 writeback fences remain BC21's; the governed L1 write remains BC20's; the shape/provenance/federation trinity remains BC22's. The Loom supplies the *authoritative artifact* those contexts consume, and adds exactly one genuinely new machine — the **deferred distillation job** — behind the façade.

## 2. Ubiquitous language

| Term | Meaning in this context |
|---|---|
| **Loom** | The bounded context and the node role: portable module, stable façade contract, pluggable deployment. "The Loom façade" = the endpoint; "a Loom sidecar" = a containerised deployment; "a Loom generation" = a published corpus snapshot. |
| **Loom façade** | The one deployment-agnostic contract consumers call. Model identity and corpus generation are carried in *results*, never in the endpoint — so the backend model swaps (Gemma → Muse-Glimmer → next) and even the host swaps with zero consumer change (ADR-135). |
| **Distillation backend** | The LLM behind the façade, addressed by `DISTILL_BACKEND_URL` (HP llama.cpp `:8084`, a cloud model, or a local container). A config line, not an architecture change. The only GPU-bound facet. |
| **Corpus Generation** | An atomic, content-addressed snapshot of the corpus at one GitHub commit: `{commitSha, buildId, generatedAt, pipelineVersion, artifacts:{path:{sha256,bytes,count}}}`. Identified by `commitSha` (`GITHUB_SHA`) and pinned by per-artifact `sha256`. The unit VisionClaw loads (replacing CLEAR+INSERT) and the unit distillation pins against. |
| **Build Manifest** | `api/build-manifest.json`, written **last** by the pipeline; the root record that makes a Generation atomically verifiable. No envelope rule is expressible without it (WS-A). |
| **Ontology Authority** | The single owner of parse → reason → conflict-gate → index, per Generation. The authoritative EL closure; the canonical parser; the authoritative index set. Downstream reasoners/indexers **derive from**, never re-derive, its output. |
| **Conformance Test** | The gate that proves a thin client's derived state equals the Authority's for a Generation (e.g. VisionClaw `:inferred` ≡ published `ontology-inferred.ttl`). The mechanism that makes "no drift" structural rather than hoped-for. |
| **Distillation Job** | A job-URN-anchored request for an LLM-distilled, ontology-grounded summary over a corpus scope. The aggregate root of the deferred channel. |
| **Job URN** | `urn:agentbox:job:<pubkey>:<sha256-12>`, minted via the agentbox ADR-013 §6 URN extension API (`contentAddressed:true`, `ownerScope:true`), content-addressed over the **identity core** only. The idempotency + provenance anchor. Distinct from the work-ledger **bead**. |
| **Identity Core** | The hashed subset of a job: `{kind:"ontology.distill", corpusSha, scope, budget_tokens}`, canonicalised with **RFC 8785 JCS**. `budget_tokens` **is** content (it changes the answer). Execution fields (deadline, requester, sig, rendezvous, model policy) are **not** hashed. |
| **Result Envelope** | The signed (BIP-340), sha-pinned payload a provider delivers: summary + full provenance metadata. The unit both delivery sinks store and both verify. |
| **Distiller-Provider Allowlist** | The set of `did:nostr` provider identities permitted to write `:summary` / land in `ontology-distilled`. **Not** generic `power_user`. The trust boundary at both the derived write door and the RuVector read. |
| **Scaffold Engaged** | Boolean in the envelope: `true` iff ontology retrieval seeded the distillation. `scaffold_engaged=false` output is **fail-labelled** (quarantined), never delivered as ontology-grounded. |
| **corpusNature** | `"synthetic-ai-generated-human-directed"` — carried on every distillate for corpus honesty. The corpus is AI-authored under human direction; distillates never imply otherwise. |
| **Lease Epoch** | A monotonic counter incremented on every claim/reclaim of a distill bead, carried in the envelope. Both delivery sinks reject stale-epoch deliveries (fencing). |
| **Reconciliation Mapping** | The alignment vocabulary that reconciles the two provenance graphs (VisionClaw PRD-022 `urn:ngm:graph:provenance` vs agentbox ADR-049 `urn:agentbox:graph:provenance`) without declaring either canonical. |

## 3. Strategic placement

> **Status:** aspirational design — not implemented as of 2026-08-11. No `CorpusGeneration`, `OntologyAuthority`, `DistillationJob`, or `ResultEnvelope` types exist yet. Precursors, scattered: logseq `pipeline/build.py` + `pipeline/reason.py` + `pipeline/conflicts.py` (lifecycle fragments, no build-manifest); VisionClaw `github_sync_service` (the CLEAR+INSERT to be **retired**); HP `ontology_scaffold.py` + `jobd` (unbuilt); agentbox `@agentbox/ontology-retrieval` (ADR-112, the one-brain that becomes a Loom client). This is a **direct-to-target** build on a dev/test estate: no live-migration shims — the workstreams below are a build order toward one end-state, not phased protection of a running system (OCP §0 direct-to-target).

```mermaid
graph TD
    GH[("GitHub corpus repo<br/>Logseq md + JSON-LD")]

    subgraph LOOM["OntologyLoom (BC24 — this context; portable node)"]
        direction TB
        subgraph LIFE["Lifecycle + façade facet (stdlib-portable, always-on-ish)"]
            CG["CorpusGeneration (root)<br/>build-manifest + sha-pins"]
            OA["OntologyAuthority (root)<br/>parse · EL closure · conflict-gate · index"]
            FACADE["Loom façade<br/>retrieval / scaffold / distill-submit / model-swap-behind"]
        end
        subgraph DIST["Distillation facet (GPU; swappable model)"]
            DJ["DistillationJob (root)<br/>job-URN lifecycle"]
            RE["ResultEnvelope (root)<br/>signed · sha-pinned"]
            JOBD["HP jobd (stdlib pull-worker)<br/>DISTILL_BACKEND_URL"]
        end
        JANITOR["Reconciliation janitor + deadline reaper"]
    end

    subgraph CLOUD["Published site (cloud read replica)"]
        MIRROR["mirror.sh — atomic generation publish"]
    end

    subgraph VC["VisionClaw (Rust)"]
        LOAD["load-generation N (atomic; replaces CLEAR+INSERT)"]
        INFER[("urn:ngm:graph:ontology:inferred<br/>DERIVES FROM Authority")]
        DERIVED["POST /api/ontology/derived<br/>(:summary fence — BC21 L3)"]
        PE["ProvenanceEmitter → urn:ngm:graph:provenance (BC22)"]
        PROP["propose door → Whelk → PR (BC20 write)"]
    end

    subgraph BOX["agentbox"]
        ONEBRAIN["@agentbox/ontology-retrieval (ADR-112)<br/>one-brain — NOW a Loom index client"]
        MCP["ontology-bridge MCP<br/>distill_submit / fetch / await (WS-J)"]
    end

    RV[("RuVector · ns ontology-distilled<br/>MCP-only")]

    GH -->|sync + parse| OA
    OA --> CG
    CG -->|publish generation| MIRROR
    CG -->|atomic feed| LOAD
    OA -.authoritative closure.-> INFER
    OA -.index generation.-> ONEBRAIN
    OA -.index generation.-> MCP
    MCP -->|ontology_distill_submit| FACADE
    FACADE --> DJ
    JOBD -->|claim / distill / deliver| DJ
    DJ --> RE
    RE -->|(a) memory_store jobUrn| RV
    RE -->|(c) CAS bead close| JANITOR
    RE -.Phase 2 :summary.-> DERIVED
    RE -.Phase 2 PROV-O.-> PE
    RE -.Phase 2 elevation.-> PROP
    JANITOR -.reap/reconcile.-> DJ
```

## 4. Strategic patterns

### 4.1 Context relationships

The Loom sits at the **centre** of the ontology mesh — it is the supplier the other four contexts consume — but it deliberately owns **none** of their write fences.

- **OntologyLoom → OntologyAugmentation (BC21):** **Customer / Supplier**, two-directional but fenced.
  - *Downstream (Loom supplies BC21):* BC21's `ClassSummaryIndex` and the ADR-112 one-brain stop being independent index builders; they become **thin clients of the Loom's index generation** (scaffold-index / prose-index / RuVector-condense are derived **once**, by the Loom, per Generation). This preserves ADR-112: the in-process library keeps the fast local hot path, it just resolves the authoritative index from the Loom rather than re-condensing.
  - *Upstream (Loom hands to BC21):* a significant distillate becomes an `EnrichmentCandidate` (BC21 §12.3). The Loom **hands distillates to BC21's fenced L3 `:summary`/`:usage` writeback; it does NOT own that fence.** The `/api/ontology/derived` graph fence (BC21 invariant I10) is BC21's; the Loom is a provider that must pass it (signed, allowlisted — §7).
- **OntologyLoom → SemanticTrustLayer (BC22):** **Published Language.** The Loom **consumes** BC22's `ProvenanceEmitter` — a distillate's PROV-O triples route **through** the PRD-022 `ProvenanceEmitter` into `urn:ngm:graph:provenance`, **not** into `:summary` (PROV-O in `:summary` would violate VisionClaw PRD-022 constraint 3). The Loom also consumes BC22's **reconciliation mapping** to align its own agentbox-side provenance (`urn:agentbox:graph:provenance`, agentbox ADR-049) with the VisionClaw graph without declaring either canonical. The Loom never writes shapes or the provenance graph directly; it emits *through* BC22's aggregate.
- **OntologyLoom → SemanticIntegrity & Provenance (BC23, agentbox — grammar):** **Conformist.** The Loom mints **no** identifier grammar of its own. Job URNs, bead URNs, `did:nostr`, and the `sha256-12` content-address discipline are all conformed to the agentbox URI/DID grammar (agentbox PRD-022 semantic-integrity; `uris.js` / ADR-013 §6). The new `job` URN kind is added **via** the ADR-013 §6 extension API (KINDS entry + resolver case + contract test), not hand-minted. The grammar is closed; the Loom takes a seat, it does not widen it. (Distinguish from VisionClaw PRD-022 semantic-trust-layer, above — two different PRD-022s.)
- **OntologyLoom → AgentboxIntegration (BC20 — governed write door):** **Customer / Supplier on the elevation axis only.** For Phase-2 durability, a significant distillate flows `EnrichmentCandidate → ElevationActor + KnowledgeEnrichment broker case (ACSP 31402/31403)` composed with the VisionClaw ADR-121 WS-10 propose spine → corpus → CI conflict gate → published site. The Loom **produces candidates**; it owns **no** asserted write. (agentbox ADR-050 decision-elevation is cited as *pattern precedent only* — it is DECISION elevation, not this mechanism.)
- **OntologyLoom → BeadProvenance (BC-BP):** **Shared Kernel on the bead lifecycle.** The distill bead and recombine bead are ordinary BC-BP beads (minted normally, nonce-carrying). The Loom adds `lease_epoch`, a typed `result_ref {namespace,key,content_sha}`, and CAS-close semantics (WS-C) as a **schema migration + contract tests**, not a fork of the ledger.
- **OntologyLoom → MeshFederation (BC-MF):** **Partner (Phase 2).** Phase-2 cloud-consumer job events ride the relay as **new** ACSP kinds **31408 DistillJobRequest / 31409 DistillJobResult** (never reusing 31406/31407 SPARQL `semantic_query`), NIP-59 gift-wrapped to the provider key. The Loom is a new consumer of the mesh, not a new relay.

### 4.2 Invariants (the structural spine)

- **I1 — One authority, no drift.** For any Generation, there is exactly one Ontology Authority output (one EL closure, one parse, one index set, one conflict verdict). Every downstream reasoner/parser/indexer **derives from** it and is **conformance-tested** to equal it. VisionClaw `:inferred` ≡ published `ontology-inferred.ttl` ≡ Authority closure, for the same `commitSha`. The 8152-vs-5975 drift is structurally impossible once conformance is a gate.
- **I2 — Closed-done ⇒ payload retrievable.** A distill bead closed with outcome `done` **guarantees** its Result Envelope is retrievable at its `jobUrn`. Enforced by **close-last ordering** (the CAS bead close is the linearisation point; the RuVector payload write strictly precedes it) plus the reconciliation janitor. "Closed-done-with-no-payload" cannot occur.
- **I3 — `scaffold_engaged=false` is never grounded.** An envelope with `scaffold_engaged=false` is **fail-labelled and quarantined** — never delivered, stored, elevated, or presented as ontology-grounded. Fail-labelled, not fail-open.
- **I4 — The write path is never widened.** The Loom adds **no** new write authority. It writes `:summary`/`:usage` only through BC21's existing fenced `/api/ontology/derived` (as an allowlisted, signed provider); it reaches `:assert` only through BC20's governed propose spine (as a candidate). Generic `power_user` never suffices to land a distillate. The corpus GitHub write is the Loom's own lifecycle (it owns sync), and it is the **only** widening — and it replaces, not adds to, the retired VisionClaw CLEAR+INSERT.
- **I5 — Content-address separates identity from execution.** The Job URN is content-addressed over the Identity Core only (RFC 8785 JCS over `{kind, corpusSha, scope, budget_tokens}`). Two identical requests resolve to the same URN (dedupe-on-create); execution differences (deadline, requester, model policy) never fork identity.
- **I6 — Grammar-closed.** Every identifier the Loom emits conforms to the agentbox URI/DID grammar (BC23): job/bead URNs via `uris.js`/ADR-013 §6, `did:nostr` wrapped, `sha256-12` truncation. No ad-hoc IDs.
- **I7 — No LLM on the hot path (ADR-112).** The Loom's distillation and reasoning are the slow/authoritative path only. No consumer's hot retrieval path acquires an LLM or network call because the Loom exists. The **no-synchronous-await LAW** holds: distillation is submit-in-turn-N, recombine-in-a-later-turn; no consumer ever holds a turn open on a distill job.
- **I8 — Corpus honesty.** Every distillate carries `corpusNature:"synthetic-ai-generated-human-directed"`. The corpus is AI-generated under human direction and is never implied to be otherwise.
- **I9 — Atomic generations.** A Generation is published atomically (`mirror.sh` writes `data.new/`, verifies every artifact `sha256` against the build-manifest fetched first, then renames). Consumers never observe a mixed-build window. VisionClaw loads a whole Generation atomically; it never CLEAR+INSERTs.
- **I10 — SPARQL SERVICE is forbidden; RuVector is MCP-only.** No Loom path emits a federated `SPARQL SERVICE` clause (agentbox ADR-011 lineage). All RuVector reads/writes go through the `mcp__claude-flow__memory_*` tools; no raw SQL/CLI (which bypass the embedding pipeline). ADR-090 ring order is honoured for any cross-node write ordering.

## 5. Aggregate detail

### 5.1 CorpusGeneration (root)

The atomic, content-addressed corpus snapshot. Owns generation identity; owns nothing about *meaning* (that is the Authority).

| Field | Type | Invariant |
|---|---|---|
| `commitSha` | `Sha` (`GITHUB_SHA`) | Generation identity; equals the source repo commit |
| `buildId` | `String` | Unique per pipeline run |
| `generatedAt` | `DateTime` | One shared timestamp threaded through **every** emitter (WS-A) |
| `pipelineVersion` | `SemVer` | Reproducibility pin |
| `artifacts` | `Map<Path,{sha256,bytes,count}>` | Every published artifact sha-pinned; manifest written **last** |
| `versionIRI` | `IRI` | `owl:versionIRI` + `prov:wasDerivedFrom <repo@sha>` in both TTL headers |

**Commands:**
- `SyncAndBuild(commitSha)` — pull corpus from GitHub, parse via the canonical parser, emit artifacts, write `api/build-manifest.json` **last**.
- `Publish()` — hand the Generation to `mirror.sh` for atomic cloud replica publish and to VisionClaw for atomic `load-generation`.
- `Resolve(corpusSha_match: exact|at_least|latest)` — on a distill admit, reconcile a job's `corpusSha` to a Generation; on mismatch run **one** on-demand mirror refresh; `exact` + still-mismatched → terminal `corpus-unavailable`.

**Events:** `GenerationBuilt {commitSha, buildId, artifactCount}` · `GenerationPublished {commitSha, replicaUrl, generatedAt}` · `GenerationLoadRequested {commitSha}` (VisionClaw atomic feed).

**Invariants:** I9 (atomic publish); the manifest is written last or the Generation is invalid; a Generation is immutable once published (a new corpus = a new `commitSha`).

### 5.2 OntologyAuthority (root)

The single owner of parse → reason → conflict-gate → index for a Generation. This aggregate is the consolidation target: it absorbs the *authoritative* half of every duplicated capability (§8), and every fast in-process piece becomes its client.

| Field | Type | Invariant |
|---|---|---|
| `generation` | `CorpusGeneration` | Every derivation is pinned to exactly one Generation |
| `closure` | `RdfGraph` | The one authoritative EL closure → `ontology-inferred.ttl` |
| `parserRev` | `String` | The canonical corpus→entity parser revision (one parser, not two) |
| `indexSet` | `{scaffoldIndex, proseIndex, ruvectorCondense}` | Derived once; the source BC21/one-brain/ontology-mcp all read |
| `conflictReport` | `ConflictReport` | Typed pre-publish gate (`pipeline/conflicts.py`) |

**Commands:**
- `Reason()` — compute the authoritative closure. **Which engine is canonical (promote VisionClaw Whelk EL++ into the Loom vs make logseq `pipeline/reason.py` authoritative and have VisionClaw load its output) is an OPEN DECISION for ADR-135 — see §12.** The invariant is one authority, conformance-tested, whichever engine wins.
- `GateConflicts()` — compose the two checkers, not duplicate them: **`conflicts.py` (typed `ConflictReport`) is the pre-publish gate; Whelk consistency is the pre-assert gate.** High-severity conflict → block publish (the CI gate already hard-fails on high-severity, per the recent `publish.yml` change).
- `BuildIndex()` — derive `scaffold-index v1`, `prose-index v1`, and the RuVector condensation **once**, for downstream clients.
- `Conform(client, derivedState) → bool` — the conformance test a thin client runs to prove its derived state equals the Authority's.

**Events:** `AuthorityReasoned {commitSha, axiomsProcessed, closureTriples}` · `ConflictGatePassed {commitSha}` / `ConflictGateBlocked {commitSha, highSeverityCount}` · `IndexGenerated {commitSha, scaffoldCount, proseCount, vectorCount}` · `ConformanceVerified {client, commitSha}` / `ConformanceDrift {client, commitSha, delta}`.

**Invariants:** I1 (one authority, no drift); I7 (authoritative/slow path only — reasoning never runs on a consumer hot path); a `ConformanceDrift` event is a **hard** failure, never a warning.

### 5.3 DistillationJob (root)

The job-URN-anchored lifecycle of one deferred distillation. Owns the state machine; owns the idempotency anchor; owns the relationship to its work-ledger bead.

| Field | Type | Invariant |
|---|---|---|
| `jobUrn` | `urn:agentbox:job:<pubkey>:<sha256-12>` | Content-addressed over Identity Core (I5); ADR-013 §6 |
| `identityCore` | `{kind:"ontology.distill", corpusSha, scope, budget_tokens}` | RFC 8785 JCS canonical; `budget_tokens` **is** content |
| `scope` | `{slugs[] sorted+deduped \| domain \| question-normalised}` | Arrays sorted where semantically unordered |
| `deadline` | `DateTime` | Execution field — **not** hashed |
| `modelPolicy` | `any-serving \| pinned:<id>` | Execution field; `pinned`+not-loaded → `waiting-for-model` |
| `corpusSha_match` | `exact \| at_least \| latest` | Resolution rule against the live Generation |
| `beadId` | `BeadUrn` | The distill bead (work ledger); carries `jobUrn` in a typed field |
| `lease_epoch` | `u64` | Monotonic; incremented on every claim/reclaim (fencing) |

**State machine** (provider side — HP `jobd`, stdlib pull-worker; concurrency 1 per agentbox ADR-113 §2.2; queue **in front of** the model, never inside):

```
submitted → admitted → queued → running → distilled → stamped → delivered → acked
                │                                                    │
                ├─ waiting-for-model (pinned model not loaded)       └─ expired
                ├─ preempted (bench wins the GPU flock)
                └─ [terminal causes] corpus-unavailable | model-unavailable | gpu-contended
```

- **Claim-time admission** (a pull worker has nobody to 429): `jobd` claims **only** when queue depth < cap AND estimated completion fits the deadline; otherwise it leaves the job unclaimed for the harness reaper.
- **admitted** probes `/v1/models`, records the exact model id + file path/metadata, and takes the GPU flock (`~/githubs/llm-server/.gpu.lock`) **non-blocking**. Arbitration: **benches always win**; `jobd` never preempts a bench; a job blocked past deadline expires `cause=gpu-contended`.
- **running** re-probes model identity per LLM call and aborts on mid-job change; map-reduce over the class set (scaffold retrieval → 1..N LLM calls).
- `jobd` is **stateless-by-design**: queue durability = re-pull on restart; nothing persisted on HP.

**Commands:** `Submit(scope, question, budget, deadline, modelPolicy, corpusSha_match)` (dedupe-on-create → same `jobUrn`) · `Admit()` · `Claim(actor, pubkey)` — conditional `UPDATE ... WHERE actor IS NULL` returning changes-count; binds the lease to the claimant's verified pubkey · `Reclaim()` — increments `lease_epoch` · `Deliver(envelope)` · `Expire(cause)`.

**Events:** `JobSubmitted {jobUrn, beadId}` · `JobAdmitted {jobUrn, model_id_probed, lease_epoch}` · `JobDistilled {jobUrn, engaged_class_slugs, scaffold_engaged}` · `JobDelivered {jobUrn, sink, lease_epoch}` · `JobExpired {jobUrn, cause}`.

**Invariants:** I5 (content-address identity); I3 (`scaffold_engaged=false` quarantined at `distilled`, never reaches `delivered`); the delivering pubkey **must** equal the job's designated provider (§7); concurrency 1.

### 5.4 ResultEnvelope (root)

The signed, sha-pinned unit of delivery. Self-contained and verifiable — which is what lets the reconciliation janitor act as courier-of-record.

```json
{
  "jobUrn": "urn:agentbox:job:<pubkey>:<sha256-12>",
  "summary": "…distilled ontology-grounded prose…",
  "corpusSha_used": "<GITHUB_SHA>",
  "corpus_generation": "<buildId>",
  "corpus_generation_mismatch": false,
  "model_id_probed": "gemma-4-31B-it-qat",
  "model_file_meta": { "path": "…/model.gguf", "size": 0, "mtime": "…" },
  "toolkit_rev": "…",
  "llama_build": "…",
  "engaged_class_slugs": ["blockchain", "zk-rollup"],
  "scaffold_engaged": true,
  "retrieval_transcript_hash": "<sha256>",
  "injected_tokens": 0,
  "tokens_used": 0,
  "latency_ms": 0,
  "derivation_labels": { "blockchain": "asserted", "zk-rollup": "inferred" },
  "corpusNature": "synthetic-ai-generated-human-directed",
  "generatedAt": "2026-08-11T…Z",
  "lease_epoch": 3,
  "sig": "<BIP-340 signature over the canonical envelope>"
}
```

| Field group | Invariant |
|---|---|
| `sig` (BIP-340) | Verified at **both** the derived write door AND the RuVector read, against the distiller-provider allowlist (§7). Unsigned/unverifiable → 400 (write) / fail-labelled-quarantine (read). Never crosses `clampToBudget`. |
| `jobUrn / corpusSha_used / scaffold_engaged` | Reconciled server-side against the submitted job; mismatch → reject. |
| `scaffold_engaged / engaged_class_slugs` | SHOULD bind to `retrieval_transcript_hash` the harness can reproduce (self-assertion hardening). |
| `corpus_generation / corpus_generation_mismatch` | Always present — the envelope states which Generation it used and whether it matched the request. |
| `derivation_labels` | Per-slug `asserted \| inferred \| summary`; `inferred` is never presented as ground truth. |
| `lease_epoch` | Both sinks reject stale-epoch deliveries. |

**Delivery — Phase 1 is TWO paths, not five** (strict ordering; close is the linearisation point):
- **(a) RuVector payload** — the harness-side adapter (HP has no MCP) does `memory_store(key=jobUrn, namespace="ontology-distilled")` on receipt. Typed-metadata gate REQUIRED for this namespace. **TTL law:** `TTL ≥ consumer_deadline + lease_TTL × max_redeliveries + sweep_period + slack`, clock starts at delivery. **First-write-wins on content:** an existing key with a different result sha → reject + log divergence (not upsert).
- **(c) bead close** — a **CAS**: `UPDATE … SET status='closed' WHERE id=? AND status='claimed' AND actor=? AND lease_epoch=?` returning changes-count; a failed CAS is a no-op. Close is strictly **after** the RuVector write (I2).

**Phase-2 delivery** (additive, strictly-after-close): (b) VisionClaw fenced `:summary` durable copy + ProvenanceEmitter emit; (d) agent-events fast-wake (fire-and-forget accelerator only — it *schedules* the recombine worker, never holds a turn); (e) kind-30840 operator digest.

**Events:** `ResultStamped {jobUrn, sig}` · `ResultDeliveredToRuVector {jobUrn, ttl}` · `BeadClosedDone {jobUrn, lease_epoch}` · `ResultElevated {jobUrn, candidateId}` (Phase 2 — a significant distillate promoted to an `EnrichmentCandidate`).

**Invariants:** I2 (closed-done ⇒ retrievable); I3; I8 (`corpusNature`); the envelope is self-contained + signed so the janitor can complete a tail from it alone.

### 5.5 Deferred: ConnectorDescriptor (noted, not built)

The capability-discovery aggregate (a machine-readable descriptor of `backend × transport × latency-class`) has **no Phase-1 consumer** and is **deferred**. Recorded here so a later "generalised connector platform" claim has a home — but per the honesty rule (OCP §Connector classes) that claim is only made when a **second** distillation provider lands. Until then this context ships the three **existing** latency classes (static-cloud replica / live-lan façade / slow-llm distillation) as Loom facets, not as a general registry.

## 6. Domain events

| Event | Producer | Consumer(s) | Channel |
|---|---|---|---|
| `GenerationPublished` | CorpusGeneration | mirror.sh (cloud replica), VisionClaw `load-generation` | In-process + HTTP |
| `AuthorityReasoned` | OntologyAuthority | Conformance tests, publish gate | In-process |
| `ConflictGateBlocked` | OntologyAuthority | CI (`publish.yml` hard-fail), operator | In-process + CI |
| `ConformanceDrift` | OntologyAuthority | CI hard-fail, operator alert | In-process |
| `IndexGenerated` | OntologyAuthority | BC21 one-brain, HP scaffold, ontology-mcp (thin clients) | In-process |
| `JobSubmitted` | DistillationJob | jobd pull loop, dedupe check | RuVector / mgmt-api |
| `JobAdmitted` | DistillationJob | Liveness (heartbeat), queue-depth metric | In-process |
| `JobDistilled` | DistillationJob | Stamp/deliver pipeline | In-process |
| `JobDelivered` | DistillationJob | Recombine worker, reconciliation janitor | RuVector + bead |
| `JobExpired` | DistillationJob | Outcome-aware recombine, cause-split telemetry | In-process |
| `ResultElevated` | ResultEnvelope | BC20 propose spine (candidate) | Phase 2 · ACSP 31402 |
| `ResultDeliveredToRuVector` | ResultEnvelope | `ontology-distilled` ns consumers | RuVector (MCP-only) |
| `BeadClosedDone` | ResultEnvelope | getReady recombine (outcome-aware) | BC-BP bead |

These are observable signals for the verifiability story (§10). Phase-2 job events additionally ride the relay as **ACSP kinds 31408 DistillJobRequest / 31409 DistillJobResult** (new IS-Envelope kind `ontology.distill` via agentbox ADR-075 D1), NIP-59 gift-wrapped — never reusing 31406/31407.

## 7. Anti-corruption layer & published language

**The provider door (WS-D) is the primary ACL.** New management-api verbs (`pending-per-provider`, `claim`, `result-upload`) — **not** the existing local-subprocess `/v1/tasks`. Hardening obligations, all binding:

- **Verify the BIP-340 envelope sig at BOTH trust-consumption points**: the derived write door (`POST /api/ontology/derived`) AND the RuVector read (recombine). Against a **distiller-provider allowlist** of `did:nostr` identities — **generic `power_user` must NOT suffice to write `:summary`.** Unsigned/unverifiable → 400 (write) / fail-labelled-quarantine (read).
- **Reconcile envelope fields** (`jobUrn / corpusSha_used / scaffold_engaged`) against the submitted job server-side; mismatch → reject.
- **Provider door = strict-nip98; drop Bearer.** The shared `MANAGEMENT_API_KEY` is a broadly held LAN dev secret with no replay defence. Bind the claim lease to the claimant's verified pubkey; the **delivering pubkey MUST equal the job's designated provider**. Bind the bead close to job identity.
- **HP key at rest**: age/agenix-encrypted or hardware-held (never a plaintext dotfile); a short-lived capability grant, not long-lived `power_user` membership; a **per-request revocation list** (not env-at-boot) so a stolen key is cut without redeploy.
- **CI gate** that the LAN-facing VisionClaw binary is a **release build** — the `dev-session-token → power_user` path is debug/dev-auth only; a debug build on the LAN reopens the fence.
- **Phase-2 nostr**: **NIP-59 gift-wrap** scope/question to the provider key (the content-addressed URN hides nothing; plaintext scope would leak to the relay). Same for the kind-30840 digest. **Retire NIP-26 delegation** (deprecated, unrevocable) → per-consumer NIP-59 capabilities.

**Provenance routing ACL** (BC22 published language): the distillate's PROV-O triples route through the PRD-022 `ProvenanceEmitter` into `urn:ngm:graph:provenance` — **not** into `:summary` (the `/derived` fence writes summary quads `:summary`/`:usage` **only**; PROV-O in `:summary` would violate VisionClaw PRD-022 constraint 3, verified in `ontology_derived_handler.rs:30-40`). Neither provenance graph is canonical: the **reconciliation MAPPING** (alignment vocabulary) reconciles `urn:ngm:graph:provenance` (VisionClaw PRD-022 constraint 3) with `urn:agentbox:graph:provenance` (agentbox ADR-049); each graph keeps its owner's invariants.

**Published language — the consumer-side MCP tools (WS-J).** The single biggest gap the operator named: no agent should hand-roll the six-step submit/claim/deliver/close/fetch/recombine dance. The Loom publishes three `ontology-bridge` MCP tools as its consumer surface:

- `ontology_distill_submit({scope, question, budget_tokens, deadline, model_policy, corpusSha_match})` → mints the job URN (via management-api `uris`), signs with the **HARNESS machine key** (individual agents never touch keys), creates distill + recombine beads, returns `{jobUrn, beadId}`. Dedupe-on-create.
- `ontology_distill_fetch({jobUrn})` → budget-clamped retrieval of the delivered, sig-verified result (fail-labelled on miss/unverified).
- `ontology_distill_await({jobUrn, deadline})` → deadline-bounded poll for mid-workflow use — polls across the deadline and returns whatever landed (or a labelled timeout); it **does NOT hold a turn on the LLM** (honours the no-synchronous-await LAW, I7).

Two consumption modes, stated explicitly: **fire-and-collect-later** (cross-session, the default) and **deadline-bounded await** (mid-workflow, bounded). **getReady is outcome-aware** (recombine reads the blocker's `outcome` before dereferencing `result_ref`: `done` → fetch; `expired|failed` → propagate a *labelled* failure into search-only recombination). Recombine-worker ownership: Phase 1 = consumption is TOOL-side (`await`/`fetch`), the recombine bead is optional; the autonomous recombine-bead worker is a Phase-2 workstream with a named owner (a claude-flow daemon consumer polling getReady).

## 8. Ownership migration (the consolidation map)

Direct-to-target, on a dev/test estate: the Loom becomes the **single owner** of each scattered capability, and the fast in-process pieces become **thin clients** of its authoritative Generation. This is not a big-bang rewrite of the algorithms — it is a re-homing of *authority*. ADR-112's no-hot-path-LLM law is preserved because the Loom owns only the slow/authoritative path.

| Capability | Was (scattered / duplicated) | Moves INTO BC24 (authority) | Stays as a client (fast local path) |
|---|---|---|---|
| **Reasoning** | VisionClaw Whelk EL++ → `:inferred` **vs** logseq `pipeline/reason.py` transitive closure → `ontology-inferred.ttl` (two reasoners → drift) | The **one** authoritative closure, per Generation (engine choice = OPEN DECISION, ADR-135) | VisionClaw `:inferred` **derives from** the Loom closure; conformance-tested (I1) |
| **Retrieval / index** | agentbox one-brain (ADR-112) **vs** HP `ontology_scaffold.py` **vs** ontology-mcp (three impls) | The **one** index generation (scaffold-index / prose-index / RuVector-condense), derived once | All three become **thin clients** of the Loom index; the ADR-112 in-process lib stays the hot-path retriever, stops being an independent index builder |
| **Parsing + sync** | Rust canonical-entity `knowledge_graph_parser` **vs** Python `jsonld_parser` (two parsers); VisionClaw `github_sync_service` CLEAR+INSERT | The **one** canonical parser lives with the Loom (which owns sync) | VisionClaw consumes **Generations** (atomic `load-generation N`), never re-parses or CLEAR+INSERTs — that service is **RETIRED** |
| **Conflict / consistency** | Whelk consistency **vs** `pipeline/conflicts.py` (two checkers) | **Composed, not duplicated**: `conflicts.py` (typed `ConflictReport`) = pre-publish gate; Whelk consistency = pre-assert gate | VisionClaw runs Whelk pre-assert as a client of the Loom's published Generation |
| **Corpus GitHub write** | VisionClaw `github_sync_service` force_full CLEAR+INSERT (wiped runtime decision triples) | The Loom owns the GitHub **write** side (enrichment/elevation commits) via the BC20 propose spine + CI conflict gate | VisionClaw keeps the **propose door** (governance outcomes flow to the Loom for the merge) |
| **The "one brain" (ADR-112)** | agentbox in-process retrieval that also built its own index | Nothing — the brain is preserved | The one brain **resolves the Loom façade** for authoritative state; it is now a **Loom client** |

What VisionClaw keeps and does **not** cede: visualisation, GPU physics, live-linkage (`graphUpdated`), and the governance write **door** (`propose`). What the published site becomes: a Generation the Loom publishes — a cloud read replica and an always-available fallback (corpus changes slowly, stale reads are fine; only fresh distillation pauses when the Loom host is down).

## 9. BC catalogue reconciliation & the BC22 collision

Recording a real numbering collision the catalogue must resolve, per the OCP DDD must-fix.

- **`ddd-xr-godot-context.md`** hard-titles itself **"XR Godot Bounded Context (BC22)"** (and the README lists it as BC22).
- **`ddd-semantic-trust-layer-context.md`** claims **"BC22 — provisional, pending BC catalogue update."**

Two contexts hold BC22. Both were authored provisionally against a catalogue that was never updated. **Proposed resolution (for the catalogue owner):**

| Number | Context | Basis |
|---|---|---|
| BC20 | AgentboxIntegration | Settled (write/spawn door) |
| BC21 | OntologyAugmentation | Settled (read/augment + L2/L3 writeback) |
| **BC22** | **SemanticTrustLayer** | Keep — the OCP brief and this context both bind BC22 = SemanticTrustLayer; it is the earlier-dated claimant (2026-06-21 vs 2026-07-15) and the one BC24 depends on by number |
| **BC23** | **SemanticIntegrity & Provenance** (agentbox — the URI/DID grammar authority; agentbox PRD-022) | Reserve — referenced by BC24 as the grammar Conformist target; no VisionClaw DDD doc yet |
| **BC24** | **OntologyLoom** | This document |
| **BC25** | **XR Godot** (renumbered from BC22) | Move — it is the later claimant and has **no** relationship to the ontology/trust axis; renumbering it is the lowest-blast-radius fix. All internal "BC22" references in `ddd-xr-godot-context.md` become BC25; the README row updates. |

This is a **proposal**, not a fait accompli — the catalogue owner ratifies. The point of recording it here: BC24 cites "BC22 SemanticTrustLayer" and "BC23 grammar" by number, and those numbers must be unambiguous before this context is ratified.

## 10. Validation strategy (liveness proof — anti-PRD-018)

Each aggregate is validated as **live**, not merely wired. Following the "wired ≠ working" doctrine and the OCP liveness must-fixes (WS-G):

| Aggregate | Startup / build canary | Runtime proof |
|---|---|---|
| CorpusGeneration | Build a Generation, verify every artifact `sha256` against the manifest, atomic publish → `GenerationPublished` | `build_manifest_written_last` assertion; VisionClaw `load-generation` round-trips the same `commitSha` |
| OntologyAuthority | Reason one Generation; run the conformance test VisionClaw `:inferred` ≡ published closure | `ConformanceDrift` count == 0 (hard gate); `axiomsProcessed > 0`; `conflictGate` hard-fails on high severity |
| DistillationJob | **Periodic cap-exempt canary**: submit at T, alert if not landed by T+deadline; `canary:true` → lands in `ontology-canary` ns, never writes `:summary`, never digests, excluded from elevation + queue-depth metrics | jobd **heartbeats every poll** (short-TTL RuVector key / provider-status); liveness = heartbeat-staleness threshold (kills the "green-but-zero" / ".48-is-dead" class). **Boot completion is NOT liveness.** |
| ResultEnvelope | Deliver a synthetic signed envelope; verify sig at both doors; confirm `fetch` returns it budget-clamped | `closed-done ⇒ payload-retrievable` audited by the janitor; divergence count == 0 |

**The reconciliation janitor** (idempotent, harness-side, period `P < min(lease_TTL, memory_TTL)/2`): (1) claimed beads past `lease_TTL` → increment epoch, clear actor → re-eligible; (2) OPEN distill beads whose `jobUrn` key exists in RuVector → complete the tail from the stored signed envelope (courier-of-record); (3) open beads past consumer deadline with no payload → CAS-close `expired` (outcome-aware recombine handles it); (4) divergence audit: RuVector entries missing from `:summary` re-posted before TTL. **Deadline reaper**: at a job's deadline, CAS-close the distill bead `expired` (`cause: unclaimed | claimed-not-delivered`), unblocking recombine to proceed search-only — this is what makes "the Loom host's absence never blocks a turn" TRUE. `jobd.service` ships `Restart=on-failure`; the runbook makes `systemctl enable --now jobd` an explicit operator step with a verify command.

## 11. Migration & coexistence (direct-to-target build order)

Not phased live-migration — a **build order** toward one end-state on a dev/test estate (OCP §0 direct-to-target). All adversarial must-fixes (sig-verify, CAS-close, lease fencing, reconciliation janitor, atomic-mirror corpusSha, strict-nip98 provider door, distiller allowlist, release-build CI gate, no-synchronous-await LAW) apply to the **end state** and are **not** waived by going direct — they are correctness/security, not staging scaffolding.

**Phase 1 — close the loop** (the smallest honest capstone):
- **WS-A** build-manifest + sha-pinning + atomic mirror (logseq pipeline) → CorpusGeneration.
- **WS-B** two JSON Schemas: job envelope + result envelope.
- **WS-C** beads: `job` URN kind (deterministic, ADR-013 §6), typed `result_ref`, `lease_epoch` + claim-lease + reclaim-TTL, CAS close — a **schema migration + contract tests**, not "~20 lines."
- **WS-D** management-api provider door: new verbs (pending-per-provider / claim / result-upload); strict-nip98.
- **WS-E** HP `jobd` (stdlib pull-worker; **CREATE** the `.gpu.lock` convention; heartbeat).
- **WS-J** consumer MCP tools (submit / fetch / await) — **same weight as WS-E**; without it the loop does not close for a real agent.
- **WS-G** reaper + janitor + heartbeat + cap-exempt canary.
- **Consolidation authorities**: designate the one reasoner / one index / one parser / one conflict authority (§8) and stand up conformance tests; VisionClaw's `github_sync_service` CLEAR+INSERT is **retired** and replaced by atomic `load-generation`.

**Phase 2 — generalise + durable** (sequence after the core loop stands up):
- **WS-F** VisionClaw fenced `:summary` durable landing + ProvenanceEmitter routing + `NotifyGraphUpdated` wake.
- **WS-H** elevation: `EnrichmentCandidate → ElevationActor + KnowledgeEnrichment broker case (ACSP 31402/31403)` composed with VisionClaw ADR-121 WS-10 propose spine → corpus → CI conflict gate → published site. **Replay-before-accept is a HARD gate on this path** or the replay claim is dropped (verification rests on sig + sha-pins; deterministic replay is best-effort same-binary/same-weights only).
- **WS-I** nostr relay federation for cloud consumers (new kinds 31408/31409, NIP-59 wrapped).
- The autonomous recombine-bead worker (a named claude-flow daemon consumer polling getReady).

**Coexistence:** none of BC21's retrieval/budget algorithms, BC22's shape/provenance/federation aggregates, or BC20's governed write are altered — the Loom re-homes *authority* and adds the distillation channel; the neighbours become clients of one authoritative Generation.

## 12. Open decisions (for the operator)

1. **Canonical reasoner engine (ADR-135).** Promote VisionClaw Whelk EL++ into the Loom as the authoritative closure engine, **or** make logseq `pipeline/reason.py` authoritative and have VisionClaw load its output? The invariant (I1: one authority, conformance-tested) holds either way; the choice is which engine is canonical. **Left to the operator.**
2. **Deployment topology default (ADR-135).** Deployment A (HP host, GPU-local façade+distillation) vs Deployment B (Docker sidecar on `visionclaw_network`, distillation delegated via `DISTILL_BACKEND_URL`). Both expose the identical façade; which is the reference default for the capstone demo?
3. **BC catalogue ratification (§9).** Confirm BC22 = SemanticTrustLayer, reserve BC23 = SemanticIntegrity & Provenance, renumber XR Godot → BC25. Catalogue owner ratifies.
4. **Distiller-provider allowlist bootstrap.** N=1 provider today (HP). What is the allowlist admission process when a second provider lands (the trigger for the honest "generalised platform" claim)?
5. **`budget_tokens` tiering vs ADR-116.** The Identity Core hashes `budget_tokens` as content. Confirm the permitted budget values align with the agentbox ADR-116 tier ceilings so two callers at the same tier hash-collide (dedupe) as intended.
6. **Elevation replay gate (WS-H).** Ratify replay-before-accept as a hard gate, or accept the honest downgrade (verification on sig + sha-pins only, replay best-effort). Determines whether the "deterministic replay" line stays in the verification story.
