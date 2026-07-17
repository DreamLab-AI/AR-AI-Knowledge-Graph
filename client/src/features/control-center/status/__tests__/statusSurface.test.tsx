/**
 * StatusSurface jsdom coverage — the unified status surface's own
 * composition/interaction contract.
 *
 * StatusFlyout is mocked out: it stands up the heavy telemetry hooks
 * (constraint-stats polling, inferred-edges refresh, the motion sampler) which
 * are the flyout's own concern and are exercised by statusFlyout.test.tsx. This
 * suite proves the rest-state cheapness (no flyout mounted while collapsed → no
 * polling), the expand/collapse contract, the agent-count readout from the
 * store, and the SpacePilot connected dot.
 */

import React from 'react';
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { render, screen, fireEvent, cleanup, act } from '@testing-library/react';
import { SpaceDriver } from '../../../../services/SpaceDriverService';
import { StatusSurface } from '../StatusSurface';

// Mutable agent-count fixture, read by the mocked bots context per test.
const bots = { current: null as { botsData: { nodeCount: number } | null } | null };
vi.mock('../../../bots/contexts/BotsDataContext', () => ({
  useBotsDataOptional: () => bots.current,
}));

// The flyout body is the heavy half — stub it so the surface test stays cheap.
vi.mock('../StatusFlyout', () => ({
  StatusFlyout: () => <div data-testid="status-flyout" />,
}));

beforeEach(() => {
  bots.current = { botsData: { nodeCount: 0 } };
});
afterEach(() => cleanup());

describe('StatusSurface', () => {
  it('renders the collapsed chip at rest with no flyout mounted (cheap rest state)', () => {
    render(<StatusSurface />);
    const chip = screen.getByTestId('status-surface');
    expect(chip).toBeInTheDocument();
    expect(chip).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByTestId('status-flyout')).not.toBeInTheDocument();
  });

  it('expands on click, flipping aria-expanded and mounting the flyout', () => {
    render(<StatusSurface />);
    const chip = screen.getByTestId('status-surface');

    fireEvent.click(chip);

    expect(chip).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByTestId('status-flyout')).toBeInTheDocument();
  });

  it('collapses again on a second click', () => {
    render(<StatusSurface />);
    const chip = screen.getByTestId('status-surface');

    fireEvent.click(chip);
    fireEvent.click(chip);

    expect(chip).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByTestId('status-flyout')).not.toBeInTheDocument();
  });

  it('is keyboard-operable: the chip is a real button with aria-expanded state', () => {
    render(<StatusSurface />);
    const chip = screen.getByTestId('status-surface');
    // A native <button> is focusable and Enter/Space activate it (→ onClick),
    // so keyboard users toggle it without a bespoke focus handler that would
    // fight the click.
    expect(chip.tagName).toBe('BUTTON');
    expect(chip).toHaveAttribute('aria-expanded', 'false');
    act(() => chip.focus());
    expect(chip).toHaveFocus();
  });

  it('shows the agent count from the bots store', () => {
    bots.current = { botsData: { nodeCount: 7 } };
    render(<StatusSurface />);
    expect(screen.getByTestId('status-surface')).toHaveTextContent('7');
  });

  it('defaults the agent count to 0 with no bots data', () => {
    bots.current = { botsData: null };
    render(<StatusSurface />);
    expect(screen.getByTestId('status-surface')).toHaveTextContent('0');
  });

  it('shows the SpacePilot dot only once a device connects', () => {
    render(<StatusSurface />);
    expect(screen.queryByLabelText('SpacePilot connected')).not.toBeInTheDocument();

    act(() => {
      SpaceDriver.dispatchEvent(new CustomEvent('connect'));
    });
    expect(screen.getByLabelText('SpacePilot connected')).toBeInTheDocument();

    act(() => {
      SpaceDriver.dispatchEvent(new Event('disconnect'));
    });
    expect(screen.queryByLabelText('SpacePilot connected')).not.toBeInTheDocument();
  });
});
