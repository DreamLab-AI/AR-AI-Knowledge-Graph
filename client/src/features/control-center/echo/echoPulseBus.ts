/**
 * echoPulseBus — commit-time visual feedback event bus.
 *
 * Reuses the codebase's established `window` CustomEvent pattern (already used
 * for 'visionclaw:search', 'visionclaw:status', 'visionclaw:node-selected').
 * Producers (SettingSlider, MacroDial, SettingToggle) call `emitEchoPulse` on
 * COMMIT ONLY (pointerup/change), never per drag tick. The R3F consumer
 * (WP4's EchoPulseLayer) subscribes via `subscribeEchoPulse`.
 *
 * See design-spec.md §4.4.
 */

export const ECHO_PULSE_EVENT = 'visionclaw:echo-pulse';

export interface EchoPulseDetail {
  /** Selected node world-pos, else camera focus. */
  origin: [number, number, number] | 'camera-center';
  /** 0..1, optional tint (defaults to accent). */
  hue?: number;
  /** 0..1, drives ring emissive peak + radius. */
  strength?: number;
  ts: number;
}

function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}

/**
 * Fires a single commit-time pulse. Fails open under `prefers-reduced-motion`
 * as a source-level safety net; consumers may layer additional feature-flag
 * gating (e.g. useControlCenterUI.echoPulseEnabled) on top of this.
 */
export function emitEchoPulse(detail: Omit<EchoPulseDetail, 'ts'>): void {
  if (typeof window === 'undefined') return;
  if (prefersReducedMotion()) return;

  window.dispatchEvent(
    new CustomEvent<EchoPulseDetail>(ECHO_PULSE_EVENT, {
      detail: { ...detail, ts: performance.now() },
    })
  );
}

/** Subscribe to echo pulses; returns an unsubscribe function. */
export function subscribeEchoPulse(cb: (detail: EchoPulseDetail) => void): () => void {
  if (typeof window === 'undefined') return () => {};

  const handler = (event: Event) => {
    cb((event as CustomEvent<EchoPulseDetail>).detail);
  };

  window.addEventListener(ECHO_PULSE_EVENT, handler);
  return () => window.removeEventListener(ECHO_PULSE_EVENT, handler);
}
