# Unified TODO — VisionClaw + agentbox

**Status:** Living document (single combined register — supersedes split tracking)
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
| C-1 | XR residue removal **after** the Tock-2 decision | VisionClaw | `quest3AutoDetector.ts` (live `setXRMode` caller at :143), `client/src/services/vircadia/` (5 files + 2 bridges), XR settings schema (`settings.ts:339,782,822`), platformManager XR surface, Vircadia compose service. Blocked-by: T-2 (posture). |
| C-2 | AUTH-001 execution after Tock-2 decision | VisionClaw | Either merge `sprint-3/jss-cut-scaffold`'s `enterprise_auth.rs` (four-tier RBAC) to main, or close AUTH-001 as banner-resolved. Branch preserved at `6520d6f2e`. Blocked-by: T-2. |
| C-3 | SQLite backup workstream | VisionClaw | Exposed by the PRD-014 correction: the deleted "Neo4j daily backup" checkbox masked that `data/{kpi,enrichment,settings}.sqlite3` have **no backup at all**. Scripts + runbook + restore test. |
| C-4 | `tree-search-coder` author-or-disarm | agentbox | Gate armed (`ENABLE_TREE_SEARCH_CODER=true`) but skill never authored. Author per ADR-020 §tree-search / PRD-008 §3.3, or flip the gate off for manifest honesty. Decision at Tock-2, execution as tick. |
| C-5 | ~~SK-2 / MCP-1 / MCP-2 projection rollout~~ **DONE 2026-07-22 (Tick 1)** | agentbox | project-skill-roots.mjs + project-mcp-servers.mjs; skills/mcp.json is now the projected MCP source; codebase-memory registers via projection. Entrypoint blocks are next-rebuild payload. |
| C-6 | ~~GATE-1 validator schema fix~~ **DONE 2026-07-22 (Tick 1)** | agentbox | openmed schema node added; validator exits 0 on HEAD; skill counts 115→116 (RES-d). |
| C-7 | ~~MCP-3 secrets hardening~~ **DONE 2026-07-22 (Tick 1)** | agentbox | Runtime chmod 600 applied to all live secret-bearing .mcp.json/.claude.json NOW; entrypoint 0600-enforcement is next-rebuild payload. Follow-up: rotate historically-exposed Perplexity key + email bearer token. |
| C-8 | ~~Env consolidation execution~~ **DONE 2026-07-22 (Tick 1)** | agentbox | .env.example now 107 keys per the plan; wizard knows CERAMIC_API_KEY; retired templates carry deprecation pointers; plan stamped EXECUTED. |
| C-9 | GPU-1/GPU-2 nix library-path fix | agentbox | Every nix GPU binary except wrapped Blender silently CPU-falls-back; in-container Vulkan dead. Apply Blender's wrapper pattern. Only if in-container GPU is wanted (confirm at a tock). |
| C-10 | ~~Minor follow-ups~~ **DONE 2026-07-22 (Tick 1)** | agentbox | XINFERENCE_ENDPOINT host-side fallback; browser.md renamed with stub; backlog Done section updated. |
| C-11 | Branch graveyard triage | VisionClaw | 126 local branches remain post-purge (33 merged ones deleted 2026-07-22; `crashbug` + `docs/neo4j-schema-update` archive-tagged and deleted). Classify remaining unmerged branches: archive-tag + delete, or keep with an owner. |
| C-12 | CI clippy debt — main is red | VisionClaw | Exposed 2026-07-23 by the first CI run to complete in weeks (prior runs cancelled by push trains): blocking Rust job fails on clippy lints (approximate-PI literals in `binary_settings_protocol.rs:571`, ~9 more across domain/adapters crates) + `cargo fmt` job red. Pre-existing debt, not from the docs pushes. One tick: clippy --fix + fmt sweep, then confirm blocking jobs green. |
| C-13 | narrativegoldmine follow-ups (logseq repo) | logseq | From PRD-NG-001 closeout 2026-07-23: edge-label off/hover/on feature (needs NGG1 v2 per-edge predicate strings + renderer — control removed as dead until then); TRAVERSE neighbourhood query-builder (M, same format bump); DQ report page precomputed by pipeline (S–M); "equivalent SPARQL" tab (S–M); V-1 21 dangling parent slugs (emit validator + alias resolution); V-2 6 duplicate labels (corpus); V-3 ARCHITECTURE.md key-numbers refresh (restrictions 2.4k→38.6k). External validation receipt: `logseq/docs/validation/external-validation-rdf-studio-2026-07-23.md`. |

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
| T-2 | XR residue: keep Quest 3 browser-AR path or delete it | Decides C-1 scope. `quest3AutoDetector` is the sole reason platformManager XR surface survives. |
| T-3 | AUTH-001: merge four-tier RBAC or stay coarse | Decides C-2. KNOWN_ISSUES banner (2026-07-22) documents current truth. |
| T-4 | Remaining held surfaces | Multi-user DIDs, git pods/host gateway, payments (parked until counterparty), Solid OIDC issuer, pod MCP surface, kernel pip. Re-date if still deferred. |
| T-5 | Remote `crashbug` deletion | Local branch deleted + `archive/crashbug` tag exists; deleting `dreamlab-github/crashbug` is a push — operator authorises. |
| T-6 | Agentbox image rebuild window (round 2) | Round 1 (2026-07-22) activated: condense scheduler, relay allowlist, sweep/distill scheduling — verified live. Round 2 payload now staged: MCP-registry projection + skill-root collapse (C-5 entrypoint blocks), .mcp.json 0600 enforcement at source (C-7), GPU wrappers if C-9 approved. ~15 min host op. |

## 5. Data-floor (`data-floor` — the clock)

| # | Entry | Detail |
|---|---|---|
| D-1 | `feed_retrieval` / `feed_routing` learning consumers | Wilson floor: `aggregate_min_samples = 20` per action pattern (recording since 2026-07-05; 12 aggregates past floor at last audit). When cleared: `./agentbox.sh ruvector aggregate-effectiveness` dry-run → flip `feed_retrieval` → observe → flip `feed_routing`. |
| D-2 | SONA / attention re-rank | Inert by measurement (384-dim vs hardcoded 256; no-op on L2-normalised corpus). Revisit only if the corpus geometry changes. Correctly documented; no action. |

## 6. External blockers (`external`)

| # | Entry | Blocked on |
|---|---|---|
| E-1 | ComfyUI integration | No ComfyUI service on `visionclaw_network` (builtin path needs heavy image rebuild + source hash). |
| E-2 | ~~GitHub enrichment~~ **DONE 2026-07-22** | Token validated live (HTTP 200); present in agentbox runtime env + .env (0600); `github_enrichment = true` (boot apply-class — active at next restart). Follow-up: swap broad-scope classic PAT for a fine-grained read-only one. |
| E-3 | Ollama sidecar | Confirm host ollama on :11434; sidecar-off is correct while absent. |
| E-4 | Nagual QE toolchain | Upstream sqlx 0.9 `SqlSafeStr` compile error. Wait or pin. |
| E-5 | ~~VisionClaw self-hosted GPU runner offline~~ **RESOLVED 2026-07-24** | The non-blocking GPU CI job queued indefinitely (`[self-hosted, gpu]` runner not registered) — 15h/3h zombie runs. Operator decision: the GPU crates compile+run on the CUDA host during the normal build/deploy flow (authoritative validation), so the CI job added nothing at deploy time. **Job removed** (VisionClaw 5d28071c2). Re-add only if a GPU runner is registered and real-hardware CI is wanted. |

## 7. Cleanly deferred (frozen — do not reopen without a new ADR)

ADR-073..085 window (Nostr relay federation mesh, forum extraction, website-kit cutover — frozen by closeout 2026-07-03, except ADR-074/075/076/077 which were never frozen), ADR-122/123 (two-speed writeback routing, voice sign-off), RVF file store (KNOWN_ISSUES AGENT-001 — honest "not implemented"), XR APK cross-build + LiveKit Android AAR (sprint-scale, PRD-008 §5.5).

---

*Done and removed from this register (2026-07-22): C1 immersive-tree deletion, C2 telemetry sink, C3 SPARQL clamp, C4–C6 dead protocol encoders, C7 condense scheduler, crashbug local purge, jss worktree removal, RVF orphans, 33 merged branches, the full §1 doc banner sweep, ADR-113/115–120/131/132, three P0 evidence files. See ADR-131 and `git log` for receipts.*
