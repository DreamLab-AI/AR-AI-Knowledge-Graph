# Obsidian migration close-out — 2026-09-02

Queen: Fable 5.1. Workers: 10 Opus/Sonnet agents on the ruflo mesh (research ×2,
implementers ×6, ADR verifier, physics diagnosis). Wall time about 6 hours.
Governing document: [`docs/VAULT-corpus-format.md`](../VAULT-corpus-format.md) (1.4.1).
Ledger: ADR-2040, ADR-2041, ADR-2042; agentbox ADR-2028, ADR-2029.

## What changed

| Repo | Merge | Content |
|---|---|---|
| VisionClaw | `619b439be` → `main` | vault readers/writers behind one `PageMeta` parser; Obsidian shortest-path link resolution with identity relative to the matched base path; `graphs.logseq` → `graphs.knowledge` (alias one release); `crates/vault-migrate`; GitHub path percent-encoding; docs sweep; token env `PRIVATE_REPO_GITHUB_PAT` |
| agentbox | `8980e59a0` → `main` | `[vault]` path authority (root, pages, format, tui, working, transcripts); Rune 1.4.0 via `lib/rune.nix` behind `[vault].tui`; tmux window 9 "Notes"; manifest catalogue split boot/rebuild |
| jjohare/visionGraph | new private repo, `main` at `fabcdbcc9` | history-preserving `git filter-repo` split of the converted corpus: `knowledge/`, `working/`, `transcripts/`, publish pipeline ported (`_misc/` excluded, `/notes` frozen) |
| jjohare/logseq | `obsidian` at `4f233f321` | archived converted source; `main` untouched |

Owner decisions taken during the day, all recorded in RuVector
`project-state`: branch-first clean cut → revised to the `visionGraph` split;
vault root = `knowledge/`; validation by shadow sync; legacy tolerance kept
until first clean sync; image rebuild after merge; `_misc/` unpublished;
`/notes` frozen.

## Validation evidence

| Run | Nodes | Edges | `page` nodes |
|---|---|---|---|
| Original baseline: old binary, Logseq `main` | 13,164 | 145,692 | 382 |
| Control: pre-fix binary, Logseq `main` | 13,351 | 156,153 | 567 |
| Pre-fix binary, converted branch | 13,351 | 156,101 | 568 |
| Link-fix binary, `visionGraph` | 13,162 | 145,474 | 378 |
| Final binary, `visionGraph`, zero fetch errors | 13,165 | 145,561 | 381 |
| Production sync after the owner's rebuild (`.env` flipped) | 13,165 | 153,875 (incl. inferred) | 381 |

Conversion proved format-neutral by the same-binary control (identical node
sets bar three `title`-driven labels). Converter: 70 unit + 16 integration
tests; `--check` clean on both corpora. Rust: workspace check clean, 1,125
lib tests, one inherited failure. Client: `tsc` clean, vitest 68 files / 758.
Pipeline: 58 pytest, `parse_corpus` public count 8,433 (the pre-split baseline).

Headset (VIVE Pro on HP-Desktop): client resynced from `main`, gdext rebuilt
(218 tests), full suite restart; OpenXR FOCUSED at 90 fps, topology 2,467
nodes / positions 13,165. Multi-client drag verified from the server side
(3,268 drag updates on `Ethereum`, pinned 360 units from start). Recordings:
`xr-capture-2026-09-02.mp4` (before the swarm) and
`xr-capture-beams-2026-09-02.mp4` (swarm active), both 5 min 2560×1440@30.

## Defects found and fixed

1. Page identity was the bare basename in both listing paths: 288 basename
   collisions, 254 of them the intended main↔working twin join, 34 distinct
   pages silently merged. Fixed: identity relative to the matched base path;
   Obsidian shortest-path wikilink resolution (`vault/link.rs`).
2. Converter wrote the identity path into `title:` on 223 pages. Fixed:
   `title` is display-only; echoes removed on rerun; a title the page's H1
   confirms (`A/B Testing`, `TCP/IP`) is kept and carried on fresh conversion.
3. Converter refused to complete the rename of zero-byte files (12 journals).
4. Three outliner pages whose only marker was a body-level `- public:: true`
   would have gone private under the strict gate; converter now promotes
   once (`public_promoted_from_body`).
5. `DecisionElevation` drafts carried no public marker at all; the OMS
   amendment path appended `rel::` lines after the body (Invariant 1). Both
   writers now emit frontmatter.
6. Publish pipeline used a non-recursive glob: the 13 namespace pages moved
   into folders vanished from the site walk. Fixed with a four-cell proof
   (byte-identical page sets on both corpora).
7. GitHub fetch: filenames with a literal `%` never fetched (400 / silent
   404 for a decoded name). Fixed at URL construction, one module.
8. GPU physics: 316 isolated nodes ran away in an exact sphere at
   max-velocity because the peripheral-shell target was the live AABB they
   dominated, and the soft bound saturates at `max_force`. Fixed
   (`9423abdb3`); GPU re-verification owed after the next relaunch.
9. agentbox agent-events: emit route rejected all numeric ids; forwarder to
   VisionClaw never started. Fixed (`d392bd4c2`), live after rebuild.
10. agentbox manifest reader leaks a trailing comment containing `"` into the
    value (`manifest-loader.js:35`); worked around for `[vault].tui`,
    recorded as a deferred fix (`toolchains.codebase_memory` still affected).

## Open items (tracked in `docs/TODO-unified.md` C-14)

VisionClaw NIP-98 double validation on write routes; no `agent_list`
provider behind the bots relay; `get_full_path` prefix heuristic; the
self-contradictory `dag_rank` test; diagram PNGs; ADR-2040 tolerance removal
at its trigger; `/notes` Obsidian-native rebuild; pipeline `iri_integrity` /
`DUPLICATE_IRI` reds; one corrupted `title:`; `licensing/NOTICE` dangling
reference; hard positional clamp and connected-only AABB in the GPU kernel.

## Owner steps remaining

`./agentbox.sh rebuild` on the host (bakes Rune, the new entrypoint and the
agent-events fixes); optionally archive `jjohare/logseq`; open the host
checkout's `visionGraph/knowledge` in desktop Obsidian.
