/**
 * Legacy-shaped view of the registry for CommandInput.tsx.
 *
 * CommandInput::buildSettingsContext() iterates `Object.values(UNIFIED_SETTINGS_CONFIG)`
 * and reads `section.fields[].{path,label,type,min,max,step,description}`. RegistryField
 * is a superset of that shape, so this export is a drop-in replacement for the old
 * `ControlPanel/unifiedSettingsConfig`. WP3 re-points CommandInput's import here.
 */
import { GROUP_DATA } from './manifest';
import type { RegistryField } from './types';

export interface CompatSection {
  title: string;
  fields: RegistryField[];
}

export const UNIFIED_SETTINGS_CONFIG: Record<string, CompatSection> = Object.fromEntries(
  GROUP_DATA.map((g) => [g.id, { title: g.label, fields: g.fields }]),
);
