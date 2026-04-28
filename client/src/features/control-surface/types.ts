/**
 * Control-Surface bounded context — descriptor type system.
 * See ADR-061 §"Descriptor type — final shape" + DDD-control-surface §Aggregates.
 *
 * The single new abstraction: every existing tab becomes an array of these.
 * Adding a setting is now adding one object.
 */

import type React from 'react';
import type { SettingsPath } from '@/features/settings/config/settings';

export type Tier = 1 | 2 | 3 | 4;

export type Category = 'visual' | 'behaviour' | 'data' | 'team' | 'power' | 'operator';

export type AuditDecision = 'KEEP' | 'MERGE' | 'EXPOSE' | 'WIRE';

/**
 * Snapshot context passed to summary functions and editors.
 * Pure read from settings store + auth state at render time.
 */
export interface SpineContext {
  /** Settings store snapshot. Editors should NOT close over this; use props. */
  state: any;
  /** Current session pubkey, if NIP-98 authenticated. */
  pubkey?: string;
  /** Server-attached flag — pubkey is in POWER_USER_PUBKEYS. */
  isPowerUser: boolean;
  /** Server-attached flag — pubkey is in OPERATOR_PUBKEYS. */
  isOperator: boolean;
}

export interface EditorProps<T> {
  value: T;
  onChange: (next: T) => void;
  context: SpineContext;
  descriptor: Setting<T>;
}

/**
 * Per-descriptor LLM call envelope. Authored once, used by NLPromptInline
 * + the server-side prompt template at /api/nl-query/translate.
 */
export interface LLMContext {
  /** Numeric range hints for slider-like values. */
  bounds?: { min?: number; max?: number; step?: number };
  /** Example utterances shown as quick-pick chips. */
  examples?: string[];
  /** Override for the "what does this do?" plain-language explanation. */
  explainPrompt?: string;
  /** How NL output is applied. Default 'optimistic'. */
  applyMode?: 'optimistic' | 'confirm' | 'dry-run';
}

/**
 * The Setting<T> descriptor. Frozen at module load.
 *
 * Invariants (enforced by registerDescriptors at startup):
 *   - id is unique within the catalogue.
 *   - path resolves to a known leaf in AppFullSettings (or is documented client-only).
 *   - folds reference existing descriptor ids.
 *   - tier ∈ {1, 2, 3, 4}; category is one of the 6 enum values.
 */
export interface Setting<T = unknown> {
  /** Stable URN-compatible slug, e.g. "render.quality". */
  id: string;

  /** Settings store dot-path, e.g. ["visualisation", "rendering", "quality"]. */
  path: ReadonlyArray<string>;

  tier: Tier;
  category: Category;

  /** Short label for search + breadcrumbs. */
  label: string;

  /**
   * THE design work: pure function that returns a present-tense sentence
   * describing the current state of the graph.
   *
   * Examples:
   *   "Render at standard quality (AA on, shadows on, AO off)"
   *   "Cluster tightness: tight"
   *   "Diagnostics: errors only"
   */
  summary: (value: T, ctx: SpineContext) => string;

  /** Editor component rendered when row is expanded. */
  Editor: React.FC<EditorProps<T>>;

  /** Child descriptor ids absorbed by this MERGE parent. */
  folds?: ReadonlyArray<string>;

  decision?: AuditDecision;

  /** Back-ref to audit § (e.g. "§3.4-6"). */
  ref?: string;

  /** Tier-4 operator block — never mutates. */
  readOnly?: boolean;

  llm?: LLMContext;

  /** Conditional visibility based on store state. */
  visibleWhen?: (state: any) => boolean;
}

/**
 * Spine session aggregate per ADR-061 §State / store interaction.
 * Lives in URL (deep-link grammar) + ephemeral React state.
 */
export interface SpineSession {
  expandedId: string | null;
  searchQuery: string;
  /** Visual demotion only — never escalates beyond auth-attached tier. */
  tierOverride: Tier | null;
  /** Off by default; ?annotate=1 flips on for internal review. */
  annotate: boolean;
}

/**
 * Settings store path utility — narrows to known SettingsPath strings when possible
 * but accepts the descriptor's ReadonlyArray<string> for dynamic descriptor lookup.
 */
export function pathToString(path: ReadonlyArray<string>): SettingsPath {
  return path.join('.') as SettingsPath;
}
