# Bots Visualization Architecture

This document describes the architecture for visualizing AI agents (bots) in the VisionFlow 3D environment.

## Overview

The bots visualization system displays Claude Flow agents as interactive 3D nodes with connections representing communication patterns. It runs parallel to the Logseq graph visualization using the parallel graphs architecture.

## Core Components

### 1. Data Types

```typescript
interface BotsAgent {
  id: string;
  name: string;
  type: 'coordinator' | 'researcher' | 'coder' | 'analyst' | 'tester' | ...;
  status: 'active' | 'idle' | 'busy' | 'error';
  capabilities: string[];
  metrics: {
    tasksActive: number;
    tasksCompleted: number;
    successRate: number;
    avgResponseTime: number;
  };
  position?: Vector3;  // 3D position
}

interface BotsEdge {
  id: string;
  source: string;      // Agent ID
  target: string;      // Agent ID
  dataVolume: number;  // Bytes transferred
  messageCount: number;
  lastMessageTime: number;
}

interface BotsCommunication {
  id: string;
  timestamp: string;
  sender: string;
  receivers: string[];
  messageType: string;
  metadata: {
    size: number;
    tokens?: number;
    model?: string;
  };
}
```

### 2. Physics Simulation

The `BotsPhysicsWorker` runs a dedicated physics simulation:

```typescript
class BotsPhysicsWorker {
  private positions: Map<string, Vector3>;
  private velocities: Map<string, Vector3>;
  private config: BotsVisualConfig;
  
  updateAgents(agents: BotsAgent[]) {
    // Add/update agents in simulation
  }
  
  updateEdges(edges: BotsEdge[]) {
    // Update connection forces
  }
  
  simulateStep(deltaTime: number) {
    // Apply forces and update positions
  }
}
```

**Physics Configuration:**
```typescript
{
  springStrength: 0.3,      // Edge attraction
  damping: 0.95,            // Velocity damping
  repulsionStrength: 0.8,   // Node repulsion
  centerForce: 0.002,       // Center attraction
  maxVelocity: 0.5,         // Speed limit
  linkDistance: 3.0         // Ideal edge length
}
```

### 3. Visual Representation

Agents are rendered as 3D objects with visual indicators:

```typescript
function renderAgent(agent: BotsAgent) {
  const geometry = getAgentGeometry(agent.type);  // Shape by type
  const material = getAgentMaterial(agent.status); // Color by status
  
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.copy(getAgentPosition(agent.id));
  
  // Add labels, icons, metrics display
  addAgentLabels(mesh, agent);
  addMetricsBadges(mesh, agent.metrics);
  
  return mesh;
}
```

**Visual Encoding:**
- **Shape**: Agent type (cube=coordinator, sphere=researcher, etc.)
- **Color**: Status (green=active, yellow=idle, red=error)
- **Size**: Task load or importance
- **Glow**: Recent activity
- **Connections**: Communication volume

### 4. Data Flow Pipeline

```
Backend (ClaudeFlowActor)
    ↓ Agent data
REST API (/api/bots/agents)
    ↓ JSON response
Frontend (MCPWebSocketService)
    ↓ Process communications into edges
BotsPhysicsWorker
    ↓ Physics simulation
ParallelGraphCoordinator
    ↓ Position updates
3D Renderer
    ↓ Visual output
User Interface
```

### 5. Mock Data System

For development and demos, a comprehensive mock data system:

```typescript
class MockBotsDataProvider {
  private mockAgents: BotsAgent[] = [
    {
      id: "coordinator-001",
      name: "System Coordinator",
      type: "coordinator",
      status: "active",
      capabilities: ["orchestration", "task-management"],
      metrics: {
        tasksActive: 3,
        tasksCompleted: 15,
        successRate: 100,
        avgResponseTime: 250
      }
    },
    // ... more mock agents
  ];
  
  generateCommunications(count: number): BotsCommunication[] {
    // Generate realistic communication patterns
  }
}
```

## Integration Points

### 1. Backend Integration

The `GraphServiceActor` receives agent updates:

```rust
impl Handler<UpdateBotsGraph> for GraphServiceActor {
    fn handle(&mut self, msg: UpdateBotsGraph, _ctx: &mut Context<Self>) {
        // Store agent data
        self.bots_agents = msg.agents;
        
        // Notify connected clients
        self.broadcast_bots_update();
    }
}
```

### 2. REST API Endpoints

```
GET /api/bots/agents          # Get all agents
GET /api/bots/communications  # Get recent communications
GET /api/bots/metrics         # Get performance metrics
POST /api/bots/spawn          # Create new agent
DELETE /api/bots/agent/:id    # Remove agent
```

### 3. Frontend Services

**MCPWebSocketService**: Manages agent data and polling
```typescript
class MCPWebSocketService {
  async connect() {
    // Start polling backend for updates
    this.pollInterval = setInterval(() => {
      this.fetchAgentsFromBackend();
    }, 10000);
  }
}
```

**BotsWebSocketIntegration**: Coordinates data sources
```typescript
class BotsWebSocketIntegration {
  // Note: Despite the name, uses REST API not WebSocket
  // MCP connections are backend-only
}
```

## Performance Optimizations

### 1. Differential Updates
Only changed agents are sent from backend to frontend

### 2. Physics LOD (Level of Detail)
Distant agents use simplified physics calculations

### 3. Render Culling
Only visible agents are rendered each frame

### 4. Data Aggregation
Communications are aggregated into edges to reduce data volume

## User Interactions

### 1. Agent Selection
- Click to select and view detailed metrics
- Double-click to focus camera on agent

### 2. Filtering
- Filter by agent type
- Filter by status
- Show/hide inactive agents

### 3. Time Controls
- Replay communication history
- Adjust simulation speed
- Pause/resume physics

## Configuration

### Visual Themes

```typescript
const AGENT_THEMES = {
  default: {
    coordinator: { color: 0x4A90E2, shape: 'cube' },
    researcher: { color: 0x7ED321, shape: 'sphere' },
    coder: { color: 0xF5A623, shape: 'octahedron' },
    analyst: { color: 0xBD10E0, shape: 'tetrahedron' },
    tester: { color: 0x50E3C2, shape: 'icosahedron' }
  },
  dark: { /* ... */ },
  highContrast: { /* ... */ }
};
```

### Physics Presets

```typescript
const PHYSICS_PRESETS = {
  tight: { springStrength: 0.5, linkDistance: 2.0 },
  loose: { springStrength: 0.1, linkDistance: 5.0 },
  organic: { springStrength: 0.3, damping: 0.8 },
  grid: { springStrength: 0.8, centerForce: 0.0 }
};
```

## Future Enhancements

1. **Real-time WebSocket**: Replace polling with push updates
2. **Agent Trails**: Visualize movement history
3. **Communication Replay**: Replay message sequences
4. **3D Charts**: Embed performance metrics in 3D space
5. **VR Interaction**: Manipulate agents in VR
6. **Swarm Patterns**: Visualize emergent swarm behaviors
7. **Cross-Graph Links**: Connect agents to Logseq nodes