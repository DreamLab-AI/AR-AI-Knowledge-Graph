# Frontend MCP Connection Issue

## Problem

The frontend is attempting to connect directly to MCP WebSocket endpoints, which is architecturally incorrect and causes connection errors.

### Current (Incorrect) Behavior:
1. `MCPWebSocketService` in frontend tries to connect to:
   - `ws://192.168.0.51:3001/ws/mcp` (fails)
   - `ws://192.168.0.51/ws/mcp` (fails)
   - `ws://localhost:3001/ws/mcp` (fails)
   - `ws://192.168.0.51:3001/wss` (sometimes works but shouldn't be used)

2. These connection attempts generate errors in the console
3. The system falls back to REST API which works correctly

### Root Cause:
- `MCPWebSocketService.ts` exists in the frontend
- `BotsWebSocketIntegration.ts` initializes MCP connections
- `BotsVisualization.tsx` tries to fetch data directly from MCP

## Correct Architecture

```
┌─────────────┐     REST API      ┌─────────────┐     MCP Protocol    ┌──────────────┐
│   Frontend  │ ←───────────────→ │   Backend   │ ←─────────────────→ │ Claude Flow  │
│   (React)   │   /api/bots/*     │   (Rust)    │                     │     MCP      │
└─────────────┘                   └─────────────┘                     └──────────────┘
      ↑                                  ↓
      │                                  │
      └────── WebSocket Updates ─────────┘
         (Position data via binary protocol)
```

### Frontend Should Only Use:
1. **REST API Endpoints:**
   - `GET /api/bots/data` - Get current bots/agents data
   - `POST /api/bots/initialize-swarm` - Spawn new hive mind
   - `POST /api/bots/update` - Update bots data (if needed)

2. **WebSocket for Position Updates:**
   - Uses existing `WebSocketService` (not MCP)
   - Binary protocol for efficient position streaming
   - Request type: `requestBotsPositions`

### Backend Handles All MCP Communication:
1. `ClaudeFlowActor` connects to Claude Flow MCP
2. Polls for agent updates every 5 seconds
3. Stores data in `BOTS_GRAPH` static storage
4. Provides data via REST API to frontend

## Fix Required

### Remove from Frontend:
1. `/client/src/features/bots/services/MCPWebSocketService.ts`
2. MCP initialization in `BotsWebSocketIntegration.ts`
3. Direct MCP calls in `BotsVisualization.tsx`

### Update BotsVisualization to:
1. Only use `apiService.getBotsData()` for data fetching
2. Remove `fetchMCPData()` function
3. Remove MCP connection status checks
4. Rely on REST API and WebSocket position updates only

### Update BotsWebSocketIntegration to:
1. Remove `initializeMCPConnection()` method
2. Only handle Logseq WebSocket connection
3. Remove MCP-related event listeners

## Benefits of Correct Architecture:
1. No connection errors in console
2. Cleaner separation of concerns
3. Better security (MCP access only from backend)
4. Simpler frontend code
5. Easier to debug and maintain

## Implementation Notes:
- The backend already has all necessary endpoints
- ClaudeFlowActor already polls Claude Flow correctly
- WebSocket binary updates already work for positions
- Only frontend cleanup is needed