/**
 * Behaviour category descriptors. Physics, motion, simulation, layout.
 */

import type { Setting } from '../types';
import {
  BooleanEditor,
  makeNumberEditor,
  makeEnumEditor,
  makePresetEditor,
} from '../editors';

// ─── physics.enabled ────────────────────────────────────────────────────

export const physicsEnabled: Setting<boolean> = {
  id: 'physics.enabled',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'enabled'] as const,
  tier: 1,
  category: 'behaviour',
  label: 'Physics simulation',
  decision: 'KEEP',
  summary: (v) => (v ? 'Physics simulation: running' : 'Physics simulation: paused'),
  Editor: BooleanEditor,
  llm: { examples: ['pause physics', 'resume physics', 'freeze layout'] },
};

// ─── cluster.tightness (MERGE — currently centerGravityK alone, aspirational macro) ───

export const clusterTightness: Setting<number> = {
  id: 'cluster.tightness',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'centerGravityK'] as const,
  tier: 1,
  category: 'behaviour',
  label: 'Cluster tightness',
  decision: 'MERGE',
  ref: 'audit §4.18',
  summary: (v) => {
    const k = typeof v === 'number' ? v : 0;
    if (k <= 1) return 'Cluster tightness: loose';
    if (k <= 5) return 'Cluster tightness: normal';
    if (k <= 15) return 'Cluster tightness: tight';
    return `Cluster tightness: very tight (K=${k.toFixed(1)})`;
  },
  Editor: makeNumberEditor({ min: 0, max: 50, step: 0.1 }),
  llm: {
    bounds: { min: 0, max: 50, step: 0.1 },
    examples: ['tighter clusters', 'looser layout', 'spread out', 'pack tightly'],
    explainPrompt:
      'Centre gravity coefficient. Higher values pull all nodes more strongly toward their cluster centre.',
  },
};

// ─── boundary.feel (MERGE — folds boundary damping + extreme forces) ───

interface BoundaryFeel {
  enableBounds: boolean;
  boundsSize: number;
  boundaryDamping?: number;
  boundaryExtremeForce?: number;
}

const boundaryPresets: Record<string, Partial<BoundaryFeel>> = {
  Soft: { enableBounds: true, boundaryDamping: 0.6, boundaryExtremeForce: 0.5 },
  Standard: { enableBounds: true, boundaryDamping: 0.85, boundaryExtremeForce: 1.2 },
  Hard: { enableBounds: true, boundaryDamping: 0.95, boundaryExtremeForce: 2.5 },
  Off: { enableBounds: false },
};

function detectBoundaryPreset(v: BoundaryFeel): string {
  if (!v?.enableBounds) return 'Off';
  for (const [name, patch] of Object.entries(boundaryPresets)) {
    const matches = Object.entries(patch).every(
      ([k, val]) => Math.abs(((v as any)[k] ?? 0) - (val as any)) < 0.05 ||
        (v as any)[k] === val
    );
    if (matches) return name;
  }
  return 'custom';
}

export const boundaryFeel: Setting<BoundaryFeel> = {
  id: 'boundary.feel',
  path: ['visualisation', 'graphs', 'logseq', 'physics'] as const,
  tier: 1,
  category: 'behaviour',
  label: 'Boundary feel',
  decision: 'MERGE',
  ref: 'audit §4.30-34',
  folds: [
    'physics.enableBounds',
    'physics.boundsSize',
    'physics.boundaryDamping',
    'physics.boundaryExtremeForce',
  ],
  summary: (v) => {
    if (!v) return 'Boundary: unset';
    const preset = detectBoundaryPreset(v);
    if (preset === 'Off') return 'No boundary (graph free to drift)';
    return `Boundary feel: ${preset.toLowerCase()}`;
  },
  Editor: makePresetEditor<BoundaryFeel>({
    presets: boundaryPresets,
    detectPreset: detectBoundaryPreset,
  }),
  llm: {
    examples: ['softer boundary', 'hard wall', 'no boundary', 'standard'],
    explainPrompt:
      'Boundary feel folds the bounding-box on/off, damping at edges, and extreme-force kick when nodes try to escape.',
  },
};

// ─── physics.restLength ──────────────────────────────────────────────────

export const restLength: Setting<number> = {
  id: 'physics.restLength',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'restLength'] as const,
  tier: 2,
  category: 'behaviour',
  label: 'Edge rest length',
  decision: 'KEEP',
  summary: (v) => `Rest length: ${typeof v === 'number' ? v.toFixed(1) : '—'} units`,
  Editor: makeNumberEditor({ min: 1, max: 200, step: 0.5 }),
  llm: { bounds: { min: 1, max: 200 }, examples: ['longer edges', 'shorter edges'] },
};

// ─── physics.damping (tier-2) ────────────────────────────────────────────

export const physicsDamping: Setting<number> = {
  id: 'physics.damping',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'damping'] as const,
  tier: 2,
  category: 'behaviour',
  label: 'Motion damping',
  decision: 'KEEP',
  summary: (v) => `Motion damping: ${typeof v === 'number' ? v.toFixed(2) : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 1, step: 0.01 }),
  llm: { bounds: { min: 0, max: 1 }, examples: ['less bouncy', 'more responsive'] },
};

// ─── physics.springK ─────────────────────────────────────────────────────

export const springK: Setting<number> = {
  id: 'physics.springK',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'springK'] as const,
  tier: 2,
  category: 'behaviour',
  label: 'Edge tension',
  decision: 'KEEP',
  summary: (v) => `Edge tension: ${typeof v === 'number' ? v.toFixed(2) : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 1000, step: 0.1 }),
  llm: { bounds: { min: 0, max: 1000 } },
};

// ─── physics.repelK ──────────────────────────────────────────────────────

export const repelK: Setting<number> = {
  id: 'physics.repelK',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'repelK'] as const,
  tier: 2,
  category: 'behaviour',
  label: 'Node repulsion',
  decision: 'KEEP',
  summary: (v) => `Node repulsion: ${typeof v === 'number' ? v.toFixed(0) : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 50000, step: 10 }),
  llm: { bounds: { min: 0, max: 50000 } },
};

// ─── auto-pause (EXPOSE tier-2 — when settled) ──────────────────────────

interface AutoPauseValue {
  autoPauseEnabled: boolean;
  autoPauseThreshold: number;
}

export const autoPause: Setting<AutoPauseValue> = {
  id: 'physics.autoPause',
  path: ['visualisation', 'graphs', 'logseq', 'physics'] as const,
  tier: 2,
  category: 'behaviour',
  label: 'Auto-pause when settled',
  decision: 'EXPOSE',
  folds: ['physics.autoPauseEnabled', 'physics.autoPauseThreshold'],
  summary: (v) => {
    if (!v) return 'Auto-pause: unknown';
    if (!v.autoPauseEnabled) return 'Auto-pause: off';
    const t = (v.autoPauseThreshold ?? 0.04).toFixed(2);
    return `Auto-pause when settled (kinetic < ${t})`;
  },
  Editor: ({ value, onChange }) => {
    const v = value ?? { autoPauseEnabled: true, autoPauseThreshold: 0.04 };
    return (
      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={!!v.autoPauseEnabled}
            onChange={(e) => onChange({ ...v, autoPauseEnabled: e.target.checked })}
          />
          <span>Enable auto-pause</span>
        </label>
        <label className="block text-xs text-slate-600 dark:text-slate-300">
          Threshold (kinetic energy)
        </label>
        <input
          type="number"
          step="0.01"
          min="0.001"
          max="1"
          value={v.autoPauseThreshold ?? 0.04}
          onChange={(e) =>
            onChange({ ...v, autoPauseThreshold: Number(e.target.value) })
          }
          className="w-32 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm"
        />
      </div>
    );
  },
  llm: {
    examples: ['off', 'pause earlier', 'never auto-pause', 'pause when calm'],
    explainPrompt:
      'When kinetic energy drops below the threshold the simulation pauses to save GPU. Set high to pause early, set 0 to disable.',
  },
};

// ─── layout.algorithm (EXPOSE tier-3) ────────────────────────────────────

export const layoutAlgorithm: Setting<string> = {
  id: 'layout.algorithm',
  path: ['qualityGates', 'layoutMode'] as const,
  tier: 3,
  category: 'power',
  label: 'Layout algorithm',
  decision: 'MERGE',
  ref: 'audit §4.50 + disconnect #2',
  summary: (v) => `Layout algorithm: ${typeof v === 'string' ? v : 'force-directed'}`,
  Editor: makeEnumEditor<string>([
    { value: 'force-directed', label: 'Force-directed (default)' },
    { value: 'stress-min', label: 'Stress minimisation' },
    { value: 'grid', label: 'Grid' },
    { value: 'hierarchical', label: 'Hierarchical' },
  ]),
  llm: {
    examples: ['stress min', 'grid', 'force directed'],
    explainPrompt:
      'Layout strategy. Force-directed = spring physics. Stress-min = global error minimisation. Grid = uniform spacing. Hierarchical = DAG-style.',
  },
};

export const BEHAVIOUR_DESCRIPTORS: ReadonlyArray<Setting<any>> = [
  physicsEnabled,
  clusterTightness,
  boundaryFeel,
  restLength,
  physicsDamping,
  springK,
  repelK,
  autoPause,
  layoutAlgorithm,
];
