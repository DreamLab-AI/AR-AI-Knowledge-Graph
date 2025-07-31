#!/bin/bash

# Diagnose why swarm initialization times out

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Swarm Initialization Timeout Diagnosis${NC}"
echo "The backend IS reachable but timing out on swarm init"
echo

# 1. Check if MCP relay is actually running and accessible
echo -e "${YELLOW}1. MCP Relay Status:${NC}"
echo -n "MCP process in powerdev: "
docker exec powerdev ps aux | grep -q "mcp-ws-relay" && echo -e "${GREEN}✓ Running${NC}" || echo -e "${RED}✗ Not running${NC}"

echo -n "MCP relay port 3000: "
docker exec powerdev netstat -tlnp 2>/dev/null | grep -q ":3000" && echo -e "${GREEN}✓ Listening${NC}" || echo -e "${RED}✗ Not listening${NC}"

# 2. Test MCP connectivity from Rust container
echo -e "\n${YELLOW}2. MCP Connectivity from Rust Backend:${NC}"
echo "Testing connection to powerdev:3000..."
docker exec logseq_spring_thing_webxr sh -c 'nc -zv powerdev 3000 2>&1' || echo "Connection failed"

# 3. Check ClaudeFlowActor status in logs
echo -e "\n${YELLOW}3. ClaudeFlowActor Status (last 10 entries):${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -i "claudeflow" | tail -10

# 4. Check for timeout/connection errors
echo -e "\n${YELLOW}4. Recent Timeout/Connection Errors:${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -iE "(timeout|connection refused|failed to connect)" | tail -5

# 5. Try to start MCP relay if not running
echo -e "\n${YELLOW}5. Ensuring MCP Relay is Started:${NC}"
curl -X POST http://localhost:3001/api/mcp/start -s -w "\nHTTP Status: %{http_code}\n" || echo "Failed to start MCP"

# 6. Test with shorter timeout
echo -e "\n${YELLOW}6. Testing Swarm Init with 10s timeout:${NC}"
curl -X POST http://localhost:3001/api/bots/initialize-swarm \
    -H "Content-Type: application/json" \
    -d '{"topology": "mesh", "maxAgents": 3}' \
    -s -w "\nHTTP Status: %{http_code}\n" \
    --max-time 10 || echo -e "${RED}Still timing out after 10s${NC}"

# 7. Suggestions
echo -e "\n${BLUE}Diagnosis Summary:${NC}"
if docker exec powerdev ps aux | grep -q "mcp-ws-relay"; then
    echo "✓ MCP relay is running"
else
    echo "✗ MCP relay NOT running - this is likely the cause"
    echo "  Fix: The backend should auto-start it via the MCP health endpoint"
fi

echo -e "\n${YELLOW}The 504 timeout suggests:${NC}"
echo "1. Backend receives the request (good!)"
echo "2. ClaudeFlowActor tries to connect to MCP"
echo "3. Connection hangs or times out"
echo "4. nginx gives up after 2 minutes"

echo -e "\n${YELLOW}Possible Solutions:${NC}"
echo "1. Check if ClaudeFlowActor is initialized properly"
echo "2. Verify MCP relay is accessible from Rust container"
echo "3. Check for firewall/network issues between containers"
echo "4. The ClaudeFlowActor might need to handle connection failures better"