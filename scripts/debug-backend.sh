#!/bin/bash

# Debug why backend isn't listening on any port

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Backend Debug${NC}"

# 1. Check if backend process exists
echo -e "\n${YELLOW}1. Backend process:${NC}"
docker exec logseq_spring_thing_webxr ps aux | grep -E "(webxr|target)" | grep -v grep || echo "No backend process found"

# 2. Check last 20 lines of logs
echo -e "\n${YELLOW}2. Recent backend logs:${NC}"
docker logs logseq_spring_thing_webxr --tail 20 2>&1

# 3. Check environment variables
echo -e "\n${YELLOW}3. Port configuration:${NC}"
docker exec logseq_spring_thing_webxr sh -c 'echo "SYSTEM_NETWORK_PORT=$SYSTEM_NETWORK_PORT"'

# 4. Check if it's a startup failure
echo -e "\n${YELLOW}4. Startup errors (if any):${NC}"
docker logs logseq_spring_thing_webxr 2>&1 | grep -E "(panic|error|Error|ERROR|Failed to|Could not)" | tail -10

# 5. Try to start backend manually
echo -e "\n${YELLOW}5. Container entrypoint:${NC}"
docker exec logseq_spring_thing_webxr sh -c 'ps aux | head -1; ps aux | grep -v "ps aux"'