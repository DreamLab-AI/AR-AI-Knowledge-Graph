# PRD-007: Unified Control Surface — Spine

**Status:** Accepted
**Date:** 2026-04-28
**Author:** VisionClaw platform team
**Supersedes:** —
**Related:**
- `docs/control-surface-audit/controlrefactor.md` (draft this PRD formalises)
- `docs/control-surface-audit/comprehensive-settings-table.md` (entity inventory)
- `docs/control-surface-audit/aspirational-inventory.md` (decision codes)
- ADR-061 (Architecture decision — descriptor type, Spine renderer, deep-link grammar)
- `docs/ddd-control-surface-context.md` (Domain model — ControlSurface bounded context)
- ADR-050 (sovereign visibility / owner_pubkey — informs tier-3 multi-tenant gating)
- ADR-059 (bidirectional agent channel — informs the per-descriptor LLM intelligence layer)

## TL;DR

Replace the current 10-tab / 205-leaf-control / ~70-server-only-tail Control Center with a single scrollable **Spine** of plain-English sentences. Each sentence describes the current state of the graph and expands in place to reveal the underlying controls. The Spine hosts ~40 visible rows at tier 1 (down from 205), absorbs all 11 audit MERGEs, retires 12 CUTs, EXPOSEs 35 server-only tunables, and WIREs 25 dormant features. Per-descriptor LLM intelligence (granular variant of the existing settings idea — there is no existing LLM chat, just regex-based CommandInput; we wire `/api/nl-query/*` for the first time). Multi-tenant: pubkey-scoped tier-3 power-user gate, role-claim tier-4 operator block (read-only). Desktop and mobile from day 1. Total descriptor count: **88**.

## 1. Problem

Three numbers:

- **205** leaf controls across **10 tabs** in `IntegratedControlPanel.tsx` + `unifiedSettingsConfig.ts` (496 lines of single-source-of-truth). Tabs answer "what knobs exist?", not "what is the graph doing?".
- **~70 server-only tunables** in `AppFullSettings` (`src/config/app_settings.rs`) with **no UI today** — physics extras, feature flags, network limits, semantic constraint config.
- **30 distinct cross-cutting disconnects** documented in the audit. Top 5 user-visible:
  1. CommandInput parses keywords with regex; `/api/nl-query/*` exists server-side but is **never called**.
  2. `qualityGates.layoutMode` and `physics.layoutAlgorithm` duplicate each other with different vocabularies.
  3. `system.debug.enabled` + `developer_config.debug_mode` + 10 scattered toggles instead of one tri-state.
  4. XR settings update `localStorage` only — never reach the `visualisation.xr.*` server schema (transport gap).
  5. `POST /api/settings/profiles` returns a dummy ID — full STUB; no Neo4j persistence.

Every tab today answers "what knobs exist?". The Spine answers "what is the graph doing right now, and what would I touch to change it?". This is the change in shape that the audit decisions cannot land into the existing tab structure.

## 2. Proposal

A single scrollable list. Every row is a sentence describing current state. Click to expand the underlying knobs in place. No tabs; no per-panel "Show advanced" toggles. Tier gating is a single global filter, not a separate UI.

```
┌─ Spine ────────────────────────────────────┐
│ 🔍 Search settings…                        │
├────────────────────────────────────────────┤
│ Showing knowledge nodes only.              │
│ Cluster tightness: tight                   │
│ Render at standard quality (AA on, …)      │   ← merged
│ Diagnostics: errors only                   │
│ Auto-pause when settled (kinetic < 0.04)   │
│                                            │
│ ── Power user ─── (only if pubkey gated)   │
│ Layout algorithm: stress-min               │
│ Re-sync graph from GitHub: last 12m ago    │
│ Audit trail: 1,287 events (last 24h)       │
│                                            │
│ ── Operator ──── (read-only)               │
│ Build: 032ec78058 · GPU sm_86              │
│ Live WS subscribers: 4                     │
└────────────────────────────────────────────┘
```

The single new abstraction is the **`Setting<T>` descriptor**. Every existing tab becomes an array of these. Adding a setting is adding one object to an array; the store schema stays unchanged.

```ts
type Setting<T> = {
  id: string;                                       // e.g. "render.quality"
  path: string[];                                   // store dot-path
  tier: 1 | 2 | 3 | 4;
  category: 'visual' | 'behaviour' | 'data' | 'team' | 'power' | 'operator';
  label: string;                                    // search + breadcrumb
  summary: (v: T, ctx: Ctx) => string;             // THE design work — present-tense state
  Editor: React.FC<{ value: T; onChange: (v: T) => void }>;
  folds?: string[];                                 // ids of merged children
  decision?: 'KEEP' | 'MERGE' | 'EXPOSE' | 'WIRE';
  ref?: string;                                     // back-ref to audit §
  readOnly?: boolean;                               // tier-4 ops
  llmContext?: {                                    // §6: per-descriptor LLM
    bounds?: { min: number; max: number; step?: number };
    examples?: string[];
    explainPrompt?: string;
  };
};
```

## 3. Why this over the alternatives

| Direction | Strength | Why not now |
|---|---|---|
| Tier mode (toggle to power) | Familiar | Doesn't reduce control count |
| Preset-first | Lowest cognitive load | PUs hate "lift out of preset" |
| Instrument rack | Tactile | Heavy build; performer UX, not analyst UX |
| Command-first | Wires NL endpoints | Strong companion, weak primary |
| Patch bay | Best PU surface for grants | Sub-problem, not whole surface |
| **Spine (this PRD)** | Absorbs every audit decision | Summary-writing is real work — accepted |

The Spine is the most additive: command-first overlay + patch-bay panel can ship later **without rework**.

## 4. Goals & non-goals

### Goals
- Reduce visible-at-tier-1 control count from ~205 to **≤ 40 summary rows**.
- Every visible row reads as a **sentence about state**, not a label.
- Land all **11 audit MERGEs**, **12 CUTs**, **35 EXPOSEs**, **25 WIREs** in one shape.
- **Deep-link any row** (`?expand=render.quality`) for support and team handoff.
- Power-user and operator settings live in the **same scroll**, not a separate app.
- **Mobile responsive** from v1 (breakpoint 768px; no separate component, same Spine flows differently).
- **Per-descriptor LLM intelligence**: every row offers a free-text "describe what you want" entry that translates to a setting mutation through `/api/nl-query/*`.
- **Multi-tenant**: tier 3 gated by NIP-98 pubkey allowlist (`POWER_USER_PUBKEYS`); tier 4 gated by operator role claim. Per-user settings (filter rules) round-trip through `GET/PUT /api/user/filter`.

### Non-goals (this PRD)
- Replacing the in-graph HUD pill (knowledge/ontology/agent) — stays as is.
- Building the standalone NL command surface — Spine acts as the host.
- Re-theming. Spine ships in current visual language.
- VR/XR — tier-2 EXPOSE rows for XR settings, but no XR Spine surface in v1.

## 5. Users & tiers

| Tier | Audience | Visible by default | Gated by | Concrete check |
|---|---|---|---|---|
| 1 — Basic | Analysts, viewers | ~25 rows | none | always |
| 2 — Advanced | Heavy users | ~40 rows total | per-user toggle (sticky) | row 1.30 `user_preferences.advanced_mode` |
| 3 — Power | Owners, integrators | + ~25 rows after divider | NIP-98 pubkey allowlist | `POWER_USER_PUBKEYS` env-var via `is_power_user` flag from auth |
| 4 — Operator | SRE, support | + deployment block (read-only) | role claim | `RequireAuth::admin()` + new `OPERATOR_PUBKEYS` env-var |

**Key invariant**: tiers do not duplicate. A descriptor lives at exactly one tier; the gating filter only varies which tier-N rows are emitted.

## 6. Per-descriptor LLM intelligence layer

The user's request was "wire LLM intelligence to the descriptions system, more granular than the settings LLM chat we already have". **Research finding**: there is no existing settings LLM chat — `CommandInput.tsx` is a regex-only keyword parser with hardcoded mutations (lines 59-160). The dormant `/api/nl-query/*` family exists but is unreachable from the client.

This PRD wires it for the first time. Each descriptor declares an `llmContext` block:

```ts
llmContext?: {
  bounds?: { min: number; max: number; step?: number };
  examples?: string[];                              // "tighter clusters", "show only knowledge"
  explainPrompt?: string;                           // for "what does this do?" affordance
};
```

The Spine renders an inline NL prompt in every editor:

```
┌──────────────────────────────────────────────┐
│ Cluster tightness: tight                  ▼ │
│ ─────────────────────────────────────────── │
│ centerGravityK: ━━━●━ (12.4)                │
│ ┌─ ✨ Describe in your own words ──────────┐ │
│ │ "make clusters bigger"                   │ │ ⏎
│ └──────────────────────────────────────────┘ │
└──────────────────────────────────────────────┘
```

Submit goes to `POST /api/nl-query/translate` with `{intent: <user_text>, context: <descriptor.id, current_value, bounds>}`. Server returns `{action: 'set', path: [...], value: <new>}`. Client applies optimistically + confirms via the existing `autoSaveManager` 500ms debounce. Failures roll back with toast.

A second affordance per row, "what does this do?" (info icon), calls `POST /api/nl-query/explain` with the descriptor metadata and renders the response in a popover.

This **scales to all 88 descriptors** without per-row prompt engineering — the descriptor declares its bounds/examples/explainPrompt; the LLM gets a structured context per call.

## 7. Data model: `Setting<T>` descriptor (full)

```ts
type Tier = 1 | 2 | 3 | 4;
type Category = 'visual' | 'behaviour' | 'data' | 'team' | 'power' | 'operator';
type AuditDecision = 'KEEP' | 'MERGE' | 'EXPOSE' | 'WIRE';

type Setting<T> = {
  id: string;
  path: string[];
  tier: Tier;
  category: Category;
  label: string;
  summary: (v: T, ctx: SpineContext) => string;
  Editor: React.FC<EditorProps<T>>;
  folds?: string[];
  decision?: AuditDecision;
  ref?: string;
  readOnly?: boolean;
  llmContext?: { bounds?: any; examples?: string[]; explainPrompt?: string };
  visibleWhen?: (state: any) => boolean;            // dependent visibility
};
```

Adding a setting = adding one object to `descriptors/index.ts`. The store layer (`settingsStore.ts` Zustand + Trie subscribers) is **unchanged** — descriptors read/write through `path`.

## 8. The summary function

The design work, not styling.

| Bad (label-form) | Good (state-form) |
|---|---|
| Quality: standard | Render at standard quality (AA on, shadows on, AO off) |
| Show clusters: true | Cluster tightness: tight |
| Boundary damping: 0.85 | Boundary feel: standard |
| Auto-pause threshold: 0.04 | Auto-pause when settled (kinetic < 0.04) |
| Verbosity: 2 | Diagnostics: errors only |

### Authoring rules
- Present tense, declarative.
- Numbers only when meaningful at a glance (kinetic thresholds yes; raw spring constants no).
- Compound state needs `detectPreset()` + `'custom'` fallback. Never crash.
- Localisation-ready: summary functions return strings via i18n helper.

## 9. The Spine renderer

~80 lines, stateless on top of the existing store.

```tsx
function Spine({ descriptors, state, dispatch, expandedId, setExpandedId, currentTier }) {
  const visible = descriptors.filter(d => d.tier <= currentTier && (d.visibleWhen?.(state) ?? true));
  const grouped = groupBy(visible, d => d.category);
  return (
    <div className="spine">
      <SpineSearch />
      {Object.entries(grouped).map(([cat, items]) => (
        <Section key={cat} title={cat} divider={cat === 'power' || cat === 'operator'}>
          {items.map(d => (
            <SettingRow
              key={d.id}
              descriptor={d}
              value={getPath(state, d.path)}
              expanded={expandedId === d.id}
              onToggle={() => setExpandedId(expandedId === d.id ? null : d.id)}
              onChange={(v) => dispatch({ type: 'set', path: d.path, value: v })}
            />
          ))}
        </Section>
      ))}
    </div>
  );
}
```

Single-row expansion (accordion) at v1. Multi-row open is a v2 power-user setting (`spine.multi_open`).

## 10. Key interactions

### Search
Top-of-spine input (also ⌘F). Fuzzy match against `summary(value)` + `label` + folded child labels. Matched rows highlight, non-matches dim. Hitting Enter expands the top match.

### Deep links
`?expand=render.quality&tier=power`. Support links straight to the row. Right-click a node in the graph: "edit how this is shown" → links to relevant row, scrolls, expands. URL state via `URLSearchParams` on the existing hash router (no new history layer).

### Drift indication
If a folded child has been edited away from its parent's preset shape, the row's summary shows the `'custom'` form. A "reset to preset" affordance appears in the editor.

### Audit annotations
Off by default. Internal toggle (`?annotate=1`) shows the `decision` chip plus the `§` reference per row.

### Mobile
Below 768px:
- Search becomes a sticky header.
- Section dividers become collapsible.
- Editors render single-column, slider tracks min 44px hit-target.
- Inline NL prompt becomes a fullscreen sheet on focus to avoid keyboard occlusion.
- Single-row accordion (no multi-open even for power users).

## 11. How audit decisions land

| Decision | Count | What changes in the descriptor |
|---|---|---|
| KEEP | ~30 | Descriptor exists; summary rephrased to state-form |
| MERGE | 11 (parents) | Parent has `folds: [childIds]`; children have no top-level descriptor; rendered only inside parent editor |
| CUT | 12 | No descriptor. Store key may persist for back-compat read |
| EXPOSE | 35 | New descriptor wired to a previously server-only key. Tier per audit |
| WIRE | 25 | Descriptor exists; Editor calls the new transport |

**Total: 88 descriptors authored.**

## 12. WIRE work — server endpoints to call

| Audit § | What | Endpoint | Status |
|---|---|---|---|
| 5.38 | Settings profiles save/load | `POST /api/settings/profiles/{id}` | currently STUB → must implement Neo4j persistence |
| 9.7 | GitHub re-sync trigger | `POST /api/admin/sync` | exists, just wire button |
| 8.27-30 | NL query | `POST /api/nl-query/{translate,explain,validate,examples}` | dormant — implement handler stubs first |
| 11.4-11 | Broker/cases/timeline/proposals/KPIs | `/api/broker/*`, `/api/workflows/*`, `/api/mesh-metrics/*` | mocks today — wire to real backend (out of v1 scope, descriptor renders read-only "pending implementation") |
| operator.* | Build SHA, GPU stats, WS subscribers | `GET /api/admin/operator/status` | new endpoint required (defined in this PRD §15) |
| audit | Audit trail viewer | `GET /api/audit/events?since=...` | new endpoint required (out of v1 scope; descriptor displays "no events yet") |

## 13. Migration plan

### Phase 0 — Scaffolding (this PR)
- Land `Setting<T>` type, `Spine` component, `SettingRow`, `SpineSearch`, deep-link plumbing.
- Feature-flag `spine.enabled` per pubkey (env var `SPINE_ENABLED_PUBKEYS`).
- Both surfaces co-exist; same store.
- Author 5 descriptors covering one tab (Visual) end-to-end.

### Phase 1 — Visual category (this PR)
- Highest leverage; most MERGE wins.
- Land `render.quality`, `glow`, `metadata.viz`, `node.visibility`, `cluster.tightness`.
- Delete the Visual tab condition: when `spine.enabled`, the visual tab in old shell is hidden.

### Phase 2 — Behaviour, Data, Team
- 23 descriptors; includes boundary-feel and detail-policy merges.
- Tier-2 reveal logic exercised here.

### Phase 3 — Power & Operator (this PR)
- Pubkey-gated section. Diagnostics tri-state, GitHub sync (WIRE), settings profiles (WIRE+STUB→Neo4j), feature flags.
- Operator block read-only; calls new `GET /api/admin/operator/status`.

### Phase 4 — Cleanup
- Remove tab shell.
- Remove dead store keys (12 CUTs).
- Flip `spine.enabled` default to `true`.

### Phase 5 — Per-descriptor LLM intelligence
- Implement `/api/nl-query/translate`, `/explain`, `/validate`, `/examples`.
- Wire NL prompt into descriptor editor.

This PR (PRD-007 v1) covers Phases 0, 1, 3, 5 in one pass. Phase 2 + Phase 4 follow.

## 14. Multi-tenant model

### Pubkey-scoped tier-3 (power user)
- Read auth pubkey from existing NIP-98 path (`request.pubkey`, `is_power_user` flag from middleware).
- `currentTier(state)` returns 3 if `auth.is_power_user`, else 2 if `user_preferences.advanced_mode`, else 1.
- Spine filters descriptors by `tier <= currentTier`.
- Per-user filter rules already round-trip through `GET/PUT /api/user/filter` (per-pubkey Neo4j-persisted) — Spine row "Filter rules for this account" shows owner's rules only.

### Tier-4 (operator) read-only
- New env var `OPERATOR_PUBKEYS` (CSV, sibling to `POWER_USER_PUBKEYS`).
- Auth middleware sets `is_operator` flag when pubkey matches.
- Operator descriptors are `readOnly: true` and call `GET /api/admin/operator/status`.

### Settings profiles per-user
- `POST /api/settings/profiles` becomes per-pubkey (the current STUB returns a dummy ID with no scoping).
- Schema: `(p:SettingsProfile {id, owner_pubkey, name, json_blob, created_at})`.
- Tier-3 descriptor "Settings profile: {name}" with editor for save/load.

### Cross-user invariant
A user never sees another user's profile. Tier-1/tier-2 descriptors that touch user-scoped data (filter rules, profiles) auto-attach `owner_pubkey` from session.

## 15. New server endpoints (this PRD)

### `POST /api/nl-query/translate`
```json
Request:  { "intent": "make clusters tighter", "context": { "id": "cluster.tightness", "current_value": 8, "bounds": { "min": 0, "max": 50 } } }
Response: { "action": "set", "path": ["visualisation", "graphs", "logseq", "physics", "centerGravityK"], "value": 16, "explanation": "Increased centerGravityK from 8 to 16 for tighter clustering." }
```

### `POST /api/nl-query/explain`
```json
Request:  { "id": "cluster.tightness", "label": "Cluster tightness", "current_value": 8 }
Response: { "explanation": "Cluster tightness controls how strongly nodes pull toward their centre. Higher values produce denser, tighter clusters; lower values let the graph spread out." }
```

### `GET /api/admin/operator/status` (NEW)
Tier-4 read-only composite:
```json
{
  "build": { "version": "0.5.0", "commit_sha": "032ec78058", "build_timestamp": "2026-04-28T19:52:13Z", "rust_version": "1.85" },
  "gpu": { "compute_capability": "8.6", "vram_used_mb": 4127, "vram_total_mb": 49140, "utilisation_percent": 23 },
  "container": { "memory_limit_mb": 32768, "memory_used_mb": 5621, "cpu_cores": 16, "cpu_percent": 12.4 },
  "ws_subscribers": { "total": 4, "per_workspace": { "default": 4 } },
  "db_pool": { "active": 2, "idle": 8, "waiting": 0 },
  "physics": { "iterations_per_sec": 47, "avg_iteration_ms": 21.3, "convergence_detected": true },
  "ontology": { "loaded_count": 3501, "total_axioms": 1321, "total_classes": 3501 }
}
```

Implementation in `src/handlers/operator_status_handler.rs` (new). Behind `RequireAuth::admin()`.

### `POST /api/settings/profiles/{id}` (replace STUB)
Currently returns dummy ID with no persistence. Implement:
- Neo4j node `(p:SettingsProfile {id, owner_pubkey, name, json_blob, created_at, updated_at})`.
- `POST` save: compose from current settings or supplied JSON.
- `GET` list: per-pubkey only.
- `PUT/{id}` update: owner-only.
- `DELETE/{id}` retire: owner-only.

## 16. Risks & open questions

| Risk | Mitigation |
|---|---|
| Summary copy is real work, easy to underestimate | Treat as content design, not strings. Each phase reviewed by two technical writers. |
| Preset detection edge cases produce ugly "custom" labels | Telemetry on `'custom'` rate per merge — anything > 30% is a sign the preset shape is wrong. |
| Single-accordion expansion frustrates users comparing two settings | Mitigation: ⌘click opens a row in side popover without collapsing the current one (v2). |
| Discoverability of folded children | Search MUST index folded-child labels (`children.flatMap(c => c.label)`), not just parent labels. |
| Tier 4 read-only block: in spine or separate `/ops` route? | **Decision**: spine, with strong divider. Confirmed with SRE: a unified surface beats route-switching. |
| Per-user tier-2 reveal: infer or opt in? | **Decision**: opt-in toggle in spine itself (a tier-1 row "Show advanced settings: off / on"). |
| LLM nl-query latency at 60-80 row x 200ms responses | Server-side caching by `(intent_hash, context_hash)`. Default LLM is local Ollama; fallback to OpenAI configured. |
| Multi-tenant data leak through tier-3 descriptors that reference shared resources | All tier-3+ writes go through `RequireAuth::power_user()`; reads through `RequireAuth::optional()` already filter by pubkey on owned resources. |

## 17. Success metrics

| Metric | Today | Target (90 days post-launch) |
|---|---|---|
| Visible controls at first open | ~205 | ≤ 40 |
| Median time to find named setting (usability test) | ~38s | ≤ 12s |
| Settings touched per session | 2.1 | 3.5+ |
| Support tickets tagged "can't find setting" | baseline | −60% |
| Telemetry: rows opened that read 'custom' | n/a | < 30% per merge |
| PU adoption of GitHub sync (WIRE) | 0 | 40% of PU pubkeys |
| LLM nl-query acceptance rate (user accepts suggestion) | n/a | ≥ 60% |
| Mobile usage of Spine | n/a | 25% of sessions |

## 18. Out of scope (later PRDs)

- Replacing the in-graph HUD pill (knowledge/ontology/agent toggles).
- Standalone NL command surface (`/cmd <intent>` floating prompt — Spine hosts the row-scoped variant for now).
- XR Spine surface (palm-anchored panel for Quest/Vision).
- Cross-device settings sync via Solid Pod (ADR-053/056 territory).
- Full audit trail viewer (the descriptor renders "0 events" until the audit table backend lands).

---

**Total descriptor count to author this PRD: 88.**
