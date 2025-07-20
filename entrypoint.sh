#!/bin/bash
set -e

echo "--- DEBUG: Checking environment variables ---"
echo "REMOTE_MCP_HOST='${REMOTE_MCP_HOST}'"
echo "-------------------------------------------"

echo "=== MCP 3D Environment Starting ==="
echo "Container IP: $(hostname -I | awk '{print $1}')"
echo "Available MCP Servers: Blender (9876), Revit (8080), Unreal (55557)"
echo ""

# Function to wait for port availability
wait_for_port() {
    local host=$1
    local port=$2
    local timeout=${3:-30}
    local elapsed=0

    echo "Waiting for $host:$port to be available..."
    while ! timeout 1 bash -c "echo >/dev/tcp/$host/$port" 2>/dev/null; do
        if [ $elapsed -ge $timeout ]; then
            echo "Timeout waiting for $host:$port"
            return 1
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done
    echo "$host:$port is ready!"
    return 0
}

# Start X Virtual Framebuffer for GUI applications (headless)
echo "--- Starting X Virtual Framebuffer ---"
rm -f /tmp/.X99-lock
Xvfb :99 -screen 0 1920x1080x24 -ac +extension GLX +render -noreset &
export DISPLAY=:99
sleep 2

# Create workspace directories with proper permissions
echo "--- Setting up workspace directories ---"
mkdir -p /workspace/.claude /workspace/.mcp /workspace/memory /workspace/logs
mkdir -p /workspace/.roo /workspace/ext
chown -R dev:dev /workspace

# Initialize Claude Flow configuration
echo "--- Initializing Claude Code Configuration ---"

# Create main Claude settings
cat > /workspace/.claude/settings.json << 'EOF'
{
  "env": {
    "CLAUDE_FLOW_AUTO_COMMIT": "false",
    "CLAUDE_FLOW_AUTO_PUSH": "false",
    "CLAUDE_FLOW_HOOKS_ENABLED": "true",
    "CLAUDE_FLOW_TELEMETRY_ENABLED": "true",
    "CLAUDE_FLOW_REMOTE_EXECUTION": "true",
    "CLAUDE_FLOW_GITHUB_INTEGRATION": "true"
  },
  "permissions": {
    "allow": [
      "Bash(npx claude-flow *)",
      "Bash(npm run lint)",
      "Bash(npm run test:*)",
      "Bash(npm test *)",
      "Bash(git status)",
      "Bash(git diff *)",
      "Bash(git log *)",
      "Bash(git add *)",
      "Bash(git commit *)",
      "Bash(git push)",
      "Bash(git config *)",
      "Bash(gh *)",
      "Bash(node *)",
      "Bash(which *)",
      "Bash(pwd)",
      "Bash(ls *)",
      "Bash(ping *)",
      "Bash(nc *)",
      "Bash(python3 *)",
      "Bash(curl *)",
      "Bash(timeout *)"
    ],
    "deny": [
      "Bash(rm -rf /)",
      "Bash(curl * | bash)",
      "Bash(wget * | sh)",
      "Bash(eval *)"
    ]
  },
  "hooks": {},
  "includeCoAuthoredBy": true,
  "enabledMcpjsonServers": ["claude-flow", "ruv-swarm", "blender-tcp"]
}
EOF

# Create local settings for project-specific overrides
cat > /workspace/.claude/settings.local.json << 'EOF'
{
  "permissions": {
    "allow": [
      "mcp__ruv-swarm",
      "mcp__claude-flow",
      "mcp__blender",
      "mcp__blender-tcp"
    ],
    "deny": []
  },
  "enableAllProjectMcpServers": true,
  "enabledMcpjsonServers": [
    "claude-flow",
    "ruv-swarm",
    "blender-tcp"
  ]
}
EOF

# Create MCP configuration with proper networking
REMOTE_HOST=${REMOTE_MCP_HOST:-192.168.0.216}
cat > /workspace/.mcp.json << EOF
{
  "mcpServers": {
    "claude-flow": {
      "command": "npx",
      "args": [
        "claude-flow@alpha",
        "mcp",
        "start"
      ],
      "type": "stdio"
    },
    "ruv-swarm": {
      "command": "npx",
      "args": [
        "ruv-swarm@latest",
        "mcp",
        "start"
      ],
      "type": "stdio"
    },
    "blender-tcp": {
      "transport": "tcp",
      "host": "${REMOTE_HOST}",
      "port": 9876
    }
  }
}
EOF

# Initialize Claude Flow if not already done
if [ ! -f "/workspace/.claude-flow-initialized" ]; then
    echo "First-time Claude Flow initialization..."
    cd /workspace
    su - dev -c "cd /workspace && npx claude-flow@alpha init --force --hive-mind --neural-enhanced" || true
    touch /workspace/.claude-flow-initialized
fi

# Add MCP servers to Claude Code
echo "--- Configuring Claude Code MCP Servers ---"
su - dev -c "claude mcp add claude-flow npx claude-flow@alpha mcp start" 2>/dev/null || echo "Claude Flow MCP already configured"
su - dev -c "claude mcp add ruv-swarm npx ruv-swarm mcp start" 2>/dev/null || echo "Ruv Swarm MCP already configured"

# Set proper permissions
chown -R dev:dev /workspace/.claude /workspace/.mcp.json

# Start MCP Servers
echo "--- Starting MCP Servers ---"

# Start Blender MCP Server with proper network binding
# echo "Starting Blender MCP Server on port 9876..."
# tmux new-session -d -s blender-mcp "
#     ${BLENDER_PATH}/blender \
#         --background \
#         --python /app/keep_alive.py \
#         -- \
#         --blendermcp-autostart \
#         --blendermcp-port 9876 \
#         --blendermcp-host 0.0.0.0 \
#         2>&1 | tee /app/mcp-logs/blender-mcp.log
# "

# Start Revit MCP Server
echo "Starting Revit MCP Server on port 8080..."
tmux new-session -d -s revit-mcp "
    cd /app/revit-mcp && \
    PORT=8080 HOST=0.0.0.0 node build/index.js \
    2>&1 | tee /app/mcp-logs/revit-mcp.log
"

# Start Unreal MCP Server
echo "Starting Unreal MCP Server on port 55557..."
tmux new-session -d -s unreal-mcp "
    cd /app/unreal-mcp-source/Python && \
    UNREAL_HOST=0.0.0.0 UNREAL_PORT=55557 /opt/venv312/bin/python unreal_mcp_server.py \
    2>&1 | tee /app/mcp-logs/unreal-mcp.log
"

# Wait for all MCP servers to be ready
echo "--- Waiting for MCP Servers to be ready ---"
MCP_HOST=${REMOTE_MCP_HOST:-localhost}
wait_for_port $MCP_HOST 9876 60   || echo "Warning: Blender MCP server not found at $MCP_HOST:9876. Continuing..."
wait_for_port $MCP_HOST 8080 30   || echo "Warning: Revit MCP server not found at $MCP_HOST:8080. Continuing..."
wait_for_port $MCP_HOST 55557 30  || echo "Warning: Unreal MCP server not found at $MCP_HOST:55557. Continuing..."

# Configure Claude Code MCP settings
echo "--- Configuring Claude Code MCP Settings ---"
mkdir -p /home/dev/.claude
cat > /home/dev/.claude/settings.json << 'EOF'
{
  "mcpServers": {
    "blender": {
      "command": "uvx",
      "args": ["blender-mcp"],
      "env": {
        "BLENDER_HOST": "localhost",
        "BLENDER_PORT": "9876"
      }
    },
    "blender-tcp": {
      "_comment": "Alternative TCP connection method",
      "transport": "tcp",
      "host": "localhost",
      "port": 9876
    },
    "revit": {
      "command": "node",
      "args": ["/app/revit-mcp/build/index.js"],
      "env": {
        "PORT": "8080",
        "HOST": "localhost"
      }
    },
    "unreal": {
      "command": "uv",
      "args": [
        "--directory",
        "/app/unreal-mcp-source/Python",
        "run",
        "unreal_mcp_server.py"
      ],
      "env": {
        "UNREAL_HOST": "localhost",
        "UNREAL_PORT": "55557"
      }
    },
    "ruv-swarm": {
      "command": "npx",
      "args": ["ruv-swarm", "mcp", "start"]
    },
    "claude-flow": {
      "command": "npx",
      "args": ["claude-flow@alpha", "mcp", "start"]
    }
  }
}
EOF

echo "--- Blender MCP process started in the background ---"


# Create helpful aliases and functions
cat >> /home/dev/.bashrc << 'EOF'

# MCP Environment Variables
export REMOTE_MCP_HOST="${REMOTE_MCP_HOST:-192.168.0.216}"

# MCP Testing Functions
test_mcp_connection() {
    local host=$1
    local port=$2
    local name=$3
    
    if python3 -c "
import socket
try:
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    result = sock.connect_ex(('$host', $port))
    sock.close()
    exit(0 if result == 0 else 1)
except:
    exit(1)
" 2>/dev/null; then
        echo "✅ $name ($host:$port) is accessible"
        return 0
    else
        echo "❌ $name ($host:$port) is not accessible"
        return 1
    fi
}

# MCP Management Aliases
alias mcp-test-blender='test_mcp_connection ${REMOTE_MCP_HOST} 9876 "Blender MCP"'
alias mcp-test-all='echo "Testing MCP connections..."; mcp-test-blender'
alias mcp-list='claude mcp list'
alias mcp-resources='echo "Available MCP resources:"; claude mcp list'

# Claude Flow shortcuts
alias cf='npx claude-flow@alpha'
alias cf-status='cf status'
alias cf-swarm='cf swarm init'
alias cf-help='cf --help'

# Navigation
alias cdw='cd /workspace'
alias cdc='cd /workspace/.claude'

# Show startup info
mcp-info() {
    echo "🚀 MCP Environment Information"
    echo "==============================="
    echo "Remote Blender Host: ${REMOTE_MCP_HOST}:9876"
    echo ""
    echo "Available MCP Servers:"
    mcp-list
    echo ""
    echo "Quick Commands:"
    echo "  mcp-test-all    - Test all MCP connections"
    echo "  mcp-list        - List configured MCP servers"
    echo "  cf-swarm        - Initialize Claude Flow swarm"
    echo "  cf-status       - Check Claude Flow status"
}

# Auto-display info on login
if [ -n "$PS1" ]; then
    echo ""
    mcp-info
    echo ""
fi
EOF

# Print startup information
echo ""
echo "=== Enhanced MCP Environment Ready ==="
echo ""
echo "Configuration:"
echo "  - Remote Blender MCP: ${REMOTE_HOST}:9876"
echo "  - Claude Flow: Local (stdio)"
echo "  - Ruv Swarm: Local (stdio)"
echo ""
echo "Claude settings configured at:"
echo "  - /workspace/.claude/settings.json"
echo "  - /workspace/.claude/settings.local.json"
echo "  - /workspace/.mcp.json"
echo ""


echo "Network Information:"
echo "  - Container IP: $(hostname -I | awk '{print $1}')"
echo "  - Bridge Network: 172.20.0.0/16"
echo "  - From container to host: host.docker.internal or host-gateway"
echo "  - From host to container: localhost:PORT"
echo ""

echo "Management:"
echo "  - Run 'mcp-status' to check server status"
echo "  - Run 'mcp-logs' to view logs"
echo "  - Run 'tmux ls' to see background sessions"
echo ""

# First run logic for interactive session
FIRST_RUN_MARKER="/workspace/.first_run_complete"
if [ ! -f "$FIRST_RUN_MARKER" ] && [ "$1" = "--interactive" ]; then
    echo "--- First run: setting up Claude login ---"
    touch "$FIRST_RUN_MARKER"
    # Try to login to Claude (may fail if already logged in)
    su - dev -c "claude login" 2>/dev/null || true
fi

# Start appropriate process based on command
if [ "$1" = "--interactive" ]; then
    echo "--- Starting interactive shell ---"
    exec su - dev
else
    echo "--- Running command: $@ ---"
    exec "$@"
fi