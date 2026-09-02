---
title: VAULT — authored corpus format (Obsidian vault)
version: 1.4.1
status: living
verified_commit:
owner: jjohare
domain: VAULT-corpus-format
ledger: [ADR-2040, ADR-2041, ADR-2042]
agentbox_ledger: [ADR-2028, ADR-2029]
---

# VAULT — authored corpus format

This is the governing document for the **authored knowledge corpus**: the
markdown files that GitHub sync ingests into the knowledge graph, that the
elevation and mutation services write back, and that agentbox skills read and
extend. It replaces the implicit Logseq conventions that were previously spread
across `file_service.rs`, `github_sync_service.rs`, and the agentbox skills.

Related governing documents: [`DATA-authority-erasure.md`](DATA-authority-erasure.md)
(ownership of the "Authored content" class), [`BASELINE-architecture.md`](BASELINE-architecture.md)
(the GitHub → Oxigraph → client pipeline), and in agentbox
[`agentbox/docs/BASELINE-container.md`](../agentbox/docs/BASELINE-container.md)
(the `[vault]` manifest section and the Rune TUI window).

## Purpose

1. Define the **one** on-disk format the system reads and writes: an
   [Obsidian](https://obsidian.md) vault of plain markdown with YAML frontmatter.
2. Define the **legacy tolerance**: which Logseq constructs the readers still
   accept during the transition, and the date/trigger at which tolerance ends.
3. Define the **converter contract** (`vault-migrate`) that turns a Logseq graph
   into a vault, and what the converter must report rather than guess.
4. Give every consumer (VisionClaw backend, client, agentbox skills, MCP servers,
   tmux TUI) a single path authority for the vault.

## Current state

### Corpus survey (2026-09-02, `/home/devuser/workspace/logseq/mainKnowledgeGraph`)

| Statistic | Value |
|---|---|
| Pages in `pages/` | 8,638 |
| Pages with `public:: true` | 8,601 |
| Pages with a `json-ld` fence | 8,447 |
| Namespace pages stored as `a___b.md` | 201 |
| Journals (`YYYY_MM_DD.md`) | 308 |
| Working-graph pages (`workingGraph/pages/`) | 574 |
| Pages with `{{embed ((uuid))}}` block embeds (no page embeds exist) | 14 (34 occurrences) |
| Pages with `((block-ref))` | 13 in `pages/` + 1 journal (zero `id::` targets outside `pages/.deleted/` — all dangling) |
| Pages with `#[[multi word]]` tags | 6 |
| Pages with TODO/DOING/NOW/LATER/DONE markers | 13 (12 outside code fences; 239 marker occurrences incl. journals) |
| Pages with body-level `- key:: value` lines (Dataview-style inline fields carrying the relation graph: `enables::` ×7,882, `relatedTo::` ×6,640, `requires::` ×5,982, `uses::` ×5,834, `owl-class::` ×3,854) | 6,541 files / 98,674 lines |
| Pages referencing `../assets/` | 36 (177 targets; 9 inside code fences) |
| Pages starting with a `- ` outliner bullet | 0 |
| Pages with YAML frontmatter | 0 |

The corpus is therefore prose-first markdown with a leading Logseq property
block. The heavy lifting (JSON-LD `Page` and `Class` blocks) is format-neutral
and carries over unchanged.

### Readers and writers (post-ADR-2040, verified 2026-09-02)

Single parsing entry point: `visionclaw_domain::vault::parse`
(`crates/visionclaw-domain/src/vault/mod.rs:149`) → `PageMeta { public,
owl_class, source_domain, aliases, title, elevated_from, tags, extra, format }`
with `is_kg_included()`, plus `split()` / `render_page()` /
`to_frontmatter_yaml()` (the one emitter, so YAML quoting of `mv:Foo` and
`"[[Page]]"` is solved once), `legacy_properties_anywhere()` (enrichment
only — never the gate) and `page_name_from_path()`.

| Component | Reads / writes | Citation |
|---|---|---|
| `FileService::page_is_kg_included` | gate via `PageMeta` | `src/services/file_service.rs:747` |
| `FileService::extract_owl_class_iri` / `extract_ontology_data` | enrichment, whole page (`legacy_properties_anywhere`) | `file_service.rs:728`, `:759` |
| `github_sync_service::page_is_kg_included` | gate via `PageMeta` | `src/services/github_sync_service.rs:2256` |
| `github_sync_service` elevation bridge | `PageMeta.elevated_from` | `github_sync_service.rs:1667` |
| `KnowledgeGraphParser::create_page_node` | `vault::parse` for owl-class, source-domain, tags, aliases, title (label honours `title`) | `src/services/parsers/knowledge_graph_parser.rs:116`; identity via `page_name_from_path` `:77` |
| `EnhancedContentAPI::list_markdown_files` | skips `/bak/`, `/logseq/`, `/.recycle/`, `/journals/`, `/.obsidian/`, `/.trash/` | `src/services/github/content_enhanced.rs:113-114` (files), `:234-235` (dirs) |
| `EnhancedContentAPI` namespace lookup | decodes `%2F` **and** `___` → `/` | `content_enhanced.rs:395` |
| `OntologyMutationService::generate_vault_markdown` | writes frontmatter pages; amendment path edits frontmatter via `split`/`render_page` | `src/services/ontology_mutation_service.rs:53`, `:484` |
| `DecisionElevation` page draft | writes `public: true` (+ `title`) via `render_page` — previously emitted no property at all, so drafts would have been private | `src/services/decision_elevation.rs:223` |
| agentbox `ontology-local.js` / `ontology-index-build.js` / `continual-harness.js` | corpus from `VAULT_PAGES` / `VAULT_ROOT`; writes via `vault-frontmatter.js` `ensureFrontmatter` | `agentbox/mcp/servers/lib/` (ADR-2028) |
| agentbox entrypoint | exports `VAULT_ROOT`/`VAULT_PAGES`/`VAULT_FORMAT`/`VAULT_TUI`; `ONTOLOGY_PAGES_DIR` derives from `VAULT_PAGES` | `agentbox/config/entrypoint-unified.sh` `_ab_vault_resolve` |
| agentbox `podcast-knowledge-ingest`, `web-summary` | write frontmatter pages under `$VAULT_ROOT` | `agentbox/skills/*/SKILL.md` (ADR-2028) |
| `vault-migrate` | one-shot converter | `crates/vault-migrate` (ADR-2042) |
| Rune "Notes" window | tmux window 9 at `$VAULT_ROOT` | `agentbox/config/tmux-autostart.sh` (ADR-2029) |

Tests: `crates/visionclaw-domain/src/vault/mod.rs` (40 unit),
`crates/visionclaw-domain/tests/vault_fixtures.rs` (5),
`tests/vault_gate_test.rs` (9) over `tests/fixtures/vault/`;
`tests/fixtures/data-model/valid/pages/*.md` remain Logseq-format and exercise
the legacy-tolerance path.

## The vault contract

### V1 — Layout

```
<VAULT_ROOT>/                     # the Obsidian vault root (was mainKnowledgeGraph/)
  .obsidian/                      # app config; only app/appearance/core-plugins/community-plugins/hotkeys are committed
  pages/                          # authored pages — GitHub sync base path stays "pages"
    <Title>.md
    <Namespace>/<Title>.md        # was <Namespace>___<Title>.md
  journals/YYYY-MM-DD.md          # was YYYY_MM_DD.md; excluded from KG ingest as before
  assets/                         # unchanged; links rewritten to vault-root-relative "assets/..."
  templates/                      # optional
```

- The vault root is the single path authority: `VAULT_ROOT` (env) ← agentbox
  `[vault].root` (manifest). Every consumer derives sub-paths from it; no
  consumer hard-codes `/home/devuser/workspace/logseq/...`.
- Page **identity** is the path relative to the **matched `GITHUB_BASE_PATH`
  prefix** (each vault's `pages/`), without the `.md` extension, with `/` as the
  namespace separator — never the repo-relative path. The same relative path in
  `knowledge/pages/` and `working/pages/` is deliberately **one node** (the
  main↔working twin join: 254 such pairs in the 2026-09-02 corpus).
  `page_name_to_id` slugifies that name exactly as before, so node ids for
  root-level pages are unchanged; pages in subfolders gain distinct identities
  (they were previously merged by basename — 34 genuinely distinct pages
  collided). Legacy encodings `___` and `%2F` decode to `/` on read.
- `title` is a **display** value only. A `title` that merely echoes the
  identity path is ignored by readers, and the converter never writes one.
- **Wikilink resolution follows Obsidian's shortest-path rule.** A link target
  is normalised (trim; strip `|alias`, `#heading`, `^block`; decode `___` and
  `%2F` to `/`). A target containing `/` resolves by **exact** identity match
  with no basename fallback (rebinding `[[Wrong_Folder/Economy]]` to another
  folder's `Economy` would invent an edge). A bare target
  resolves by **basename**: exactly one page with that basename anywhere under
  `pages/` → that page's full identity; several → the one in the linking
  page's own folder, else the first in sorted path order, and the ambiguity is
  reported; none → a `linked_page` stub. Pages that already lived in plain
  subfolders (e.g. `working/pages/podcast-evidence/<slug>.md`) are therefore
  reachable by the bare `[[<slug>]]` links the corpus uses, and no stub is
  minted beside a real page. (Found by the 2026-09-02 shadow sync: +186 stub
  pages before this rule.)

### V2 — Frontmatter (Obsidian Properties)

Every page begins with a YAML frontmatter block delimited by `---` lines.
Keys are lower-kebab-case. The reserved Obsidian keys `aliases`, `tags`,
`cssclasses` keep their Obsidian meaning.

| Key | Type | Meaning | Logseq origin |
|---|---|---|---|
| `public` | checkbox | KG inclusion gate (see V4) | `public:: true` |
| `aliases` | list | Obsidian aliases; also KG alias metadata | `alias::` |
| `title` | text | Display title when it differs from the filename; never the identity path | `title::` |
| `tags` | list | Obsidian tags; KG `tags` metadata | `tags::`, `#[[..]]` |
| `owl-class` | text | Formal class IRI; **bypasses the public gate** | `owl:class::` |
| `source-domain` | text | Domain prefix (ai/bc/mv/rb/tc/ngm) | `source-domain::` |
| `elevatedFrom` | text (quoted link) | `"[[Working Page]]"` provenance bridge | `elevatedFrom:: [[..]]` |
| any other `key` | text/list | Preserved verbatim from the Logseq property block | `key::` |

Rules:
- Wikilinks inside property values are quoted strings: `elevatedFrom: "[[Working Page]]"`.
- `public` is a real YAML boolean (`true`/`false`), never the string `"true"`.
- The JSON-LD `Page` and `Class` fences stay in the body unchanged; frontmatter
  never duplicates their content.
- A page with no frontmatter is **private** (fail-closed), exactly as a page
  with no `public:: true` was.

### V3 — Body dialect

| Construct | Vault form | Note |
|---|---|---|
| Wikilinks | `[[Page]]`, `[[Page\|Alias]]`, `[[Ns/Page]]` | unchanged |
| Page embeds | `![[Page]]` | was `{{embed [[Page]]}}` (none exist in the corpus) |
| Block embeds | left literal, reported | `{{embed ((uuid))}}` — no `id::` targets exist |
| Tasks | `- [ ] text`, `- [x] text` | was `- TODO/DOING/NOW/LATER text`, `- DONE text` |
| Multi-word tags | `#multi-word` | was `#[[multi word]]` |
| Block refs | left literal, reported | `((uuid))` with no `id::` target anywhere in the corpus |
| Assets | `assets/<file>` (vault-root-relative) | was `../assets/<file>`; rewritten in bodies **and** in leading-block property values (a note-relative path breaks once the page moves into a namespace folder) |
| Body-level `- key:: value` | preserved verbatim, reported | 6,541 pages: Dataview inline fields carrying the relation graph; readable by Obsidian's Dataview, navigable via their `[[links]]`; never part of the gate |
| `collapsed:: true` | dropped | outliner-only |
| Code fences (`json-ld` etc.) | unchanged | |

### V4 — Inclusion gate (amends ADR-2014)

A page is ingested as a KG node iff **either**:

1. its frontmatter has `public: true`, or
2. its frontmatter has a non-empty `owl-class` (formal data ingests unconditionally),

**or**, during the legacy-tolerance window, the corresponding Logseq line
(`public:: true`, `owl:class::`) appears in the leading property block. Absence
of both means private. The gate anchors on parsed metadata, never on the file
path. A page whose formal data is a `json-ld` **`Class` fence** (not an
`owl-class` key) is claimed by the canonical JSON-LD path
(`parse_canonical_entity`) before the publish gate runs, so its ontology data
surfaces even when the page is private — legacy ADR-08 D3 is honoured by the
canonical path, not by the gate (pinned by
`tests/vault_gate_test.rs::legacy_data_model_fixtures_still_gate_as_authored`). `/journals/`, `/.obsidian/`, `/bak/`, `/logseq/`, `/.recycle/`,
`/.trash/` are skipped at listing time.

### V5 — Writers emit vault format only

`OntologyMutationService`, `DecisionElevation`, agentbox `ontology-local.js`'s
write path, `podcast-knowledge-ingest`, and `web-summary` emit **frontmatter**
pages. No writer emits `key:: value` lines after ADR-2040 lands. A writer that
must touch a legacy page converts the leading property block on write.

### V6 — Converter (`vault-migrate`, ADR-2042)

- One Rust binary, `crates/vault-migrate`, no LLM, deterministic, idempotent.
- Default mode writes to an **output directory**; `--in-place` is explicit.
- Never deletes; unknown constructs are preserved and **reported**, not guessed.
- Emits a machine-readable report (`vault-migrate-report.json`) with per-rule
  counts and the list of pages carrying unconverted constructs.
- Round-trip property: converting an already-converted vault is a no-op.
- The converter never writes `title:` from the identity; a `title` that echoes
  the identity or its leaf is removed on any run (`title_echo_removed`).
- Zero-byte pages and journals rename like any other file.
- `pages/` subdirectories: a plain subdirectory (e.g. `pages/_misc/`) converts
  as a namespace folder; dot-directories (`pages/.deleted/`, `.swarm`,
  `.claude-flow`) are copied verbatim and never converted, matching V4's
  listing skips. `mainKnowledgeGraph/assets` is a symlink into
  `workingGraph/assets`; the converter follows it so each vault owns an
  independent copy of its assets.

### V7 — Settings and wire vocabulary (ADR-2041)

The graph-settings key `visualisation.graphs.logseq` is renamed
`visualisation.graphs.knowledge`. Rust deserialises both (`serde(alias)`), the
client migrates persisted settings on load, and the wire/query value `logseq`
for `graph_type` is accepted as a synonym of `knowledge` for one release.

### V8 — TUI (agentbox ADR-2029)

The vault has a first-class terminal surface: Rune (`aka-rider/rune`, MIT,
Rust/ratatui) launched from `VAULT_ROOT` in tmux window 9 "Notes". Presence is
detected at session start like the AoE plane; absence prints the rebuild notice.

## Invariants (must not silently change)

1. **One format on write.** Every writer in either repo emits frontmatter pages
   (V2) for page metadata. Adding a leading-block `key:: value` emitter is a
   violation. Body-level `- pred:: [[Target]]` inline fields (V3) are content,
   not metadata, and remain the corpus's relation-authoring form.
2. **Fail-closed gate.** No frontmatter, or `public` absent/false and no
   `owl-class`, means the page is not a KG node.
3. **Path authority is `VAULT_ROOT`.** No consumer hard-codes a corpus path;
   the manifest `[vault].root` is the only default.
4. **Identity stability.** Non-namespace page ids are byte-identical before and
   after conversion; namespace page ids derive from `Ns/Title`, matching the
   `[[Ns/Title]]` wikilink form already used in the corpus.
5. **Converter never destroys.** Output-dir default, explicit `--in-place`,
   preserve-and-report for anything not in V3.
6. **Legacy tolerance is bounded.** Readers accept Logseq property lines only
   in the leading block, only for the keys in V2, and the tolerance is removed
   by the `review_trigger` on ADR-2040.

## Expectations (EDD)

| ID | Priority | Expectation | Evidence |
|---|---|---|---|
| EXP-V01 | critical, regression | A page whose frontmatter is `public: true` and nothing else ingests as a KG node; the same page with `public: false` or with no frontmatter does not. | `cargo test -p webxr vault_gate` |
| EXP-V02 | critical, regression | A page with `owl-class: mv:Foo` and no `public` key ingests, and its node carries `owl_class_iri = "mv:Foo"`. | same |
| EXP-V03 | high, regression | A legacy page starting `public:: true` still ingests; a legacy page with `public:: true` only inside a code fence or after the first heading does not. | same |
| EXP-V04 | high, regression | `vault-migrate` on the 2026-09-02 corpus emits `public: true` on 8,615 pages (8,601 top-level + 14 under `pages/_misc/`), moves 201 namespace files into folders, renames 308 journals, rewrites 239 task markers and 168 asset links, rewrites no page embeds (none exist), and reports 14 block-embed/block-ref files, 6,541 body-property files and 4 SCHEDULED/DEADLINE journals — with `--check` on the output exiting 0. | `vault-migrate --report` on the corpus; verified 2026-09-02 (66 unit + 16 integration tests green; real run 2.6 s) |
| EXP-V05 | high, regression | Running `vault-migrate` twice yields byte-identical output the second time. | converter test |
| EXP-V06 | medium | `visualisation.graphs.logseq` in a persisted `settings.yaml` loads into `graphs.knowledge` without loss; the client renders with the same colours. | settings test + vitest |
| EXP-V08 | critical | Shadow sync: syncing the converted corpus with the new binary yields the same node set as syncing the unconverted `main` with the same binary, except labels that now honour `title`; after the shortest-path link rule the graph returns to the original baseline. | 2026-09-02 dev-container runs (`GITHUB_BRANCH`/`GITHUB_REPO` override on `sync_github`): same pre-fix binary → main 13,351 nodes / 156,153 edges vs converted 13,351 / 156,101 (3 title-driven label diffs); link-fix binary → `jjohare/visionGraph` 13,162 / 145,474 / 378 `page` nodes against the original old-binary baseline 13,164 / 145,692 / 382 — the 191 `podcast-evidence___…` stubs are gone and the 254 main↔working twin joins hold; final run with the percent-encoding and leaf-label fixes → 13,165 / 145,561 / 381, zero fetch errors, only genuine slashed names (`TCP/IP`, `ISO/IEC …`) as labels. |
| EXP-V07 | medium | tmux window 9 "Notes" opens Rune at `VAULT_ROOT` when the binary exists and prints the rebuild notice when it does not; window 0 remains the tab0-bridge target. | `bash -n` + a dry run of `tmux-autostart.sh` in a scratch socket |

## Change process

This is a living document. Amend it in the same commit that changes any
reader, writer, gate rule, converter rule, or the path authority. Every
load-bearing claim carries a `file:line` citation; update the citation when
the code moves. Bump `version` (patch for wording, minor for a new key or
rule, major for a change to the gate or identity rule) and refresh
`verified_commit`.
