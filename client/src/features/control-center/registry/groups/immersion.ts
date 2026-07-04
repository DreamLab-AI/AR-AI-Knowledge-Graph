/**
 * Group 6 — Immersion (XR) (id `xr`, hotkey 6, 5 fields).
 * All from the legacy XR tab; local-only today, kept local-only.
 */
import type { GroupData, RegistryField } from '../types';

const fields: RegistryField[] = [
  { key: 'xrEnabled', subgroup: 'Core XR', label: 'XR Mode', type: 'toggle', path: 'xr.enabled', description: 'Enable XR features' },
  // options are the REAL stored enum values (settings.ts: 'low' | 'medium' | 'high',
  // boot default 'high'). The capitalized display label is derived by SettingSelect's
  // labelize() — storing the label here left the Radix value unmatched, so the trigger
  // showed the placeholder and no option carried aria-selected.
  { key: 'xrQuality', subgroup: 'Core XR', label: 'XR Quality', type: 'select', options: ['low', 'medium', 'high'], path: 'xr.quality', description: 'Rendering quality' },
  { key: 'xrRenderScale', subgroup: 'Core XR', label: 'XR Render Scale', type: 'slider', min: 0.5, max: 2, step: 0.1, path: 'xr.renderScale', description: 'Resolution scale' },
  { key: 'handTracking', subgroup: 'Hand & Haptics', label: 'Hand Tracking', type: 'toggle', path: 'xr.enableHandTracking', description: 'Enable hand input' },
  { key: 'enableHaptics', subgroup: 'Hand & Haptics', label: 'Haptics', type: 'toggle', path: 'xr.enableHaptics', description: 'Haptic feedback' },
];

export const immersion: GroupData = {
  id: 'xr',
  label: 'Immersion (XR)',
  description: 'VR/AR mode, render scale, hand tracking, and haptics.',
  hotkey: '6',
  loadPaths: ['xr'],
  fields,
};
