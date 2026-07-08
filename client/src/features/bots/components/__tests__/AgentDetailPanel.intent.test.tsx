// D7 (PRD-023 / ADR-130 register position — pre-action intent legibility): the
// mounted per-agent panel renders "about to: <declared action>" when the
// selected agent carries a declared intent, and shows no such line otherwise.
// Guards the D7 affordance the task scopes (envelope `intent` field + panel
// display).

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';

const makeAgent = (extra: Record<string, unknown>) => ({
  id: 'task-1',
  name: 'Alpha',
  type: 'coder',
  status: 'active',
  health: 90,
  cpuUsage: 10,
  memoryUsage: 20,
  workload: 5,
  age: 1000,
  swarmId: 'swarm-a',
  ...extra,
});

const mockAgentState: { agent: Record<string, unknown> } = { agent: makeAgent({}) };

vi.mock('../../contexts/BotsDataContext', () => ({
  useBotsData: () => ({ botsData: { agents: [mockAgentState.agent] } }),
}));

vi.mock('../../../../services/api/UnifiedApiClient', () => ({
  unifiedApiClient: { post: vi.fn(), get: vi.fn(), getData: vi.fn() },
}));

import { AgentDetailPanel } from '../AgentDetailPanel';

afterEach(() => cleanup());

describe('AgentDetailPanel declared intent (D7)', () => {
  it('renders "About to: <declared action>" when the agent declares an intent', () => {
    mockAgentState.agent = makeAgent({
      declaredIntent: 'rewrite the budget node with Q3 figures',
    });

    render(<AgentDetailPanel />);

    expect(screen.getByText('Declared Intent')).toBeTruthy();
    expect(
      screen.getByText('About to: rewrite the budget node with Q3 figures'),
    ).toBeTruthy();
  });

  it('shows no "About to" line when the agent declares no intent (never fabricated)', () => {
    mockAgentState.agent = makeAgent({}); // no declaredIntent

    render(<AgentDetailPanel />);

    expect(screen.queryByText('Declared Intent')).toBeNull();
    expect(screen.queryByText(/About to:/)).toBeNull();
  });
});
