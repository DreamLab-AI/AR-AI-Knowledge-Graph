---
id: ADR-2042
title: "`vault-migrate` is the sole Logseq→Obsidian converter — deterministic, output-dir by default, preserve-and-report"
date: 2026-09-02
decision_status: proposed
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit:
verified_paths: [crates/vault-migrate, Cargo.toml, docs/VAULT-corpus-format.md]
owner: jjohare
review_trigger: the in-place conversion of the corpus repo is committed, after which the crate is kept only as the round-trip/no-op checker
repo: visionclaw
domain: VAULT-corpus-format
lineage: corpus-repo `obsidian_to_visionflow.py` (Obsidian→VisionFlow JSON-LD injector, the reverse direction), PRD-LCR-01 (swarm refactor of the same corpus)
---

# ADR-2042 — `vault-migrate` is the sole converter

## Context

The corpus is the owner's private graph (never pushed by agents). A
conversion that guesses, deletes, or edits in place by default would be an
unrecoverable action on that graph. The transformation is mostly mechanical
(leading property block → frontmatter, `___` → folders, a handful of body
rewrites) with a small tail of constructs that have no faithful Obsidian
equivalent (13 dangling block refs, 193 pages with body-level properties).

## Decision

1. One Rust binary, `crates/vault-migrate`, in the server workspace. No LLM,
   no network. Rules are exactly the V2/V3 tables in the governing doc.
2. Modes: `vault-migrate <logseq-graph-dir> --out <vault-dir>` (default,
   creates `<vault-dir>` and never touches the source) and
   `vault-migrate <dir> --in-place` (explicit; refuses to run on a dirty git
   working tree unless `--allow-dirty`).
3. Output includes a starter `.obsidian/` (`app.json` with `newLinkFormat:
   "shortest"`, `attachmentFolderPath: "assets"`; `daily-notes.json` with
   `folder: "journals"`, `format: "YYYY-MM-DD"`; `core-plugins.json`) and a
   `.gitignore` excluding `.obsidian/workspace*.json` and `.obsidian/cache`.
4. Unknown or unconvertible constructs are preserved verbatim and counted in
   `vault-migrate-report.json` (per-rule counts, per-file list of leftovers).
   The binary exits non-zero only on I/O failure, never on leftovers.
5. Idempotent: running on its own output is a byte-identical no-op and the
   report shows zero conversions (`--check` mode exits non-zero if it would
   change anything — the CI hook for the converted repo).
6. `--dry-run` prints the report without writing.

## Consequences

- The owner decides when to apply in place; agents run output-dir mode.
- The report is the evidence artefact for EXP-V04/V05.
- The crate is small (frontmatter emitter, property-block parser, path mapper,
  body rewriters) and unit-tested per rule with fixtures under
  `crates/vault-migrate/tests/fixtures/`.

## Verification

2026-09-02 on the `obsidian` branch: `cargo test -p vault-migrate` — 66 unit
+ 16 integration tests green (per-rule fixtures under
`crates/vault-migrate/tests/fixtures/logseq-mini` against a byte-exact
`expected-vault` golden, two-run byte identity, no-op on own output,
`--check` drift detection, source-graph never modified, `--in-place`
namespace rename). Real runs, output-dir mode only, source graphs
fingerprint-identical before and after:
`mainKnowledgeGraph` → `/home/devuser/workspace/vault` in 2.6 s
(8,655 pages, 8,634 converted; public 8,615, namespace 201, journals 308,
tasks 239, assets 168; leftovers: 14 block-ref files, 6,541 body-property
files, 4 SCHEDULED/DEADLINE journals; one unreadable root-owned tool file
reported); `workingGraph` → `/home/devuser/workspace/vault-working` in
0.65 s. `--check` on both outputs exits 0. Three survey rows in the
governing doc were corrected from the report (no page embeds exist; body
properties are the corpus's inline relation fields, 6,541 files not 193;
one task page is fully fenced). `activation_status: staged` until the
owner applies `--in-place` to the corpus repo.
