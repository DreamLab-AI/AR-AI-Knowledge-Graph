/**
 * Connection-telemetry hooks for the unified Status surface.
 *
 * These read the websocket store/service directly (the surface no longer has
 * status threaded in as props). Two concerns live here:
 *
 *  - useWebSocketStatus: the 3-state socket-lifecycle readout, ported verbatim
 *    from ControlCenter's D5 fix (CANARY-VC-D5-WS). It tracks the real
 *    onConnectionStatusChange lifecycle, never a hardcoded literal, so the
 *    health dot flips to `disconnected` when the socket actually drops.
 *
 *  - useLayoutMotion: an HONEST physics-motion readout. The server's
 *    settlementState is not trustworthy — `kineticEnergy` reports 0.0 and
 *    `isSettled` true even while nodes are visibly moving (queen-measured
 *    server nit). So rather than trust that field, we watch the real inbound
 *    position-frame throughput: during a live layout the binary broadcaster
 *    streams frames continuously (many/sec); once the graph settles the stream
 *    drops to the occasional periodic full broadcast + 30s heartbeat. Sampling
 *    `messagesReceived` on a 1s cadence separates 'morphing' from 'settled'
 *    without a reactive per-frame re-render of the panel.
 */

import { useEffect, useRef, useState } from 'react';
import { webSocketService, useWebSocketStore } from '../../../store/websocketStore';

export type WebSocketStatus = 'connected' | 'connecting' | 'disconnected';

export function useWebSocketStatus(): WebSocketStatus {
  const [status, setStatus] = useState<WebSocketStatus>(() =>
    webSocketService.isReady() ? 'connected' : 'connecting',
  );

  useEffect(() => {
    const unsubscribe = webSocketService.onConnectionStatusChange((connected) => {
      setStatus(connected ? 'connected' : 'disconnected');
    });
    // Seed from the current lifecycle state in case a transition fired before
    // this effect subscribed.
    setStatus(webSocketService.isReady() ? 'connected' : 'connecting');
    return unsubscribe;
  }, []);

  return status;
}

export type LayoutMotion = 'morphing' | 'settled' | 'stale';

export interface LayoutMotionReadout {
  motion: LayoutMotion;
  /** Inbound frames/sec over the last sample window (diagnostic). */
  ratePerSec: number;
  /** False when the feed has gone quiet past the liveness grace window. */
  feedFresh: boolean;
}

/** Frames/sec above this sustained rate reads as an actively morphing layout. */
const MORPHING_RATE_THRESHOLD = 3;
/** Liveness grace: heartbeat pings refresh activity every 30s regardless. */
const FEED_STALE_MS = 90_000;

export function useLayoutMotion(enabled: boolean): LayoutMotionReadout {
  const [readout, setReadout] = useState<LayoutMotionReadout>({
    motion: 'settled',
    ratePerSec: 0,
    feedFresh: true,
  });
  const sample = useRef<{ count: number; time: number } | null>(null);

  useEffect(() => {
    if (!enabled) {
      sample.current = null;
      return;
    }
    const read = () => {
      const stats = useWebSocketStore.getState().statistics;
      const now = Date.now();
      const prev = sample.current;
      sample.current = { count: stats.messagesReceived, time: now };
      const feedFresh = now - stats.lastActivity < FEED_STALE_MS;
      if (!prev) {
        setReadout((r) => ({ ...r, feedFresh }));
        return;
      }
      const dt = (now - prev.time) / 1000;
      if (dt <= 0) return;
      const ratePerSec = Math.max(0, (stats.messagesReceived - prev.count) / dt);
      const motion: LayoutMotion = !feedFresh
        ? 'stale'
        : ratePerSec > MORPHING_RATE_THRESHOLD
          ? 'morphing'
          : 'settled';
      setReadout({ motion, ratePerSec, feedFresh });
    };
    read();
    const id = window.setInterval(read, 1000);
    return () => window.clearInterval(id);
  }, [enabled]);

  return readout;
}
