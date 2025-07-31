# Parallel Graphs Feature

This document provides comprehensive documentation for the parallel graphs feature that enables simultaneous visualization of multiple graph data sources with independent visual configurations.

## Overview

The parallel graphs feature allows the application to visualize multiple graph data sources simultaneously, each with its own independent visual theme and physics configuration. This enables side-by-side comparison and analysis of different knowledge graphs.

## Architecture

### Multi-Graph Settings Structure

The application now uses a nested settings structure to support multiple graphs:

```typescript
interface VisualisationSettings {
  // Graph-specific settings
  graphs: {
    logseq: GraphSettings;      // Blue/purple theme
    visionflow: GraphSettings;   // Green theme
  };
  
  // Global settings (shared across all graphs)
  rendering: RenderingSettings;
  animations: AnimationSettings;
  bloom: BloomSettings;
  hologram: HologramSettings;
  camera?: CameraSettings;
}
```

### Key Components

#### 1. Graph Manager
Each graph instance has its own Graph Manager that handles:
- Data loading and processing
- Physics simulation
- Position updates
- WebSocket communication

#### 2. Settings Store
The settings store maintains separate configurations for each graph:
- Independent visual themes
- Separate physics parameters
- Individual node/edge styling

#### 3. Viewport Renderer
The viewport can render multiple graphs simultaneously:
- Separate Three.js scenes or layers
- Independent camera controls per graph
- Shared or split viewport modes

## Implementation Details

### Graph Registration

Graphs are registered with unique identifiers:

```typescript
enum GraphType {
  LOGSEQ = 'logseq',
  VISIONFLOW = 'visionflow'
}

// Register a new graph
graphRegistry.register({
  id: GraphType.LOGSEQ,
  name: 'Logseq Knowledge Graph',
  defaultTheme: 'blue',
  dataSource: 'logseq-api'
});
```

### Theme Configuration

Each graph has its own visual theme:

```typescript
// Logseq theme (Blue/Purple)
const logseqTheme = {
  nodes: {
    baseColor: '#4B5EFF',
    metalness: 0.4,
    roughness: 0.6
  },
  edges: {
    color: '#F59E0B',
    glowStrength: 0.5
  }
};

// VisionFlow theme (Green)
const visionflowTheme = {
  nodes: {
    baseColor: '#10B981',
    metalness: 0.3,
    roughness: 0.7
  },
  edges: {
    color: '#34D399',
    glowStrength: 0.6
  }
};
```

### Data Isolation

Each graph maintains its own data structures:

```typescript
class GraphDataManager {
  private graphs: Map<GraphType, GraphData> = new Map();
  
  getGraphData(type: GraphType): GraphData {
    return this.graphs.get(type);
  }
  
  updateGraphData(type: GraphType, data: GraphData): void {
    this.graphs.set(type, data);
  }
}
```

## User Interface

### Graph Selector

Users can switch between graphs or view them simultaneously:

```typescript
interface GraphSelectorProps {
  activeGraphs: GraphType[];
  onGraphToggle: (graph: GraphType) => void;
  viewMode: 'single' | 'split' | 'overlay';
}
```

### Settings Panel

The settings panel provides graph-specific controls:

1. **Graph Tabs**: Switch between graph settings
2. **Theme Presets**: Quick theme selection
3. **Fine Controls**: Detailed customization per graph

### Viewport Modes

Three viewport modes are supported:

1. **Single View**: Display one graph at a time
2. **Split View**: Side-by-side comparison
3. **Overlay View**: Superimposed graphs with transparency

## Performance Considerations

### Resource Management

- **Memory**: Each graph maintains its own data structures
- **GPU**: Separate render passes for each graph
- **CPU**: Independent physics simulations

### Optimization Strategies

1. **Lazy Loading**: Load graph data on demand
2. **LOD System**: Level-of-detail based on viewport
3. **Culling**: Frustum culling per graph
4. **Instancing**: Share geometry between similar nodes

## API Integration

### Loading Multiple Graphs

```typescript
// Load Logseq graph
await graphManager.loadGraph({
  type: GraphType.LOGSEQ,
  source: '/api/logseq/graph',
  settings: settings.visualisation.graphs.logseq
});

// Load VisionFlow graph
await graphManager.loadGraph({
  type: GraphType.VISIONFLOW,
  source: '/api/visionflow/graph',
  settings: settings.visualisation.graphs.visionflow
});
```

### WebSocket Updates

Each graph can have its own WebSocket connection:

```typescript
// Logseq updates
wsManager.subscribe('logseq-updates', (data) => {
  graphManager.updatePositions(GraphType.LOGSEQ, data);
});

// VisionFlow updates
wsManager.subscribe('visionflow-updates', (data) => {
  graphManager.updatePositions(GraphType.VISIONFLOW, data);
});
```

## Migration Path

### From Single to Multi-Graph

1. **Settings Migration**: Automatic via `settingsMigration.ts`
2. **Component Updates**: Gradual migration of components
3. **API Updates**: Backend support for multiple data sources
4. **UI Updates**: New controls for graph selection

### Backward Compatibility

- Legacy single-graph mode remains functional
- Automatic migration of user settings
- Fallback to default graph if only one is available

## Future Enhancements

### Planned Features

1. **Dynamic Graph Addition**: Add new graph types at runtime
2. **Graph Merging**: Combine multiple graphs into one
3. **Cross-Graph Links**: Visualize relationships between graphs
4. **Graph Diffing**: Highlight differences between graphs

### Extensibility

The architecture supports:
- Custom graph types
- Plugin-based themes
- External data source integration
- Custom physics engines per graph

## Best Practices

### Development Guidelines

1. **Isolation**: Keep graph-specific code isolated
2. **Reusability**: Share common components where possible
3. **Performance**: Monitor resource usage per graph
4. **Testing**: Test each graph configuration independently

### User Experience

1. **Clear Indication**: Always show which graph is active
2. **Smooth Transitions**: Animate between graph switches
3. **Consistent Controls**: Maintain UI consistency across graphs
4. **Help Text**: Provide tooltips explaining graph differences

## Troubleshooting

### Common Issues

1. **Performance Degradation**: 
   - Check if both graphs are rendering
   - Verify LOD settings
   - Monitor memory usage

2. **Settings Not Applied**:
   - Ensure correct graph namespace
   - Check migration status
   - Verify settings persistence

3. **Data Loading Issues**:
   - Check API endpoints
   - Verify graph type configuration
   - Monitor WebSocket connections

### Debug Tools

```typescript
// Enable debug mode for specific graph
settings.visualisation.graphs.logseq.debug = true;

// Monitor graph performance
graphManager.getPerformanceStats(GraphType.LOGSEQ);

// Export graph state
graphManager.exportState(GraphType.VISIONFLOW);
```