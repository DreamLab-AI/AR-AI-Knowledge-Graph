/**
 * Macro layer — 5 derived dials that write to existing FROZEN paths via a transfer
 * function over t ∈ [0..1], and read them back (`derive`) for their at-rest position.
 * No macro introduces a new path; every path here also appears as a RegistryField
 * (annotated `macro: '<id>'`). See spec §1.1.
 */
import type { LucideIcon } from 'lucide-react';
// @ts-ignore - Atom exists in lucide-react but its type export lags the runtime.
import { Atom } from 'lucide-react';
import { Sun, Wind, Focus, CloudFog } from 'lucide-react';
import type { MacroDef } from './types';

const clamp01 = (v: number): number => Math.max(0, Math.min(1, v));
const num = (get: (p: string) => unknown, path: string, fallback: number): number => {
  const v = get(path);
  return typeof v === 'number' && Number.isFinite(v) ? v : fallback;
};

const P = 'visualisation.graphs.logseq.physics.';
const L = 'visualisation.graphs.logseq.labels.';
const S = 'visualisation.sceneEffects.';

export const MACROS: MacroDef[] = [
  {
    id: 'density',
    label: 'Density',
    icon: Atom,
    apply: (t) => [
      { path: `${P}repelK`, value: 40 + t * 360 },        // 40 → 400
      { path: `${P}restLength`, value: 20 + t * 100 },     // 20 → 120
      { path: `${P}centerGravityK`, value: 0.4 - t * 0.3 }, // 0.40 → 0.10
    ],
    derive: (get) => clamp01((num(get, `${P}repelK`, 120) - 40) / 360),
  },
  {
    id: 'luminosity',
    label: 'Luminosity',
    icon: Sun,
    apply: (t) => [
      { path: 'visualisation.glow.enabled', value: t > 0 },
      { path: 'visualisation.glow.intensity', value: 1.5 * t },              // 0 → 1.5
      { path: 'visualisation.rendering.ambientLightIntensity', value: 0.2 + t * 1.2 }, // 0.2 → 1.4
      { path: 'visualisation.gemMaterial.emissiveIntensity', value: 1.2 * t }, // 0 → 1.2
    ],
    derive: (get) => clamp01(num(get, 'visualisation.glow.intensity', 0) / 1.5),
  },
  {
    id: 'motion',
    label: 'Motion',
    icon: Wind,
    apply: (t) => [
      { path: `${P}globalSpeed`, value: 0.05 + t * 1.95 }, // 0.05 → 2.0
      { path: `${P}damping`, value: 0.98 - t * 0.33 },     // 0.98 → 0.65 (inverse)
      { path: `${P}temperature`, value: 0.6 * t },         // 0 → 0.6
    ],
    derive: (get) => clamp01((num(get, `${P}globalSpeed`, 0.4) - 0.05) / 1.95),
  },
  {
    id: 'focus',
    // Depth-of-field label focus. Turning the dial UP tightens the field: the
    // label draw distance pulls IN (fewer, nearer labels) while the font grows
    // for readability. It deliberately does NOT touch nodeFilter.* — writing the
    // node filter on every drag tick re-ran useGraphFiltering over the whole
    // corpus and popped nodes (and their labels) in/out as minConnections
    // quantised and includeLinkedPages flipped at t=0.5, which was the "labels
    // cycle on/off" redraw thrash. The draw distance also never rises above the
    // 1200 shipped default, so max focus can't flood the scene with every label.
    label: 'Focus',
    icon: Focus,
    apply: (t) => [
      { path: `${L}labelDistanceThreshold`, value: 1200 - t * 900 },      // 1200 → 300 (tightens)
      { path: `${L}desktopFontSize`, value: 0.25 + t * 0.35 },            // 0.25 → 0.6
    ],
    // Derive from the same continuous primary param the dial writes (as every
    // other macro does) — not the old Math.round(minConnections) read, whose
    // 5-step quantisation made the dial position snap and never match the drag.
    derive: (get) => clamp01((1200 - num(get, `${L}labelDistanceThreshold`, 1200)) / 900),
  },
  {
    id: 'atmosphere',
    label: 'Atmosphere',
    icon: CloudFog,
    apply: (t) => [
      { path: `${S}enabled`, value: t > 0 },
      { path: `${S}particleOpacity`, value: 0.8 * t }, // 0 → 0.8
      { path: `${S}wispOpacity`, value: 0.8 * t },     // 0 → 0.8
      { path: `${S}fogOpacity`, value: 0.12 * t },     // 0 → 0.12
    ],
    derive: (get) => clamp01(num(get, `${S}particleOpacity`, 0) / 0.8),
  },
];

export const MACROS_BY_ID: Record<string, MacroDef> = Object.fromEntries(
  MACROS.map((m) => [m.id, m]),
);

/** Every path any macro writes to — used by tests to prove macros never drift off-registry. */
export const MACRO_PATHS: string[] = Array.from(
  new Set(MACROS.flatMap((m) => m.apply(0.5).map((w) => w.path))),
);
