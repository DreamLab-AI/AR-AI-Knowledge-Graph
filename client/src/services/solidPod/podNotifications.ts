/**
 * Pod Notifications
 *
 * Manages the WebSocket connection to a Solid/JSS server for the
 * solid-0.1 notification protocol:
 * - Connect / disconnect with exponential-backoff reconnect
 * - Subscribe / unsubscribe to resource URLs
 * - Route incoming pub/ack messages to registered callbacks
 * - Integrates with WebSocketRegistry and WebSocketEventBus
 */

import { createLogger } from '../../utils/loggerConfig';
import { webSocketRegistry } from '../WebSocketRegistry';
import { webSocketEventBus } from '../WebSocketEventBus';

const logger = createLogger('SolidPodService:ws');

export const JSS_WS_URL = import.meta.env.VITE_JSS_WS_URL || null;

const REGISTRY_NAME = 'solid-pod';

/**
 * The ONE reconnect policy for the JSS notification socket (ADR-2100).
 *
 * There used to be a second, independent client to this same VITE_JSS_WS_URL in
 * the Zustand store (`store/websocket/solidWebSocket.ts`), carrying its own
 * 10-attempt ladder against this file's 5. Two sockets meant the pod saw two
 * subscriber sets for the same resources and two divergent backoff policies.
 * The store now consumes `podNotificationManager` below, so these constants are
 * the single source of truth for retry behaviour.
 */
export const SOLID_MAX_RECONNECT_ATTEMPTS = 5;
export const SOLID_RECONNECT_DELAY_MS = 1000;

export interface SolidNotification {
  type: 'pub' | 'ack';
  url: string;
}

type NotificationCallback = (notification: SolidNotification) => void;

/**
 * Connection-level events, distinct from the per-resource notifications above.
 * The Zustand store mirrors these into its own state and event bus so store
 * consumers keep the surface they had back when they owned a socket of their
 * own (ADR-2100).
 */
export type SolidLifecycleEvent =
  | { type: 'open'; url: string }
  | { type: 'close'; code: number; reason: string }
  | { type: 'error'; error: unknown }
  | { type: 'protocol'; protocol: string }
  | { type: 'server-error'; message: string }
  | { type: 'pub'; url: string };

type LifecycleListener = (event: SolidLifecycleEvent) => void;

export class PodNotificationManager {
  private wsConnection: WebSocket | null = null;
  private subscriptions: Map<string, Set<NotificationCallback>> = new Map();
  private lifecycleListeners: Set<LifecycleListener> = new Set();
  private reconnectAttempts = 0;
  private readonly maxReconnectAttempts = SOLID_MAX_RECONNECT_ATTEMPTS;
  private readonly reconnectDelay = SOLID_RECONNECT_DELAY_MS;
  private reconnectTimerId: ReturnType<typeof setTimeout> | null = null;
  private isDisconnecting = false;

  /** Connect to JSS WebSocket for real-time notifications. */
  connect(): void {
    if (!JSS_WS_URL) {
      logger.warn('JSS WebSocket URL not configured');
      return;
    }

    if (this.wsConnection?.readyState === WebSocket.OPEN) {
      logger.debug('WebSocket already connected');
      return;
    }

    try {
      const validatedUrl = new URL(JSS_WS_URL);
      if (validatedUrl.protocol !== 'ws:' && validatedUrl.protocol !== 'wss:') {
        logger.error('Invalid WebSocket protocol', { protocol: validatedUrl.protocol });
        return;
      }

      this.wsConnection = new WebSocket(validatedUrl.href);

      this.wsConnection.onopen = () => {
        logger.info('JSS WebSocket connected');
        this.reconnectAttempts = 0;
        webSocketRegistry.register(REGISTRY_NAME, validatedUrl.href, this.wsConnection!);
        webSocketEventBus.emit('connection:open', { name: REGISTRY_NAME, url: validatedUrl.href });
        this.emitLifecycle({ type: 'open', url: validatedUrl.href });
      };

      this.wsConnection.onmessage = (event) => {
        const msg = event.data.toString().trim();
        webSocketEventBus.emit('message:pod', { data: msg });
        this.handleMessage(msg);
      };

      this.wsConnection.onerror = (error) => {
        logger.error('JSS WebSocket error', { error });
        webSocketEventBus.emit('connection:error', { name: REGISTRY_NAME, error });
        this.emitLifecycle({ type: 'error', error });
      };

      this.wsConnection.onclose = (event) => {
        logger.info('JSS WebSocket disconnected');
        webSocketRegistry.unregister(REGISTRY_NAME);
        webSocketEventBus.emit('connection:close', {
          name: REGISTRY_NAME,
          code: event.code,
          reason: event.reason,
        });
        this.emitLifecycle({ type: 'close', code: event.code, reason: event.reason });
        if (this.isDisconnecting) {
          this.isDisconnecting = false;
          return;
        }
        this.handleReconnect();
      };
    } catch (error) {
      logger.error('Failed to connect WebSocket', { error });
    }
  }

  /** Subscribe to notifications for a resource URL. Returns an unsubscribe fn. */
  subscribe(resourceUrl: string, callback: NotificationCallback): () => void {
    if (!this.subscriptions.has(resourceUrl)) {
      this.subscriptions.set(resourceUrl, new Set());
      if (this.wsConnection?.readyState === WebSocket.OPEN) {
        this.wsConnection.send(`sub ${resourceUrl}`);
      }
    }

    this.subscriptions.get(resourceUrl)!.add(callback);

    return () => {
      this.subscriptions.get(resourceUrl)?.delete(callback);
      if (this.subscriptions.get(resourceUrl)?.size === 0) {
        if (this.wsConnection?.readyState === WebSocket.OPEN) {
          this.wsConnection.send(`unsub ${resourceUrl}`);
        }
        this.subscriptions.delete(resourceUrl);
      }
    };
  }

  /** Close the WebSocket and cancel any pending reconnect timer. */
  disconnect(): void {
    if (this.reconnectTimerId !== null) {
      clearTimeout(this.reconnectTimerId);
      this.reconnectTimerId = null;
    }
    this.isDisconnecting = true;
    webSocketRegistry.unregister(REGISTRY_NAME);
    if (this.wsConnection) {
      this.wsConnection.close();
      this.wsConnection = null;
    }
    this.subscriptions.clear();
  }

  /** Whether a WebSocket connection is currently requested. */
  get isConnected(): boolean {
    return this.wsConnection?.readyState === WebSocket.OPEN;
  }

  /** The live socket, for consumers that mirror readyState into their own state. */
  get socket(): WebSocket | null {
    return this.wsConnection;
  }

  /** URLs with at least one live subscriber. */
  get subscribedUrls(): string[] {
    return Array.from(this.subscriptions.keys());
  }

  /**
   * Register a connection-level listener. Returns an unregister fn. Listener
   * errors are contained so one bad consumer cannot break the socket's own
   * bookkeeping.
   */
  onLifecycle(listener: LifecycleListener): () => void {
    this.lifecycleListeners.add(listener);
    return () => {
      this.lifecycleListeners.delete(listener);
    };
  }

  /** Drop every callback for a resource URL and send `unsub` on the wire. */
  unsubscribeAll(resourceUrl: string): void {
    if (!this.subscriptions.has(resourceUrl)) return;
    if (this.wsConnection?.readyState === WebSocket.OPEN) {
      this.wsConnection.send(`unsub ${resourceUrl}`);
    }
    this.subscriptions.delete(resourceUrl);
  }

  /** Clear the backoff ladder and cancel a pending reconnect, keeping the socket. */
  resetReconnect(): void {
    this.reconnectAttempts = 0;
    if (this.reconnectTimerId !== null) {
      clearTimeout(this.reconnectTimerId);
      this.reconnectTimerId = null;
    }
  }

  // -------------------------------------------------------------------------
  // Private
  // -------------------------------------------------------------------------

  private emitLifecycle(event: SolidLifecycleEvent): void {
    this.lifecycleListeners.forEach((listener) => {
      try {
        listener(event);
      } catch (error) {
        logger.error('Error in Solid lifecycle listener', { event: event.type, error });
      }
    });
  }

  private handleMessage(msg: string): void {
    if (msg.startsWith('protocol ')) {
      const protocol = msg.slice(9);
      logger.debug('WebSocket protocol handshake complete', { protocol });
      for (const url of this.subscriptions.keys()) {
        this.wsConnection?.send(`sub ${url}`);
      }
      this.emitLifecycle({ type: 'protocol', protocol });
    } else if (msg.startsWith('ack ')) {
      const url = msg.slice(4);
      logger.debug('Subscription acknowledged', { url });
      this.notifySubscribers(url, { type: 'ack', url });
    } else if (msg.startsWith('pub ')) {
      const url = msg.slice(4);
      logger.debug('Resource changed', { url });
      this.notifySubscribers(url, { type: 'pub', url });
      this.emitLifecycle({ type: 'pub', url });
    } else if (msg.startsWith('error ')) {
      // Server-reported protocol error. Carried by the store's client before
      // consolidation (ADR-2100); kept here so no consumer loses the signal.
      const message = msg.slice(6);
      logger.error('Solid WebSocket error message', { error: message });
      this.emitLifecycle({ type: 'server-error', message });
    }
  }

  private notifySubscribers(url: string, notification: SolidNotification): void {
    this.dispatch(url, url, notification);

    // Also notify container (parent directory) subscribers
    const containerUrl = url.substring(0, url.lastIndexOf('/') + 1);
    if (containerUrl !== url) {
      this.dispatch(containerUrl, url, notification);
    }
  }

  /**
   * Callback errors are contained per subscriber: with one shared socket
   * (ADR-2100) a throwing consumer must not stop its peers from being notified.
   */
  private dispatch(key: string, url: string, notification: SolidNotification): void {
    this.subscriptions.get(key)?.forEach((cb) => {
      try {
        cb(notification);
      } catch (error) {
        logger.error('Error in Solid notification callback', { key, url, error });
      }
    });
  }

  private handleReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      logger.warn('Max reconnect attempts reached');
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
    logger.info(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    this.reconnectTimerId = setTimeout(() => {
      this.reconnectTimerId = null;
      this.connect();
    }, delay);
  }
}

/**
 * The single JSS notification client for the whole app (ADR-2100).
 *
 * Both consumers bind to this instance: SolidPodService (pod reads/writes) and
 * the Zustand WebSocket store (`store/websocket/solidWebSocket.ts`). One socket,
 * one subscriber registry, one reconnect ladder, one WebSocketRegistry entry
 * under the name `solid-pod`.
 */
export const podNotificationManager = new PodNotificationManager();
