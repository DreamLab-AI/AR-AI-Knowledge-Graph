#!/bin/bash

# Start Claude Flow MCP services inside the powerdev container
# This makes the MCP service available at ws://powerdev:3000/ws for other containers

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MCP_PORT=${MCP_PORT:-3000}
MCP_HOST=${MCP_HOST:-0.0.0.0}  # Listen on all interfaces for Docker networking

echo "Starting Claude Flow MCP services in powerdev container..."
echo "Container hostname: $(hostname)"
echo "Container IP: $(hostname -i)"
echo "Service will be available at ws://powerdev:${MCP_PORT}/ws"

# Change to the directory containing the MCP relay
cd "${SCRIPT_DIR}/src"

# Check if the relay script exists
if [ ! -f "mcp-ws-relay.js" ]; then
    echo "Error: mcp-ws-relay.js not found in ${SCRIPT_DIR}/src"
    echo "Please ensure the WebSocket relay script exists"
    exit 1
fi

# Check if Node.js is available
if ! command -v node &> /dev/null; then
    echo "Error: Node.js is not installed"
    exit 1
fi

# Kill any existing MCP processes on the port
echo "Checking for existing processes on port ${MCP_PORT}..."
if lsof -Pi :${MCP_PORT} -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "Killing existing process on port ${MCP_PORT}..."
    lsof -Pi :${MCP_PORT} -sTCP:LISTEN -t | xargs kill -9 2>/dev/null || true
    sleep 1
fi

# Start the MCP WebSocket relay
echo "Starting MCP WebSocket relay..."
echo "Listening on ${MCP_HOST}:${MCP_PORT}"

# Export environment variables for the relay
export MCP_PORT=${MCP_PORT}
export MCP_HOST=${MCP_HOST}

# Start the relay in the foreground
exec node mcp-ws-relay.js