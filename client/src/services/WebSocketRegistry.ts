/**
 * WebSocketRegistry
 *
 * A connection registry that tracks all active WebSocket connections.
 * Emits lifecycle events to the WebSocketEventBus. Singleton export.
 */

import { createLogger } from '../utils/loggerConfig';
import { webSocketEventBus } from './WebSocketEventBus';

const logger = createLogger('WebSocketRegistry');

export interface RegistryEntry {
  url: string;
  ws: WebSocket;
  state: string;
  registeredAt: number;
  /** Internal: event listeners attached by register(), removed on unregister(). */
  _listeners?: Array<{ event: string; handler: EventListener }>;
}

class WebSocketRegistryImpl {
  private connections = new Map<string, RegistryEntry>();

  /**
   * Register a WebSocket connection under a unique name.
   * If a connection with the same name already exists, the old entry is
   * unregistered first (listeners removed) before registering the new one.
   */
  register(name: string, url: string, ws: WebSocket): void {
    // Clean up any existing registration with the same name to prevent
    // orphaned listeners and silent overwrite.
    if (this.connections.has(name)) {
      this.unregister(name);
    }

    const updateState = () => {
      const current = this.connections.get(name);
      if (current && current.ws === ws) {
        current.state = this.readyStateLabel(ws.readyState);
      }
    };

    const entry: RegistryEntry = {
      url,
      ws,
      state: this.readyStateLabel(ws.readyState),
      registeredAt: Date.now(),
      _listeners: [
        { event: 'open', handler: updateState },
        { event: 'close', handler: updateState },
        { event: 'error', handler: updateState },
      ],
    };

    ws.addEventListener('open', updateState);
    ws.addEventListener('close', updateState);
    ws.addEventListener('error', updateState);

    this.connections.set(name, entry);
    logger.info(`Registered WebSocket "${name}" -> ${url}`);

    webSocketEventBus.emit('registry:registered', { name, url });
  }

  /**
   * Remove a named connection from the registry.
   * Removes any event listeners that were attached during registration.
   */
  unregister(name: string): void {
    const entry = this.connections.get(name);
    if (entry) {
      // Remove listeners to prevent memory leaks
      if (entry._listeners) {
        for (const { event, handler } of entry._listeners) {
          entry.ws.removeEventListener(event, handler);
        }
      }
      this.connections.delete(name);
      logger.info(`Unregistered WebSocket "${name}"`);
      webSocketEventBus.emit('registry:unregistered', { name });
    }
  }

  /**
   * Get the WebSocket for a named connection.
   */
  get(name: string): WebSocket | undefined {
    return this.connections.get(name)?.ws;
  }

  /**
   * Get the full entry for a named connection.
   */
  getEntry(name: string): RegistryEntry | undefined {
    return this.connections.get(name);
  }

  /**
   * Return a snapshot of all active connections.
   */
  getAll(): Map<string, { url: string; ws: WebSocket; state: string }> {
    const result = new Map<string, { url: string; ws: WebSocket; state: string }>();
    for (const [name, entry] of this.connections) {
      // Update state label on read so callers always see the latest.
      entry.state = this.readyStateLabel(entry.ws.readyState);
      result.set(name, { url: entry.url, ws: entry.ws, state: entry.state });
    }
    return result;
  }

  /**
   * Number of currently tracked connections.
   */
  get size(): number {
    return this.connections.size;
  }

  // -- helpers --

  private readyStateLabel(state: number): string {
    switch (state) {
      case WebSocket.CONNECTING:
        return 'connecting';
      case WebSocket.OPEN:
        return 'open';
      case WebSocket.CLOSING:
        return 'closing';
      case WebSocket.CLOSED:
        return 'closed';
      default:
        return 'unknown';
    }
  }
}

/** Singleton registry shared across all WebSocket services. */
export const webSocketRegistry = new WebSocketRegistryImpl();
