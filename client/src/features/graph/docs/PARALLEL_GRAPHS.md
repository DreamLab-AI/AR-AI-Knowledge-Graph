# Parallel Graph Implementation

This document describes the implementation of parallel graph support for running both Logseq and VisionFlow graphs simultaneously without data conflicts.

## Overview

The parallel graph system allows two independent graph visualizations to run at the same time:
- **Logseq Graph**: Displays knowledge graph data from Logseq
- **VisionFlow Graph**: Displays swarm agent visualization data

## Architecture

### Key Components

1. **Graph Type Identifiers**
   - Each data manager and worker has a `graphType` property
   - Types: `'logseq'` or `'visionflow'`
   - Prevents data mixing between graph systems

2. **Parallel Graph Coordinator** (`parallelGraphCoordinator.ts`)
   - Central service managing both graph systems
   - Handles enabling/disabling each graph independently
   - Maintains separate state for each graph type
   - Provides unified API for accessing both graphs

3. **Data Managers**
   - `graphDataManager.ts`: Handles Logseq data with graph type checking
   - `MCPWebSocketService.ts`: Handles VisionFlow data with data type filtering

4. **Physics Workers**
   - `graph.worker.ts`: Runs Logseq physics simulation
   - `swarmPhysicsWorker.ts`: Runs VisionFlow physics simulation
   - Both workers check graph type before processing

## Usage

### React Hook

```typescript
import { useParallelGraphs } from '@/features/graph/hooks/useParallelGraphs';

function MyComponent() {
  const {
    state,
    enableLogseq,
    enableVisionFlow,
    logseqPositions,
    visionFlowPositions
  } = useParallelGraphs({
    enableLogseq: true,
    enableVisionFlow: true
  });

  // Toggle graphs
  const toggleLogseq = () => enableLogseq(!state.logseq.enabled);
  const toggleVisionFlow = () => enableVisionFlow(!state.visionflow.enabled);

  // Access positions for rendering
  // logseqPositions: Map<nodeId, {x, y, z}>
  // visionFlowPositions: Map<agentId, {x, y, z}>
}
```

### Direct API Usage

```typescript
import { parallelGraphCoordinator } from '@/features/graph/services/parallelGraphCoordinator';

// Initialize the coordinator
await parallelGraphCoordinator.initialize();

// Enable/disable graphs
parallelGraphCoordinator.setLogseqEnabled(true);
parallelGraphCoordinator.setVisionFlowEnabled(true);

// Subscribe to state changes
const unsubscribe = parallelGraphCoordinator.onStateChange((state) => {
  console.log('Logseq nodes:', state.logseq.data?.nodes.length);
  console.log('VisionFlow agents:', state.visionflow.agents.length);
});

// Get positions
const logseqPos = await parallelGraphCoordinator.getLogseqPositions();
const visionFlowPos = parallelGraphCoordinator.getVisionFlowPositions();
```

## Data Flow

### Logseq Data Flow
1. WebSocketService receives binary position data
2. Checks if `graphDataManager.getGraphType() === 'logseq'`
3. Updates positions via `graphDataManager.updateNodePositions()`
4. Graph worker processes binary data with physics simulation
5. Positions available via `parallelGraphCoordinator.getLogseqPositions()`

### VisionFlow Data Flow
1. MCPWebSocketService receives MCP updates
2. Filters data with `dataType === 'visionflow'`
3. Updates agents and edges in `swarmPhysicsWorker`
4. Physics simulation runs independently
5. Positions available via `parallelGraphCoordinator.getVisionFlowPositions()`

## Key Features

1. **Independent Operation**
   - Each graph can be enabled/disabled independently
   - No data mixing between graph types
   - Separate physics simulations

2. **Type Safety**
   - Graph type checking at data ingestion points
   - TypeScript interfaces for graph-specific data

3. **Performance**
   - Workers run physics simulations off main thread
   - Binary data processing only for appropriate graph type
   - Efficient position updates

4. **Extensibility**
   - Easy to add new graph types
   - Modular architecture
   - Clear separation of concerns

## Configuration

Graph physics can be configured independently:

```typescript
// From graphTypes.ts
DEFAULT_GRAPH_CONFIG = {
  logseq: {
    physics: {
      springStrength: 0.2,
      damping: 0.95,
      maxVelocity: 0.02,
      // ...
    }
  },
  visionflow: {
    physics: {
      springStrength: 0.3,
      damping: 0.95,
      maxVelocity: 0.5,
      // ...
    }
  }
}
```

## Testing

Test files are provided in `tests/parallelGraphs.test.ts` to verify:
- Graph type configuration
- Independent enable/disable functionality
- State change notifications
- Position map independence

## Future Enhancements

1. Add more graph types (e.g., dependency graphs, flow charts)
2. Implement cross-graph linking/relationships
3. Add graph-specific rendering optimizations
4. Support for graph data persistence
5. Enhanced filtering and search across graph types