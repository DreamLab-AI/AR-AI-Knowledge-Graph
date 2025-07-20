# MCP Orchestrator Setup for Development

## Current Issue
The MCP relay handler is trying to connect to an orchestrator service that's not running in the dev environment, causing "Orchestrator not connected" errors.

## Quick Fix (Already Applied)
The SwarmVisualizationEnhanced component now defaults to mock data mode to avoid errors.

## Proper Solution Options

### Option 1: Run MCP Orchestrator in Dev (Recommended)
Add the mcp-orchestrator service to your dev environment:

```bash
# Create a docker-compose.override.yml in the ext directory
cat > docker-compose.override.yml << 'EOF'
services:
  mcp-orchestrator:
    build:
      context: .
      dockerfile: Dockerfile.orchestrator
    container_name: mcp-orchestrator-dev
    environment:
      - NODE_ENV=development
      - LOG_LEVEL=debug
      - MCP_ORCHESTRATOR_PORT=9000
      - WEBSOCKET_PORT=9001
      - POLL_INTERVAL=5000
      - MCP_SERVERS=blender:9876,claude-flow:3000
    ports:
      - "9000:9000"
      - "9001:9001"
    networks:
      - docker_ragflow
    restart: unless-stopped
EOF

# Then run both services together
docker-compose -f docker-compose.dev.yml -f docker-compose.override.yml up
```

### Option 2: Update MCP Relay Handler Environment
Set the correct orchestrator URL in the dev environment:

```bash
# In docker-compose.dev.yml, add to the webxr service environment:
- ORCHESTRATOR_WS_URL=ws://mcp-orchestrator:9001/ws
```

### Option 3: Use Direct MCP Connection (Skip Relay)
Modify the MCPWebSocketService to connect directly to MCP servers instead of through the relay:

```typescript
// In MCPWebSocketService.ts, change the connection URL to:
this.wsUrl = `${protocol}//${host}/api/mcp/direct`; // Hypothetical direct endpoint
```

## Current Workaround
The system currently falls back to mock data when MCP is unavailable, which is suitable for development but won't show real swarm data.

## WebSocket Endpoints Reference
- `/wss` - Main backend WebSocket (Logseq graph data)
- `/ws/mcp` - MCP relay WebSocket (expects orchestrator at ws://orchestrator:8080/ws)
- `/ws/speech` - Voice system WebSocket
- `/ws` - Vite HMR WebSocket

## Testing the Fix
1. Check if mock data is displaying: Look for "VisionFlow (MOCK)" in the UI
2. Monitor console for MCP connection attempts
3. No more "Orchestrator not connected" errors should appear