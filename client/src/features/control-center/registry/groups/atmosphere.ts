/**
 * Group 5 — Effects & Atmosphere (id `atmosphere`, hotkey 5, 22 fields).
 * All from the legacy Effects tab: WASM scene particles, wisps, fog, the
 * embedding point cloud, and node animations.
 */
import type { GroupData, RegistryField } from '../types';

const S = 'visualisation.sceneEffects.';
const E = 'visualisation.embeddingCloud.';
const A = 'visualisation.animations.';

const fields: RegistryField[] = [
  // Scene Particles
  { key: 'sceneEffectsEnabled', subgroup: 'Scene Particles', label: 'Scene Effects', type: 'toggle', path: `${S}enabled`, description: 'Enable WASM ambient effects', macro: 'atmosphere' },
  { key: 'particleCount', subgroup: 'Scene Particles', label: 'Particle Count', type: 'slider', min: 64, max: 512, step: 32, path: `${S}particleCount`, description: 'Number of ambient dust particles' },
  { key: 'particleOpacity', subgroup: 'Scene Particles', label: 'Particle Opacity', type: 'slider', min: 0, max: 1, step: 0.05, path: `${S}particleOpacity`, description: 'Brightness of ambient particles', macro: 'atmosphere' },
  { key: 'particleDrift', subgroup: 'Scene Particles', label: 'Particle Drift', type: 'slider', min: 0, max: 2, step: 0.1, path: `${S}particleDrift`, description: 'Drift speed of particles' },
  // Energy Wisps
  { key: 'wispsEnabled', subgroup: 'Energy Wisps', label: 'Energy Wisps', type: 'toggle', path: `${S}wispsEnabled`, description: 'Ephemeral glowing orbs that drift and fade' },
  { key: 'wispCount', subgroup: 'Energy Wisps', label: 'Wisp Count', type: 'slider', min: 8, max: 128, step: 8, path: `${S}wispCount`, description: 'Number of energy wisps' },
  { key: 'wispOpacity', subgroup: 'Energy Wisps', label: 'Wisp Opacity', type: 'slider', min: 0, max: 1, step: 0.05, path: `${S}wispOpacity`, description: 'Brightness of wisps', macro: 'atmosphere' },
  { key: 'wispDriftSpeed', subgroup: 'Energy Wisps', label: 'Wisp Speed', type: 'slider', min: 0, max: 3, step: 0.1, path: `${S}wispDriftSpeed`, description: 'How fast wisps drift' },
  // Atmosphere / Fog
  { key: 'fogEnabled', subgroup: 'Atmosphere / Fog', label: 'Atmosphere', type: 'toggle', path: `${S}fogEnabled`, description: 'Nebula background texture' },
  { key: 'fogOpacity', subgroup: 'Atmosphere / Fog', label: 'Atmosphere Opacity', type: 'slider', min: 0, max: 0.15, step: 0.01, path: `${S}fogOpacity`, description: 'Intensity of nebula background', macro: 'atmosphere' },
  { key: 'atmosphereResolution', subgroup: 'Atmosphere / Fog', label: 'Atmosphere Detail', type: 'slider', min: 64, max: 256, step: 32, path: `${S}atmosphereResolution`, description: 'Texture resolution (higher = more detail)' },
  // Embedding Cloud
  { key: 'embeddingCloudEnabled', subgroup: 'Embedding Cloud', label: 'Embedding Cloud', type: 'toggle', path: `${E}enabled`, description: 'Show RuVector embedding point cloud' },
  { key: 'embeddingCloudScale', subgroup: 'Embedding Cloud', label: 'Cloud Scale', type: 'slider', min: 0.5, max: 20, step: 0.5, path: `${E}cloudScale`, description: 'Overall scale of embedding cloud' },
  { key: 'embeddingPointSize', subgroup: 'Embedding Cloud', label: 'Point Size', type: 'slider', min: 0.5, max: 25, step: 0.5, path: `${E}pointSize`, description: 'Size of embedding points' },
  { key: 'embeddingOpacity', subgroup: 'Embedding Cloud', label: 'Cloud Opacity', type: 'slider', min: 0, max: 1, step: 0.05, path: `${E}opacity`, description: 'Transparency of embedding points' },
  { key: 'embeddingRotation', subgroup: 'Embedding Cloud', label: 'Rotation Speed', type: 'slider', min: 0, max: 0.005, step: 0.0001, path: `${E}rotationSpeed`, description: 'Auto-rotation speed' },
  // Animation
  { key: 'nodeAnimations', subgroup: 'Animation', label: 'Node Animations', type: 'toggle', path: `${A}enableNodeAnimations`, description: 'Enable node animations' },
  { key: 'pulseEnabled', subgroup: 'Animation', label: 'Pulse Effect', type: 'toggle', path: `${A}pulseEnabled`, description: 'Pulsing effect on nodes' },
  { key: 'pulseSpeed', subgroup: 'Animation', label: 'Pulse Speed', type: 'slider', min: 0.1, max: 2, step: 0.1, path: `${A}pulseSpeed`, description: 'Speed of pulse' },
  { key: 'pulseStrength', subgroup: 'Animation', label: 'Pulse Strength', type: 'slider', min: 0.1, max: 2, step: 0.1, path: `${A}pulseStrength`, description: 'Intensity of pulse' },
  { key: 'selectionWave', subgroup: 'Animation', label: 'Selection Wave', type: 'toggle', path: `${A}selectionWaveEnabled`, description: 'Wave effect on selection' },
  { key: 'waveSpeed', subgroup: 'Animation', label: 'Wave Speed', type: 'slider', min: 0.1, max: 2, step: 0.1, path: `${A}waveSpeed`, description: 'Speed of selection wave' },
];

export const atmosphere: GroupData = {
  id: 'atmosphere',
  label: 'Effects & Atmosphere',
  description: 'Ambient particles, energy wisps, nebula fog, the embedding cloud, and node animations.',
  hotkey: '5',
  loadPaths: ['visualisation.sceneEffects', 'visualisation.embeddingCloud', 'visualisation.animations'],
  fields,
};
