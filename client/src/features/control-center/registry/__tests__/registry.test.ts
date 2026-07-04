import { describe, it, expect } from 'vitest';
import { REGISTRY, ALL_FIELDS, ALL_PATHS, testIdFor } from '../settingsRegistry';
import { buildManifest } from '../manifest';
import { MACRO_PATHS } from '../macros';
import manifestJson from '../settings-manifest.json';
// The SOURCE OF TRUTH for the frozen backend contract. Captured from the legacy
// ControlPanel/unifiedSettingsConfig.ts (now deleted — WP5 cutover) before deletion.
// Every registry path must equal one of these, and vice versa — zero drift.
import legacyFixture from './legacy-paths.fixture.json';

const EXPECTED_GROUP_COUNTS: Record<string, number> = {
  motion: 48,
  look: 29,
  labels: 10,
  quality: 32,
  atmosphere: 22,
  xr: 5,
  ai: 6,
  system: 16,
};

/** The frozen `path` strings captured from the legacy unifiedSettingsConfig. */
function legacyPaths(): string[] {
  return legacyFixture.paths;
}

/** Every field (path or localKey/action) declared in the legacy config. */
function legacyFieldCount(): number {
  return legacyFixture.fieldCount;
}

describe('control-center settings registry', () => {
  it('(a) enumerates exactly 168 fields', () => {
    expect(ALL_FIELDS.length).toBe(168);
    // and the legacy config it mirrors is also 168 (sanity on the frozen fixture)
    expect(legacyFieldCount()).toBe(168);
  });

  it('(b) has the exact per-group field counts', () => {
    const actual: Record<string, number> = {};
    for (const g of REGISTRY) actual[g.id] = g.fields.length;
    expect(actual).toEqual(EXPECTED_GROUP_COUNTS);
    // group order realises hotkeys 1..8
    expect(REGISTRY.map((g) => g.id)).toEqual([
      'motion', 'look', 'labels', 'quality', 'atmosphere', 'xr', 'ai', 'system',
    ]);
    expect(REGISTRY.map((g) => g.hotkey)).toEqual(['1', '2', '3', '4', '5', '6', '7', '8']);
  });

  it('(c) has ZERO path drift vs legacy unifiedSettingsConfig (both directions)', () => {
    const registrySet = new Set(ALL_PATHS);
    const legacySet = new Set(legacyPaths());

    // No path exists in the registry that is absent from the frozen legacy set.
    const addedByRegistry = [...registrySet].filter((p) => !legacySet.has(p));
    // No frozen legacy path was dropped by the registry.
    const droppedFromLegacy = [...legacySet].filter((p) => !registrySet.has(p));

    expect(addedByRegistry).toEqual([]);
    expect(droppedFromLegacy).toEqual([]);
    // identical size ⇒ identical sets given the two empty diffs above
    expect(registrySet.size).toBe(legacySet.size);
    // no duplicate paths within the registry
    expect(ALL_PATHS.length).toBe(registrySet.size);
  });

  it('(d) every testid is unique', () => {
    const ids = REGISTRY.flatMap((g) => g.fields.map((f) => testIdFor(f, g.id)));
    expect(ids.length).toBe(168);
    expect(new Set(ids).size).toBe(168);
  });

  it('(e) manifest count matches the registry count', () => {
    const fresh = buildManifest();
    expect(fresh.count).toBe(ALL_FIELDS.length);
    expect(fresh.settings.length).toBe(168);
    // the committed JSON is in sync with the live registry (CI freshness)
    expect(manifestJson.count).toBe(fresh.count);
    expect(manifestJson.settings.length).toBe(fresh.settings.length);
    expect(manifestJson.groups.map((g) => `${g.id}:${g.fieldCount}`))
      .toEqual(fresh.groups.map((g) => `${g.id}:${g.fieldCount}`));
  });

  it('(f) every macro writes only to registered frozen paths (no macro drift)', () => {
    const paths = new Set(ALL_PATHS);
    const offRegistry = MACRO_PATHS.filter((p) => !paths.has(p));
    expect(offRegistry).toEqual([]);
  });

  it('(g) transient fields carry a localKey or action, never a path', () => {
    for (const f of ALL_FIELDS) {
      const isTransient = f.type === 'action-button' || f.localKey !== undefined;
      if (f.localKey !== undefined) expect(f.path).toBeUndefined();
      if (f.type === 'action-button') expect(f.path).toBeUndefined();
      // every field is addressable: it has a path, a localKey, or is an action
      expect(Boolean(f.path) || Boolean(f.localKey) || isTransient).toBe(true);
    }
  });

  // Every slider's range must be an exact whole number of steps: (max - min) / step
  // must be integral, otherwise the max (and any default sitting on it) is off the
  // step grid and silently snaps on first touch/restore — the defect-3 class of bug.
  // Known pre-existing off-grid sliders are whitelisted by path (out of defect-3
  // scope; documented here so future drift on the FIXED fields is still caught).
  const OFF_GRID_WHITELIST = new Set<string>([
    'visualisation.graphs.logseq.tweening.lerpBase',            // (0.15-0.0001)/0.001 = 149.9
    'visualisation.graphs.logseq.physics.maxForce',            // (2000-1)/5     = 399.8
    'visualisation.graphs.logseq.physics.constraintMaxForcePerNode', // (2000-1)/5 = 399.8
    'perplexity.maxTokens',                                    // (4096-100)/100 = 39.96
  ]);

  it('(h) every slider range is a whole number of steps (no step-grid drift)', () => {
    const offenders: string[] = [];
    for (const f of ALL_FIELDS) {
      if (f.type !== 'slider') continue;
      const id = f.path ?? f.localKey ?? f.key;
      expect(typeof f.min, `slider ${id} missing min`).toBe('number');
      expect(typeof f.max, `slider ${id} missing max`).toBe('number');
      expect(typeof f.step, `slider ${id} missing step`).toBe('number');
      if (OFF_GRID_WHITELIST.has(f.path ?? '')) continue;
      const steps = ((f.max as number) - (f.min as number)) / (f.step as number);
      if (Math.abs(Math.round(steps) - steps) > 1e-6) {
        offenders.push(`${id} → (max-min)/step = ${steps}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('(i) the defect-3 sliders now land exactly on their step grid', () => {
    const maxNodeCount = ALL_FIELDS.find((f) => f.path === 'qualityGates.maxNodeCount')!;
    expect(((maxNodeCount.max! - maxNodeCount.min!) / maxNodeCount.step!)).toBe(100);

    const outline = ALL_FIELDS.find((f) => f.path === 'visualisation.graphs.logseq.labels.textOutlineWidth')!;
    expect(outline.step).toBe(0.0001);
    // the 0.0074725277 stored default now snaps to 0.0075 (loss ~2.7e-5) not 0.007 (loss ~4.7e-4)
    const snapped = Math.round(0.0074725277 / outline.step!) * outline.step!;
    expect(Math.abs(snapped - 0.0074725277)).toBeLessThan(0.0001);
  });
});
