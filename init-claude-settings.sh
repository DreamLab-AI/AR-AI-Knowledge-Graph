#!/bin/bash
# init-claude-settings.sh - Initialize Claude Code settings for MCP environment

set -e

echo "🔧 Initializing Claude Code settings for MCP environment..."

# Create directories
mkdir -p /workspace/.claude /workspace/.mcp /workspace/memory /workspace/logs

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
      "Bash(timeout *)",
      "Bash(uvx *)"
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

# Create local settings
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

# Create MCP configuration
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

# Copy CLAUDE.md if it exists
if [ -f /workspace/ext/multi-agent-docker/workspace/CLAUDE.md ]; then
    cp /workspace/ext/multi-agent-docker/workspace/CLAUDE.md /workspace/CLAUDE.md
fi

# Set permissions
chown -R dev:dev /workspace/.claude /workspace/.mcp.json /workspace/memory

echo "✅ Claude Code settings initialized successfully!"
echo ""
echo "📁 Configuration files created:"
echo "   - /workspace/.claude/settings.json"
echo "   - /workspace/.claude/settings.local.json"
echo "   - /workspace/.mcp.json"
echo ""
echo "🔌 MCP Servers configured:"
echo "   - claude-flow (local stdio)"
echo "   - ruv-swarm (local stdio)"
echo "   - blender-tcp (remote at ${REMOTE_HOST}:9876)"