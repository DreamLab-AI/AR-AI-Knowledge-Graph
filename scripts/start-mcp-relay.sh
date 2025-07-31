#!/bin/bash

# MCP WebSocket Relay Startup Script
# This script ensures the Claude Flow MCP WebSocket relay is running

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$(cd "${SCRIPT_DIR}/../src" && pwd)"
MCP_PORT=${MCP_PORT:-3002}
MCP_HOST=${MCP_HOST:-0.0.0.0}

echo "Starting MCP WebSocket relay..."
echo "Service will be available at ws://${MCP_HOST}:${MCP_PORT}"

# Change to the source directory
cd "${SRC_DIR}"

# Check if the relay script exists
if [ ! -f "mcp-ws-relay.js" ]; then
    echo "Error: mcp-ws-relay.js not found in ${SRC_DIR}"
    exit 1
fi

# Kill any existing process
if lsof -Pi :${MCP_PORT} -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "Killing existing process on port ${MCP_PORT}..."
    lsof -Pi :${MCP_PORT} -sTCP:LISTEN -t | xargs kill -9 2>/dev/null || true
fi

# Start the relay in the foreground
exec node mcp-ws-relay.js