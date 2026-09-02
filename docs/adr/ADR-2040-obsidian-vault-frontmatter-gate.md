---
id: ADR-2040
title: "The authored corpus is an Obsidian vault; YAML frontmatter `public`/`owl-class` gate KG inclusion with bounded Logseq tolerance"
date: 2026-09-02
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: [ADR-2014]
superseded_by: []
verified_commit:
verified_paths: [src/services/file_service.rs, src/services/github_sync_service.rs, src/services/parsers/knowledge_graph_parser.rs, src/services/github/content_enhanced.rs, src/services/ontology_mutation_service.rs, src/services/decision_elevation.rs, docs/VAULT-corpus-format.md]
owner: jjohare
review_trigger: the first GitHub sync run after the corpus repo is converted in place, or 2026-12-01, whichever is earlier — at which point the Logseq `key:: value` tolerance is removed
repo: visionclaw
domain: VAULT-corpus-format
lineage: ADR-2014 (GitHub `public:: true` gate; `owl:class::` bypass), legacy ADR-050/051 (publish arbiter), PRD-LCR-01 in the corpus repo (JSON-LD canonical blocks)
---

# ADR-2040 — The authored corpus is an Obsidian vault; frontmatter gates inclusion

## Context

The authored corpus (8,638 pages) is read by GitHub sync through Logseq
conventions: a `public:: true` property line, `owl:class::`, `source-domain::`,
`elevatedFrom:: [[..]]`, namespace files named `a___b.md`. Those conventions
are scattered across five Rust files and three agentbox scripts. The owner is
moving authoring from Logseq to Obsidian, whose native metadata is YAML
frontmatter and whose namespaces are folders. Obsidian renders `key:: value`
as plain text, so continuing to write Logseq properties would make every
new page invisible to the gate in the editor the owner actually uses.

## Decision

1. The authored corpus **is an Obsidian vault** as specified in
   [`docs/VAULT-corpus-format.md`](../VAULT-corpus-format.md) §V1–V5. That
   document is the governing authority for layout, frontmatter keys, body
   dialect, and the inclusion gate.
2. The KG inclusion gate is: frontmatter `public: true` **or** a non-empty
   frontmatter `owl-class`. Absence of both is private. This keeps ADR-2014's
   rule (formal data bypasses the publish gate; the gate anchors on parsed
   metadata, never on the path) and changes only the carrier.
3. **Bounded legacy tolerance.** Readers additionally accept the Logseq lines
   `public:: true`, `owl:class::`, `source-domain::`, `alias::`, `title::`,
   `elevatedFrom::` **only** when they occur in the leading property block
   (contiguous `key:: value` lines before the first blank line or heading).
   `public:: true` found anywhere else (the previous `is_public_file`
   behaviour, `file_service.rs:736`) no longer counts. Tolerance ends at the
   `review_trigger`.
4. A single parsing entry point — `visionclaw_domain::vault::PageMeta`
   (`parse(content) -> PageMeta { public, owl_class, source_domain, aliases,
   title, elevated_from, tags, extra }`) — replaces the six ad-hoc line
   scanners. Every reader listed in the governing doc's "Readers and writers"
   table calls it.
5. Page identity is the vault-relative path under `pages/` without `.md`,
   with `/` as the namespace separator; `___` and `%2F` decode to `/` on read.
6. Listing skips `/.obsidian/` and `/.trash/` in addition to the existing
   `/bak/`, `/logseq/`, `/.recycle/`, `/journals/`.
7. Writers (`OntologyMutationService`, `DecisionElevation`) emit frontmatter
   pages only.

## Consequences

- ADR-2014 is superseded (same rule, new carrier); its "re-checked server-side"
  property is preserved because the gate still runs in the sync path.
- A private page that previously leaked into the KG because `public:: true`
  appeared mid-body (e.g. inside a quoted example) is now correctly excluded.
  This is a deliberate narrowing.
- Node ids of non-namespace pages are unchanged. Namespace pages gain ids
  derived from `Ns/Title`, which matches how the corpus already links to them.
- The corpus repo must be converted with `vault-migrate` (ADR-2042) before the
  tolerance is removed; until then both formats ingest.
- agentbox consumers must move to the `[vault]` path authority (agentbox
  ADR-2028) in the same release.

## Verification

Unit tests in `crates/visionclaw-domain/src/vault/` cover EXP-V01–EXP-V03 of
the governing doc (frontmatter true/false/absent, `owl-class` bypass, legacy
leading-block acceptance, mid-body rejection). Integration test in
`tests/vault_gate_test.rs` runs the sync parser over fixtures in
`tests/fixtures/vault/`. `implementation_status` is set to `complete` only
when those tests pass under `cargo test --all-features` with
`RUSTFLAGS='-D warnings'` and `verified_commit` records the SHA.
