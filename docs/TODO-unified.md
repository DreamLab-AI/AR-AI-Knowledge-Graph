# Unified TODO — VisionClaw + agentbox

**Status:** Living document (single combined register — supersedes split tracking)
**Last refreshed:** 2026-08-31 (was frozen 2026-07-22; refreshed for the ADR-137–141 XR/layout landing)
**Created:** 2026-07-22, from the doc-drift audit (`docs/audit-doc-drift-2026-07-22.md`, ADR-131) and the agentbox backlog/audit (`agentbox/docs/developer/backlog.md`, `agentbox/docs/reference/audit-2026-07-15.md`)
**Governed by:** [PRD-024 Final-Mile Closeout](prd/PRD-024-final-mile-closeout.md), [ADR-133](adr/ADR-133-final-mile-sprint.md)
**Rule:** every entry carries exactly one unblock state. Mislabelling a state is itself a defect (the REC-9 rule). Remove entries when done; closures need evidence files.

The six unblock states:

| State | Meaning | Unblocks when |
|---|---|---|
| `code-gap` | Code has never been written or is half-shipped | An agent tick writes it |
| `ops-action` | Infrastructure/config action, minutes of work | Operator (or authorised tick) performs it |
| `live-session` | Code+test proven; needs observation on real traffic | An operator tock drives the session |
| `posture` | Deliberately held closed; a decision, not a gap | Operator decides (deciding "stay closed" also closes the entry — re-date it) |
| `data-floor` | Waiting on samples/corpus to accumulate | The clock; check floor, then flip |
| `external` | Blocked outside both repos | Upstream/network change |

---

## 1. The keystone (ops-action, unblocks the most entries)

| # | Entry | State | Detail |
|---|---|---|---|
| K-1 | ~~Bring `visionclaw-server:4000` up~~ **DONE 2026-07-22** | `ops-action` | Operator launched on host; `http://visionclaw-server:4000/api/health` → 200 verified from the dev container (alias resolves). K-2, §3, MCP-4, KG elevation, RES-d now unblocked. |
| K-2 | ~~Canary registration sweep~~ **DONE 2026-07-22** | `ops-action` | All nine canaries registered and armed on the live LivenessHarness (200s, `sha_at_registration: c889bdf6`, confirmed via `/api/canary/status`): DID, VOICE, CTC, MAST, AUTH, LEARN, DIVERSITY, PROV, ONTO-TELEM. Evidence files amended with receipts. Live **fires** remain §3 items. |

## 2. Code gaps (`code-gap` — swarm ticks)

| # | Entry | Repo | Detail |
|---|---|---|---|
| C-1 | ~~XR residue removal after the Tock-2 decision~~ **OBSOLETE 2026-08-31** | VisionClaw | Superseded by the shipped native XR client (Godot + gdext + OpenXR, ADR-071/ADR-136/ADR-137). The keep-or-delete question this entry planned around the old browser-AR path is moot: XR now ships as a first-class native client with its own `RenderStore` render path (ADR-137), immersive interaction (ADR-139), and agent-swarm visualisation (ADR-140). Any remaining `vircadia`/browser-AR residue is dead code to be swept opportunistically, not a decision-blocked workstream. Closes with T-2. **Sweep executed 2026-08-31:** 12 files removed (11 client — `quest3AutoDetector`, `useQuest3Integration`, `VircadiaContext`, `VircadiaBridgesContext`, `GraphVircadiaBridge`, `BotsVircadiaBridge`, and the 5 `services/vircadia/*` classes — plus the orphaned backend `handlers/api_handler/quest3/` REST route); `App.tsx` and `api_handler/mod.rs` unwired. The island was mounted but dormant (autoConnect=false, no external consumer read the context, App discarded the hook result, and nothing repo-wide called the `/quest3` REST endpoints). **Kept (not browser-AR residue):** `platformManager.ts` quest3 platform-detection enum (generic device detection); `remoteLogger.logXRInfo()` (generic WebXR capability logging, self-invoked); `LiveKitVoiceService.ts` (live service, Vircadia refs are comments only); `settings.ts` `VircadiaSettings` type (unread but harmless, no island import); `streaming_pipeline.rs` `GPUSafetyConfig::quest3()` GPU preset (unused but native-render territory, not browser-AR). Gates green: client `tsc --noEmit` exit 0, backend `cargo check` exit 0 (pre-existing warnings only). |
| C-2 | ~~AUTH-001 execution after Tock-2 decision~~ **DONE 2026-08-31** | VisionClaw | Resolved by *porting* (not merging) the reference RBAC. `sprint-3/jss-cut-scaffold`'s `enterprise_auth.rs` was a design reference only — it keyed off a spoofable `X-Enterprise-Role` header with an unsuited workflow taxonomy. Shipped instead ([ADR-142](adr/ADR-142-multi-user-rbac.md)): a persisted four-tier lattice `Owner/Admin/Editor/Viewer` (`src/models/rbac.rs`) bound to the NIP-98-verified pubkey, per-pubkey SQLite store (`src/services/role_store.rs`), central `/api`-scope enforcement (`src/middleware/rbac_gate.rs`, closes the unauthenticated-mutation gap), admin surface `/api/admin/rbac/*`, and role resolution wired into the existing `verify_access`. Tests: rbac lattice + role store + gate policy (unit) and `tests/adr142_rbac_gate.rs` (route-guard); `cargo check`/`cargo test` green. Branch `6520d6f2e` retained for history. |
| C-3 | ~~SQLite backup workstream~~ **DONE 2026-08-31** | VisionClaw | Exposed by the PRD-014 correction: the deleted "Neo4j daily backup" checkbox masked that `data/{kpi,enrichment,settings}.sqlite3` have **no backup at all**. Delivered: `scripts/backup-sqlite.sh` (online `.backup` API, docker/host auto-detect, timestamped + rotate keep-14, per-DB integrity_check), `scripts/restore-sqlite.sh` (`--yes`-gated), runbook `docs/how-to/sqlite-backup-restore.md`. **Reality:** live DBs are in Docker volume `visionclaw-data` → `visionclaw_container:/app/data` (repo `data/*.sqlite3` are stale). **Restore drill evidence:** backup→restore→`PRAGMA integrity_check`=ok; live & restored `kpi_agent_events` both **205,577**. |
| C-4 | ~~`tree-search-coder` author-or-disarm~~ **DONE 2026-08-31** | agentbox | Authored (not disarmed): `skills/tree-search-coder/SKILL.md` + 4 references (algorithm, negative-routing, exemplars, failure-contract) implement ADR-020 Surface 2 — execution-gated best-of-N code search (generate N via `sparc:coder` → fresh `kernel.reset`/`kernel.exec` per branch → score on assertion-pass count → shortest-code tie-break → mandatory `spend_cap_usd` halt → audit JSONL). Per ADR-020 §Decision, Surface 2 is an **orchestration skill (SKILL.md-only, no MCP server)**; runtime path is the coordinator agent, no daemon. Registration complete: SKILL-DIRECTORY row, `schema/agentbox.toml.schema.json` `tree_search_coder` node, `agentbox.toml` gate (`enabled=true`, `spend_cap_usd=0.50`), and a new `system-manifest.js` catalogue entry (apply_class `rebuild`). Validators green: `scripts/agentbox-config-validate.js` exit 0 (E052/W051/W052 checked-and-passing — `code_interpreter.enabled=true`), `skills/lint-skills.sh` exit 0. |
| C-5 | ~~SK-2 / MCP-1 / MCP-2 projection rollout~~ **DONE 2026-07-22 (Tick 1)** | agentbox | project-skill-roots.mjs + project-mcp-servers.mjs; skills/mcp.json is now the projected MCP source; codebase-memory registers via projection. Entrypoint blocks are next-rebuild payload. |
| C-6 | ~~GATE-1 validator schema fix~~ **DONE 2026-07-22 (Tick 1)** | agentbox | openmed schema node added; validator exits 0 on HEAD; skill counts 115→116 (RES-d). |
| C-7 | ~~MCP-3 secrets hardening~~ **DONE 2026-07-22 (Tick 1)** | agentbox | Runtime chmod 600 applied to all live secret-bearing .mcp.json/.claude.json NOW; entrypoint 0600-enforcement is next-rebuild payload. Follow-up: rotate historically-exposed Perplexity key + email bearer token. |
| C-8 | ~~Env consolidation execution~~ **DONE 2026-07-22 (Tick 1)** | agentbox | .env.example now 107 keys per the plan; wizard knows CERAMIC_API_KEY; retired templates carry deprecation pointers; plan stamped EXECUTED. |
| C-9 | GPU-1/GPU-2 nix library-path fix — **STAGED 2026-08-31 (next-rebuild payload)** | agentbox | Root cause confirmed live: nix RUNPATHs exclude `/usr/lib` (where nvidia-container-toolkit injects libcuda/libGLX_nvidia; `NVIDIA_CTK_LIBCUDA_DIR=/usr/lib`), so nix GPU binaries dlopen-fail and CPU-fall-back. Fix: nixGL-style wrapper `lib/gpu-wrap.nix` (`--suffix LD_LIBRARY_PATH` = host driver dirs + glvnd/EGL/Vulkan vendor env), wired in `flake.nix` for ffmpeg/blender/qgis/3DGS (colmap+lichtfeld), plus `LD_LIBRARY_PATH` in `gpu-backend.nix` `supervisorExtraEnv` for supervised services (comfyui/torch). Proven before/after: Blender 0→3 CUDA devices; ffmpeg `h264_nvenc` "Cannot load libcuda.so.1"→inits. Bakes on T-6 round-2 rebuild. Vulkan ICD JSON provisioning is a container-toolkit runtime concern (needs a non-`void` GPU allocation) — verify post-rebuild. |
| C-10 | ~~Minor follow-ups~~ **DONE 2026-07-22 (Tick 1)** | agentbox | XINFERENCE_ENDPOINT host-side fallback; browser.md renamed with stub; backlog Done section updated. |
| C-11 | Branch graveyard triage — **partial: 123 branches triaged 2026-08-31; 4 disposed, 3 kept, 116 operator-gated** | VisionClaw | Triage of all 123 non-`main` local branches (evidence: `docs/gap-close-evidence/branch-triage-2026-08-31.md`). Outcome — (a) MERGED deleted: 1 (`xr-vive-runtime`); (b) SUPERSEDED archive-tagged+deleted: 3 (`archive/deprecated-docs`, `worktree-agent-a7c66ae9b4265894b`, `new-docs`→already-archived); (c) VALUABLE kept, need owner decision: 3 (`refactor/kg-node-rename` 63-commit KGNode refactor, `report/soundings-qe-audit`, `impl/khive-investigation`); (d) **116 branches are a locked `/batch` worktree pool** (antigravity/codex/deepseek/gemma/loom-raw/ollama lanes) — fully merged but checked-out+locked, so `branch -d` refuses them. **Operator-gated (T-5):** confirm pool idle, then bulk `git worktree remove --force` + branch delete. Tags/deletes local only, not pushed. |
| C-12 | ~~CI clippy debt — main is red~~ **DONE (verified 2026-08-15)** | VisionClaw | Cleared by intervening pushes since the 2026-07-23 observation: blocking jobs green on the last three main CI runs (receipts: runs 31877215531 @ af21095d1, 31877367976 @ d03c2519d, both success 2026-08-15). A worktree sweep found nothing left to fix. |
| C-13 | narrativegoldmine follow-ups (logseq repo) — **partial: ADR-NG-002 P1 IRI-integrity gate landed 2026-08-15** (logseq b23061587: baseline-aware gate live in publish workflow, 30 slug-divergences repaired at source, 3,998 missing-concept IRIs baselined as authoring backlog; V-1's dangling-parent case is now caught by the same ratchet; ADR-NG-002 P2 Loom reload trigger still open) | logseq | From PRD-NG-001 closeout 2026-07-23: edge-label off/hover/on feature (needs NGG1 v2 per-edge predicate strings + renderer — control removed as dead until then); TRAVERSE neighbourhood query-builder (M, same format bump); DQ report page precomputed by pipeline (S–M); "equivalent SPARQL" tab (S–M); V-1 21 dangling parent slugs (emit validator + alias resolution); V-2 6 duplicate labels (corpus); V-3 ARCHITECTURE.md key-numbers refresh (restrictions 2.4k→38.6k). External validation receipt: `logseq/docs/validation/external-validation-rdf-studio-2026-07-23.md`. |

## 3. Live-session pending (`live-session` — operator tocks)

| # | Entry | Detail |
|---|---|---|
| L-1 | Fire the agentbox envelope canaries on real traffic | One driven session covering: `/v1/voice-intent` (COM-15), MAST `failure_mode` tag (REC-5), zero-tolerance block→31403→release (REC-6), CTC cost fields (REC-3), provenance URN resolve (REC-9), DID mint federation proof (COM-14). Requires K-1/K-2. |
| L-2 | REC-8 diversity canary | Needs a **second model family** in the candidate pool first (same-family-only candidates cannot fire it). Config change + live fire. |
| L-3 | ADR-117 clamp live fire | POST an un-LIMITed >10k-row SELECT against the live server; verify `truncated:true` ≤8MiB. Falsification in `docs/gap-close-evidence/P0-ADR117-CLAMP.md`. |
| L-4 | ADR-119 telemetry live fire | Induce a seed-stage failure in a live session; verify `fail_open_count` increments in `ontology_health`. Falsification in `P0-ADR119-TELEMETRY.md`. |
| L-5 | Quest 3 physical on-device validation | Godot MR client canaries are Monado/headless-proven only; needs a human wearing the headset (P2-M6 discipline). |
| L-6 | Mobile bridge end-to-end on phone | After T-1 posture flips: Amethyst + Amber, phone npub allowlisted, note-to-self thread observed. |

## 4. Posture decisions (`posture` — operator tocks, deciding "no" is also closure)

| # | Entry | Detail |
|---|---|---|
| T-1 | Relay exposure chain | Step 1 DONE 2026-07-22: `allowed_pubkeys` populated (operator human key, visionclaw-server, beema, RedDread¹, junkiejarvis, mobile phone key) — **baked at nix build, inert until the T-6 image rebuild**. Remaining: `expose = true` → `mobile_bridge.enabled = true`. ¹RedDread = presumed "house admin" (third D1 admin); remove if misidentified. Note: `[sovereign_mesh.operator].pubkey_hex` still carries the visionclaw-server key — same ADR-040 D3 key-split defect as the forum config; fix via that runbook, not piecemeal. |
| T-2 | ~~XR residue: keep Quest 3 browser-AR path or delete it~~ **DECIDED 2026-08-31 — delete** | Resolved by the native XR client shipping (ADR-137 render offload lands the Rust `RenderStore` path; ADR-139 immersive interaction; ADR-140 agent-swarm XR). The browser-AR path is superseded, not kept. C-1 is now an opportunistic dead-code sweep, not a blocked decision. |
| T-3 | ~~AUTH-001: merge four-tier RBAC or stay coarse~~ **DECIDED 2026-08-31 — port, don't merge** | Neither "merge the branch" nor "stay coarse": the reference `enterprise_auth.rs` was ported fresh onto `main` as a pubkey-bound `Owner/Admin/Editor/Viewer` lattice ([ADR-142](adr/ADR-142-multi-user-rbac.md)). Executes C-2. |
| T-4 | Remaining held surfaces | ~~Multi-user DIDs~~ **DONE 2026-08-31** — per-pubkey persisted RBAC roles + central `/api` enforcement + admin management surface shipped ([ADR-142](adr/ADR-142-multi-user-rbac.md)); did:nostr identity (ADR-120) now carries an authorization role. Still held: git pods/host gateway, payments (parked until counterparty), Solid OIDC issuer, pod MCP surface, kernel pip. Re-date if still deferred. |
| T-5 | Remote `crashbug` deletion | Local branch deleted + `archive/crashbug` tag exists; deleting `dreamlab-github/crashbug` is a push — operator authorises. |
| T-6 | Agentbox image rebuild window (round 2) | Round 1 (2026-07-22) activated: condense scheduler, relay allowlist, sweep/distill scheduling — verified live. Round 2 payload now staged: MCP-registry projection + skill-root collapse (C-5 entrypoint blocks), .mcp.json 0600 enforcement at source (C-7), GPU library-path wrappers (C-9, **staged 2026-08-31**: `lib/gpu-wrap.nix` + `flake.nix` package wrapping + `gpu-backend.nix` supervisor `LD_LIBRARY_PATH`). ~15 min host op. |

## 5. Data-floor (`data-floor` — the clock)

| # | Entry | Detail |
|---|---|---|
| D-1 | `feed_retrieval` / `feed_routing` learning consumers — **floor cleared, `feed_retrieval` ENABLED 2026-08-31** | Floor cleared: 78 aggregates ≥20 samples (was 12 at last audit). Prescribed sequence executed: `aggregate-effectiveness --yes` applied (78 stored), `feed_retrieval = true` in agentbox.toml, recall gate re-run post-flip per I14 — **PASS** (artifact `agentbox/backups/ruvector-sidecar/recall-runs/2026-08-31T08-08-50-003Z.json`). Remaining: observation window, then flip `feed_routing`. |
| D-2 | SONA / attention re-rank | Inert by measurement (384-dim vs hardcoded 256; no-op on L2-normalised corpus). Revisit only if the corpus geometry changes. Correctly documented; no action. |

## 6. External blockers (`external`)

| # | Entry | Blocked on |
|---|---|---|
| E-1 | ComfyUI integration | No ComfyUI service on `visionclaw_network` (builtin path needs heavy image rebuild + source hash). |
| E-2 | ~~GitHub enrichment~~ **DONE 2026-07-22** | Token validated live (HTTP 200); present in agentbox runtime env + .env (0600); `github_enrichment = true` (boot apply-class — active at next restart). Follow-up: swap broad-scope classic PAT for a fine-grained read-only one. |
| E-3 | Ollama sidecar — **re-confirmed absent 2026-08-31** | No ollama answering on :11434 at the docker gateway or known LAN hosts; sidecar-off remains correct. Re-check only if the operator installs one. |
| E-4 | Nagual QE toolchain | Upstream sqlx 0.9 `SqlSafeStr` compile error. Wait or pin. |
| E-5 | ~~VisionClaw self-hosted GPU runner offline~~ **RESOLVED 2026-07-24** | The non-blocking GPU CI job queued indefinitely (`[self-hosted, gpu]` runner not registered) — 15h/3h zombie runs. Operator decision: the GPU crates compile+run on the CUDA host during the normal build/deploy flow (authoritative validation), so the CI job added nothing at deploy time. **Job removed** (VisionClaw 5d28071c2). Re-add only if a GPU runner is registered and real-hardware CI is wanted. |

## 7. Cleanly deferred (frozen — do not reopen without a new ADR)

ADR-073..085 window (Nostr relay federation mesh, forum extraction, website-kit cutover — frozen by closeout 2026-07-03, except ADR-074/075/076/077 which were never frozen), ADR-122/123 (two-speed writeback routing, voice sign-off), RVF file store (KNOWN_ISSUES AGENT-001 — honest "not implemented"), XR APK cross-build + LiveKit Android AAR (sprint-scale, PRD-008 §5.5).

## 8. Landed 2026-08-31 — XR immersive-interaction + layout programme (ADR-137–141)

Recorded here for the register's completeness; code is shipped and reflected in
both CHANGELOGs, `docs/reference/rest-api.md`, and the new developer reference
`docs/reference/render-store-and-force-channels.md`. Only on-headset observation
remains open (folds into L-5).

| # | Entry | State | Detail |
|---|---|---|---|
| X-1 | ~~XR render offload + runtime quality dials~~ **DONE (ADR-137)** | `code-gap` | Per-frame hunt + MultiMesh packing moved GDScript→Rust `RenderStore` (`xr-client/rust/src/render_store.rs`); topology-derived draw budgets; `initialNodeLimit` dial; 256 MiB receive cap; full-3D layout default. 90 fps at 13,164 nodes / 145,692 edges. |
| X-2 | ~~GPU force-channel registry + pinned-node mask~~ **DONE (ADR-138)** | `code-gap` | `ForceChannel` mapping-layer registry (`src/models/force_channels.rs`); GPU `pinned_mask` buffer masks pinned nodes out of integration while still exerting force on neighbours (`visionclaw_unified.cu:933,941`; `force_compute_actor.rs`). Array-backed `SimParams` refactor is the deferred step 2 (not owed now). |
| X-3 | ~~Graph2VR-class immersive interaction~~ **DONE (ADR-139)** | `code-gap` | Two-hand pinch scale/rotate, radial menu, in-graph search, node expansion API — clean-room re-implementations, no external code vendored. |
| X-4 | Agent-swarm XR visualisation — **P1 DONE (ADR-140)** | `code-gap` | `0x23 AGENT_ACTION` consume side (`AgentBeamActor`/`BeamCoalescer`), embodied agents, work beams, HUD Swarm tab. ADR status is Proposed (P1 shipped); later phases open. |
| X-5 | ~~Constrained-layout engine programme~~ **P1–P4 DONE (ADR-141)** | `code-gap` | Sugiyama layers, stratified planes, spherical shells, ego-radial `RadialModes`; `POST /api/layout/mode` + `/api/layout/radial`. P5/P6 deferred by the ADR. Also lands the visual query builder (`POST /api/graph/query/pattern`) and fold ladder (`GET /api/graph/fold`), plus the V5 wire wrapper. |
| X-6 | On-headset validation of the new XR features | `live-session` | The ADR-137–141 client work is Monado/desktop-OpenXR proven; physical Vive/Quest on-device observation of render offload, immersive interaction, and swarm beams folds into L-5 (P2-M6 discipline). |

---

*Done and removed from this register (2026-07-22): C1 immersive-tree deletion, C2 telemetry sink, C3 SPARQL clamp, C4–C6 dead protocol encoders, C7 condense scheduler, crashbug local purge, jss worktree removal, RVF orphans, 33 merged branches, the full §1 doc banner sweep, ADR-113/115–120/131/132, three P0 evidence files. See ADR-131 and `git log` for receipts.*
