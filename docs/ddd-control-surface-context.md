# DDD: Control-Surface Bounded Context

**Status:** Accepted
**Date:** 2026-04-28
**Related:** PRD-007 (Unified control surface — Spine), ADR-061 (Spine descriptor architecture), ADR-050 (sovereign visibility), ADR-059 (bidirectional agent channel)

## Bounded context

The Control-Surface bounded context owns the user-facing settings experience: the descriptor catalogue, tier gating, deep-linking, search, and the Spine renderer. It collaborates with — but is **not** — the existing Settings persistence context (Zustand store + `AppFullSettings` server schema) and the new NL-query context.

```
                           ┌────────────────────────────┐
                           │  Control-Surface context   │
                           │  (this DDD)                │
                           │                            │
            UI events ──── │  Descriptor catalogue      │
                           │  Spine renderer            │
                           │  Tier-gating policy        │
                           │  Deep-link router          │
                           │  NL prompt envelope        │
                           └────┬───────────────────────┘
                                │ reads / writes via path
                  ┌─────────────┴──────────────┐
                  ▼                            ▼
   ┌────────────────────────┐   ┌─────────────────────────┐
   │  Settings context      │   │  NL-query context       │
   │  (Zustand + autoSave   │   │  (translate / explain   │
   │  + REST + WS)          │   │  / validate / examples) │
   └────────────────────────┘   └─────────────────────────┘
                  │                            │
                  ▼                            ▼
   ┌────────────────────────┐   ┌─────────────────────────┐
   │  Server schema         │   │  LLM backends           │
   │  AppFullSettings       │   │  (Ollama default,       │
   │  Neo4j SettingsProfile │   │   OpenAI fallback)      │
   └────────────────────────┘   └─────────────────────────┘
```

Control-Surface knows nothing about how settings are persisted or replicated; it only knows the path-based access contract. The NL-query context knows nothing about descriptor identity beyond the structured envelope per ADR-061 §LLM call envelope.

## Aggregates

### Aggregate 1 — `DescriptorCatalogue`

The flat array of `Setting<T>` exported from `descriptors/index.ts`. Frozen at module load; not mutable at runtime.

**Invariants**:
- Every `id` is unique within the catalogue.
- Every `path` resolves to a known leaf in the `AppFullSettings` schema OR is documented as client-only state in PRD-007 §11.
- A descriptor with `folds` lists only descriptor ids that exist in the catalogue at tiers ≤ parent's tier (no parent at tier 1 with folds at tier 3).
- A `MERGE` parent's children have no top-level descriptor (verified at module load).
- `tier ∈ {1, 2, 3, 4}`; `category` is one of the 6 enum values.

**Behaviours**:
- `findById(id) → Setting<unknown> | undefined`
- `byCategory(cat) → Setting[]`
- `byTier(maxTier) → Setting[]`
- `expand(id) → { descriptor, foldedChildren }` for MERGE parents

### Aggregate 2 — `SpineSession`

The active Spine UI state for one tab/window. Lives in URL + ephemeral React state.

**Invariants**:
- At most one row expanded at v1 (`expandedId` is `string | null`).
- `currentTier ≤ effectiveTier(authState, prefs)` — the user can demote their tier visually but never escalate beyond their auth.
- `searchQuery` is a string (max 256 chars) or null.
- `annotate` is a boolean (default false; `?annotate=1` flips on).

**State transitions**:
- `expand(id)` from `null` → `id`
- `collapse()` from `id` → `null`
- `toggle(id)`: from `id` → `null`; from anything else → `id`
- `search(q)`: replaces query; if any rows match, top match auto-expands on Enter
- `setTier(tier)`: visual demotion only

### Aggregate 3 — `TierPolicy`

The function `currentTier(authState, prefs) → Tier`.

**Invariants**:
- Defaults to 1.
- Returns 2 iff `prefs.advanced_mode === true` (per-user sticky toggle).
- Returns 3 iff server-attached `auth.is_power_user === true` (server-authoritative; client mirror only).
- Returns 4 iff server-attached `auth.is_operator === true` (server-authoritative).
- A user override (`?tier=power`) cannot escalate above the auth-attached tier — server enforcement is the security boundary.

### Aggregate 4 — `NLPromptEnvelope`

The structured payload sent to `/api/nl-query/translate` per ADR-061.

**Invariants**:
- `intent` is non-empty; ≤ 1024 chars.
- `descriptor` echoes the canonical descriptor metadata (id, label, path, tier, category, current_value, bounds, examples).
- `session_pubkey` is optional and informational — server reads from request auth, not envelope.

**Server response invariants** (enforced by the NL-query context, mirrored here):
- `path` matches the descriptor's path exactly (rejection otherwise).
- `value` is within `bounds` if bounds are declared.
- `confidence` ∈ [0, 1]; client may render uncertainty differently below 0.5.

## Domain events

| Event | Producer | Consumer |
|---|---|---|
| `DescriptorExpanded { id, pubkey?, timestamp }` | `Spine` | telemetry (success metric §17 of PRD-007) |
| `SettingMutationApplied { id, oldValue, newValue, source: 'ui'\|'nl'\|'preset' }` | `SettingRow` | autoSaveManager (existing); audit (future) |
| `NLQueryTranslated { intent, descriptor_id, suggestion, accepted: bool }` | `NLPromptInline` | telemetry (LLM acceptance rate metric) |
| `TierGateRefused { descriptor_id, requested_tier, current_tier }` | `Spine` | telemetry (UX gap signal) |
| `DeepLinkOpened { id, source: 'support'\|'graph'\|'cmd' }` | `SpineProvider` | telemetry (support handoff metric) |

## Anti-corruption layer (ACL)

The Control-Surface context speaks descriptor language. The Settings context speaks `AppFullSettings` schema. The translation layer:

- `descriptor.path` → store path: passthrough.
- `value: T` from descriptor's editor → store value: typed identity (T mirrors the schema leaf type).
- Server-side write rejection (validation failure, tier denial) → toast + revert to last-known-good. The Spine never sees raw `400`/`403` codes; only structured `{ ok: false, reason: <enum> }` from `autoSaveManager`.

The NL-query context's response (`{action, path, value}`) goes through the **same ACL** — the Spine doesn't apply NL output directly; it dispatches a normal `{type:'set', path, value}` action so the autoSave + retry pipeline catches any server-side rejection identically.

## Ubiquitous language

| Term | Definition (binding for code, prose, UI copy) |
|---|---|
| **Descriptor** | A `Setting<T>` object. Never "field", "knob", "control" in code. |
| **Spine** | The single scrollable list. Never "panel", "drawer", "settings page". |
| **Tier** | 1-4 integer denoting visibility scope. Never "level", "permission". |
| **MERGE parent / folded child** | A descriptor with `folds: [...]` and the descriptors it absorbs. |
| **Summary (function)** | The pure `(value, ctx) → string` describing current state. Never "label" or "title". |
| **Editor** | The React component rendered when a row is expanded. Never "control" or "widget". |
| **NL prompt** | The inline text input wired to `/api/nl-query/translate`. Never "AI box", "search bar", "command line". |
| **Audit decision** | `KEEP \| MERGE \| EXPOSE \| WIRE \| CUT` from the audit. |
| **Power user (PU)** | Pubkey in `POWER_USER_PUBKEYS`. Never "admin" (that's tier 4). |
| **Operator** | Pubkey in `OPERATOR_PUBKEYS` env. Never "SRE", "support", "root". |
| **Tenant** | A pubkey-identified user. Multi-tenant invariants attach to this term. |
| **Deep link** | A URL with `?expand=<id>` or related. Never "permalink" or "anchor". |
| **Drift** | When a folded child is edited away from its parent's preset; surfaced as `'custom'` in the parent summary. |

## Multi-tenant invariants

I01. A tier-1 user MUST never see a tier-3 row (server-side `RequireAuth` enforces; client-side filter mirrors).

I02. A user's per-pubkey resources (filter rules, settings profiles) MUST round-trip through the existing per-pubkey-scoped routes (`GET/PUT /api/user/filter`, new `/api/settings/profiles`); never global.

I03. Tier-4 operator descriptors are `readOnly: true` AND served by `RequireAuth::admin()`; mutating them is impossible from the Spine.

I04. NL-query intents that resolve to a path the user does not have write authority over are rejected at `/api/nl-query/translate` with `{ ok: false, reason: 'tier_denied' }`. The Spine displays "this would change settings you don't have access to" and offers no apply button.

I05. Settings profiles attach `owner_pubkey` from the session; cross-pubkey reads are server-side filtered. A user enumerating profiles sees only their own.

I06. Deep-link `?tier=power` cannot escalate; it can only narrow visibility. Server checks remain authoritative.

I07. The Spine never logs full descriptor values (some carry secrets, e.g. `OPENAI_API_KEY` exposed at tier 4). Only `descriptor.id` and `category` go to telemetry.

I08. Drift detection (`'custom'` summary form) MUST NOT leak across tenants — folded children's values are per-tenant when the path is per-tenant; the summary function reads only the current tenant's snapshot.

## Domain services

### `DescriptorRegistry`
Loads `descriptors/index.ts` at module load; validates aggregate invariants; throws on duplicate IDs or invalid `folds`. Single instance per page load.

### `TierResolver`
`(authState, userPrefs) → Tier`. Pure. Reads `auth.is_power_user`, `auth.is_operator`, `userPrefs.advanced_mode`.

### `SummaryRenderer`
Composes `descriptor.summary(value, ctx)` plus i18n helper. Memoises by `(id, JSON.stringify(value))`.

### `NLPromptDispatcher`
Handles per-row "describe in your own words" submissions: builds envelope per ADR-061 §LLM, posts to `/api/nl-query/translate`, dispatches the resulting action through autoSaveManager. Caches client-side by `(intent_hash, descriptor.id, current_value_hash)`.

### `DeepLinkRouter`
Bidirectional URL ↔ `SpineSession` aggregate. Reads `URLSearchParams` on mount; writes on state change.

## Cross-context contracts

| Source | Sink | Wire format | Backpressure |
|---|---|---|---|
| Spine | Settings context | `dispatch({ type: 'set', path, value })` | autoSaveManager 500ms debounce; SettingsRetryManager exponential |
| Spine | NL-query context | `POST /api/nl-query/translate` | Client cache `(intent_hash, ctx_hash)`; 10 req/s rate-limit |
| Spine | Operator status (tier 4) | `GET /api/admin/operator/status` | 5s polling when block visible; pause when collapsed |
| Settings context | Spine | `subscribe(path, cb)` Trie | n/a |
| Auth middleware | Spine | `useAuth()` returns `{is_power_user, is_operator, pubkey}` | n/a — pushed once per session |

## Out of scope

- The descriptor catalogue does NOT govern which settings are persisted to Solid Pod (ADR-053/056) — that's a Pod context concern.
- The Spine does NOT own audit-trail recording — that's an Audit bounded context (future).
- The Spine does NOT own theme tokens — global theming is its own context (ADR territory; not yet ADR'd).
