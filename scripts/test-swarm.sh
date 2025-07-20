#!/bin/bash

# Test script for swarm visualization

echo "🐝 Testing Swarm Visualization Integration..."

# Check if services are running
echo "Checking services..."
supervisorctl status

# Send test swarm data to the backend
echo -e "\n📊 Sending test swarm data to backend..."
curl -X POST http://localhost:3001/api/swarm/update \
  -H "Content-Type: application/json" \
  -d '{
    "nodes": [
      {"id": "agent-1", "type": "coordinator", "status": "active", "name": "Coordinator Alpha", "cpuUsage": 45, "health": 95, "workload": 0.7},
      {"id": "agent-2", "type": "coder", "status": "active", "name": "Coder Beta", "cpuUsage": 78, "health": 88, "workload": 0.9},
      {"id": "agent-3", "type": "tester", "status": "active", "name": "Tester Gamma", "cpuUsage": 32, "health": 92, "workload": 0.5},
      {"id": "agent-4", "type": "analyst", "status": "active", "name": "Analyst Delta", "cpuUsage": 56, "health": 90, "workload": 0.6}
    ],
    "edges": [
      {"id": "edge-1", "source": "agent-1", "target": "agent-2", "dataVolume": 1024, "messageCount": 15},
      {"id": "edge-2", "source": "agent-1", "target": "agent-3", "dataVolume": 512, "messageCount": 8},
      {"id": "edge-3", "source": "agent-2", "target": "agent-4", "dataVolume": 2048, "messageCount": 22}
    ]
  }'

# Get swarm data from backend
echo -e "\n📥 Fetching swarm data from backend..."
curl -X GET http://localhost:3001/api/swarm/data | jq '.'

echo -e "\n✅ Test complete! Check the browser at http://192.168.0.51:3001 to see the swarm visualization."
echo "The swarm should appear as a separate graph with gold/green nodes showing agent activity."