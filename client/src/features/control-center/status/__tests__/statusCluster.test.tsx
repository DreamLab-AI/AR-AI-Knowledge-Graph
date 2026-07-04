/**
 * StatusCluster jsdom coverage. design-spec.md §6.1, §9.1.
 *
 * The three composed widgets (SystemHealthIndicator, BotsStatusPanel,
 * SpacePilotStatus) are mocked out: SystemHealthIndicator polls a websocket
 * store + constraint stats hook, and BotsStatusPanel needs a BotsDataContext
 * provider — neither is StatusCluster's concern to stand up here. This test
 * covers StatusCluster's own composition/interaction contract only; the
 * widgets themselves are exercised by their own suites.
 */

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, act } from '@testing-library/react';
import { StatusCluster } from '../StatusCluster';

vi.mock('../../../visualisation/components/ControlPanel/SystemHealthIndicator', () => ({
  SystemHealthIndicator: () => <div data-testid="mock-system-health" />,
}));
vi.mock('../../../visualisation/components/ControlPanel/BotsStatusPanel', () => ({
  BotsStatusPanel: () => <div data-testid="mock-bots-status" />,
}));
vi.mock('../../../visualisation/components/ControlPanel/SpacePilotStatus', () => ({
  SpacePilotStatus: () => <div data-testid="mock-space-pilot" />,
}));

afterEach(() => cleanup());

describe('StatusCluster', () => {
  it('renders the collapsed pill at rest with aria-expanded=false and no flyout', () => {
    render(<StatusCluster />);
    const pill = screen.getByTestId('status-cluster');
    expect(pill).toBeInTheDocument();
    expect(pill).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByTestId('status-cluster-expanded')).not.toBeInTheDocument();
  });

  it('expands on click, flipping aria-expanded and rendering all three widgets', () => {
    render(<StatusCluster />);
    const pill = screen.getByTestId('status-cluster');

    fireEvent.click(pill);

    expect(pill).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByTestId('status-cluster-expanded')).toBeInTheDocument();
    expect(screen.getByTestId('mock-system-health')).toBeInTheDocument();
    expect(screen.getByTestId('mock-bots-status')).toBeInTheDocument();
    expect(screen.getByTestId('mock-space-pilot')).toBeInTheDocument();
  });

  it('collapses again on a second click', () => {
    render(<StatusCluster />);
    const pill = screen.getByTestId('status-cluster');

    fireEvent.click(pill);
    fireEvent.click(pill);

    expect(pill).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByTestId('status-cluster-expanded')).not.toBeInTheDocument();
  });

  it('is keyboard accessible: focusing the pill expands the flyout', () => {
    render(<StatusCluster />);
    const pill = screen.getByTestId('status-cluster');

    act(() => {
      pill.focus();
    });

    expect(pill).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByTestId('status-cluster-expanded')).toBeInTheDocument();
  });

  it('shows the SpacePilot dot only when connected', () => {
    const { rerender } = render(<StatusCluster spacePilotConnected={false} />);
    expect(screen.queryByLabelText('SpacePilot connected')).not.toBeInTheDocument();

    rerender(<StatusCluster spacePilotConnected={true} />);
    expect(screen.getByLabelText('SpacePilot connected')).toBeInTheDocument();
  });

  it('shows the agent count badge from botsData.nodeCount', () => {
    render(
      <StatusCluster
        botsData={{ nodeCount: 7, edgeCount: 3, tokenCount: 100, mcpConnected: true, dataSource: 'live' }}
      />,
    );
    expect(screen.getByTestId('status-cluster')).toHaveTextContent('7');
  });

  it('defaults the agent count badge to 0 with no botsData', () => {
    render(<StatusCluster />);
    expect(screen.getByTestId('status-cluster')).toHaveTextContent('0');
  });
});
