#!/bin/bash

# MCP Health Check Script
# Checks all MCP servers and reports their status

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BLENDER_PORT=${BLENDER_MCP_PORT:-9876}
REVIT_PORT=${REVIT_MCP_PORT:-8080}
UNREAL_PORT=${UNREAL_MCP_PORT:-55557}
CLAUDE_FLOW_PORT=3000

# Remote host if configured
MCP_HOST=${REMOTE_MCP_HOST:-localhost}

# Function to check if a port is accessible
check_port() {
    local host=$1
    local port=$2
    local service=$3

    if nc -z -w 2 "$host" "$port" 2>/dev/null; then
        echo -e "${GREEN}✓ $service ($host:$port) is accessible${NC}"
        return 0
    else
        echo -e "${RED}✗ $service ($host:$port) is not accessible${NC}"
        return 1
    fi
}

# Function to check MCP endpoint
check_mcp_endpoint() {
    local host=$1
    local port=$2
    local tool=$3
    local service=$4

    # Create JSON-RPC request
    local request='{
        "jsonrpc": "2.0",
        "id": "'$(uuidgen)'",
        "method": "tools/call",
        "params": {
            "name": "'$tool'",
            "arguments": {}
        }
    }'

    # Make the request
    response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$request" \
        "http://$host:$port/api/mcp" \
        --max-time 5 2>/dev/null || echo "FAILED")

    if [[ "$response" == "FAILED" ]]; then
        echo -e "${RED}✗ $service MCP endpoint ($tool) failed${NC}"
        return 1
    elif echo "$response" | jq -e '.result' >/dev/null 2>&1; then
        echo -e "${GREEN}✓ $service MCP endpoint ($tool) responded successfully${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠ $service MCP endpoint ($tool) returned unexpected response${NC}"
        return 1
    fi
}

echo "=== MCP Server Health Check ==="
echo "Checking MCP servers at ${MCP_HOST}..."
echo ""

# Track overall health
overall_health=0

# Check basic connectivity
echo "## Basic Connectivity ##"
check_port "$MCP_HOST" "$BLENDER_PORT" "Blender MCP" || overall_health=$((overall_health + 1))
check_port "$MCP_HOST" "$REVIT_PORT" "Revit MCP" || overall_health=$((overall_health + 1))
check_port "$MCP_HOST" "$UNREAL_PORT" "Unreal MCP" || overall_health=$((overall_health + 1))
check_port "localhost" "$CLAUDE_FLOW_PORT" "Claude Flow" || overall_health=$((overall_health + 1))

echo ""
echo "## MCP Endpoint Tests ##"

# Check Claude Flow MCP endpoints
if check_port "localhost" "$CLAUDE_FLOW_PORT" "Claude Flow" >/dev/null 2>&1; then
    check_mcp_endpoint "localhost" "$CLAUDE_FLOW_PORT" "agents/list" "Claude Flow" || overall_health=$((overall_health + 1))
    check_mcp_endpoint "localhost" "$CLAUDE_FLOW_PORT" "analysis/token-usage" "Claude Flow" || overall_health=$((overall_health + 1))
    check_mcp_endpoint "localhost" "$CLAUDE_FLOW_PORT" "memory/query" "Claude Flow" || overall_health=$((overall_health + 1))
    check_mcp_endpoint "localhost" "$CLAUDE_FLOW_PORT" "system/health" "Claude Flow" || overall_health=$((overall_health + 1))
else
    echo -e "${YELLOW}⚠ Skipping Claude Flow endpoint tests (service not accessible)${NC}"
fi

echo ""
echo "## Summary ##"
if [ $overall_health -eq 0 ]; then
    echo -e "${GREEN}All health checks passed!${NC}"
    exit 0
else
    echo -e "${RED}$overall_health health checks failed${NC}"
    exit 1
fi