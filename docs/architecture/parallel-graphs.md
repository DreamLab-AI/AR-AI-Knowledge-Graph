# Parallel Graph Architecture

This document describes the parallel graph architecture that enables LogseqXR to run multiple independent graph visualizations simultaneously without data conflicts.

## Overview

The parallel graph system allows two (or more) independent graph visualizations to coexist:
- **Logseq Graph**: Knowledge graph data from Logseq (nodes and edges from pages/blocks)
- **VisionFlow Graph**: AI agent visualization data from Claude Flow MCP integration

## Architecture Components

### 1. Parallel Graph Coordinator (`parallelGraphCoordinator.ts`)

The central orchestrator managing multiple graph systems:

```typescript
class ParallelGraphCoordinator {
  private state: ParallelGraphState = {
    logseq: { enabled: boolean, data: GraphData, lastUpdate: number },
    visionflow: { enabled: boolean, agents: BotsAgent[], edges: BotsEdge[], ... }
  };
}
```

**Key Responsibilities:**
- Maintains separate state for each graph type
- Coordinates data flow between different sources
- Manages graph enable/disable states
- Provides unified API for position queries

### 2. Graph Type System

Each component in the system has a `graphType` property to prevent data mixing:

```typescript
type GraphType = 'logseq' | 'visionflow';

// Data managers check graph type before processing
if (graphDataManager.getGraphType() === 'logseq') {
  // Process Logseq-specific data
}
```

### 3. Separate Physics Workers

Each graph runs its own physics simulation in a dedicated web worker:

- **`graph.worker.ts`**: Handles Logseq graph physics
- **`botsPhysicsWorker.ts`**: Handles VisionFlow/bots physics

This separation ensures:
- No performance interference between graphs
- Independent physics configurations
- Parallel computation on multi-core systems

### 4. Data Flow Architecture

#### Logseq Data Flow
```
WebSocketService (binary protocol)
    ↓
graphDataManager (type check: 'logseq')
    ↓
graph.worker.ts (physics simulation)
    ↓
parallelGraphCoordinator.getLogseqPositions()
    ↓
3D Rendering
```

#### VisionFlow Data Flow
```
Backend ClaudeFlowActor (MCP via stdio)
    ↓
REST API (/api/bots/agents)
    ↓
MCPWebSocketService (type: 'visionflow')
    ↓
botsPhysicsWorker.ts (physics simulation)
    ↓
parallelGraphCoordinator.getVisionFlowPositions()
    ↓
3D Rendering
```

## React Hook API

The `useParallelGraphs` hook provides a clean interface for components:

```typescript
const {
  state,                    // Current state of both graphs
  isLogseqEnabled,         // Logseq graph status
  isVisionFlowEnabled,     // VisionFlow graph status
  enableLogseq,            // Toggle Logseq graph
  enableVisionFlow,        // Toggle VisionFlow graph
  logseqPositions,         // Map<nodeId, {x,y,z}>
  visionFlowPositions,     // Map<agentId, {x,y,z}>
  refreshPositions         // Manual position refresh
} = useParallelGraphs({
  enableLogseq: true,
  enableVisionFlow: true,
  autoConnect: true
});
```

## Configuration

Each graph type has independent physics configuration:

```typescript
const DEFAULT_GRAPH_CONFIG = {
  logseq: {
    physics: {
      springStrength: 0.2,
      damping: 0.95,
      maxVelocity: 0.02,
      centerForce: 0.001,
      repulsionStrength: 0.5,
      linkDistance: 2.0
    }
  },
  visionflow: {
    physics: {
      springStrength: 0.3,
      damping: 0.95,
      maxVelocity: 0.5,
      centerForce: 0.002,
      repulsionStrength: 0.8,
      linkDistance: 3.0
    }
  }
};
```

## Key Features

### 1. Independent Operation
- Each graph can be enabled/disabled without affecting the other
- Separate data sources and update mechanisms
- No shared state between graph types

### 2. Type Safety
- TypeScript interfaces ensure type safety at compile time
- Runtime graph type checking prevents data corruption
- Strongly typed position maps for each graph

### 3. Performance Optimizations
- Web Workers keep physics calculations off the main thread
- Binary protocol for efficient Logseq position updates
- Differential updates only send changed positions
- Independent update rates for each graph

### 4. Extensibility
The architecture supports adding new graph types:

```typescript
// Future graph types could include:
type GraphType = 'logseq' | 'visionflow' | 'dependency' | 'flowchart';
```

## Implementation Details

### WebSocket Handling

The system uses different WebSocket connections:
- **Main WebSocket** (`/wss`): Logseq graph data using binary protocol
- **MCP Updates**: VisionFlow data via backend REST API (no direct WebSocket)

### Binary Protocol Integration

Logseq positions use an optimized binary protocol:
- 28 bytes per node (ID, position, velocity)
- Compression for messages > 1KB
- Processed in web worker to avoid blocking UI

### Memory Management

- Position maps are updated incrementally
- Old positions are garbage collected
- Workers manage their own memory pools

## Testing

The parallel graph system includes comprehensive tests:
- Graph type isolation tests
- Independent enable/disable verification
- State change notification tests
- Position map independence checks

## Migration Notes

When migrating from single-graph to parallel-graph architecture:

1. Update components to use `useParallelGraphs` hook
2. Replace direct `graphDataManager` calls with coordinator API
3. Ensure graph type is specified for all data operations
4. Update physics configuration for each graph type

## Future Enhancements

1. **Dynamic Graph Types**: Runtime registration of new graph types
2. **Cross-Graph Relationships**: Visual links between related nodes
3. **Unified Search**: Search across all graph types
4. **Graph Persistence**: Save/restore graph states
5. **Performance Monitoring**: Per-graph performance metrics