# Claude Flow Direct Stdio Integration

## Overview

This document describes the updated integration between the Rust-based force-directed graph visualization backend and the claude-flow hive-mind system using direct stdio communication via the Model Context Protocol (MCP).

## Architecture

### Direct Process Spawning

The integration now uses direct process spawning instead of WebSocket connections:

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

### Key Benefits

1. **No Network Dependencies**: Direct process communication eliminates network-related issues
2. **Simplified Architecture**: No need for WebSocket bridges or port configuration
3. **Better Performance**: Lower latency through direct stdio pipes
4. **Process Isolation**: Each actor gets its own claude-flow process

### Implementation Details

#### StdioTransport (`/src/services/claude_flow/transport/stdio.rs`)

The StdioTransport now properly spawns claude-flow:

```rust
// Spawn claude-flow MCP process
let mut child = Command::new("npx")
    .args(&["claude-flow@alpha", "mcp", "start", "--stdio"])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .spawn()?;
```

Communication happens via:
- **stdin**: Send MCP requests (JSON-RPC 2.0)
- **stdout**: Receive MCP responses and notifications
- **stderr**: Inherited for debugging

#### ClaudeFlowActor Configuration

The actor now uses stdio transport directly:

```rust
let client_result = ClaudeFlowClientBuilder::new()
    .use_stdio()
    .build()
    .await;
```

### MCP Protocol Flow

1. **Server Initialization**:
   ```json
   → {"jsonrpc":"2.0","method":"server.initialized","params":{...}}
   ```

2. **Client Initialize**:
   ```json
   ← {"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{...}}
   → {"jsonrpc":"2.0","id":"init-1","result":{...}}
   ```

3. **Tool Calls** (e.g., agent_list):
   ```json
   ← {"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"agent_list","arguments":{...}}}
   → {"jsonrpc":"2.0","id":"1","result":{"content":[{"text":"..."}]}}
   ```

## Available Tools

Claude-flow exposes 70+ MCP tools including:

### Core Agent Management
- `swarm_init` - Initialize swarm topology
- `agent_spawn` - Create specialized agents
- `agent_list` - List active agents
- `agent_metrics` - Get performance metrics

### Task Orchestration
- `task_orchestrate` - Coordinate complex workflows
- `task_status` - Check execution status
- `task_results` - Get completion results

### Memory & Persistence
- `memory_usage` - Store/retrieve persistent data
- `memory_search` - Pattern-based search
- `memory_persist` - Cross-session persistence

### Neural & AI
- `neural_train` - Train patterns with WASM SIMD
- `neural_predict` - Make predictions
- `neural_patterns` - Analyze cognitive patterns

### Performance & Monitoring
- `swarm_status` - Monitor health
- `bottleneck_analyze` - Identify issues
- `performance_report` - Generate reports

## Configuration

### Environment Variables

No longer needed for basic operation! The stdio transport doesn't require:
- ~~CLAUDE_FLOW_HOST~~
- ~~CLAUDE_FLOW_PORT~~

### Requirements

Only requirement is having claude-flow available via npm:
```bash
npx claude-flow@alpha --version
```

## Usage Examples

### Initialize a Swarm

```rust
// Via ClaudeFlowActor
let params = InitializeSwarm {
    topology: "hierarchical".to_string(),
    max_agents: 8,
    strategy: Some("balanced".to_string()),
    enable_neural: true,
    agent_types: vec!["coordinator", "researcher", "coder", "tester"],
};
```

### List Active Agents

```rust
// Automatically handled by ClaudeFlowActor::GetActiveAgents
match claude_flow_addr.send(GetActiveAgents).await {
    Ok(Ok(agents)) => {
        // Process AgentStatus list
    }
}
```

## Error Handling

The stdio transport provides clear error messages:

1. **Process Spawn Failures**:
   - Missing claude-flow npm package
   - Permission issues
   - Invalid arguments

2. **Communication Failures**:
   - Broken pipes (process crashed)
   - Parse errors (invalid JSON)
   - Protocol errors (invalid MCP)

3. **Graceful Degradation**:
   - Falls back to mock data
   - Continues visualization
   - Logs errors without crashing

## Testing

Test the integration:

```bash
# In one terminal, watch logs
docker logs -f webxr

# In another, trigger swarm initialization
curl -X POST http://localhost:8080/api/bots/initialize-swarm \
  -H "Content-Type: application/json" \
  -d '{
    "topology": "hierarchical",
    "max_agents": 6,
    "agent_types": ["coordinator", "researcher", "coder", "tester"]
  }'

# Check agent status
curl http://localhost:8080/api/bots/status
```

## Migration from WebSocket

If migrating from the WebSocket-based approach:

1. Remove environment variables for CLAUDE_FLOW_HOST/PORT
2. Update ClaudeFlowClientBuilder to use `.use_stdio()`
3. Remove any WebSocket bridge dependencies
4. Test with local claude-flow installation

## Future Enhancements

1. **Process Pool**: Reuse claude-flow processes for better performance
2. **Binary Protocol**: Use MessagePack for faster serialization
3. **Streaming Updates**: Use server-sent events for real-time data
4. **Process Monitoring**: Health checks and automatic restarts