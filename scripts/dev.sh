#!/bin/bash
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Get the absolute path of the script's directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Global variables
DOCKER_COMPOSE_FILE="$PROJECT_ROOT/docker-compose.dev.yml"
CONTAINER_NAME="logseq_spring_thing_webxr"
PROJECT_IDENTIFIER="logseq_spring_thing_dev"  # Unique identifier for our project's processes

# Parse command line arguments
NO_CACHE=false
for arg in "$@"; do
    case $arg in
        --no-cache)
            NO_CACHE=true
            shift
            ;;
        *)
            ;;
    esac
done

log() {
    echo -e "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

# Cleanup function
cleanup() {
    log "${YELLOW}Cleaning up development environment...${NC}"
    
    # Stop Docker containers
    if docker ps -q -f name=$CONTAINER_NAME > /dev/null; then
        log "${YELLOW}Stopping Docker containers...${NC}"
        cd "$PROJECT_ROOT" && docker compose -f $DOCKER_COMPOSE_FILE stop # Changed 'down' to 'stop' to prevent container removal
    fi
    
    # Host process cleanup removed as Node/Vite run inside the container
    
    log "${GREEN}Cleanup complete${NC}"
}

# Trap signals
trap cleanup SIGINT SIGTERM EXIT

# Setup function removed as dependencies are installed during Docker build
# Start development servers
start_dev() {
    log "${YELLOW}Starting development servers...${NC}"
    
    # Build and start containers with updated configuration
    log "${YELLOW}Starting Docker containers...${NC}"
    if [ "$NO_CACHE" = true ]; then
        log "${YELLOW}Building with --no-cache option...${NC}"
        cd "$PROJECT_ROOT" && DOCKER_BUILDKIT=1 docker compose -f $DOCKER_COMPOSE_FILE build --no-cache && \
        docker compose -f $DOCKER_COMPOSE_FILE up
    else
        cd "$PROJECT_ROOT" && DOCKER_BUILDKIT=1 docker compose -f $DOCKER_COMPOSE_FILE up --build
    fi
}

# Main execution
cleanup  # Clean up any existing processes first
# setup_dev call removed
start_dev
