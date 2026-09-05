// @ts-ignore - vitest types may not be available in all environments
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

/**
 * ADR-2100 — one JSS notification client.
 *
 * The Zustand store used to open its own WebSocket to VITE_JSS_WS_URL
 * (registry name `solid-store`, 10 reconnect attempts) alongside
 * PodNotificationManager's (`solid-pod`, 5 attempts). These tests pin the
 * consolidation: the store adapter opens no socket of its own, delegates to the
 * shared manager, dispatches each callback exactly once, and shares the single
 * reconnect policy.
 */

const WS_URL = 'ws://jss.test/ws';

// --- Fake WebSocket, recording every construction -------------------------

class FakeWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;

  static instances: FakeWebSocket[] = [];

  readyState: number = FakeWebSocket.CONNECTING;
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: ((error: unknown) => void) | null = null;
  onclose: ((event: { code: number; reason: string }) => void) | null = null;

  constructor(public url: string) {
    FakeWebSocket.instances.push(this);
  }

  send(msg: string) {
    this.sent.push(msg);
  }

  close() {
    this.readyState = FakeWebSocket.CLOSED;
  }

  /** Drive the handshake the way a live JSS server would. */
  open() {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.();
  }

  receive(msg: string) {
    this.onmessage?.({ data: msg });
  }
}

const registryRegister = vi.fn();
const registryUnregister = vi.fn();

vi.mock('../../../utils/loggerConfig', () => ({
  createLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
  createErrorMetadata: vi.fn((e: unknown) => e),
}));

vi.mock('../../../services/WebSocketRegistry', () => ({
  webSocketRegistry: {
    register: registryRegister,
    unregister: registryUnregister,
  },
}));

vi.mock('../../../services/WebSocketEventBus', () => ({
  webSocketEventBus: { emit: vi.fn() },
}));

const storeEmit = vi.fn();
vi.mock('../connectionManager', () => ({
  emit: storeEmit,
  notifyBinaryMessageHandlers: vi.fn(),
}));

type PodModule = typeof import('../../../services/solidPod/podNotifications');
type AdapterModule = typeof import('../solidWebSocket');

let pod: PodModule;
let adapter: AdapterModule;

/** Minimal stand-in for the Zustand store slice the adapter writes into. */
function makeStore() {
  const state: Record<string, unknown> = {
    solidSocket: null,
    isSolidConnected: false,
    solidSubscriptions: new Map<string, Set<(n: unknown) => void>>(),
  };
  const set = (partial: Record<string, unknown> | ((s: never) => Record<string, unknown>)) => {
    const next = typeof partial === 'function' ? (partial as (s: unknown) => Record<string, unknown>)(state) : partial;
    Object.assign(state, next);
  };
  return { state, set };
}

describe('ADR-2100: one Solid/JSS WebSocket client', () => {
  beforeEach(async () => {
    FakeWebSocket.instances = [];
    vi.clearAllMocks();
    vi.resetModules();
    vi.stubGlobal('WebSocket', FakeWebSocket);
    vi.stubEnv('VITE_JSS_WS_URL', WS_URL);

    pod = await import('../../../services/solidPod/podNotifications');
    adapter = await import('../solidWebSocket');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it('opens exactly ONE socket when the store adapter connects', () => {
    const { set } = makeStore();

    adapter.connectSolidWebSocket(set as never);

    expect(FakeWebSocket.instances).toHaveLength(1);
    expect(FakeWebSocket.instances[0].url).toBe(WS_URL);
  });

  it('registers only under the shared name `solid-pod` — never `solid-store`', () => {
    const { set } = makeStore();

    adapter.connectSolidWebSocket(set as never);
    FakeWebSocket.instances[0].open();

    expect(registryRegister).toHaveBeenCalledTimes(1);
    expect(registryRegister.mock.calls[0][0]).toBe('solid-pod');
    const names = registryRegister.mock.calls.map((c) => c[0]);
    expect(names).not.toContain('solid-store');
  });

  it('does not open a second socket when both consumers connect', () => {
    const { set } = makeStore();

    adapter.connectSolidWebSocket(set as never);
    FakeWebSocket.instances[0].open();

    // The other consumer (SolidPodService) connects through the same singleton.
    pod.podNotificationManager.connect();

    expect(FakeWebSocket.instances).toHaveLength(1);
  });

  it('mirrors the shared client lifecycle into store state', () => {
    const { state, set } = makeStore();

    adapter.connectSolidWebSocket(set as never);
    FakeWebSocket.instances[0].open();

    expect(state.isSolidConnected).toBe(true);
    expect(state.solidSocket).toBe(FakeWebSocket.instances[0]);
    expect(storeEmit).toHaveBeenCalledWith('solid-connected', { url: WS_URL });
  });

  it('delivers a `pub` to a store subscriber EXACTLY once (mirror is not a dispatch path)', () => {
    const { state, set } = makeStore();
    const callback = vi.fn();

    adapter.connectSolidWebSocket(set as never);
    FakeWebSocket.instances[0].open();
    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', callback);

    FakeWebSocket.instances[0].receive('pub https://pod.test/c/r');

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith({ type: 'pub', url: 'https://pod.test/c/r' });
    expect(storeEmit).toHaveBeenCalledWith('solid-resource-changed', { url: 'https://pod.test/c/r' });
    expect((state.solidSubscriptions as Map<string, unknown>).has('https://pod.test/c/r')).toBe(true);
  });

  it('sends sub/unsub on the one socket and clears the store mirror', () => {
    const { state, set } = makeStore();

    adapter.connectSolidWebSocket(set as never);
    const socket = FakeWebSocket.instances[0];
    socket.open();

    const unsubscribe = adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', vi.fn());
    expect(socket.sent).toContain('sub https://pod.test/c/r');

    unsubscribe();
    expect(socket.sent).toContain('unsub https://pod.test/c/r');
    expect((state.solidSubscriptions as Map<string, unknown>).has('https://pod.test/c/r')).toBe(false);
  });

  it('unsubscribeSolidResource drops every callback for the URL', () => {
    const { state, set } = makeStore();
    const a = vi.fn();
    const b = vi.fn();

    adapter.connectSolidWebSocket(set as never);
    const socket = FakeWebSocket.instances[0];
    socket.open();

    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', a);
    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', b);
    adapter.unsubscribeSolidResource(set as never, 'https://pod.test/c/r');

    socket.receive('pub https://pod.test/c/r');

    expect(a).not.toHaveBeenCalled();
    expect(b).not.toHaveBeenCalled();
    expect((state.solidSubscriptions as Map<string, unknown>).has('https://pod.test/c/r')).toBe(false);
  });

  it('exposes ONE reconnect policy, shared by both consumers', () => {
    expect(pod.SOLID_MAX_RECONNECT_ATTEMPTS).toBe(5);
    expect(pod.SOLID_RECONNECT_DELAY_MS).toBe(1000);
    // The store adapter carries no ladder of its own; it delegates the reset.
    const spy = vi.spyOn(pod.podNotificationManager, 'resetReconnect');
    adapter.resetSolidReconnect();
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('routes a server `error` frame to the store as solid-error', () => {
    const { set } = makeStore();

    adapter.connectSolidWebSocket(set as never);
    FakeWebSocket.instances[0].open();
    FakeWebSocket.instances[0].receive('error resource locked');

    expect(storeEmit).toHaveBeenCalledWith('solid-error', { message: 'resource locked' });
  });

  it('resubscribes every URL on the protocol handshake', () => {
    const { set } = makeStore();

    adapter.connectSolidWebSocket(set as never);
    const socket = FakeWebSocket.instances[0];
    socket.open();
    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', vi.fn());
    socket.sent.length = 0;

    socket.receive('protocol solid-0.1');

    expect(socket.sent).toContain('sub https://pod.test/c/r');
    expect(storeEmit).toHaveBeenCalledWith('solid-protocol', { protocol: 'solid-0.1' });
  });

  it('contains a throwing subscriber so its peers still receive the notification', () => {
    const { set } = makeStore();
    const bad = vi.fn(() => {
      throw new Error('consumer blew up');
    });
    const good = vi.fn();

    adapter.connectSolidWebSocket(set as never);
    const socket = FakeWebSocket.instances[0];
    socket.open();
    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', bad);
    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/r', good);

    expect(() => socket.receive('pub https://pod.test/c/r')).not.toThrow();
    expect(good).toHaveBeenCalledTimes(1);
  });

  it('notifies container subscribers once for a child resource change', () => {
    const { set } = makeStore();
    const containerCb = vi.fn();

    adapter.connectSolidWebSocket(set as never);
    const socket = FakeWebSocket.instances[0];
    socket.open();
    adapter.subscribeSolidResource(set as never, 'https://pod.test/c/', containerCb);

    socket.receive('pub https://pod.test/c/child');

    expect(containerCb).toHaveBeenCalledTimes(1);
    expect(containerCb).toHaveBeenCalledWith({ type: 'pub', url: 'https://pod.test/c/child' });
  });
});
