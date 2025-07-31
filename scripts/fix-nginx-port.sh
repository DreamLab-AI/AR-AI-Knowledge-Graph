#!/bin/bash

# Quick fix script for nginx port mismatch

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Fixing nginx port configuration${NC}"

# 1. Find actual backend port
BACKEND_PORT=$(docker exec logseq_spring_thing_webxr netstat -tlnp 2>&1 | grep -oE ":(3001|4000)" | head -1 | tr -d ':')

if [ -z "$BACKEND_PORT" ]; then
    echo -e "${RED}Error: Backend not listening on any port${NC}"
    echo "Checking if backend is running..."
    docker exec logseq_spring_thing_webxr ps aux | grep webxr
    exit 1
fi

echo "Backend found on port: $BACKEND_PORT"

# 2. Check current nginx config
NGINX_CONFIG="/workspace/ext/nginx.dev.conf"
CURRENT_PORT=$(grep -oE "server 127.0.0.1:[0-9]+" $NGINX_CONFIG | grep -oE "[0-9]+$")

echo "Nginx configured for port: $CURRENT_PORT"

if [ "$BACKEND_PORT" == "$CURRENT_PORT" ]; then
    echo -e "${GREEN}✓ Ports already match${NC}"
    exit 0
fi

# 3. Update nginx config
echo -e "${YELLOW}Updating nginx config...${NC}"
sed -i.bak "s/server 127.0.0.1:$CURRENT_PORT/server 127.0.0.1:$BACKEND_PORT/" $NGINX_CONFIG

echo "Updated nginx.dev.conf to use port $BACKEND_PORT"
echo -e "${YELLOW}Please restart the container for changes to take effect:${NC}"
echo "  docker-compose restart"

# 4. Test the configuration
echo -e "\n${BLUE}Testing current state:${NC}"
curl -s -X POST http://localhost:3001/api/bots/initialize-swarm \
    -H "Content-Type: application/json" \
    -d '{"topology": "mesh", "maxAgents": 3}' \
    -o /dev/null -w "Status: %{http_code}\n"