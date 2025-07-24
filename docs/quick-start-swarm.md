# Quick Start: Spawning a Claude Flow Hive Mind

## Prerequisites

1. Ensure the backend server is running with the latest code
2. Claude Flow MCP server should be accessible (powerdev container)
3. VisionFlow UI is loaded in your browser

## Steps to Spawn a Hive Mind

### 1. Open the Control Panel
Look for the control panel in the top-left corner of the VisionFlow interface.

### 2. Click "Initialize Swarm"
When no agents are active, you'll see an "Initialize Swarm" button in the VisionFlow (MCP) section.

### 3. Configure Your Hive Mind

#### Required Fields:
- **Task Description**: What you want the hive mind to accomplish
  - Example: "Build a REST API with user authentication"
  - Example: "Create comprehensive documentation for this codebase"
  - Example: "Build a boilerplate LaTeX contract document"

#### Configuration Options:
- **Topology**: Choose how agents communicate
  - `mesh` - All agents can communicate (best for collaboration)
  - `hierarchical` - Structured command chain
  - `ring` - Sequential processing
  - `star` - Central coordinator

- **Maximum Agents**: 3-20 agents (default: 8)

- **Agent Types**: Select which specialists to spawn
  - Coordinator - Orchestrates the team
  - Researcher - Gathers information
  - Coder - Implements solutions
  - Analyst - Analyzes data and patterns
  - Tester - Validates implementations
  - Architect - Designs systems
  - Optimizer - Improves performance
  - Reviewer - Reviews code
  - Documenter - Creates documentation

- **Neural Enhancement**: Enable WASM-accelerated AI

### 4. Click "Spawn Hive Mind"
The system will:
1. Connect to Claude Flow MCP server
2. Initialize the swarm with your configuration
3. Spawn the selected agents
4. Apply your task to the hive mind
5. Begin visualization of agent activity

## Troubleshooting

### 404 Error When Spawning
The server needs to be restarted after adding new endpoints:
```bash
# If using supervisorctl
supervisorctl restart webxr

# If running directly
# Stop the server (Ctrl+C) and restart:
cargo run --features gpu
```

### No Agents Appearing
1. Check browser console for errors
2. Verify Claude Flow is running in powerdev container
3. Check that the MCP WebSocket is connected (should show "MCP relay connection established")

### Connection Issues
The MCP service will try multiple endpoints:
- `ws://192.168.0.51:3001/ws/mcp`
- `ws://192.168.0.51/ws/mcp`
- `ws://localhost:3001/ws/mcp`
- `ws://192.168.0.51:3001/wss` (usually works)

## Example Tasks for Testing

### Simple Tasks:
- "Create a hello world REST API"
- "Write unit tests for the math module"
- "Document the authentication system"

### Complex Tasks:
- "Build a full-stack application with React frontend and Express backend, including user authentication, database integration, and REST API"
- "Analyze the entire codebase and create a comprehensive architectural documentation with diagrams"
- "Refactor the legacy module to use modern TypeScript patterns and add full test coverage"

### Specialized Tasks:
- "Build a boilerplate LaTeX contract document and compile it to PDF"
- "Create a machine learning pipeline for text classification"
- "Design and implement a microservices architecture for an e-commerce platform"

## Monitoring Progress

Once spawned, you can monitor the hive mind through:
1. **3D Visualization** - See agents as nodes in the graph
2. **Control Panel Stats** - View agent count, links, and tokens
3. **Console Logs** - Check for detailed agent activity
4. **Backend Logs** - Monitor Claude Flow interactions

## Next Steps

After spawning your hive mind:
1. Watch the visualization update as agents work
2. Monitor the console for task progress
3. Agents will coordinate through the MCP protocol
4. Results will be reflected in the visualization

Remember: The actual work is performed by Claude Flow in the powerdev container. The visualization shows the coordination and activity of the agents as they work on your task.