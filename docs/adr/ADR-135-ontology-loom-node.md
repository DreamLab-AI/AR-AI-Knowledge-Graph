# ADR-135 — Adopt the Ontology Loom node + deferred distillation job pattern

**Status:** Proposed (direct-to-target end state; Phase-1 workstreams WS-A/B/C/D/E/G/J, Phase-2 WS-F/H/I)
**Date:** 2026-08-11
**Decision-type:** Architecture (keystone — node boundary + corpus-lifecycle re-home)
**Deciders:** Dr John O'Hare (operator)
**Relates (this repo, VisionClaw):** ADR-112 (retrieval spine / one-brain / no hot-path LLM),
ADR-113 (condensation mesh, concurrency-1), ADR-116 (tiered token budgets), ADR-117
(server-side SPARQL clamp), ADR-118 (`/load` hardening), ADR-119 (verifiable per-channel
liveness telemetry, anti-PRD-018), ADR-090 (hexagonal crate ring), ADR-099 (Whelk-rs EL
primary reasoner posture), ADR-121 (self-improving writeback loop, WS-10 propose spine),
ADR-122 (two-speed writeback governance routing), ADR-125 (did:nostr multikey), ADR-127 +
**VisionClaw PRD-022 (semantic-trust-layer)** (ProvenanceEmitter, `urn:ngm:graph:provenance`),
**VisionClaw ADR-050 (pod-backed-kgnode)**, ADR-075 (IS-Envelope), ADR-110 (ACSP control
surfaces).
**Relates (agentbox):** **agentbox ADR-013 (§6 URN-kind extension API, `uris.js`)**, **agentbox
PRD-022 (semantic-integrity / provenance-decisions)**, **agentbox ADR-049
(`urn:agentbox:graph:provenance`)**, **agentbox ADR-050 (decision-elevation)** — *pattern
precedent only, not the mechanism here*.
**Relates (logseq pipeline):** `pipeline/reason.py`, `pipeline/conflicts.py` (typed
`ConflictReport`), `pipeline/build.py`, `api/build-manifest.json`.

> Keystone ADR for the Ontology Loom family. Companion PRD-025 owns requirements; sibling
> decisions on the provider door (WS-D), HP `jobd` (WS-E), and consumer MCP tools (WS-J) are
> summarised in the Decision register (§4) and split into implementation ADRs as they land.
> This ADR records the **architecture decision**; it does not re-derive the design brief.

---

## 1. Context

VisionFlow's ontology intelligence is scattered across four codebases and, worse, **duplicated**
in ways that have already caused live drift (the 8152-vs-5975 class-count divergence; a stale
`:assert` graph). There are two reasoners (VisionClaw Whelk EL++ → `:inferred` vs
`pipeline/reason.py` transitive closure → `ontology-inferred.ttl`), three retrieval
implementations (agentbox `ontology-retrieval` "one brain" vs HP `ontology_scaffold.py` vs
`ontology-mcp`), two parsers (Rust `knowledge_graph_parser` vs Python `jsonld_parser`), and two
conflict checkers (Whelk consistency vs `conflicts.py`).

The corpus-lifecycle owner today is the wrong component. VisionClaw's
`github_sync_service` performs a `force_full` **CLEAR+INSERT** of the entire
`urn:ngm:graph:ontology:assert` graph on sync — wiping runtime decision-class triples so
thoroughly that VisionClaw ADR-050 (pod-backed-kgnode) needed an inverse-corpus-path just so
the graph could survive its own reload. A real-time render/physics engine is playing
corpus-lifecycle-manager. That is the "clunky GitHub connector."

The operator reframe (2026-08-11) is that HP-Desktop is not a peripheral connector: it is the
**reference deployment of a first-class VisionFlow node, the Ontology Loom**. The Loom weaves
the corpus into a reasoned ontology, holds it as the canonical source of truth, and serves that
intelligence behind a **stable, model-swappable façade**. This is a DEV/TEST estate, so we build
the **target end state directly** — no staged live migration, no transitional shims. The one
genuinely new machinery is (a) the Loom node (stable façade + corpus-lifecycle ownership) and
(b) the **deferred distillation job pattern**; everything else is alignment and contract
formalisation over shipped surfaces.

PRD-018 remains the governing cautionary precedent (ADR-119): every path below ships with a
liveness proof; "wired ≠ working" is the failure mode we design against.

---

## 2. Decision

Adopt the **Ontology Loom** as a first-class VisionFlow node with a deployment-agnostic stable
façade; re-home corpus-lifecycle ownership to it; consolidate each duplicated capability to one
authority; and add the deferred distillation job pattern as one Loom capability. Decisions D1–D8.

### D1 — The Loom is a node with a deployment-agnostic, model-swappable façade contract

The Loom is a **role with a stable façade contract**; HP-Desktop is its reference deployment; the
role is host-portable. The façade is the ONLY thing consumers bind to. It has two facets that
separate cleanly: a lightweight **lifecycle+façade facet** (stdlib-portable; git creds + outbound
internet) and a GPU **distillation facet** (the swappable model). Model identity is carried in
*results*, never in the endpoint — this is the "no technical debt on upgrade" guarantee.

**D1.1 Façade endpoints (deployment-agnostic, stable contract).** Versioned under `/loom/v1`:

| Method + path | Purpose | Latency class |
|---|---|---|
| `GET /loom/v1/generation` | current generation descriptor `{commitSha, buildId, generatedAt, artifacts{path:{sha256,bytes,count}}, corpusNature}` | ms |
| `GET /loom/v1/generation/{buildId}/artifact/{name}` | fetch one sha-pinned artifact (ttl, page-json, scaffold-index, prose-index) | ms |
| `GET /loom/v1/retrieval/scaffold?scope=…&budget_tokens=…` | budget-clamped scaffold/prose retrieval derived from the current generation index | fast-lan |
| `POST /loom/v1/distill` | submit a deferred distillation job → `{jobUrn, beadId}` (dedupe-on-create) | submit ms; result 10s–10min |
| `GET /loom/v1/distill/{jobUrn}/status` | outcome-aware lifecycle state (§D4) | ms |
| `GET /loom/v1/distill/{jobUrn}/result` | signed, sha-pinned, budget-clamped result envelope (§D4.6); fail-labelled on miss/unverified | ms |
| `GET /loom/v1/healthz` | heartbeat/liveness (§Consequences, ADR-119) | ms |

The façade is **read/serve + submit/collect only**. It NEVER exposes a write door into the corpus
graph (write-path-never-widened): governance outcomes reach the Loom out-of-band (§D2) and the
Loom manages the GitHub merge; the façade does not.

**D1.2 Model-swap seam.** The distillation backend is a single config line,
`DISTILL_BACKEND_URL` (an OpenAI-compatible base URL), never an architecture change. Gemma →
Muse-Glimmer → next swaps behind the façade with ZERO consumer change, because the contract is
stable and the exact model id + file metadata are probed at admission and stamped into every
result envelope (§D4.6). Retrieval/scaffold and generation reads have **no LLM on their path**
(ADR-112 one-brain / no hot-path LLM holds): only `/loom/v1/distill` touches a model, and it does
so deferred, off the hot path.

**D1.3 Two deployment topologies, identical façade.**
- **Deployment A — HP host (reference):** façade + distillation co-located on HP; `DISTILL_BACKEND_URL`
  → HP llama.cpp `:8084`/`:8085` (GPU-local).
- **Deployment B — Docker sidecar (visionclaw_network):** the lightweight façade+lifecycle facet
  as a container, no GPU; `DISTILL_BACKEND_URL` → HP `:8084`, a cloud model, or a local model
  container. Consumers cannot tell which is running. The lifecycle+façade facet is stdlib-portable
  and runs on HP, in a container, or on ml; the GPU is needed only by whichever backend the façade
  points at.

**OPEN DECISION D1-a (operator):** whether the reference deployment stays HP-host (A) or moves to
a docker sidecar (B) with HP as the delegated backend for the capstone demo. Recommendation:
ship Deployment A as reference, keep B green in CI as the portability proof.

### D2 — Corpus-lifecycle ownership moves to the Loom; VisionClaw retires CLEAR+INSERT and loads GENERATIONS

**D2.1 Define a generation.** A **generation** is an atomic, content-addressed corpus snapshot
identified by the build-manifest (WS-A): `{commitSha (GITHUB_SHA), buildId, generatedAt,
pipelineVersion, artifacts:{path:{sha256,bytes,count}}, corpusNature}`. The manifest is written
**LAST** by the pipeline; one shared timestamp + `commitSha` threads through every emitter; both
TTL headers carry `owl:versionIRI` + `prov:wasDerivedFrom <repo@sha>`. A generation is either
fully present (all artifact shas verify) or absent — never mixed-build.

**D2.2 The Loom owns the full loop.** Sync corpus FROM GitHub → compute generation + reason (EL
closure, §D3) → run the pre-publish conflict gate (`conflicts.py` typed `ConflictReport`) →
publish the generation (LAN façade + the cloud read-replica on narrativegoldmine.com) → manage
the GitHub **write side** (enrichment/elevation merges, §D6/§WS-H). The published site is simply a
generation the Loom publishes: an always-available fallback when the Loom host is down (corpus
changes slowly, stale reads are fine; only fresh distillation pauses).

**D2.3 VisionClaw becomes a pure generation consumer.** `github_sync_service`'s
`force_full` CLEAR+INSERT is **RETIRED**. VisionClaw ingests generations by an atomic
**load-generation-N** operation, replacing today's `rebuild_assert_graph`:
1. `GET /loom/v1/generation`; if `commitSha` unchanged, no-op.
2. fetch each artifact, verify `sha256` against the manifest (reject the whole generation on any
   mismatch — atomic, never partial).
3. build `:assert` + `:inferred` into a **shadow named graph pair**
   (`urn:ngm:graph:ontology:assert.staging`), then swap by rename (the atomic-mirror discipline
   of WS-A applied graph-side). Runtime decision-class triples are preserved across the swap
   because the swap is additive-then-rename, not CLEAR+INSERT.

VisionClaw keeps visualisation, GPU physics, live-linkage (`graphUpdated`), and the governance
**propose door** (ADR-120/121). Governance OUTCOMES flow to the Loom, which owns the GitHub merge
(§D6). VisionClaw no longer parses the corpus or reasons over it as an independent authority — it
consumes the Loom's authoritative generation.

### D3 — ONE reasoner authority + a conformance test (engine choice is an OPEN DECISION)

The invariant is **one authority**: the Loom runs the authoritative EL closure once per
generation; the published `ontology-inferred.ttl` AND VisionClaw's `:inferred` graph both DERIVE
from that single closure. They must never re-derive independently again.

**OPEN DECISION D3-a (operator) — which engine is canonical:**
- **Option 1 — promote Whelk to the Loom.** Whelk-rs EL++ (ADR-099 posture) becomes the Loom's
  closure engine; `pipeline/reason.py` is retired to a conformance oracle. Pro: EL++ is
  strictly more complete than the Python transitive closure; keeps the incremental-reasoner
  investment. Con: pulls a Rust reasoner into the stdlib-portable lifecycle facet (Deployment B
  must then ship it, or delegate closure like it delegates distillation).
- **Option 2 — make `reason.py` authoritative; VisionClaw imports the result.** The Python
  pipeline closure produces `ontology-inferred.ttl` as the canonical artifact; VisionClaw's
  `:inferred` is *loaded from it*, not recomputed by Whelk. Pro: keeps the lifecycle facet pure
  stdlib/rdflib; single artifact of record. Con: loses Whelk's EL++ completeness unless
  `reason.py` is upgraded.

**Recommendation:** Option 2 for the capstone (keeps Deployment B GPU-free and stdlib-portable;
the generation artifact is the single source of truth), with Whelk retained inside VisionClaw as
an **incremental read-side accelerator that is conformance-gated against the artifact** rather
than an independent authority. Revisit to Option 1 if EL++ completeness gaps bite.

**D3.1 Conformance-test contract (stops drift regardless of engine choice).** A CI gate computes
the closure with BOTH engines over a fixed corpus fixture and asserts
`whelk_inferred_triples ≡ reason_py_inferred_triples` (set-equality over `(s,p,o)` after IRI
canonicalisation, ADR-099 canonical-IRI rules). Divergence FAILS the build with a triple-level
diff. This is the same-authority guarantee made mechanical: the two can differ in *speed* and
*incrementality*, never in *output*. Class-count parity (the 8152-vs-5975 regression) is a
first-class assertion in this gate.

### D4 — The deferred distillation job pattern

Distillation is a deferred, content-addressed job that never holds a consumer turn open.

**D4.1 Job URN — a distinct kind, NOT a bead.** The shipped bead mint injects
`nonce: crypto.randomUUID()` (`local-sqlite.js:85,121`), which is the OPPOSITE of
content-addressed. So we mint a **distinct `job` URN kind** via the **agentbox ADR-013 §6 URN-kind
extension API** (`uris.js`): `urn:agentbox:job:<pubkey>:<sha256-12>`, `contentAddressed:true`,
`ownerScope:true` (KINDS entry + resolver case + contract test — do NOT overload the `bead` kind).
The distill **bead** (work-ledger) is minted normally (nonce-carrying) and *carries* the job URN
in a typed field; the job URN is the idempotency + provenance anchor. Resubmitting an identical
request resolves to the same job URN → **dedupe-on-create** at the submit tool.

**D4.2 Content-address identity CORE (RFC 8785 JCS).** Hash ONLY the identity core:

```json
{ "kind": "ontology.distill",
  "corpusSha": "<generation commitSha>",
  "scope": { "slugs": ["a","b","c"] },
  "budget_tokens": 4096 }
```

`budget_tokens` IS content (it changes the answer) — decided and documented. `scope` is one of
`{slugs sorted+deduped | domain | question-normalised}`; arrays are sorted where semantically
unordered (slugs), preserved where ordered. Canonicalisation is **RFC 8785 JCS** (named so two
implementations agree). **EXECUTION fields are NOT hashed:** `deadline`, `requester`, `sig`,
`result rendezvous`, `model_policy`. This keeps identity stable across retries with different
deadlines/keys while distinguishing genuinely different questions.

**D4.3 Job request envelope (ACSP kind 31408 for Phase-2 nostr; direct POST body Phase-1).**

```json
{
  "isEnvelopeKind": "ontology.distill",
  "jobUrn": "urn:agentbox:job:<pubkey>:<sha256-12>",
  "identityCore": { "kind":"ontology.distill", "corpusSha":"…", "scope":{"slugs":["…"]}, "budget_tokens":4096 },
  "question": "…natural-language question…",
  "deadline": "2026-08-11T14:30:00Z",
  "model_policy": "any-serving | pinned:<model-id>",
  "corpusSha_match": "exact | at_least | latest",
  "result_ref": { "namespace": "ontology-distilled", "key": "<jobUrn>", "content_sha": null },
  "requester_did": "did:nostr:<hex>",
  "sig": "<BIP-340 over JCS(identityCore ∥ execution-binding)>"
}
```

**D4.4 Provider-side lifecycle state machine** (HP `jobd`, stdlib pull-worker, WS-E):

```
submitted ──▶ admitted ──▶ queued ──▶ running ──▶ distilled ──▶ stamped ──▶ delivered ──▶ acked
     │            │            │          │
     │            │            │          └──(model id changes mid-job)──▶ preempted
     │            │            └──(depth≥cap ∨ won't-fit-deadline: not claimed)──▶ (harness reaper)──▶ expired
     │            └──(pinned model not loaded)──▶ waiting-for-model ──(max-wait)──▶ expired(cause=model-unavailable)
     └──(corpusSha_match=exact still mismatched after 1 refresh)──▶ expired(cause=corpus-unavailable)
```

Laws:
- **Concurrency 1** (ADR-113 §2.2, cited exact); the queue sits in FRONT of the model, never
  inside it.
- **Claim-time admission** replaces "429 past cap" (a pull worker has nobody to 429): `jobd`
  claims a job only when `depth < cap` AND estimated completion fits the deadline; otherwise it
  leaves the job unclaimed for the harness reaper.
- **GPU flock must be CREATED, not referenced:** ship `~/githubs/llm-server/.gpu.lock`, patch the
  bench harnesses + protocol to take it (shared for benches), `jobd` tries it non-blocking.
  Arbitration: **benches always win**; `jobd` never preempts/kills a bench; jobs blocked past
  deadline expire `cause=gpu-contended`.
- **Re-probe model identity per LLM call**; abort → `preempted` on mid-job change. Record model
  file path/metadata (guards a same-alias gguf swap; residual risk noted).
- **FAIL-LABELLED, not fail-open** (binding constraint): a `scaffold_engaged=false` output is
  NEVER delivered as ontology-grounded — it is quarantined with cause-split telemetry. (This is
  the delivery-payload rule; it composes with ADR-119's channel-level fail-open, D5/§Consequences.)
- `jobd` is **stateless-by-design**: queue durability = re-pull on restart; nothing is persisted
  on HP.

**D4.5 Corpus resolution on admit.** If `job.corpusSha != current Loom generation`, `jobd` runs ONE
on-demand mirror refresh (cron cadence irrelevant). `exact` + still-mismatched → terminal
`corpus-unavailable`. Every result envelope carries `corpus_generation_used` + a mismatch flag.

**D4.6 Result envelope schema** (extends the bench-row schema; signed):

```json
{
  "jobUrn": "urn:agentbox:job:<pubkey>:<sha256-12>",
  "summary": "…distilled prose…",
  "corpusSha_used": "…", "corpus_generation": "<buildId>",
  "model_id_probed": "…", "model_file_meta": { "path":"…", "sha256":"…", "bytes":0 },
  "toolkit_rev": "…", "llama_build": "…",
  "engaged_class_slugs": ["…"], "scaffold_engaged": true,
  "injected_tokens": 0, "tokens_used": 0, "latency_ms": 0,
  "derivation_labels": { "slug": "asserted | inferred | summary" },
  "corpusNature": "synthetic-ai-generated-human-directed",
  "generatedAt": "…", "lease_epoch": 7,
  "sig": "<BIP-340>"
}
```

`corpusNature` is mandatory (corpus-honesty constraint): the corpus is synthetic AI-generated,
human-directed, and every distillate says so. `scaffold_engaged`/`engaged_class_slugs` SHOULD be
bound to a retrieval-transcript hash the harness can reproduce (self-assertion hardening).

**D4.7 Consumer-side tools (WS-J, Phase-1, same weight as WS-E).** Add `ontology-bridge` MCP tools
so no agent hand-rolls the six steps:
- `ontology_distill_submit({scope, question, budget_tokens, deadline, model_policy, corpusSha_match})`
  → mints the job URN (via management-api `uris`), signs with the HARNESS machine key (individual
  agents never touch keys), creates distill+recombine beads, returns `{jobUrn, beadId}`. Dedupe-on-create.
- `ontology_distill_fetch({jobUrn})` → budget-clamped retrieval of the delivered, sig-verified
  result (fail-labelled on miss/unverified).
- `ontology_distill_await({jobUrn, deadline})` → deadline-bounded poll for mid-workflow use.

**No-synchronous-await LAW** (binding): distillation is submit-in-turn-N,
recombine-in-a-later-turn/worker; no consumer ever holds a turn open on a distill job;
`await` polls across the deadline and returns whatever landed (or a labelled timeout), it does NOT
hold a turn on the LLM; fast-wake only *schedules* the recombine worker. Two modes stated:
**fire-and-collect-later** (cross-session, default) and **deadline-bounded await** (mid-workflow).

### D5 — Delivery + consistency: two paths, CAS close, lease fencing, janitor + reaper, RuVector TTL law

Phase-1 delivery is **two paths, not five**: (a) RuVector payload + (c) bead close. Strict
ordering; **close is the linearisation point**; invariant **"closed-done ⇒ payload retrievable at
jobUrn."**

- **(a) RuVector payload.** The harness-side adapter (HP has no MCP) does
  `memory_store(key=jobUrn, namespace='ontology-distilled')` on receipt via the RuVector MCP tools
  (**RuVector MCP-only** — CLI/raw SQL bypass the embedding pipeline). Typed-metadata gate REQUIRED
  for this namespace. **RuVector TTL law:** `TTL ≥ consumer_deadline + lease_TTL × max_redeliveries
  + sweep_period + slack`, clock starts at delivery. **First-write-wins on content:** an existing
  key with a different result sha → reject + log divergence (NOT upsert).
- **(c) CAS bead close** (WS-C):
  `UPDATE … SET status='closed' WHERE id=? AND status='claimed' AND actor=? AND lease_epoch=?`
  returning changes-count; a failed CAS is a no-op.
- **Lease fencing** (WS-C): a monotonic `lease_epoch` is incremented on every claim/reclaim and
  carried in the envelope; **both sinks reject stale-epoch deliveries**. `claim()` is the
  conditional `UPDATE … WHERE actor IS NULL` (changes-count).
- **getReady is outcome-aware:** the recombine worker MUST read the blocker's `outcome` before
  dereferencing `result_ref` — `done` → fetch; `expired|failed` → propagate a labelled failure
  into search-only recombination.
- **Reconciliation janitor** (idempotent, harness-side, period P < min(lease_TTL, memory_TTL)/2):
  (1) claimed beads past `lease_TTL` → increment epoch, clear actor → re-eligible; (2) OPEN distill
  beads whose jobUrn key exists in RuVector → complete the tail from the stored signed envelope
  (courier-of-record: CAS-close `done`, ensure `:summary` write in Phase-2); (3) open beads past
  the consumer deadline with no payload → CAS-close `expired`; (4) divergence audit: RuVector
  entries missing from `:summary` re-posted before TTL. "Closed-done-with-no-payload" is
  structurally impossible once close-last + CAS holds.
- **Deadline reaper:** at the job deadline the reaper CAS-closes the distill bead `expired`
  (`cause: unclaimed | claimed-not-delivered`), unblocking recombine to proceed search-only. This
  is what makes "HP absence never blocks a turn" TRUE.

Phase-2 delivery paths (b) VisionClaw fenced `:summary` durable copy + provenance emit, (d)
agent-events fast-wake (strictly-after-close accelerator only), (e) kind-30840 operator digest.

### D6 — Provenance routing: portable-reification is a MAPPING target, not "canonical"

Neither provenance graph is canonical — both have binding owners: **VisionClaw PRD-022
(semantic-trust-layer) constraint 3** owns `urn:ngm:graph:provenance`; **agentbox ADR-049** owns
`urn:agentbox:graph:provenance`. This ADR specifies a **reconciliation MAPPING** (an alignment
vocabulary); each graph keeps its owner's invariants. Portable-reification is a mapping *target*,
never a new canonical store.

- The distillate's PROV-O triples route through the **VisionClaw PRD-022 ProvenanceEmitter**
  (ADR-127 D2) into `urn:ngm:graph:provenance` — NOT into `:summary`. PROV-O in `:summary` would
  violate PRD-022 constraint 3.
- `POST /api/ontology/derived` receives ONLY the summary quads (`:summary`/`:usage`), fenced at
  two layers (verified in code: `ontology_derived_handler.rs:30-40`). The `:provenance` graph is
  append-only and not reasoned over (ADR-127 D2.1/D2.5); the distillate activity is reified there
  as `prov:Activity` with `prov:wasDerivedFrom <repo@sha>` back to the generation.

### D7 — Security: verify signatures where trust is consumed

Binding posture — sig-verify at the derived door AND the RuVector read; provider door
strict-nip98; write-path-never-widened.

- **Verify the BIP-340 envelope sig at BOTH the `/derived` write door AND the RuVector read**
  (recombine), against a **distiller-provider allowlist** — NOT generic `power_user`. Generic
  `power_user` must NOT suffice to write `:summary`. Unsigned/unverifiable → 400 (write) /
  fail-labelled-quarantine (read); it never crosses `clampToBudget` (ADR-116).
- **Reconcile envelope fields** (`jobUrn/corpusSha/scaffold_engaged`) against the submitted job
  server-side; mismatch → reject.
- **Provider door = strict-nip98, drop Bearer.** The shared `MANAGEMENT_API_KEY` is a broadly held
  LAN dev secret with no replay defence; the provider door uses strict NIP-98 (bind claim lease to
  the claimant's verified pubkey; the delivering pubkey MUST equal the job's designated provider;
  bind the bead close to job identity). WS-D adds NEW verbs (pending-per-provider, claim,
  result-upload) — NOT the existing local-subprocess `/v1/tasks`.
- **HP key at rest:** age/agenix-encrypted or hardware-held (never a plaintext dotfile); a
  short-lived capability grant rather than long-lived `power_user` membership; a **per-request
  revocation list** (not env-at-boot) so a stolen key is cut without redeploy.
- **Release-build CI gate:** assert the LAN-facing VisionClaw binary is a release build (the
  `dev-session-token` → `power_user` path is debug/dev-auth only; a debug build on the LAN reopens
  the fence).
- **SPARQL SERVICE stays forbidden** (ADR-011/ADR-117): the Loom never federates via `SERVICE`;
  cross-consumer reach is the façade + (Phase-2) NIP-59-wrapped ACSP events.
- **Phase-2 nostr** (WS-I): allocate NEW ACSP kinds **31408 DistillJobRequest / 31409
  DistillJobResult** (leave 31406/31407 SPARQL `semantic_query` UNTOUCHED); job scope/question and
  the kind-30840 digest travel **NIP-59 gift-wrapped** to the provider key (a content-addressed URN
  hides nothing; plaintext scope would leak to the relay). Retire NIP-26 delegation (deprecated,
  unrevocable) → per-consumer NIP-59 capabilities (ADR-125 did:nostr).

### D8 — ADR-090 ring placement for any Rust surface

Any Rust the Loom adds inside VisionClaw honours the ADR-090 acyclic ring
(`contracts → domain → {gpu, ontology, protocol} → adapters → actors → server → webxr`):
- Generation ingest + shadow-graph swap (D2.3): `visionclaw-ontology` (build) + `visionclaw-adapters`
  (Oxigraph repository) — no dependency back toward `server`.
- `/loom/v1` façade client + the `/api/ontology/derived` landing handler (D6): `visionclaw-server`
  (handlers) calling inward to `visionclaw-adapters` (ProvenanceEmitter, RuVector adapter).
- Typed contracts for the generation descriptor, job envelope, and result envelope live in the
  leaf `visionclaw-contracts` crate (no workspace deps), so every ring above can share them.
The lifecycle+façade facet itself is out-of-process (stdlib-portable Loom node), so most Loom code
is NOT in the VisionClaw ring at all — only the thin consumer/landing surfaces are, and they stay
ring-legal.

---

## 3. How the Loom façade reconciles with ADR-112 (one brain / no hot-path LLM)

ADR-112's "one brain" is the in-process retrieval library on the hot path. The Loom does not
contradict it: **the Loom owns the SLOW/AUTHORITATIVE path** (lifecycle, reasoning, index
generation, distillation), and the fast in-process libraries become **thin clients of the Loom's
authoritative generation** rather than independent index builders. The hot path still has no LLM
and no network dependency for augmentation reads (it reads the published generation/index); only
`/loom/v1/distill` engages a model, and it is deferred, off-turn, and never synchronously awaited
(D4.7 no-synchronous-await LAW). One brain remains one brain; the Loom is where that brain's
authoritative state is minted.

---

## 4. Decision register (sibling implementation ADRs / workstreams)

| WS | Scope | Key decision | Phase |
|---|---|---|---|
| WS-A | logseq pipeline | build-manifest + sha-pinning + atomic mirror (generation identity) | 1 |
| WS-B | schemas | job envelope + result envelope JSON Schemas (page/scaffold/prose retro-schemas Phase 2) | 1 |
| WS-C | beads | `job` URN kind, typed `result_ref`, `lease_epoch` + claim-lease + reclaim-TTL, CAS close — schema migration + contract tests | 1 |
| WS-D | management-api | hp-ontology provider door: pending-per-provider / claim / result-upload; strict-nip98 | 1 |
| WS-E | HP `jobd` | stdlib pull-worker; CREATE the GPU flock; heartbeat | 1 |
| WS-F | VisionClaw | derived landing + ProvenanceEmitter routing + NotifyGraphUpdated wake | 2 |
| WS-G | liveness | heartbeat + periodic cap-exempt canary + reaper + janitor (ADR-119) | 1 |
| WS-H | elevation | EnrichmentCandidate → propose spine → corpus (ACSP 31402/31403) | 2 |
| WS-I | nostr federation | NEW kinds 31408/31409, NIP-59 wrapped | 2 |
| WS-J | consumer MCP | `ontology_distill_submit/fetch/await` + recombine ownership | 1 |

---

## 5. Consequences

### Positive
- **The capstone loop closes.** An agent calls `ontology_distill_submit` mid-workflow, keeps
  working, and a later turn (or a bounded `ontology_distill_await`) `fetch`es a signed, sha-pinned,
  budget-clamped distilled summary to recombine with search. Provenance is provable from the signed
  envelope + the build-manifest sha.
- **One authority each.** Two reasoners → one (D3, conformance-gated); three retrieval impls → one
  index generation with thin clients; two parsers → one (Loom owns sync); two conflict checkers →
  composed (Whelk consistency = pre-assert gate, `conflicts.py` = pre-publish gate). The
  8152-vs-5975 drift class is closed by the D3.1 gate.
- **Model swaps carry no debt.** `DISTILL_BACKEND_URL` + result-stamped model identity means
  Gemma → Muse-Glimmer → next changes nothing consumers bind to.
- **The engine stops playing librarian.** VisionClaw sheds corpus-lifecycle management and becomes
  a clean generation consumer + viz/physics/propose door.

### What breaks (deliberately)
- **`github_sync_service` CLEAR+INSERT is retired.** Any operator runbook, cron, or dashboard that
  triggers `force_full` sync must be repointed to Loom generation-load. VisionClaw ADR-050
  (pod-backed-kgnode)'s inverse-corpus-path workaround becomes vestigial once the shadow-graph swap
  (D2.3) lands — track its removal.
- **`rebuild_assert_graph` is replaced** by atomic load-generation-N; downstream code that assumed
  a wiped-then-rebuilt `:assert` graph must tolerate additive-then-rename semantics.
- **Direct-to-target:** no dual code path is kept alive; this is a DEV/TEST estate, so we cut over
  and debug the integrated system rather than run a phased live migration.

### Liveness harness (ADR-119)
- `jobd` **heartbeats every poll** (short-TTL RuVector key / management-api provider-status);
  WS-G liveness = heartbeat-staleness threshold (kills the green-but-zero / ".48-is-dead" class).
- A **periodic cap-exempt canary** with a harness-side landing deadline (submitted at T, not landed
  by T+deadline → alert); `canary:true` lands in `ontology-canary` ns, never writes `:summary`,
  never digests, excluded from elevation + queue-depth metrics. Boot completion is NOT liveness.
- `jobd.service` ships `Restart=on-failure`; the runbook makes `systemctl enable --now jobd` an
  explicit operator step with a verify command.
- **Fail-open channel vs fail-labelled payload** (reconciling ADR-119 with D4.4): the augmentation
  *channel* fails open (a missing/late distillate never blocks a turn — recombine proceeds
  search-only via the deadline reaper); the distillate *payload* fails labelled (a
  `scaffold_engaged=false` result is quarantined, never delivered as grounded). Both hold together.

### Negative
- New moving parts: a portable Loom node, a GPU flock convention, a provider door, and a janitor.
  Mitigated by the WS-G liveness harness and the direct-to-target debug posture.
- The cloud read-replica can serve a stale generation while the Loom host is down; acceptable
  (corpus changes slowly; only fresh distillation pauses).

### Neutral
- The `:provenance` graph is not reasoned over (ADR-127); the reconciliation mapping (D6) keeps
  both owners' invariants without declaring a winner.

---

## 6. Alternatives considered

### A1 — Keep the intelligence scattered (status quo)
Leave two reasoners, three retrieval impls, two parsers, two conflict checkers in place.
**Rejected:** this is exactly what produced the 8152-vs-5975 drift and the stale `:assert` graph.
No stable façade means every model or host change is a cross-repo migration.

### A2 — One-brain only (fold everything into agentbox `ontology-retrieval`, no node)
Push all consolidation into the ADR-112 in-process library and skip the Loom node.
**Rejected:** the in-process library is a hot-path READ brain and must stay LLM-free and
network-free (ADR-112). Corpus lifecycle (GitHub sync, generations, EL closure, GPU distillation,
GitHub write-back) is slow and authoritative — it cannot live on the hot path. The Loom owns the
slow/authoritative path; one-brain stays the fast client (§3). They are complementary, not
alternatives.

### A3 — A new always-on HTTP microservice for distillation
Stand up a generic HTTP distillation service consumers call synchronously.
**Rejected by ADR-112** (no LLM/network on the hot path) and by the no-synchronous-await LAW
(D4.7): a synchronous LLM service would let a consumer hold a turn open on a 10s–10min job. The
Loom's façade is deliberately submit/collect (deferred), and the distillation facet is a swappable
backend URL, not a bespoke always-on service. The façade is reconciled with ADR-112 in §3: Loom
owns slow/authoritative, in-process libs stay hot-path clients.

### A4 — Staged live migration (phased cutover with dual paths)
Run VisionClaw CLEAR+INSERT and Loom generation-load side by side behind a flag, migrate
gradually.
**Rejected (operator decision 2026-08-11):** this is a DEV/TEST estate, not a live system. A
phased migration would only add transitional shims, dual code paths, and confusion. Build the
target end state directly and debug the integrated system from there. All adversarial must-fixes
(sig-verify, CAS-close, lease fencing, janitor, atomic-mirror corpusSha, strict-nip98, distiller
allowlist, release-build CI gate, no-synchronous-await LAW) are correctness/security — they apply
to the END STATE and are NOT waived by going direct.

### A5 — Reuse ACSP kinds 31406/31407 for the distill channel
Extend the existing SPARQL `semantic_query` kinds for distillation events.
**Rejected:** distillation is a different envelope (`ontology.distill`) with a different security
profile (NIP-59 gift-wrap to a designated provider, distiller allowlist). Overloading
31406/31407 would blur the SPARQL federation contract (ADR-127 D3) with the deferred-job contract.
Allocate NEW **31408 DistillJobRequest / 31409 DistillJobResult** and a new IS-Envelope kind
`ontology.distill` via ADR-075 D1.

---

## 7. Verification (liveness proofs)

| Decision | Verification |
|---|---|
| D1.1 façade contract | `GET /loom/v1/generation` on Deployment A and Deployment B returns byte-identical descriptor shape for the same `commitSha` |
| D1.2 model-swap seam | swap `DISTILL_BACKEND_URL` (Gemma→other); a fresh distill result stamps the new `model_id_probed`; NO consumer code changes |
| D2.3 CLEAR+INSERT retired | grep shows `github_sync_service` `force_full` path removed; a decision-class triple present before a generation load is still present after (shadow-swap, not wiped) |
| D3.1 conformance gate | CI computes closure with both engines over the fixture; set-equality holds; a seeded divergence FAILS with a triple diff; class-count parity asserted |
| D4.1 job URN kind | `uris.js` resolves `urn:agentbox:job:<pubkey>:<sha256-12>`; resolver contract test passes; identical resubmit → same jobUrn (dedupe-on-create) |
| D4.2 JCS identity | two independent impls hash the same identity core to the same sha256-12; changing `budget_tokens` changes the URN; changing `deadline` does NOT |
| D5 CAS + fencing | a stale-`lease_epoch` delivery is rejected at both sinks; "closed-done ⇒ payload retrievable at jobUrn" holds under a fault-injection reclaim; reaper CAS-closes an unclaimed job `expired` and recombine proceeds search-only |
| D6 fence | `POST /api/ontology/derived` with a PROV-O quad in the `:summary` payload is rejected (two-layer fence, `ontology_derived_handler.rs:30-40`); PROV-O lands only in `urn:ngm:graph:provenance` |
| D7 security | unsigned/allowlist-missing envelope → 400 at write door and fail-labelled-quarantine at RuVector read; strict-nip98 provider door rejects a Bearer `MANAGEMENT_API_KEY`; release-build CI gate fails a debug LAN binary |
| D8 ring | `cargo tree` shows no cycle; the `/loom/v1` client + `/derived` handler depend inward only (server→adapters→…), never outward |
| WS-G liveness | heartbeat staleness past threshold alerts even while `/loom/v1/healthz` returns 200 (green-but-zero caught); canary landing-deadline miss alerts; `systemctl enable --now jobd` verified in the runbook |
