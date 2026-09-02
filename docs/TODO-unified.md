# Unified TODO - VisionClaw + agentbox

**Status:** Living document, single combined register. Slimmed **2026-09-02**: resolved rows removed with evidence (bottom), the board re-ordered on the assumption that the sovereign mesh WILL be enabled with a second agentbox node on HP-Desktop (peer reachable from machinelearn over the `10.10.10.0/30` rail; see agentbox `docs/developer/hp-peer-node.md`). The 2026-08-31 long form is in git history (`git show e8fab1098:docs/TODO-unified.md`).
**Governed by:** [PRD-024 Final-Mile Closeout](prd/PRD-024-final-mile-closeout.md), [ADR-133](adr/ADR-133-final-mile-sprint.md)
**Rule:** one unblock state per entry; closures cite evidence; remove rows when done. Open count: **34 before, 25 carried + 15 new = 40 after** (7 removed, see bottom).

| State | Meaning | Unblocks when |
|---|---|---|
| `code-gap` | Code never written or half-shipped | An agent tick writes it |
| `ops-action` | Infrastructure/config action, minutes of work | Operator (or authorised tick) performs it |
| `live-session` | Code+test proven; needs observation on real traffic | An operator tock drives the session |
| `posture` | Deliberately held closed; a decision, not a gap | Operator decides (deciding "stay closed" also closes it; re-date) |
| `data-floor` | Waiting on samples/corpus to accumulate | The clock; check floor, then flip |
| `external` | Blocked outside both repos | Upstream/network change |

## Next items, priority order (dependency first, then value, then cost)

| # | ID | State | Item | Size | Why here |
|---|---|---|---|---|---|
| 1 | M-1 | `live-session` | HP-Desktop agentbox peer node: DONE 2026-09-02 (fresh `did:nostr`, June volumes kept, node pubkey recorded in RuVector `hp-desktop-agentbox-peer-node-2026-09-02`); remaining is observation of the NIP-98 door under real traffic | S | Landed today; row closes at the first real cross-node exchange |
| 2 | G-5 | `code-gap`+`ops-action` | Publisher-key split (ADR-040 D3, AB ADR-2027): `[sovereign_mesh.operator].pubkey_hex` (agentbox.toml:117) still carries the visionclaw-server key; custody/rotation per secret | M | Must land before any relay is exposed; compromise window is one build cycle, so ride the M-3 rebuild |
| 3 | G-6 | `code-gap` | Abort boot on failed key derivation instead of minting `did:nostr:local` (`management-api/lib/agent-identity.js:184` still fail-open) | S | Quick win; stops the new HP node minting a placeholder DID into the mesh |
| 4 | M-2 | `code-gap`+`ops-action` | ml relay expose for the rail: `[sovereign_mesh.relay]` `bind` 127.0.0.1 → in-container 0.0.0.0, `expose = true`; `flake.nix:2269-2270` publishes only `127.0.0.1:7777`, needs the rail address (ml side of 10.10.10.0/30); add the HP node pubkey to `allowed_pubkeys` (both nodes); keep `ingress_policy = "allowlist"` | S | The concrete config the rebuild bakes; scoped to the rail, not the LAN |
| 5 | T-1 | `posture` | Relay exposure chain decision: expose → mobile bridge (`mobile_bridge.enabled = false`). Allowlist is now baked AND live (nostr-pod-bridge env `AGENTBOX_ALLOWED_PUBKEYS` populated) | S | Formal decision M-2 executes; decide once with rail-only scope so it is not reopened per node |
| 6 | M-3 | `ops-action` | Agentbox image rebuild round 3 on ml (bakes M-2, G-5, G-6, `[mesh]` mode); repeat on HP | S | ~15 min host op that gates every downstream mesh item |
| 7 | M-4 | `code-gap` | Federation Phase 3 is config-only: `[mesh]` `peer_relays`/`federated_kinds`/`allowed_remote_dids` have no code consumers; no `[federation]` table exists (comments only); the only fan-out lever is `AGENTBOX_RELAY_FANOUT` (`management-api/server.js:1308`, `mcp/nostr-bridge/relay-consumer.js`) which bakes `off` although `agentbox.toml:161` says `bidirectional` (`flake.nix:3129`, trace the override). Wire RelayConsumer peer subscription + fan-out per PRD-010/ADR-073 | L | The code that makes two relays talk; can start in parallel with 2-6, cannot be verified before 6 |
| 8 | G-3 | `code-gap` | Cross-repo federation CI fixture (AB ADR-2025): sha12/hex identity byte-parity test in both repos' CI (none in `tests/contract/`, no `sha12` in either workflows dir) | S | Cheap insurance before two nodes exchange identities under M-4 |
| 9 | M-5 | `posture`+`ops-action` | Dream sovereign-mesh slot: nostr-bridge path-deps escape the per-night clone to `~/dream-annexe/{nostr-rust-forum,solid-pod-rs}`; provision the siblings on the HP annexe or re-slot | S | Unblocks the nightly mesh slot on the new node; decision only |
| 10 | L-6+L-1 | `live-session` | Mobile bridge e2e on phone (Amethyst+Amber, note-to-self thread) and the envelope canaries (voice-intent, MAST tag, 31403 release, CTC cost, URN resolve, DID federation proof) | M | First real traffic through the exposed relay; observation, no code |
| 11 | G-4 | `posture`→`code-gap` | Session-mirror egress boundary (AB ADR-2026): `config/hooks/nostr-live-mirror.cjs` has no redaction and `AGENTBOX_LIVE_MIRROR=0` fails open (line 355) | S | Once transcripts cross nodes the sovereignty hole widens; decide redaction before mesh traffic |
| 12 | DR-1+DR-2 | `posture`+`ops-action` | Ratify VisionFlow ADR-0057 and dream-machine "mock Darwin is smoke-class"; run `/dream` on nostr-rust-forum now the bench fix is committed (7-night INCONCLUSIVE streak) | S | Independent of the mesh, cheap, restores nightly engine value |

## Carried open entries (compressed)

**Mesh follow-ons:** M-6 `live-session` (S) verify HP↔ml relay handshake and allowlist rejection of an unlisted key after M-3. LM-1 `posture` (S) Loom confidence-surfacing contract, analysis in flight.

**ADR governance (§9):** G-1 `code-gap` (S) dev-auth build guard, VC ADR-2037 still `implementation_status: none`. G-2 `code-gap` (S) boot-profile assertion, ADR-2038 `none`; no `PUBKEY_VISIBILITY_FILTER` check in `main.rs`/`rbac_gate.rs`. G-7 `code-gap` (L) provenance store backup + erasure path (ADR-2016/2017). G-8 `code-gap` (M) per-pubkey admission ahead of `ReplayCacheFull` (`src/utils/nip98.rs:171` has TTL cache only). G-9 `ops-action` (S) Loom `:8084` LAN-reachability audit. G-10 `posture` (M) LAN-door threat model, nine `0.0.0.0` publishes, raw CDP 9222 first. G-11 `posture` session-bearer sunset, blocked on the React client signing. G-13 `code-gap` (S) PTX `.version` rewrite fails on unexpected headers (`ptx_loader.rs:317`). G-14 `code-gap` (L) supersession graph, 42 records still `supersedes: []`; per-repo ID namespacing. G-15 `posture` status-axis lattice.

**Live-session (§3):** L-2 diversity canary (needs a second model family). L-3 ADR-117 clamp fire. L-4 ADR-119 telemetry fire. L-5 on-headset XR (folds X-6; later ADR-140 phases not owed).

**Posture (§4):** T-4 held surfaces: git pods/host gateway, payments (until counterparty), Solid OIDC issuer, pod MCP, kernel pip; re-date.

**Data-floor (§5):** D-1 flip `feed_routing` after the observation window (`feed_retrieval = true` since 2026-08-31, agentbox.toml:415; window length undefined in LEARNING-memory.md:169, define it).

**External (§6):** E-1 ComfyUI, no container on `visionclaw_network`. E-4 Nagual QE, upstream sqlx 0.9 `SqlSafeStr` (no pin in `lib/nagual-qe.nix`).

**Docs (§12):** DOC-1 `rest-api.md` 34 broker/workflow phantom mentions. DOC-2 kernel count: 83 `__global__` vs `actor-hierarchy.md:601` "37". DOC-3 `installation.md` docker-compose v1 narrative (lines 66-92) vs `launch.sh`. **DOC-4 (new, S)** ADR-2002 `activation_status: staged` but AoE runs `--auth token`; flip to active and reconcile with AB-2009.

**Branches:** C-11 `ops-action` (S): pool regenerated, 27 local branches = 24 locked `/batch` worktrees (antigravity/codex/deepseek/loom x6) + main/obsidian/logseq-archive; the three VALUABLE branches are local-deleted, remote-kept (`refactor/kg-node-rename` also tagged `archive/kg-node-rename`), `report/soundings-qe-audit`, `impl/khive-investigation` still need remote keep/drop.

**Obsidian corpus residue (new, from C-14 close-out):** V-1 `code-gap` (M, visionGraph) pipeline reds: `DUPLICATE_IRI` (`Open Source AI.md`/`Open-Source AI.md`), 102 orphan refs beyond baseline, ADR-NG-002 P2 Loom reload trigger, `/notes` SPA rebuild with Obsidian tooling. V-2 `live-session` (S) GPU re-verify of 9423abdb3 (committed 12:28Z, after the 11:11Z dev relaunch): isolated nodes settle r≈320, spatial-grid warnings gone; follow-ups `integrate_pass_kernel` clamp + connected-only AABB. V-3 `code-gap` (S) NIP-98 verified twice on `POST /api/bots/update` → `401 Token replayed`; session realm workaround. V-4 `code-gap` (S) `GitHubClient::get_full_path` prefix heuristic; `dag_rank_tests::directed_hierarchy_relation_accepts_only_class_subsumption` self-contradictory (`force_compute_actor.rs:4563`), needs an owner; ADR-2040 tolerance removal at review trigger. V-5 doc (S): diagram PNGs 01/04/06/07 still Logseq-labelled; `Two Heads Are Better Than One.md` corrupted `title:`.

**Frozen (§7), unchanged:** ADR-073..085 window, ADR-122/123, RVF file store, XR APK cross-build. §11 design open-questions remain pointers.

## Removed as resolved (with evidence)

| ID | Evidence |
|---|---|
| T-5 remote `crashbug` delete | `git ls-remote --heads dreamlab-github` lists 10 heads, none `crashbug`; tag `archive/crashbug` present |
| T-6 rebuild round 2 | Live: `ffmpeg` resolves to `/nix/store/...-ffmpeg-8.1.2-gpuwrapped`; `/opt/agentbox/skills/tree-search-coder` present; baked entrypoint carries `VAULT_ROOT`; `rune-1.4.0` in `/nix/store`; tmux window 9 "Notes"; baked `management-api/{server.js,routes/agent-events.js}` sha1-identical to d392bd4c2. Round 3 is M-3 |
| G-12 AoE token activation | Running process: `aoe serve --auth token --behind-proxy --host 127.0.0.1 --port 9095` (agent-of-empires-1.13.2; `flake.nix:1993`). Residual doc flip is DOC-4 |
| C-14 Obsidian migration owner steps | `.env:12,15` `GITHUB_REPO=visionGraph`, `GITHUB_BASE_PATH=knowledge/pages,working/pages`; submodule bump cfa20a73b, merge 619b439be, e8fab1098; `visionclaw_container` created 2026-09-02T11:11Z after the 10:37Z merge; agentbox rebuilt (see T-6 row); `gh repo view jjohare/logseq` → `isArchived: true`. Residue split into V-1..V-5 |
| C-13 logseq narrativegoldmine follow-ups | `jjohare/logseq` archived (`isArchived: true`); publish pipeline now lives in `jjohare/visionGraph`; live residue (ADR-NG-002 P2, IRI reds) carried as V-1 |
| C-9 / X-1..X-5 / T-2 / T-3 (already struck in source) | Confirmed done per source; dropped from the slim board as non-open |
| dreamlab-ai-website PR #49 kit-pin-guard CI | Merged 2026-09-02 (lead context); was not a register row, noted so it is not re-added |
