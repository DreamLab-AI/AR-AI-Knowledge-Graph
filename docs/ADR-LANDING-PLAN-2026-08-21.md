# ADR Landing Plan — Corrected Against Verification Mesh (2026-08-21)

Source: 39 verified ADR survey verdicts (VisionClaw `docs/adr` + agentbox `docs/reference/adr`), each with evidence citations, plus a completeness-critic coverage list. Every claim below is traceable to that evidence. UK English.

---

## 1. Survey accuracy scorecard

| Verdict (on the survey claim) | Count |
|---|---|
| CONFIRMED (survey accurate) | 19 |
| PARTIALLY_WRONG | 12 |
| WRONG | 8 |
| **Total** | **39** |

CONFIRMED set: ADR-016, 031, 038, 048, 056, 059, 060, 064, 065, 066, 067, 070, 072, 111, 121, 122, 123, 126, 127.

### 1.1 Every WRONG verdict (survey was wrong — corrected classification + evidence)

The recurring WRONG pattern: the survey called work "partially completable / pending" or "blocked" when the code is **already fully implemented and committed**; only the ADR's own status line lags.

| ADR | Survey said | Corrected classification | Anchor evidence |
|---|---|---|---|
| **ADR-020** (aci-mcp tree-search) | Surface 2 blocked behind ADR-018 acceptance gate | Gate is **already green** (ADR-018 Accepted, `mcp/code-interpreter/server.py` ships, `code_interpreter.enabled=true`). Surface 2 is *enabled-in-manifest-but-unbuilt*: `agentbox.toml:621-628` flips `tree_search_coder.enabled=true` yet `skills/tree-search-coder/` does not exist. | `agentbox.toml:535-539,621-628`; `ADR-018:3`; `ls skills/tree-search-coder/` → absent |
| **ADR-026** (cross-substrate seams) | Blocked | Not blocked — WS5-D1 render + WS7 ACSP-31402 dispatcher are built in the VisionClaw tree; the ADR's OPEN status line is stale. 96 `urn:agentbox` refs vs ADR's claimed zero. | `src/services/acsp/events.rs:20`; `src/actors/agent_beam_actor.rs:68`; `src/agent_events/ingest.rs:325`; git `e7771eb90`,`43a12e401` |
| **ADR-037** (gap-close agentbox) | Partially completable pending work | Substantially implemented: D1–D8 all have landed committed code; only the ADR status line reads "proposed". | `management-api/lib/{failure-taxonomy,authority,uris}.js`; `entrypoint-unified.sh:608-629`; `routes/voice-intent.js:97-290`; `scripts/skill-count-check.js` (d13f8688f) |
| **ADR-042** (AoE interaction plane) | Partially completable | D1–D3 + D6 fully implemented and committed (`52ab1afa1`); remainder is operational rebuild + status flip. | `flake.nix:18,146`; `agentbox.toml:1131-1182`; `config/harness-wrappers/openrouter.sh:88-114` |
| **ADR-043** (session identity binding) | Blocked on two rebuild-class flips | Both flips are **done and committed**: `beads="local-sqlite"` (`agentbox.toml:18`), `admin_access_mode="scoped"` (`agentbox.toml:347`); D4.x wiring exists. | `agentbox.toml:12-18,347`; `management-api/routes/{beads,mandate}.js`; `config/nip98-proxy/proxy.mjs` |
| **ADR-049** (bi-temporal + PROV-O) | Blocked on Whelk | Not blocked — Whelk live per the ADR's own 2026-08-10 note; ADR is unblocked-but-unbuilt (data model absent). | `ADR-049:17-23`; `mcp/servers/lib/ontology-retrieval.js:301`; grep `dl:validFrom`/`state_at` → absent |
| **ADR-050** (decision elevation) | Partially completable (write-half + PR) | Fully implemented — write-half **and** read-half committed (`e7771eb90`); ships default-OFF. | `src/actors/decision_elevation_actor.rs:1-12,415-427`; `src/services/github_sync_service.rs:995,1018-1064`; `src/main.rs:523-545` |
| **ADR-052** (dream-machine annexe) | Partially completable | v1 built, gated `enabled=true`, supervised and **demonstrably running** (ledger rows + witnesses through 2026-08-18). | `agentbox.toml:1497-1502`; `flake.nix:1887-1896`; `services/dream-engine/src` (ed230fb05); `docs/dream-cycle/LEDGER.md:3-7` |

### 1.2 Every PARTIALLY_WRONG verdict (correction + evidence)

| ADR | Correction to the survey | Anchor evidence |
|---|---|---|
| **ADR-017** (multi-tenant pods) | Accepted-and-partially-realised, not "Blocked": per-user NIP-98 signing + `/admin/users/provision` shipped behind `enabled=false`; only suspend/archive (501) + federation open. | `management-api/lib/per-user-agent.js:134`; `routes/admin-users.js:137-189,240-258`; `server.js:1014-1024` |
| **ADR-044** (voice-plane repoint) | D1–D7 already committed (`52ab1afa1`); a multi-file code change, not "config over surfaces"; D8 auth posture as shipped (direct loopback) **deviates** from the ADR's NIP-98-signer default. | `config/tab0-bridge/server.mjs:147-160,207-238`; `agentbox.toml:1192-1194`; git `52ab1afa1`,`923f1e848` |
| **ADR-045** (sovereign ingress) | agentbox side (multi-upstream ingress, :9096 publish, NIP-07, shared-npub allowlist) already shipped; residual forum link-through is **external repo** work, not agent-completable here. | `config/nip98-proxy/proxy.mjs:55-123,411-572`; `docker-compose.yml:49-54`; `CHANGELOG.md:17`; `README.md:50` |
| **ADR-046** (semantica complement) | Caps 2-4 now gated **only** on corpus/store drift; the Whelk-live / VisionClaw-up gate was satisfied and explicitly lifted 2026-08-10 — "Whelk live still needed" is stale. | `ADR-046:29-34,74-79,115-117` |
| **ADR-047** (tenant integration boundary) | Rules largely coded already; the residual is a **human architectural accept** (proposed→accepted) + rule 7's standing ADR-amendment gate, not agent code. | `ADR-047:4,47-72,19-24`; `management-api/lib/bc20-provenance-bridge.js`; `tests/contract/governance-flow.spec.js` |
| **ADR-051** (Loom client harness side) | Unbuilt; **all** D1–D7 are client-side (survey's "D1/D2/D4/D7 subset" is a mischaracterisation). Most self-contained agent slice is **D3** (survey omitted it); D7 depends on remote HP jobd. | `ADR-051:23-30,467-485`; grep `ontology_distill_submit`/`lease_epoch` → 0 hits; `management-api/lib/uris.js:87,287` |
| **ADR-054** (ontology-bridge findings) | "Three agent-completable fixes" is wrong: Defect 1 is cross-repo (VisionClaw handler), Defect 2 root cause **undiagnosed**, Defect 3 needs a **manual relay-allowlist step** + governance decision. | `ADR-054:33-43,57-63`; `mcp/servers/ontology-bridge.js:60,308-371`; git `5100e8813` (doc-only) |
| **ADR-055** (dream cockpit panel) | Read-only nature-claim accurate, but classification as pending work is stale — **fully implemented, tested, committed** (`598b7248f`); residual is a redeploy only. | `management-api/routes/dream.js`; `lib/dream-ledger.js`; `voice/console/site/dream.html`; `tests/integration/dream.test.js`; git `598b7248f` |
| **ADR-064** (typed graph schema) | Worse than "unresolved work remains": **zero implementation**, and the ADR's own "graph-cognition-core … 25 tests passing" line is fabricated. Named crate/enums/mint/validator/migration all absent. | `ADR-064:5`; `ls crates/` (no `graph-cognition-core`); grep `EdgeKind`/`SchemaValidatorActor` → nothing; `src/uri/` has only `mod.rs` |
| **ADR-068** (logseq block fidelity) | Effectively not-started; "Implementing / 15 tests passing" is fictional; target crate is a hollow directory. | `ADR-068:3,4`; `find crates/graph-cognition-extract -type f` → only `pending-insights.jsonl`; grep `LogseqBlockParser` → nothing |
| **ADR-069** (force-preset system) | Nominally "Implementing" but effectively unstarted; "21 tests passing" false, kernel path mis-cited; only pre-existing plumbing exists. | `ADR-069:5`; `find *preset*.toml` → none; grep `ForcePreset` → 0; `semantic_type_registry.rs` (hardcoded, not data-driven) |
| **ADR-135** (Ontology Loom node) | Not a doc-only status flip: Loom façade + Qwen3.8 deployed **externally**, but the keystone D2.3 corpus re-home is unbuilt (`force_full` CLEAR+INSERT still active) and WS-C/D/E/J machinery has no code here. | `ADR-135:521`; `src/services/github_sync_service.rs:307-313,558-572`; grep `loom/v1`/`lease_epoch` → 0; git `33da8db50` |

### 1.3 Disputed verdicts (first pass vs final) — reconciled

Six verdicts were revised on second pass; the plan below uses the **final** verdict.

- Revised toward *survey-accurate*: ADR-064 (PARTIALLY_WRONG→CONFIRMED — the survey's "unstarted-with-phantom-line" is upheld), ADR-016, ADR-056, ADR-048 (each WRONG/PARTIALLY_WRONG first pass → CONFIRMED, i.e. the correction that the work is already fully landed stands).
- Held: ADR-042, ADR-050, ADR-026 (WRONG confirmed WRONG); ADR-017, ADR-046, ADR-047, ADR-055 (PARTIALLY_WRONG confirmed).

---

## 2. Corrected ADR landing plan

### (a) Agent-completable now — ordered by effort (ascending)

| # | ADR | Deliverable | Files touched |
|---|---|---|---|
| 1 | **ADR-111** | Flip status line `Proposed → Accepted (implemented 2026-07-22)`; optionally fold in the receipt's deferral caveats. No code paths depend on it. | `docs/adr/ADR-111-ecosystem-infographic-modernisation.md:3` |
| 2 | **ADR-026** | Update the stale OPEN status line + Seam-E prose to record WS5-D1 render, `urn:agentbox` BC20 ingest, ACSP 31400–31404, the 31402 dispatcher, and the `/wss/agent-events` consume side as built. | `agentbox/docs/reference/adr/ADR-026-cross-substrate-agent-loop-seams.md:4,52-59` |
| 3 | **ADR-020** | Author the missing skill and fix the incoherent manifest (enabled against a non-existent skill). SKILL.md needs when-to-choose, the 7-step algorithm, negative-routing vs `sparc:coder`/`build-with-quality`/Edit, and 3 ICL exemplars. Dependency (ADR-018 kernel) already satisfied. | `agentbox/skills/tree-search-coder/SKILL.md` (new); confirm `agentbox.toml:621-628` |
| 4 | **ADR-038** | Land the trial setup: build/fetch the aict binary to a scratch/overlay path (not in the Nix image), append an optional `aict` stanza to the workspace `.mcp.json` (idempotent writer leaves existing multi-server file intact), run token/accuracy comparison on 2-3 tasks. | `${WORKSPACE}/.mcp.json`; scratch path; ref `entrypoint-unified.sh:748` |
| 5 | **ADR-060** | Add the env-gated (default-OFF) drop-set filter: compute `private_opaque_ids` per caller, drop matching `(id,data)` before `encode_node_data_with_live_analytics_and_privacy`. ~80 lines per ADR-059 §Phase-4. | `src/handlers/socket_flow_handler/position_updates.rs` |
| 6 | **ADR-031** | Convert the already-written D7 correctness suite into an actual CI gate: split the CPU-reference known-answer tests (two_clique/pagerank/dbscan/lof) into a CUDA-free test target so the ubuntu `CPU_CRATES` job runs them; refresh the stale `gpu_lof` stub whose panic claims the LOF kernel is unfixed (it is fixed at `gpu_clustering_kernels.cu:489-493`). | `.github/workflows/ci.yml`; `tests/analytics_correctness_test.rs:292`; `tests/analytics_fixtures.rs`; `Cargo.toml` |
| 7 | **ADR-070** | Implement P1 D2.2 (constraint-force stability third criterion), D2.3 (input-edge NaN guard), and P2 sparse compute mask (largest open gap; drives Epic E.4 persona masking). One minor human confirm outstanding (D1.4 cap location — see tier c). | `crates/visionclaw-gpu` CUDA sources; `src/actors/gpu/` |
| 8 | **ADR-114** | Build the RuVector seed leg (Oxigraph half already wired via `ontology_derived_handler.rs`): condensation job (ADR-113) writing per-class summaries via `memory_store` to namespace `ontology-classes` (bge-small 384-dim HNSW, never raw SQL); `ClassSummaryIndexRefreshed{changed_count}` refresh trigger on GitHubSync; liveness canary `memory_search` (ADR-119). | condensation job; GitHubSync hook; ADR-119 self-test |
| 9 | **ADR-059** | Two already-decided follow-ons (no fresh human decision): (1) gluon force — `UpsertTransientEdge{src,tgt,weight,ttl_ms}` message + a separate transient-edge GPU buffer the spring kernel sums alongside static CSR + TTL sweep; (2) `:9500` cutover — carry agent-state snapshots over the WS contract, then retire the poll. | `agent_beam_actor.rs:355`; `unified_gpu_compute` CSR paths; spring kernel; `bots_client.rs` |
| 10 | **ADR-072** | Build the five named modules: `embedding_service.rs` (384-dim MiniLM via HTTP to 192.168.2.132:9997), `nhop_materializer.rs` (**note: cited Neo4j path is stale post-ADR-132 — retarget Oxigraph**), `kge_trainer.rs` (pure-Rust TransE 128-dim), `discovery_handler.rs`, `edge_type_physics.rs`. Needs live deps for integration verification. | `src/services/{embedding_service,nhop_materializer,kge_trainer,edge_type_physics}.rs`; `src/handlers/discovery_handler.rs` |
| 11 | **ADR-049** | Build the v1 data model (Whelk dependency already satisfied): a `urn:agentbox:graph:provenance` named graph, content-addressed `prov:Entity` versions with `dl:validFrom/validTo` + PROV-O relations, an authenticated idempotent write transaction (write-ahead-intent + deterministic recovery for non-atomic cross-graph writes), plus golden/failure-injection temporal test suites. RDF 1.2 quoted-triple upgrade is explicitly out of v1. | new provenance write-service; temporal test suites |

### (b) Partially completable — the exact completable slice, plus the blocked remainder

| ADR | Agent-completable slice **now** | Blocked remainder (see tier c for the question) |
|---|---|---|
| **ADR-064** | Correct the Status line and **delete the fabricated Implementation line** (phantom `graph-cognition-core`, "25 tests passing"). | Full crate build (21 NodeKind/35 EdgeKind/8 EdgeCategory, `mint.rs::mint_typed_concept`, phf `NODE_TYPE_ALIASES`, `SchemaValidatorActor`, `kind_id` side-table, migration) gated on a human store-migration redesign (D5 Cypher is stale post-ADR-132). |
| **ADR-068** | Correct the false "Implementing / 15 tests passing" status line. | Full crate build (stack-machine `LogseqBlockParser`, `BlockNode`, did:nostr URN, caps, parity fixtures) + human decision: reset to Proposed vs supersede, since the Neo4j D4/D8 store design is obsolete. |
| **ADR-069** | Downgrade Status, delete the fictional "21 tests passing" line, fix the mis-cited kernel path. | ~3–4 week presets build (`graph-cognition-physics-presets` crate, 5 presets + `edge-kinds.toml`, data-driven registry, SimParams validation/NaN guard/ease-in, D11 temporal-Z soft-spring). D3 calibration acceptance is human. |
| **ADR-121** | Build the W1/W2 plumbing: usage-telemetry extractor generalising `kg-proposal-extractor.js`, a durable `EnrichmentProposal` store replacing the `writeback_triggered` flag, autonomous propose + post-merge refresh; pass §6 tests. W0 already shipped. | Activation (default-off `[sovereign_mesh].ontology_self_improvement`) gated on PRD-020 WS-11 sign-off + ADR-112/114 acceptance. |
| **ADR-123** | Extend `SwarmIntent` with governance intents (ReviewBacklog/ApproveProposal/RejectProposal/AmendProposal/ExplainProposal) in `crates/visionclaw-actors/src/voice_commands.rs`, wire spoken summarisation, connect to the existing `broker_inbox_handler.rs`. | did:nostr authority binding + channel-tagged signed-decision parity (security-sensitive) need human sign-off; depends on ADR-121's durable proposal store. |
| **ADR-051** | **D3 in full** (only no-human, fully-local workstream): migration 002 + a `job` URN kind in `uris.js`/`uri-resolver.js` + CAS claim/close/reclaim + contract tests. | D2 tools scaffoldable but blocked on operator key-custody (decision #4) + VisionClaw provider door; D1/D4/D7 blocked on external Loom generation / WS-D door / HP jobd heartbeat. |
| **ADR-052** | Flip the stale status line; build §4 sovereign mirrors (verdict bead under a new did:nostr key, git-mark ledger mirror, JSON-LD evidence bundle). v1 already running. | Qwen3.8 agentic tool-traversal quality benchmark — a human verdict-quality significance-bar judgement. |
| **ADR-135** | Retire the `force_full` CLEAR+INSERT rebuild path (D2.3) in `src/services/github_sync_service.rs:307-313,558-572`; update the Status line to per-workstream state (WS-F derived fence done). | Cross-repo WS-A/B/C/D/E/G/J (agentbox beads, HP jobd, management-api, logseq pipeline) not in this checkout; operator sign-off on the corpus-lifecycle re-home (cross-repo blast radius). |
| **ADR-044** | Flip status to Accepted; reconcile D8 by **amending it to record the shipped loopback-behind-proxy choice**, or wire the NIP-98 signer. | The D8 auth posture (which token/route fronts `:9095`) is a deploy-time operator decision. |
| **ADR-046** | Run the data-load: load `ontology-output.ttl` into `urn:ngm:graph:ontology:assert` to collapse the 8,152-vs-~5,975 class divergence. Needs live VisionClaw/Oxigraph (not verifiable from this checkout). | Author's acceptance decision to flip ADR proposed→accepted. |
| **ADR-043** | Runtime-verify the two boot-class consumers fire end-to-end (D4.7 `awaitDecision` authority gate, D4.2 session-create URN shim); confirm the flake actually bakes `MEMORY_ADMIN_ACCESS_MODE=scoped` + the beads adapter. | Human sign-off flipping front-matter `Proposed → Accepted` (author confirms the seven mechanisms are practised, not just wired). |
| **ADR-048** | Run the pre-merge gate: an OWL 2 EL profile checker + an executed Whelk capability test on the full imported ontology; record the result. Feature is otherwise implemented end-to-end (routes, service, Whelk gate, TTL, contract tests). | Human accept (author Dr John O'Hare) once conformance passes. |
| **ADR-054** | Defect 3 code: emit kind-31402 from the proposal spine via the elevation publisher. | Defect 1 lives in the VisionClaw ontology-agent handler (cross-repo); Defect 2 root cause undiagnosed (needs investigation of the remote discover endpoint); Defect 3 also needs a **manual relay D1-allowlist key add** + the human "decide" governance route. |

### (c) Blocked — the specific question the operator must answer

| ADR | Answerable operator question |
|---|---|
| **ADR-037** | Do you accept ADR-037 (proposed→accepted) and sign off the canon-owned wave/owner/maturity tiers plus the eight canaries + one CI gate (D8) against the cross-repo VisionClaw DriftCounter harness (ADR-004 Decision 7)? |
| **ADR-042** | Do you mark ADR-042 Accepted and authorise the rebuild-class apply (`./agentbox.sh rebuild`) so the `[interaction_plane]` gate + `aoe-serve`/`nip98-proxy` daemon blocks take effect? (Code already committed.) |
| **ADR-050** | Do you flip ADR-050 to Accepted and enable the default-OFF live write path — set `DECISION_ELEVATION_ENABLED=1` + `FORUM_RELAY_URL` + `ACSP_PANEL_NOSTR_PRIVKEY` + `LOGSEQ_PRIVATE_REPO_GITHUB`? |
| **ADR-056** | Do you grant the security sign-off to build the Phase-2 governed write path (`POST /dream/decide`, NIP-98-signed witnessed decision record)? (Phase 1 is shipped and Accepted.) |
| **ADR-047** | Do you ratify the ADR-047 integration boundary (proposed→accepted), amending ADR-046 and driving ADR-048/049, and accept rule 7's standing ADR-amendment + licence/SBOM gate for any future external dependency? |
| **ADR-045** | Will the external `nostr-rust-forum` repo ship its nostr user gate + a login link-through to the cockpit origin (`:8444`/`:9096`), and who owns that forum surface? (agentbox side already complete.) |
| **ADR-017** | Do you enable `[sovereign_mesh.multi_user]` and set `provisioning_policy` for go-live — and can `solid-pod-rs` alpha.12 (`--provision-keys` + git auto-init) / alpha.15 (suspend/archive) be scheduled? |
| **ADR-065** | Do you schedule the Rust code-analysis pipeline as a funded workstream (six crates: sandboxed tree-sitter subprocess, LLM sanitiser, `vc analyze` CLI, Ollama/VRAM infra, red-team golden corpus), or mark it Superseded/Rejected? |
| **ADR-066** | Do you prioritise the pod-federation build, how is signing-key custody resolved (prerequisite ADR-067), and who closes the `solid-rs` WebID-OIDC feature gap? |
| **ADR-067** | Do you schedule the ontobricks boundary crate (TLS-pinning, inferred-triple quarantine, egress allowlist), which client library do you pin, and does the AGPL/GPL legal review (PRD Q-11, D8) pass? |
| **ADR-122** | Do you unfreeze ADR-121 and ADR-122 together, and what is the initial L2 volatile-predicate allowlist (which the ADR states grows only by human decision, never by the loop)? |
| **ADR-126** | Do you ratify the strangler-fig OMB adoption posture (Strategy B / PRD-021, currently a non-canonical Draft) — the prerequisite for any R2/R3 additive work (RMAP manifest, glTF export, did:nostr)? |

### (d) Already landed — no work owed (status hygiene aside)

- **ADR-016** — Accepted; all 8 manifest license fields + `NOTICE` shipped. Nit: `mcp/ruvnet-brain/package.json` still declares MIT — treat as third-party or reconcile the verification wording.
- **ADR-055** — Implemented, tested, committed (`598b7248f`); only residual is the next voice-stack redeploy (operational, not a decision).
- **ADR-127** — WS-0/1/2/5 shipped (SHACL shapes + PROV-O emitter); WS-3/4 (relay-mediated SPARQL federation, kinds 31406/31407, `ontology_federate`) deferred **by design**. Future close needs a human federation-boundary decision (`FEDERATION_AUTHORIZED_PEERS`); nothing owed now.

---

## 3. ADRs the survey missed entirely

The completeness critic found 28 uncovered ADRs. None were verified; all are Proposed or frozen-Deferred, so none carry landable-now agent work — but they belong in the next survey pass.

**VisionClaw `docs/adr` — Proposed (uncovered):** ADR-033 (git-bead-provenance), ADR-075 (is-envelope-message-contract), ADR-076 (nostr-core-absorption), ADR-077 (ecosystem-qe-policy), ADR-087 (rate-limit-consolidation), ADR-088 (auth-service-extraction), ADR-091 (fixture-sync-enforcement), ADR-104 (shared-math-utilities), ADR-130 (gap-close-visionclaw-decisions).

**VisionClaw `docs/adr` — Deferred/frozen by closeout 2026-07-03 (uncovered):** ADR-057 (contributor-enablement-platform), ADR-073 (private-nostr-relay-mesh-topology), ADR-078 (cross-substrate-library-convergence), ADR-079 (forum-setup-skill-provider-abstraction), ADR-080 (forum-kit-deployment-topology), ADR-081 (federation-key-custody-rotation), ADR-082 (cross-substrate-test-fixture-sharing), ADR-083 (dreamlab-ai-website-cutover), ADR-084 (cloud-infra-mapping-for-kit-consumers), ADR-085 (forum-config-package-architecture), ADR-092 (android-nostr-client-and-signer), ADR-093 (mobile-bridge-messaging-substrate), ADR-094 (admin-pubkey-permission-delegation), ADR-095 (session-summary-event-scheme), ADR-096 (solid-pod-persistence-boundary), ADR-097 (mobile-bridge-relay-topology).

**agentbox `docs/reference/adr` — Proposed (uncovered; numbering collision with VisionClaw):** ADR-057 (replayable-agent-execution-journal), ADR-058 (lifecycle-scoped-capability-composition), ADR-059 (monotonic-agent-action-policy-pipeline). Note: covered "057/059" refer to the VisionClaw ADRs; these agentbox namesakes are distinct and unverified.

---

## 4. Recommended next mesh — 5 best-value agent-completable items to execute first

Selected for: satisfied dependencies, enumerated files, no human decision on the deliverable, and real capability delivered (not pure doc hygiene). The two trivial doc corrections (ADR-111, ADR-026) are near-zero-effort freebies to bundle alongside.

1. **ADR-020 — write `skills/tree-search-coder/SKILL.md`.** Closes an actively-incoherent manifest state (`tree_search_coder.enabled=true` against a non-existent skill dir, `agentbox.toml:621-628`); the hard dependency (ADR-018 kernel, `code_interpreter.enabled=true`) is already green, and the ADR fully specifies the deliverable (7-step algorithm + negative-routing + 3 exemplars). Highest value-per-effort.

2. **ADR-031 — CUDA-free CI test target.** Turns an already-written correctness suite (`tests/analytics_correctness_test.rs`) into a real gate — it currently gates nothing because it sits in the CUDA-linking root crate excluded from `CPU_CRATES`. Low effort, closes the D7 CI gap, and refreshes a stale panic message that falsely claims the LOF kernel is unfixed. Files fully enumerated.

3. **ADR-060 — pubkey-visibility drop-set filter (~80 lines).** Tightly specified (ADR-059 §Phase-4), single file (`position_updates.rs`), ships default-OFF so it is safe to land without release sign-off, and delivers a concrete privacy feature. The only human gate (promote-to-default) is downstream of merge.

4. **ADR-070 — P1/P2 GPU hardening (D2.2, D2.3, sparse compute mask).** No human decision blocks these three; the sparse-compute mask is the largest open gap in the CUDA pipeline and unblocks Epic E.4 persona masking. Builds on already-landed P0 launch-safety code, so it is incremental rather than greenfield.

5. **ADR-114 — RuVector ontology-class seed leg.** The Oxigraph half is already built and wired (`ontology_derived_handler.rs` + routes in `main.rs`), so this completes a half-finished substrate: a condensation job writing per-class summaries via `memory_store` to namespace `ontology-classes`, a GitHubSync refresh trigger, and a liveness canary. No fresh human decision; delivers the semantic seed index the rest of PRD-020 depends on.

Deliberately excluded from the top 5 despite being agent-completable: **ADR-072** and **ADR-049** (both large greenfield builds needing live-dependency integration verification — schedule after the quick wins), and the **status-flip-only** WRONG-verdict ADRs (037/042/050) whose acceptance is a human governance call, not agent code.
