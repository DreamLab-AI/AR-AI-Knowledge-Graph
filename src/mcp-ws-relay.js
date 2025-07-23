const WebSocket = require('ws');
const { spawn } = require('child_process');
const http = require('http');

// Create HTTP server
const server = http.createServer();

// Create WebSocket server
const wss = new WebSocket.Server({ server, path: '/ws' });

console.log('Starting MCP WebSocket relay on port 8081...');

wss.on('connection', (ws) => {
  console.log('New WebSocket connection established');

  // Spawn Claude Flow MCP process
  const mcpProcess = spawn('npx', ['claude-flow@alpha', 'mcp', 'start'], {
    stdio: ['pipe', 'pipe', 'pipe']
  });

  let buffer = '';

  // Handle MCP stdout
  mcpProcess.stdout.on('data', (data) => {
    buffer += data.toString();
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    lines.forEach(line => {
      if (line.trim()) {
        try {
          // Parse JSON-RPC messages
          const message = JSON.parse(line);
          console.log('MCP -> WS:', message);
          ws.send(JSON.stringify(message));
        } catch (e) {
          // Not JSON, might be initialization message
          console.log('MCP stdout:', line);
        }
      }
    });
  });

  // Handle MCP stderr
  mcpProcess.stderr.on('data', (data) => {
    console.error('MCP stderr:', data.toString());
  });

  // Handle WebSocket messages
  ws.on('message', (message) => {
    try {
      const data = JSON.parse(message.toString());
      console.log('WS -> MCP:', data);
      
      // Send to MCP process stdin
      mcpProcess.stdin.write(JSON.stringify(data) + '\n');
    } catch (e) {
      console.error('Failed to parse WebSocket message:', e);
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
    ws.close();
  });

  // Send initial health check response
  ws.send(JSON.stringify({
    jsonrpc: '2.0',
    method: 'health',
    params: {
      status: 'healthy',
      version: '2.0.0-alpha.59'
    }
  }));
});

server.listen(8081, () => {
  console.log('MCP WebSocket relay listening on http://localhost:8081/ws');
});