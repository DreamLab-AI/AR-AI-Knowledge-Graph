# Hardcoded Values Migration Guide

## Overview

This document provides a complete mapping of all hardcoded values found in the visualization components and guides for migrating to the new centralized configuration system.

## Configuration Structure

### 1. Main Configuration File
- **Location**: `/src/config/visualization-config.ts`
- **Purpose**: Centralized storage of all visualization constants
- **Interface**: `VisualizationConfig`

### 2. Control Panel Configuration
- **Location**: `/src/config/control-panel-config.ts`
- **Purpose**: UI structure for dynamic value modification
- **Interface**: `ControlPanelConfig`

### 3. Provider & Hooks
- **Provider**: `/src/providers/VisualizationConfigProvider.tsx`
- **Hook**: `/src/hooks/useVisualizationValue.ts`

## Complete Hardcoded Values Mapping

### MainLayout.tsx
```typescript
// Camera
position: [0, 20, 60] → config.mainLayout.camera.position
fov: 75 → config.mainLayout.camera.fov
near: 0.1 → config.mainLayout.camera.near
far: 2000 → config.mainLayout.camera.far

// Scene
backgroundColor: '#000022' → config.mainLayout.scene.backgroundColor
backgroundColorRGB: [0, 0, 0.05] → config.mainLayout.scene.backgroundColorRGB

// Lighting
ambientLight intensity: 0.6 → config.mainLayout.lighting.ambientIntensity
directionalLight intensity: 0.8 → config.mainLayout.lighting.directionalIntensity
directionalLight position: [1, 1, 1] → config.mainLayout.lighting.directionalPosition

// Controls
zoomSpeed: 0.8 → config.mainLayout.controls.zoomSpeed
panSpeed: 0.8 → config.mainLayout.controls.panSpeed
rotateSpeed: 0.8 → config.mainLayout.controls.rotateSpeed
```

### BotsVisualization.tsx
```typescript
// Colors
Role colors (greens): #2ECC71-#239B56 → config.botsVisualization.colors.roles
Coordination colors (golds): #F1C40F-#D68910 → config.botsVisualization.colors.coordination
Health colors: #2ECC71/#F1C40F/#E67E22/#E74C3C → config.botsVisualization.colors.health

// Node properties
baseSize: 0.5 → config.botsVisualization.nodes.baseSize
workloadScale: 1.5 → config.botsVisualization.nodes.workloadScale
pulseSpeedFactor: 20 → config.botsVisualization.nodes.pulseSpeedFactor
metalness: 0.8 → config.botsVisualization.nodes.metalness
roughness: 0.2 → config.botsVisualization.nodes.roughness

// Edge properties
activityTimeout: 5000 → config.botsVisualization.edges.activityTimeout
particleSize: 0.4 → config.botsVisualization.edges.particleSize
particleColor: '#FFD700' → config.botsVisualization.edges.particleColor
cylinderRadius: 0.05 → config.botsVisualization.edges.cylinderRadius

// Particles
ambientCount: 200 → config.botsVisualization.particles.ambientCount
ambientSize: 0.05 → config.botsVisualization.particles.ambientSize
```

### GraphManager.tsx
```typescript
// Node colors
typeColors → config.graphManager.colors.nodeTypes

// Node sizing
baseSize: 1.0 → config.graphManager.nodes.baseSize
typeImportance → config.graphManager.nodes.typeImportance

// Material
baseColor: '#0066ff' → config.graphManager.material.baseColor
emissiveColor: '#00ffff' → config.graphManager.material.emissiveColor
opacity: 0.8 → config.graphManager.material.opacity
glowStrength: 3.0 → config.graphManager.material.glowStrength

// Positioning
goldenAngle → config.graphManager.positioning.goldenAngle
scaleFactor: 15-20 → config.graphManager.positioning.scaleFactorMin/Max
```

### Quest3ARLayout.tsx
```typescript
// Performance
defaultUpdateRate: 30 → config.quest3ARLayout.performance.defaultUpdateRate
quest3UpdateRate: 72 → config.quest3ARLayout.performance.quest3UpdateRate
maxRenderDistance: 100 → config.quest3ARLayout.performance.maxRenderDistance

// UI positioning
voiceControlsBottom: '40px' → config.quest3ARLayout.ui.voiceControlsBottom
All other UI positions → config.quest3ARLayout.ui.*
```

### SpacePilotSimpleIntegration.tsx
```typescript
// Controller config
translationSpeed: 1.0 → config.spacePilot.controller.translationSpeed
rotationSpeed: 0.1 → config.spacePilot.controller.rotationSpeed
deadzone: 0.02 → config.spacePilot.controller.deadzone
smoothing: 0.85 → config.spacePilot.controller.smoothing

// Camera reset
resetDistance: 50 → config.spacePilot.camera.resetDistance
resetTheta: Math.PI/4 → config.spacePilot.camera.resetTheta
```

### HologramManager.tsx
```typescript
// Defaults
size: 1 → config.hologramManager.defaults.size
color: '#00ffff' → config.hologramManager.defaults.color
opacity: 0.7 → config.hologramManager.defaults.opacity
sphereSizes: [40, 80] → config.hologramManager.defaults.sphereSizes

// Geometry
segments: 64/32 → config.hologramManager.defaults.segments/segmentsLow
```

### FlowingEdges.tsx
```typescript
// Material
defaultColor: '#56b6c2' → config.flowingEdges.material.defaultColor
opacity: 0.6 → config.flowingEdges.material.opacity
linewidth: 2 → config.flowingEdges.material.linewidth

// Animation
flowSpeed: 1.0 → config.flowingEdges.animation.flowSpeed
```

## Migration Examples

### Example 1: Migrating MainLayout.tsx

```typescript
// Before
<Canvas
  camera={{
    position: [0, 20, 60],
    fov: 75,
    near: 0.1,
    far: 2000
  }}
>

// After
import { useVisualizationSection } from '../hooks/useVisualizationValue';

const MainLayout = () => {
  const { camera } = useVisualizationSection('mainLayout');
  
  return (
    <Canvas
      camera={{
        position: camera.position,
        fov: camera.fov,
        near: camera.near,
        far: camera.far
      }}
    >
  );
};
```

### Example 2: Migrating Individual Values

```typescript
// Before
const baseSize = 0.5;
const color = '#00ffff';

// After
import { useVisualizationValue } from '../hooks/useVisualizationValue';

const Component = () => {
  const [baseSize] = useVisualizationValue('botsVisualization.nodes.baseSize', 0.5);
  const [color] = useVisualizationValue('hologramManager.defaults.color', '#00ffff');
};
```

### Example 3: Using with Control Panel

```typescript
import { useVisualizationValue } from '../hooks/useVisualizationValue';

const ControlSlider = ({ path, label }) => {
  const [value, setValue] = useVisualizationValue(path);
  
  return (
    <div>
      <label>{label}</label>
      <input
        type="range"
        value={value}
        onChange={(e) => setValue(parseFloat(e.target.value))}
      />
    </div>
  );
};
```

## Control Panel Integration

The control panel configuration provides:

1. **Organized Groups**: Camera, Scene, Bots, Graph, XR, SpacePilot, Hologram, Edges
2. **Control Types**: Slider, Color picker, Toggle, Select, Number input, Text, Vector3
3. **Validation**: Min/max ranges, type checking, custom validators
4. **Presets**: Default, Cyberpunk, Nature, Performance, XR Optimized

### Adding New Controls

```typescript
// In control-panel-config.ts
{
  type: 'slider',
  label: 'New Setting',
  path: 'section.subsection.newSetting',
  min: 0,
  max: 100,
  step: 1,
  defaultValue: 50,
  description: 'Description of what this controls'
}
```

## Benefits of Migration

1. **Centralized Configuration**: All values in one place
2. **Dynamic Modification**: Change values without recompiling
3. **Preset Support**: Quick theme/mode switching
4. **Validation**: Ensure values are within acceptable ranges
5. **Documentation**: Self-documenting configuration structure
6. **Type Safety**: Full TypeScript support
7. **Legacy Compatibility**: Works alongside existing settings system

## Next Steps

1. **Phase 1**: Implement provider in app root
2. **Phase 2**: Migrate components one by one
3. **Phase 3**: Build control panel UI
4. **Phase 4**: Add preset management
5. **Phase 5**: Deprecate legacy hardcoded values

## Notes

- The configuration system is designed to work alongside the existing settings store
- Legacy settings are automatically migrated when the provider loads
- All paths use dot notation for easy access
- Control definitions include metadata for building dynamic UIs
- Presets can be extended by users and saved/loaded