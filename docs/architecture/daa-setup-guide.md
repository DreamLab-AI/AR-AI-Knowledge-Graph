# Dynamic Agent Architecture (DAA) Setup Guide

## Architecture Overview

- **powerdev container**: Runs Claude Flow MCP with full DAA capabilities
- **logseq container**: Connects to powerdev:3000 via WebSocket ONLY

## Current Status

The MCP WebSocket relay (`mcp-ws-relay.js`) is running in the powerdev container and bridges:
- WebSocket protocol (what Rust expects) on port 3000
- Stdio protocol (how Claude Flow MCP runs)

## Available DAA Tools via MCP

The following 87 tools are available through the MCP interface:

### 🐝 Swarm Orchestration (15 tools)
- `swarm_init` - Initialize swarm with topology
- `agent_spawn` - Create specialized agents  
- `task_orchestrate` - Orchestrate workflows
- `swarm_status` - Check swarm status
- `agent_list` - List active agents
- `agent_metrics` - Get agent performance
- `swarm_monitor` - Real-time monitoring
- `topology_optimize` - Auto-optimize topology
- `load_balance` - Distribute tasks
- `coordination_sync` - Sync coordination
- `swarm_scale` - Scale agent count
- `swarm_destroy` - Shutdown swarm
- `agent_list` - List agents and capabilities
- `agent_metrics` - Performance metrics
- `swarm_monitor` - Real-time monitoring

### 🤖 Dynamic Agent Architecture (6 tools)
- `daa_agent_create` - Create specialized agents with resources
- `daa_capability_match` - Match capabilities to tasks
- `daa_resource_alloc` - Allocate resources dynamically
- `daa_lifecycle_manage` - Manage agent lifecycle
- `daa_communication` - Inter-agent messaging
- `daa_consensus` - Democratic decision making

### 🧠 Neural & Cognitive (12 tools)
- `neural_train` - Train neural patterns
- `neural_predict` - Make predictions
- `neural_status` - Check neural status
- `pattern_recognize` - Pattern recognition
- `cognitive_analyze` - Analyze behaviors
- `learning_adapt` - Adaptive learning
- `neural_compress` - Model compression
- `ensemble_create` - Create ensembles
- `transfer_learn` - Transfer learning
- `neural_explain` - AI explainability
- `neural_patterns` - Analyze patterns
- `model_load/save` - Model persistence

### 💾 Memory Management (10 tools)  
- `memory_usage` - Store/retrieve with TTL
- `memory_search` - Pattern search
- `memory_persist` - Cross-session persistence
- `memory_namespace` - Namespace management
- `memory_backup` - Backup stores
- `memory_restore` - Restore from backup
- `memory_compress` - Compress data
- `memory_sync` - Sync instances
- `memory_analytics` - Usage analytics
- `cache_manage` - Cache management

## How to Use DAA Features

Since we're in the logseq container, all DAA operations must go through the Rust backend which connects to powerdev:3000.

### Example: Creating a Specialized Agent

The Rust backend can call the MCP tools via the WebSocket connection:

```rust
// In ClaudeFlowActor
let params = json!({
    "name": "daa_agent_create",
    "arguments": {
        "agent_type": "specialized-researcher",
        "capabilities": ["deep-analysis", "pattern-recognition"],
        "cognitivePattern": "lateral",
        "enableMemory": true,
        "learningRate": 0.8
    }
});
```

### Example: Initializing a Swarm with DAA

```rust
// Initialize swarm
let swarm_params = json!({
    "name": "swarm_init",
    "arguments": {
        "topology": "hierarchical",
        "maxAgents": 8,
        "strategy": "adaptive"
    }
});

// Then create DAA agents
let daa_params = json!({
    "name": "daa_agent_create",
    "arguments": {
        "id": "research-lead",
        "capabilities": ["research", "analysis", "coordination"],
        "cognitivePattern": "systems",
        "enableMemory": true
    }
});
```

## Testing DAA Features

From the powerdev container, you can test DAA features directly:

```bash
# In powerdev container
cd /workspace/ext/scripts
node test-claude-flow-connection.js

# This will show all 87 available tools including DAA tools
```

## Important Notes

1. **No Claude Flow in logseq container** - All Claude Flow operations happen in powerdev
2. **WebSocket relay required** - The mcp-ws-relay.js bridges protocols
3. **Persistent memory** - Uses SQLite database at `.swarm/memory.db`
4. **Real-time updates** - WebSocket enables real-time agent updates

## Troubleshooting

If the connection hangs after initialization:
1. Check the MCP relay is running: `netstat -tulpn | grep 3000`
2. Monitor relay logs: `tail -f /tmp/mcp-ws-relay.log`
3. Verify WebSocket responses have matching request IDs
4. Ensure the Rust backend is properly parsing MCP tool responses