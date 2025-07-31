#!/bin/bash

# Test script for verifying fixed endpoints

echo "=== Testing Fixed MCP Health Endpoints ==="
echo ""

echo "1. Testing /api/mcp/health"
curl -s http://localhost:3001/api/mcp/health | jq . || echo "Failed"

echo ""
echo "2. Testing /api/health"
curl -s http://localhost:3001/api/health | jq . || echo "Failed"

echo ""
echo "3. Testing /api/health/physics"
curl -s http://localhost:3001/api/health/physics | jq . || echo "Failed"

echo ""
echo "4. Testing swarm initialization with timeout handling"
echo "Request payload:"
cat <<EOF | jq .
{
  "topology": "mesh",
  "maxAgents": 3,
  "strategy": "balanced",
  "enableNeural": false,
  "agentTypes": ["researcher", "coder", "analyst"],
  "customPrompt": "Test swarm"
}
EOF

echo ""
echo "Sending request..."
curl -s -X POST http://localhost:3001/api/bots/initialize-swarm \
  -H "Content-Type: application/json" \
  -d '{
    "topology": "mesh",
    "maxAgents": 3,
    "strategy": "balanced",
    "enableNeural": false,
    "agentTypes": ["researcher", "coder", "analyst"],
    "customPrompt": "Test swarm"
  }' | jq .

echo ""
echo "=== Summary ==="
echo "The backend should now:"
echo "1. Not crash on startup (ClaudeFlowActor handles connection failures gracefully)"
echo "2. Respond to requests even without MCP connection"
echo "3. Return proper timeout errors instead of hanging"
echo "4. Have working health endpoints"