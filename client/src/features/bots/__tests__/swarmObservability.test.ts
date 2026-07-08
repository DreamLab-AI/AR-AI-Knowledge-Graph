// D8 (PRD-023 WP-3): swarm-summary aggregation + MAST tag extraction.

import { describe, it, expect } from 'vitest';
import { computeSwarmSummary, extractMastFailureTags } from '../swarmObservability';
import type { SwarmObservabilityInput } from '../swarmObservability';

const input: SwarmObservabilityInput = {
  agents: [
    { id: 'a1', type: 'coder', status: 'active', health: 90, workload: 40, tokens: 100 },
    { id: 'a2', type: 'coder', status: 'idle', health: 70, workload: 0, tokens: 50 },
    { id: 'a3', type: 'researcher', status: 'busy', health: 80, workload: 60, tokens: 30 },
  ] as any,
  multiAgentMetrics: {
    totalAgents: 3,
    activeAgents: 2,
    totalTasks: 10,
    completedTasks: 7,
    avgSuccessRate: 70,
    totalTokens: 180,
  },
};

describe('computeSwarmSummary', () => {
  it('aggregates counts, success rate, cost and topology', () => {
    const s = computeSwarmSummary(input);
    expect(s.agentCount).toBe(3);
    expect(s.activeAgents).toBe(2);
    expect(s.successRatePct).toBe(70);
    expect(s.totalTokens).toBe(180);
    expect(s.completedTasks).toBe(7);
    expect(s.topology).toEqual({ coder: 2, researcher: 1 });
    expect(s.avgHealthPct).toBe(80);
    expect(s.hasLiveData).toBe(true);
  });

  it('derives success rate from completed/total when no avg is supplied', () => {
    const s = computeSwarmSummary({
      agents: [{ id: 'a1', type: 'coder', status: 'active', health: 100, workload: 0, tasksActive: 1, tasksCompleted: 3 }] as any,
    });
    // 3 completed of (1 active + 3 completed) = 75%.
    expect(s.successRatePct).toBe(75);
  });

  it('reports no live data for an empty swarm', () => {
    const s = computeSwarmSummary({ agents: [] });
    expect(s.hasLiveData).toBe(false);
    expect(s.agentCount).toBe(0);
  });
});

describe('extractMastFailureTags', () => {
  it('returns null when no MAST fields are present', () => {
    expect(extractMastFailureTags(input)).toBeNull();
  });

  it('extracts and sorts top-level MAST tags most-frequent first', () => {
    const tags = extractMastFailureTags({ ...input, mastFailureTags: { 'step-repetition': 2, 'no-verification': 5 } });
    expect(tags).toEqual([
      { tag: 'no-verification', count: 5 },
      { tag: 'step-repetition', count: 2 },
    ]);
  });

  it('merges per-agent MAST tags', () => {
    const tags = extractMastFailureTags({
      agents: [
        { id: 'a1', type: 'coder', status: 'active', health: 100, workload: 0, mastFailureTags: { 'derailment': 1 } },
        { id: 'a2', type: 'coder', status: 'active', health: 100, workload: 0, mastFailureTags: { 'derailment': 2 } },
      ] as any,
    });
    expect(tags).toEqual([{ tag: 'derailment', count: 3 }]);
  });
});
