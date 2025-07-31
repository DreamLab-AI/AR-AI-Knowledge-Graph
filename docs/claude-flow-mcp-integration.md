# Claude Flow MCP Integration

## Overview

This document describes the integration between the Rust-based force-directed graph visualization backend and the claude-flow hive-mind system via the Model Context Protocol (MCP).

## Architecture

### 1. MCP Connection Architecture

**Updated**: The integration now uses direct stdio communication instead of WebSocket:

```
┌─────────────────────────┐
│   ext (Rust Backend)    │
│                         │
│  ClaudeFlowActor        │
│  ├─ StdioTransport      │
│  │  └─ Spawns Process   │
│  └─ MCP Protocol        │
│                         │
└─────────────────────────┘
            │
            ▼
    npx claude-flow@alpha 
    mcp start --stdio
```

See [Claude Flow Stdio Integration](./claude-flow-stdio-integration.md) for detailed stdio implementation.

### 2. Key Components

#### ClaudeFlowActor (`/src/actors/claude_flow_actor.rs`)
- Actor-based implementation for reliable MCP communication
- Connects to `multi-agent-container:3002` via WebSocket
- Polls for agent updates every 5 seconds
- Gracefully falls back to mock data when MCP unavailable

#### WebSocket Transport (`/src/services/claude_flow/transport/websocket.rs`)
- Implements MCP protocol over WebSocket
- JSON-RPC 2.0 message format
- Handles request/response correlation via message IDs
- Manages connection lifecycle and error recovery

#### BotsHandler (`/src/handlers/bots_handler.rs`)
- Enhanced with hive-mind agent properties
- Hierarchical positioning algorithm for Queen-led visualization
- Real-time status updates via `/api/bots/status` endpoint
- Full graph data via `/api/bots/data` endpoint

### 3. Data Flow

1. **MCP Request**: ClaudeFlowActor sends `agent_list` tool call
2. **WebSocket Bridge**: Relays to claude-flow MCP process
3. **Claude Flow**: Returns agent status data
4. **Data Transform**: AgentStatus → BotsAgent with positioning
5. **Visualization**: Force-directed graph with hierarchical layout

### 4. Agent Types Mapping

| Claude Flow Type | Visualization Type | Role |
|------------------|--------------------|------|
| Coordinator | coordinator | Queen-like leader |
| Researcher | researcher | Information gathering |
| Coder | coder | Implementation |
| Analyst | analyst | Data analysis |
| Architect | architect | System design |
| Tester | tester | Quality assurance |
| Reviewer | reviewer | Code review |
| Optimizer | optimizer | Performance tuning |
| Documenter | documenter | Documentation |
| Monitor | monitor | System monitoring |
| Specialist | specialist | Domain expert |

### 5. Enhanced Agent Properties

```rust
pub struct BotsAgent {
    // Core properties
    pub id: String,
    pub agent_type: String,
    pub status: String,
    pub name: String,
    
    // Performance metrics
    pub cpu_usage: f32,
    pub health: f32,
    pub workload: f32,
    
    // Positioning (for visualization)
    pub position: Vec3,
    pub velocity: Vec3,
    pub force: Vec3,
    pub connections: Vec<String>,
    
    // Hive-mind properties
    pub capabilities: Option<Vec<String>>,
    pub current_task: Option<String>,
    pub tasks_active: Option<u32>,
    pub tasks_completed: Option<u32>,
    pub success_rate: Option<f32>,
    pub tokens: Option<u64>,
    pub token_rate: Option<f32>,
    pub activity: Option<f32>,
    pub swarm_id: Option<String>,
    pub agent_mode: Option<String>,
    pub parent_queen_id: Option<String>,
}
```

## Configuration

### Environment Variables

```bash
# Claude Flow MCP Configuration
CLAUDE_FLOW_HOST=multi-agent-container
CLAUDE_FLOW_PORT=3002
```

### Docker Compose

The services must be on the same Docker network (`docker_ragflow`):

```yaml
services:
  webxr:
    networks:
      - docker_ragflow
    environment:
      - CLAUDE_FLOW_HOST=multi-agent-container
      - CLAUDE_FLOW_PORT=3002
```

## Usage

### 1. Initialize Swarm

```bash
# POST /api/bots/initialize-swarm
{
  "topology": "hierarchical",
  "max_agents": 8,
  "strategy": "balanced",
  "enable_neural": true,
  "agent_types": ["coordinator", "researcher", "coder", "tester"]
}
```

### 2. Get Agent Status

```bash
# GET /api/bots/status
# Returns lightweight agent status for real-time updates
```

### 3. Get Full Graph Data

```bash
# GET /api/bots/data
# Returns complete graph with nodes, edges, and positioning
```

## Implementation Status

✅ **Completed**:
- ClaudeFlowActor with MCP integration
- Hierarchical positioning algorithm
- Enhanced BotsAgent structure
- Real-time status endpoints
- Mock data fallback
- Container name updates (powerdev → multi-agent-container)

🚧 **Pending**:
- WebSocket real-time push updates
- Full swarm topology visualization
- Neural pattern integration
- Performance metrics dashboard

## Troubleshooting

### Connection Issues

1. **Verify container is running**:
   ```bash
   docker ps | grep multi-agent-container
   ```

2. **Check network connectivity**:
   ```bash
   docker exec webxr ping multi-agent-container
   ```

3. **Test MCP WebSocket**:
   ```bash
   wscat -c ws://multi-agent-container:3002
   ```

### Fallback Behavior

When MCP is unavailable, the system automatically:
- Uses mock agents for demonstration
- Logs connection failures without crashing
- Continues visualization with static data
- Retries connection periodically

## Next Steps

1. Implement WebSocket push for real-time updates
2. Add swarm topology visualization modes
3. Integrate neural pattern performance metrics
4. Create agent interaction visualization
5. Add swarm health monitoring dashboard