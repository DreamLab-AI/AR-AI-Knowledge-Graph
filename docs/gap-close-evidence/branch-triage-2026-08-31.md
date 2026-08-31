# C-11 Branch graveyard triage — VisionClaw

**Date:** 2026-08-31 · **Repo:** VisionClaw (`/home/devuser/workspace/project`, branch `main`,
remote `dreamlab-github`) · **Scope:** classify all local branches except `main`.

## Headline

123 non-`main` local branches at start. The premise ("classify remaining unmerged feature
branches") inverted on contact: **116 of the 123 are an active, _locked_ `/batch` worktree pool**,
not stale feature branches. Only **7** branches are standalone. Of those, 4 were disposed
(1 merged, 3 superseded) and 3 kept as valuable unmerged work.

- Merged into `main`: 117 (116 = locked worktree pool + `xr-vive-runtime`).
- Genuinely unmerged standalone: 6.

**No tags or deletions pushed. No remote branches touched. Nothing committed.**

## Outcome counts

| Class | Meaning | Count | Action taken |
|---|---|---|---|
| (a) MERGED | fully on main, no worktree | 1 | deleted (`git branch -d`) |
| (b) SUPERSEDED | content landed on main another way | 3 | annotated `archive/*` tag + `git branch -D` |
| (c) VALUABLE | unique unmerged content | 3 | kept |
| (d) WORKTREE POOL | locked, checked-out, cannot/should-not delete | 116 | kept — operator-gated (see below) |
| **Total** | | **123** | 4 deleted, 119 kept |

Remaining non-`main` branches after execution: **119** (116 pool + 3 valuable).

## (a) MERGED — deleted

| Branch | Commits ahead | Last commit | Note |
|---|---|---|---|
| `xr-vive-runtime` | 0 | 2026-08-22 | Fully merged into main, not bound to any worktree → safe `-d` delete. Content preserved on main; no archive tag required. |

## (b) SUPERSEDED — archive-tagged + deleted

| Branch | Archive tag | Evidence | Last commit |
|---|---|---|---|
| `archive/deprecated-docs` | `archive/deprecated-docs-2026-08-31` | Pure-deletion branch (74 files deleted, 0 added/modified). 72/74 of those files are **already gone from main** — the cleanup landed via other work. | 2025-10-23 |
| `worktree-agent-a7c66ae9b4265894b` | `archive/worktree-agent-a7c66ae9b4265894b` | Mid-flight "ADR-090 Phase A6 slice 3" (settings-repo → domain crate). Main already holds the **complete 49-file `crates/visionclaw-domain` tree** and multiple ADR-090 closeout commits ("idempotent local-file seed + dead-code purge"). Branch is a stale intermediate of finished work. | 2026-05-28 |
| `new-docs` | *(pre-existing)* `archive/new-docs` | Branch tip `6f248c0d4` is **identical** to the existing `archive/new-docs` tag (0 commits beyond). Already archived; local branch redundant. Deleted without a new tag. | 2026-01-02 |

## (c) VALUABLE — kept (unmerged, unique content; named owner needed)

| Branch | Commits | Last commit | What it holds |
|---|---|---|---|
| `refactor/kg-node-rename` | 63 | 2026-04-19 | Large `GraphNode → KGNode` unification across Rust/TS/Cypher/docs; Neo4j metadata flattening + indexes; semantic-force CUDA kernel wiring; 465 files, +52k/-4k. Substantial unmerged refactor — decide land-or-drop. Owner: jjohare. |
| `report/soundings-qe-audit` | 1 | 2026-07-13 | Soundings regions-map QE fleet audit + valuation, with report HTML and image assets (22 files). Self-contained deliverable. Owner: jjohare. |
| `impl/khive-investigation` | 1 | 2026-05-16 | khive-integration feasibility report (727-line doc + `.gitignore`). Reference investigation, not yet landed. Owner: claude-flow. |

## (d) WORKTREE POOL — operator-gated (116 branches, NOT deleted)

All 116 are checked out in **locked** worktrees under `/home/devuser/workspace/project-worktrees/`.
`git branch -d` refuses them ("cannot delete branch 'X' used by worktree at ...") and the locks are
deliberate. This is the `/batch` agent-swarm isolation pool (CLAUDE.md), six lanes:

| Lane | Count | Lane | Count |
|---|---|---|---|
| `antigravity`, `antigravity-2..29` | 29 | `gemma`, `gemma-2..11` | 11 |
| `codex`, `codex-2..29` | 29 | `loom-raw`, `loom-raw-2..14` | 14 |
| `deepseek`, `deepseek-2..29` | 29 | `ollama`, `ollama-2..4` | 4 |

All are fully merged into `main`, so their tips carry no unique content — but tearing down the pool
means removing 116 locked worktrees (`git worktree remove --force` + branch delete), which is an
infrastructure decision, not a graveyard cleanup. **Left for operator (T-5 territory):** confirm the
batch pool is idle, then bulk-remove worktrees and branches together. Doing it here risks breaking a
live swarm run.

## Reproduction

```bash
git branch --no-merged main        # 6 standalone unmerged
git worktree list                  # 116 locked pool worktrees + main
git diff main...<branch> --diff-filter=D --name-only   # supersede test for deletion branches
git tag -l 'archive/*'             # archive tags (local only, not pushed)
```
