#!/bin/bash
# Master Test Suite for Network and API Connectivity

# --- Configuration ---
BASE_URL="http://localhost:3001"
WS_URL="ws://localhost:3001"
WEBXR_CONTAINER="logseq_spring_thing_webxr"
POWERDEV_CONTAINER="powerdev"

# --- Colors and Helpers ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_header() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

check_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ PASSED${NC}"
    else
        echo -e "${RED}✗ FAILED${NC} (Code: $1)"
    fi
}

# --- Test Functions ---

check_dependencies() {
    print_header "Checking Dependencies"
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}✗ Docker is not installed. Aborting.${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Docker found.${NC}"

    if ! command -v jq &> /dev/null; then
        echo -e "${RED}✗ jq is not installed. Please install it to parse JSON responses.${NC}"
    else
        echo -e "${GREEN}✓ jq found.${NC}"
    fi

    if ! command -v wscat &> /dev/null; then
        echo -e "${YELLOW}⚠ wscat is not installed. WebSocket tests will be skipped. (Install with: npm install -g wscat)${NC}"
    else
        echo -e "${GREEN}✓ wscat found.${NC}"
    fi
}

check_container_status() {
    print_header "1. Container Status"
    echo -n "Checking for '$WEBXR_CONTAINER' container... "
    docker ps --format "{{.Names}}" | grep -q "^${WEBXR_CONTAINER}$"
    check_status $?

    echo -n "Checking for '$POWERDEV_CONTAINER' container... "
    docker ps --format "{{.Names}}" | grep -q "^${POWERDEV_CONTAINER}$"
    check_status $?
}

check_network_ports() {
    print_header "2. Network Port Connectivity"
    echo -n "Checking Nginx port (3001)... "
    curl -s -o /dev/null --connect-timeout 2 "$BASE_URL/api/health"
    check_status $?

    echo -n "Checking direct backend port (4000)... "
    curl -s -o /dev/null --connect-timeout 2 "http://localhost:4000/api/health"
    local status=$?
    if [ $status -eq 0 ]; then
        echo -e "${GREEN}✓ PASSED${NC} (Note: Direct access is for dev environment)"
    else
        echo -e "${YELLOW}✓ SKIPPED${NC} (Not exposed in production, which is correct)"
    fi
}

check_internal_processes() {
    print_header "3. Internal Process Status"
    echo -n "Checking Nginx process in '$WEBXR_CONTAINER'... "
    docker exec "$WEBXR_CONTAINER" pgrep -f "nginx: master process" &> /dev/null
    check_status $?

    echo -n "Checking Rust backend ('webxr') process in '$WEBXR_CONTAINER'... "
    docker exec "$WEBXR_CONTAINER" pgrep -f "webxr" &> /dev/null
    check_status $?

    echo -n "Checking MCP relay process in '$POWERDEV_CONTAINER'... "
    docker exec "$POWERDEV_CONTAINER" pgrep -f "mcp-ws-relay" &> /dev/null
    check_status $?
}

test_health_endpoints() {
    print_header "4. API Health Endpoints"
    echo -n "Testing backend health (/api/health)... "
    curl -s "$BASE_URL/api/health" | jq -e '.status == "healthy"' &> /dev/null
    check_status $?

    echo -n "Testing MCP relay health (/api/mcp/health)... "
    curl -s "$BASE_URL/api/mcp/health" | jq -e '.mcp_relay_running == true' &> /dev/null
    check_status $?
}

test_core_api_endpoints() {
    print_header "5. Core API Endpoints"
    echo -n "Testing graph data endpoint (/api/graph/data)... "
    curl -s "$BASE_URL/api/graph/data" | jq -e '.nodes and .edges' &> /dev/null
    check_status $?

    echo -n "Testing bots data endpoint (/api/bots/data)... "
    curl -s "$BASE_URL/api/bots/data" | jq -e '.nodes and .edges' &> /dev/null
    check_status $?

    echo -n "Testing settings endpoint (/api/user-settings)... "
    curl -s "$BASE_URL/api/user-settings" | jq -e '.visualisation and .system' &> /dev/null
    check_status $?
}

test_websocket_connections() {
    print_header "6. WebSocket Handshake Tests"
    if ! command -v wscat &> /dev/null; then
        echo -e "${YELLOW}Skipping WebSocket tests: wscat not found.${NC}"
        return
    fi

    echo -n "Testing main WebSocket (/wss)... "
    echo "ping" | wscat -c "$WS_URL/wss" -w 1 | grep -q "pong"
    check_status $?

    echo -n "Testing speech WebSocket (/ws/speech)... "
    echo '{"type":"ping"}' | wscat -c "$WS_URL/ws/speech" -w 1 | grep -q "connected"
    check_status $?

    echo -n "Testing MCP relay WebSocket (/ws/mcp-relay)... "
    # The relay doesn't have a simple ping/pong, we just check if it connects
    wscat -c "$WS_URL/ws/mcp-relay" -w 1 -x &> /dev/null
    check_status $?
}

test_api_flows() {
    print_header "7. API Integration Flows"

    echo -n "Testing Swarm Initialization Flow... "
    RESPONSE=$(curl -s -X POST "$BASE_URL/api/bots/initialize-swarm" \
        -H "Content-Type: application/json" \
        -d '{"topology": "mesh", "maxAgents": 3, "strategy": "adaptive", "enableNeural": false, "agentTypes": ["coordinator"], "customPrompt": "test"}')

    echo "$RESPONSE" | jq -e '.success == true' &> /dev/null
    local swarm_status=$?
    check_status $swarm_status
    if [ $swarm_status -ne 0 ]; then
        echo "Response: $RESPONSE"
    fi

    echo -n "Testing Bots Data POST/GET Flow... "
    POST_RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/api/bots/update" \
        -H "Content-Type: application/json" \
        -d '{"nodes": [{"id": "test-agent", "type": "test", "status": "idle", "name": "Test", "cpuUsage": 0, "health": 100, "workload": 0}]}')

    if [ "$POST_RESPONSE" -eq 200 ]; then
        curl -s "$BASE_URL/api/bots/data" | jq -e '(.nodes | length) > 0' &> /dev/null
        check_status $?
    else
        echo -e "${RED}✗ FAILED${NC} (POST returned $POST_RESPONSE)"
    fi

    echo -n "Testing Settings Update Flow... "
    PUT_RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$BASE_URL/api/user-settings" \
        -H "Content-Type: application/json" \
        -d '{"path": "visualisation.bloom.enabled", "value": false}')

    if [ "$PUT_RESPONSE" -eq 200 ]; then
        curl -s "$BASE_URL/api/user-settings" | jq -e '.visualisation.bloom.enabled == false' &> /dev/null
        check_status $?
    else
        echo -e "${RED}✗ FAILED${NC} (PUT returned $PUT_RESPONSE)"
    fi
    # Revert setting
    curl -s -X PUT "$BASE_URL/api/user-settings" -d '{"path": "visualisation.bloom.enabled", "value": true}' > /dev/null
}


# --- Main Execution ---
main() {
    check_dependencies
    check_container_status
    check_network_ports
    check_internal_processes
    test_health_endpoints
    test_core_api_endpoints
    test_websocket_connections
    test_api_flows

    echo -e "\n${GREEN}Master test suite finished.${NC}"
    echo -e "${YELLOW}Note: This script does not test external API connectivity (RAGFlow, etc.) or complex WebSocket tool calls.${NC}"
}

main