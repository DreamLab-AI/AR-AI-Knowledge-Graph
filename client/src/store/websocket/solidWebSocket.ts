/**
 * solidWebSocket.ts — Solid (JSS) store adapter over the shared pod client
 *
 * ADR-2100. This module used to open its OWN WebSocket to VITE_JSS_WS_URL,
 * registered as `solid-store`, with a 10-attempt reconnect ladder — a second,
 * independent client to the endpoint `services/solidPod/podNotifications.ts`
 * already owned as `solid-pod` with a 5-attempt ladder. Two sockets meant the
 * pod saw two subscriber sets for the same resources, two divergent backoff
 * policies, and two registry entries for one logical connection.
 *
 * There is now ONE socket: `podNotificationManager`. This file is a thin
 * adapter that keeps the Zustand store's public surface (`connectSolid`,
 * `disconnectSolid`, `subscribeSolidResource`, `unsubscribeSolidResource`,
 * `isSolidWebSocketConnected`, `getSolidSubscriptions`) and its `solid-*` event
 * emissions unchanged, mirroring the shared client's lifecycle into store state.
 */

import { createLogger } from '../../utils/loggerConfig';
import { podNotificationManager } from '../../services/solidPod/podNotifications';
import { emit } from './connectionManager';
import type { SolidNotificationCallback } from './types';

const logger = createLogger('WebSocketStore');

// ── Lifecycle mirroring ────────────────────────────────────────────────
//
// Bound once, on the first connect. Never torn down: it only mirrors state into
// the store, and the store outlives every individual connection.

let lifecycleBound = false;

function bindLifecycle(set: (partial: Record<string, unknown>) => void) {
  if (lifecycleBound) return;
  lifecycleBound = true;

  podNotificationManager.onLifecycle((event) => {
    switch (event.type) {
      case 'open':
        set({ isSolidConnected: true, solidSocket: podNotificationManager.socket });
        emit('solid-connected', { url: event.url });
        break;
      case 'close':
        set({ isSolidConnected: false, solidSocket: null });
        emit('solid-disconnected', { code: event.code, reason: event.reason });
        break;
      case 'error':
        emit('solid-error', { error: event.error });
        break;
      case 'server-error':
        emit('solid-error', { message: event.message });
        break;
      case 'protocol':
        emit('solid-protocol', { protocol: event.protocol });
        break;
      case 'pub':
        emit('solid-resource-changed', { url: event.url });
        break;
    }
  });
}

// ── Reconnect ──────────────────────────────────────────────────────────

/** Clear the shared client's backoff ladder (ADR-2100: one policy, owned there). */
export function resetSolidReconnect() {
  podNotificationManager.resetReconnect();
}

// ── Connect / Disconnect ───────────────────────────────────────────────

export function connectSolidWebSocket(set: (partial: Record<string, unknown>) => void) {
  bindLifecycle(set);

  if (podNotificationManager.isConnected) {
    logger.debug('Solid WebSocket already connected');
    // Re-sync the mirror: the socket may have been opened by the other consumer.
    set({ isSolidConnected: true, solidSocket: podNotificationManager.socket });
    return;
  }

  podNotificationManager.connect();
}

export function disconnectSolidWebSocket(set: (partial: Record<string, unknown>) => void) {
  podNotificationManager.disconnect();

  set({
    solidSocket: null,
    isSolidConnected: false,
    solidSubscriptions: new Map()
  });
}

// ── Resource subscription management ───────────────────────────────────

type SolidSubState = { solidSubscriptions: Map<string, Set<SolidNotificationCallback>>; solidSocket: WebSocket | null };
type SolidSubSet = (updater: (s: SolidSubState) => { solidSubscriptions: Map<string, Set<SolidNotificationCallback>> }) => void;

/**
 * Subscribe a callback to a Solid resource URL. The shared client owns the wire
 * (`sub`/`unsub`) and invokes the callback; `state.solidSubscriptions` is a
 * BOOKKEEPING MIRROR only — it backs `getSolidSubscriptions()` and is never
 * itself dispatched through, so a callback fires exactly once (ADR-2100).
 *
 * Returns an unsubscribe fn that drops the callback from both.
 */
export function subscribeSolidResource(
  set: SolidSubSet,
  resourceUrl: string,
  callback: SolidNotificationCallback,
): () => void {
  const unsubscribeShared = podNotificationManager.subscribe(resourceUrl, callback);
  logger.debug('Subscribed to Solid resource', { url: resourceUrl });

  set(state => {
    const newSubscriptions = new Map(state.solidSubscriptions);
    if (!newSubscriptions.has(resourceUrl)) {
      newSubscriptions.set(resourceUrl, new Set());
    }
    newSubscriptions.get(resourceUrl)!.add(callback);
    return { solidSubscriptions: newSubscriptions };
  });

  return () => {
    unsubscribeShared();

    set(state => {
      const newSubscriptions = new Map(state.solidSubscriptions);
      const callbacks = newSubscriptions.get(resourceUrl);
      if (callbacks) {
        callbacks.delete(callback);
        if (callbacks.size === 0) {
          newSubscriptions.delete(resourceUrl);
          logger.debug('Unsubscribed from Solid resource', { url: resourceUrl });
        }
      }
      return { solidSubscriptions: newSubscriptions };
    });
  };
}

/** Unsubscribe ALL callbacks for a resource URL and send `unsub` on the wire. */
export function unsubscribeSolidResource(set: SolidSubSet, resourceUrl: string): void {
  podNotificationManager.unsubscribeAll(resourceUrl);
  logger.debug('Unsubscribed from Solid resource (all callbacks)', { url: resourceUrl });

  set(state => {
    const newSubscriptions = new Map(state.solidSubscriptions);
    newSubscriptions.delete(resourceUrl);
    return { solidSubscriptions: newSubscriptions };
  });
}
