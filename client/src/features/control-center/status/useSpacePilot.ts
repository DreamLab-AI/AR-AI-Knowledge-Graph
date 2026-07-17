/**
 * useSpacePilot — the SpacePilot/SpaceDriver wiring, lifted out of the old
 * top-right cluster so the unified Status surface can read the device state
 * directly instead of having it threaded down as props.
 *
 * Ports the connect/scan flow and the WebHID/secure-context detection that
 * previously lived split across SpacePilotStatus (connect button) and the
 * standalone SpaceMouseStatus banner (support/secure-context guidance). The
 * whole SpacePilot story now lives in one place.
 *
 * Subscriptions here are event-driven (SpaceDriver is an EventTarget) — no
 * polling — so the collapsed Status chip can consume `connected` cheaply.
 */

import { useCallback, useEffect, useState } from 'react';
import { SpaceDriver } from '../../../services/SpaceDriverService';

export interface SpacePilotState {
  /** WebHID present in this browser (Chrome/Edge). */
  isSupported: boolean;
  /** Page served from a secure context (HTTPS or localhost) — the WebHID gate. */
  isSecureContext: boolean;
  /** localhost/127.0.0.1 — surfaced so the guidance can name the escape hatch. */
  isLocalhost: boolean;
  connected: boolean;
  /** HID productName once a device is opened. */
  deviceName?: string;
  buttons: string[];
  /** Opens the browser device chooser (SpaceDriver.scan). */
  connect: () => Promise<void>;
}

export function useSpacePilot(): SpacePilotState {
  const [isSupported, setIsSupported] = useState(false);
  const [connected, setConnected] = useState(false);
  const [deviceName, setDeviceName] = useState<string | undefined>(undefined);
  const [buttons, setButtons] = useState<string[]>([]);

  // Secure-context and hostname are fixed for the page lifetime — read once.
  const [isSecureContext] = useState(() =>
    typeof window !== 'undefined' ? window.isSecureContext : false,
  );
  const [isLocalhost] = useState(() => {
    if (typeof window === 'undefined') return false;
    const host = window.location.hostname;
    return host === 'localhost' || host === '127.0.0.1';
  });

  useEffect(() => {
    setIsSupported(typeof navigator !== 'undefined' && 'hid' in navigator);
  }, []);

  useEffect(() => {
    const handleConnect = () => {
      setConnected(true);
      setDeviceName(SpaceDriver.getDevice()?.productName || undefined);
    };
    const handleDisconnect = () => {
      setConnected(false);
      setDeviceName(undefined);
      setButtons([]);
    };
    const handleButtons = (event: Event) => {
      const detail = (event as CustomEvent<{ buttons?: string[] }>).detail;
      setButtons(detail?.buttons ?? []);
    };

    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);
    SpaceDriver.addEventListener('buttons', handleButtons);

    // Seed from any device already open before this effect subscribed.
    if (SpaceDriver.isConnected()) handleConnect();

    return () => {
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
      SpaceDriver.removeEventListener('buttons', handleButtons);
    };
  }, []);

  const connect = useCallback(async () => {
    try {
      await SpaceDriver.scan();
    } catch {
      /* device selection cancelled / unavailable — no-op */
    }
  }, []);

  return { isSupported, isSecureContext, isLocalhost, connected, deviceName, buttons, connect };
}
