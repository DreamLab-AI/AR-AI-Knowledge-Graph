# ADR-061: Spine descriptor architecture for unified control surface

**Status:** Accepted
**Date:** 2026-04-28
**Author:** VisionClaw platform team
**Supersedes:** —
**Related:**
- PRD-007 (Unified control surface — Spine)
- `docs/ddd-control-surface-context.md` (Domain model)
- ADR-050 (sovereign visibility — informs tier-3 multi-tenant gating)
- ADR-059 (bidirectional agent channel — informs the per-descriptor LLM call shape: structured envelope, `pubkey` carried, intent-scoped)
- ADR-013 (canonical URI grammar — descriptor IDs are URN-compatible names; deep-link query strings are ASCII slugs)

## TL;DR

This ADR fixes the technical shape of PRD-007. Three commitments:

1. **One descriptor type, one renderer, one store**: `Setting<T>` lives in `client/src/features/control-surface/descriptors/`; `Spine` lives in `client/src/features/control-surface/Spine.tsx`; the existing Zustand `settingsStore` is the only state holder. Descriptors read/write via path; the store is unaware of the Spine.
2. **Tier gating is a single global filter, not separate UIs**. `currentTier(authState, prefs)` returns `1 | 2 | 3 | 4`; the renderer filters `descriptors.filter(d => d.tier <= currentTier)`. Pubkey allowlist (`POWER_USER_PUBKEYS`, `OPERATOR_PUBKEYS`) gates tier 3 and 4.
3. **Per-descriptor LLM intelligence routes through `/api/nl-query/*`**: intent + descriptor context goes to `translate`, server returns `{action, path, value}`; client applies optimistically through the existing `autoSaveManager` 500ms debounce. No new client write path.

## Context

PRD-007 specifies the user-facing shape. The architecture must answer: where does the descriptor type live, how does it interoperate with the existing Zustand store, how is tier gating enforced both client-side (UX) and server-side (security), and how is the LLM call shape stable across 88 descriptors without per-row prompt engineering. Three decisions were considered for each axis and one chosen.

### Axis 1 — Descriptor location

**Considered**:

(a) **Co-located with feature directories** (e.g. `physics/descriptors/cluster-tightness.ts`).
(b) **Single `client/src/features/control-surface/descriptors/`** directory grouped by category subdirectory.
(c) **Server-authored** — descriptors live in Rust, codegen TypeScript, single source of truth.

(c) was rejected: summary functions are i18n-bound user-facing copy, not server policy. (a) was rejected: descriptors cross-cut features (e.g. `cluster.tightness` touches physics + visualisation); co-location creates import cycles. **(b) chosen**.

### Axis 2 — Renderer state ownership

**Considered**:

(a) **Renderer-owned** — `Spine` component holds expand/search state in React state.
(b) **Store-owned** — extend `settingsStore` with `spine: { expandedId, searchQuery, currentTier }`.
(c) **URL-owned** — every interactive bit of UI state lives in the URL.

**(c) chosen** for `expandedId`, `searchQuery`, `currentTier` (when explicitly overridden); (a) for transient editor state (focus, drag handle position). Rationale: deep-link is a hard PRD requirement; lifting state into the URL gives it for free, with React state as a fallback when the URL is empty.

### Axis 3 — Tier gating enforcement

**Considered**:

(a) **Client-side filter only** — server treats every authenticated user identically.
(b) **Server-side authority + client-side mirror** — server is authoritative; client filters as UX hint.
(c) **Cryptographic delegation** — capabilities ride on signed payloads.

**(b) chosen**. Pubkey allowlist envs (`POWER_USER_PUBKEYS`, new `OPERATOR_PUBKEYS`) are the source of truth on the server; auth middleware sets `is_power_user`/`is_operator` flags in the request context; client mirrors via the auth response. Server-side `RequireAuth::power_user()` / `RequireAuth::admin()` catch any descriptor that mutates a tier-3/tier-4 path. Client filter is a UX shortcut, not a security boundary. (c) is the long-term direction (NIP-26 delegation per ADR-059 Phase 5) but out of scope here.

### Axis 4 — LLM call shape

**Considered**:

(a) **Per-descriptor prompt** — every descriptor has a hand-tuned system prompt.
(b) **Generic envelope** — descriptors declare `{bounds, examples, explainPrompt}`; server constructs the prompt.
(c) **No LLM** — descriptors stay deterministic with sliders only.

**(b) chosen**. Scales to 88 descriptors without 88 prompts. Server-side prompt template lives in `src/handlers/nl_query_handler.rs`; descriptors supply structured context. (c) was rejected because the dormant `/api/nl-query/*` infrastructure has been on the floor for months and a goal is to wire it.

### Axis 5 — Mobile rendering

**Considered**:

(a) **Same component, CSS breakpoints** — single React tree; CSS handles responsive behaviour.
(b) **Component branch** — `<Spine />` vs `<MobileSpine />` based on `useMediaQuery`.
(c) **Platform-specific bundles** — separate desktop / mobile builds.

**(a) chosen**. Tailwind breakpoints (`sm:`, `md:`) on a single component tree; `useMediaQuery` only used to switch behaviour (NL prompt becomes fullscreen sheet on focus below 768px). (b) was rejected: descriptor authors should not need to think about mobile; the Spine handles it.

## Decision

### Descriptor type — final shape

```ts
// client/src/features/control-surface/types.ts

export type Tier = 1 | 2 | 3 | 4;
export type Category = 'visual' | 'behaviour' | 'data' | 'team' | 'power' | 'operator';
export type AuditDecision = 'KEEP' | 'MERGE' | 'EXPOSE' | 'WIRE';

export interface SpineContext {
  state: any;                                           // store snapshot
  pubkey?: string;                                      // current session
  isPowerUser: boolean;
  isOperator: boolean;
}

export interface EditorProps<T> {
  value: T;
  onChange: (next: T) => void;
  context: SpineContext;
  descriptor: Setting<T>;
}

export interface LLMContext {
  bounds?: { min?: number; max?: number; step?: number };
  examples?: string[];
  explainPrompt?: string;
  applyMode?: 'optimistic' | 'confirm' | 'dry-run';      // default: optimistic
}

export interface Setting<T = any> {
  id: string;                                           // "render.quality", URN-compatible slug
  path: ReadonlyArray<string>;                          // ["visualisation","rendering","quality"]
  tier: Tier;
  category: Category;
  label: string;
  summary: (value: T, ctx: SpineContext) => string;
  Editor: React.FC<EditorProps<T>>;
  folds?: ReadonlyArray<string>;                        // child descriptor ids
  decision?: AuditDecision;
  ref?: string;                                         // back-ref to audit §
  readOnly?: boolean;
  llm?: LLMContext;
  visibleWhen?: (state: any) => boolean;
}
```

### File layout

```
client/src/features/control-surface/
├── types.ts                          # Setting<T>, EditorProps, etc
├── Spine.tsx                         # Renderer (~120 lines)
├── SpineProvider.tsx                 # URL state + currentTier hook
├── SettingRow.tsx                    # Single row + accordion
├── SpineSearch.tsx                   # ⌘F input + fuzzy match
├── NLPromptInline.tsx                # Per-row "describe in your own words"
├── editors/
│   ├── EnumEditor.tsx
│   ├── BooleanEditor.tsx
│   ├── NumberEditor.tsx
│   ├── ColorEditor.tsx
│   ├── PresetEditor.tsx              # for MERGE parents
│   └── ReadOnlyEditor.tsx            # for tier-4 operator
├── descriptors/
│   ├── index.ts                      # exports DESCRIPTORS array (length 88)
│   ├── visual/
│   │   ├── render-quality.ts         # MERGE parent (folds aa, shadows, ao, envIntensity)
│   │   ├── glow.ts
│   │   ├── node-visibility.ts
│   │   └── ...
│   ├── behaviour/
│   ├── data/
│   ├── team/
│   ├── power/
│   └── operator/
├── llm/
│   ├── client.ts                     # POST /api/nl-query/translate, /explain
│   └── cache.ts                      # (intent_hash, context_hash) → response
└── deep-link.ts                      # parse / serialise ?expand=...&tier=...
```

### State / store interaction

The existing Zustand `settingsStore` and its `path()` access pattern are **unchanged**. The Spine reads via `getPath(state, descriptor.path)` and writes via `dispatch({ type: 'set', path, value })`. autoSaveManager continues to handle debouncing, retries, and category-based batching. No new write-path.

### Server endpoints introduced

| Route | Handler | Auth | Purpose |
|---|---|---|---|
| `POST /api/nl-query/translate` | `src/handlers/nl_query_handler.rs` (new) | `RequireAuth::optional()` | Translate intent + descriptor context → `{action, path, value, explanation}` |
| `POST /api/nl-query/explain` | same | `RequireAuth::optional()` | Plain-language explanation of a descriptor |
| `POST /api/nl-query/validate` | same | `RequireAuth::write_settings()` | Dry-run: would this mutation be accepted? |
| `POST /api/nl-query/examples` | same | `RequireAuth::optional()` | Example utterances for a descriptor |
| `GET /api/admin/operator/status` | `src/handlers/operator_status_handler.rs` (new) | `RequireAuth::admin()` | Composite read-only operator block (build SHA, GPU stats, WS subscribers, …) |
| `POST/GET/PUT/DELETE /api/settings/profiles[/{id}]` | `src/handlers/settings_handler/profiles.rs` (replace STUB) | `RequireAuth::authenticated()` | Per-pubkey settings profiles, Neo4j-persisted |

### Multi-tenant model — concrete flow

```
client request → middleware/auth.rs
                   ├── extracts NIP-98 pubkey from `Authorization: Nostr ...`
                   ├── checks POWER_USER_PUBKEYS env-var (CSV) → sets is_power_user
                   ├── checks OPERATOR_PUBKEYS env-var (CSV) → sets is_operator
                   └── attaches both to AuthenticatedUser

handler ──┬─ RequireAuth::optional() — reads OK for any
          ├─ RequireAuth::power_user() — needs is_power_user=true
          ├─ RequireAuth::admin() — needs is_operator=true
          └─ per-pubkey scoping at query level (e.g. /api/user/filter
             auto-attaches owner_pubkey from session — never global)
```

Client mirrors via `useAuth()` hook → `currentTier(authState, prefs) → 1|2|3|4`. The server is authoritative; client filter is UX hint.

### Deep-link grammar

```
?expand=<descriptor.id>            # opens that row
?expand=<id>&tier=<1|2|3|4|max>    # forces tier filter
?annotate=1                        # show audit decision chips + § refs
?q=<search>                        # populate search box
```

Multiple values comma-separated where applicable. Parsed/serialised in `deep-link.ts`. Implements `useEffect(() => setUrlParams(...))` on state change.

### LLM call envelope

```jsonc
// POST /api/nl-query/translate
// Request
{
  "intent": "make clusters tighter",
  "descriptor": {
    "id": "cluster.tightness",
    "label": "Cluster tightness",
    "path": ["visualisation","graphs","logseq","physics","centerGravityK"],
    "tier": 1,
    "category": "behaviour",
    "current_value": 8,
    "bounds": { "min": 0, "max": 50, "step": 0.1 },
    "examples": ["tighter clusters", "let the graph spread out"]
  },
  "session_pubkey": "abc123..."          // optional, server still authoritative
}

// Response (200)
{
  "action": "set",
  "path": ["visualisation","graphs","logseq","physics","centerGravityK"],
  "value": 16,
  "summary_after": "Cluster tightness: tight",
  "explanation": "Increased centerGravityK from 8 to 16 for tighter clustering.",
  "confidence": 0.91
}
```

Server-side prompt template lives in `nl_query_handler.rs`; descriptors do not author prompts.

## Consequences

**Positive.**

- Adding a setting is one file. The store, autoSave, and retry layers do not move.
- Tier gating is one filter. Tier-3/4 access flips by env-var change — no UI rebuild.
- Per-descriptor LLM intelligence scales without per-row prompt engineering.
- URL-state deep-links support the PRD's hard requirement and give support / team handoff for free.
- Mobile is one component tree, not two — descriptor authors don't think about it.

**Negative.**

- Summary functions are real work. 88 descriptors × ~10 minutes of copywriting = ~15 hours. Mitigation: phase the rollout as PRD-007 §13 specifies; treat as content design; review with two technical writers per phase.
- The new `/api/nl-query/*` endpoints carry actual LLM costs. Mitigation: server-side `(intent_hash, context_hash)` cache; default backend Ollama on the same network; OpenAI fallback configured but not default.
- Two surfaces (old tabs + new spine) co-exist for one release cycle. Mitigation: feature-flag `spine.enabled` per pubkey; flip default after Phase 4.

**Reversible?** Yes through Phase 3. Phase 4 (tab-shell removal + dead-key delete) is the irreversible step; gates on telemetry showing > 40% of sessions opened > 1 row in the spine.

## References

- Code:
  - Client: `client/src/features/control-surface/` (new), `client/src/store/settingsStore.ts` (existing, unchanged), `client/src/features/visualisation/components/IntegratedControlPanel.tsx` (existing, deprecated by Phase 4)
  - Server: `src/handlers/nl_query_handler.rs` (new), `src/handlers/operator_status_handler.rs` (new), `src/handlers/settings_handler/profiles.rs` (replace STUB), `src/middleware/auth.rs` (extend with `is_operator` flag)
  - Settings: `src/config/app_settings.rs` (no change), `src/handlers/settings_handler/routes.rs` (no change)
- Audit:
  - `docs/control-surface-audit/comprehensive-settings-table.md`
  - `docs/control-surface-audit/aspirational-inventory.md`
  - 6 raw audit files in `docs/control-surface-audit/raw/`
