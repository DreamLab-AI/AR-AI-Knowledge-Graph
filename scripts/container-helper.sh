#!/bin/bash
# Container Helper - Easy access to development container services

set -euo pipefail

CONTAINER_NAME="logseq_spring_thing_webxr"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check if container is running
check_container() {
    if ! docker ps --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        echo -e "${RED}Error: Container '${CONTAINER_NAME}' is not running${NC}"
        echo "Please start it with: ./scripts/dev.sh"
        exit 1
    fi
}

# Main command handler
case "${1:-help}" in
    "status")
        echo -e "${YELLOW}Container Status:${NC}"
        docker ps -a --filter "name=${CONTAINER_NAME}" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
        
        if docker ps --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
            echo -e "\n${YELLOW}Service Status:${NC}"
            docker exec ${CONTAINER_NAME} supervisorctl status 2>/dev/null || echo "Supervisord not running"
        fi
        ;;
        
    "logs")
        check_container
        SERVICE="${2:-all}"
        case "$SERVICE" in
            "rust")
                docker exec ${CONTAINER_NAME} tail -f /app/logs/rust.log
                ;;
            "vite")
                docker exec ${CONTAINER_NAME} tail -f /app/logs/vite.log
                ;;
            "nginx")
                docker exec ${CONTAINER_NAME} tail -f /var/log/nginx/access.log
                ;;
            "all")
                docker logs -f ${CONTAINER_NAME}
                ;;
            *)
                echo "Unknown service: $SERVICE"
                echo "Available: rust, vite, nginx, all"
                exit 1
                ;;
        esac
        ;;
        
    "restart")
        check_container
        SERVICE="${2:-all}"
        echo -e "${YELLOW}Restarting $SERVICE...${NC}"
        docker exec ${CONTAINER_NAME} supervisorctl restart $SERVICE
        ;;
        
    "shell")
        check_container
        echo -e "${GREEN}Entering container shell...${NC}"
        docker exec -it ${CONTAINER_NAME} bash
        ;;
        
    "exec")
        check_container
        shift
        docker exec ${CONTAINER_NAME} "$@"
        ;;
        
    "test")
        check_container
        echo -e "${YELLOW}Testing service endpoints:${NC}"
        
        echo -n "Nginx (3001): "
        docker exec ${CONTAINER_NAME} curl -s -o /dev/null -w "%{http_code}" http://localhost:3001 || echo "Failed"
        
        echo -n "Vite (5173): "
        docker exec ${CONTAINER_NAME} curl -s -o /dev/null -w "%{http_code}" http://localhost:5173 || echo "Failed"
        
        echo -n "Rust API (4000): "
        docker exec ${CONTAINER_NAME} curl -s -o /dev/null -w "%{http_code}" http://localhost:4000/api/health || echo "Failed"
        ;;
        
    "fix-blank")
        check_container
        echo -e "${YELLOW}Attempting to fix blank page issue...${NC}"
        
        # Restart services
        docker exec ${CONTAINER_NAME} supervisorctl restart all
        
        # Wait for services to start
        sleep 5
        
        # Check status
        $0 test
        
        echo -e "\n${GREEN}Services restarted. Try accessing http://192.168.0.51:3001${NC}"
        ;;
        
    "help"|*)
        echo "Container Helper - Manage the development container"
        echo ""
        echo "Usage: $0 [command] [options]"
        echo ""
        echo "Commands:"
        echo "  status          - Show container and service status"
        echo "  logs [service]  - Tail logs (rust/vite/nginx/all)"
        echo "  restart [svc]   - Restart service (rust-backend/vite-dev/nginx/all)"
        echo "  shell           - Enter container shell"
        echo "  exec [cmd]      - Execute command in container"
        echo "  test            - Test service endpoints"
        echo "  fix-blank       - Try to fix blank page issue"
        echo "  help            - Show this help"
        ;;
esac