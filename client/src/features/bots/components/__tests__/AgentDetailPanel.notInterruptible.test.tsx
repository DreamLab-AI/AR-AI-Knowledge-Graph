// D2 (PRD-023 WP-3) final close — client honesty. When the server cannot resolve
// the selected agent's id to a Management-API task (an externally-spawned /
// MCP-native claude-flow agent, which has no terminate verb on the MCP surface),
// it returns a DISTINCT 422 (`resolution: "unresolved"`, `interruptible: false`).
// The panel must disclose a disabled explanatory state on the FIRST failure —
// "Externally spawned — not interruptible from here" — rather than leaving a dead
// retrying button. This test drives that state transition.

import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

const { postSpy } = vi.hoisted(() => ({ postSpy: vi.fn() }));

vi.mock('../../contexts/BotsDataContext', () => ({
  useBotsData: () => ({
    botsData: {
      agents: [
        {
          id: 'agent-swarm-7f3a', // a claude-flow swarm agent_id with no backing task
          name: 'Externally Spawned',
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

describe('AgentDetailPanel interrupt — not-interruptible disclosure (D2)', () => {
  it('discloses a disabled explanatory state on the distinct 422 resolution error', async () => {
    // The client throws an ApiError carrying the server's distinct 422 signal.
    postSpy.mockRejectedValueOnce({
      status: 422,
      data: {
        success: false,
        interruptible: false,
        resolution: 'unresolved',
        message: 'externally spawned — not interruptible from here',
      },
    });

    render(<AgentDetailPanel />);

    const button = await screen.findByText('Interrupt / Stop Agent');
    fireEvent.click(button);

    // The button transitions to the disclosed terminal label…
    const disclosed = await screen.findByText('Not interruptible');
    // …and is disabled (a <button> renders the label; assert the button itself).
    expect(disclosed.closest('button')).toBeDisabled();

    // …with the explanatory message surfaced.
    await screen.findByText('Externally spawned — not interruptible from here');

    // Exactly one call — no retry storm behind a dead button.
    expect(postSpy).toHaveBeenCalledTimes(1);

    // A second click on the now-disabled control must NOT re-issue the request.
    fireEvent.click(disclosed);
    await waitFor(() => expect(postSpy).toHaveBeenCalledTimes(1));
  });
});
