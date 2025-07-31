#!/bin/bash

# Test swarm initialization with correct payload

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Testing Swarm Initialization with Correct Payload${NC}"

# 1. Test with minimal required fields
echo -e "\n${YELLOW}1. Minimal payload test:${NC}"
curl -X POST http://localhost:3001/api/bots/initialize-swarm \
    -H "Content-Type: application/json" \
    -d '{
        "topology": "mesh",
        "maxAgents": 3,
        "strategy": "adaptive"
    }' \
    -s -w "\nHTTP Status: %{http_code}\n" \
    --max-time 10

# 2. Test with full payload (like the UI sends)
echo -e "\n${YELLOW}2. Full payload test (matching UI):${NC}"
RESPONSE=$(curl -X POST http://localhost:3001/api/bots/initialize-swarm \
    -H "Content-Type: application/json" \
    -d '{
        "topology": "mesh",
        "maxAgents": 8,
        "strategy": "adaptive",
        "enableNeural": true,
        "agentTypes": ["coordinator", "researcher", "coder", "analyst", "tester", "architect", "optimizer"],
        "customPrompt": "test"
    }' \
    -s -w "\nHTTP_STATUS:%{http_code}" \
    --max-time 30)

# Extract status code and body
HTTP_STATUS=$(echo "$RESPONSE" | grep -o "HTTP_STATUS:[0-9]*" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed 's/HTTP_STATUS:[0-9]*$//')

echo "Response Body:"
echo "$BODY" | jq . 2>/dev/null || echo "$BODY"
echo "HTTP Status: $HTTP_STATUS"

# 3. Check what's happening
echo -e "\n${YELLOW}3. Checking backend logs for errors:${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -iE "(initialize_swarm|error|failed)" | tail -10

# 4. Check if ClaudeFlowActor is even initialized
echo -e "\n${YELLOW}4. Checking if MCP/Claude Flow integration is enabled:${NC}"
docker exec logseq_spring_thing_webxr env | grep -E "(CLAUDE_FLOW|MCP)" || echo "No Claude Flow env vars found"

echo -e "\n${BLUE}Analysis:${NC}"
if [ "$HTTP_STATUS" == "200" ]; then
    echo -e "${GREEN}✓ Swarm initialization working!${NC}"
elif [ "$HTTP_STATUS" == "500" ]; then
    echo -e "${RED}✗ Backend error - check logs above${NC}"
elif [ "$HTTP_STATUS" == "504" ]; then
    echo -e "${RED}✗ Still timing out - ClaudeFlowActor issue${NC}"
    echo "The ClaudeFlowActor might not be initialized or is blocking"
else
    echo -e "${YELLOW}Status $HTTP_STATUS - see response above${NC}"
fi