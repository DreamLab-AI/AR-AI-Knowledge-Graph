const WebSocket = require('ws');
const { spawn } = require('child_process');
const http = require('http');

const PORT = process.env.MCP_PORT || 3000;
const HOST = process.env.MCP_HOST || '0.0.0.0';

const server = http.createServer();
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

  // Handle MCP stdout - FIXED: Better JSON parsing
  mcpProcess.stdout.on('data', (data) => {
    buffer += data.toString();
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    lines.forEach(line => {
      line = line.trim();
      if (line) {
        // Only forward actual JSON-RPC messages, skip logs
        if (line.startsWith('{') && line.includes('"jsonrpc"')) {
          try {
            const message = JSON.parse(line);
            console.log('MCP -> WS:', message);
            if (ws.readyState === WebSocket.OPEN) {
              ws.send(JSON.stringify(message));
            }
          } catch (e) {
            console.error('Failed to parse JSON-RPC message:', line, e);
          }
        } else {
          // Log non-JSON messages for debugging
          console.log('MCP log:', line);
        }
      }
    });
  });

  // Handle MCP stderr
  mcpProcess.stderr.on('data', (data) => {
    console.error('MCP stderr:', data.toString());
  });

  // Handle WebSocket messages - send to MCP stdin
  ws.on('message', (message) => {
    try {
      const data = JSON.parse(message.toString());
      console.log('WS -> MCP:', data);
      
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
    console.log('WebSocket connection closed, terminating MCP process');
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

server.listen(PORT, HOST, () => {
  console.log(`MCP WebSocket relay listening on http://${HOST}:${PORT}/ws`);
});

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('\nShutting down MCP WebSocket relay...');
  wss.close(() => {
    process.exit(0);
  });
});