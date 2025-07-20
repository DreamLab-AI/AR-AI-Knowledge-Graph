#!/usr/bin/env node

// Test script for MCP WebSocket connection
const WebSocket = require('ws');

const WS_URL = process.env.WS_URL || 'ws://localhost:9001';

console.log(`🔍 Testing WebSocket connection to: ${WS_URL}`);

const ws = new WebSocket(WS_URL);

ws.on('open', () => {
  console.log('✅ Connected to MCP WebSocket');
  
  // Send a test request for agents
  const request = {
    type: 'mcp-request',
    requestId: `test_${Date.now()}`,
    jsonrpc: '2.0',
    id: `test_${Date.now()}`,
    method: 'tools/call',
    params: {
      name: 'agents/list',
      arguments: {}
    }
  };
  
  console.log('📤 Sending request for agents list...');
  ws.send(JSON.stringify(request));
});

ws.on('message', (data) => {
  try {
    const message = JSON.parse(data.toString());
    console.log('\n📥 Received message:');
    console.log('Type:', message.type);
    
    if (message.type === 'welcome') {
      console.log('Client ID:', message.clientId);
      console.log('Initial data:', JSON.stringify(message.data, null, 2));
    } else if (message.type === 'mcp-update') {
      console.log('Update data:', JSON.stringify(message.data, null, 2));
    } else if (message.type === 'mcp-response') {
      console.log('Response data:', JSON.stringify(message.data, null, 2));
    }
    
    // Close after receiving a few messages
    setTimeout(() => {
      console.log('\n👋 Closing connection...');
      ws.close();
    }, 5000);
  } catch (error) {
    console.error('❌ Error parsing message:', error);
  }
});

ws.on('error', (error) => {
  console.error('❌ WebSocket error:', error.message);
  console.error('Make sure the MCP orchestrator is running:');
  console.error('  docker-compose up -d mcp-orchestrator');
});

ws.on('close', () => {
  console.log('🔌 Connection closed');
  process.exit(0);
});