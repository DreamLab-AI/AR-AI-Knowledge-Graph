# Unified TODO — VisionClaw + agentbox

**Status:** Living document (single combined register — supersedes split tracking)
**Last refreshed:** 2026-08-31 (consolidation sweep: absorbed the ADR consultant-panel roadmap as §9, reviewed + folded the stale 2026-06-12 `todo.md` hand-off as §10, indexed design open-questions as §11 — see §0 for the full source manifest)
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

## ⭐ Open now — the whole remaining list (at a glance)

Everything in §1–§8 is **done** except the four rows flagged below; §9–§12 are the
live work. This board is the complete set of what's open — jump to the cited
section for detail. (Paused here 2026-08-31 by operator request.)

**ADR governance — §9 (decision written, code owed):**
- **Quick win:** `G-6` `did:nostr:local` fail-open abort (S).
- **Proposed records to build:** `G-1` dev-auth build guard · `G-2` boot-profile assertion · `G-3` federation CI fixture · `G-4` mirror-egress boundary · `G-5` secret custody + key split.
- **Larger / parked:** `G-7` provenance durability (L) · `G-8` replay-cache DoS · `G-9` Loom `:8084` audit · `G-10` LAN-door threat model *(posture)* · `G-11` session-bearer sunset *(posture)* · `G-12` AoE token rebuild *(the one live HIGH)* · `G-13` PTX hardening · `G-14` supersession graph + ID namespacing · `G-15` status-axis lattice *(posture)*.

**Operator tocks — live-session §3:** `L-1` envelope canary fires · `L-2` diversity canary (needs 2nd model family) · `L-3` ADR-117 clamp · `L-4` ADR-119 telemetry · `L-5`/`X-6` on-headset XR · `L-6` mobile bridge.

**Posture decisions — §4:** `T-1` relay expose + mobile bridge · `T-4` held surfaces (git pods, payments, Solid OIDC, pod MCP, kernel pip) · `T-5` remote `crashbug` delete · `T-6` agentbox image rebuild window.

**Data-floor — §5:** `D-1` flip `feed_routing` after the observation window.

**External — §6:** `E-1` ComfyUI service · `E-4` Nagual QE (upstream sqlx 0.9).

**Doc-accuracy — §12:** `DOC-1` rest-api phantom endpoints · `DOC-2` CUDA kernel-count truth · `DOC-3` `installation.md` refresh.

**Obsidian vault migration — §2 `C-14` (decisions written, code + two operator steps owed):** ADR-2040/2041/2042 + agentbox ADR-2028/2029, governed by [`VAULT-corpus-format.md`](VAULT-corpus-format.md). Operator owes the in-place conversion of the corpus repo and the agentbox image rebuild that ships Rune.

**Near-closed (one operator confirm):** `C-11` branch pool (9 left) · `C-13` logseq narrativegoldmine (ADR-NG-002 P2 + minor).

**Frozen — §7 (do not reopen without a new ADR):** listed for completeness; not open work.

---

## 0. Consolidated sources (this is the single index — 2026-08-31)

Every backlog across the two repos now funnels here. Disposition of each source found in the 2026-08-31 sweep:

| Source | Repo | Disposition |
|---|---|---|
| `docs/TODO-unified.md` (this file) | VisionClaw | **Canonical register.** Everything below. |
| `docs/ROADMAP-consultant-panel-2026-08-31.md` | VisionClaw | Five-model ADR gap-analysis. Open items pulled into **§9**. |
| `agentbox/docs/developer/backlog.md` | agentbox | Subordinate (self-declared). Live items reflected here; stale items retired in place. |
| `todo.md` (repo root) | VisionClaw | Stale 2026-06-12 session hand-off. Unique live candidates pulled into **§10** (liveness-unverified); file reduced to a redirect stub. |
| `agentbox/docs/developer/code-as-harness.md` §Open Questions | agentbox | Design questions (ADR-018/019 revisions). Pointer in **§11**. |
| `docs/explanation/ddd-contributor-enablement-context.md` §Open Questions | VisionClaw | Design questions (DDD/BC). Pointer in **§11**. |
| tutorial/how-to "Next Steps" endings | both | Doc-local, not project backlog — excluded. |
| `agentbox/skills/.../khive-v2-roadmap.md` | agentbox | About an external project (KHIVE) — excluded. |
| `docs/archive/**`, `agentbox/docs/archive/**` | both | Frozen history — not authority (see §7). |

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
| C-9 | ~~GPU-1/GPU-2 nix library-path fix~~ **DONE 2026-08-31 (verified live in T-6 round-2 rebuild)** | agentbox | Wrappers baked (057304da5): `ffmpeg`/`blender`/`qgis`/colmap/lichtfeld resolve to `-gpuwrapped` store paths; live `h264_nvenc` encode succeeded in the rebuilt container; nvidia-smi sees all 3 GPUs (A6000 + 2× RTX 6000 Ada); full graphics stack now injected (`/etc/vulkan/icd.d/nvidia_icd.json`, libGLX/EGL — the `NVIDIA_VISIBLE_DEVICES=void` env var is a cosmetic leftover, allocation is real). **Accepted limitation:** Vulkan `vkCreateInstance` → `VK_ERROR_INCOMPATIBLE_DRIVER` (-9) — nixpkgs-glibc vs host-driver (610.57) userland mismatch (nixGL-class problem); CUDA unaffected because libcuda's deliberately minimal deps are why the LD_LIBRARY_PATH suffix wrapper suffices. Disposition: graphics/Vulkan workloads run in Debian-userland sidecars (`browsercontainer`, `xr-runtime`); Blender uses CUDA/OptiX for Cycles, so nothing is lost in-agentbox. Revisit only on a concrete in-agentbox Vulkan need (nixGL wrapper pinned to host driver version). |
| C-10 | ~~Minor follow-ups~~ **DONE 2026-07-22 (Tick 1)** | agentbox | XINFERENCE_ENDPOINT host-side fallback; browser.md renamed with stub; backlog Done section updated. |
| C-11 | Branch graveyard triage — **near-closed: pool cleaned, 9 local branches remain (was 123/116-gated)** | VisionClaw | The 116-branch locked `/batch` worktree pool (antigravity/codex/deepseek/gemma/loom-raw/ollama lanes) has been cleared — `git branch` now shows **9** (verified 2026-08-31). Triage evidence: `docs/gap-close-evidence/branch-triage-2026-08-31.md`. Remaining: the 3 VALUABLE-kept branches still need an owner keep/merge/drop decision (`refactor/kg-node-rename` 63-commit KGNode refactor, `report/soundings-qe-audit`, `impl/khive-investigation`). Closes on that confirmation. |
| C-12 | ~~CI clippy debt — main is red~~ **DONE (verified 2026-08-15)** | VisionClaw | Cleared by intervening pushes since the 2026-07-23 observation: blocking jobs green on the last three main CI runs (receipts: runs 31877215531 @ af21095d1, 31877367976 @ d03c2519d, both success 2026-08-15). A worktree sweep found nothing left to fix. |
| C-13 | narrativegoldmine follow-ups (logseq repo) — **partial: ADR-NG-002 P1 IRI-integrity gate landed 2026-08-15** (logseq b23061587: baseline-aware gate live in publish workflow, 30 slug-divergences repaired at source, 3,998 missing-concept IRIs baselined as authoring backlog; V-1's dangling-parent case is now caught by the same ratchet; ADR-NG-002 P2 Loom reload trigger still open) | logseq | From PRD-NG-001 closeout 2026-07-23: edge-label off/hover/on feature (needs NGG1 v2 per-edge predicate strings + renderer — control removed as dead until then); TRAVERSE neighbourhood query-builder (M, same format bump); DQ report page precomputed by pipeline (S–M); "equivalent SPARQL" tab (S–M); V-1 21 dangling parent slugs (emit validator + alias resolution); V-2 6 duplicate labels (corpus); V-3 ARCHITECTURE.md key-numbers refresh (restrictions 2.4k→38.6k). External validation receipt: `logseq/docs/validation/external-validation-rdf-studio-2026-07-23.md`. |
| C-14 | Obsidian vault migration (branch `obsidian`) — **decisions written 2026-09-02, code + two operator steps owed** | VisionClaw + agentbox | The authored corpus moves from Logseq conventions to an Obsidian vault. Governing doc: [`VAULT-corpus-format.md`](VAULT-corpus-format.md). Records: [ADR-2040](adr/ADR-2040-obsidian-vault-frontmatter-gate.md) (frontmatter `public`/`owl-class` inclusion gate, bounded Logseq tolerance, supersedes ADR-2014) · [ADR-2041](adr/ADR-2041-graph-settings-key-knowledge.md) (`visualisation.graphs.logseq` → `graphs.knowledge`, `logseq` a read-only alias for one release) · [ADR-2042](adr/ADR-2042-vault-migrate-converter.md) (`crates/vault-migrate`, output-dir default, preserve-and-report) · agentbox ADR-2028 (`[vault].root` manifest path authority) · agentbox ADR-2029 (Rune markdown TUI, tmux window 9 "Notes"). **Code owed:** the single `visionclaw_domain::vault::PageMeta` parse entry point replacing the six ad-hoc line scanners; `crates/vault-migrate`; the settings rename with `serde(alias)` + client migration shim. **Operator steps remaining:** (1) **in-place conversion of the corpus repo** — `vault-migrate --in-place` against `jjohare/logseq` once the crate lands; the graph is the owner's private corpus and is never converted or pushed by an agent; (2) **agentbox image rebuild** so the Rune binary and the `[vault]` manifest root ship in the container (until then window 9 prints the rebuild notice). Living-docs sweep landed 2026-09-02 — `docs/` now describes the vault; `docs/archive/**` is frozen and deliberately untouched. The corpus repository keeps its GitHub name `logseq`, so C-13's narrativegoldmine work is unaffected. |

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

## 9. ADR governance backlog (consultant panel 2026-08-31)

From the five-model gap analysis (`docs/ROADMAP-consultant-panel-2026-08-31.md`).
This session **closed** P0-1 (staleness gate armed on 31 records), the two
code-with-defect fixes (node-ID overflow guard, ADR-2035 doc-comment), and the
ADR-2031 tombstone. Five gaps were **converted to `proposed` records** (decision
written, code owed): the "code-gap" rows below whose entry names an ADR carry a
ratified-but-unbuilt decision, so the design round-trip is already done.

| # | Entry | State | Detail |
|---|---|---|---|
| G-1 | dev-auth build guard (VC ADR-2037) | `code-gap` | CI/image assertion that production binaries carry the real `enforce_release_env_hygiene`, not the `#[cfg(feature="dev-auth")]` no-op stub (`src/main.rs:169`). Small; closes a verified release-security hole. |
| G-2 | Boot-time profile assertion (VC ADR-2038) | `code-gap` | Boot selector + abort on the ADR-2003 illegal combo (`RBAC_PUBLIC_READS=1`+`PUBKEY_VISIBILITY_FILTER=0`); production defaults to multi-user-locked. Stops demo-open reaching prod. The `RBAC_DEFAULT_ROLE=viewer` lever already shipped (8e78a9d19). |
| G-3 | Cross-repo federation CI fixture (AB ADR-2025) | `code-gap` | Shared sha12/hex-identity byte-parity test run in **both** repos' CI so an agentbox helper change can't silently break the visionclaw join. |
| G-4 | Session-mirror egress boundary (AB ADR-2026) | `code-gap`→`posture` | Inspect `config/hooks/nostr-live-mirror.cjs`; decide transcript redaction before NIP-59 wrap; make `AGENTBOX_LIVE_MIRROR=0` fail-**closed**. The sovereignty/privacy hole. |
| G-5 | Secret custody + publisher-key split (AB ADR-2027) | `code-gap`+`ops-action` | Custodian/rotation/revocation per load-bearing secret; execute the pending ADR-040 D3 publisher-key split (compromise window = one build-deploy cycle). |
| G-6 | `did:nostr:local` fail-open abort (AB ADR-2011) | `code-gap` | **Quick win (S):** abort boot on failed key derivation instead of minting the non-canonical placeholder into storage. |
| G-7 | Provenance durability + estate erasure (VC ADR-2016/2017) | `code-gap` | (L) Back up the Oxigraph provenance store (or dual-write to a backed-up SQLite), and scope the redaction/crypto-shred erasure path ADR-2016 defers. |
| G-8 | Replay-cache DoS mitigation (VC ADR-2002, AB ADR-2009) | `code-gap` | Per-pubkey admission/rate-limit ahead of `ReplayCacheFull`; consider replica-coordinated replay state. |
| G-9 | Loom `:8084` exposure audit (AB ADR-2023) | `ops-action` | Verify the plaintext model door isn't LAN-reachable beyond intended consumers; bind/authenticate if it is. |
| G-10 | Sanctioned LAN-door threat model (AB ADR-2009/2013) | `posture`+`ops-action` | Per-door decision for the nine `0.0.0.0` publishes (raw CDP `9222` first); bind to loopback or authenticate. Operationally disruptive — deliberately parked. |
| G-11 | Session-bearer sunset (VC ADR-2009) | `posture` | Deprecate the replayable UUID bearer realm — blocked on the React client signing requests; parked (breaks client until then). |
| G-12 | AoE token activation (AB ADR-2002/2009) | `ops-action` | The one genuinely **live** HIGH exposure: the running box is `--auth none` pending a T-6-style agentbox rebuild; reconcile AB-2002/AB-2009 on activation. |
| G-13 | PTX splice hardening (VC ADR-2030) | `code-gap` | Make the PTX `.version` rewrite fail on unexpected headers; pin/attest the bundled fallback. |
| G-14 | Type + populate supersession graph; namespace ADR IDs per repo | `code-gap` | The reciprocity CI check still governs an empty graph (all 59 `supersedes: []`); `VC-2022`/`AB-2022` collide with no repo qualifier. Editorial (L). |
| G-15 | Define the status-axis lattice | `posture` | Owner convention call: make incoherent triples (e.g. `complete`+admitted-defect, `none`+enforced-freeze like AB-2019) unrepresentable. Declined to flip unilaterally this session. |

## 10. 2026-06-12 hand-off review (`todo.md`, verified 2026-08-31)

The root `todo.md` was a June session hand-off predating the final-mile sprint.
Reviewed against current code during this consolidation — most of it has landed:

| June work-queue item | Verified state 2026-08-31 | Disposition |
|---|---|---|
| PRD-015 payments Phase 1 (pay402, gate, payer) | `lib/pay402.js` + `middleware/payment-gate.js` + `tests/contract/pay402/` exist; `[payments]=on` in agentbox.toml | **Built.** Enabling is posture (T-4, parked until counterparty). |
| Spend-approval ACSP case (kind-31402) | Payments plane shipped; approval reuses the ElevationActor pattern | Folds into T-4 posture. |
| Elevation loop operations (VC) | `src/actors/elevation_actor.rs` built + wired in `app_state.rs` | Code **done**; live-fire is L-1. |
| BC20 receipt/activity crossing (agentbox) | `receipt-minter.js:108` is the production `crossOutbound` caller the note said was missing | **Closed.** |
| XR on-device validation | Tracked as X-6 / L-5 | Already in register. |
| Unauth `GET /api/graph/data` (FINDING-1) | `rbac_gate.rs:344` maps it to `None` deliberately, with a test | **Governed posture** (demo-open reads), not a bug; safety governed by G-2. |
| `protocol-matrix.md` 0x42/36B claim | Zero hits — corrected | **Fixed.** |
| `rest-api.md` ~30 broker/workflow endpoints | 34 broker/workflow mentions still present | **Still live** — `code-gap`/doc: delete phantom tables or implement. |
| CUDA kernel-count truth (docs say 37/39/92) | 83 `__global__` in cuda sources now | **Still live** — `code-gap`/doc: bless one counting method, align claims. |
| `installation.md` stale `docker ps` narrative | Not re-verified this pass | Carry as doc-debris `code-gap` (low). |

**Residue that survives review → tracked as:** DOC-1 (`rest-api.md` phantom endpoints), DOC-2 (CUDA kernel-count truth), DOC-3 (`installation.md` refresh). Small doc-accuracy `code-gap`s; `todo.md` reduced to a redirect stub.

Note: **C-11** (branch graveyard) is now effectively resolved — local branches are down to **9** (from the 123/116-gated pool); close it on the next operator confirmation.

## 11. Design open-questions (pointers, not action items)

Unresolved *design* questions living with their governing docs; each is owned by a
future ADR revision, not a code tick. Listed so the register is the complete index:

- **agentbox code-execution** (`agentbox/docs/developer/code-as-harness.md` §Open Questions): kernel scope per-session vs per-worktree, pip-install policy, kernel GPU access, cross-session state persistence, lesson-quality threshold, Voyager discovery surface — all deferred to ADR-018/019 revisions.
- **DDD contributor enablement** (`docs/explanation/ddd-contributor-enablement-context.md` §Open Questions): cross-device workspace identity, suggestion-acceptance graph mutation timing, skill-version retirement propagation, inbox PII/GDPR retention (ADR-041 append-only vs ADR-052 contributor-owned), multi-partner lineage roots.

## 12. Doc-accuracy residue (`code-gap` — small)

Survived the §10 review; concrete doc-vs-code drift, low priority, no decision owed.

| # | Entry | Repo | Detail |
|---|---|---|---|
| DOC-1 | rest-api.md phantom endpoints | VisionClaw | `docs/reference/rest-api.md` still documents ~30 broker/workflow REST endpoints that don't exist (34 broker/workflow mentions as of 2026-08-31). Decide: delete the tables or implement. |
| DOC-2 | CUDA kernel-count truth | VisionClaw | Docs variously claim 37/39/92 kernels; `crates/visionclaw-gpu/src/cuda_sources/*.cu` hold **83** `__global__` decls. Bless one counting method and align the claims. |
| DOC-3 | installation.md stale narrative | VisionClaw | `docs/tutorials/installation.md` carries a stale `docker ps` narrative (not re-verified this pass). Refresh against the current compose. |

---

*Done and removed from this register (2026-07-22): C1 immersive-tree deletion, C2 telemetry sink, C3 SPARQL clamp, C4–C6 dead protocol encoders, C7 condense scheduler, crashbug local purge, jss worktree removal, RVF orphans, 33 merged branches, the full §1 doc banner sweep, ADR-113/115–120/131/132, three P0 evidence files. See ADR-131 and `git log` for receipts.*
