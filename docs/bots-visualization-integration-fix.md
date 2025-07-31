# Bots Visualization Integration Fix

## Problem
The bots visualization was showing "MOCK" data and not displaying the actual agents spawned by claude-flow, even though the backend logs showed successful agent spawning.

## Root Causes

### 1. Data Flow Disconnect
The `GraphServiceActor` was updating its internal `bots_graph_data`, but the HTTP handlers were reading from a separate static `BOTS_GRAPH`, causing the data to never reach the client.

### 2. Missing REST API Call
The `BotsWebSocketIntegration.requestInitialData()` method wasn't actually fetching bots data from the REST API endpoint.

### 3. No Event Listener
The `BotsVisualization` component wasn't listening for bots data updates from the WebSocket integration.

## Solutions Implemented

### 1. Added GetBotsGraphData Message (Backend)
```rust
// In messages.rs
#[derive(Message)]
#[rtype(result = "GraphData")]
pub struct GetBotsGraphData;

// In graph_actor.rs
impl Handler<GetBotsGraphData> for GraphServiceActor {
    type Result = GraphData;
    
    fn handle(&mut self, _msg: GetBotsGraphData, _ctx: &mut Context<Self>) -> Self::Result {
        self.bots_graph_data.clone()
    }
}
```

### 2. Updated Bots Handler to Query Actor
```rust
// In bots_handler.rs
if let Some(graph_actor) = &state.graph_actor {
    match graph_actor.send(GetBotsGraphData).await {
        Ok(graph_data) => {
            return HttpResponse::Ok().json(graph_data);
        }
        Err(e) => {
            warn!("Failed to get bots data from GraphServiceActor: {}", e);
        }
    }
}
```

### 3. Fixed REST API Fetching (Frontend)
```typescript
// In BotsWebSocketIntegration.ts
async requestInitialData() {
    // ... existing code ...
    
    // Fetch bots data via REST API from the backend
    try {
        const { apiService } = await import('../../../services/apiService');
        const botsData = await apiService.get('/api/bots/data');
        
        if (botsData && botsData.nodes) {
            this.emit('bots-data', botsData);
        }
    } catch (error) {
        logger.error('Failed to fetch bots data:', error);
    }
}
```

### 4. Added Event Listener and Polling
```typescript
// In BotsVisualization.tsx
// Listen for bots data updates
const unsubscribe = botsWebSocketIntegration.on('bots-data', (data) => {
    if (data && data.nodes && data.nodes.length > 0) {
        setDataSource('live');
        processBotsData(data);
    }
});

// Poll for updates periodically
useEffect(() => {
    const pollInterval = setInterval(async () => {
        const data = await apiService.get('/api/bots/data');
        if (data && data.nodes && data.nodes.length > 0 && !data._isMock) {
            processBotsData(data);
        }
    }, 5000);
    
    return () => clearInterval(pollInterval);
}, [dataSource]);
```

## Data Flow After Fix

1. **Claude Flow spawns agents** → Sends to ClaudeFlowActor via stdio
2. **ClaudeFlowActor polls** → Sends UpdateBotsGraph to GraphServiceActor
3. **GraphServiceActor stores** → Updates internal bots_graph_data
4. **HTTP Handler queries** → Sends GetBotsGraphData to GraphServiceActor
5. **GraphServiceActor responds** → Returns current bots data
6. **Client receives** → Updates visualization with real agent data

## Testing

1. Spawn agents through the UI
2. Check that the visualization shows real agents (not MOCK)
3. Verify agent types and properties match what was spawned
4. Confirm real-time position updates work

## Next Steps

1. Implement WebSocket push updates for real-time data
2. Add bidirectional position/velocity sync with GPU physics
3. Enhance visualization with agent communication patterns