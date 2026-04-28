/**
 * Team category descriptors. Per-user, multi-tenant, identity.
 */

import type { Setting } from '../types';
import { BooleanEditor, ReadOnlyEditor, makeEnumEditor } from '../editors';

// ─── auth.nostr.connected (read-only state indicator at tier 1) ─────────

export const authConnected: Setting<boolean> = {
  id: 'auth.connected',
  path: ['auth', 'nostr', 'connected'] as const,
  tier: 1,
  category: 'team',
  label: 'Identity status',
  decision: 'KEEP',
  readOnly: true,
  summary: (v) => (v ? 'Signed in via Nostr' : 'Not signed in'),
  Editor: ReadOnlyEditor,
};

// ─── auth.nostr.publicKey (read-only display) ───────────────────────────

export const authPubkey: Setting<string | null | undefined> = {
  id: 'auth.pubkey',
  path: ['auth', 'nostr', 'publicKey'] as const,
  tier: 1,
  category: 'team',
  label: 'Your pubkey',
  decision: 'KEEP',
  readOnly: true,
  summary: (v) => {
    if (!v) return 'No pubkey (anonymous)';
    return `Pubkey: ${String(v).slice(0, 12)}…${String(v).slice(-4)}`;
  },
  Editor: ReadOnlyEditor,
};

// ─── advanced mode toggle (the per-user tier-2 reveal — sticky) ─────────

export const advancedMode: Setting<boolean> = {
  id: 'user.advanced_mode',
  path: ['user_preferences', 'advanced_mode'] as const,
  tier: 1,
  category: 'team',
  label: 'Show advanced settings',
  decision: 'EXPOSE',
  ref: 'PRD-007 §16 (per-user tier-2 reveal)',
  summary: (v) =>
    v
      ? 'Advanced settings: shown'
      : 'Advanced settings: hidden (toggle to reveal tier-2 rows)',
  Editor: BooleanEditor,
  llm: { examples: ['show advanced', 'hide advanced'] },
};

// ─── language ───────────────────────────────────────────────────────────

export const userLanguage: Setting<string> = {
  id: 'user.language',
  path: ['user_preferences', 'language'] as const,
  tier: 1,
  category: 'team',
  label: 'Language',
  decision: 'EXPOSE',
  summary: (v) => `Language: ${v || 'auto'}`,
  Editor: makeEnumEditor<string>([
    { value: 'auto', label: 'Auto-detect' },
    { value: 'en', label: 'English' },
    { value: 'es', label: 'Español' },
    { value: 'fr', label: 'Français' },
    { value: 'de', label: 'Deutsch' },
    { value: 'ja', label: '日本語' },
  ]),
  llm: { examples: ['english', 'español', 'auto-detect'] },
};

// ─── persistSettings ────────────────────────────────────────────────────

export const persistSettings: Setting<boolean> = {
  id: 'system.persistSettings',
  path: ['system', 'persistSettings'] as const,
  tier: 2,
  category: 'team',
  label: 'Sync settings to server',
  decision: 'KEEP',
  summary: (v) =>
    v ? 'Settings sync to server' : 'Settings stay in this browser only',
  Editor: BooleanEditor,
  llm: { examples: ['turn off sync', 'enable sync'] },
};

export const TEAM_DESCRIPTORS: ReadonlyArray<Setting<any>> = [
  authConnected,
  authPubkey,
  advancedMode,
  userLanguage,
  persistSettings,
];
