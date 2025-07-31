#!/usr/bin/env node

const WebSocket = require('ws');
const http = require('http');
const { spawn } = require('child_process');

// Configuration
const PORT = process.env.MCP_PORT || 3000;
const HOST = process.env.MCP_HOST || '0.0.0.0';

// Create HTTP server for health checks and WebSocket upgrade
const server = http.createServer((req, res) => {
  // Handle health check endpoint
  if (req.url === '/api/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      status: 'healthy',
      service: 'mcp-relay',
      version: '1.0.0',
      timestamp: new Date().toISOString()
    }));
    return;
  }

  // Handle root endpoint
  if (req.url === '/') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      service: 'Claude Flow MCP WebSocket Relay',
      endpoints: {
        health: '/api/health',
        websocket: 'ws://' + req.headers.host + '/ws'
      }
    }));
    return;
  }

  // 404 for other paths
  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'Not found' }));
});

// Create WebSocket server
const wss = new WebSocket.Server({ server, path: '/ws' });

console.log(`Starting MCP HTTP/WebSocket relay on port ${PORT}...`);

// Handle WebSocket connections
wss.on('connection', (ws) => {
  console.log('New WebSocket connection established');

  // Spawn Claude Flow MCP process with stdio mode
  const mcpProcess = spawn('npx', ['claude-flow@alpha', 'mcp', 'start', '--stdio'], {
    stdio: ['pipe', 'pipe', 'pipe'],
    cwd: '/workspace',
    env: {
      ...process.env,
      CLAUDE_FLOW_AUTO_ORCHESTRATOR: 'true',
      CLAUDE_FLOW_NEURAL_ENABLED: 'true',
      CLAUDE_FLOW_WASM_ENABLED: 'true'
    }
  });

  let buffer = '';

  // Handle MCP stdout
  mcpProcess.stdout.on('data', (data) => {
    buffer += data.toString();
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    lines.forEach(line => {
      line = line.trim();
      if (line && line.startsWith('{') && line.includes('"jsonrpc"')) {
        try {
          const message = JSON.parse(line);
          console.log('MCP -> WS:', JSON.stringify(message).substring(0, 100) + '...');
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify(message));
          }
        } catch (e) {
          console.error('Failed to parse JSON-RPC message:', e);
        }
      }
    });
  });

  // Handle MCP stderr
  mcpProcess.stderr.on('data', (data) => {
    const msg = data.toString();
    if (!msg.includes('Warning: Detected unsettled')) {
      console.error('MCP stderr:', msg);
    }
  });

  // Handle WebSocket messages
  ws.on('message', (message) => {
    try {
      const data = JSON.parse(message.toString());
      console.log('WS -> MCP:', JSON.stringify(data).substring(0, 100) + '...');
      
      // Send to MCP process stdin
      mcpProcess.stdin.write(JSON.stringify(data) + '\n');
    } catch (e) {
      console.error('Failed to parse WebSocket message:', e);
      ws.send(JSON.stringify({
        jsonrpc: "2.0",
        error: { code: -32700, message: "Parse error" },
        id: null
      }));
    }
  });

  // Handle WebSocket close
  ws.on('close', () => {
    console.log('WebSocket connection closed');
    mcpProcess.kill();
  });

  // Handle MCP process exit
  mcpProcess.on('exit', (code) => {
    console.log(`MCP process exited with code ${code}`);
    if (ws.readyState === WebSocket.OPEN) {
      ws.close();
    }
  });

  // Handle process errors
  mcpProcess.on('error', (err) => {
    console.error('MCP process error:', err);
    if (ws.readyState === WebSocket.OPEN) {
      ws.close();
    }
  });

  console.log('WebSocket client connected, MCP process spawned');
});

// Start server
server.listen(PORT, HOST, () => {
  console.log(`MCP HTTP/WebSocket relay listening on http://${HOST}:${PORT}`);
  console.log(`Health check: http://${HOST}:${PORT}/api/health`);
  console.log(`WebSocket: ws://${HOST}:${PORT}/ws`);
});

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('\nShutting down MCP relay...');
  wss.close(() => {
    server.close(() => {
      process.exit(0);
    });
  });
});