/**
 * Visual category descriptors. Highest leverage for MERGEs.
 * See PRD-007 §11 / aspirational-inventory.md visual section.
 */

import React from 'react';
import type { Setting } from '../types';
import {
  BooleanEditor,
  ColorEditor,
  makeNumberEditor,
  makeEnumEditor,
  makePresetEditor,
} from '../editors';

// ─── render.quality (MERGE parent — folds aa, shadows, ao, envIntensity) ───

interface RenderQualityValue {
  enableAntialiasing: boolean;
  enableShadows: boolean;
  enableAmbientOcclusion: boolean;
  environmentIntensity: number;
}

const renderQualityPresets: Record<string, Partial<RenderQualityValue>> = {
  Lite: {
    enableAntialiasing: false,
    enableShadows: false,
    enableAmbientOcclusion: false,
    environmentIntensity: 0.5,
  },
  Standard: {
    enableAntialiasing: true,
    enableShadows: true,
    enableAmbientOcclusion: false,
    environmentIntensity: 1.0,
  },
  High: {
    enableAntialiasing: true,
    enableShadows: true,
    enableAmbientOcclusion: true,
    environmentIntensity: 1.4,
  },
};

function detectRenderQualityPreset(v: RenderQualityValue): string {
  for (const [name, patch] of Object.entries(renderQualityPresets)) {
    const matches = Object.entries(patch).every(
      ([k, val]) => (v as any)[k] === val
    );
    if (matches) return name;
  }
  return 'custom';
}

export const renderQuality: Setting<RenderQualityValue> = {
  id: 'render.quality',
  path: ['visualisation', 'rendering'] as const,
  tier: 1,
  category: 'visual',
  label: 'Render quality',
  decision: 'MERGE',
  ref: 'audit §3.4-6',
  folds: ['render.aa', 'render.shadows', 'render.ao', 'render.envIntensity'],
  summary: (v) => {
    if (!v) return 'Render quality: unset';
    const preset = detectRenderQualityPreset(v);
    if (preset === 'custom') {
      const aa = v.enableAntialiasing ? 'on' : 'off';
      const sh = v.enableShadows ? 'on' : 'off';
      const ao = v.enableAmbientOcclusion ? 'on' : 'off';
      return `Render at custom quality (AA ${aa}, shadows ${sh}, AO ${ao})`;
    }
    return `Render at ${preset} quality`;
  },
  Editor: makePresetEditor<RenderQualityValue>({
    presets: renderQualityPresets,
    detectPreset: detectRenderQualityPreset,
  }),
  llm: {
    examples: ['lite', 'standard', 'high', 'turn off shadows', 'maximum quality'],
    explainPrompt:
      'Render quality folds antialiasing, shadows, ambient occlusion, and environment-light intensity into a single preset.',
  },
};

// ─── glow (MERGE — folds glow.color, glow.intensity, bloom.intensity, bloom.radius, bloom.threshold) ───

interface GlowValue {
  baseColor: string;
  intensity: number;
  enableBloom?: boolean;
  bloomIntensity?: number;
  bloomRadius?: number;
  bloomThreshold?: number;
}

export const glow: Setting<GlowValue> = {
  id: 'glow',
  path: ['visualisation', 'glow'] as const,
  tier: 1,
  category: 'visual',
  label: 'Glow & bloom',
  decision: 'MERGE',
  ref: 'audit §3.7-12',
  folds: ['glow.color', 'glow.intensity', 'bloom.intensity', 'bloom.radius', 'bloom.threshold'],
  summary: (v) => {
    if (!v) return 'Glow: unset';
    const i = (v.intensity ?? 0).toFixed(2);
    const bloom = v.enableBloom ? `bloom ${(v.bloomIntensity ?? 0).toFixed(2)}` : 'no bloom';
    return `Glow ${i} (${v.baseColor ?? '—'}, ${bloom})`;
  },
  Editor: ({ value, onChange, descriptor }) => (
    <div className="space-y-2">
      <label className="block text-xs text-slate-600 dark:text-slate-300">Base colour</label>
      <ColorEditor
        value={value?.baseColor ?? '#ffffff'}
        onChange={(c) => onChange({ ...(value ?? {} as GlowValue), baseColor: c })}
        context={{} as any}
        descriptor={descriptor as Setting<string>}
      />
      <label className="block text-xs text-slate-600 dark:text-slate-300 mt-2">Intensity</label>
      {React.createElement(makeNumberEditor({ min: 0, max: 5, step: 0.05 }), {
        value: value?.intensity ?? 1,
        onChange: (n: number) => onChange({ ...(value ?? {} as GlowValue), intensity: n }),
        context: {} as any,
        descriptor: descriptor as Setting<number>,
      })}
    </div>
  ),
  llm: {
    examples: ['warmer glow', 'no bloom', 'subtle glow', 'electric blue', 'turn glow off'],
    explainPrompt:
      'Glow controls the soft halo around nodes and bloom adds a post-processing bright bleed.',
  },
};

// ─── node visibility (MERGE — folds knowledge / ontology / agent toggles) ───

interface NodeVisibility {
  knowledge: boolean;
  ontology: boolean;
  agent: boolean;
}

export const nodeVisibility: Setting<NodeVisibility> = {
  id: 'node.visibility',
  path: ['visualisation', 'nodes', 'typeVisibility'] as const,
  tier: 1,
  category: 'visual',
  label: 'Node visibility',
  decision: 'MERGE',
  ref: 'audit §3.13',
  folds: ['nodeTypeVisibility.knowledge', 'nodeTypeVisibility.ontology', 'nodeTypeVisibility.agent'],
  summary: (v) => {
    if (!v) return 'Showing all nodes';
    const on = [
      v.knowledge && 'knowledge',
      v.ontology && 'ontology',
      v.agent && 'agent',
    ].filter(Boolean) as string[];
    if (on.length === 3) return 'Showing all nodes';
    if (on.length === 0) return 'Hiding all nodes';
    return `Showing ${on.join(' + ')} nodes only`;
  },
  Editor: ({ value, onChange, descriptor }) => {
    const v = value ?? { knowledge: true, ontology: true, agent: true };
    return (
      <div className="space-y-1">
        {(['knowledge', 'ontology', 'agent'] as const).map((k) => (
          <label key={k} className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={!!v[k]}
              onChange={(e) => onChange({ ...v, [k]: e.target.checked })}
              disabled={descriptor.readOnly}
            />
            <span className="capitalize">{k}</span>
          </label>
        ))}
      </div>
    );
  },
  llm: {
    examples: ['only knowledge', 'hide ontology', 'show all', 'just agents'],
    explainPrompt:
      'Toggle which node types are rendered. Hidden types are still in the graph and edges to them remain.',
  },
};

// ─── nodes.baseColor ─────────────────────────────────────────────────────

export const nodeBaseColor: Setting<string> = {
  id: 'nodes.baseColor',
  path: ['visualisation', 'nodes', 'baseColor'] as const,
  tier: 1,
  category: 'visual',
  label: 'Node colour',
  decision: 'KEEP',
  summary: (v) => `Node colour: ${v ?? '—'}`,
  Editor: ColorEditor,
  llm: {
    examples: ['change to teal', 'red nodes', 'reset to default'],
    explainPrompt: 'Base colour applied to nodes that have no per-node tint.',
  },
};

// ─── nodes.nodeSize ──────────────────────────────────────────────────────

export const nodeSize: Setting<number> = {
  id: 'nodes.nodeSize',
  path: ['visualisation', 'nodes', 'nodeSize'] as const,
  tier: 1,
  category: 'visual',
  label: 'Node size',
  decision: 'KEEP',
  summary: (v) => `Node size: ${typeof v === 'number' ? v.toFixed(2) : '—'}`,
  Editor: makeNumberEditor({ min: 0.1, max: 10, step: 0.05 }),
  llm: {
    bounds: { min: 0.1, max: 10, step: 0.05 },
    examples: ['bigger nodes', 'tiny nodes', 'medium', 'huge'],
    explainPrompt: 'Visual scale multiplier for every node.',
  },
};

// ─── nodes.opacity ───────────────────────────────────────────────────────

export const nodeOpacity: Setting<number> = {
  id: 'nodes.opacity',
  path: ['visualisation', 'nodes', 'opacity'] as const,
  tier: 1,
  category: 'visual',
  label: 'Node opacity',
  decision: 'KEEP',
  summary: (v) =>
    `Node opacity: ${typeof v === 'number' ? `${Math.round(v * 100)}%` : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 1, step: 0.05 }),
  llm: {
    bounds: { min: 0, max: 1 },
    examples: ['ghosted nodes', 'fully solid', 'half-transparent'],
  },
};

// ─── edges.color ─────────────────────────────────────────────────────────

export const edgeColor: Setting<string> = {
  id: 'edges.color',
  path: ['visualisation', 'edges', 'color'] as const,
  tier: 1,
  category: 'visual',
  label: 'Edge colour',
  decision: 'KEEP',
  summary: (v) => `Edge colour: ${v ?? '—'}`,
  Editor: ColorEditor,
  llm: { examples: ['warm edges', 'cool edges'] },
};

// ─── edges.baseWidth ─────────────────────────────────────────────────────

export const edgeWidth: Setting<number> = {
  id: 'edges.baseWidth',
  path: ['visualisation', 'edges', 'baseWidth'] as const,
  tier: 1,
  category: 'visual',
  label: 'Edge thickness',
  decision: 'KEEP',
  summary: (v) =>
    `Edge thickness: ${typeof v === 'number' ? v.toFixed(3) : '—'}`,
  Editor: makeNumberEditor({ min: 0.001, max: 0.5, step: 0.001 }),
  llm: { bounds: { min: 0.001, max: 0.5 }, examples: ['thicker edges', 'hair-thin edges'] },
};

// ─── edges.enableArrows ──────────────────────────────────────────────────

export const edgeArrows: Setting<boolean> = {
  id: 'edges.enableArrows',
  path: ['visualisation', 'edges', 'enableArrows'] as const,
  tier: 1,
  category: 'visual',
  label: 'Edge direction arrows',
  decision: 'KEEP',
  summary: (v) => (v ? 'Edges show direction arrows' : 'Edges have no arrows'),
  Editor: BooleanEditor,
  llm: { examples: ['show arrows', 'hide arrows'] },
};

// ─── labels.enableLabels ─────────────────────────────────────────────────

export const labelsEnabled: Setting<boolean> = {
  id: 'labels.enableLabels',
  path: ['visualisation', 'labels', 'enableLabels'] as const,
  tier: 1,
  category: 'visual',
  label: 'Node labels',
  decision: 'KEEP',
  summary: (v) => (v ? 'Showing node labels' : 'Hiding node labels'),
  Editor: BooleanEditor,
  llm: { examples: ['hide labels', 'show labels'] },
};

// ─── labels.desktopFontSize ──────────────────────────────────────────────

export const labelFontSize: Setting<number> = {
  id: 'labels.desktopFontSize',
  path: ['visualisation', 'labels', 'desktopFontSize'] as const,
  tier: 1,
  category: 'visual',
  label: 'Label size',
  decision: 'KEEP',
  summary: (v) =>
    `Label size: ${typeof v === 'number' ? v.toFixed(2) : '—'} em`,
  Editor: makeNumberEditor({ min: 0.05, max: 3, step: 0.01 }),
  llm: { bounds: { min: 0.05, max: 3 }, examples: ['bigger labels', 'tiny labels'] },
};

// ─── labels.textColor ────────────────────────────────────────────────────

export const labelColor: Setting<string> = {
  id: 'labels.textColor',
  path: ['visualisation', 'labels', 'textColor'] as const,
  tier: 1,
  category: 'visual',
  label: 'Label colour',
  decision: 'KEEP',
  summary: (v) => `Label colour: ${v ?? '—'}`,
  Editor: ColorEditor,
  llm: { examples: ['white labels', 'high contrast labels'] },
};

// ─── rendering.ambientLightIntensity ─────────────────────────────────────

export const ambientLight: Setting<number> = {
  id: 'rendering.ambientLight',
  path: ['visualisation', 'rendering', 'ambientLightIntensity'] as const,
  tier: 1,
  category: 'visual',
  label: 'Ambient light',
  decision: 'KEEP',
  summary: (v) => `Ambient light: ${typeof v === 'number' ? v.toFixed(2) : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 2, step: 0.05 }),
  llm: { bounds: { min: 0, max: 2 } },
};

// ─── theme (EXPOSE, tier-1 user pref) ────────────────────────────────────

export const themeChoice: Setting<'auto' | 'light' | 'dark'> = {
  id: 'user.theme',
  path: ['user_preferences', 'theme'] as const,
  tier: 1,
  category: 'visual',
  label: 'UI theme',
  decision: 'EXPOSE',
  ref: 'audit §15.1',
  summary: (v) =>
    v === 'dark'
      ? 'Dark theme'
      : v === 'light'
        ? 'Light theme'
        : 'Theme: follow system',
  Editor: makeEnumEditor<'auto' | 'light' | 'dark'>([
    { value: 'auto', label: 'Auto (system)' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ]),
  llm: { examples: ['dark mode', 'light mode', 'follow system'] },
};

// ─── tier-2 EXPOSE: nodes.metalness ───────────────────────────────────────

export const nodeMetalness: Setting<number> = {
  id: 'nodes.metalness',
  path: ['visualisation', 'nodes', 'metalness'] as const,
  tier: 2,
  category: 'visual',
  label: 'Node metalness',
  decision: 'EXPOSE',
  summary: (v) => `Node metalness: ${typeof v === 'number' ? v.toFixed(2) : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 1, step: 0.05 }),
  llm: { bounds: { min: 0, max: 1 }, explainPrompt: 'Material metalness — 0 is fully diffuse, 1 is fully metallic.' },
};

export const nodeRoughness: Setting<number> = {
  id: 'nodes.roughness',
  path: ['visualisation', 'nodes', 'roughness'] as const,
  tier: 2,
  category: 'visual',
  label: 'Node roughness',
  decision: 'EXPOSE',
  summary: (v) => `Node roughness: ${typeof v === 'number' ? v.toFixed(2) : '—'}`,
  Editor: makeNumberEditor({ min: 0, max: 1, step: 0.05 }),
  llm: { bounds: { min: 0, max: 1 } },
};

export const cameraFov: Setting<number> = {
  id: 'camera.fov',
  path: ['visualisation', 'camera', 'fov'] as const,
  tier: 2,
  category: 'visual',
  label: 'Camera field of view',
  decision: 'EXPOSE',
  summary: (v) => `FOV: ${typeof v === 'number' ? `${v}°` : '—'}`,
  Editor: makeNumberEditor({ min: 30, max: 120, step: 1 }),
  llm: { bounds: { min: 30, max: 120 }, examples: ['wider view', 'narrower view'] },
};

export const VISUAL_DESCRIPTORS: ReadonlyArray<Setting<any>> = [
  renderQuality,
  glow,
  nodeVisibility,
  nodeBaseColor,
  nodeSize,
  nodeOpacity,
  edgeColor,
  edgeWidth,
  edgeArrows,
  labelsEnabled,
  labelFontSize,
  labelColor,
  ambientLight,
  themeChoice,
  nodeMetalness,
  nodeRoughness,
  cameraFov,
];
