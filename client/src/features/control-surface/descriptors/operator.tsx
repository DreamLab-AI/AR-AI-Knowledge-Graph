/**
 * Operator category descriptors (tier 4). Read-only. Calls
 * GET /api/admin/operator/status (new endpoint per ADR-061).
 */

import React, { useEffect, useState } from 'react';
import type { Setting, EditorProps } from '../types';
import { ReadOnlyEditor } from '../editors';
import { unifiedApiClient } from '@/services/api/UnifiedApiClient';

interface OperatorStatus {
  build?: { version?: string; commit_sha?: string; build_timestamp?: string; rust_version?: string };
  gpu?: { compute_capability?: string; vram_used_mb?: number; vram_total_mb?: number; utilisation_percent?: number };
  container?: { memory_limit_mb?: number; memory_used_mb?: number; cpu_cores?: number; cpu_percent?: number };
  ws_subscribers?: { total?: number; per_workspace?: Record<string, number> };
  db_pool?: { active?: number; idle?: number; waiting?: number };
  physics?: { iterations_per_sec?: number; avg_iteration_ms?: number; convergence_detected?: boolean };
  ontology?: { loaded_count?: number; total_axioms?: number; total_classes?: number };
}

let _statusCache: OperatorStatus | null = null;
let _statusFetchedAt = 0;
const TTL_MS = 5_000;

async function fetchStatus(): Promise<OperatorStatus> {
  const now = Date.now();
  if (_statusCache && now - _statusFetchedAt < TTL_MS) return _statusCache;
  try {
    const res = await unifiedApiClient.get<OperatorStatus>(
      '/api/admin/operator/status'
    );
    _statusCache = res?.data ?? {};
    _statusFetchedAt = now;
    return _statusCache;
  } catch {
    return _statusCache ?? {};
  }
}

function useOperatorStatus() {
  const [status, setStatus] = useState<OperatorStatus | null>(_statusCache);
  useEffect(() => {
    let alive = true;
    void fetchStatus().then((s) => {
      if (alive) setStatus(s);
    });
    const t = setInterval(async () => {
      const s = await fetchStatus();
      if (alive) setStatus(s);
    }, TTL_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);
  return status;
}

// ─── build / version ─────────────────────────────────────────────────────

export const operatorBuild: Setting<unknown> = {
  id: 'operator.build',
  path: ['admin', 'operator', 'build'] as const,
  tier: 4,
  category: 'operator',
  label: 'Build',
  decision: 'EXPOSE',
  ref: 'PRD-007 §15',
  readOnly: true,
  summary: () => {
    const s = _statusCache;
    if (!s?.build) return 'Build: loading…';
    const sha = s.build.commit_sha?.slice(0, 10) ?? '—';
    const v = s.build.version ?? '—';
    return `Build: v${v} · ${sha}`;
  },
  Editor: ((props: EditorProps<unknown>) => {
    const s = useOperatorStatus();
    return <ReadOnlyEditor {...props} value={s?.build ?? {}} />;
  }) as Setting<unknown>['Editor'],
};

// ─── GPU stats ───────────────────────────────────────────────────────────

export const operatorGpu: Setting<unknown> = {
  id: 'operator.gpu',
  path: ['admin', 'operator', 'gpu'] as const,
  tier: 4,
  category: 'operator',
  label: 'GPU',
  decision: 'EXPOSE',
  readOnly: true,
  summary: () => {
    const s = _statusCache;
    if (!s?.gpu) return 'GPU: loading…';
    const cap = s.gpu.compute_capability ?? '—';
    const used = s.gpu.vram_used_mb ?? 0;
    const total = s.gpu.vram_total_mb ?? 0;
    const util = s.gpu.utilisation_percent ?? 0;
    return `GPU sm_${cap} · ${used}/${total} MB VRAM · ${util}% util`;
  },
  Editor: ((props: EditorProps<unknown>) => {
    const s = useOperatorStatus();
    return <ReadOnlyEditor {...props} value={s?.gpu ?? {}} />;
  }) as Setting<unknown>['Editor'],
};

// ─── live WS subscribers ────────────────────────────────────────────────

export const operatorWs: Setting<unknown> = {
  id: 'operator.ws_subscribers',
  path: ['admin', 'operator', 'ws_subscribers'] as const,
  tier: 4,
  category: 'operator',
  label: 'Live WebSocket subscribers',
  decision: 'EXPOSE',
  readOnly: true,
  summary: () => {
    const s = _statusCache;
    const t = s?.ws_subscribers?.total;
    return typeof t === 'number'
      ? `Live WS subscribers: ${t}`
      : 'WS subscribers: loading…';
  },
  Editor: ((props: EditorProps<unknown>) => {
    const s = useOperatorStatus();
    return <ReadOnlyEditor {...props} value={s?.ws_subscribers ?? {}} />;
  }) as Setting<unknown>['Editor'],
};

// ─── physics iteration health ───────────────────────────────────────────

export const operatorPhysics: Setting<unknown> = {
  id: 'operator.physics',
  path: ['admin', 'operator', 'physics'] as const,
  tier: 4,
  category: 'operator',
  label: 'Physics simulation health',
  decision: 'EXPOSE',
  readOnly: true,
  summary: () => {
    const s = _statusCache;
    const ips = s?.physics?.iterations_per_sec;
    const ms = s?.physics?.avg_iteration_ms;
    const c = s?.physics?.convergence_detected;
    if (typeof ips !== 'number') return 'Physics: loading…';
    return `Physics: ${ips} iter/s · ${ms?.toFixed(1)} ms avg · converged: ${c ? 'yes' : 'no'}`;
  },
  Editor: ((props: EditorProps<unknown>) => {
    const s = useOperatorStatus();
    return <ReadOnlyEditor {...props} value={s?.physics ?? {}} />;
  }) as Setting<unknown>['Editor'],
};

// ─── ontology stats ──────────────────────────────────────────────────────

export const operatorOntology: Setting<unknown> = {
  id: 'operator.ontology',
  path: ['admin', 'operator', 'ontology'] as const,
  tier: 4,
  category: 'operator',
  label: 'Ontology load',
  decision: 'EXPOSE',
  readOnly: true,
  summary: () => {
    const s = _statusCache;
    const c = s?.ontology?.loaded_count;
    const a = s?.ontology?.total_axioms;
    return typeof c === 'number'
      ? `Ontology: ${c} classes, ${a ?? 0} axioms`
      : 'Ontology: loading…';
  },
  Editor: ((props: EditorProps<unknown>) => {
    const s = useOperatorStatus();
    return <ReadOnlyEditor {...props} value={s?.ontology ?? {}} />;
  }) as Setting<unknown>['Editor'],
};

// ─── db pool ────────────────────────────────────────────────────────────

export const operatorDbPool: Setting<unknown> = {
  id: 'operator.db_pool',
  path: ['admin', 'operator', 'db_pool'] as const,
  tier: 4,
  category: 'operator',
  label: 'DB connection pool',
  decision: 'EXPOSE',
  readOnly: true,
  summary: () => {
    const s = _statusCache;
    const a = s?.db_pool?.active;
    const i = s?.db_pool?.idle;
    if (typeof a !== 'number') return 'DB pool: loading…';
    return `DB pool: ${a} active · ${i} idle`;
  },
  Editor: ((props: EditorProps<unknown>) => {
    const s = useOperatorStatus();
    return <ReadOnlyEditor {...props} value={s?.db_pool ?? {}} />;
  }) as Setting<unknown>['Editor'],
};

export const OPERATOR_DESCRIPTORS: ReadonlyArray<Setting<any>> = [
  operatorBuild,
  operatorGpu,
  operatorWs,
  operatorPhysics,
  operatorOntology,
  operatorDbPool,
];
