/**
 * Echo Pulse bus + pool unit tests.
 *
 * Scope: the bus wiring (emit -> subscribe -> unsubscribe) and the pure pool
 * bookkeeping module (echoPulsePool.ts) that EchoPulseLayer drives from
 * useFrame. Deliberately does NOT render EchoPulseLayer itself — R3F scenes
 * need a real WebGL/WebGPU canvas and reconciler tick to exercise
 * meaningfully in jsdom; instead we test the same functions the component
 * calls, plus the event contract it subscribes to, which together prove the
 * wiring without a false sense of coverage from a canvas-less render.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as THREE from 'three';
import { emitEchoPulse, subscribeEchoPulse, ECHO_PULSE_EVENT, type EchoPulseDetail } from '../echoPulseBus';
import {
  createPulsePoolState,
  spawnPulse,
  tickPulsePool,
  pulseProgress,
  easeOutCubic,
  ECHO_PULSE_MAX_CONCURRENT,
  ECHO_PULSE_DURATION_S,
  ECHO_PULSE_DEFAULT_HUE,
} from '../echoPulsePool';

const identityResolver = (origin: EchoPulseDetail['origin']): THREE.Vector3 =>
  origin === 'camera-center' ? new THREE.Vector3(0, 0, -600) : new THREE.Vector3(...origin);

describe('echoPulseBus wiring', () => {
  it('delivers emitted detail to a subscriber, stamped with ts', () => {
    const received: EchoPulseDetail[] = [];
    const unsubscribe = subscribeEchoPulse((detail) => received.push(detail));

    emitEchoPulse({ origin: 'camera-center', strength: 0.8 });

    expect(received).toHaveLength(1);
    expect(received[0].origin).toBe('camera-center');
    expect(received[0].strength).toBe(0.8);
    expect(typeof received[0].ts).toBe('number');

    unsubscribe();
  });

  it('stops delivering events after unsubscribe', () => {
    const received: EchoPulseDetail[] = [];
    const unsubscribe = subscribeEchoPulse((detail) => received.push(detail));
    unsubscribe();

    emitEchoPulse({ origin: 'camera-center' });

    expect(received).toHaveLength(0);
  });

  it('dispatches on the documented event name', () => {
    const handler = vi.fn();
    window.addEventListener(ECHO_PULSE_EVENT, handler);

    emitEchoPulse({ origin: [1, 2, 3] });

    expect(handler).toHaveBeenCalledTimes(1);
    window.removeEventListener(ECHO_PULSE_EVENT, handler);
  });
});

describe('echoPulsePool', () => {
  it('creates a pool of ECHO_PULSE_MAX_CONCURRENT inactive slots', () => {
    const pool = createPulsePoolState();
    expect(pool.slots).toHaveLength(ECHO_PULSE_MAX_CONCURRENT);
    expect(pool.slots.every((s) => !s.active)).toBe(true);
  });

  it('spawnPulse fills slots round-robin and wraps around the pool', () => {
    const pool = createPulsePoolState(3);

    const i0 = spawnPulse(pool, { origin: [0, 0, 0], ts: 0 }, identityResolver, 0);
    const i1 = spawnPulse(pool, { origin: [1, 0, 0], ts: 0 }, identityResolver, 0);
    const i2 = spawnPulse(pool, { origin: [2, 0, 0], ts: 0 }, identityResolver, 0);
    expect([i0, i1, i2]).toEqual([0, 1, 2]);
    expect(pool.slots.every((s) => s.active)).toBe(true);

    // Fourth spawn wraps and retires the oldest (slot 0).
    const i3 = spawnPulse(pool, { origin: [3, 0, 0], ts: 0 }, identityResolver, 0);
    expect(i3).toBe(0);
    expect(pool.slots[0].origin.x).toBe(3);
  });

  it('defaults hue and clamps strength when not supplied / out of range', () => {
    const pool = createPulsePoolState(1);
    spawnPulse(pool, { origin: 'camera-center', ts: 0 }, identityResolver, 0);
    expect(pool.slots[0].hue).toBe(ECHO_PULSE_DEFAULT_HUE);
    expect(pool.slots[0].strength).toBe(1);

    spawnPulse(pool, { origin: 'camera-center', strength: 4, ts: 0 }, identityResolver, 0);
    expect(pool.slots[0].strength).toBe(1);

    spawnPulse(pool, { origin: 'camera-center', strength: -2, ts: 0 }, identityResolver, 0);
    expect(pool.slots[0].strength).toBe(0);
  });

  it('resolves camera-center origin via the supplied resolver', () => {
    const pool = createPulsePoolState(1);
    spawnPulse(pool, { origin: 'camera-center', ts: 0 }, identityResolver, 0);
    expect(pool.slots[0].origin.z).toBe(-600);
  });

  it('tickPulsePool retires a slot once its duration elapses', () => {
    const pool = createPulsePoolState(1);
    spawnPulse(pool, { origin: [0, 0, 0], ts: 0 }, identityResolver, 10);

    expect(tickPulsePool(pool, 10.5, ECHO_PULSE_DURATION_S)).toBe(true);
    expect(pool.slots[0].active).toBe(true);

    expect(tickPulsePool(pool, 10 + ECHO_PULSE_DURATION_S + 0.01, ECHO_PULSE_DURATION_S)).toBe(false);
    expect(pool.slots[0].active).toBe(false);
  });

  it('tickPulsePool reports idle for a pool with no spawned pulses', () => {
    const pool = createPulsePoolState();
    expect(tickPulsePool(pool, 5, ECHO_PULSE_DURATION_S)).toBe(false);
  });

  it('pulseProgress is monotonic and clamped to [0,1]', () => {
    const pool = createPulsePoolState(1);
    spawnPulse(pool, { origin: [0, 0, 0], ts: 0 }, identityResolver, 0);
    const slot = pool.slots[0];

    expect(pulseProgress(slot, 0, ECHO_PULSE_DURATION_S)).toBe(0);
    expect(pulseProgress(slot, ECHO_PULSE_DURATION_S / 2, ECHO_PULSE_DURATION_S)).toBeCloseTo(0.5, 5);
    expect(pulseProgress(slot, ECHO_PULSE_DURATION_S, ECHO_PULSE_DURATION_S)).toBe(1);
    // Past end of life still clamps to 1 (caller retires via tickPulsePool separately).
    expect(pulseProgress(slot, ECHO_PULSE_DURATION_S * 5, ECHO_PULSE_DURATION_S)).toBe(1);
  });

  it('easeOutCubic starts at 0, ends at 1, and is monotonically increasing', () => {
    expect(easeOutCubic(0)).toBe(0);
    expect(easeOutCubic(1)).toBe(1);
    let prev = -1;
    for (let t = 0; t <= 1; t += 0.1) {
      const v = easeOutCubic(t);
      expect(v).toBeGreaterThanOrEqual(prev);
      prev = v;
    }
  });
});

describe('bus -> pool integration (as EchoPulseLayer wires it)', () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it('a subscribeEchoPulse callback spawning into the pool reflects the emitted detail', () => {
    const pool = createPulsePoolState();
    let clock = 42;

    const unsubscribe = subscribeEchoPulse((detail) => {
      spawnPulse(pool, detail, identityResolver, clock);
    });

    emitEchoPulse({ origin: [10, 20, 30], hue: 0.3, strength: 0.5 });

    expect(pool.slots[0].active).toBe(true);
    expect(pool.slots[0].origin.x).toBe(10);
    expect(pool.slots[0].hue).toBe(0.3);
    expect(pool.slots[0].strength).toBe(0.5);
    expect(pool.slots[0].startTime).toBe(clock);

    unsubscribe();
  });

  it('respects the ECHO_PULSE_MAX_CONCURRENT budget across a burst of commits', () => {
    const pool = createPulsePoolState();
    const unsubscribe = subscribeEchoPulse((detail) => {
      spawnPulse(pool, detail, identityResolver, 0);
    });

    for (let i = 0; i < 10; i++) {
      emitEchoPulse({ origin: [i, 0, 0] });
    }

    expect(pool.slots).toHaveLength(ECHO_PULSE_MAX_CONCURRENT);
    expect(pool.slots.every((s) => s.active)).toBe(true);
    // Round-robin: the last write lands at (10 - 1) % 3 = 0, holding origin.x = 9.
    expect(pool.slots[0].origin.x).toBe(9);

    unsubscribe();
  });
});
