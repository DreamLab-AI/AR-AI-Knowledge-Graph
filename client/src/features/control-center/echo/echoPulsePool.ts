/**
 * echoPulsePool — pure pool bookkeeping for Echo Pulse ring animations.
 *
 * Deliberately framework-free (no React, no @react-three/fiber) so it can be
 * unit-tested directly in jsdom without rendering an R3F scene. EchoPulseLayer
 * owns the THREE.Mesh pool and drives it once per useFrame tick using these
 * functions; all mutable animation state (position, hue, life) lives here in
 * a plain object so the R3F layer stays a thin read/apply shell.
 *
 * Time basis: callers pass a single monotonic clock (R3F's
 * `state.clock.elapsedTime`, sampled once per frame into a ref) for both
 * `spawnPulse` and `tickPulsePool`/`pulseProgress` so pulse lifetimes are not
 * skewed by mixing `performance.now()` with the R3F clock.
 */

import * as THREE from 'three';
import type { EchoPulseDetail } from './echoPulseBus';

/** Max concurrent pulses — the perf budget from design-spec.md §4.4 / §8 (WP4). */
export const ECHO_PULSE_MAX_CONCURRENT = 3;
/** Full pulse lifetime in seconds (design-spec.md §5.3: "1200ms in-scene"). */
export const ECHO_PULSE_DURATION_S = 1.2;
/** Radius (world units) the ring reaches at end of life. */
export const ECHO_PULSE_MAX_RADIUS = 1200;
/** Default hue (0..1) — matches `--primary` (hsl(217 91% 60%)) when no hue is supplied. */
export const ECHO_PULSE_DEFAULT_HUE = 217 / 360;

export interface PulseSlot {
  active: boolean;
  origin: THREE.Vector3;
  hue: number;
  strength: number;
  startTime: number;
}

export interface PulsePoolState {
  slots: PulseSlot[];
  /** Round-robin write cursor; wraps so an oversubscribed pool retires the oldest pulse first. */
  cursor: number;
}

export function createPulsePoolState(size: number = ECHO_PULSE_MAX_CONCURRENT): PulsePoolState {
  return {
    slots: Array.from({ length: size }, () => ({
      active: false,
      origin: new THREE.Vector3(),
      hue: ECHO_PULSE_DEFAULT_HUE,
      strength: 1,
      startTime: 0,
    })),
    cursor: 0,
  };
}

/**
 * Spawns a pulse into the next round-robin slot. `resolveOrigin` maps the
 * bus's `EchoPulseDetail.origin` (a literal position or `'camera-center'`)
 * into a world-space `THREE.Vector3`; EchoPulseLayer supplies a resolver
 * closed over the live R3F camera so this module never imports @react-three/fiber.
 * Returns the slot index written, for callers that want it (tests, debugging).
 */
export function spawnPulse(
  pool: PulsePoolState,
  detail: EchoPulseDetail,
  resolveOrigin: (origin: EchoPulseDetail['origin']) => THREE.Vector3,
  now: number,
): number {
  const idx = pool.cursor;
  const slot = pool.slots[idx];
  slot.active = true;
  slot.origin.copy(resolveOrigin(detail.origin));
  slot.hue = detail.hue ?? ECHO_PULSE_DEFAULT_HUE;
  slot.strength = THREE.MathUtils.clamp(detail.strength ?? 1, 0, 1);
  slot.startTime = now;
  pool.cursor = (pool.cursor + 1) % pool.slots.length;
  return idx;
}

/**
 * Advances slot lifetimes against `now`, retiring any slot whose life has
 * elapsed. Returns whether any slot is still active after retirement, so the
 * caller can skip all per-mesh matrix/material work on a fully idle frame.
 */
export function tickPulsePool(
  pool: PulsePoolState,
  now: number,
  duration: number = ECHO_PULSE_DURATION_S,
): boolean {
  let anyActive = false;
  for (const slot of pool.slots) {
    if (!slot.active) continue;
    if (now - slot.startTime >= duration) {
      slot.active = false;
      continue;
    }
    anyActive = true;
  }
  return anyActive;
}

/** Normalised [0..1] life progress for a slot at time `now`. */
export function pulseProgress(
  slot: PulseSlot,
  now: number,
  duration: number = ECHO_PULSE_DURATION_S,
): number {
  return THREE.MathUtils.clamp((now - slot.startTime) / duration, 0, 1);
}

/** Standard ease-out-cubic — fast expansion, settling near the end of life. */
export function easeOutCubic(t: number): number {
  const inv = 1 - t;
  return 1 - inv * inv * inv;
}
