#!/bin/bash

# Check why MCP routes return 404

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Checking MCP Route Registration${NC}"

# Test all MCP-related endpoints
echo -e "\n${YELLOW}Testing MCP endpoints:${NC}"

endpoints=(
    "/api/mcp/health"
    "/api/mcp/start"
    "/api/mcp/logs"
)

for endpoint in "${endpoints[@]}"; do
    echo -n "$endpoint: "
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3001$endpoint)
    case $STATUS in
        200) echo -e "${GREEN}200 OK${NC}" ;;
        404) echo -e "${RED}404 Not Found${NC}" ;;
        *) echo -e "${YELLOW}$STATUS${NC}" ;;
    esac
done

echo -e "\n${YELLOW}Checking route configuration in logs:${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -i "configuring.*routes" | tail -10

echo -e "\n${YELLOW}Looking for MCP handler registration:${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -iE "(mcp|health.*handler)" | tail -5

echo -e "\n${BLUE}Note:${NC}"
echo "If all MCP routes return 404, the mcp_health_handler might not be"
echo "registered properly in main.rs route configuration."