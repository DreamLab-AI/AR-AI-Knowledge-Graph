// D8 swarm observability (PRD-023 WP-3): the AgentOps table-stakes panel.
//
// A swarm-level aggregate view — distinct from the per-message
// `AgentTelemetryStream` — that mounts with the live poll data already carried by
// `BotsDataContext`: agent list with status/health/workload, task success rate,
// cost (tokens) and topology, MAST failure-tag counts when agentbox emits them,
// plus the `kg_backend_up` gauge and canary status from the LivenessHarness.
//
// Fires `CANARY-VC-D8-OBS` (one-shot) the first time it mounts with live poll
// data — observed traffic, never a synthetic probe (DDD invariant 5).

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useBotsDataOptional } from '../contexts/BotsDataContext';
import { computeSwarmSummary, extractMastFailureTags } from '../swarmObservability';
import { unifiedApiClient } from '../../../services/api/UnifiedApiClient';
import { observeCanary } from '../../../services/livenessCanary';
import { createLogger } from '../../../utils/loggerConfig';

const logger = createLogger('SwarmObservabilityPanel');

interface CanaryRow {
  canary_id?: string;
  canaryId?: string;
  armed?: boolean;
  fired?: boolean;
  observation_count?: number;
  observationCount?: number;
}

interface CanaryStatus {
  kg_backend_up: boolean | null;
  canaries: CanaryRow[];
}

const statusDot: Record<string, string> = {
  active: '#10b981',
  busy: '#f59e0b',
  idle: '#6b7280',
  error: '#ef4444',
  initializing: '#3b82f6',
  terminating: '#a855f7',
  offline: '#374151',
};

export const SwarmObservabilityPanel: React.FC<{ className?: string }> = ({ className }) => {
  const botsData = useBotsDataOptional()?.botsData ?? null;
  const summary = useMemo(() => computeSwarmSummary(botsData ?? undefined), [botsData]);
  const mastTags = useMemo(() => extractMastFailureTags(botsData ?? undefined), [botsData]);

  const [canary, setCanary] = useState<CanaryStatus | null>(null);
  const firedRef = useRef(false);

  // D8-OBS canary: fire once when the dashboard is live with real poll data.
  useEffect(() => {
    if (firedRef.current || !summary.hasLiveData) return;
    firedRef.current = true;
    observeCanary(
      'CANARY-VC-D8-OBS',
      `swarm dashboard mounted agents=${summary.agentCount} active=${summary.activeAgents} tasks=${summary.completedTasks}/${summary.totalTasks}`,
    );
  }, [summary.hasLiveData, summary.agentCount, summary.activeAgents, summary.completedTasks, summary.totalTasks]);

  // KG liveness + canary status from the harness (RES-a).
  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const resp = await unifiedApiClient.getData<Record<string, unknown>>('/canary/status');
        const payload = (resp?.data ?? resp) as unknown as CanaryStatus;
        if (!cancelled && payload && Array.isArray(payload.canaries)) setCanary(payload);
      } catch (error) {
        logger.debug('canary status unavailable:', error);
      }
    };
    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const agents = botsData?.agents ?? [];
  const kgUp = canary?.kg_backend_up;

  return (
    <div className={className} data-testid="swarm-observability">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold">Swarm Observability</h3>
        <span
          className="text-[10px] px-2 py-0.5 rounded-full"
          style={{
            background: kgUp === true ? 'rgba(16,185,129,0.15)' : kgUp === false ? 'rgba(239,68,68,0.15)' : 'rgba(107,114,128,0.15)',
            color: kgUp === true ? '#10b981' : kgUp === false ? '#ef4444' : '#9ca3af',
          }}
          title="KG backend liveness (LivenessHarness watchdog)"
        >
          KG {kgUp === true ? 'up' : kgUp === false ? 'down' : '—'}
        </span>
      </div>

      {/* Aggregate tiles */}
      <div className="grid grid-cols-2 gap-2 mb-3">
        <Tile label="Agents" value={`${summary.activeAgents}/${summary.agentCount}`} sub="active/total" />
        <Tile label="Success" value={`${summary.successRatePct.toFixed(1)}%`} sub="task success rate" />
        <Tile label="Tokens" value={summary.totalTokens.toLocaleString()} sub="cost proxy" />
        <Tile label="Tasks" value={`${summary.completedTasks}/${summary.totalTasks}`} sub="done/total" />
      </div>

      {/* Topology */}
      {Object.keys(summary.topology).length > 0 && (
        <div className="mb-3">
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">Topology</div>
          <div className="flex flex-wrap gap-1">
            {Object.entries(summary.topology).map(([type, count]) => (
              <span key={type} className="text-[10px] px-2 py-0.5 rounded bg-white/5">
                {type} × {count}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* MAST failure tags — rendered only when agentbox emits them. */}
      {mastTags && mastTags.length > 0 && (
        <div className="mb-3" data-testid="mast-failures">
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">MAST failures</div>
          <div className="flex flex-wrap gap-1">
            {mastTags.map(({ tag, count }) => (
              <span key={tag} className="text-[10px] px-2 py-0.5 rounded bg-red-500/15 text-red-400">
                {tag}: {count}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Agent list */}
      <div className="mb-2">
        <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">
          Agents ({agents.length})
        </div>
        {agents.length === 0 ? (
          <p className="text-xs text-muted-foreground">No live agents polled yet.</p>
        ) : (
          <ul className="space-y-1 max-h-40 overflow-y-auto">
            {agents.map((a) => (
              <li key={a.id} className="flex items-center gap-2 text-[11px]">
                <span
                  className="inline-block h-2 w-2 rounded-full flex-shrink-0"
                  style={{ background: statusDot[a.status] ?? '#6b7280' }}
                />
                <span className="flex-1 truncate">{a.name || a.id}</span>
                <span className="text-muted-foreground">{a.type}</span>
                <span title="health">{(a.health ?? 0).toFixed(0)}%</span>
                <span className="text-muted-foreground" title="workload">
                  w{(a.workload ?? 0).toFixed(0)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Canary status */}
      {canary && canary.canaries.length > 0 && (
        <div className="pt-2 border-t border-white/10">
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground mb-1">Canaries</div>
          <ul className="space-y-0.5">
            {canary.canaries.map((c) => {
              const id = c.canary_id ?? c.canaryId ?? '';
              const fired = !!c.fired;
              return (
                <li key={id} className="flex items-center justify-between text-[10px]">
                  <span className="truncate mr-2 font-mono">{id}</span>
                  <span style={{ color: fired ? '#10b981' : '#9ca3af' }}>
                    {fired ? 'fired' : 'armed'}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
};

const Tile: React.FC<{ label: string; value: string; sub: string }> = ({ label, value, sub }) => (
  <div className="rounded bg-white/5 p-2">
    <div className="text-[10px] text-muted-foreground">{label}</div>
    <div className="text-sm font-semibold">{value}</div>
    <div className="text-[9px] text-muted-foreground">{sub}</div>
  </div>
);

export default SwarmObservabilityPanel;
