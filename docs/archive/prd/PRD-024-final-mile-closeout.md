# PRD-024 — Final-Mile Closeout

**Status:** Accepted — 2026-07-22
**Owner:** Operator (Dr J. O'Hare) + agent swarm, alternating
**Governed by:** [ADR-133](../adr/ADR-133-final-mile-sprint.md) (sprint mechanics), [ADR-131](../adr/ADR-131-doc-drift-reconciliation-2026-07.md) (reconciliation baseline)
**Work register:** [TODO-unified.md](../TODO-unified.md) — the single combined VisionClaw + agentbox register this PRD drains
**Bounded context:** [ddd-final-mile-closeout-context.md](../ddd/ddd-final-mile-closeout-context.md)

---

## 1. Problem

The 2026-07-22 doc-drift audit (ADR-131) and the agentbox 2026-07-15 system audit converge on one diagnosis: **almost nothing is secretly broken — the unfinished work clusters on the seam between "implemented" and "observed live".** Code exists, gates exist, receipts exist. What is missing is canaries firing in real sessions, consumers reading real aggregates, posture decisions being dated, and the last dead surfaces being deleted rather than deprecated in place.

This PRD closes that seam. It is a *closeout* PRD: its success state is an empty §1–§3 of TODO-unified.md, not a new feature.

## 2. Goals

| G | Goal | Measured by |
|---|---|---|
| G1 | Every `live-session` canary fired or reclassified `external` | Zero `pending-live-session` tags in gap-close evidence except externally-blocked ones |
| G2 | Every `posture` surface carries a dated decision (open, closed, or re-deferred) | TODO-unified §4 empty or every row re-dated ≥ sprint start |
| G3 | Every `code-gap` in TODO-unified §2 closed or explicitly re-frozen with an ADR pointer | §2 empty |
| G4 | Zero deprecated-in-place code surfaces (XR residue, dead branches, unwired gates) | Falsification greps in §7 all return clean |
| G5 | The audit's projection principle ("a source of truth exists but nothing projects it") rolled out to SK-2/MCP-1/MCP-2 | agentbox validator green; skill roots = 1; mcp.json is the projected source |

## 3. Non-goals

- Reopening anything frozen by the 2026-07-03 closeout (ADR-073..085 window, ADR-122/123) — needs a new ADR, not this sprint.
- The XR APK cross-build and LiveKit Android AAR (sprint-scale features, stay in PRD-008).
- RVF file store implementation (KNOWN_ISSUES AGENT-001 stays honest).
- New product features of any kind.

## 4. The tick-tock protocol

The sprint alternates **ticks** (agent swarm, autonomous, hours) and **tocks** (operator, decisions + live observation, minutes-to-one-session each). The contract, enforced by ADR-133:

- A tick may **implement, document, and stage** — it may never promote a maturity tier that requires live observation.
- A tock is a **bounded, enumerated menu** — every tock below lists its exact decisions/actions so the operator never has to rediscover context.
- Evidence flows one way: tocks produce observations → the next tick files them as evidence and promotes tiers.
- A blocked item never stalls the alternation: it moves to the correct state lane in TODO-unified and the sprint continues.

### Tock 0 — keystone (operator, ~30 min, unblocks everything)

1. `tmux send-keys -t 6 './scripts/launch.sh up dev' Enter` — bring the backend up; verify/add the `visionclaw-server` network alias on `visionclaw_network`; confirm `curl http://visionclaw-server:4000/api/health` from agentbox. (K-1)
2. Set `GITHUB_TOKEN` (E-2). Optionally confirm host Ollama :11434 (E-3).
3. Populate `[sovereign_mesh.relay] allowed_pubkeys` with operator + phone npubs (T-1 step 1).
4. Authorise: remote `crashbug` deletion (T-5), MCP-3 secrets fix, and the Tick-1 menu below.

### Tick 1 — mechanism rollout (swarm)

C-6 GATE-1 schema fix · C-7 MCP-3 secrets · C-5 SK-2/MCP-1/MCP-2 projection rollout · C-8 env consolidation execution · C-10 minor follow-ups · K-2 canary registration sweep (4000 now answers) · stage T-6 image-rebuild payload · push remote crashbug deletion if authorised.

### Tock 1 — first live session (operator, one session + one host op)

1. Approve and run the agentbox image rebuild (T-6, ~15 min) — activates the condense scheduler and sweep/distill supervisord entries.
2. Drive **one live session** exercising: voice-intent call, a MAST-tagged failure, a zero-tolerance block/release, CTC fields, provenance URN resolve, DID mint (L-1). Add a second model family to the candidate pool, then trigger a cross-model verification (L-2).
3. Fire the two VisionClaw falsification probes: un-LIMITed >10k SELECT (L-3) and induced seed-failure for `fail_open_count` (L-4).

### Tick 2 — evidence filing (swarm)

File canary receipts; promote tiers in all gap-close evidence files (`pending-live-session` → `integrated`/`released` where fired); close RES-d (canon pinning now reachable); update TODO-unified; prepare the Tock-2 decision briefs (one page each: XR residue, AUTH-001, tree-search-coder, remaining postures).

### Tock 2 — decision batch (operator, 4 decisions)

1. **T-2 XR residue:** keep the Quest 3 browser-AR path (`quest3AutoDetector`) or delete the entire remaining XR surface.
2. **T-3 AUTH-001:** merge four-tier RBAC from `sprint-3/jss-cut-scaffold` or close as banner-resolved.
3. **C-4 tree-search-coder:** author it or disarm the gate.
4. **T-4 held surfaces:** re-date or flip each (multi-user, git pods, payments, OIDC, pod MCP, kernel pip; mobile bridge enable if T-1 chain complete).

### Tick 3 — decision execution (swarm)

Execute the four Tock-2 outcomes (C-1, C-2, C-4) · C-3 SQLite backup workstream · C-11 branch graveyard triage · C-9 GPU wrappers if approved · full verification gate (cargo check + test, client tsc + vitest, agentbox node suite, validator) · docs alignment for everything above.

### Tock 3 — physical validation (operator, when hardware is to hand)

L-5 Quest 3 on-device session (Godot APK canaries) · L-6 mobile bridge on phone (Amethyst note-to-self thread observed). These may trail the sprint without blocking closeout — they reclassify to `external (hardware access)` if the headset is unavailable.

### Tick 4 — closeout (swarm)

Final evidence files · anomaly-register close-set · CHANGELOG entry · ADR-133 closure amendment with the exit-criteria checklist below · TODO-unified reduced to §5–§7 (clock, external, frozen).

### Async lane (no tick/tock — the clock)

D-1: when the Wilson floor clears, `aggregate-effectiveness` dry-run → flip `feed_retrieval` → observe → flip `feed_routing`. Any tick may check the floor; flipping requires one tock nod.

## 5. Acceptance criteria

- [ ] TODO-unified §1 (keystone), §2 (code-gap), §3 (live-session) empty; §4 rows all dated ≥ 2026-07-22.
- [ ] Zero `pending-live-session` tags in `docs/gap-close-evidence/` + `agentbox/docs/reference/gap-close-evidence/` except entries reclassified `external` with a named blocker.
- [ ] agentbox validator exits 0 on HEAD; exactly one skill root; `skills/mcp.json` consumed by the entrypoint; codebase-memory registered or mandate struck.
- [ ] `.mcp.json` contains no plaintext secrets and is mode 600.
- [ ] Full verification gate green: `cargo check --workspace`, `cargo test`, client `tsc --noEmit` + vitest, agentbox `node --test`.

## 6. Falsification

This PRD is falsified as "closed" if any of the following return non-clean after Tick 4:

```bash
grep -rn "pending-live-session" docs/gap-close-evidence agentbox/docs/reference/gap-close-evidence | grep -v "external"
grep -rn "immersive/\|VRGraphCanvas" client/src                      # and, if T-2 = delete: quest3AutoDetector\|vircadia
git branch --list crashbug 'docs/neo4j*'                             # must be empty (tags archive/* may exist)
node agentbox/scripts/agentbox-config-validate.js; echo $?           # must be 0
ls -l agentbox/.mcp.json | grep -v '^-rw-------'                     # must be empty (mode 600)
```

## 7. Open questions (carried, not blocking)

1. Does the Godot client fully replace the deleted browser VR path for demo scenarios, or does T-2 need a "keep until first APK demo" clause?
2. Payments counterparty: the trigger condition ("a concrete counterparty exists") has no watcher — should a tock add one?
3. Should the `data-floor` flip (D-1) get its own falsification statement before `feed_routing` goes live, given it changes agent routing behaviour?
