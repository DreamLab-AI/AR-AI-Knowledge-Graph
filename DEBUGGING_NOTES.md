# VisionFlow Debugging Notes

## Issue: Blank Page on 192.168.0.51:3001

### Root Cause Analysis
1. **Docker Stack Not Running**: The application requires 3 services:
   - Nginx (port 3001) - Entry point proxy
   - Vite Dev Server (port 5173) - Frontend
   - Rust Backend (port 4000) - API server

2. **Only Vite is Running**: Currently only the Vite dev server is active
3. **API Calls Failing**: Frontend cannot fetch graph data from `/api/graph/data`

### Temporary Fix Applied
- Modified `client/src/services/api.ts` to return mock data when backend is unavailable
- This allows the frontend to load and display basic visualization

### Next Steps
1. Start Docker services with: `cd /workspace/ext && ./scripts/dev.sh`
2. Verify all services are running
3. Implement the swarm visualization component as per task.md

### Key Requirements from task.md
- Create a second force-directed graph for agent swarm visualization
- Connect to MCP server via WebSocket: `ws://<claude-flow-container>:3000/ws`
- Poll these endpoints for real-time data:
  - `agents/list` - Agent info
  - `analysis/token-usage` - Token metrics
  - `memory/query` - Communication logs
  - `agent_metrics` - Performance data

### Visual Design
- Gold and green color scheme
- Nodes represent agents (size = workload, color = type)
- Edges show communication (thickness = data volume)
- "Living Hive" metaphor with dynamic animations