# Claude Flow Swarm Visualization

## Overview

This implementation adds a real-time, force-directed graph visualization of the Claude Flow agent swarm to the VisionFlow application. The visualization creates a "living hive" or "digital organism" metaphor, displaying agents as nodes and their communications as animated edges.

## Visual Design

The swarm visualization uses a **gold and green color palette** as specified:

### Node Colors (by Agent Type)
- **Coder**: Emerald green (#2ECC71)
- **Tester**: Nephritis green (#27AE60)
- **Coordinator**: Sunflower gold (#F1C40F)
- **Analyst**: Orange gold (#F39C12)
- **Researcher**: Turquoise green (#1ABC9C)
- **Architect**: Carrot gold (#E67E22)
- **Reviewer**: Green sea (#16A085)
- **Optimizer**: Dark gold (#D68910)
- **Documenter**: Medium green (#229954)
- **Monitor**: Gold yellow (#D4AC0D)
- **Specialist**: Dark green (#239B56)

### Visual Mappings

**Nodes (Agents)**:
- **Size**: Proportional to workload/CPU usage
- **Shape**: Sphere (normal), Tetrahedron (error), Box (initializing)
- **Border Color**: Health indicator (green=healthy, red=critical)
- **Pulse Animation**: Frequency based on CPU usage

**Edges (Communications)**:
- **Thickness**: Data volume transferred
- **Animation**: Particle flow showing message direction
- **Speed**: Message frequency

**Physics**:
- **Spring Forces**: Agents that communicate frequently cluster together
- **Gravity**: Token-heavy agents pull others toward them
- **Repulsion**: Unhealthy agents push others away

## Architecture

### Components

1. **SwarmVisualization** (`/client/src/features/swarm/components/SwarmVisualization.tsx`)
   - Main React component rendering the swarm
   - Manages WebSocket connection to MCP orchestrator
   - Updates node positions from physics simulation

2. **MCPWebSocketService** (`/client/src/features/swarm/services/MCPWebSocketService.ts`)
   - Handles WebSocket connection to MCP orchestrator
   - Implements JSON-RPC 2.0 protocol
   - Provides methods to fetch agents, communications, and token usage

3. **SwarmPhysicsWorker** (`/client/src/features/swarm/workers/swarmPhysicsWorker.ts`)
   - Simulates force-directed graph physics
   - Runs continuously to update node positions
   - Implements spring forces, gravity, and repulsion

4. **MCP Orchestrator** (Docker service)
   - Polls Claude Flow MCP servers
   - Broadcasts updates via WebSocket
   - Provides real-time swarm data

## Setup Instructions

### 1. Build and Start Services

```bash
# Build and start all services including MCP orchestrator
docker-compose up -d

# Or if you need to rebuild
docker-compose up -d --build
```

### 2. Verify Services

Check that all services are running:
```bash
docker-compose ps
```

You should see:
- `powerdev`: Main container with VisionFlow
- `mcp-orchestrator`: WebSocket server for swarm data

### 3. Access the Visualization

1. Open your browser to `http://localhost:5173` (or the configured VisionFlow port)
2. The swarm visualization will appear to the right of the main knowledge graph
3. Agents will initialize with random positions and begin organizing based on their interactions

### 4. Troubleshooting

**WebSocket Connection Issues**:
- Check orchestrator logs: `docker logs mcp-orchestrator`
- Verify port 9001 is accessible
- Check browser console for connection errors

**No Agents Visible**:
- Ensure Claude Flow is running in the powerdev container
- Verify MCP_SERVERS environment variable includes claude-flow
- Check if agents are spawned: `docker exec powerdev npx claude-flow@alpha agent list`

**Performance Issues**:
- Reduce physics simulation frequency in swarmPhysicsWorker
- Limit number of communication edges displayed
- Adjust particle effects density

## API Endpoints

### MCP Orchestrator REST API (Port 9000)
- `GET /health` - Health check
- `GET /api/mcp/data` - Get all MCP data
- `POST /api/mcp/tool` - Execute MCP tool
- `GET /api/mcp/servers` - List configured servers

### WebSocket API (Port 9001)
- Auto-broadcasts every 5 seconds
- Message types: `welcome`, `mcp-update`, `mcp-response`
- Supports JSON-RPC 2.0 requests

## Development

### Adding New Agent Types

1. Add color in `VISUAL_CONFIG` in SwarmVisualization.tsx
2. Update `SwarmAgent` type definition
3. Implement custom visual behavior if needed

### Modifying Physics

Edit parameters in `swarmPhysicsWorker.ts`:
- `springStrength`: Edge attraction force
- `nodeRepulsion`: Node separation force
- `gravityStrength`: Token-based gravity
- `damping`: Velocity reduction factor

### Custom Visual Effects

The visualization supports:
- Custom node geometries
- Particle systems
- Glow effects
- Animation curves

## Performance Optimization

1. **Batch Updates**: Communications are polled every 2 seconds
2. **Efficient Rendering**: Uses Three.js instanced meshes
3. **Worker Threading**: Physics run in separate thread
4. **Selective Updates**: Only active edges are rendered

## Future Enhancements

- [ ] 3D spatial audio for agent activity
- [ ] Heat maps for communication density
- [ ] Historical playback of swarm activity
- [ ] Agent clustering algorithms
- [ ] Performance metrics overlay
- [ ] Interactive agent inspection
- [ ] Swarm pattern recognition

## Integration with VisionFlow

The swarm visualization is designed to coexist with the main knowledge graph:
- Positioned 50 units to the right of origin
- Independent physics simulation
- Shared Three.js renderer for efficiency
- Non-interfering camera controls

## Security Considerations

- WebSocket connections are not encrypted (use WSS in production)
- MCP orchestrator has rate limiting enabled
- CORS is configured for local development
- Authentication should be added for production use