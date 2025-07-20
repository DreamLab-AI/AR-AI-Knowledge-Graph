#!/usr/bin/env bash
# powerdev.sh - helper for the powerdev development environment
set -euo pipefail

# Configuration
COMPOSE_FILE=docker-compose.yml
PROJECT_NAME=powerdev
ENVFILE=.env
IMAGE=powerdev:latest

# Source the .env file to make variables available to the script
if [[ -f "${ENVFILE}" ]]; then
  source "${ENVFILE}"
fi

# ---- Resource detection and validation ---------------------
detect_resources() {
  AVAILABLE_CPUS=$(nproc)
  AVAILABLE_MEM=$(free -g | awk '/^Mem:/{print $2}')

  # Set sensible defaults based on available resources
  DEFAULT_CPUS=$((AVAILABLE_CPUS > 24 ? 24 : AVAILABLE_CPUS))
  DEFAULT_MEM=$((AVAILABLE_MEM > 200 ? 200 : AVAILABLE_MEM > 10 ? AVAILABLE_MEM-10 : 4))

  # Apply user overrides or use calculated defaults
  DOCKER_CPUS=${DOCKER_CPUS:-$DEFAULT_CPUS}
  DOCKER_MEMORY=${DOCKER_MEMORY:-${DEFAULT_MEM}g}

  # Validate resources don't exceed available
  if [[ $DOCKER_CPUS -gt $AVAILABLE_CPUS ]]; then
    echo "Warning: Requested CPUs ($DOCKER_CPUS) exceeds available ($AVAILABLE_CPUS)"
    DOCKER_CPUS=$AVAILABLE_CPUS
  fi
}

# ---- Pre-flight checks ---------------------
preflight() {
  # Check Docker daemon
  if ! docker info >/dev/null 2>&1; then
    echo "Error: Docker daemon not accessible"
    exit 1
  fi

  # Check docker-compose
  if ! command -v docker-compose >/dev/null 2>&1; then
    echo "Error: docker-compose not found. Please install docker-compose"
    exit 1
  fi

  # Ensure required directories exist with proper permissions
  mkdir -p "./.agent-mount/docker_data" "./.agent-mount/docker_workspace"
  mkdir -p "./.agent-mount/docker_analysis" "./.agent-mount/docker_logs" "./.agent-mount/docker_output" "./.agent-mount/docker_latex"
  mkdir -p "./.agent-mount/docker_data/claude-flow"
  mkdir -p "./workspace" "./blender-files" "./mcp-configs" "./mcp-logs"
  mkdir -p "./orchestrator" "./mcp-scripts" "./mcp-tools" "./grafana"

  # Create EXTERNAL_DIR if set
  if [[ -n "${EXTERNAL_DIR:-}" ]]; then
    mkdir -p "${EXTERNAL_DIR}"
  else
    mkdir -p "./.agent-mount/ext"
  fi

  # Set proper permissions for container user (UID 1000)
  if command -v chown >/dev/null 2>&1; then
    chown -R 1000:1000 "./.agent-mount/" 2>/dev/null || echo "Warning: Could not set ownership (this is normal on some systems)"
  fi

  # Ensure directories are writable
  chmod -R 755 "./.agent-mount/"

  echo "Created persistent directories"

  # Create .env file if it doesn't exist
  if [[ ! -f "$ENVFILE" ]]; then
    echo "Creating default .env file..."
    cat > "$ENVFILE" <<EOF
# Resource limits
DOCKER_CPUS=$DOCKER_CPUS
DOCKER_MEMORY=$DOCKER_MEMORY

# External directory for mounted files
EXTERNAL_DIR=./.agent-mount/ext

# MCP Configuration
REMOTE_MCP_HOST=
MCP_LOG_LEVEL=debug
POLL_INTERVAL=5000

# Grafana password
GRAFANA_PASSWORD=admin

EOF
    echo ".env file created with defaults"
  fi

  # Validate config
  validate_config
}

# ---- Config validation ---------------------
validate_config() {
  if [[ ! -f "$COMPOSE_FILE" ]]; then
    echo "Error: $COMPOSE_FILE not found"
    exit 1
  fi

  if [[ ! -f "$ENVFILE" ]]; then
    echo "Warning: $ENVFILE not found - using environment variables only"
  fi
}

# ---- Help message ---------------------
help() {
  cat <<EOF
Usage: $0 {build|start|stop|restart|status|logs|shell|health|monitor|tools|cleanup|rm}

PowerDev Commands:

  build [--no-cache]   Build all Docker images:
                       $0 build
                       $0 build --no-cache

  start [profile...]   Start services (with optional profiles):
                       $0 start                    # Basic services only
                       $0 start monitoring         # With Grafana monitoring
                       $0 start tools              # With development tools
                       $0 start monitoring tools   # Multiple profiles

  stop                 Stop all services:
                       $0 stop

  restart [profile...] Restart services:
                       $0 restart
                       $0 restart monitoring

  status               Show service status:
                       $0 status

  logs [service]       View logs (all services or specific):
                       $0 logs                     # All services
                       $0 logs orchestrator        # Specific service
                       $0 logs -f orchestrator     # Follow logs

  shell [service]      Open shell in service:
                       $0 shell                    # Main container
                       $0 shell orchestrator       # Orchestrator container
                       $0 shell tools              # Tools container

  health               Run comprehensive health checks:
                       $0 health

  monitor              Open real-time monitoring dashboard:
                       $0 monitor

  tools                Run MCP testing tools:
                       $0 tools ws-test            # Test WebSocket
                       $0 tools api-test           # Test REST API

  cleanup              Clean up all containers and volumes:
                       $0 cleanup

  rm                   Remove orphan containers:
                       $0 rm

Service URLs:
  - Claude Flow UI: http://localhost:3000
  - MCP Orchestrator API: http://localhost:9000
  - MCP WebSocket: ws://localhost:9001
  - Grafana: http://localhost:3002 (when monitoring profile is active)

Available Profiles:
  - monitoring: Loki, Promtail, and Grafana for log aggregation and visualization
  - tools: Development utilities container with MCP testing tools
  - cache: Redis for MCP response caching
EOF
}

# ---- Build command ---------------------
build() {
  preflight
  detect_resources

  echo "Building powerdev images..."

  # Build with docker-compose
  docker-compose -f "$COMPOSE_FILE" build "${@:2}"

  echo "Build complete!"
}

# ---- Start command ---------------------
start() {
  preflight
  detect_resources

  local profiles=""

  # Add profiles if specified
  for arg in "${@:2}"; do
    profiles="$profiles --profile $arg"
  done

  echo "Starting powerdev services..."
  docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" $profiles up -d

  echo ""
  echo "Services started! Waiting for health checks..."
  sleep 5

  # Show status
  status

  echo ""
  echo "Access points:"
  echo "  - Claude Flow UI: http://localhost:3000"
  echo "  - MCP Orchestrator API: http://localhost:9000"
  echo "  - MCP WebSocket: ws://localhost:9001"

  if [[ "$*" == *"monitoring"* ]]; then
    echo "  - Grafana: http://localhost:3002 (admin/admin)"
  fi

  # Automatically enter the main container shell
  shell main
}

# ---- Stop command ---------------------
stop() {
  echo "Stopping all powerdev services..."
  docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down
  echo "Services stopped."
}

# ---- Restart command ---------------------
restart() {
  stop
  sleep 2
  start "$@"
}

# ---- Status command ---------------------
status() {
  echo "PowerDev Service Status:"
  echo "========================"
  docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" ps

  echo ""
  echo "Container Health:"
  docker ps --filter "label=com.docker.compose.project=$PROJECT_NAME" \
            --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

  # Check orchestrator health
  echo ""
  echo "Orchestrator Health:"
  if curl -s http://localhost:9000/health | jq . 2>/dev/null; then
    echo "✓ Orchestrator is healthy"
  else
    echo "✗ Orchestrator is not responding"
  fi
}

# ---- Logs command ---------------------
logs() {
  local service="${2:-}"
  shift

  if [[ -z "$service" ]]; then
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" logs "${@:2}"
  else
    # Map friendly names
    case $service in
      main) service="powerdev" ;;
      orchestrator) service="mcp-orchestrator" ;;
      tools) service="mcp-tools" ;;
    esac

    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" logs "${@:2}" "$service"
  fi
}

# ---- Shell command ---------------------
shell() {
  local service="${2:-main}"

  # Map friendly names to service names
  case $service in
    main) service="powerdev" ;;
    orchestrator) service="mcp-orchestrator" ;;
    tools) service="mcp-tools" ;;
  esac

  echo "Opening shell in $service..."
  docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec "$service" /bin/bash
}

# ---- Health command ---------------------
health() {
  echo "Running comprehensive health checks..."

  # Use the MCP manager script
  if [[ -x "./mcp-scripts/mcp-manager.sh" ]]; then
    ./mcp-scripts/mcp-manager.sh health
  else
    echo "Error: mcp-manager.sh not found or not executable"
    echo "Running basic health check..."

    # Basic health check
    if docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec powerdev /app/mcp-scripts/health-check.sh; then
      echo "✓ Basic health check passed"
    else
      echo "✗ Basic health check failed"
    fi
  fi
}

# ---- Monitor command ---------------------
monitor() {
  echo "Opening monitoring dashboard..."

  # Check if monitoring profile is active
  if ! docker ps --filter "name=powerdev-grafana" -q | grep -q .; then
    echo "Grafana is not running. Starting monitoring stack..."
    start monitoring
    echo "Waiting for Grafana to be ready..."
    sleep 10
  fi

  echo "Opening Grafana dashboard at http://localhost:3002"
  echo "Default credentials: admin/admin"

  # Try to open in browser
  if command -v xdg-open >/dev/null 2>&1; then
    xdg-open http://localhost:3002
  elif command -v open >/dev/null 2>&1; then
    open http://localhost:3002
  else
    echo "Please open http://localhost:3002 in your browser"
  fi
}

# ---- Tools command ---------------------
tools() {
  local tool="${2:-}"

  case $tool in
    ws-test)
      echo "Testing WebSocket connection..."
      if [[ -x "./mcp-scripts/mcp-manager.sh" ]]; then
        ./mcp-scripts/mcp-manager.sh test-ws
      else
        echo "Running basic WebSocket test..."
        docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec mcp-tools python3 /app/ws-test-client.py
      fi
      ;;

    api-test)
      echo "Testing REST API..."
      if [[ -x "./mcp-scripts/mcp-manager.sh" ]]; then
        ./mcp-scripts/mcp-manager.sh test-api
      else
        echo "Running basic API test..."
        curl -s http://localhost:9000/health | jq .
      fi
      ;;

    *)
      echo "Available tools:"
      echo "  ws-test   - Test WebSocket connection"
      echo "  api-test  - Test REST API endpoints"
      ;;
  esac
}

# ---- Remove orphan containers command ---------------------
rm_orphans() {
  echo "Removing orphan containers..."
  docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down --remove-orphans
  echo "Orphan containers removed."
}

# ---- Cleanup command ---------------------
cleanup() {
  echo "This will remove all containers and volumes. Are you sure? (y/N)"
  read -r response

  if [[ "$response" =~ ^[Yy]$ ]]; then
    echo "Stopping and removing all containers..."
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down -v

    echo "Cleaning up Docker resources..."
    docker system prune -f
    docker volume prune -f

    echo "Cleanup complete."
  else
    echo "Cleanup cancelled."
  fi
}

# ---- Shell completion ---------------------
_powerdev_completion() {
  local cur="${COMP_WORDS[COMP_CWORD]}"
  local prev="${COMP_WORDS[COMP_CWORD-1]}"

  local commands="build start stop restart status logs shell health monitor tools cleanup"
  local profiles="monitoring tools cache"
  local tools_cmds="ws-test api-test"

  case "$prev" in
    start|restart)
      COMPREPLY=($(compgen -W "$profiles" -- "$cur"))
      ;;
    tools)
      COMPREPLY=($(compgen -W "$tools_cmds" -- "$cur"))
      ;;
    *)
      COMPREPLY=($(compgen -W "$commands" -- "$cur"))
      ;;
  esac
}
complete -F _powerdev_completion powerdev.sh

# ---- Graceful shutdown handler ---------------------
trap 'echo "Shutting down..."; stop 2>/dev/null; exit' SIGINT SIGTERM

# ---- Main entry point ---------------------
if [[ $# -eq 0 ]]; then
  help
  exit 1
fi

case $1 in
  build|start|stop|restart|status|logs|shell|health|monitor|tools|cleanup|rm)
    case $1 in
      rm)
        rm_orphans
        ;;
      *)
        "$@"
        ;;
    esac
    ;;
  *)
    help
    exit 1
    ;;
esac