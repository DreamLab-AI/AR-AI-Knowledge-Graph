---
id: ADR-2046
title: Remove the dead SettingsActor and the orphaned src/config/* copies
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any proposal to reintroduce a settings actor other than OptimizedSettingsActor, or to add a file under src/config/ without declaring it in src/config/mod.rs
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2046 — Remove the dead SettingsActor and the orphaned src/config/* copies

## Context

Diagrams VC-06.10/VC-09.2 flag three findings, all the same class of dead code. (A)
`SettingsActor` (`src/settings/settings_actor.rs`, 441 lines, 14 message types) is never
`.start()`ed outside tests; `src/app_state.rs:1170` starts `OptimizedSettingsActor` instead, and
`src/settings/mod.rs` already documented the actor as "retained for backward compatibility".
(B) `src/handlers/tests/settings_tests.rs` was already commented out of `src/handlers/tests/mod.rs`
and imported two nonexistent modules (`crate::actors::settings_actor`, `crate::handlers::settings_paths`).
(C) Six files under `src/config/` (`field_mappings.rs`, `physics.rs`, `services.rs`, `system.rs`,
`validation.rs`, `xr.rs`) were never declared as modules in `src/config/mod.rs` (which declares
only `dev_config`, `feature_access`, `path_access`, `security_profile`, and an inline `pub mod
physics { ... }` re-exporting `visionclaw_domain::types::physics_config`) — orphaned duplicates of
the canonical `crates/visionclaw-domain/src/config/*` definitions, the same pattern ADR-2041
already found and removed for `visualisation.rs`/`app_settings.rs`.

## Decision

All three are unreachable code with zero live callers and are deleted outright, not stubbed or
commented out: `src/settings/settings_actor.rs`, `src/handlers/tests/settings_tests.rs`, and the
six named `src/config/*.rs` files. `src/settings/mod.rs` drops `pub mod settings_actor;` and the
`pub use settings_actor::{GetPhysicsSettings, LoadProfile, SaveProfile, SettingsActor,
UpdatePhysicsSettings};` re-export block — verification (below) found zero importers of those
re-exported names anywhere in `src/`, so nothing repoints to `crate::application::settings`'s
same-named CQRS types; that pair coexists unrelated. `src/handlers/tests/mod.rs` drops the
already-commented `// pub mod settings_tests;` line. Each deletion site gets one ADR-2046 removal
comment in the house style (`src/handlers/mod.rs:44-49`).

## Consequences

- `OptimizedSettingsActor` is now the only settings actor in the tree; a future contributor cannot
  accidentally wire up the dead one.
- `src/config/` contains only modules `src/config/mod.rs` actually declares; the canonical settings
  types live solely in `crates/visionclaw-domain/src/config/*`, re-exported through
  `src/config/mod.rs:21-67` (untouched by this change).
- `crate::settings::{GetPhysicsSettings, LoadProfile, SaveProfile, UpdatePhysicsSettings}` no
  longer exist; the CQRS types of the same names in `crate::application::settings` are unaffected
  and were never aliased to these.
- No follow-on work: all three findings resolve to `complete`/`live` in this change.

## Verification

Greps run on the uncommitted working tree above `verified_commit`; must be re-run at the landing
commit.

```
$ grep -rn "SettingsActor" src/ crates/ --include=*.rs | grep -v OptimizedSettingsActor | grep -v ProtectedSettingsActor
```
Before deletion: 17 hits — the type/impl definitions and 9 `Handler<...>` impls in
`src/settings/settings_actor.rs`, the re-export at `src/settings/mod.rs:19`, one construction
(`SettingsActor::new(settings).start()`) in `src/handlers/tests/settings_tests.rs:23` (a test file
already excluded from the module tree), and one stale comment mentioning "SettingsActor" in
`src/handlers/api_handler/mod.rs:133`. After deletion: 0 hits.

```
$ grep -rn "settings_actor" src/ crates/ --include=*.rs
```
Confirmed the only non-comment hits were the file itself, its `mod`/`use` declarations in
`src/settings/mod.rs:13,18`, and the disabled test file's import
`crate::actors::settings_actor::SettingsActor` at `src/handlers/tests/settings_tests.rs:7` — a
module path that has never existed (`settings_actor` lives under `src/settings/`, not
`src/actors/`), confirming the test could not have compiled if re-enabled. Live actors
(`optimized_settings_actor`, `protected_settings_actor`) are declared separately in
`src/actors/mod.rs:40,42` and untouched.

```
$ grep -rn "config::field_mappings\|config::physics::\|config::services\|config::system\|config::xr\|config::validation" src/ crates/ --include=*.rs
```
Every hit resolved to `visionclaw_domain::config::{validation,system,xr,services}` re-exports in
`src/config/mod.rs:28-49` or the inline `pub mod physics` block at `:57-62` — none to the orphaned
files. A follow-up literal check (`grep -rn "field_mappings" src/ crates/`) found
`src/config/field_mappings.rs` byte-identical in content to
`crates/visionclaw-domain/src/config/field_mappings.rs` (the latter is the one
`crates/visionclaw-domain/src/config/app_settings.rs:7` imports), and `src/handlers/settings_validation_fix.rs`
has its own unrelated same-named local function — not a caller of the orphaned file.

```
$ grep -n "^pub mod\|^mod " src/config/mod.rs
```
Before deletion: `dev_config`, `feature_access`, `path_access`, `security_profile`,
`path_accessible_impls` (private), plus the inline `pub mod physics { ... }` block at `:57`. None
of `field_mappings`, `services`, `system`, `validation`, `xr` appear, and the on-disk `physics.rs`
file was shadowed by, not resolved through, the inline module — confirming all six files were
unreachable dead code, not just undocumented.

```
$ grep -rn "crate::settings::{" src/ --include=*.rs ; grep -rnw "LoadProfile\|SaveProfile" src/ --include=*.rs ; grep -rn "GetPhysicsSettings\|UpdatePhysicsSettings" src/ --include=*.rs
```
Trap check per this task's instructions: `GetPhysicsSettings`/`UpdatePhysicsSettings` also name
CQRS types in `crate::application::settings::{queries,directives}` (both live, used by
`src/settings/api/settings_routes.rs` via `crate::actors::messages`, not via `crate::settings`).
Zero hits for `use crate::settings::{...}` or bare `settings::GetPhysicsSettings` etc. anywhere in
`src/`, confirming `src/settings/mod.rs`'s re-export of the `settings_actor` versions had no
importers at all — safe to delete outright rather than repoint.

```
$ cargo check -p visionclaw-server --lib
```
Before this change: 0 errors, 17 warnings, `Finished` dev profile (the task brief's stated 4-error
`owl_extractor_service.rs` baseline had already been fixed by its owning lead before this
verification ran). After deletion and the `src/settings/mod.rs` / `src/handlers/tests/mod.rs`
edits: 0 errors, 17 identical warnings, `Finished` dev profile. One transient `error[E0583]: file
not found for module lifecycle` was observed on a single intermediate run — traced to a concurrent
lead's in-progress edit of `src/actors/mod.rs` (untouched by this change; ADR-2045's territory), not
to this change; a re-run seconds later, and the final run below, both show 0 errors. `cargo test -p
visionclaw-server --lib --list` also compiles clean (test profile) after the change.

```
$ node scripts/adr-index-gen.js docs/adr --check
```
Exits 0.

Working-tree caveat: verification ran on the uncommitted working tree above `verified_commit` and
must be re-run at the landing commit.
