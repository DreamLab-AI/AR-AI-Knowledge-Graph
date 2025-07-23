#!/usr/bin/env node

/**
 * Test script to verify Claude Flow stdio communication
 * This simulates what the Rust stdio transport does
 */

const { spawn } = require('child_process');
const readline = require('readline');

console.log('Testing Claude Flow stdio communication...\n');

// Spawn Claude Flow MCP process
const mcpProcess = spawn('npx', ['claude-flow@alpha', 'mcp', 'start'], {
  stdio: ['pipe', 'pipe', 'pipe']
});

// Create readline interface for stdout
const rl = readline.createInterface({
  input: mcpProcess.stdout,
  crlfDelay: Infinity
});

// Track if we've seen the initialization
let initialized = false;
let messageCount = 0;

// Handle stdout line by line
rl.on('line', (line) => {
  messageCount++;
  console.log(`[MCP stdout ${messageCount}]: ${line}`);
  
  try {
    const json = JSON.parse(line);
    console.log('[Parsed JSON]:', JSON.stringify(json, null, 2));
    
    if (json.method === 'server.initialized') {
      console.log('\n✅ Server initialized successfully!\n');
      initialized = true;
      
      // Send an initialize request
      const initRequest = {
        jsonrpc: '2.0',
        id: '1',
        method: 'initialize',
        params: {
          protocolVersion: { major: 2024, minor: 11, patch: 5 },
          clientInfo: { name: 'Test Client', version: '1.0.0' },
          capabilities: { tools: { listChanged: true } }
        }
      };
      
      console.log('Sending initialize request...');
      mcpProcess.stdin.write(JSON.stringify(initRequest) + '\n');
    }
    
    if (json.id === '1' && json.result) {
      console.log('\n✅ Initialize response received!\n');
      
      // Try listing tools
      const listToolsRequest = {
        jsonrpc: '2.0',
        id: '2',
        method: 'tools/list',
        params: {}
      };
      
      console.log('Sending tools/list request...');
      mcpProcess.stdin.write(JSON.stringify(listToolsRequest) + '\n');
    }
    
    if (json.id === '2' && json.result) {
      console.log('\n✅ Tools list received!\n');
      console.log(`Found ${json.result.tools?.length || 0} tools`);
      
      // Success! Terminate the process
      console.log('\n🎉 All tests passed! Stdio communication works correctly.\n');
      mcpProcess.kill();
      process.exit(0);
    }
  } catch (e) {
    // Not JSON, probably initialization message
    if (!line.includes('INFO') && !line.includes('WARN')) {
      console.log('[Non-JSON output]');
    }
  }
});

// Handle stderr
mcpProcess.stderr.on('data', (data) => {
  const lines = data.toString().split('\n').filter(line => line.trim());
  lines.forEach(line => {
    if (line.includes('ERROR')) {
      console.error(`[MCP stderr ERROR]: ${line}`);
    } else {
      console.log(`[MCP stderr]: ${line}`);
    }
  });
});

// Handle process exit
mcpProcess.on('exit', (code) => {
  console.log(`\nMCP process exited with code ${code}`);
  
  if (!initialized) {
    console.error('❌ Failed to initialize MCP server');
    process.exit(1);
  }
});

// Timeout after 10 seconds
setTimeout(() => {
  console.error('\n❌ Timeout: Test did not complete within 10 seconds');
  mcpProcess.kill();
  process.exit(1);
}, 10000);

console.log('Waiting for MCP server to start...\n');