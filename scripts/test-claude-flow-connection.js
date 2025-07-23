#!/usr/bin/env node
/**
 * Test script to establish connection with Claude Flow MCP
 * and retrieve agent/swarm information
 */

import { spawn } from 'child_process';
import { v4 as uuidv4 } from 'uuid';

console.log('🚀 Testing Claude Flow MCP Connection...\n');

// Spawn the MCP server
const mcp = spawn('npx', ['claude-flow@alpha', 'mcp', 'start'], {
  stdio: ['pipe', 'pipe', 'pipe'],
  env: {
    ...process.env,
    CLAUDE_FLOW_AUTO_ORCHESTRATOR: 'true',
    CLAUDE_FLOW_NEURAL_ENABLED: 'true',
    CLAUDE_FLOW_WASM_ENABLED: 'true',
  }
});

let buffer = '';
const pendingRequests = new Map();

// Handle stdout from MCP
mcp.stdout.on('data', (data) => {
  buffer += data.toString();
  
  // Process complete JSON messages
  const lines = buffer.split('\n');
  buffer = lines.pop() || '';
  
  for (const line of lines) {
    if (line.trim()) {
      try {
        const message = JSON.parse(line);
        console.log('📨 Received:', JSON.stringify(message, null, 2));
        
        // Handle responses to our requests
        if (message.id && pendingRequests.has(message.id)) {
          const { resolve } = pendingRequests.get(message.id);
          pendingRequests.delete(message.id);
          resolve(message);
        }
      } catch (err) {
        console.log('📝 Non-JSON output:', line);
      }
    }
  }
});

// Handle stderr
mcp.stderr.on('data', (data) => {
  console.error('⚠️  MCP stderr:', data.toString());
});

// Handle process exit
mcp.on('exit', (code) => {
  console.log(`\n💤 MCP process exited with code ${code}`);
  process.exit(code);
});

// Send a request and wait for response
function sendRequest(request) {
  return new Promise((resolve, reject) => {
    const requestId = request.id || uuidv4();
    request.id = requestId;
    
    pendingRequests.set(requestId, { resolve, reject });
    
    console.log('📤 Sending:', JSON.stringify(request, null, 2));
    mcp.stdin.write(JSON.stringify(request) + '\n');
    
    // Timeout after 10 seconds
    setTimeout(() => {
      if (pendingRequests.has(requestId)) {
        pendingRequests.delete(requestId);
        reject(new Error('Request timeout'));
      }
    }, 10000);
  });
}

// Run test sequence
async function runTests() {
  try {
    console.log('\n1️⃣  Initializing MCP connection...\n');
    
    // Wait a bit for the server to start
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    // Initialize
    const initResponse = await sendRequest({
      jsonrpc: '2.0',
      method: 'initialize',
      params: {
        capabilities: {}
      },
      id: 'init'
    });
    
    console.log('\n✅ Initialization successful!\n');
    
    // List available tools
    console.log('\n2️⃣  Listing available tools...\n');
    const toolsResponse = await sendRequest({
      jsonrpc: '2.0',
      method: 'tools/list',
      params: {},
      id: 'tools-list'
    });
    
    if (toolsResponse.result?.tools) {
      console.log(`\n📦 Found ${toolsResponse.result.tools.length} tools\n`);
      console.log('First 5 tools:', toolsResponse.result.tools.slice(0, 5).map(t => t.name));
    }
    
    // Check swarm status
    console.log('\n3️⃣  Checking swarm status...\n');
    const swarmStatusResponse = await sendRequest({
      jsonrpc: '2.0',
      method: 'tools/call',
      params: {
        name: 'swarm_status',
        arguments: {}
      },
      id: 'swarm-status'
    });
    
    if (swarmStatusResponse.result?.content?.[0]?.text) {
      const swarmData = JSON.parse(swarmStatusResponse.result.content[0].text);
      console.log('\n🐝 Swarm Status:', swarmData);
    }
    
    // List agents
    console.log('\n4️⃣  Listing agents...\n');
    const agentListResponse = await sendRequest({
      jsonrpc: '2.0',
      method: 'tools/call',
      params: {
        name: 'agent_list',
        arguments: {}
      },
      id: 'agent-list'
    });
    
    if (agentListResponse.result?.content?.[0]?.text) {
      const agentData = JSON.parse(agentListResponse.result.content[0].text);
      console.log('\n🤖 Agents:', agentData);
    }
    
    console.log('\n✅ All tests completed successfully!\n');
    
    // Keep connection open for manual testing
    console.log('📌 Connection remains open. Press Ctrl+C to exit.\n');
    
  } catch (error) {
    console.error('\n❌ Test failed:', error.message);
    process.exit(1);
  }
}

// Start tests after a short delay
setTimeout(runTests, 500);

// Handle Ctrl+C
process.on('SIGINT', () => {
  console.log('\n\n👋 Shutting down...');
  mcp.kill();
  process.exit(0);
});