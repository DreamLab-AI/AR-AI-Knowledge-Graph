---
id: ADR-2068
title: Remove superseded vendored assets and dead config
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any future need to run a standalone JavaScript Solid Server sidecar instead of the embedded solid-pod-rs library, or any code that starts reading `ontology_physics.toml` by path
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2068 — Remove superseded vendored assets and dead config

## Context
Phase 1 diagrams VC-26.1/26.2 flagged `JavaScriptSolidServer/` (63 MB) as a
vendored copy of the third-party `github.com/JavaScriptSolidServer/JavaScriptSolidServer`
upstream project, superseded by the embedded Rust `solid-pod-rs` behind the
`solid-pod-embed` feature (see ADR-2067). Its only in-repo mention outside its
own tree, docs, and archive was a doc-comment URL at `src/utils/nip98.rs:5`.
Phase 1 diagram VC-20.11 flagged `ontology_physics.toml` (repo root) as
matched by no path literal anywhere in the Rust codebase — the live toggle is
the boolean `Settings.ontology_physics` field
(`src/settings/models.rs:63`).

## Decision
Both are deleted:
- `JavaScriptSolidServer/` — REMOVE, not ARCHIVE. `docs/archive/` exists for
  our own superseded documents, not for a 63 MB vendored upstream server that
  remains fully recoverable from its own public upstream repository at any
  time; archiving it in-tree would only keep dead weight the archive
  convention was never meant to carry. The doc-comment URL in
  `src/utils/nip98.rs:5` is left untouched — it references the upstream
  project by URL, not this vendored copy.
- `ontology_physics.toml` (repo root) — REMOVE. No Rust path literal reads it;
  `data/ontology_physics.toml.example` (a distinct file) and
  `src/settings/models.rs`'s `Settings.ontology_physics: bool` field plus its
  routes under `src/handlers/api_handler/ontology_physics/` are untouched —
  they remain the live toggle and are out of this ADR's scope.

## Consequences
Repository size drops by ~63 MB with zero code impact — nothing referenced
either path outside comments and documentation. A future embedded-vs-external
Solid pod redesign would start from `solid-pod-rs` and the JSS upstream repo
directly, not from this vendored, increasingly stale copy. Anyone looking for
a file-based ontology-physics toggle must use the `Settings.ontology_physics`
boolean and its API routes instead of a root-level TOML file that no code
ever read.

## Verification
Ran on the uncommitted working tree above `verified_commit`; must be
re-verified at the landing commit.

```
$ du -sh JavaScriptSolidServer
63M     JavaScriptSolidServer

$ grep -rln "JavaScriptSolidServer" --include="*.rs" --include="*.toml" .
src/utils/nip98.rs   # doc-comment URL only, left untouched

$ grep -rln "JavaScriptSolidServer" docker-compose*.yml
(no output — not referenced by any compose file)

$ grep -n "JavaScriptSolidServer" .gitignore
22:JavaScriptSolidServer   # already gitignored — untracked in git history, confirming it was never canonical repo content

$ rm -rf JavaScriptSolidServer

$ grep -rn "ontology_physics\.toml" --include="*.rs" .
(no output — no Rust path literal reads it)

$ grep -rln "ontology_physics\.toml" .
CHANGELOG.md
docs/how-to/development.md
docs/diagrams/visionclaw/20-ontology-pipeline-oxigraph-whelk.md
data/ontology_physics.toml.example   # distinct file, untouched

$ rm ontology_physics.toml

$ grep -n "ontology_physics" src/settings/models.rs
63:    pub ontology_physics: bool,   # untouched — the live toggle

$ cargo check -p visionclaw-server
    error: could not compile `visionclaw-server` (lib) due to 4 previous errors
    # all 4 in src/services/owl_extractor_service.rs (unrelated ADR-2064
    # in-flight work by another lead on this shared tree — see ADR-2066
    # Verification). Neither removal in this ADR touches any Rust source, so
    # neither can be implicated; both are pure filesystem deletions with zero
    # remaining path references outside docs/archive-equivalent mentions.
```
