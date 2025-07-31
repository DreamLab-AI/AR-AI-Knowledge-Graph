# Architecture Migration Guide

This guide helps developers migrate from the old single-graph architecture to the new parallel graphs architecture with integrated Claude Flow MCP support.

## Overview of Changes

### Old Architecture
- Single graph visualization (Logseq only)
- Direct WebSocket connections from frontend to MCP
- Stdio integration documentation in root docs
- No agent visualization support

### New Architecture
- Parallel graphs (Logseq + VisionFlow)
- MCP connections handled by backend only
- Comprehensive architecture docs
- Full agent visualization with physics

## Migration Steps

### 1. Update Graph Components

**Old Pattern:**
```typescript
// Direct graphDataManager usage
import { graphDataManager } from '@/features/graph/managers/graphDataManager';

const data = await graphDataManager.fetchInitialData();
graphDataManager.updateNodePositions(binaryData);
```

**New Pattern:**
```typescript
// Use parallel graphs hook
import { useParallelGraphs } from '@/features/graph/hooks/useParallelGraphs';

const { 
  logseqPositions, 
  visionFlowPositions,
  enableLogseq,
  enableVisionFlow 
} = useParallelGraphs({
  enableLogseq: true,
  enableVisionFlow: true
});
```

### 2. Update WebSocket Connections

**Old Pattern:**
```typescript
// Direct MCP WebSocket from frontend
const mcpSocket = new WebSocket('ws://localhost:3000/mcp');
mcpSocket.onmessage = (event) => {
  // Process MCP data
};
```

**New Pattern:**
```typescript
// Use REST API for agent data
const response = await fetch('/api/bots/agents');
const { agents } = await response.json();

// Or use MCPWebSocketService
import { mcpWebSocketService } from '@/features/bots/services/MCPWebSocketService';
const agents = await mcpWebSocketService.getAgents();
```

### 3. Update Physics Workers

**Old Pattern:**
```typescript
// Single physics worker
import { graphWorkerProxy } from '@/features/graph/managers/graphWorkerProxy';
graphWorkerProxy.updatePositions(nodes);
```

**New Pattern:**
```typescript
// Graph type specific workers
// Logseq uses graphWorkerProxy (unchanged)
// VisionFlow uses botsPhysicsWorker
import { botsPhysicsWorker } from '@/features/bots/workers/botsPhysicsWorker';
botsPhysicsWorker.updateAgents(agents);
botsPhysicsWorker.updateEdges(edges);
```

### 4. Update Backend Integration

**Old Pattern:**
```rust
// Direct stdio integration in main code
let client = ClaudeFlowClientBuilder::new()
    .use_stdio()
    .build()
    .await?;
```

**New Pattern:**
```rust
// Use ClaudeFlowActor
let claude_flow_actor = ClaudeFlowActor::new(graph_service_addr).await;
ctx.spawn(claude_flow_actor);

// Send messages to actor
claude_flow_addr.send(InitializeSwarm {
    topology: "hierarchical".to_string(),
    max_agents: 8,
    agent_types: vec!["coordinator", "researcher", "coder"],
    enable_neural: true,
    custom_prompt: None,
}).await?;
```

### 5. Update Configuration

**Old Pattern:**
```typescript
// Single graph config
const graphConfig = {
  physics: {
    springStrength: 0.2,
    damping: 0.95
  }
};
```

**New Pattern:**
```typescript
// Per-graph-type config
const DEFAULT_GRAPH_CONFIG = {
  logseq: {
    physics: {
      springStrength: 0.2,
      damping: 0.95,
      // ... Logseq specific
    }
  },
  visionflow: {
    physics: {
      springStrength: 0.3,
      damping: 0.95,
      // ... VisionFlow specific
    }
  }
};
```

## API Changes

### Deprecated Endpoints
- `WS /mcp` - Direct MCP WebSocket (removed)
- `POST /api/claude-flow/*` - Direct Claude Flow proxy (removed)

### New Endpoints
- `GET /api/bots/agents` - Get agent list
- `POST /api/bots/spawn` - Spawn new agent
- `POST /api/bots/swarm/init` - Initialize swarm
- `DELETE /api/bots/agent/:id` - Terminate agent

## Component Updates

### Affected Components

1. **Graph Visualization Components**
   - Update to use `useParallelGraphs` hook
   - Add graph type selector UI

2. **Settings Panel**
   - Add VisionFlow enable/disable toggle
   - Add agent visualization settings

3. **Performance Monitoring**
   - Update to track both graph types
   - Add agent-specific metrics

### New Components

1. **Agent Visualization**
   - `BotsRenderer` - 3D agent rendering
   - `AgentInspector` - Agent details panel
   - `CommunicationFlow` - Message visualization

2. **Swarm Controls**
   - `SwarmInitializer` - UI for swarm setup
   - `AgentSpawner` - UI for adding agents
   - `TopologySelector` - Choose swarm topology

## Testing Updates

### Unit Tests
```typescript
// Test parallel graphs isolation
it('should maintain separate state for each graph type', () => {
  const { result } = renderHook(() => useParallelGraphs());
  
  act(() => {
    result.current.enableLogseq(true);
    result.current.enableVisionFlow(false);
  });
  
  expect(result.current.isLogseqEnabled).toBe(true);
  expect(result.current.isVisionFlowEnabled).toBe(false);
});
```

### Integration Tests
```typescript
// Test backend MCP relay
it('should relay agent data from backend', async () => {
  const response = await fetch('/api/bots/agents');
  const data = await response.json();
  
  expect(data.agents).toBeDefined();
  expect(data.agents.length).toBeGreaterThan(0);
});
```

## Performance Considerations

1. **Memory Usage**: Two physics simulations run in parallel
   - Monitor total memory usage
   - Implement cleanup for inactive graphs

2. **CPU Usage**: Multiple web workers
   - Use worker pooling if needed
   - Implement LOD for distant nodes

3. **Network Traffic**: Polling vs WebSocket
   - Current: REST polling every 10s
   - Future: WebSocket push updates

## Rollback Plan

If issues arise, you can disable VisionFlow:

```typescript
// Disable in settings
const settings = {
  features: {
    parallelGraphs: false,  // Disable parallel graphs
    agentVisualization: false  // Disable agent viz
  }
};
```

Or via environment:
```bash
DISABLE_VISIONFLOW=true npm start
```

## Common Issues

### 1. "Cannot find module" Errors
Update import paths:
```typescript
// Old
import { mcpService } from '@/services/mcpService';

// New
import { mcpWebSocketService } from '@/features/bots/services/MCPWebSocketService';
```

### 2. WebSocket Connection Failures
Frontend no longer connects to MCP directly. Check backend logs:
```bash
RUST_LOG=debug cargo run
```

### 3. Missing Agent Data
Ensure ClaudeFlowActor is running:
```bash
# Check actor logs
grep "ClaudeFlowActor" /app/logs/visionflow.log
```

## Support Resources

- Architecture Docs: `/docs/architecture/`
- Example Code: `/examples/parallel-graphs/`
- Issue Tracker: Create issues with tag `migration`

## Timeline

1. **Phase 1** (Completed): Core parallel graphs implementation
2. **Phase 2** (Completed): MCP backend relay
3. **Phase 3** (Current): Documentation and migration support
4. **Phase 4** (Future): WebSocket push updates