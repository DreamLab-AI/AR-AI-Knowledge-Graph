# Archive-Cut Migration Plan — Operator Ratification

**Cut date:** 2026-08-31
**Scope:** two repos — this repo (`/home/devuser/workspace/project`) and the vendored `agentbox/` tree.
**Status:** AWAITING RATIFICATION. No moves have been executed. This document is the plan only.

## 1. Rationale

The legacy `docs/{adr,prd,ddd}` (this repo) and `agentbox/docs/reference/{adr,prd,ddd}` corpora
have drifted from the code. The living successor set is `docs/next/` — eight authority documents
(BASELINE-architecture, DATA-authority-erasure, GPU-wire-abi, IDENTIFIER-taxonomy,
IDENTITY-authority-chain, PROTOCOL-registry, SECURITY-profiles, XR-client) plus a fresh
`docs/next/adr/` ledger. We freeze the legacy trees in place under `archive/` so history and
cross-references survive, then repoint (or archive-qualify) the code comments that cite them.

The cut is a single `git mv` commit per repo so it reverts cleanly (see §6).

## 2. Exact move commands — DO NOT RUN until ratified

Run from the repo root. `git mv` preserves history; the archive dirs already exist as empty
holders, so we move the child dirs into them.

```bash
# ---- Repo A: /home/devuser/workspace/project ----
cd /home/devuser/workspace/project
git mv docs/adr  docs/archive/adr
git mv docs/prd  docs/archive/prd
git mv docs/ddd  docs/archive/ddd

# ---- Repo B: agentbox (same working tree, separate doc root) ----
git mv agentbox/docs/reference/adr  agentbox/docs/archive/adr
git mv agentbox/docs/reference/prd  agentbox/docs/archive/prd
git mv agentbox/docs/reference/ddd  agentbox/docs/archive/ddd
```

If `docs/archive/` or `agentbox/docs/archive/` do not already contain the tombstone READMEs
below, add them in the same commit (`git add`).

Commit message:

```
docs: cut legacy adr/prd/ddd to archive/ (2026-08-31)

Freezes drifted decision corpora under archive/. Living docs now in docs/next/.
Single mv commit — revert this to roll back (see docs/next/MIGRATION-plan.md §6).
```

## 3. Tombstone README content

Write one `README.md` into each archived leaf dir. Template (substitute `<KIND>` =
ADR / PRD / DDD and `<REPO>` label):

```markdown
# ARCHIVED — <KIND> (<REPO>)

**Frozen:** 2026-08-31. **Do not add or edit records here.**

These <KIND> records drifted from the code and were retired in the archive cut of
2026-08-31. They are kept read-only for history and to resolve inbound cross-references.

The living decision surface is **`docs/next/`**:
- Architecture baseline .......... docs/next/BASELINE-architecture.md
- Identity / authority chain ...... docs/next/IDENTITY-authority-chain.md
- Identifier taxonomy ............. docs/next/IDENTIFIER-taxonomy.md
- Data authority & erasure ........ docs/next/DATA-authority-erasure.md
- Protocol registry .............. docs/next/PROTOCOL-registry.md
- GPU wire ABI ................... docs/next/GPU-wire-abi.md
- Security profiles .............. docs/next/SECURITY-profiles.md
- XR client ...................... docs/next/XR-client.md
- New ADR ledger ................. docs/next/adr/

New decisions go in `docs/next/adr/` using `docs/next/adr/TEMPLATE.md`.
See `docs/next/MIGRATION-plan.md` for the redirect table mapping legacy numbers here.
```

Six files total: `docs/archive/{adr,prd,ddd}/README.md` and
`agentbox/docs/archive/{adr,prd,ddd}/README.md`.

## 4. Redirect table — 30 most-cited legacy ADRs → successor living doc

Ranked by citation count in code comments across `src/`, `crates/`, `xr-client/`, `agentbox/src`.
"Successor" is the living doc a repointed comment should reference. Short-form numbers
(ADR-01/02/06/07/08/10/11) are zero-pad-ambiguous between the two repos' ledgers and MUST be
disambiguated by the editor before repointing (column notes the likely source).

| Legacy ADR | Cites | Legacy title | Successor living doc |
|-----------|------:|--------------------------------------------|------------------------------------|
| ADR-031 | 133 | gpu-analytics-correctness-and-wiring | GPU-wire-abi.md |
| ADR-011 (`ADR-11`) | 104 | auth-enforcement | SECURITY-profiles.md |
| ADR-130 | 75 | gap-close-visionclaw-decisions | BASELINE-architecture.md (XR refs → XR-client.md) |
| ADR-090 | 74 | hexagonal-crate-modularisation | BASELINE-architecture.md |
| ADR-141 | 66 | constrained-layout-engine-programme | GPU-wire-abi.md |
| ADR-100 | 47 | canonical-iri-and-vocabulary-alignment | IDENTIFIER-taxonomy.md |
| ADR-10 | 42 | (contracts) enterprise event/agent contracts | PROTOCOL-registry.md |
| ADR-059 | 40 | bidirectional-agent-channel-server | PROTOCOL-registry.md |
| ADR-098 | 34 | semantic-constraint-path-reuse | GPU-wire-abi.md |
| ADR-070 | 34 | cuda-integration-hardening | GPU-wire-abi.md |
| ADR-050 | 34 | pod-backed-kgnode-schema / decision-elevation | DATA-authority-erasure.md |
| ADR-049 | 34 | insight-migration / bitemporal-facts (agentbox) | DATA-authority-erasure.md |
| ADR-01 | 33 | (agentbox) nixos-flakes / supervisor baseline | BASELINE-architecture.md |
| ADR-124 | 25 | smart-contract-features-web-contracts | PROTOCOL-registry.md |
| ADR-048 | 22 | dual-tier-identity-model | IDENTITY-authority-chain.md |
| ADR-014 | 19 | semantic-pipeline-unification / graph ingress | BASELINE-architecture.md |
| ADR-099 | 17 | reasoner-posture-whelk-el-primary | BASELINE-architecture.md |
| ADR-101 | 16 | triple-store-migration-framework | IDENTIFIER-taxonomy.md |
| ADR-06 | 16 | (agentbox) immutable-runtime-bootstrap | SECURITY-profiles.md |
| ADR-142 | 14 | multi-user-rbac | SECURITY-profiles.md |
| ADR-110 | 13 | agentic-actors-acsp-control-surfaces | BASELINE-architecture.md |
| ADR-060 | 12 | pubkey-filtered-binary-encoder | PROTOCOL-registry.md |
| ADR-125 | 11 | did-nostr-multikey-convergence | IDENTITY-authority-chain.md |
| ADR-043 | 11 | kpi-lineage / session-identity-binding | DATA-authority-erasure.md |
| ADR-140 | 10 | xr-agent-swarm-visualisation | XR-client.md |
| ADR-02 | 10 | (agentbox) ruvector-standalone | BASELINE-architecture.md |
| ADR-114 | 9 | ontology-class-index-memory-substrate | BASELINE-architecture.md |
| ADR-037 | 9 | gap-close-agentbox-decisions | BASELINE-architecture.md |
| ADR-119 | 7 | verifiable-liveness-telemetry | SECURITY-profiles.md |
| ADR-061 | 7 | binary-protocol-unification | PROTOCOL-registry.md |

Editor rule: repoint each comment to the successor doc, **or** leave the ADR number with an
archive-qualified suffix — e.g. `ADR-031 (archived: docs/archive/adr/ADR-031-…md → GPU-wire-abi.md)`.
Repointing is preferred where a clean successor exists; archive-qualify only when the comment
documents a frozen historical decision with no live successor.

### 4a. Full citation inventory (for the repointing pass)

The complete `file:line` list of every code-comment ADR citation is large (993 citations in
`.rs/.cu/.gd/.gdshader` across 245 files; 1052 including generated TS bindings, `.godot`,
`.claude-flow` state and log files). Regenerate the authoritative list at repointing time with:

```bash
cd /home/devuser/workspace/project
grep -rnoE "ADR-[0-9]+" --include='*.rs' --include='*.cu' \
  --include='*.gd' --include='*.gdshader' src/ crates/ xr-client > /tmp/adr-citations.txt
```

Heaviest files to repoint first (citation count):
- `src/actors/client_coordinator_actor.rs` — ~24 (mostly ADR-031)
- `crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs` — ~28 (ADR-11 / ADR-099)
- `src/app_state.rs` — ~30 (ADR-11 / ADR-130 / ADR-031)
- `src/main.rs` — ~28 (ADR-06 / ADR-11 / ADR-142 / ADR-050)
- `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu` — ~28 (ADR-141 / ADR-070)
- `src/actors/task_orchestrator_actor.rs` — ~15 (ADR-031)
- `crates/visionclaw-contracts/**` — ~40 (ADR-10, generated bindings; repoint the source `.rs`,
  regenerate the `.ts`)
- `xr-client/scripts/graph_scene.gd` — ~11 (ADR-130 / ADR-140 / ADR-141)

Excluded from the repointing pass (machine state, not authored comments): `**/.claude-flow/**`,
`**/daemon-state.json`, `**/policy/state.json`, `**/pending-insights.jsonl`, `**/logs/**`.
Their ADR strings are incidental and must not be edited.

## 5. Ratification checklist

The operator reads and ticks each living doc before the cut executes. The cut is authorised only
when every box is ticked.

- [ ] `docs/next/BASELINE-architecture.md` read and accepted as the architecture authority
- [ ] `docs/next/IDENTITY-authority-chain.md` read and accepted
- [ ] `docs/next/IDENTIFIER-taxonomy.md` read and accepted
- [ ] `docs/next/DATA-authority-erasure.md` read and accepted
- [ ] `docs/next/PROTOCOL-registry.md` read and accepted
- [ ] `docs/next/GPU-wire-abi.md` read and accepted
- [ ] `docs/next/SECURITY-profiles.md` read and accepted
- [ ] `docs/next/XR-client.md` read and accepted
- [ ] `docs/next/adr/TEMPLATE.md` confirmed as the new-decision template
- [ ] Redirect table (§4) reviewed; short-form ambiguities (ADR-01/02/06/07/08/10/11) assigned
- [ ] Tombstone README text (§3) approved
- [ ] Operator authorises the `git mv` commit(s) in §2

On full sign-off:
1. Execute §2 moves + §3 tombstones as one commit per repo root.
2. Run the §4a grep, execute the repointing pass as a **separate** follow-up commit (keeps the
   pure-`mv` commit revertable).
3. Verify build (`cargo check`) and that no comment references a path that no longer resolves.

## 6. Rollback

The archive cut is a single move commit; reverting restores the legacy trees exactly.

```bash
cd /home/devuser/workspace/project
git log --oneline -5                 # find the "docs: cut legacy adr/prd/ddd" commit hash
git revert --no-edit <cut-commit>    # restores docs/{adr,prd,ddd} + agentbox/docs/reference/*
```

If the repointing pass (step 2) shipped separately, revert it first, then revert the cut commit —
order matters because the repointing commit assumes the archive paths exist. If both are reverted,
the tree returns to its pre-cut state with all citations intact. The tombstone READMEs vanish with
the revert since they were added in the same commit.

Do not `git rm` the archive dirs to roll back — always `git revert` so history stays linear and the
move is auditable.
