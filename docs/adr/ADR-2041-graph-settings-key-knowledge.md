---
id: ADR-2041
title: "The knowledge-graph settings key and graph-type value are `knowledge`; `logseq` is a read-only alias for one release"
date: 2026-09-02
decision_status: proposed
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit:
verified_paths: [crates/visionclaw-domain/src/config/visualisation.rs, crates/visionclaw-domain/src/config/app_settings.rs, src/config/mod.rs, src/config/path_accessible_impls.rs, src/protocols/binary_settings_protocol.rs, xr-client/scripts/graph_scene.gd, client/src/features/graph/types/graphTypes.ts, client/src/features/settings/config/settings.ts, data/settings.yaml]
owner: jjohare
review_trigger: the release after ADR-2040's tolerance ends — remove the `logseq` alias and the client migration shim
repo: visionclaw
domain: VAULT-corpus-format
lineage: legacy settings consolidation (path-accessible settings, `GraphsSettings { logseq, visionclaw }`)
---

# ADR-2041 — Graph settings key and graph-type value are `knowledge`

## Context

`GraphsSettings` (`crates/visionclaw-domain/src/config/visualisation.rs:512`,
re-exported through `src/config/mod.rs`; the same-named files under `src/config/`
were orphaned dead copies) has two members,
`logseq` and `visionclaw`; the client `GraphType` is `'logseq' | 'visionclaw'`
(`client/src/features/graph/types/graphTypes.ts:3`) with 18 literal uses and
86 `graphs.logseq` path uses; `data/settings.yaml:67` persists the key; the
server already accepts `"logseq" | "knowledge"` when resolving physics
(`crates/visionclaw-domain/src/config/app_settings.rs:159-171`). After ADR-2040 the word names a tool the system no
longer uses, and the split brain (`knowledge` server-side, `logseq`
client-side) is a standing source of confusion.

## Decision

1. The Rust field is renamed `knowledge` with `#[serde(alias = "logseq")]`
   on deserialisation; serialisation emits `knowledge`. `path_accessible_impls`
   resolves both `knowledge` and `logseq` path segments to the same field.
2. `data/settings.yaml` and every generated type (`src/bin/generate_types.rs`
   output, `client/src/types/generated/settings.ts`) use `knowledge`.
3. The client `GraphType` becomes `'knowledge' | 'visionclaw'`. A one-line
   migration in the settings store maps a persisted `graphs.logseq` object to
   `graphs.knowledge` on load and drops the old key on next save.
4. Any query-string, WebSocket, or REST value `graph_type=logseq` is accepted
   as `knowledge` for one release, then rejected with 400.

## Consequences

- Persisted user settings survive the upgrade without a manual edit.
- The alias and the client shim are debt with a named removal trigger.
- The rename touches ~100 client sites; it is mechanical and covered by the
  existing registry/shell vitest suites plus a new settings-migration test.

## Verification

2026-09-02 on the `obsidian` branch. Field renamed at its authoritative home
`crates/visionclaw-domain/src/config/visualisation.rs` with
`#[serde(alias = "logseq")]`; the orphaned dead copies
`src/config/{visualisation,app_settings}.rs` (never declared in
`src/config/mod.rs`) were deleted. One `normalise_graph_type` in the domain
crate, re-exported from `src/config/mod.rs`, plus `knowledge_graph_value`,
`graphs_map_has_knowledge`, `path_targets_knowledge_graph` as the only alias
sites. `PathRegistry` ids are registration-order counters, so renaming the
nine pre-registered path strings leaves every `path_id` byte-identical;
`canonical_path()` maps an inbound `graphs.logseq` path to the same slot.
Client: 421 sites across 39 files (incl. the generated settings manifest,
regenerated), `GraphType = 'knowledge' | 'visionclaw'`,
`migrateGraphSettingsKey` in the zustand merge hook, frozen WP5
`legacy-paths.fixture.json` kept byte-identical. XR client: `?graph=knowledge`
in `graph_scene.gd`, render-store label flipped. Evidence:
`cargo check --workspace` clean; `cargo test settings` 11 green incl. the new
`adr2041_graph_settings_key` suite (legacy key loads into `knowledge`,
re-serialisation emits only `knowledge`, both path segments resolve);
`cargo test --lib services::` 276; xr-client `render_store` 52;
client `tsc --noEmit` clean and `vitest run` 68 files / 758 tests incl. the
8-test `settingsMigration.test.ts` (EXP-V06). `activation_status: staged`
until the branch merges and a persisted `settings.yaml` is loaded by the new
binary.
