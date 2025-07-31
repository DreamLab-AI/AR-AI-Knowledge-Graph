# Agent Visualization Architecture

## Overview

The agent visualization system provides real-time, GPU-accelerated rendering of MCP agent swarms with rich metadata and visual feedback. It uses a JSON initialization followed by high-frequency WebSocket position updates for optimal performance.

## Architecture Components

### 1. Server-Side (Rust)

#### Data Flow Pipeline
1. **MCP Data Source** → Claude Flow Actor polls agent status
2. **Agent Visualization Processor** → Transforms raw MCP data into visualization format
3. **Agent Visualization Protocol** → Handles JSON/WebSocket message formatting
4. **WebSocket Handler** → Streams data to connected clients

#### Key Files:
- `/src/services/agent_visualization_processor.rs` - Processes MCP data for visualization
- `/src/services/agent_visualization_protocol.rs` - Defines JSON/WebSocket protocol
- `/src/handlers/bots_visualization_handler.rs` - WebSocket endpoint and handlers

### 2. Client-Side (TypeScript/Three.js)

#### Rendering Pipeline
1. **WebSocket Connection** → Receives JSON init and position updates
2. **Agent Visualization Client** → Processes messages and maintains state
3. **GPU Physics Solver** → Shared with knowledge graph for unified space
4. **Three.js Renderer** → GPU-accelerated visualization with effects

#### Key Files:
- `/client/src/features/bots/services/AgentVisualizationClient.ts` - Client protocol handler
- `/client/src/features/bots/components/AgentVisualizationGPU.tsx` - GPU rendering
- `/client/src/features/bots/workers/BotsPhysicsWorker.ts` - Shared physics engine

## Protocol Design

### Initial JSON Message
```json
{
  "type": "init",
  "timestamp": 1234567890,
  "swarm_id": "swarm-001",
  "topology": "hierarchical",
  "agents": [
    {
      "id": "agent-123",
      "name": "Coordinator Alpha",
      "type": "coordinator",
      "status": "active",
      "color": "#00FFFF",
      "shape": "octahedron",
      "size": 1.5,
      "health": 0.95,
      "cpu": 0.45,
      "memory": 0.30,
      "activity": 0.8,
      "tasks_active": 3,
      "tasks_completed": 47,
      "success_rate": 0.92,
      "tokens": 15420,
      "token_rate": 125.5,
      "capabilities": ["orchestration", "planning"],
      "created_at": 1234567890
    }
  ],
  "connections": [
    {
      "id": "conn-456",
      "source": "agent-123",
      "target": "agent-789",
      "strength": 0.7,
      "flow_rate": 0.5,
      "color": "#4444FF",
      "active": true
    }
  ],
  "visual_config": {
    "colors": { ... },
    "effects": { ... }
  },
  "physics_config": {
    "spring_strength": 0.3,
    "link_distance": 25,
    "damping": 0.92
  }
}
```

### Position Updates (60Hz)
```json
{
  "type": "positions",
  "timestamp": 1234567891000,
  "positions": [
    {
      "id": "agent-123",
      "x": 12.5,
      "y": -3.2,
      "z": 8.7,
      "vx": 0.1,
      "vy": -0.05,
      "vz": 0.02
    }
  ]
}
```

## Visual Features

### Agent Representation
- **Shape by Type**: Coordinators (octahedron), Coders (cube), Architects (cone), etc.
- **Color by Function**: Each agent type has a distinct color palette
- **Size by Workload**: Visual scaling based on active tasks and resource usage
- **Glow by Activity**: Intensity indicates current activity level
- **Health Rings**: Colored rings show agent health status

### Connection Visualization
- **Flowing Particles**: Show data flow direction and volume
- **Line Thickness**: Represents connection strength
- **Color Coding**: Different colors for different communication types
- **Pulse Effects**: Active connections pulse with activity

### Metadata Display
- **Interactive Labels**: Click to expand detailed agent information
- **Performance Metrics**: Real-time CPU, memory, token usage
- **Task Information**: Current task, completed count, success rate
- **Capability Tags**: Visual badges for agent capabilities

## GPU Optimization

### Shared Physics Engine
The visualization shares the same GPU-accelerated spring physics solver as the knowledge graph, ensuring:
- Unified coordinate space
- Consistent physics behavior
- Optimal GPU utilization
- Single source of truth for positions

### Rendering Optimizations
- **Instanced Rendering**: For large agent counts (>50)
- **LOD System**: Level of detail based on camera distance
- **Frustum Culling**: Only render visible agents
- **Batch Updates**: Position updates batched at 60Hz

## Integration Points

### With Knowledge Graph
- Shares same 3D space and origin point
- Uses same GPU physics solver
- Compatible visual styles (different color schemes)
- Can show relationships between agents and knowledge nodes

### With MCP
- Real-time polling of agent status
- Automatic swarm discovery
- Token usage tracking
- Task assignment visualization

## Performance Considerations

### Network Efficiency
- JSON initialization: ~5-50KB (depending on agent count)
- Position updates: ~50-200 bytes per agent per frame
- State updates: Sent only on changes
- WebSocket compression enabled

### Rendering Performance
- 60 FPS with up to 500 agents
- GPU instancing for large swarms
- Adaptive quality based on performance
- Mobile-optimized fallbacks

## Future Enhancements

1. **Swarm Patterns**: Visualize emergent swarm behaviors
2. **Historical Playback**: Record and replay swarm activity
3. **3D Heatmaps**: Show activity hotspots in the swarm
4. **Voice Integration**: Audio cues for agent states
5. **AR/VR Support**: Immersive swarm visualization

## Usage Example

```typescript
// Client-side initialization
const vizClient = new AgentVisualizationClient();

// Set up callbacks
vizClient.onInit((agents, connections) => {
  console.log(`Initialized with ${agents.length} agents`);
  // Create Three.js meshes
});

vizClient.onPositionChange((agentId, position) => {
  // Update mesh position
});

// Connect to WebSocket
const ws = new WebSocket('ws://localhost:3000/api/visualization/agents/ws');
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  vizClient.processMessage(message);
};

// In render loop
function animate() {
  vizClient.updatePositions(deltaTime);
  renderer.render(scene, camera);
  requestAnimationFrame(animate);
}
```

## Testing

1. Start MCP WebSocket relay in powerdev
2. Ensure Claude Flow is connected and has active agents
3. Open visualization endpoint in browser
4. Monitor WebSocket messages in DevTools
5. Check GPU performance metrics

The system provides a comprehensive, performant solution for visualizing complex agent swarms with rich real-time data.