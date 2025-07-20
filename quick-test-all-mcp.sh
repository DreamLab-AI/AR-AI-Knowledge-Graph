#!/bin/bash

echo "=== MCP Server Quick Test Script ==="
echo ""

# Test Blender connection
echo "1. Testing Blender MCP Connection..."
python /workspace/test-blender-connection.py
echo ""

# Check Revit MCP build
echo "2. Checking Revit MCP Server..."
if [ -f "/workspace/revit-mcp/build/index.js" ]; then
    echo "✅ Revit MCP server is built and ready"
    echo "   Location: /workspace/revit-mcp/build/index.js"
else
    echo "❌ Revit MCP server needs to be built"
    echo "   Run: cd /workspace/revit-mcp && npm run build"
fi
echo ""

# Check Unreal MCP
echo "3. Checking Unreal MCP Server..."
if [ -f "/workspace/unreal-mcp-source/Python/unreal_mcp_server.py" ]; then
    echo "✅ Unreal MCP server is available"
    echo "   Location: /workspace/unreal-mcp-source/Python/unreal_mcp_server.py"
else
    echo "❌ Unreal MCP server not found"
fi
echo ""

# Show configuration
echo "4. Suggested Claude Code Configuration:"
echo "   Configuration file: /workspace/mcp-integration-config.json"
echo ""
cat /workspace/mcp-integration-config.json
echo ""

echo "=== Setup Instructions ==="
echo "1. Install addons/plugins in your host applications:"
echo "   - Blender: Install /workspace/blender-mcp-source/addon.py"
echo "   - Revit: Install revit-mcp-plugin from GitHub"
echo "   - Unreal: Copy UnrealMCP plugin to your project"
echo ""
echo "2. Start the applications and enable their MCP servers"
echo ""
echo "3. Add servers to Claude Code using:"
echo "   claude mcp add blender uvx blender-mcp"
echo "   claude mcp add revit node /workspace/revit-mcp/build/index.js"
echo "   claude mcp add unreal uv --directory /workspace/unreal-mcp-source/Python run unreal_mcp_server.py"