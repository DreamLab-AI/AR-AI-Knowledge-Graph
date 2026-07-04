/**
 * Flattened, searchable index over every field in the registry. One entry per
 * field, carrying the reveal command id, testid, and the keyword set the command
 * palette fuzzy-matches (label words AND the dot-path).
 */
import { REGISTRY } from './settingsRegistry';
import { testIdFor } from './manifest';

export interface PaletteIndexEntry {
  /** command id: `reveal:setting-…` */
  id: string;
  groupId: string;
  groupLabel: string;
  key: string;
  label: string;
  path?: string;
  localKey?: string;
  action?: string;
  control: string;
  subgroup?: string;
  testid: string;
  keywords: string[];
}

export const PALETTE_INDEX: PaletteIndexEntry[] = REGISTRY.flatMap((g) =>
  g.fields.map((f) => {
    const testid = testIdFor(f, g.id);
    const keywords = [
      f.path ?? '',
      f.localKey ?? '',
      f.action ?? '',
      f.key,
      g.id,
      g.label,
      f.subgroup ?? '',
      ...f.label.toLowerCase().split(' '),
    ].filter(Boolean);
    return {
      id: `reveal:${testid}`,
      groupId: g.id,
      groupLabel: g.label,
      key: f.key,
      label: f.label,
      path: f.path,
      localKey: f.localKey,
      action: f.action,
      control: f.type,
      subgroup: f.subgroup,
      testid,
      keywords,
    };
  }),
);
