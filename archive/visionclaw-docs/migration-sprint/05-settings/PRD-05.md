# PRD-05 — Settings & Control Panel

Status      : Proposed
Date        : 2026-05-16
Owner       : anthropic@xrsystems.uk
Related     : ADR-05 (this section), ADR-04 (Rendering), ADR-06 (Auth & Security), ADR-11 (Persistence)

## Summary

The Settings & Control Panel subsystem owns the typed schema, server- and
client-side defaults, declarative UI definition, validation, and per-user
persistence of every user-controllable knob in VisionClaw. The capability
must survive the persistence migration (Neo4j → Oxigraph + SQLite),
absorb the Nostr-only auth posture, and drop accreted UI complexity that
no longer earns its cost.

This PRD scopes 227 live settings across 10 control-panel tabs, the
single-source-of-truth schema flow `settings.ts → generated/settings.ts →
defaults → UI definition → unified panel`, the validation discipline that
keeps `[object Object]` and out-of-range numbers out of the UI, and the
Section 4 cross-cuts (`GemMaterialSettings`, `GlowSettings`) where the
renderer consumes settings types directly.

## Capability

Settings & Control Panel delivers:

1. **A single typed schema** for every user-controllable parameter in the
   system, with one hand-authored source (`settings.ts`) that produces
   one generated mirror (`generated/settings.ts`) consumed by both
   client and Rust server.
2. **One defaults source per side.** Client defaults live in
   `settingsApi.ts`; server defaults live in `src/config/`. They are
   asserted equal by a generated test, not coordinated by hand.
3. **A declarative UI definition** (`settingsUIDefinition.ts` plus the
   unified panel composition in `unifiedSettingsConfig.ts`) that maps each
   setting key to its widget, range, units, group, and tab — with zero
   imperative panel code per setting.
4. **Per-setting value validation** at three layers (TypeScript type,
   declarative UI bounds, Rust deserialiser) with the declarative bounds
   as the operative range for both UI clamping and server-side rejection.
5. **Per-user persistence** keyed by Nostr pubkey, stored in SQLite (per
   ADR-11), survivable across server restarts and decoupled from
   ontology/graph storage.
6. **A presets layer** for quality and viewport presets that overlay the
   user's stored profile non-destructively.
7. **A UI guard layer** that prevents rendering complex objects as text,
   prevents out-of-range numerical inputs reaching the server, and hides
   settings whose value-shape is incompatible with the configured widget.

## In scope

- Hand-authored types: `client/src/features/settings/config/settings.ts`.
- Generated mirror: `client/src/types/generated/settings.ts`.
- Rust-side typed config: `src/config/{visualisation,physics,xr,system,
  services,app_settings,validation}.rs`.
- Defaults: `client/src/api/settingsApi.ts` (client) and Rust
  `Default` impls in `src/config/` (server).
- UI definition: `settingsUIDefinition.ts`, `unifiedSettingsConfig.ts`,
  `debugSettingsUIDefinition.ts`, `viewportSettings.ts`, `widgetTypes.ts`.
- Validation: `src/config/validation.rs` and client-side range/shape
  guards.
- Presets: `client/src/features/settings/presets/qualityPresets.ts`.
- Persistence: SQLite-backed user settings store, keyed by Nostr pubkey.
- Section 4 cross-cut types: `GemMaterialSettings`, `GlowSettings`.

## Out of scope

- Quality-Gate toggles (the 11 inert toggles per stash `7ffe33c5`).
  These are removed (see ADR-05 §Rejected).
- "Coming Soon" panels (the five placeholder panels in the existing
  control panel). Removed.
- Settings that no live code reads. Catalogued and removed during
  migration.
- Hologram geometry settings (`buckminster`, `geodesic`,
  `triangleSphere`, `quantumField`, `plasmaEffects`, `energyFlow`,
  `ringParticles`) — already removed pre-baseline; ensure they stay
  gone.
- The graph data model (Section 8). Settings reference graph-type names
  but do not own them.
- Authentication itself (Section 6). Settings consume the resolved Nostr
  pubkey; auth flow is owned elsewhere.
- The WebSocket transport (Section 2). Settings save is HTTP only.

## Non-goals

- Real-time multi-user editing of one user's settings.
- Server-pushed settings change notifications (settings are pull-on-load,
  push-on-save).
- A general-purpose form-builder library (the UI definition is
  intentionally domain-specific to settings).
- Backwards compatibility with `SETTINGS_AUTH_BYPASS=true` in production.
  Dev-mode `?skipAuth=true` (per Section 6) remains for browser
  automation only.

## Functional requirements

### FR-1. Single schema source

The hand-authored `settings.ts` is the source of truth for setting names,
types, and groupings. A code-gen step produces `generated/settings.ts`
on the client and corresponding Serde structs on the Rust side. CI
fails if the generated artefacts drift from the source.

### FR-2. Declarative UI definition

Every setting that appears in the control panel has exactly one entry
in `settingsUIDefinition.ts` keyed by its schema path. The entry
specifies:

- Widget type (slider, toggle, select, colour, text, number,
  read-only).
- Range (min, max, step) for numerical widgets, authoritative for
  both UI clamping and server validation.
- Units and display label.
- Tab and group.
- Visibility predicate (e.g. `agentVisibility.show: only when bots tab
  enabled`).

A setting without a UI-definition entry does not render. Imperative
"if this setting then add this control" code is forbidden.

### FR-3. Validation at three layers, range from one source

- TypeScript types prevent shape errors at compile time.
- The declarative range in the UI definition clamps input client-side
  and is the authoritative range source.
- Rust `validation.rs` enforces the same range on save; out-of-range
  values are rejected with a 400, never silently clamped.

The historical `centerGravityK` range bug (UI range disagreed with
physics range) is impossible by construction once the UI definition's
range is the only declared range.

### FR-4. Per-user persistence on SQLite

Settings are stored in a SQLite table `user_settings(pubkey TEXT
PRIMARY KEY, settings JSON, updated_at INTEGER)`. Anonymous sessions
read defaults and cannot save. Authenticated sessions (Nostr pubkey
resolved per Section 6) save by upsert on pubkey.

### FR-5. Presets are overlays

`qualityPresets.ts` and viewport presets produce a partial settings
object that is merged on top of the user's stored profile, never
replacing it. The user can clear the overlay to fall back to their
saved profile.

### FR-6. UI guard layer

The UI definition's widget type and the runtime value's shape must
agree. If a setting value resolves to an object where the widget
expects a primitive, the control panel renders a "type mismatch" cell
(not `[object Object]`) and surfaces a diagnostic in the console with
the schema path. This guard is sufficient as a defence; the root cause
(schema drift) is prevented by FR-1's generated mirror.

### FR-7. Settings save endpoint authenticated as a normal route

The settings save endpoint authenticates via the same Nostr middleware
as every other authenticated route (Section 6). No bespoke
WebSocket-only auth path, no `SETTINGS_AUTH_BYPASS`. A signed-out user
attempting to save receives 401, not 403, and the UI surfaces a
"sign in to save" affordance.

### FR-8. Tab inventory bounded

The control panel has at most 10 tabs at the end of this migration.
"Quality Gate" and the five "Coming Soon" panels are removed (see
ADR-05 §Rejected). Adding an 11th tab requires an ADR amendment.

## Non-functional requirements

- **NFR-1. Load time.** A cold control-panel open renders within 100 ms
  of the user clicking the toggle. The 227-setting render cost is
  bounded by virtualisation, not by widget count.
- **NFR-2. Save latency.** A settings save round-trip completes in <
  150 ms p95 on a warm connection.
- **NFR-3. No schema drift.** CI rejects any PR where
  `client/src/types/generated/settings.ts` is out of date with
  `client/src/features/settings/config/settings.ts`.
- **NFR-4. Memory.** The control panel's mounted DOM at any time holds
  at most one tab's worth of widgets; other tabs are unmounted.
- **NFR-5. Accessibility.** Every input has a label, every slider has
  ARIA value/min/max, every group is a `fieldset` with `legend`.

## Acceptance criteria

The migration is complete for Section 5 when:

1. The 11 inert Quality Gate toggles and 5 Coming Soon panels are deleted
   from `unifiedSettingsConfig.ts` and the supporting widget files.
2. The control panel renders at most 10 tabs, each populated entirely
   from `settingsUIDefinition.ts`.
3. A code-gen check in CI fails if `generated/settings.ts` is stale.
4. Every numerical setting's range is declared in exactly one place
   (UI definition) and asserted by both the client clamp and the Rust
   validator in tests.
5. The settings save endpoint returns 401 (not 403) for anonymous
   requests and 200 for an authenticated Nostr session, with no
   WebSocket-specific auth code path.
6. A SQLite migration `0001_user_settings.sql` exists, schema-migrated
   by ADR-11's standard runner, and round-trip save/load is covered by
   integration tests.
7. The UI never renders the literal string `[object Object]`. A test
   feeds a deliberately object-shaped value into a primitive widget and
   asserts the type-mismatch cell renders.
8. `GemMaterialSettings` and `GlowSettings` types are imported by the
   renderer (Section 4) and their default values originate from
   `settingsApi.ts`.

## Bugs and smells at the reset point (41979d33e)

To migrate forward intentionally:

- **`[object Object]` rendered in UI** when a setting value is a complex
  object handed to a primitive widget. Cause: the UI definition's
  widget type and the runtime value shape are not cross-checked.
  Fix: FR-6.
- **`centerGravityK` range disagrees between UI and physics**, so the
  slider permits values the physics rejects. Cause: range declared
  in two places. Fix: FR-3 — UI definition is the only range source.
- **WebSocket 403 from settings save endpoint.** Cause: the settings
  save path coupled to WebSocket auth state rather than the HTTP Nostr
  middleware. Fix: FR-7 — settings save is a normal authenticated HTTP
  route.
- **11 inert Quality Gate toggles** present in the panel that wire to
  no live consumer (per stash `7ffe33c5`). Removed (ADR-05 §Rejected).
- **5 Coming Soon panels** present as visual placeholders with no
  delivery date. Removed (ADR-05 §Rejected).
- **Hologram geometry settings already removed.** Migration must
  not reintroduce them via a stale generated mirror.

## Dependencies

- ADR-06 (Auth & Security) resolves Nostr pubkey before save.
- ADR-11 (Persistence) provides the SQLite migration runner and the
  Settings repository trait.
- ADR-04 (Rendering) consumes `GemMaterialSettings` and `GlowSettings`.
- Section 2 (Binary Protocol) is upstream of the WebSocket path that
  must no longer be entangled with settings save (FR-7).

## Risks

- **R1. Code-gen authoring cost.** The generated mirror requires a
  build-time generator. Mitigation: a single Node script that parses
  `settings.ts` AST and emits both the TS mirror and a Rust file via
  Serde-compatible JSON Schema. Runs in CI.
- **R2. Range duplication may creep back.** A new contributor may add
  a range check in a Rust handler. Mitigation: lint rule that flags
  hard-coded `assert(x < N)` in `src/handlers/`; range checks belong
  in `validation.rs`.
- **R3. Settings volume drift.** 227 settings today, easy growth to
  300. Mitigation: NFR-1 (virtualisation) and FR-8 (10-tab cap)
  bound the user-facing cost; tab cap requires explicit ADR
  amendment.
- **R4. Migration of stored settings.** Existing users' settings live
  somewhere (or nowhere) on the rollback baseline. Mitigation:
  Section 11's migration plan covers a one-shot dump from whatever
  baseline storage exists into the new SQLite table, keyed by
  pubkey. If no prior store exists, users start with defaults.

## Success metrics

- Zero `[object Object]` reports in production logs over a 30-day
  window after migration.
- Zero "slider permits invalid value" bugs filed over a 30-day window.
- p95 settings save round-trip < 150 ms.
- Control panel cold-open p95 < 100 ms.
- Control panel widget count: 227 → ≤ 210 (drop the Quality Gate
  toggles, drop dead settings; do not add to compensate).

## Open questions

- Q1. Do per-device overrides (mobile vs desktop vs XR) need to be
  per-user-per-device, or is one profile per user sufficient? Default
  position: one profile, with viewport presets covering the device
  axis (no per-device storage).
- Q2. Does the XR client (Section 12) write settings, or only read?
  Default position: read-only — XR uses the user's stored profile
  but cannot save back.
