#!/bin/bash

# MCP Server Manager Script
# Provides convenient commands for managing MCP servers

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
COMPOSE_FILE=${COMPOSE_FILE:-docker-compose.enhanced.yml}
PROJECT_NAME=${PROJECT_NAME:-powerdev}

# Function to print colored output
print_color() {
    local color=$1
    shift
    echo -e "${color}$@${NC}"
}

# Function to check if docker-compose is available
check_docker() {
    if ! command -v docker-compose &> /dev/null; then
        print_color $RED "Error: docker-compose is not installed"
        exit 1
    fi
}

# Function to show usage
usage() {
    echo "MCP Server Manager"
    echo ""
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  start [profile]    - Start MCP servers (profiles: monitoring, tools, cache)"
    echo "  stop              - Stop all services"
    echo "  restart           - Restart all services"
    echo "  status            - Show service status"
    echo "  logs [service]    - Show logs (service: main, orchestrator, loki, etc.)"
    echo "  health            - Run health checks"
    echo "  test-ws           - Test WebSocket connection"
    echo "  test-api          - Test REST API endpoints"
    echo "  shell [service]   - Open shell in service container"
    echo "  update            - Pull latest images and restart"
    echo "  clean             - Remove all containers and volumes"
    echo ""
    echo "Examples:"
    echo "  $0 start monitoring  - Start with monitoring profile"
    echo "  $0 logs orchestrator - Show orchestrator logs"
    echo "  $0 shell main       - Open shell in main container"
}

# Start services
cmd_start() {
    local profiles=""

    # Add profiles if specified
    for profile in "$@"; do
        profiles="$profiles --profile $profile"
    done

    print_color $BLUE "Starting MCP services..."
    docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME $profiles up -d

    print_color $GREEN "Services started. Waiting for health checks..."
    sleep 5

    cmd_status
}

# Stop services
cmd_stop() {
    print_color $YELLOW "Stopping MCP services..."
    docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME down
    print_color $GREEN "Services stopped."
}

# Restart services
cmd_restart() {
    cmd_stop
    sleep 2
    cmd_start "$@"
}

# Show service status
cmd_status() {
    print_color $BLUE "MCP Service Status:"
    docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME ps

    echo ""
    print_color $BLUE "Container Health:"
    docker ps --filter "label=com.docker.compose.project=$PROJECT_NAME" \
              --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
}

# Show logs
cmd_logs() {
    local service=${1:-}

    if [ -z "$service" ]; then
        docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME logs -f --tail=100
    else
        # Map friendly names to container names
        case $service in
            main) service="powerdev-main" ;;
            orchestrator) service="mcp-orchestrator" ;;
            loki) service="loki" ;;
            grafana) service="grafana" ;;
            redis) service="redis" ;;
            tools) service="mcp-tools" ;;
        esac

        docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME logs -f --tail=100 $service
    fi
}

# Run health checks
cmd_health() {
    print_color $BLUE "Running MCP health checks..."

    # Check if main container is running
    if docker ps --filter "name=powerdev-main" --filter "status=running" -q | grep -q .; then
        docker exec powerdev-main /app/mcp-scripts/health-check.sh
    else
        print_color $RED "Main container is not running"
        return 1
    fi

    # Check orchestrator
    echo ""
    print_color $BLUE "Checking orchestrator health..."
    if curl -s http://localhost:9000/health | jq . 2>/dev/null; then
        print_color $GREEN "Orchestrator is healthy"
    else
        print_color $RED "Orchestrator is not responding"
    fi
}

# Test WebSocket connection
cmd_test_ws() {
    print_color $BLUE "Testing WebSocket connection..."

    # Create a simple WebSocket test script
    cat > /tmp/ws-test.js << 'EOF'
const WebSocket = require('ws');

const ws = new WebSocket('ws://localhost:9001');

ws.on('open', () => {
    console.log('✓ Connected to WebSocket server');
    ws.send(JSON.stringify({ type: 'ping' }));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    console.log('✓ Received:', msg.type);
    if (msg.type === 'pong') {
        console.log('✓ WebSocket test successful');
        process.exit(0);
    }
});

ws.on('error', (err) => {
    console.error('✗ WebSocket error:', err.message);
    process.exit(1);
});

setTimeout(() => {
    console.error('✗ WebSocket test timeout');
    process.exit(1);
}, 5000);
EOF

    # Run the test
    if command -v node &> /dev/null; then
        npm install ws &>/dev/null 2>&1 || true
        node /tmp/ws-test.js
    else
        print_color $YELLOW "Node.js not installed. Using curl for basic test..."
        if curl -s http://localhost:9001 &>/dev/null; then
            print_color $GREEN "WebSocket server is listening"
        else
            print_color $RED "WebSocket server is not responding"
        fi
    fi

    rm -f /tmp/ws-test.js
}

# Test REST API
cmd_test_api() {
    print_color $BLUE "Testing REST API endpoints..."

    # Test health endpoint
    echo -n "Health endpoint: "
    if curl -s http://localhost:9000/health | jq -e '.status == "ok"' &>/dev/null; then
        print_color $GREEN "✓ OK"
    else
        print_color $RED "✗ Failed"
    fi

    # Test MCP data endpoint
    echo -n "MCP data endpoint: "
    if curl -s http://localhost:9000/api/mcp/data | jq -e '.lastUpdate' &>/dev/null; then
        print_color $GREEN "✓ OK"
    else
        print_color $RED "✗ Failed"
    fi

    # Test MCP servers endpoint
    echo -n "MCP servers endpoint: "
    if curl -s http://localhost:9000/api/mcp/servers | jq -e '.servers' &>/dev/null; then
        print_color $GREEN "✓ OK"
    else
        print_color $RED "✗ Failed"
    fi
}

# Open shell in container
cmd_shell() {
    local service=${1:-main}

    # Map friendly names to container names
    case $service in
        main) container="powerdev-main" ;;
        orchestrator) container="powerdev-mcp-orchestrator" ;;
        tools) container="powerdev-mcp-tools" ;;
        *) container="powerdev-$service" ;;
    esac

    print_color $BLUE "Opening shell in $container..."
    docker exec -it $container /bin/bash || docker exec -it $container /bin/sh
}

# Update services
cmd_update() {
    print_color $BLUE "Pulling latest images..."
    docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME pull

    print_color $YELLOW "Restarting services..."
    cmd_restart "$@"
}

# Clean everything
cmd_clean() {
    print_color $YELLOW "This will remove all containers and volumes. Are you sure? (y/N)"
    read -r response

    if [[ "$response" =~ ^[Yy]$ ]]; then
        print_color $RED "Removing all containers and volumes..."
        docker-compose -f $COMPOSE_FILE -p $PROJECT_NAME down -v
        print_color $GREEN "Cleanup complete."
    else
        print_color $BLUE "Cleanup cancelled."
    fi
}

# Main command handler
main() {
    check_docker

    case "${1:-}" in
        start)
            shift
            cmd_start "$@"
            ;;
        stop)
            cmd_stop
            ;;
        restart)
            shift
            cmd_restart "$@"
            ;;
        status)
            cmd_status
            ;;
        logs)
            cmd_logs "${2:-}"
            ;;
        health)
            cmd_health
            ;;
        test-ws)
            cmd_test_ws
            ;;
        test-api)
            cmd_test_api
            ;;
        shell)
            cmd_shell "${2:-main}"
            ;;
        update)
            shift
            cmd_update "$@"
            ;;
        clean)
            cmd_clean
            ;;
        *)
            usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"