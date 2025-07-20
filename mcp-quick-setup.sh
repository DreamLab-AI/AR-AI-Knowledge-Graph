#!/bin/bash
# Quick MCP Server Setup Script for Claude Code
# This script automates the installation of common MCP servers

echo "🚀 MCP Server Quick Setup for Claude Code"
echo "========================================"

# Function to check if server already exists
check_server_exists() {
    claude mcp list | grep -q "^$1:" && return 0 || return 1
}

# Function to add server with confirmation
add_server() {
    local name=$1
    local cmd=$2
    
    if check_server_exists "$name"; then
        echo "✓ $name already configured"
    else
        echo "📦 Adding $name MCP server..."
        claude mcp add $name $cmd
        echo "✅ $name added successfully"
    fi
}

# 1. Blender MCP (Special case - network connection)
echo ""
echo "1️⃣ Blender MCP Server (TCP Connection)"
if check_server_exists "blender"; then
    echo "✓ Blender already configured"
else
    echo "📦 Adding Blender MCP server..."
    # Check if we're inside Docker or on host
    if [ -f /.dockerenv ]; then
        # Inside Docker container
        claude mcp add-json blender '{
          "transport": "tcp",
          "host": "localhost", 
          "port": 9876
        }'
    else
        # On host machine - try uvx first
        if command -v uvx &> /dev/null; then
            claude mcp add blender uvx blender-mcp
        else
            claude mcp add-json blender '{
              "transport": "tcp",
              "host": "localhost",
              "port": 9876
            }'
        fi
    fi
    echo "✅ Blender added successfully"
fi

# 2. GitHub MCP
echo ""
echo "2️⃣ GitHub MCP Server"
if [ -z "$GITHUB_TOKEN" ]; then
    echo "⚠️  Warning: GITHUB_TOKEN not set. GitHub MCP may not work properly."
    echo "   Set it with: export GITHUB_TOKEN=your_token"
fi
add_server "github" "npx @modelcontextprotocol/server-github"

# 3. Filesystem MCP
echo ""
echo "3️⃣ Filesystem MCP Server"
add_server "filesystem" "npx @modelcontextprotocol/server-filesystem --allowed-directories /workspace"

# 4. PostgreSQL MCP
echo ""
echo "4️⃣ PostgreSQL MCP Server"
if [ -z "$DATABASE_URL" ]; then
    echo "⚠️  Warning: DATABASE_URL not set. PostgreSQL MCP will need configuration."
    echo "   Set it with: export DATABASE_URL=postgresql://user:pass@host:port/db"
fi
add_server "postgres" "npx @modelcontextprotocol/server-postgres"

# 5. Puppeteer MCP
echo ""
echo "5️⃣ Puppeteer MCP Server"
add_server "puppeteer" "npx @modelcontextprotocol/server-puppeteer"

# 6. Fetch MCP
echo ""
echo "6️⃣ Fetch MCP Server"
add_server "fetch" "npx @modelcontextprotocol/server-fetch"

# Display final status
echo ""
echo "📋 Final MCP Server Configuration:"
echo "=================================="
claude mcp list

echo ""
echo "✨ Setup complete! You can now use MCP tools in Claude Code."
echo ""
echo "🧪 Test commands:"
echo "  - Blender: mcp__blender__get_scene_info"
echo "  - GitHub: mcp__github__search_repositories" 
echo "  - Filesystem: mcp__filesystem__read_file"
echo ""
echo "📖 For more details, see: /workspace/mcp-integration-plan.md"