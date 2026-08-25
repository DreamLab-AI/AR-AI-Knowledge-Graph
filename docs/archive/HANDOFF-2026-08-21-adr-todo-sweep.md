# Session Handoff — ADR/TODO Sweep + XR Close-Out Completion (2026-08-21)

> **Historical session record (2026-08-21) — queue reconciled 2026-08-25; see ADR-LANDING-PLAN annotations**

**Goal that was set:** deploy a ruflo-managed mesh of opus agents; find all unresolved
ADRs and TODO items in VisionClaw (`.`) and `./agentbox`; complete all remaining tasks
and integrations unless there is reason to ask questions.

**State at stop:** surveys COMPLETE (both repos), receipts green, one deliverable
created (`xr-godot-ci.yml`), commit groups identified but **not committed**, doc-drift
repairs queued but **not applied**, codex adversarial pass + AQE fleet checks **not yet
run** (requested by operator). Implementation halted on operator instruction.

---

## 1. Mesh status

- Opus subagent tier **unaffordable**: all three opus survey agents failed with OpenRouter
  `402` ("can only afford 60039 tokens" vs 64000 requested). Haiku tier works fine.
- Surveys were completed by direct extraction (status-line grep across all 177 ADRs) +
  two haiku deep-read agents. Method is sound; only the model tier changed.

## 2. Test receipts gathered this session (local container)

| Suite | Result |
|---|---|
| `cargo test -p visionclaw-xr-gdext` (xr-client/rust workspace) | **195 passed, 0 failed** — matches HP close-out receipt |
| `cargo test -p visionclaw-protocol -p visionclaw-xr-presence` (root workspace) | **107 passed, 0 failed** |

## 3. Deliverable created

`.github/workflows/xr-godot-ci.yml` — NEW, uncommitted. Three jobs:
- `xr-rust-tests` (blocking): gdext --all-features + xr-presence.
- `gut-headless` (blocking): Godot 4.3 headless + GUT vendored at 9.3.1 (not committed
  upstream), canonical invocation `-gconfig=res://.gutconfig.json`, JUnit artifact.
- `quest3-android` (**continue-on-error** until first green hosted run — NDK r26d,
  cargo-ndk, export templates, APK ≤80 MB gate per PRD-008 G4).

Note: PRD-008 §5.7 claims the workflow is "473 lines, operational" — it wasn't. The new
file supersedes that claim; PRD line still needs the drift edit (queued, §6).

## 4. Uncommitted working tree — three clean commit groups

1. **XR WS-C close-out fixes** (verified, ready to commit):
   `xr-client/scripts/*`, `xr-client/tests/*`, `xr-client/.gutconfig.json`,
   `xr-client/rust/src/{binary_protocol,transport}.rs`,
   `xr-client/rust/examples/gen_closeout_key.rs`,
   `crates/visionclaw-protocol/src/socket_flow_messages.rs`, `src/handlers/socket_flow_handler/types.rs`,
   `src/actors/{client_coordinator_actor,client_filter,graph_state_actor}.rs`,
   `docs/adr/ADR-136-*` (new), `docs/gap-close-evidence/P2-vive-closeout-2026-08-20.md` (new).
   Do NOT commit `xr-client/tests/report_hp_junit.xml` (test artifact).
   Defects fixed: set_identity→set_avatar_identity Node3D collision; GDScript 4.3 `:=`
   Variant inference ×5 test files; run_gut.gd GUT 9.3 shim; server `did_nostr` in
   initialGraphLoad; client GraphInbound::Text forwarding; M4-RAY observe latch.
2. **Public-demo deployment posture** (separate concern):
   `client/src/*` (VITE_PUBLIC_DEMO gate on junkiejarvis.com, quality-decile default 0.9,
   maxNodeCount→MAX_SAFE_INTEGER), `src/settings/*` (mirrors same defaults),
   `Dockerfile.production` (clang for oxrocksdb-sys, crates COPY for metadata, PTX path
   move, /workspace/ext symlink), `nginx*.conf`, `docker-compose.*`, `config.yml`
   (cloudflared hostname junkiejarvis.com), `env.production.template`, `scripts/*`.
3. **agentbox submodule pointer** 5100e88 → 866cde8 (podcast ingest commits inside
   agentbox; agentbox itself has only untracked `test-results/`).

Standing discipline: **local commits only, nothing pushed** — push stays operator-gated.

## 5. ADR survey results

### VisionClaw (docs/adr/, 118 files)

Unresolved:
- **Implementing:** ADR-064 typed graph schema · ADR-068 logseq block fidelity ·
  ADR-069 force presets (P0/P1 done; D11 temporal Z soft-spring open) ·
  ADR-070 CUDA hardening (P0 landed; P1/P2 open)
- **Partial:** ADR-031 GPU analytics (LOF kernel landed; D7 host-side correctness tests
  gate CI still open) · ADR-072 autordf2gml (only semantic_type_registry exists; five
  named services unbuilt)
- **Proposed, unbuilt:** ADR-060 pubkey visibility filter (verified absent 2026-07-03;
  blocked behind ADR-059 Phase 4) · ADR-066 pod-federated storage (3–4 weeks; crypto
  primitives specified, actors unbuilt) · ADR-114 ontology class-index memory substrate
  (~2 weeks; RuVector HNSW + Oxigraph binding)
- **Deferred frozen (do NOT build; unfreeze conditions recorded in-file):**
  ADR-065 rust-code-analysis pipeline · ADR-067 ontobricks-bridge crate ·
  ADR-121 writeback flywheel · ADR-122 two-speed routing · ADR-123 voice sign-off
- **Awaiting operator ratification:** ADR-126 OMB adoption posture
- **Accepted with deferrals:** ADR-127 (WS-3/WS-4 → Phase 2) · ADR-059 (gluon force
  needs transient-edge GPU buffer; :9500 state-poll cutover follow-on)
- **Drafts:** rvf-integration-{afd,ddd,prd} (×3)
- **Hygiene:** ~50 files have NO status line — do not mass-stamp; verify each first.
- **Stale-status candidates (verify, then update with citations):** ADR-135 Loom node
  says "Proposed" but the Loom is deployed load-bearing (Qwen3.8 cutover 2026-08-14);
  ADR-111 says "Proposed" but its §7 execution note records the follow-up campaign as
  completed 2026-07-22.

### agentbox (docs/reference/adr/, 59 files)

Haiku deep-read classification (23 proposed/no-status ADRs):
- **Agent-completable without human decision:** ADR-016 (license field mechanics) ·
  ADR-038 (AICT bounded trial setup) · ADR-044 (voice-plane repoint — config over
  existing surfaces) · ADR-047 (seven boundary rules = policy/test discipline) ·
  ADR-054 (three concrete ontology-bridge defect fixes) · ADR-055 (read-only dream
  cockpit view over ledger files)
- **Partially completable:** ADR-037 (D1–D7 agent-side; D8 one-liner CI script) ·
  ADR-042 (D1–D3, D6) · ADR-045 (forum link-through wiring) · ADR-050 (write-half:
  broker case + PR) · ADR-051 (D1/D2/D4/D7 client-side) · ADR-052 v1 (nightly gate
  orchestration) · ADR-056 Phase 1 · ADR-057/058/059 (contract/design halves)
- **Blocked / needs human decision:** ADR-017 (needs solid-pod-rs alpha.12 + federation
  peer-trust decisions) · ADR-020 Surface 2 (behind ADR-018 acceptance gate) ·
  ADR-026 (cross-repo WS5-D1/WS7 slices) · ADR-043 (two rebuild-class flips) ·
  ADR-046 capabilities 2–4 (needs Whelk live + corpus drift resolved) ·
  ADR-048/049 (need Whelk live)

## 6. Queued work at stop (in order)

1. **Doc-drift repairs:** PRD-008 §5.7 workflow claim; agentbox `xr-runtime` README
   stale "no production network transport" finding (TungsteniteWsTransport +
   NostrAuth exist, test-covered); P2-M2 "broker:new_case egress unshipped" stale
   server-side note; ADR-135 + ADR-111 status refreshes with evidence citations.
2. **POWER_USER_PUBKEYS** in `.env`: add close-out test pubkey
   `1543b25f2c34fcdff7b83c6cef041999f8657881eb2d6c6213fa29c824b2b22a` (COM18 decide
   POST prep; prescribed by close-out record §4; flagged to operator as auth-config).
3. ~~Commit groups 1–3 above~~ — **OPERATOR DIRECTIVE 2026-08-21: do not commit
   anything.** The three groups stay identified in §4 for whenever the operator
   authorises committing; nothing is staged or committed as of this handoff.
4. **Finish TODO sweep:** VisionClaw seed list (15 markers) needs FIXABLE-NOW
   classification pass; agentbox TODO sweep not yet run at all.
5. **Codex adversarial pass** over the commit groups + new CI file, then
   **agentic-qe fleet checks** (fleet_status / quality gates) — operator-requested,
   not yet executed.
6. Re-run `./agentbox.sh ruvector recall` band check if memory writes land.

## 7. Operator-blocked (cannot be done by any agent)

Per close-out record §6: Steam login on HP (expired cached token), SteamVR install
(appid 250820) + active_runtime.json, XRBoot smoke, in-headset canaries
M1-HUD / M4-RAY (auto-fires from the new latch) / COM18-INTERV, then tier promotions
standalone→integrated in P2-M* evidence files citing the close-out record + ADR-136.

## 8. Traps (do not rediscover the hard way)

- Never rebuild containers in-container (host bind-mount bakes stale code); build via
  tmux tab 6 `./scripts/launch.sh`.
- DO NOT curl `POST /api/canary/observe/CANARY-VC-M4-RAY` manually — one-shot latch
  falsely fires.
- HP `192.168.2.48` is dead; HP is reachable only via `ssh john@10.10.10.1`.
- Opus subagents 402 until OpenRouter credits topped up; use haiku tier meanwhile.
- Memory discipline: RuVector MCP tools only (namespace `project-state`); CLI bypasses
  embeddings.

## 9. Memory

Session state stored under RuVector `project-state` key
`handoff-adr-todo-sweep-2026-08-21` before stop.
