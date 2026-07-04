/**
 * Group 3 — Labels & Text (id `labels`, hotkey 3, 10 fields).
 * All from the legacy Graph tab. `labelLayoutEvery` keeps its `rendering.*` path
 * but lives here for user semantics (see spec §1.4).
 */
import type { GroupData, RegistryField } from '../types';

const L = 'visualisation.graphs.logseq.labels.';

const fields: RegistryField[] = [
  { key: 'enableLabels', subgroup: 'Labels', label: 'Show Labels', type: 'toggle', path: `${L}enableLabels`, description: 'Display node labels' },
  { key: 'labelSize', subgroup: 'Labels', label: 'Label Size', type: 'slider', min: 0.05, max: 3.0, step: 0.05, path: `${L}desktopFontSize`, description: 'Font size for labels', macro: 'focus' },
  { key: 'labelColor', subgroup: 'Labels', label: 'Label Color', type: 'color', path: `${L}textColor`, description: 'Color of label text' },
  { key: 'showMetadata', subgroup: 'Labels', label: 'Show Metadata', type: 'toggle', path: `${L}showMetadata`, description: 'Show domain, links, and quality info under labels' },
  { key: 'labelStandoff', subgroup: 'Labels', label: 'Label Standoff', type: 'slider', min: -1.0, max: 3.0, step: 0.05, path: `${L}textPadding`, description: 'Gap between node surface and label' },
  { key: 'labelOutlineColor', subgroup: 'Labels', label: 'Outline Color', type: 'color', path: `${L}textOutlineColor`, description: 'Label outline color' },
  { key: 'labelOutlineWidth', subgroup: 'Labels', label: 'Outline Width', type: 'slider', min: 0, max: 0.01, step: 0.001, path: `${L}textOutlineWidth`, description: 'Label outline width' },
  { key: 'labelDistanceThreshold', subgroup: 'Labels', label: 'Label Draw Distance', type: 'slider', min: 0, max: 2000, step: 25, path: `${L}labelDistanceThreshold`, description: 'Max camera distance for label visibility', macro: 'focus' },
  { key: 'maxLabelWidth', subgroup: 'Labels', label: 'Max Label Width', type: 'slider', min: 2, max: 20, step: 0.5, path: `${L}maxLabelWidth`, description: 'Maximum text wrapping width' },
  { key: 'labelLayoutEvery', subgroup: 'Labels', label: 'Label Layout Cadence (frames)', type: 'slider', min: 1, max: 10, step: 1, path: 'visualisation.rendering.labelLayoutEvery', description: 'Frames between full label re-layout passes' },
];

export const labels: GroupData = {
  id: 'labels',
  label: 'Labels & Text',
  description: 'Node label visibility, sizing, colour, outline, and layout cadence.',
  hotkey: '3',
  loadPaths: ['visualisation.graphs.logseq.labels', 'visualisation.rendering'],
  fields,
};
