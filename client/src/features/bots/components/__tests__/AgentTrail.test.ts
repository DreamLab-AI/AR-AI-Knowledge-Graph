import { describe, it, expect } from 'vitest';
import {
  createTrailRing,
  shouldSample,
  pushSample,
  copyOrdered,
  trailFade,
  TRAIL_MIN_LENGTH,
  TRAIL_MAX_LENGTH,
} from '../AgentTrail';

describe('AgentTrail — shouldSample (displacement gate)', () => {
  it('always samples when there is no previous sample (seeds the trail)', () => {
    expect(shouldSample(null, { x: 0, y: 0, z: 0 }, 0.05)).toBe(true);
    expect(shouldSample(undefined, { x: 9, y: 9, z: 9 }, 0.05)).toBe(true);
  });

  it('rejects sub-epsilon displacement (idle agent grows no trail)', () => {
    const prev = { x: 1, y: 2, z: 3 };
    // moved 0.03 in x → below the 0.05 threshold
    expect(shouldSample(prev, { x: 1.03, y: 2, z: 3 }, 0.05)).toBe(false);
    // no movement at all
    expect(shouldSample(prev, { x: 1, y: 2, z: 3 }, 0.05)).toBe(false);
  });

  it('accepts displacement at or beyond epsilon', () => {
    const prev = { x: 0, y: 0, z: 0 };
    // exactly epsilon along one axis
    expect(shouldSample(prev, { x: 0.05, y: 0, z: 0 }, 0.05)).toBe(true);
    // clearly beyond
    expect(shouldSample(prev, { x: 0, y: 0, z: 0.2 }, 0.05)).toBe(true);
  });

  it('uses full 3-D euclidean distance, not per-axis', () => {
    const prev = { x: 0, y: 0, z: 0 };
    // each axis 0.03 (< 0.05) but combined ≈ 0.052 (> 0.05)
    expect(shouldSample(prev, { x: 0.03, y: 0.03, z: 0.03 }, 0.05)).toBe(true);
  });
});

describe('AgentTrail — pushSample / ring semantics', () => {
  it('grows count up to capacity, then holds', () => {
    const ring = createTrailRing(3);
    expect(ring.count).toBe(0);
    pushSample(ring, 1, 0, 0);
    expect(ring.count).toBe(1);
    pushSample(ring, 2, 0, 0);
    pushSample(ring, 3, 0, 0);
    expect(ring.count).toBe(3);
    // overwrites past capacity, count stays pinned
    pushSample(ring, 4, 0, 0);
    expect(ring.count).toBe(3);
  });

  it('advances head circularly', () => {
    const ring = createTrailRing(3);
    expect(ring.head).toBe(0);
    pushSample(ring, 1, 0, 0);
    expect(ring.head).toBe(1);
    pushSample(ring, 2, 0, 0);
    pushSample(ring, 3, 0, 0);
    expect(ring.head).toBe(0); // wrapped
    pushSample(ring, 4, 0, 0);
    expect(ring.head).toBe(1);
  });

  it('writes x,y,z interleaved at the head slot', () => {
    const ring = createTrailRing(2);
    pushSample(ring, 1, 2, 3);
    expect(Array.from(ring.positions.slice(0, 3))).toEqual([1, 2, 3]);
    pushSample(ring, 4, 5, 6);
    expect(Array.from(ring.positions.slice(3, 6))).toEqual([4, 5, 6]);
    // third push overwrites slot 0
    pushSample(ring, 7, 8, 9);
    expect(Array.from(ring.positions.slice(0, 3))).toEqual([7, 8, 9]);
  });
});

describe('AgentTrail — copyOrdered (oldest→newest)', () => {
  it('emits samples in insertion order before the buffer fills', () => {
    const ring = createTrailRing(4);
    pushSample(ring, 1, 0, 0);
    pushSample(ring, 2, 0, 0);
    const out = new Float32Array(12);
    const count = copyOrdered(ring, out);
    expect(count).toBe(2);
    expect(Array.from(out.slice(0, 6))).toEqual([1, 0, 0, 2, 0, 0]);
  });

  it('emits oldest→newest after wrap (tail decays by rotation)', () => {
    const ring = createTrailRing(3);
    // push 1,2,3,4,5 → ring holds the newest three: 3,4,5 (oldest→newest)
    for (const x of [1, 2, 3, 4, 5]) pushSample(ring, x, 0, 0);
    const out = new Float32Array(9);
    const count = copyOrdered(ring, out);
    expect(count).toBe(3);
    expect([out[0], out[3], out[6]]).toEqual([3, 4, 5]);
  });

  it('is empty for a fresh ring', () => {
    const ring = createTrailRing(4);
    const out = new Float32Array(12);
    expect(copyOrdered(ring, out)).toBe(0);
  });
});

describe('AgentTrail — trailFade envelope', () => {
  it('is 0 at the tail and 1 at the head, monotonic between', () => {
    expect(trailFade(0)).toBe(0);
    expect(trailFade(1)).toBe(1);
    expect(trailFade(-0.5)).toBe(0);
    expect(trailFade(1.5)).toBe(1);
    // quadratic ease: the tail half fades faster than linear (0.5 → 0.25)
    expect(trailFade(0.5)).toBeCloseTo(0.25, 6);
    expect(trailFade(0.25)).toBeLessThan(trailFade(0.75));
  });
});

describe('AgentTrail — length bounds', () => {
  it('exposes the slider bounds', () => {
    expect(TRAIL_MIN_LENGTH).toBe(8);
    expect(TRAIL_MAX_LENGTH).toBe(48);
  });
});
