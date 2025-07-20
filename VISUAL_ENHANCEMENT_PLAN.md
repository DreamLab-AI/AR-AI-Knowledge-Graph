# Knowledge Graph Visual Enhancement Plan

## Current State Analysis

### Working Features
- Basic instanced mesh rendering with spheres
- GPU-accelerated force-directed layout
- Node dragging with server synchronization
- Basic edge rendering with Line component
- Settings integration (colors, opacity, metalness, roughness)
- Node labels with Billboard

### Visual Settings Structure
The settings are managed through a complex but well-structured system:
- `visualisation.nodes.*` - Node appearance settings
- `visualisation.edges.*` - Edge appearance settings
- `visualisation.animation.*` - Animation toggles
- `visualisation.rendering.*` - Rendering quality settings

### Missing/Non-functional Features
1. **Hologram effects** (`enableHologram` setting exists but not implemented)
2. **Flow effects on edges** (`enableFlowEffect` setting exists but not implemented)
3. **Node type differentiation** (`enableMetadataShape` setting exists but not implemented)
4. **Pulse animations** (`pulseEnabled` setting exists but not implemented)
5. **Bloom/glow effects** (settings exist but not connected)
6. **Gradient edges** (`useGradient` setting exists but not implemented)

## Enhancement Strategy

### Phase 1: Foundation Improvements
1. **Create enhanced node shader material** with hologram effects
2. **Implement node type visual differentiation** (shape/color by metadata)
3. **Add basic glow/emission effects** to nodes

### Phase 2: Edge Enhancements
1. **Implement animated flow effect** on edges
2. **Add gradient coloring** based on connection strength
3. **Create pulsing animation** for active data flow

### Phase 3: Atmospheric Effects
1. **Add particle system** for ambient atmosphere
2. **Implement depth-based fog** for visual hierarchy
3. **Create selection/hover effects** with ripples

### Phase 4: Advanced Features
1. **Dynamic LOD system** for performance
2. **Temporal trails** for moving nodes
3. **Connection strength visualization** through edge thickness

## Implementation Plan

### 1. Enhanced Node Material (Priority: HIGH)
- Custom shader with hologram scanlines
- Pulsing emission based on node activity
- Shape morphing based on node type

### 2. Animated Edge System (Priority: HIGH)
- Replace Line with custom edge renderer
- Implement flow particles along edges
- Add gradient support

### 3. Post-processing Pipeline (Priority: MEDIUM)
- Bloom effect for glowing elements
- Depth of field for focus
- Motion blur for smooth animations

### 4. Particle Effects (Priority: MEDIUM)
- Ambient floating particles
- Node spawn/despawn effects
- Selection ripples

## Technical Considerations

### Performance
- Use instancing for all repeated geometry
- Implement LOD system for large graphs
- Batch draw calls where possible
- Use GPU animations via shaders

### Settings Management
- Maintain backwards compatibility
- Use existing settings structure
- Add new settings incrementally
- Test synchronization carefully

### Visual Consistency
- Follow existing color schemes
- Maintain readability
- Balance effects with performance
- Support both AR and desktop contexts

## Next Steps
1. Start with hologram shader material
2. Test settings flow carefully
3. Implement features incrementally
4. Document each enhancement