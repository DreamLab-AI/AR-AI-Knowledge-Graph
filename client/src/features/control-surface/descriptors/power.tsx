/**
 * Power-user category descriptors (tier 3). Pubkey-gated.
 * Includes WIRE entities: GitHub sync, settings profiles, audit trail, NL command.
 */

import React, { useEffect, useState } from 'react';
import type { Setting } from '../types';
import {
  BooleanEditor,
  makeActionEditor,
  makeEnumEditor,
  ReadOnlyEditor,
} from '../editors';
import { unifiedApiClient } from '@/services/api/UnifiedApiClient';

// ─── github.sync (WIRE — endpoint exists at POST /api/admin/sync) ─────────

interface GitHubSyncState {
  lastSync?: string;
  inProgress?: boolean;
  lastError?: string;
}

export const githubSyncTrigger: Setting<GitHubSyncState> = {
  id: 'github.sync',
  path: ['admin', 'github_sync_state'] as const,
  tier: 3,
  category: 'power',
  label: 'Re-sync graph from GitHub',
  decision: 'WIRE',
  ref: 'audit §9.7',
  summary: (v) => {
    if (v?.inProgress) return 'GitHub sync: running…';
    if (v?.lastError) return `GitHub sync: error — ${v.lastError}`;
    if (v?.lastSync) return `GitHub last synced: ${v.lastSync}`;
    return 'GitHub sync: never';
  },
  Editor: makeActionEditor<GitHubSyncState>({
    buttonLabel: 'Re-sync now',
    onClick: async () => {
      await unifiedApiClient.post('/api/admin/sync', {});
    },
  }),
  llm: {
    examples: ['sync now', 'pull latest from github'],
    explainPrompt:
      'Triggers POST /api/admin/sync. Pulls fresh markdown + ontology from the configured GitHub repo.',
  },
};

// ─── settings.profiles (WIRE — replaces STUB) ──────────────────────────

interface SettingsProfile {
  id: string;
  name: string;
  created_at?: string;
}

export const settingsProfiles: Setting<SettingsProfile[]> = {
  id: 'settings.profiles',
  path: ['user_preferences', 'profiles'] as const,
  tier: 3,
  category: 'power',
  label: 'Settings profiles',
  decision: 'WIRE',
  ref: 'audit §5.38',
  summary: (v) => {
    if (!Array.isArray(v) || v.length === 0) return 'No saved profiles';
    return `${v.length} saved profile${v.length === 1 ? '' : 's'}: ${v
      .slice(0, 3)
      .map((p) => p.name)
      .join(', ')}${v.length > 3 ? '…' : ''}`;
  },
  Editor: ({ value, onChange }) => {
    const [busy, setBusy] = useState(false);
    const [name, setName] = useState('');
    const [error, setError] = useState<string | null>(null);

    const refresh = async () => {
      try {
        const res = await unifiedApiClient.get<SettingsProfile[]>(
          '/api/settings/profiles'
        );
        if (Array.isArray(res?.data)) onChange(res.data);
      } catch (e: any) {
        setError(e?.message ?? 'fetch failed');
      }
    };

    useEffect(() => {
      void refresh();
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    const save = async () => {
      if (!name.trim()) return;
      setBusy(true);
      setError(null);
      try {
        await unifiedApiClient.post('/api/settings/profiles', { name });
        setName('');
        await refresh();
      } catch (e: any) {
        setError(e?.message ?? 'save failed');
      } finally {
        setBusy(false);
      }
    };

    const load = async (id: string) => {
      setBusy(true);
      try {
        await unifiedApiClient.post(`/api/settings/profiles/${id}/apply`, {});
      } catch (e: any) {
        setError(e?.message ?? 'apply failed');
      } finally {
        setBusy(false);
      }
    };

    const remove = async (id: string) => {
      setBusy(true);
      try {
        await unifiedApiClient.delete(`/api/settings/profiles/${id}`);
        await refresh();
      } catch (e: any) {
        setError(e?.message ?? 'delete failed');
      } finally {
        setBusy(false);
      }
    };

    return (
      <div className="space-y-2">
        <div className="flex gap-2">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Profile name"
            className="flex-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm"
            disabled={busy}
          />
          <button
            type="button"
            onClick={() => void save()}
            disabled={busy || !name.trim()}
            className="rounded bg-sky-600 px-3 py-1 text-sm font-medium text-white hover:bg-sky-500 disabled:opacity-50"
          >
            Save current
          </button>
        </div>
        {Array.isArray(value) && value.length > 0 && (
          <ul className="space-y-1">
            {value.map((p) => (
              <li
                key={p.id}
                className="flex items-center gap-2 rounded bg-slate-100 dark:bg-slate-900/60 px-2 py-1 text-sm"
              >
                <span className="flex-1 truncate">{p.name}</span>
                <button
                  type="button"
                  onClick={() => void load(p.id)}
                  className="text-xs text-sky-600 hover:underline"
                >
                  apply
                </button>
                <button
                  type="button"
                  onClick={() => void remove(p.id)}
                  className="text-xs text-rose-600 hover:underline"
                >
                  delete
                </button>
              </li>
            ))}
          </ul>
        )}
        {error && (
          <div className="text-xs text-rose-600 dark:text-rose-400">{error}</div>
        )}
      </div>
    );
  },
  llm: {
    examples: ['save current as “demo”', 'apply demo profile'],
    explainPrompt:
      'Save current settings as a named profile and re-apply later. Per-pubkey scoped.',
  },
};

// ─── diagnostics.tri (MERGE — folds debug toggles into 3-state) ─────────

export const diagnosticsTri: Setting<'off' | 'errors' | 'verbose'> = {
  id: 'diagnostics.tri',
  path: ['system', 'diagnostics_level'] as const,
  tier: 3,
  category: 'power',
  label: 'Diagnostics',
  decision: 'MERGE',
  ref: 'audit disconnect #3',
  folds: [
    'system.debug.enabled',
    'developer_config.debug_mode',
    'developer_config.show_performance_stats',
  ],
  summary: (v) => `Diagnostics: ${v ?? 'off'}`,
  Editor: makeEnumEditor<'off' | 'errors' | 'verbose'>([
    { value: 'off', label: 'Off' },
    { value: 'errors', label: 'Errors only' },
    { value: 'verbose', label: 'Verbose' },
  ]),
  llm: {
    examples: ['errors only', 'turn diagnostics off', 'verbose mode'],
    explainPrompt:
      'Tri-state replacement for the scattered debug toggles (system.debug + developer_config.debug_mode + perf stats).',
  },
};

// ─── audit.trail (WIRE — placeholder until audit table backend lands) ──

interface AuditEntry {
  id: string;
  timestamp: string;
  pubkey?: string;
  action: string;
}

export const auditTrail: Setting<AuditEntry[]> = {
  id: 'audit.trail',
  path: ['admin', 'audit_trail_recent'] as const,
  tier: 3,
  category: 'power',
  label: 'Audit trail',
  decision: 'WIRE',
  summary: (v) => {
    if (!Array.isArray(v)) return 'Audit trail: 0 events (not yet implemented)';
    return `Audit trail: ${v.length} events (last 24h)`;
  },
  Editor: ReadOnlyEditor,
  readOnly: true,
};

export const POWER_DESCRIPTORS: ReadonlyArray<Setting<any>> = [
  githubSyncTrigger,
  settingsProfiles,
  diagnosticsTri,
  auditTrail,
];
