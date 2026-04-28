/**
 * Data category descriptors. Filters, quality gates, feature flags affecting data flow.
 */

import type { Setting } from '../types';
import { BooleanEditor, makeNumberEditor } from '../editors';

// ─── nodeFilter.enabled ─────────────────────────────────────────────────

export const nodeFilterEnabled: Setting<boolean> = {
  id: 'nodeFilter.enabled',
  path: ['visualisation', 'nodeFilter', 'enabled'] as const,
  tier: 1,
  category: 'data',
  label: 'Filter rules',
  decision: 'KEEP',
  summary: (v) => (v ? 'Filter rules: active' : 'Showing all nodes (no filter)'),
  Editor: BooleanEditor,
  llm: { examples: ['turn off filter', 'enable filter rules'] },
};

// ─── quality.gates (MERGE — folds gpuAcceleration, autoAdjust, fps, max nodes) ───

interface QualityGates {
  gpuAcceleration: boolean;
  autoAdjust: boolean;
  minFpsThreshold: number;
  maxNodeCount: number;
  showClusters?: boolean;
  showAnomalies?: boolean;
}

export const qualityGates: Setting<QualityGates> = {
  id: 'quality.gates',
  path: ['qualityGates'] as const,
  tier: 2,
  category: 'data',
  label: 'Quality gates',
  decision: 'MERGE',
  ref: 'audit §6.1-7',
  folds: [
    'qualityGates.gpuAcceleration',
    'qualityGates.autoAdjust',
    'qualityGates.minFpsThreshold',
    'qualityGates.maxNodeCount',
    'qualityGates.showClusters',
    'qualityGates.showAnomalies',
  ],
  summary: (v) => {
    if (!v) return 'Quality gates: unset';
    const parts: string[] = [];
    parts.push(v.gpuAcceleration ? 'GPU on' : 'CPU only');
    parts.push(v.autoAdjust ? 'auto-adjust' : 'fixed quality');
    parts.push(`fps≥${v.minFpsThreshold ?? 30}`);
    parts.push(`max ${v.maxNodeCount ?? 50000} nodes`);
    return `Quality gates: ${parts.join(', ')}`;
  },
  Editor: ({ value, onChange }) => {
    const v: QualityGates = value ?? {
      gpuAcceleration: true,
      autoAdjust: true,
      minFpsThreshold: 30,
      maxNodeCount: 50000,
    };
    return (
      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={!!v.gpuAcceleration}
            onChange={(e) => onChange({ ...v, gpuAcceleration: e.target.checked })}
          />
          <span>GPU acceleration</span>
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={!!v.autoAdjust}
            onChange={(e) => onChange({ ...v, autoAdjust: e.target.checked })}
          />
          <span>Auto-adjust quality based on framerate</span>
        </label>
        <label className="block text-xs text-slate-600 dark:text-slate-300 pt-1">
          Min framerate threshold (drop quality below)
        </label>
        <input
          type="number"
          min={5}
          max={120}
          value={v.minFpsThreshold ?? 30}
          onChange={(e) =>
            onChange({ ...v, minFpsThreshold: Number(e.target.value) })
          }
          className="w-24 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm"
        />
        <label className="block text-xs text-slate-600 dark:text-slate-300 pt-1">
          Max node count
        </label>
        <input
          type="number"
          min={100}
          max={100000}
          step={100}
          value={v.maxNodeCount ?? 50000}
          onChange={(e) =>
            onChange({ ...v, maxNodeCount: Number(e.target.value) })
          }
          className="w-32 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 py-1 text-sm"
        />
      </div>
    );
  },
  llm: {
    examples: ['target 60 fps', 'cap at 25k nodes', 'CPU only mode'],
    explainPrompt:
      'Quality gates control automatic quality reduction when framerate drops or graphs grow large.',
  },
};

// ─── feature_flags.gpu_clustering (EXPOSE tier-3) ───────────────────────

export const ffGpuClustering: Setting<boolean> = {
  id: 'feature_flags.gpu_clustering',
  path: ['feature_flags', 'gpu_clustering'] as const,
  tier: 3,
  category: 'power',
  label: 'GPU-accelerated clustering',
  decision: 'EXPOSE',
  ref: 'audit §15.7',
  summary: (v) =>
    v ? 'GPU clustering: enabled' : 'GPU clustering: off (CPU-only)',
  Editor: BooleanEditor,
  llm: { examples: ['enable GPU clustering', 'CPU only'] },
};

export const ffOntologyValidation: Setting<boolean> = {
  id: 'feature_flags.ontology_validation',
  path: ['feature_flags', 'ontology_validation'] as const,
  tier: 3,
  category: 'power',
  label: 'Ontology constraint validation',
  decision: 'EXPOSE',
  summary: (v) => (v ? 'Ontology validation: on' : 'Ontology validation: off'),
  Editor: BooleanEditor,
};

export const ffGpuAnomaly: Setting<boolean> = {
  id: 'feature_flags.gpu_anomaly_detection',
  path: ['feature_flags', 'gpu_anomaly_detection'] as const,
  tier: 3,
  category: 'power',
  label: 'GPU anomaly detection',
  decision: 'EXPOSE',
  summary: (v) => (v ? 'Anomaly detection: on' : 'Anomaly detection: off'),
  Editor: BooleanEditor,
};

export const ffStressMajorization: Setting<boolean> = {
  id: 'feature_flags.stress_majorization',
  path: ['feature_flags', 'stress_majorization'] as const,
  tier: 3,
  category: 'power',
  label: 'Stress majorisation layout',
  decision: 'EXPOSE',
  summary: (v) =>
    v ? 'Stress majorisation: enabled' : 'Stress majorisation: off',
  Editor: BooleanEditor,
};

export const ffSemanticConstraints: Setting<boolean> = {
  id: 'feature_flags.semantic_constraints',
  path: ['feature_flags', 'semantic_constraints'] as const,
  tier: 3,
  category: 'power',
  label: 'Semantic constraints',
  decision: 'EXPOSE',
  summary: (v) => (v ? 'Semantic constraints: applied' : 'Semantic constraints: off'),
  Editor: BooleanEditor,
};

// ─── Performance: max nodes / max velocity (EXPOSE tier-2) ──────────────

export const physicsMaxVelocity: Setting<number> = {
  id: 'physics.maxVelocity',
  path: ['visualisation', 'graphs', 'logseq', 'physics', 'maxVelocity'] as const,
  tier: 2,
  category: 'data',
  label: 'Max node velocity',
  decision: 'EXPOSE',
  summary: (v) =>
    `Max velocity: ${typeof v === 'number' ? v.toFixed(0) : '—'} units/sec`,
  Editor: makeNumberEditor({ min: 0, max: 10000, step: 10 }),
  llm: { bounds: { min: 0, max: 10000 } },
};

export const DATA_DESCRIPTORS: ReadonlyArray<Setting<any>> = [
  nodeFilterEnabled,
  qualityGates,
  ffGpuClustering,
  ffOntologyValidation,
  ffGpuAnomaly,
  ffStressMajorization,
  ffSemanticConstraints,
  physicsMaxVelocity,
];
