/**
 * D5 status-honesty test (jsdom, CANARY-VC-D5-WS).
 *
 * Guards the fix at ControlCenter.tsx: the WS status threaded into StatusCluster
 * must come from the real webSocketService lifecycle, never the old hardcoded
 * `websocketStatus="connected"` literal. StatusCluster is stubbed to surface the
 * prop it receives; webSocketService is faked so the test can drive the
 * connection-status handler and assert the dot follows ground truth.
 */

import React from 'react';
import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, screen, act, cleanup } from '@testing-library/react';

// Controllable fake socket. Built via vi.hoisted so the mock factory (hoisted to
// the top of the module) can reference it without a TDZ error.
const { fakeWs, getStatusHandler, resetHandler } = vi.hoisted(() => {
  let statusHandler: ((connected: boolean) => void) | null = null;
  return {
    fakeWs: {
      isReady: () => false,
      onConnectionStatusChange: (handler: (connected: boolean) => void) => {
        statusHandler = handler;
        return () => {
          statusHandler = null;
        };
      },
    },
    getStatusHandler: () => statusHandler,
    resetHandler: () => {
      statusHandler = null;
    },
  };
});

vi.mock('../../../store/websocketStore', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  return { ...actual, webSocketService: fakeWs };
});

// Stub StatusCluster to expose the websocketStatus prop it is handed.
vi.mock('../status/StatusCluster', () => ({
  StatusCluster: (props: { websocketStatus?: string }) => (
    <div data-testid="ws-probe" data-ws={props.websocketStatus} />
  ),
}));

// No network: mirror the shell test's settingsApi stub.
vi.mock('../../../api/settingsApi', () => ({
  settingsApi: {
    getSettingsByPaths: vi.fn(async () => ({})),
    updateSettingsByPaths: vi.fn(async () => ({})),
    getAllSettings: vi.fn(async () => ({})),
  },
}));

import { ControlCenter } from '../ControlCenter';

afterEach(() => {
  cleanup();
  resetHandler();
  vi.clearAllMocks();
});

describe('ControlCenter WS status (D5)', () => {
  it('threads the live socket lifecycle into StatusCluster, not a literal', () => {
    render(<ControlCenter showStats={false} enableBloom={false} />);

    // isReady() === false at mount → 'connecting', never a hardcoded 'connected'.
    expect(screen.getByTestId('ws-probe').getAttribute('data-ws')).toBe('connecting');
    const handler = getStatusHandler();
    expect(typeof handler).toBe('function');

    act(() => handler!(true));
    expect(screen.getByTestId('ws-probe').getAttribute('data-ws')).toBe('connected');

    // CANARY-VC-D5-WS: a real socket drop flips the dot to disconnected.
    act(() => handler!(false));
    expect(screen.getByTestId('ws-probe').getAttribute('data-ws')).toBe('disconnected');
  });
});
