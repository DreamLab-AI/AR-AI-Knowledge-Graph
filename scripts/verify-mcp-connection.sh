#!/bin/bash

echo "=== MCP Connection Verification Script ==="
echo "This script will help verify MCP connectivity"
echo ""

# Test from inside multi-agent-container (where we are now)
echo "1. Testing MCP server locally (from multi-agent-container):"
echo -n "   Testing localhost:9500... "
if echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":true}},"clientInfo":{"name":"test","version":"1.0.0"}}}' | timeout 2 nc localhost 9500 > /dev/null 2>&1; then
    echo "✅ SUCCESS"
else
    echo "❌ FAILED"
fi

echo ""
echo "2. Network configuration:"
echo "   Container: multi-agent-container"
echo "   IP Address: $(hostname -I | awk '{print $1}')"
echo "   MCP Port: 9500"

echo ""
echo "3. WebXR container must connect to:"
echo "   Host: multi-agent-container"
echo "   Port: 9500"
echo "   Full address: multi-agent-container:9500"

echo ""
echo "4. Environment variables to set in WebXR container:"
echo "   MCP_HOST=multi-agent-container"
echo "   MCP_TCP_PORT=9500"

echo ""
echo "5. Code changes needed in WebXR (visionclaw container):"
echo ""
echo "   In src/actors/claude_flow_actor.rs:"
echo "   - Line 100: Set host to 'multi-agent-container'"
echo ""
echo "   In src/services/bots_client.rs (if using TCP):"
echo "   - Connect to 'multi-agent-container:9500'"
echo ""

echo "6. Test command to run FROM WebXR container:"
echo '   echo '"'"'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}'"'"' | nc multi-agent-container 9500'