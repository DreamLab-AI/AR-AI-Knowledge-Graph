---
id: ADR-2041
title: "The knowledge-graph settings key and graph-type value are `knowledge`; `logseq` is a read-only alias for one release"
date: 2026-09-02
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
verified_paths: [src/config/visualisation.rs, src/config/path_accessible_impls.rs, src/config/app_settings.rs, client/src/features/graph/types/graphTypes.ts, client/src/features/settings/config/settings.ts, data/settings.yaml]
owner: jjohare
review_trigger: the release after ADR-2040's tolerance ends — remove the `logseq` alias and the client migration shim
repo: visionclaw
domain: VAULT-corpus-format
lineage: legacy settings consolidation (path-accessible settings, `GraphsSettings { logseq, visionclaw }`)
---

# ADR-2041 — Graph settings key and graph-type value are `knowledge`

## Context

`GraphsSettings` (`src/config/visualisation.rs:512`) has two members,
`logseq` and `visionclaw`; the client `GraphType` is `'logseq' | 'visionclaw'`
(`client/src/features/graph/types/graphTypes.ts:3`) with 18 literal uses and
86 `graphs.logseq` path uses; `data/settings.yaml:67` persists the key; the
server already accepts `"logseq" | "knowledge"` when resolving physics
(`app_settings.rs:174`). After ADR-2040 the word names a tool the system no
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

`cargo test settings_` (deserialisation of both keys into `knowledge`,
serialisation emits only `knowledge`), `client: vitest run` including a new
`settingsMigration.test.ts` (EXP-V06). `implementation_status` moves to
`complete` when both pass and `verified_commit` is recorded.
