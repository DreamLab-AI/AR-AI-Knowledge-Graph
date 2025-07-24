#!/bin/bash
# Claude-Flow MCP Service Startup Script for PowerDev Container

echo "=== Claude-Flow MCP Service Startup Script ==="
echo "Container: PowerDev ($(hostname))"
echo "Network: Docker network at $(ip addr show eth0 | grep inet | awk '{print $2}')"
echo ""

# Configuration
CLAUDE_FLOW_DIR="/workspace/ext/claude-flow"
MCP_PORT=3000
MCP_HOST="0.0.0.0"  # Listen on all interfaces for container access

# Check if MCP is already running
if pgrep -f "mcp-server.js" > /dev/null; then
    echo "⚠️  MCP server is already running!"
    echo "Process: $(pgrep -f "mcp-server.js" -a)"
    echo ""
    echo "To restart, first stop the existing process:"
    echo "pkill -f mcp-server.js"
    exit 0
fi

# Navigate to claude-flow directory
cd "$CLAUDE_FLOW_DIR" || exit 1

# Set environment variables
export CLAUDE_FLOW_AUTO_ORCHESTRATOR=true
export CLAUDE_FLOW_NEURAL_ENABLED=true
export CLAUDE_FLOW_WASM_ENABLED=true
export MCP_MODE=server
export MCP_PORT=$MCP_PORT

echo "Starting Claude-Flow MCP Service..."
echo "Configuration:"
echo "  - Directory: $CLAUDE_FLOW_DIR"
echo "  - Port: $MCP_PORT"
echo "  - Host: $MCP_HOST"
echo "  - Transport: HTTP/WebSocket"
echo "  - Features: Auto-orchestrator, Neural, WASM"
echo ""

# Start MCP server in HTTP mode for WebSocket access
echo "Command: npx claude-flow mcp start --transport http --port $MCP_PORT --host $MCP_HOST --auto-orchestrator"
echo ""

# Start the service
npx claude-flow mcp start --transport http --port $MCP_PORT --host $MCP_HOST --auto-orchestrator

# Note: The above command will run in foreground. 
# For background execution, add & at the end or use --daemon flag