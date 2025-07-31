# MCP WebSocket Relay Architecture

This document describes the architecture for relaying MCP (Model Context Protocol) data through the backend to the frontend visualization.

## Overview

The MCP WebSocket relay architecture enables real-time visualization of Claude Flow agent activity without exposing the MCP protocol directly to the frontend. The backend acts as a relay, transforming MCP data into a format suitable for 3D visualization.

## Architecture Components

### 1. Backend: ClaudeFlowActor (`claude_flow_actor.rs`)

The Rust backend actor that manages the MCP connection:

```rust
pub struct ClaudeFlowActor {
    client: ClaudeFlowClient,
    graph_service_addr: Addr<GraphServiceActor>,
    is_connected: bool,
}
```

**Key Responsibilities:**
- Spawns Claude Flow MCP process via stdio
- Polls for agent updates every 5 seconds
- Transforms MCP agent data to visualization format
- Provides mock data when MCP is unavailable

### 2. MCP Connection Types

The backend supports three transport mechanisms:

```rust
enum TransportType {
    Http,       // REST API (deprecated)
    WebSocket,  // WebSocket connection
    Stdio,      // Process stdio (default)
}
```

**Current Implementation:** Uses stdio transport by spawning `npx claude-flow@alpha mcp start`

### 3. Data Flow

```
Claude Flow MCP Server (stdio process)
    ↓ JSON-RPC
ClaudeFlowActor (Rust backend)
    ↓ UpdateBotsGraph message
GraphServiceActor
    ↓ REST API
Frontend REST Client
    ↓ State update
MCPWebSocketService (frontend)
    ↓ Physics simulation
BotsPhysicsWorker
    ↓ Position updates
3D Visualization
```

### 4. Agent Data Transformation

MCP agent data is transformed for visualization:

```rust
AgentStatus {
    agent_id: String,
    status: String,              // "active", "idle", "busy"
    profile: AgentProfile {
        name: String,
        agent_type: AgentType,   // Coordinator, Researcher, Coder, etc.
        capabilities: Vec<String>,
        max_concurrent_tasks: u32,
        priority: u32,
    },
    active_tasks_count: u32,
    completed_tasks_count: u32,
    success_rate: f64,
    // ... performance metrics
}
```

## Frontend Integration

### MCPWebSocketService

The frontend service that manages agent data:

```typescript
class MCPWebSocketService {
  private agents: Map<string, BotsAgent> = new Map();
  private communications: BotsCommunication[] = [];
  
  async fetchAgentsFromBackend() {
    const response = await fetch('/api/bots/agents');
    const data = await response.json();
    this.processAgentData(data.agents);
  }
}
```

**Note:** Despite the name, this service uses REST API polling, not WebSocket

### BotsWebSocketIntegration

Manages the integration between data sources:

```typescript
class BotsWebSocketIntegration {
  // MCP connections are backend-only
  // Frontend uses REST API for agent data
  
  getConnectionStatus() {
    return {
      mcp: false,  // MCP handled by backend
      logseq: this.logseqConnected,
      overall: this.logseqConnected
    };
  }
}
```

## API Endpoints

### GET /api/bots/agents
Returns current agent status from ClaudeFlowActor:

```json
{
  "agents": [
    {
      "agent_id": "coordinator-001",
      "status": "active",
      "profile": {
        "name": "System Coordinator",
        "agent_type": "coordinator",
        "capabilities": ["orchestration", "task-management"]
      },
      "active_tasks_count": 3,
      "success_rate": 100.0
    }
  ]
}
```

### POST /api/bots/spawn
Spawns a new agent via ClaudeFlowActor:

```json
{
  "agent_type": "researcher",
  "name": "Research Agent",
  "capabilities": ["data-gathering", "analysis"]
}
```

### POST /api/bots/swarm/init
Initializes a swarm configuration:

```json
{
  "topology": "hierarchical",
  "max_agents": 8,
  "agent_types": ["coordinator", "researcher", "coder"],
  "enable_neural": true
}
```

## Mock Mode

When Claude Flow MCP is unavailable, the backend provides mock data:

```rust
fn create_mock_agents() -> Vec<AgentStatus> {
    vec![
        AgentStatus {
            agent_id: "coordinator-001",
            status: "active",
            profile: AgentProfile {
                name: "System Coordinator",
                agent_type: AgentType::Coordinator,
                // ... mock configuration
            },
            // ... mock metrics
        },
        // Additional mock agents...
    ]
}
```

This ensures the visualization remains functional for development and testing.

## Security Considerations

1. **No Direct MCP Access**: Frontend never connects directly to MCP
2. **Backend Validation**: All agent operations validated by backend
3. **Rate Limiting**: Polling intervals prevent API abuse
4. **Session Management**: MCP sessions managed server-side

## Performance Optimizations

1. **Polling Intervals**: 
   - Agent updates: 5 seconds
   - Health checks: 30 seconds
   - Frontend polling: 10 seconds

2. **Data Caching**: Backend caches agent state between polls

3. **Differential Updates**: Only changed agents sent to frontend

4. **Mock Data**: Instant response when MCP unavailable

## Error Handling

The system gracefully degrades when MCP is unavailable:

1. **Connection Failure**: Falls back to mock data
2. **Polling Errors**: Logged but don't crash the system
3. **Frontend Resilience**: Continues with last known state

## Future Enhancements

1. **WebSocket Push**: Replace polling with real-time push
2. **Binary Protocol**: Optimize agent data transmission
3. **Event Streaming**: Stream agent events as they occur
4. **Multi-MCP Support**: Connect to multiple Claude Flow instances
5. **Agent Metrics Dashboard**: Detailed performance visualization