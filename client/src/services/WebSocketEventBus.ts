/**
 * WebSocketEventBus
 *
 * A typed pub/sub event bus that all WebSocket services use to communicate
 * cross-service events without direct coupling. Singleton export.
 */

import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('WebSocketEventBus');

// --- Event type catalogue ---

export type WebSocketEventType =
  | 'connection:open'
  | 'connection:close'
  | 'connection:error'
  | 'message:voice'
  | 'message:bots'
  | 'message:pod'
  | 'registry:registered'
  | 'registry:unregistered'
  | 'registry:closedAll';

export interface WebSocketEventPayload {
  'connection:open': { name: string; url: string };
  'connection:close': { name: string; code?: number; reason?: string };
  'connection:error': { name: string; error: unknown };
  'message:voice': { data: unknown };
  'message:bots': { data: unknown };
  'message:pod': { data: unknown };
  'registry:registered': { name: string; url: string };
  'registry:unregistered': { name: string };
  'registry:closedAll': { count: number };
}

type Handler<T = unknown> = (data: T) => void;

class WebSocketEventBusImpl {
  private handlers = new Map<string, Set<Handler<any>>>();

  /**
   * Subscribe to an event. Returns an unsubscribe function.
   */
  on<E extends WebSocketEventType>(
    event: E,
    handler: Handler<WebSocketEventPayload[E]>,
  ): () => void {
    if (!this.handlers.has(event)) {
      this.handlers.set(event, new Set());
    }
    this.handlers.get(event)!.add(handler);

    return () => {
      this.off(event, handler);
    };
  }

  /**
   * Unsubscribe a handler from an event.
   */
  off<E extends WebSocketEventType>(
    event: E,
    handler: Handler<WebSocketEventPayload[E]>,
  ): void {
    const set = this.handlers.get(event);
    if (set) {
      set.delete(handler);
      if (set.size === 0) {
        this.handlers.delete(event);
      }
    }
  }

  /**
   * Emit an event to all registered handlers.
   */
  emit<E extends WebSocketEventType>(
    event: E,
    data: WebSocketEventPayload[E],
  ): void {
    const set = this.handlers.get(event);
    if (!set || set.size === 0) return;

    for (const handler of set) {
      try {
        handler(data);
      } catch (error) {
        logger.error(`Error in WebSocketEventBus handler for "${event}"`, { error });
      }
    }
  }

  /**
   * Remove all handlers (useful for testing / teardown).
   */
  clear(): void {
    this.handlers.clear();
  }
}

/** Singleton event bus shared across all WebSocket services. */
export const webSocketEventBus = new WebSocketEventBusImpl();
