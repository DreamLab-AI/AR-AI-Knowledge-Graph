#!/bin/bash
# Script to initialize MCP from the host system

echo "Initializing Claude Flow MCP service..."

# Check if powerdev container is running
if ! docker ps | grep -q powerdev; then
    echo "Error: powerdev container is not running"
    exit 1
fi

# Kill any existing MCP processes
echo "Stopping any existing MCP processes..."
docker exec powerdev pkill -f mcp-ws-relay || true
sleep 2

# Start the MCP WebSocket relay
echo "Starting MCP WebSocket relay on port 3000..."
docker exec -d powerdev bash -c "cd /workspace/ext/src && node mcp-ws-relay.js > /var/log/mcp-ws-relay.log 2>&1"

# Wait for the service to start
echo "Waiting for MCP service to start..."
sleep 5

# Check if the service is listening
if docker exec powerdev netstat -tulpn 2>/dev/null | grep -q ":3000"; then
    echo "✅ MCP WebSocket relay is running on port 3000"
    
    # Restart the webxr container to reconnect
    echo "Restarting webxr container to establish connection..."
    docker restart logseq_spring_thing_webxr
    
    echo "✅ Complete! MCP service initialized and Rust backend restarted."
else
    echo "❌ Failed to start MCP WebSocket relay"
    echo "Check logs with: docker exec powerdev cat /var/log/mcp-ws-relay.log"
    exit 1
fi