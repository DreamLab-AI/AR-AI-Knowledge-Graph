import { describe, it, expect } from 'vitest';
import { REGISTRY, ALL_FIELDS, ALL_PATHS, testIdFor } from '../settingsRegistry';
import { buildManifest } from '../manifest';
import { MACRO_PATHS } from '../macros';
import manifestJson from '../settings-manifest.json';
// The SOURCE OF TRUTH for the frozen backend contract. Every registry path must
// equal one of these, and vice versa — zero drift.
import { UNIFIED_SETTINGS_CONFIG } from '../../../visualisation/components/ControlPanel/unifiedSettingsConfig';

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

/** Collect every `path` string declared in the legacy unifiedSettingsConfig. */
function legacyPaths(): string[] {
  const out: string[] = [];
  for (const section of Object.values(UNIFIED_SETTINGS_CONFIG)) {
    for (const f of section.fields) {
      if (f.path) out.push(f.path);
    }
  }
  return out;
}

/** Every field (path or localKey/action) declared in the legacy config. */
function legacyFieldCount(): number {
  return Object.values(UNIFIED_SETTINGS_CONFIG).reduce((n, s) => n + s.fields.length, 0);
}

describe('control-center settings registry', () => {
  it('(a) enumerates exactly 168 fields', () => {
    expect(ALL_FIELDS.length).toBe(168);
    // and the legacy config it mirrors is also 168 (sanity on the source of truth)
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
});
