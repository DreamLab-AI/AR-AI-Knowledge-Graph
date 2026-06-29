# ADR-05 — Settings & Control Panel

Status      : Proposed
Date        : 2026-05-16
Supersedes  : Ad-hoc settings architecture on `main` (no prior ADR)
Related     : ADR-04 (Rendering), ADR-06 (Auth & Security), ADR-11 (Persistence)

## Context

Settings & Control Panel at baseline `41979d33e` is the right shape but
the wrong size. It already has the four moving parts a settings system
needs — typed schema, defaults, declarative UI definition, validation
— but the moving parts disagree with each other in three ways that
produce user-visible defects:

1. **`[object Object]` rendered in the panel** when a complex setting
   value reaches a primitive widget. The UI definition's widget type
   and the runtime value's shape are not cross-checked.
2. **`centerGravityK` slider permits values the physics rejects**, the
   range is declared in two places (UI definition and a Rust handler)
   and the two declarations have drifted.
3. **WebSocket 403 from the settings save endpoint** — the save path
   was coupled to WebSocket auth state rather than to the HTTP Nostr
   middleware that every other authenticated route uses.

Between baseline and `main`, several rounds of bandaging have attempted
to fix these symptomatically. The `main` panel also accumulates:

- **11 inert Quality Gate toggles** (stash `7ffe33c5`) that render
  controls wired to no live consumer.
- **5 "Coming Soon" panels** that present empty UI surface with no
  delivery commitment.
- Dead settings whose schema entries exist but whose runtime values
  no read site consumes.

Plus the cross-cutting decision that settings now persist on SQLite
(ADR-11) keyed by Nostr pubkey (Section 6) rather than wherever they
lived on the rollback baseline.

This ADR settles those four directions — schema discipline, UI/value
agreement, range single-sourcing, SQLite persistence — and prunes the
accreted complexity.

## Decision

### D1. Schema is single-source, mirror is generated

`client/src/features/settings/config/settings.ts` is the only
hand-authored schema. A build-time generator parses its AST and emits:

- `client/src/types/generated/settings.ts` — TypeScript mirror used
  by every other client module that needs Settings types.
- `src/config/generated_settings.rs` — Rust Serde structs used by the
  config crate (`src/config/`) and the SQLite repository.

The generator runs as a `prepublish` and a CI check. CI fails if the
checked-in generated files differ from a fresh run. No editor lives
in `generated/`. No type in `generated/` is hand-modified.

The generated Rust file is `#[derive(Serialize, Deserialize)]` and
references handwritten validation impls in `src/config/validation.rs`
so the schema and the constraints stay separable.

### D2. Defaults: one per side, asserted equal by test

Client defaults live in `client/src/api/settingsApi.ts` as a single
`DEFAULTS` constant. Server defaults live in `src/config/` as
`impl Default for AppSettings`. A test in
`client/src/api/__tests__/settingsApi.defaults.test.ts` posts the
client defaults to a test fixture of the server `AppSettings` Default
and asserts deep-equal. This makes default drift a CI failure rather
than a runtime surprise.

### D3. UI definition is authoritative for range and widget shape

`settingsUIDefinition.ts` declares each setting's widget, range
(`min`, `max`, `step`), units, label, group, tab, and visibility. The
panel composition in `unifiedSettingsConfig.ts` reads only this
definition — no imperative widget creation. Each entry is keyed by the
schema dotted path (e.g. `physics.centerGravityK`).

The range declared here is the operative range:

- Client clamps inputs to `[min, max]` before save.
- Server validator (`src/config/validation.rs`) is generated from the
  same source via the schema generator (D1), so the Rust check
  derives from the UI definition's bounds, not from a hand-rewritten
  copy.

This eliminates the `centerGravityK` class of bug structurally.

### D4. Widget/value shape cross-check at runtime

Each widget type declares the value shapes it accepts (e.g. a slider
accepts `number`, a toggle accepts `boolean`, a colour widget accepts
`string` matching `/^#[0-9a-f]{6}$/i`). The panel renderer checks the
runtime value against the widget contract before mounting the widget:

- Shape matches → mount widget as normal.
- Shape mismatch → mount a `TypeMismatchCell` that renders the schema
  path, the expected shape, and the offending value (truncated to 60
  characters and `JSON.stringify`-encoded). A `console.warn` carries
  the same payload.

The literal string `[object Object]` cannot reach the DOM by
construction. A regression test in
`client/src/features/visualisation/__tests__/controlPanel.shape.test.tsx`
feeds an object into a primitive widget and asserts the mismatch cell
renders.

### D5. Persistence: SQLite, schema per ADR-11

Settings persist in SQLite. **The schema, primary-key layout, per-user
resolution order, pragmas, and backup mechanics are owned by ADR-11 §D5
and DDD-11 §"Settings per-user resolution"** — this ADR does not
duplicate them. Section 5 owns only the contents of what gets stored:

- The `AppFullSettings` Rust type (generated from `settings.ts` per D1)
  is the domain-level shape. ADR-11's adapter persists it via the
  17-method `SettingsRepository` trait at
  `src/ports/settings_repository.rs`.
- `AppFullSettings` embeds its own `schema_version: u32` field,
  generated from the `settings.ts` AST. This is distinct from ADR-11's
  `schema_migrations` table: the table tracks which SQL migrations the
  database has applied; the embedded version tracks the shape of an
  individual user's stored document. Section 5 owns the document-shape
  migration ladder (`fn(&mut Value, from: u32) -> u32`); ADR-11's
  adapter invokes it on read when the embedded version is lower than
  current.
- Anonymous sessions read defaults and receive 401 on save (FR-7).
  Authenticated sessions read and write their own settings; the pubkey
  threads through ADR-11's task-local context, not as a method parameter.
- The repository trait is frozen by ADR-11 PRD A2. Changes require
  coordinated edits to both ADRs.

### D6. Settings save is a normal authenticated HTTP route

`POST /api/settings` authenticates via the standard Nostr middleware
(Section 6). The handler:

1. Resolves pubkey from the middleware-attached principal.
2. Validates the payload via the generated validator (D3) and
   rejects 400 with field path on out-of-range values.
3. Calls `SettingsRepository::save_partial(pubkey, patch)`.
4. Returns 200 with the materialised post-save `AppSettings`.

There is no WebSocket-side save path. The freeze-era "WebSocket 403"
class of bug ceases to exist because the entanglement that produced
it is removed.

### D7. Presets are non-destructive overlays

`qualityPresets.ts` defines named overlays (`high`, `medium`, `low`,
`xr`). A preset is a `Partial<AppSettings>`. Applying a preset is a
client-side merge on top of the user's stored profile, persisted as
an "active overlay" in client state only — the stored profile is
untouched. Clearing the overlay falls back instantly.

This keeps "I tried the XR preset and lost my custom layout settings"
from happening.

### D8. Quality Gate and Coming Soon are removed

The 11 inert Quality Gate toggles and the 5 Coming Soon panels do not
migrate. The widget files
(`unifiedSettingsConfig.ts:qualityGateGroup` and the five panel
component files) are deleted. The corresponding schema entries are
removed from `settings.ts`. The tab inventory drops to 10 tabs that
all do something.

If "Quality Gate" returns as a real feature, it returns through a
fresh ADR with delivered semantics, not as ambient UI.

### D9. Section 4 cross-cut types stay in settings

`GemMaterialSettings` and `GlowSettings` are owned by the settings
schema. The renderer (Section 4) imports them. Defaults live in
`settingsApi.ts` like every other default. The renderer does not
declare its own copy of these types; ADR-04 references this ADR for
their source.

## Options considered

### O1. Reintroduce a runtime schema library (Zod, JSON Schema, …)

Rejected. The hand-authored TypeScript schema plus a code-gen step is
sufficient. Adding Zod adds a parser layer at every read site and a
duplicate source of truth (the Zod schema vs the TypeScript types).
Code-gen keeps one truth and gets the parser-free read path.

### O2. Keep multi-source defaults; reconcile at runtime

Rejected. The "client and server defaults reconcile on first save"
pattern was tried implicitly on the baseline and produced subtle
drift bugs. CI-asserted equality (D2) is cheaper to maintain and
catches drift before it ships.

### O3. Range duplication, document the rule

Rejected. The discipline didn't hold. The `centerGravityK` bug is
direct evidence. Single-sourcing (D3) makes the rule mechanical.

### O4. Persist settings inline with graph data

Rejected. Settings and graph data have different read patterns,
different change cadence, and different sensitivity profiles.
Inlining them couples ADR-11's Oxigraph migration to settings, which
slows both. SQLite gives settings exactly the access pattern they
need (one row per user, point lookup, full overwrite or JSON patch).

### O5. Keep the Quality Gate toggles "for future use"

Rejected. UI that does nothing actively misleads users — they expect
the toggle to do something and waste time debugging when it does not.
Bring back the panel surface when the feature ships, with an ADR.

### O6. Push settings changes over WebSocket

Rejected. Real-time push for settings is solving a problem nobody has
(one user editing their own settings does not need server-pushed
notification). It also reintroduces the auth-coupling smell that
produced the WebSocket 403 bug. Save is HTTP, load is HTTP, the
WebSocket stays focused on graph telemetry.

## Risks

- **R1. Generator becomes load-bearing.** The schema generator must
  produce correct TypeScript and Rust from a single TypeScript AST.
  Mitigation: generator output is checked into the repo and CI-asserted;
  a generator regression surfaces as a diff, not as a runtime bug.
- **R2. JSON-patch save races.** Two tabs of the same user editing
  simultaneously can produce a lost update. Mitigation: server applies
  patches with optimistic concurrency on `updated_at`; client retries
  once on conflict by re-reading and re-merging. Documented in PRD-05.
- **R3. Per-document `schema_version` drift.** `AppFullSettings::schema_version`
  (Section 5) and `schema_migrations.id` (Section 11) are independent
  counters answering different questions. Mitigation: distinct names in
  code, plus a unit test asserting they cannot be conflated.
- **R4. UI definition becomes a god file.** 227 entries in one
  TypeScript file is borderline. Mitigation: shard
  `settingsUIDefinition.ts` by tab (one file per tab, re-exported by
  an `index.ts`). Tab files cap at ~50 entries each.
- **R5. Code-gen latency in CI.** A slow generator hurts iteration.
  Mitigation: generator runs incrementally on `settings.ts` mtime
  in dev; CI runs a full clean generation. Target full run < 2s.

## Rejected from main as buggy / unjustified

- **`7ffe33c5` Quality Gate toggle stash** — 11 inert toggles, no
  matching consumer. Deleted (D8).
- **5 "Coming Soon" panels** — placeholder surface without delivery.
  Deleted (D8).
- **Bespoke WebSocket save path** — produced the 403 bug. Deleted (D6).
- **Settings-table column proliferation on `main`** (separate columns
  for each tab) — replaced by single JSON blob with versioning (D5).
  Per-column indexing is unnecessary at the access pattern in play
  (point lookup by pubkey).
- **Per-handler range checks** in `src/handlers/settings_handler.rs`
  on `main` — moved into the generated validator (D3).

## Bugs and smells at the reset point (41979d33e)

To flag for migration awareness:

- `settings.ts` and the generated mirror at baseline are kept in sync
  by hand. The generator (D1) is the first net-new piece of
  infrastructure this ADR introduces.
- The control panel at baseline already mixes declarative entries
  with a small number of imperative widgets. D3 mandates 100%
  declarative; the imperative escapes are migrated to declarative
  entries with appropriate widget types added to `widgetTypes.ts`.
- `viewportSettings.ts` at baseline contains both schema and UI
  definition for viewport widgets. Split per FR-2: schema moves into
  `settings.ts`, UI moves into `settingsUIDefinition.ts/viewport.ts`.
- The hologram geometry types (already removed pre-baseline) must not
  return via a stale generated mirror. The generator must clear and
  rewrite, not append.
- Default values for `GemMaterialSettings` and `GlowSettings` live in
  several places at baseline (component constants, settingsApi). D9
  consolidates them in `settingsApi.ts`.
- Settings persistence at baseline is ad-hoc / in-memory. D5
  introduces the SQLite store as the first concrete settings
  persistence in this codebase.

## Migration notes

- The repository trait `SettingsRepository` and the SQLite
  implementation are part of Phase 1 (Persistence ports, per the
  README phasing). Settings save endpoint (D6) lands in Phase 7
  along with Auth.
- The code-gen step (D1) is a Phase 0 deliverable — it must be in
  place before any subsequent section adds Settings types, to avoid
  a fresh generation of drift.
- Existing user profiles, if any, are migrated by a one-shot script
  during the Phase 7 cut-over: read whatever the baseline persists
  (likely localStorage on the client and/or the server in-memory
  default copy), serialise as `AppSettings`, upsert into
  `user_settings` keyed by the user's resolved Nostr pubkey. Users
  without a resolvable pubkey start fresh; we surface this in the
  release notes.
