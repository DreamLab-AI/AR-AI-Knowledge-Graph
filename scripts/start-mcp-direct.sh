#!/bin/bash

# Start Claude Flow MCP server directly without WebSocket wrapper
# This provides the MCP service that the Rust backend expects

echo "Starting Claude Flow MCP server directly..."

# Kill any existing MCP processes
pkill -f "mcp-ws-relay" || true
pkill -f "mcp-http-wrapper" || true
pkill -f "claude-flow mcp" || true

# Wait for processes to stop
sleep 2

# Start Claude Flow MCP in server mode with HTTP transport
# This creates an HTTP API that the Rust backend can connect to
cd /workspace

echo "Starting MCP server on port 3000..."
npx claude-flow@alpha mcp start --server --port 3000 --host 0.0.0.0 > /tmp/mcp-direct.log 2>&1 &

sleep 3

# Check if it started
if ps aux | grep -v grep | grep "claude-flow mcp" > /dev/null; then
    echo "✅ MCP server started successfully"
    echo "Service available at http://0.0.0.0:3000"
    tail -10 /tmp/mcp-direct.log
else
    echo "❌ Failed to start MCP server"
    cat /tmp/mcp-direct.log
    exit 1
fi