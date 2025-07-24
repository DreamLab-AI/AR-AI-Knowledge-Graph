const WebSocket = require('ws');
const { spawn } = require('child_process');
const http = require('http');

// Get port from environment or use default
const PORT = process.env.MCP_PORT || 3000;
const HOST = process.env.MCP_HOST || '0.0.0.0';

// Create HTTP server
const server = http.createServer();

// Create WebSocket server
const wss = new WebSocket.Server({ server, path: '/ws' });

console.log(`Starting MCP WebSocket relay on port ${PORT}...`);

wss.on('connection', (ws) => {
  console.log('New WebSocket connection established');

  // Spawn Claude Flow MCP process with stdio mode
  const mcpProcess = spawn('npx', ['claude-flow@alpha', 'mcp', 'start', '--stdio'], {
    stdio: ['pipe', 'pipe', 'pipe'],
    cwd: '/workspace/ext/claude-flow',
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

  // Don't send any initial message - wait for client to initialize
  console.log('WebSocket client connected, waiting for initialization...');
});

server.listen(PORT, HOST, () => {
  console.log(`MCP WebSocket relay listening on http://${HOST}:${PORT}/ws`);
});