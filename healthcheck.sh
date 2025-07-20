#!/bin/bash
# Health check script for MCP 3D Environment
# Verifies that at least one MCP server is responding

# Check if MCP servers are responding
BLENDER_OK=false
REVIT_OK=false
UNREAL_OK=false

# Use REMOTE_MCP_HOST if set, otherwise default to localhost
MCP_HOST=${REMOTE_MCP_HOST:-localhost}

# Check Blender MCP (port 9876)
if nc -z $MCP_HOST 9876 2>/dev/null; then
    BLENDER_OK=true
fi

# Check Revit MCP (port 8080)
if nc -z $MCP_HOST 8080 2>/dev/null; then
    REVIT_OK=true
fi

# Check Unreal MCP (port 55557)
if nc -z $MCP_HOST 55557 2>/dev/null; then
    UNREAL_OK=true
fi

# Generate status output
echo "MCP Servers Health Check (Host: $MCP_HOST):"
echo "  Blender MCP (9876): $( [ "$BLENDER_OK" = true ] && echo "✅ UP" || echo "❌ DOWN" )"
echo "  Revit MCP (8080): $( [ "$REVIT_OK" = true ] && echo "✅ UP" || echo "❌ DOWN" )"
echo "  Unreal MCP (55557): $( [ "$UNREAL_OK" = true ] && echo "✅ UP" || echo "❌ DOWN" )"

# Check if at least one server is running
if [ "$BLENDER_OK" = true ] || [ "$REVIT_OK" = true ] || [ "$UNREAL_OK" = true ]; then
    echo "Overall Status: ✅ HEALTHY"
    exit 0
else
    echo "Overall Status: ⚠️  Warning - No MCP servers are responding"
    exit 0
fi