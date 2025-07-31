#!/bin/bash

# Fix swarm initialization timeout issue

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Swarm Initialization Timeout Fix${NC}"

echo -e "\n${YELLOW}Problem Summary:${NC}"
echo "1. ClaudeFlowActor is not responding to messages"
echo "2. The initialize_swarm handler hangs on actor.send()"
echo "3. MCP health endpoints are not registered (404)"

echo -e "\n${YELLOW}Checking ClaudeFlowActor initialization:${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -E "(ClaudeFlowActor|Claude Flow)" | tail -20

echo -e "\n${YELLOW}Checking for actor panic/errors:${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -iE "(panic|actor.*failed|mailbox)" | tail -10

echo -e "\n${BLUE}Solutions:${NC}"
echo -e "\n${GREEN}1. Quick Fix - Restart the container:${NC}"
echo "   docker-compose restart"
echo "   This will reinitialize all actors"

echo -e "\n${GREEN}2. Check if ClaudeFlowActor is conditional:${NC}"
echo "   The actor might only initialize if CLAUDE_FLOW_HOST is set"
echo "   Your env shows: CLAUDE_FLOW_HOST=powerdev ✓"

echo -e "\n${GREEN}3. Temporary workaround - Use mock data:${NC}"
echo "   Since MCP_RELAY_FALLBACK_TO_MOCK=true is set,"
echo "   the system should fall back to mock data if MCP fails"

echo -e "\n${YELLOW}Testing if restart helps:${NC}"
echo "After restarting, the initialization should either:"
echo "- Work (if ClaudeFlowActor initializes properly)"
echo "- Return immediately with mock data (if actor fails)"
echo "- But NOT hang indefinitely"

echo -e "\n${YELLOW}The hanging suggests:${NC}"
echo "- ClaudeFlowActor was created (claude_flow_addr is Some)"
echo "- But the actor is deadlocked or crashed"
echo "- The send() waits forever for a response"