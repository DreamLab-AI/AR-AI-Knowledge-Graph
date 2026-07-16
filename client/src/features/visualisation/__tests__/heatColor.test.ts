import { describe, it, expect } from 'vitest';
import { HEAT_BRIGHTEN_K, heatGain, heatBrightenFactor } from '../heatColor';

/** Apply the factor the way GemNodes does (zero-alloc scalar multiply). */
function bright(r: number, g: number, b: number, heat: number): [number, number, number] {
  const f = heatBrightenFactor(r, g, b, heat);
  return [r * f, g * f, b * f];
}

describe('heatColor brightening', () => {
  it('cold heat is the identity for any colour', () => {
    expect(heatBrightenFactor(0.8, 0.2, 0.2, 0)).toBe(1);
    const [r, g, b] = bright(0.8, 0.2, 0.2, 0);
    expect([r, g, b]).toEqual([0.8, 0.2, 0.2]);
  });

  it('heatGain matches (1 + heat*K) and floors negatives at the identity', () => {
    expect(heatGain(0)).toBe(1);
    expect(heatGain(0.5)).toBeCloseTo(1 + 0.5 * HEAT_BRIGHTEN_K, 6);
    expect(heatGain(-3)).toBe(1); // never darkens
  });

  it('brightens a dim colour by the full uncapped gain (headroom available)', () => {
    // maxc = 0.4, gain = 1.4 < 1/0.4 (=2.5) → full gain applies.
    const f = heatBrightenFactor(0.4, 0.1, 0.1, 0.5);
    expect(f).toBeCloseTo(1.4, 6);
    const [r, g, b] = bright(0.4, 0.1, 0.1, 0.5);
    expect(r).toBeGreaterThan(0.4); // measurably brighter
    // Ratio (hue + saturation) preserved exactly.
    expect(r / g).toBeCloseTo(4, 6);
    expect(g).toBeCloseTo(b, 6);
  });

  it('caps a bright colour so it never clips to white', () => {
    // Community-red at ~L0.5: maxc 0.8, hot heat → gain would clip.
    const f = heatBrightenFactor(0.8, 0.2, 0.2, 1);
    expect(f).toBeCloseTo(1 / 0.8, 6); // capped at max-channel headroom
    const [r, g, b] = bright(0.8, 0.2, 0.2, 1);
    expect(r).toBeCloseTo(1, 6);       // brightest channel hits 1.0…
    expect(g).toBeLessThan(0.4);       // …but the others stay well below it
    expect(r / g).toBeCloseTo(4, 6);   // still red, NOT white (ratio held)
    expect(r).toBeGreaterThan(0.8);    // and genuinely brighter
  });

  it('leaves an already-saturated primary untouched (stays that colour)', () => {
    const f = heatBrightenFactor(1, 0, 0, 1);
    expect(f).toBe(1); // pure red cannot brighten without desaturating → identity
    expect(bright(1, 0, 0, 1)).toEqual([1, 0, 0]);
  });

  it('brightening is monotonic non-decreasing in heat', () => {
    const r = 0.5, g = 0.3, b = 0.1;
    let prev = -Infinity;
    for (const h of [0, 0.2, 0.4, 0.6, 0.8, 1]) {
      const lum = r * heatBrightenFactor(r, g, b, h); // proxy: brightest-tracking channel
      expect(lum).toBeGreaterThanOrEqual(prev - 1e-9);
      prev = lum;
    }
  });

  it('black stays black (no divide-by-zero)', () => {
    expect(bright(0, 0, 0, 1)).toEqual([0, 0, 0]);
  });
});
