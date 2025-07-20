const express = require('express');
const WebSocket = require('ws');
const cors = require('cors');
const helmet = require('helmet');
const rateLimit = require('express-rate-limit');
const winston = require('winston');
const axios = require('axios');
const { v4: uuidv4 } = require('uuid');
require('dotenv').config();

// Initialize logger
const logger = winston.createLogger({
  level: process.env.LOG_LEVEL || 'info',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.json()
  ),
  transports: [
    new winston.transports.Console(),
    new winston.transports.File({ filename: '/app/logs/orchestrator.log' }),
    new winston.transports.File({ filename: '/app/logs/mcp-requests.log', level: 'debug' })
  ]
});

// Express app setup
const app = express();
const PORT = process.env.MCP_ORCHESTRATOR_PORT || 9000;
const WS_PORT = process.env.WEBSOCKET_PORT || 9001;

// Middleware
app.use(helmet());
app.use(cors());
app.use(express.json());

// Rate limiting
const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 100 // limit each IP to 100 requests per windowMs
});
app.use('/api/', limiter);

// MCP server configuration
const MCP_SERVERS = process.env.MCP_SERVERS ?
  process.env.MCP_SERVERS.split(',').map(s => {
    const [name, port] = s.split(':');
    return { name, port: parseInt(port) };
  }) : [];

const CLAUDE_FLOW_HOST = process.env.CLAUDE_FLOW_HOST || 'localhost';
const CLAUDE_FLOW_PORT = process.env.CLAUDE_FLOW_PORT || 3000;

// WebSocket server for real-time data
const wss = new WebSocket.Server({ port: WS_PORT });

// Connected clients
const clients = new Map();

// MCP polling state
const pollingIntervals = new Map();
const mcpDataCache = {
  agents: [],
  tokenUsage: {},
  communications: [],
  systemHealth: {},
  lastUpdate: null
};

// MCP Tool Definitions
const MCP_TOOLS = {
  'agents/list': {
    method: 'tools/call',
    params: {
      name: 'agents/list',
      arguments: {}
    }
  },
  'analysis/token-usage': {
    method: 'tools/call',
    params: {
      name: 'analysis/token-usage',
      arguments: {}
    }
  },
  'memory/query': {
    method: 'tools/call',
    params: {
      name: 'memory/query',
      arguments: {
        filter: { type: 'communication' },
        limit: 100,
        sort: { timestamp: -1 }
      }
    }
  },
  'system/health': {
    method: 'tools/call',
    params: {
      name: 'system/health',
      arguments: {}
    }
  }
};

// Function to make JSON-RPC request to MCP server
async function callMCPTool(tool, host = CLAUDE_FLOW_HOST, port = CLAUDE_FLOW_PORT) {
  const toolConfig = MCP_TOOLS[tool];
  if (!toolConfig) {
    throw new Error(`Unknown MCP tool: ${tool}`);
  }

  const request = {
    jsonrpc: '2.0',
    id: uuidv4(),
    method: toolConfig.method,
    params: toolConfig.params
  };

  try {
    logger.debug('MCP Request', { tool, request });

    const response = await axios.post(
      `http://${host}:${port}/api/mcp`,
      request,
      {
        headers: { 'Content-Type': 'application/json' },
        timeout: 10000
      }
    );

    logger.debug('MCP Response', { tool, response: response.data });
    return response.data.result;
  } catch (error) {
    logger.error('MCP Request Failed', { tool, error: error.message });
    throw error;
  }
}

// Poll MCP data
async function pollMCPData() {
  try {
    // Fetch all data in parallel
    const [agents, tokenUsage, communications, systemHealth] = await Promise.allSettled([
      callMCPTool('agents/list'),
      callMCPTool('analysis/token-usage'),
      callMCPTool('memory/query'),
      callMCPTool('system/health')
    ]);

    // Update cache
    if (agents.status === 'fulfilled') {
      mcpDataCache.agents = agents.value.agents || [];
    }
    if (tokenUsage.status === 'fulfilled') {
      mcpDataCache.tokenUsage = tokenUsage.value;
    }
    if (communications.status === 'fulfilled') {
      mcpDataCache.communications = communications.value.memories || [];
    }
    if (systemHealth.status === 'fulfilled') {
      mcpDataCache.systemHealth = systemHealth.value;
    }

    mcpDataCache.lastUpdate = new Date().toISOString();

    // Broadcast to all connected clients
    const update = {
      type: 'mcp-update',
      timestamp: mcpDataCache.lastUpdate,
      data: mcpDataCache
    };

    broadcast(update);
    logger.info('MCP data polled and broadcast', { clientCount: clients.size });

  } catch (error) {
    logger.error('Failed to poll MCP data', { error: error.message });
  }
}

// Broadcast message to all WebSocket clients
function broadcast(message) {
  const payload = JSON.stringify(message);
  clients.forEach((client, id) => {
    if (client.readyState === WebSocket.OPEN) {
      client.send(payload);
    }
  });
}

// WebSocket connection handler
wss.on('connection', (ws, req) => {
  const clientId = uuidv4();
  const clientIp = req.socket.remoteAddress;

  logger.info('WebSocket client connected', { clientId, clientIp });

  // Store client
  clients.set(clientId, ws);

  // Send initial data
  ws.send(JSON.stringify({
    type: 'welcome',
    clientId,
    data: mcpDataCache
  }));

  // Handle messages from client
  ws.on('message', async (message) => {
    try {
      const data = JSON.parse(message);
      logger.debug('WebSocket message received', { clientId, data });

      // Handle different message types
      switch (data.type) {
        case 'subscribe':
          // Client wants to subscribe to specific data
          logger.info('Client subscribed', { clientId, topics: data.topics });
          break;

        case 'mcp-request':
          // Direct MCP tool call
          if (data.tool && MCP_TOOLS[data.tool]) {
            try {
              const result = await callMCPTool(data.tool);
              ws.send(JSON.stringify({
                type: 'mcp-response',
                requestId: data.requestId,
                tool: data.tool,
                result
              }));
            } catch (error) {
              ws.send(JSON.stringify({
                type: 'mcp-error',
                requestId: data.requestId,
                tool: data.tool,
                error: error.message
              }));
            }
          }
          break;

        case 'ping':
          ws.send(JSON.stringify({ type: 'pong', timestamp: new Date().toISOString() }));
          break;
      }
    } catch (error) {
      logger.error('WebSocket message error', { clientId, error: error.message });
    }
  });

  // Handle disconnection
  ws.on('close', () => {
    logger.info('WebSocket client disconnected', { clientId });
    clients.delete(clientId);
  });

  ws.on('error', (error) => {
    logger.error('WebSocket error', { clientId, error: error.message });
  });
});

// REST API endpoints
app.get('/health', (req, res) => {
  res.json({
    status: 'ok',
    uptime: process.uptime(),
    websocketClients: clients.size,
    mcpServers: MCP_SERVERS,
    lastDataUpdate: mcpDataCache.lastUpdate
  });
});

app.get('/api/mcp/data', (req, res) => {
  res.json(mcpDataCache);
});

app.post('/api/mcp/tool', async (req, res) => {
  const { tool, params } = req.body;

  try {
    const result = await callMCPTool(tool);
    res.json({ success: true, result });
  } catch (error) {
    res.status(500).json({ success: false, error: error.message });
  }
});

app.get('/api/mcp/servers', (req, res) => {
  res.json({
    servers: MCP_SERVERS,
    claudeFlow: {
      host: CLAUDE_FLOW_HOST,
      port: CLAUDE_FLOW_PORT
    }
  });
});

// Start polling MCP data
const POLL_INTERVAL = parseInt(process.env.POLL_INTERVAL) || 5000; // 5 seconds default
setInterval(pollMCPData, POLL_INTERVAL);

// Initial poll
pollMCPData();

// Start servers
app.listen(PORT, () => {
  logger.info(`MCP Orchestrator REST API listening on port ${PORT}`);
});

logger.info(`MCP Orchestrator WebSocket server listening on port ${WS_PORT}`);

// Graceful shutdown
process.on('SIGTERM', () => {
  logger.info('SIGTERM received, shutting down gracefully');

  // Clear polling intervals
  pollingIntervals.forEach(interval => clearInterval(interval));

  // Close WebSocket connections
  clients.forEach(client => client.close());
  wss.close();

  // Close Express server
  process.exit(0);
});