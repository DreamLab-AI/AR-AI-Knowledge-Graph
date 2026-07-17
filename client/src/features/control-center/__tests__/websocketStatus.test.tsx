/**
 * D5 status-honesty test (jsdom, CANARY-VC-D5-WS).
 *
 * The WS status the Status surface shows must come from the real
 * webSocketService lifecycle, never a hardcoded literal. That logic now lives
 * in the useWebSocketStatus hook (ported out of ControlCenter into the unified
 * status surface); this test drives a faked service and asserts the hook's
 * output follows ground truth — connecting at mount, then tracking real
 * connect/disconnect transitions.
 */

import { describe, it, expect, afterEach, vi } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';

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

import { useWebSocketStatus } from '../status/useConnectionTelemetry';

afterEach(() => {
  cleanup();
  resetHandler();
  vi.clearAllMocks();
});

describe('useWebSocketStatus (D5)', () => {
  it('follows the live socket lifecycle, never a literal', () => {
    const { result } = renderHook(() => useWebSocketStatus());

    // isReady() === false at mount → 'connecting', never a hardcoded 'connected'.
    expect(result.current).toBe('connecting');
    const handler = getStatusHandler();
    expect(typeof handler).toBe('function');

    act(() => handler!(true));
    expect(result.current).toBe('connected');

    // CANARY-VC-D5-WS: a real socket drop flips the readout to disconnected.
    act(() => handler!(false));
    expect(result.current).toBe('disconnected');
  });
});
