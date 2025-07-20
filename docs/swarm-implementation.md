# Swarm Visualization Implementation

## Overview

The swarm visualization has been successfully integrated into VisionFlow to display agent activity from the claude-flow MCP server. The implementation includes:

1. **Backend Integration** - Processing swarm data through the GPU physics system
2. **API Endpoints** - RESTful endpoints for swarm data management
3. **Binary WebSocket** - Real-time position updates using the same protocol as the main graph
4. **Frontend Visualization** - 3D force-directed graph with gold/green color scheme

## Architecture

### Backend Components

#### 1. Swarm Handler (`/src/handlers/swarm_handler.rs`)
- Processes incoming swarm agent data
- Converts agents to graph nodes with physics properties
- Integrates with GPU physics simulation
- Provides endpoints for data updates and retrieval

#### 2. API Routes (`/src/handlers/api_handler/swarm/mod.rs`)
- `/api/swarm/data` - GET endpoint to retrieve current swarm state
- `/api/swarm/update` - POST endpoint to update swarm data from MCP

#### 3. WebSocket Integration (`/src/handlers/socket_flow_handler.rs`)
- Extended to handle `requestSwarmPositions` messages
- Sends binary position updates with swarm flag (0x80) for identification
- Uses same 28-byte format as main graph (4 bytes ID + 12 bytes position + 12 bytes velocity)

### Frontend Components

#### 1. SwarmVisualizationIntegrated Component
- Main visualization component using React Three Fiber
- Connects to MCP server with fallback to backend API
- Renders agents as spheres with size based on workload
- Shows communication edges between agents

#### 2. Binary Updates Hook (`useSwarmBinaryUpdates`)
- Handles binary WebSocket position updates
- Filters nodes by swarm flag (0x80)
- Updates positions in real-time

#### 3. MCP WebSocket Service
- Attempts connection to multiple possible MCP server URLs
- Implements JSON-RPC 2.0 protocol for MCP tools
- Falls back to backend API if MCP unavailable

## Data Flow

1. **MCP Server** → Sends agent data via WebSocket
2. **Backend** → Receives data, processes through GPU physics
3. **Binary WebSocket** → Streams position updates to frontend
4. **Frontend** → Renders 3D visualization with real-time updates

## Visual Design

- **Gold** (#F1C40F) - Coordinator agents
- **Green** (#2ECC71) - Worker agents (coder, tester)
- **Orange/Teal** - Specialized agents
- **Edge thickness** - Represents communication volume
- **Node size** - Scales with agent workload

## Testing

Run the test script to verify the implementation:

```bash
./scripts/test-swarm.sh
```

This will:
1. Check service status
2. Send test swarm data to the backend
3. Retrieve and display the processed data

## Next Steps

1. **Live MCP Integration** - Connect to actual claude-flow container
2. **Performance Optimization** - Batch updates for large swarms
3. **Enhanced Visuals** - Add particle effects for active communications
4. **Interaction** - Click agents to see detailed metrics

## Troubleshooting

### No Swarm Visible
- Check WebSocket connection in browser console
- Verify backend services are running: `supervisorctl status`
- Check for CORS errors with MCP server

### Position Updates Not Working
- Verify binary WebSocket messages in Network tab
- Check for swarm flag (0x80) in binary data
- Ensure GPU physics system is initialized

### MCP Connection Failed
- Verify claude-flow container is running
- Check Docker network connectivity
- Try different MCP server URLs in MCPWebSocketService