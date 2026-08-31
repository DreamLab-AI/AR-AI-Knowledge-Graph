# ADR-129: Control Center Re-imagination

## Status

Accepted

## Date

2026-07-04

## Context

The client's settings surface was `IntegratedControlPanel` — a docked, ~360px
tabbed panel permanently occupying screen width against the graph canvas, with
nine implementation-area tabs (graph, physics, effects, analytics, quality,
system, xr, ai, developer) built from `unifiedSettingsConfig.ts`. It carried all
168 configuration fields plus the Solid Pod and Ontology bespoke panels.

Three problems compounded over time:

- **A wall of sliders with no hierarchy of intent.** Every one of the 168 fields
  had equal visual weight regardless of how often it was touched. A user who
  wanted to make the graph "denser" or "brighter" had to know which of several
  physics or rendering fields to move, in which tab.
- **Dead affordances.** The tab bar rendered `buttonKey` shortcut badges (`1`–`9`
  digits) that nothing bound — pressing the displayed key did nothing.
- **A permanent width tax.** The 320–400px docked panel was concurrently competing
  with the 3D graph canvas, the intended hero of the application, for every
  pixel of horizontal space, whether or not the user was actively adjusting a
  setting.
- **An unrealised hydration path.** `coreSlice.ensureLoaded()` /
  `getSectionPaths()` already existed in the settings store, but nothing in the
  panel called them — any field outside the ~30 `ESSENTIAL_PATHS` read
  `undefined` on a cold `localStorage` until something else happened to touch it.

The backend routing constraint could not move: `client/src/api/settings/endpoints.ts`
prefix-matches every dot-path string from `unifiedSettingsConfig.ts` to decide
which bucketed `PUT` a field's write lands on (`physics`, `rendering`,
`qualityGates`, `nodeFilter`, `constraints`, `visual`, or local-only). Renaming a
path silently breaks server sync, so any rebuild had to restructure presentation
only, never the paths themselves.

## Decision

Replace `IntegratedControlPanel` with `client/src/features/control-center/` — a
glass DOM overlay over a hero canvas, built around three commitments:

1. **Progressive disclosure in three layers.** L1 is five macro dials plus a
   physics toggle and reset action, always visible in a bottom-center dock —
   covers the common "make it denser / brighter / faster / more focused / more
   atmospheric" case with one gesture, by writing derived transfer functions
   over existing frozen paths (no macro introduces a new path). L2 is a
   slide-out panel with eight semantic-intent groups (down from nine
   implementation-area tabs), each reachable by a real `1`–`8` hotkey. L3 is
   the individual field row, reachable either by opening its group or by
   fuzzy-searching its label or dot-path via the existing command palette
   (`Ctrl/Cmd+K`), which reveals, hydrates, scrolls to, and highlights the
   target control.
2. **A frozen registry as single source of truth.** `registry/settingsRegistry.ts`
   /`registry/manifest.ts` enumerate all 168 fields with their exact legacy
   dot-paths, byte-identical to the deleted `unifiedSettingsConfig.ts` — proven
   by a zero-drift test against a fixture captured before deletion. A committed,
   generated `settings-manifest.json` gives the browser-automation test phase a
   machine-readable map of every field's `data-testid`, control type, and
   inferred server bucket.
3. **Every control is a real, tested DOM element.** No control lives inside the
   WebGL canvas. Native `<input type="range">` and design-system Radix
   primitives carry `data-testid="setting-{dot.path}"` and full ARIA
   (`role`, `aria-valuemin/max/now`, `aria-label`), giving both accessibility
   and CDP-testability for free.

Additional decisions made in service of the above:

- `coreSlice.ensureLoaded()` is now called from every entry point (macro-bar
  mount, first group open, palette reveal, Ask-box first focus) instead of at
  boot, closing the hydration gap without a slow full-tree fetch.
- The Echo Pulse feature (a single-draw-call R3F ring that pulses on a
  slider/dial commit) is UI-only ephemeral state (`useControlCenterUI`,
  separate from the settings store), not a settings path, so it cannot
  contaminate the frozen 168-field contract.
- The agents surface (`BotsStatusPanel`, its spawn dialog, and the telemetry
  stream) is restyled to the same glass design language and folded into the
  top-right `StatusCluster`, rather than staying a separate visual system.
- `AgentControlPanel` / `SkillsTab` — the settings-side agent configuration
  surface — are *not* wired into the new registry. No "Agents" semantic group
  exists among the eight; both components were removed in the dead-code pass
  that followed this cutover (recoverable from git history).

## Consequences

### Positive

- **Testability.** The generated manifest plus the deterministic `data-testid`
  convention let a browser-automation pass assert all 168 fields are present,
  interactive, and hit their correct backend bucket — something the old panel's
  ad-hoc tab markup never supported.
- **Keyboard-first operation.** Real hotkeys (`1`–`8`, `Ctrl/Cmd+.`, `Esc`,
  native range-input arrow keys) replace the dead `buttonKey` badges.
- **Canvas reclaims the default view.** No permanent width tax; the graph is
  full-viewport until a control surface is deliberately summoned.
- **Zero backend risk.** The zero-drift fixture test makes a path rename in the
  registry fail CI immediately, rather than silently breaking a `PUT` bucket in
  production.
- **Legacy code deleted, not merely superseded.** `IntegratedControlPanel.tsx`,
  `UnifiedSettingsTabContent.tsx`, `unifiedSettingsConfig.ts`,
  `ControlPanelHeader.tsx`, `SystemInfo.tsx`, and a further confirmed-zero-importer
  set of settings components were removed once `grep` proved no remaining
  imports, rather than left to rot alongside the new registry.

### Negative

- **Discoverability cost for rarely-touched fields.** A field that used to be
  visible on a static tab now requires either memorising its group's hotkey or
  reaching for the command palette. Mitigated by the palette's fuzzy match on
  both label and dot-path.
- **The `settings.agents.*` surface lost its UI.** The ~20-option agent
  configuration surface (`AgentControlPanel`/`SkillsTab`, since deleted as
  dead code) is unreachable through any UI following this cutover; it was
  reachable (if awkwardly) through the old panel. Restoring it means building
  a registry group. Tracked as an open item, not resolved by this ADR.
- **A known server-serialisation gap surfaced during the rebuild, not caused by
  it.** `GET /api/settings/rendering` does not return `maxEdgesCeiling` or
  `softwareFallback` — the Rust backend never serialised them — so those two
  fields persist client-side via `localStorage` only. Pre-existing; the registry
  now makes the gap machine-checkable via the manifest's `server: null` marker
  where the true bucket should be `rendering`.

### Neutral

- The write path (`control → store.set → autoSaveManager` bucketed `PUT`s, 500ms
  debounce) is completely unchanged; this ADR is a presentation-layer rebuild.
- R3F continues to read settings and positions through the existing
  subscription/trie paths; no scene re-mount, no new per-frame React work.
- `SystemHealthIndicator`, `BotsStatusPanel`, `SpacePilotStatus`,
  `SolidTabContent`, and `OntologyTabContent` are reused as-is, wrapped rather
  than rewritten.

## Related Decisions

- ADR-039: Settings/Physics Object Consolidation (the `PhysicsSettings` shape
  the Motion & Forces group edits)
- ADR-046: Enterprise UI Architecture (a since-superseded, unrelated proposal
  that documents `IntegratedControlPanel` as it stood in 2026-04; superseded in
  outcome by ADR-103's enterprise dashboard removal, not by this ADR)
- ADR-013: Zero-Allocation Render Loop (the render-loop discipline the new
  overlay does not disturb)

## References

- `client/src/features/control-center/` — the implementation
- `client/src/features/control-center/registry/settingsRegistry.ts`,
  `registry/manifest.ts` — the frozen registry and endpoint-bucket inference
- `client/src/features/control-center/registry/__tests__/registry.test.ts` — the
  zero-drift gate against `legacy-paths.fixture.json`
- `client/src/features/control-center/registry/settings-manifest.json` — the
  committed, generated test manifest
- `client/src/api/settings/endpoints.ts` — the backend `PUT`-bucket prefix
  routing this rebuild is frozen against
- [Control Center](../explanation/control-center.md) — the full architecture
  explanation
