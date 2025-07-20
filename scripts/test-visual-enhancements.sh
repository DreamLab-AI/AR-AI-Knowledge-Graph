#!/bin/bash

# Script to test visual enhancements by updating settings

echo "🎨 Testing Visual Enhancements..."
echo "================================"

# Base URL
BASE_URL="http://localhost:3001"

# Enable hologram effects
echo -e "\n✨ Enabling hologram effects..."
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.nodes.enableHologram",
    "value": true
  }' -s | jq '.'

# Enable flow effects
echo -e "\n🌊 Enabling edge flow effects..."
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.edges.enableFlowEffect",
    "value": true
  }' -s | jq '.'

# Configure flow parameters
echo -e "\n⚡ Configuring flow parameters..."
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.edges.flowSpeed",
    "value": 2.0
  }' -s | jq '.'

curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.edges.flowIntensity",
    "value": 0.8
  }' -s | jq '.'

# Enable bloom
echo -e "\n💫 Enabling bloom effects..."
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.bloom.enabled",
    "value": true
  }' -s | jq '.'

# Configure bloom strength
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.bloom.strength",
    "value": 1.5
  }' -s | jq '.'

# Enable gradient edges
echo -e "\n🌈 Enabling gradient edges..."
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.edges.useGradient",
    "value": true
  }' -s | jq '.'

curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.edges.gradientColors",
    "value": ["#00ffff", "#ff00ff"]
  }' -s | jq '.'

# Enable animations
echo -e "\n🎭 Enabling animations..."
curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.animation.pulseEnabled",
    "value": true
  }' -s | jq '.'

curl -X PUT "$BASE_URL/api/user-settings" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "visualisation.animation.pulseSpeed",
    "value": 1.5
  }' -s | jq '.'

# Get current settings to verify
echo -e "\n📊 Current visual settings:"
curl -X GET "$BASE_URL/api/user-settings" -s | jq '.visualisation'

echo -e "\n✅ Visual enhancements enabled!"
echo "Refresh your browser to see the new effects:"
echo "- Holographic nodes with scanlines"
echo "- Flowing particles on edges"
echo "- Bloom and glow effects"
echo "- Gradient edge coloring"