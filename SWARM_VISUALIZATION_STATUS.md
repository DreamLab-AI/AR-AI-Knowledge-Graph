# Swarm Visualization Integration - Status Notes

## Project Overview
Integration of real-time agent swarm visualization into existing Three.js/React Three Fiber application. The swarm visualization displays live agent data from an MCP (Model Context Protocol) WebSocket endpoint.

## Current State (2025-01-20 09:50 UTC)

### ✅ Completed Tasks
1. **Integration Point Verified**: Swarm visualization correctly integrated into `client/src/features/graph/components/GraphViewport.tsx`
2. **Component Structure**: Both `SwarmVisualizationEnhanced.tsx` and `SwarmVisualizationSimpleTest.tsx` are properly imported and rendered
3. **Mock Data Rendering**: Confirmed working - visualization displays with fallback mock data
4. **MCP WebSocket Service Updated**: Modified to connect to backend `/wss` endpoint instead of direct `powerdev` container connection

### 🔄 In Progress
- **MCP WebSocket Connection**: Recently updated `MCPWebSocketService.ts` to connect through backend Nginx proxy instead of direct container access

### ⏳ Pending Tasks
- Remove duplicate components from GraphCanvas.tsx if they exist
- Test mock data fallback behavior when MCP connection fails
- Verify visual mappings (gold/green colors, shapes, animations)
- Check console for [SWARM] debug messages
- Test physics simulation clustering behavior
- Verify swarm positioning at x=60 doesn't overlap main graph
- Test with powerdev container running for real MCP data
- Performance testing with multiple agents
- Create troubleshooting guide

## Technical Architecture

### Key Components
1. **SwarmVisualizationEnhanced.tsx**: Main swarm visualization component
   - Handles data source selection (MCP → REST → Mock)
   - Renders agents as gold spheres, communications as green lines
   - Positioned at x=60 to avoid overlap with main graph

2. **MCPWebSocketService.ts**: WebSocket client for real-time data
   - **RECENT CHANGE**: Now connects to backend `/wss` endpoint
   - Implements three-tier fallback: MCP WebSocket → REST API → Mock Data
   - Handles reconnection logic and error recovery

3. **swarmPhysicsWorker.ts**: Web Worker for physics calculations
   - Offloads physics simulation from main thread
   - Handles agent clustering and communication visualization

### Network Architecture
- **Frontend**: Connects to backend WebSocket endpoint via Nginx proxy
- **Backend**: Rust server exposes `/wss` and `/ws/speech` endpoints
- **MCP Server**: Runs in `powerdev` container on `ragflow` Docker network
- **Proxy Flow**: Frontend → Nginx → Backend → powerdev container

### Data Flow
```
powerdev:3000/ws → Backend (/wss) → Nginx Proxy → Frontend MCPWebSocketService
                     ↓ (fallback)
                 REST API → Mock Data
```

## Recent Changes (Last Session)

### Modified Files
1. **client/src/features/swarm/services/MCPWebSocketService.ts**
   - **Changed constructor**: Now uses `window.location.host` + `/wss` instead of direct powerdev URLs
   - **Updated connect() method**: Replaced powerdev URLs with backend WebSocket endpoints
   - **Protocol handling**: Properly handles ws/wss based on current protocol

### Key Code Changes
```typescript
// OLD (direct powerdev connection)
this.wsUrl = 'ws://powerdev:3000/ws';

// NEW (backend proxy connection)
const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const host = window.location.host;
this.wsUrl = `${protocol}//${host}/wss`;
```

## Architecture Insights Discovered

### Nginx Proxy Configuration
- **Location**: `nginx.conf`, `nginx.dev.conf`
- **WebSocket Routes**:
  - `/ws` → Vite HMR
  - `/wss` → Backend WebSocket
  - `/ws/speech` → Speech backend
- **Missing**: No direct proxy rule for powerdev:3000

### Backend WebSocket Endpoints
- **Main endpoint**: `/wss` (socket_flow_handler.rs)
- **Speech endpoint**: `/ws/speech`
- **Expected behavior**: Backend should relay MCP data through `/wss`

### Docker Network Setup
- **Network**: `ragflow`
- **Containers**: `visionflow` (visualization), `powerdev` (agent swarm)
- **IPs**: powerdev at 172.18.0.10

## Issues Encountered & Solutions

### 1. Direct Container Connection Failed
**Problem**: Frontend trying to connect directly to `powerdev:3000/ws`
**Root Cause**: Bypass of Nginx proxy, no direct route configured
**Solution**: Route through backend `/wss` endpoint instead

### 2. Vite Proxy Configuration
**Problem**: Vite config explicitly states "Proxy is now handled by Nginx"
**Solution**: Don't use Vite proxy, rely on Nginx configuration

### 3. WebSocket Connection Timeout
**Problem**: Direct powerdev connections timing out
**Root Cause**: No network route from frontend to powerdev
**Solution**: Use proper proxy chain through backend

## Key Files Reference

### Primary Implementation
- `client/src/features/graph/components/GraphViewport.tsx` - Integration point
- `client/src/features/swarm/components/SwarmVisualizationEnhanced.tsx` - Main component
- `client/src/features/swarm/services/MCPWebSocketService.ts` - WebSocket client
- `client/src/features/swarm/workers/swarmPhysicsWorker.ts` - Physics simulation

### Configuration
- `client/vite.config.ts` - Build configuration (proxy delegation noted)
- `nginx.conf`, `nginx.dev.conf` - Proxy routes
- `src/handlers/socket_flow_handler.rs` - Backend WebSocket handler

### Documentation
- `docs/client/websocket.md` - WebSocket architecture
- `docs/api/websocket.md` - API WebSocket documentation
- `task.md` - Original task specification

## Testing Strategy

### 1. Connection Testing
```bash
# Check if backend WebSocket is running
curl -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" \
  http://localhost:3001/wss
```

### 2. Console Debugging
Look for these log patterns:
- `[SWARM]` - Swarm visualization debug messages
- `MCPWebSocketService` - Connection status and errors
- `SwarmVisualizationEnhanced` - Data source selection

### 3. Visual Verification
- Gold spheres for agents at x=60 position
- Green lines for communications
- No overlap with main graph (x=0)
- Smooth physics simulation

## Next Steps Priority

### High Priority
1. **Test New WebSocket Configuration**: Verify backend `/wss` connection works
2. **Verify Mock Data Fallback**: Ensure graceful degradation when MCP unavailable
3. **Check Console Logs**: Confirm [SWARM] debug messages appear

### Medium Priority
4. **Remove Duplicate Components**: Clean up any redundant code in GraphCanvas.tsx
5. **Visual Mapping Test**: Verify colors, shapes, animations work correctly
6. **Physics Simulation**: Test clustering behavior with multiple agents

### Low Priority
7. **Performance Testing**: Test with many agents
8. **Documentation**: Create troubleshooting guide
9. **Positioning Verification**: Ensure no overlap with main graph

## Environment Context
- **Current Branch**: `rUv-smash-branch`
- **Modified Files**: `MCPWebSocketService.ts` has uncommitted changes
- **Docker Status**: Unknown - need to verify powerdev container is running
- **Development Server**: Assumed running on standard ports (3001 for backend)

## Critical Success Criteria
1. WebSocket connection to backend `/wss` endpoint successful
2. Mock data displays when MCP unavailable
3. Real MCP data displays when powerdev container is running
4. No visual conflicts with main knowledge graph
5. Smooth performance with physics simulation

## Debugging Commands
```bash
# Check Docker containers
docker ps | grep powerdev

# Check backend logs
docker logs [backend-container-name]

# Check Nginx configuration
docker exec [nginx-container] cat /etc/nginx/nginx.conf

# Test WebSocket endpoint
websocat ws://localhost:3001/wss
```

---
**Last Updated**: 2025-01-20 09:50 UTC
**Next Action**: Test updated MCPWebSocketService connection to `/wss` endpoint


Verify current rendering setup - check if both graphs are visible side-by-side
Test if swarm visualization is actually rendering in the browser
Verify MCP WebSocket URLs match the powerdev container on ragflow network
Create detailed status notes for context switching
Remove duplicate components from GraphCanvas.tsx if they exist
Test mock data is working when MCP connection fails
Verify visual mappings (gold/green colors, shapes, animations) work correctly
Check console for [SWARM] debug messages
Test physics simulation produces proper clustering behavior
Verify swarm positioning at x=60 doesn't overlap main graph
Test with powerdev container running for real MCP data
Check performance with multiple agents
Create troubleshooting guide if components don't render