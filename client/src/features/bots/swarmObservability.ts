// D8 swarm observability (PRD-023 WP-3): pure aggregation of the already-polled
// bots data into the swarm-level summary the AgentOps panel renders.
//
// Renderer-free and DOM-free so the aggregate maths (task success rate, cost,
// topology, health/workload) and the MAST failure-tag extraction are
// unit-testable without React. The panel that renders these is
// `SwarmObservabilityPanel`.

import type { BotsAgent } from './types/BotsTypes';

/** The subset of `BotsData` this module needs (kept structural to avoid a
 *  circular import from BotsDataContext). */
export interface SwarmObservabilityInput {
  agents?: BotsAgent[];
  multiAgentMetrics?: {
    totalAgents?: number;
    activeAgents?: number;
    totalTasks?: number;
    completedTasks?: number;
    avgSuccessRate?: number;
    totalTokens?: number;
  } | null;
  tokenCount?: number;
  // MAST failure tags may ride the metrics blob once agentbox emits them
  // (this wave). Structural + optional: rendered only when present.
  mastFailureTags?: Record<string, number> | null;
}

export interface SwarmSummary {
  agentCount: number;
  activeAgents: number;
  totalTasks: number;
  completedTasks: number;
  /** Task success rate, 0–100. */
  successRatePct: number;
  /** Total tokens — the cost proxy the dashboard surfaces. */
  totalTokens: number;
  /** Mean agent health, 0–100. */
  avgHealthPct: number;
  /** Mean agent workload, 0–100. */
  avgWorkloadPct: number;
  /** Agent-type → count (the swarm topology). */
  topology: Record<string, number>;
  /** True once the poll has carried at least one live agent. */
  hasLiveData: boolean;
}

function round1(n: number): number {
  return Math.round(n * 10) / 10;
}

/** Compute the swarm-level summary from the polled bots data. */
export function computeSwarmSummary(input: SwarmObservabilityInput | null | undefined): SwarmSummary {
  const agents = input?.agents ?? [];
  const metrics = input?.multiAgentMetrics ?? undefined;

  const agentCount = agents.length;
  const activeAgents =
    metrics?.activeAgents ??
    agents.filter((a) => a.status === 'active' || a.status === 'busy').length;

  const totalTasks = metrics?.totalTasks ?? agents.reduce((s, a) => s + (a.tasksActive ?? 0) + (a.tasksCompleted ?? 0), 0);
  const completedTasks = metrics?.completedTasks ?? agents.reduce((s, a) => s + (a.tasksCompleted ?? 0), 0);

  // Prefer the server-supplied success rate; else derive from completed/total.
  let successRatePct: number;
  if (typeof metrics?.avgSuccessRate === 'number') {
    successRatePct = metrics.avgSuccessRate;
  } else if (totalTasks > 0) {
    successRatePct = (completedTasks / totalTasks) * 100;
  } else {
    successRatePct = 0;
  }

  const totalTokens = metrics?.totalTokens ?? input?.tokenCount ?? agents.reduce((s, a) => s + (a.tokens ?? 0), 0);

  const avgHealthPct = agentCount > 0 ? agents.reduce((s, a) => s + (a.health ?? 0), 0) / agentCount : 0;
  const avgWorkloadPct = agentCount > 0 ? agents.reduce((s, a) => s + (a.workload ?? 0), 0) / agentCount : 0;

  const topology: Record<string, number> = {};
  for (const a of agents) {
    const t = a.type ?? 'unknown';
    topology[t] = (topology[t] ?? 0) + 1;
  }

  return {
    agentCount,
    activeAgents,
    totalTasks,
    completedTasks,
    successRatePct: round1(successRatePct),
    totalTokens,
    avgHealthPct: round1(avgHealthPct),
    avgWorkloadPct: round1(avgWorkloadPct),
    topology,
    hasLiveData: agentCount > 0,
  };
}

/**
 * Extract MAST (Multi-Agent System failure Taxonomy) failure-tag counts, sorted
 * most-frequent first, or `null` when no such fields are present. agentbox emits
 * these this wave; the panel renders the row only when this returns a non-empty
 * result, and hides it otherwise (PRD-023 WP-3: "render if fields present, hide
 * otherwise").
 */
export function extractMastFailureTags(
  input: SwarmObservabilityInput | null | undefined,
): Array<{ tag: string; count: number }> | null {
  const acc: Record<string, number> = {};

  const merge = (bag: unknown) => {
    if (!bag || typeof bag !== 'object') return;
    for (const [tag, count] of Object.entries(bag as Record<string, unknown>)) {
      const n = typeof count === 'number' ? count : Number(count);
      if (Number.isFinite(n) && n > 0) acc[tag] = (acc[tag] ?? 0) + n;
    }
  };

  // Top-level blob, and a per-metrics blob if agentbox nests it there.
  merge(input?.mastFailureTags);
  merge((input?.multiAgentMetrics as Record<string, unknown> | undefined)?.mastFailureTags);

  // Per-agent MAST tags, if any agent carries them.
  for (const a of input?.agents ?? []) {
    merge((a as unknown as Record<string, unknown>).mastFailureTags);
  }

  const entries = Object.entries(acc);
  if (entries.length === 0) return null;
  return entries
    .map(([tag, count]) => ({ tag, count }))
    .sort((x, y) => y.count - x.count);
}
