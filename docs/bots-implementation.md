# Bots Visualization & Hive Mind Implementation

## Overview

The bots visualization has been successfully integrated into VisionFlow to display agent activity from the claude-flow MCP server. The implementation includes:

1. **Backend Integration** - Processing bots data through the GPU physics system
2. **API Endpoints** - RESTful endpoints for bots data management and swarm initialization
3. **Binary WebSocket** - Real-time position updates using the same protocol as the main graph
4. **Frontend Visualization** - 3D force-directed graph with gold/green color scheme
5. **Hive Mind Spawning** - Interactive UI to spawn and configure Claude Flow swarms

## Architecture

### Backend Components

#### 1. Bots Handler (`/src/handlers/bots_handler.rs`)
- Processes incoming bots agent data
- Converts agents to graph nodes with physics properties
- Integrates with GPU physics simulation
- Provides endpoints for data updates and retrieval

#### 2. API Routes
- `/api/bots/data` - GET endpoint to retrieve current bots state
- `/api/bots/update` - POST endpoint to update bots data from MCP
- `/api/bots/initialize-swarm` - POST endpoint to spawn a Claude Flow hive mind

#### 3. WebSocket Integration (`/src/handlers/socket_flow_handler.rs`)
- Extended to handle `requestBotsPositions` messages
- Sends binary position updates with bots flag (0x80) for identification
- Uses same 28-byte format as main graph (4 bytes ID + 12 bytes position + 12 bytes velocity)

### Frontend Components

#### 1. BotsVisualizationIntegrated Component
- Main visualization component using React Three Fiber
- Connects to MCP server with fallback to backend API
- Renders agents as spheres with size based on workload
- Shows communication edges between agents

#### 2. Binary Updates Hook (`useBotsBinaryUpdates`)
- Handles binary WebSocket position updates
- Filters nodes by bots flag (0x80)
- Updates positions in real-time

#### 3. MCP WebSocket Service
- Attempts connection to multiple possible MCP server URLs
- Implements JSON-RPC 2.0 protocol for MCP tools
- Falls back to backend API if MCP unavailable

#### 4. SwarmInitializationPrompt Component
- Interactive modal dialog for spawning hive minds
- Configurable options:
  - **Topology**: mesh, hierarchical, ring, or star
  - **Agent Count**: 3-20 agents
  - **Agent Types**: coordinator, researcher, coder, analyst, tester, architect, optimizer, reviewer, documenter
  - **Neural Enhancement**: Enable WASM-accelerated neural networks
  - **Task Description**: Required field describing what the hive mind should accomplish

## Data Flow

### Visualization Updates
1. **MCP Server** → Sends agent data via WebSocket
2. **Backend** → Receives data, processes through GPU physics
3. **Binary WebSocket** → Streams position updates to frontend
4. **Frontend** → Renders 3D visualization with real-time updates

### Hive Mind Spawning
1. **User** → Clicks "Initialize Swarm" button in control panel
2. **SwarmInitializationPrompt** → Collects configuration and task
3. **Frontend** → POSTs to `/api/bots/initialize-swarm`
4. **BotsHandler** → Forwards request to ClaudeFlowActor
5. **ClaudeFlowActor** → Connects to Claude Flow MCP and:
   - Initializes swarm with selected topology
   - Spawns requested agent types
   - Enables neural patterns if selected
   - Applies custom task prompt
6. **Claude Flow** → Creates agents and begins task execution
7. **Visualization** → Updates to show active agents

## Visual Design

- **Gold** (#F1C40F) - Coordinator agents
- **Green** (#2ECC71) - Worker agents (coder, tester)
- **Orange/Teal** - Specialized agents
- **Edge thickness** - Represents communication volume
- **Node size** - Scales with agent workload

## Usage

### Spawning a Hive Mind

1. Open VisionFlow and look for the control panel
2. Click the "Initialize Swarm" button when no agents are active
3. Configure your hive mind:
   - **Topology**: Choose the communication structure
     - *Mesh*: All agents can communicate (best for collaboration)
     - *Hierarchical*: Structured with clear command chain
     - *Ring*: Sequential processing pipeline
     - *Star*: Central coordinator with workers
   - **Agent Count**: Slide to select 3-20 agents
   - **Agent Types**: Check which types of agents to spawn
   - **Neural Enhancement**: Enable for advanced AI capabilities
   - **Task**: Describe what you want the hive mind to accomplish
4. Click "Spawn Hive Mind" to create the swarm

### Example Tasks

- "Build a REST API with user authentication and database integration"
- "Analyze this codebase and create comprehensive documentation"
- "Refactor the frontend components to use TypeScript"
- "Create a test suite with 90% code coverage"
- "Build a boilerplate LaTeX contract document and compile it"

## Testing

Run the test script to verify the implementation:

```bash
./scripts/test-bots.sh
```

This will:
1. Check service status
2. Send test bots data to the backend
3. Retrieve and display the processed data

### Testing Hive Mind Spawning

```bash
# Test the initialize-swarm endpoint
curl -X POST http://localhost:4000/api/bots/initialize-swarm \
  -H "Content-Type: application/json" \
  -d '{
    "topology": "mesh",
    "maxAgents": 8,
    "strategy": "adaptive",
    "enableNeural": true,
    "agentTypes": ["coordinator", "coder", "analyst", "tester"],
    "customPrompt": "Build a simple REST API"
  }'
```

## Next Steps

1. **Live MCP Integration** - Connect to actual claude-flow container
2. **Performance Optimization** - Batch updates for large botss
3. **Enhanced Visuals** - Add particle effects for active communications
4. **Interaction** - Click agents to see detailed metrics

## Troubleshooting

### No Bots Visible
- Check WebSocket connection in browser console
- Verify backend services are running: `supervisorctl status`
- Check for CORS errors with MCP server

### Position Updates Not Working
- Verify binary WebSocket messages in Network tab
- Check for bots flag (0x80) in binary data
- Ensure GPU physics system is initialized

### MCP Connection Failed
- Verify claude-flow container is running
- Check Docker network connectivity
- Try different MCP server URLs in MCPWebSocketService

### Swarm Initialization Failed (404 Error)
**This is the most common issue after adding the new endpoint!**

1. **Restart the backend server** to register the new route:
   ```bash
   # From the ext directory
   supervisorctl restart webxr
   # Or if running via cargo:
   cargo run --features gpu
   ```

2. Check the logs for route registration:
   ```
   [INFO] Configuring bots routes:
   [INFO]   - /bots/data (GET)
   [INFO]   - /bots/update (POST)
   [INFO]   - /bots/initialize-swarm (POST)
   ```

3. Verify the endpoint is accessible:
   ```bash
   curl -X POST http://localhost:3001/api/bots/initialize-swarm \
     -H "Content-Type: application/json" \
     -d '{"topology":"mesh","maxAgents":8,"strategy":"adaptive","enableNeural":true,"agentTypes":["coordinator"],"customPrompt":"Test"}'
   ```

### Agents Not Spawning
- Check ClaudeFlowActor logs for connection errors
- Verify Claude Flow container has MCP tools available
- Ensure task description is provided (required field)
- Check agent type names match Claude Flow expectations