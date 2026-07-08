// D2 (PRD-023 WP-3): the mounted per-agent panel invokes the live /bots/interrupt
// route. Guards the WP-3 falsification clause ("/bots/interrupt is never invoked
// from a mounted panel").

import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

const { postSpy } = vi.hoisted(() => ({ postSpy: vi.fn() }));

vi.mock('../../contexts/BotsDataContext', () => ({
  useBotsData: () => ({
    botsData: {
      agents: [
        {
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
        },
      ],
    },
  }),
}));

vi.mock('../../../../services/api/UnifiedApiClient', () => ({
  unifiedApiClient: {
    post: (...args: unknown[]) => postSpy(...args),
    get: vi.fn(),
    getData: vi.fn(),
  },
}));

import { AgentDetailPanel } from '../AgentDetailPanel';

beforeEach(() => {
  postSpy.mockReset();
  cleanup();
});

describe('AgentDetailPanel interrupt (D2)', () => {
  it('invokes POST /bots/interrupt for the selected agent', async () => {
    postSpy.mockResolvedValueOnce({ data: { success: true, message: 'Agent task task-1 interrupted' } });

    render(<AgentDetailPanel />);

    // The panel auto-selects the first agent, so the interrupt control mounts.
    const button = await screen.findByText('Interrupt / Stop Agent');
    fireEvent.click(button);

    await waitFor(() => expect(postSpy).toHaveBeenCalled());
    const [url, body] = postSpy.mock.calls[0];
    expect(url).toBe('/bots/interrupt');
    expect(body).toMatchObject({ taskId: 'task-1', agentId: 'task-1' });

    await screen.findByText('Agent task task-1 interrupted');
  });
});
