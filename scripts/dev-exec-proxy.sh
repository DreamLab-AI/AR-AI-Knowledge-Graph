#!/bin/bash
# Development Docker Exec Proxy
# Provides controlled access to Docker exec commands for development

set -euo pipefail

# Configuration
ALLOWED_CONTAINER="logseq_spring_thing_webxr"
LOG_FILE="/app/logs/docker-exec.log"

# Log all exec attempts
log_exec() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $@" >> "$LOG_FILE"
}

# Validate container name
validate_container() {
    local container=$1
    if [[ "$container" != "$ALLOWED_CONTAINER" ]]; then
        echo "Error: Only access to container '$ALLOWED_CONTAINER' is allowed"
        log_exec "DENIED: Attempted access to container '$container'"
        exit 1
    fi
}

# Main execution
case "${1:-}" in
    "exec")
        if [[ $# -lt 3 ]]; then
            echo "Usage: $0 exec <command> [args...]"
            exit 1
        fi
        
        shift  # Remove 'exec'
        COMMAND=$1
        shift  # Remove command
        
        # Only allow specific safe commands
        case "$COMMAND" in
            "ps"|"ls"|"cat"|"tail"|"head"|"grep"|"find"|"curl"|"nc"|"lsof")
                log_exec "ALLOWED: docker exec $ALLOWED_CONTAINER $COMMAND $@"
                docker exec "$ALLOWED_CONTAINER" "$COMMAND" "$@"
                ;;
            "bash"|"sh")
                # Allow shell access but log it prominently
                log_exec "SHELL ACCESS: docker exec -it $ALLOWED_CONTAINER $COMMAND"
                docker exec -it "$ALLOWED_CONTAINER" "$COMMAND"
                ;;
            *)
                echo "Error: Command '$COMMAND' is not in the allowed list"
                log_exec "DENIED: Attempted to run '$COMMAND' in container"
                exit 1
                ;;
        esac
        ;;
    "logs")
        log_exec "LOGS: Viewing container logs"
        docker logs "$ALLOWED_CONTAINER" "${@:2}"
        ;;
    "status")
        log_exec "STATUS: Checking container status"
        docker ps -a --filter "name=$ALLOWED_CONTAINER" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
        ;;
    "restart")
        log_exec "RESTART: Restarting services in container"
        docker exec "$ALLOWED_CONTAINER" supervisorctl restart all
        ;;
    *)
        echo "Docker Exec Proxy - Safe access to development container"
        echo ""
        echo "Usage:"
        echo "  $0 exec <command> [args...]  - Execute allowed commands"
        echo "  $0 logs [options]            - View container logs"
        echo "  $0 status                    - Check container status"
        echo "  $0 restart                   - Restart services"
        echo ""
        echo "Allowed commands: ps, ls, cat, tail, head, grep, find, curl, nc, lsof, bash, sh"
        exit 1
        ;;
esac