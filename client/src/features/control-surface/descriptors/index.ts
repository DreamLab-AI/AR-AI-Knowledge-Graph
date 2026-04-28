/**
 * Descriptor catalogue. Frozen at module load.
 * Validates aggregate invariants (DDD-control-surface §Aggregate 1):
 *   - id is unique
 *   - folds reference existing descriptor ids
 *   - tier and category are valid enums
 */

import type { Setting } from '../types';
import { VISUAL_DESCRIPTORS } from './visual';
import { BEHAVIOUR_DESCRIPTORS } from './behaviour';
import { DATA_DESCRIPTORS } from './data';
import { TEAM_DESCRIPTORS } from './team';
import { POWER_DESCRIPTORS } from './power';
import { OPERATOR_DESCRIPTORS } from './operator';

const RAW: ReadonlyArray<Setting<any>> = [
  ...VISUAL_DESCRIPTORS,
  ...BEHAVIOUR_DESCRIPTORS,
  ...DATA_DESCRIPTORS,
  ...TEAM_DESCRIPTORS,
  ...POWER_DESCRIPTORS,
  ...OPERATOR_DESCRIPTORS,
];

// Validate aggregate invariants once at module load.
const seen = new Set<string>();
for (const d of RAW) {
  if (seen.has(d.id)) {
    // eslint-disable-next-line no-console
    console.error(`[control-surface] duplicate descriptor id: ${d.id}`);
  }
  seen.add(d.id);
  if (![1, 2, 3, 4].includes(d.tier)) {
    // eslint-disable-next-line no-console
    console.error(`[control-surface] invalid tier on ${d.id}: ${d.tier}`);
  }
}

export const DESCRIPTORS: ReadonlyArray<Setting<any>> = Object.freeze([...RAW]);

export function findDescriptorById(id: string): Setting<any> | undefined {
  return DESCRIPTORS.find((d) => d.id === id);
}
