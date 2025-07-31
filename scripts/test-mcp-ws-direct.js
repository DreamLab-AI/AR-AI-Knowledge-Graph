#!/usr/bin/env node

const WebSocket = require('ws');

console.log('Testing direct WebSocket connection to MCP relay...');

const ws = new WebSocket('ws://localhost:3000/ws');

ws.on('open', () => {
  console.log('✅ Connected to MCP WebSocket');
  
  // Send initialize request
  const initRequest = {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      capabilities: {
        tools: {
          listChanged: true
        }
      },
      clientInfo: {
        name: "Test Client",
        version: "1.0.0"
      },
      protocolVersion: "2024-11-05"
    }
  };
  
  console.log('Sending initialize request...');
  ws.send(JSON.stringify(initRequest));
});

ws.on('message', (data) => {
  console.log('Received:', JSON.parse(data.toString()));
  
  // After initialize, try list tools
  const parsed = JSON.parse(data.toString());
  if (parsed.id === 1 && parsed.result) {
    console.log('Initialize successful, listing tools...');
    const listToolsRequest = {
      jsonrpc: "2.0",
      id: 2,
      method: "tools/list",
      params: {}
    };
    ws.send(JSON.stringify(listToolsRequest));
  } else if (parsed.id === 2) {
    console.log('Tools list received, closing connection...');
    ws.close();
  }
});

ws.on('error', (err) => {
  console.error('❌ WebSocket error:', err);
});

ws.on('close', () => {
  console.log('Connection closed');
  process.exit(0);
});

setTimeout(() => {
  console.log('Timeout - closing connection');
  ws.close();
  process.exit(1);
}, 10000);