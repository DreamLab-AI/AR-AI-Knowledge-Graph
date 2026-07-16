import { describe, it, expect } from 'vitest';
import {
  createAttentionHeatAccumulator,
  normaliseHeat,
  HEAT_SATURATION,
  MAX_RAW_HEAT,
} from '../attentionHeat';
import { KNOWLEDGE_NODE_FLAG, ONTOLOGY_CLASS_FLAG } from '@/types/binaryProtocol';

/** A controllable clock so decay is deterministic under test. */
function makeClock(start = 0) {
  const state = { t: start };
  return { now: () => state.t, advance: (ms: number) => { state.t += ms; }, set: (ms: number) => { state.t = ms; } };
}

describe('attentionHeat accumulator', () => {
  it('touch raises heat to the normalised single-touch value', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000 });

    expect(acc.getHeat(5)).toBe(0); // untouched → cold
    acc.touch(5);
    expect(acc.getHeat(5)).toBeCloseTo(normaliseHeat(1), 6);
    expect(acc.size()).toBe(1);
  });

  it('accumulates monotonically and saturates below 1', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000 });

    acc.touch(1);
    const one = acc.getHeat(1);
    acc.touch(1);
    const two = acc.getHeat(1);
    expect(two).toBeGreaterThan(one); // more attention → hotter

    // Hammer far past the raw cap; heat stays in (0.9, 1) — never a flat 1.0.
    for (let i = 0; i < 100; i++) acc.touch(1);
    const hot = acc.getHeat(1);
    expect(hot).toBeGreaterThan(0.9);
    expect(hot).toBeLessThan(1);
    expect(hot).toBeCloseTo(normaliseHeat(MAX_RAW_HEAT), 6); // raw is capped
  });

  it('decays exponentially with the configured half-life', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000 });

    acc.touch(7); // raw = 1 at t=0
    clock.advance(1000); // one half-life → raw halves
    expect(acc.getHeat(7)).toBeCloseTo(normaliseHeat(0.5), 6);
    clock.advance(1000); // two half-lives → quarter
    expect(acc.getHeat(7)).toBeCloseTo(normaliseHeat(0.25), 6);
  });

  it('reconciles wire flag bits with masked client node ids', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000 });

    // A beam targets a knowledge node carrying the KNOWLEDGE flag on the wire...
    acc.touch(KNOWLEDGE_NODE_FLAG | 42);
    // ...the gem's node.id is the masked string "42" — same heat entry.
    expect(acc.getHeat('42')).toBeCloseTo(normaliseHeat(1), 6);
    expect(acc.getHeat(42)).toBeCloseTo(normaliseHeat(1), 6);

    // Ontology flag bits mask the same way.
    acc.touch(ONTOLOGY_CLASS_FLAG | 9);
    expect(acc.getHeat('9')).toBeGreaterThan(0);
    // Distinct base ids never collide.
    expect(acc.getHeat('43')).toBe(0);
  });

  it('caps map size and evicts the coldest entry', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000, maxEntries: 3 });

    acc.touch(1);              // raw 1 — coldest
    acc.touch(2); acc.touch(2); // raw 2
    acc.touch(3); acc.touch(3); acc.touch(3); // raw 3
    expect(acc.size()).toBe(3);

    acc.touch(4); // over cap → evict the coldest (node 1)
    expect(acc.size()).toBe(3);
    expect(acc.getHeat(1)).toBe(0);   // evicted
    expect(acc.getHeat(4)).toBeGreaterThan(0);
    expect(acc.getHeat(3)).toBeGreaterThan(0);
  });

  it('reports and sweeps cold entries once heat decays away', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000 });

    acc.touch(1);
    expect(acc.hasHeat()).toBe(true);

    clock.advance(1000 * 15); // ~15 half-lives → raw ≈ 3e-5, below epsilon
    expect(acc.hasHeat()).toBe(false);
    expect(acc.getHeat(1)).toBeLessThan(0.001);

    const removed = acc.sweep();
    expect(removed).toBe(1);
    expect(acc.size()).toBe(0);
  });

  it('honours the enabled flag and configure()', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000, enabled: false });

    acc.touch(1);
    acc.touchMany([2, 3]);
    expect(acc.size()).toBe(0); // disabled → no accumulation
    expect(acc.getHeat(1)).toBe(0);

    acc.configure({ enabled: true });
    acc.touch(1);
    expect(acc.getHeat(1)).toBeGreaterThan(0);

    // Half-life change takes effect for subsequent reads.
    acc.configure({ halfLifeMs: 500 });
    clock.advance(500);
    expect(acc.getHeat(1)).toBeCloseTo(normaliseHeat(0.5), 6);
  });

  it('bumps the version and notifies subscribers on each touch batch', () => {
    const clock = makeClock();
    const acc = createAttentionHeatAccumulator({ now: clock.now, halfLifeMs: 1000 });

    let fired = 0;
    const unsub = acc.subscribe(() => { fired++; });

    const v0 = acc.getVersion();
    acc.touchMany([1, 2, 3]); // one bump for the batch
    expect(acc.getVersion()).toBe(v0 + 1);
    expect(fired).toBe(1);

    acc.touch(4); // single touch also bumps
    expect(acc.getVersion()).toBe(v0 + 2);
    expect(fired).toBe(2);

    // Empty batch is a no-op — no bump.
    acc.touchMany([]);
    expect(acc.getVersion()).toBe(v0 + 2);

    unsub();
    acc.touch(5);
    expect(fired).toBe(2); // unsubscribed → no further notifications
  });

  it('normaliseHeat is bounded and monotonic', () => {
    expect(normaliseHeat(0)).toBe(0);
    expect(normaliseHeat(-1)).toBe(0);
    expect(normaliseHeat(1)).toBeCloseTo(1 - Math.exp(-1 / HEAT_SATURATION), 6);
    // The curve asymptotes to 1 (float-saturates there for huge inputs); what
    // matters is the accumulator caps raw at MAX_RAW_HEAT so its output stays <1.
    expect(normaliseHeat(1000)).toBeLessThanOrEqual(1);
    expect(normaliseHeat(MAX_RAW_HEAT)).toBeLessThan(1);
    expect(normaliseHeat(2)).toBeGreaterThan(normaliseHeat(1));
  });
});
