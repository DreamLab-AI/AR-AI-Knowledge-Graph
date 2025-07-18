import express from 'express';
import { logger } from './logger.js';
import { config } from './config.js';

export async function setupAPI(agent) {
  const app = express();

  // Middleware
  app.use(express.json());
  app.use(express.urlencoded({ extended: true }));

  // Request logging
  app.use((req, res, next) => {
    logger.info(`API Request: ${req.method} ${req.path}`);
    next();
  });

  // CORS headers
  app.use((req, res, next) => {
    res.header('Access-Control-Allow-Origin', '*');
    res.header('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
    res.header('Access-Control-Allow-Headers', 'Content-Type, Authorization');

    if (req.method === 'OPTIONS') {
      return res.sendStatus(200);
    }
    next();
  });

  // Health check endpoint
  app.get('/health', (req, res) => {
    res.json({
      status: 'healthy',
      timestamp: new Date().toISOString()
    });
  });

  // Ready check endpoint
  app.get('/ready', (req, res) => {
    const ready = agent.subscriptions.size > 0;

    if (ready) {
      res.json({ ready: true });
    } else {
      res.status(503).json({ ready: false });
    }
  });

  // Status endpoint
  app.get('/status', (req, res) => {
    res.json(agent.getStatus());
  });

  // Metrics endpoint
  app.get('/metrics', async (req, res) => {
    res.set('Content-Type', agent.metrics.register.contentType);
    const metrics = await agent.metrics.register.metrics();
    res.end(metrics);
  });

  // MCP manifest endpoint
  app.get('/.well-known/mcp', (req, res) => {
    res.json(agent.serviceRegistry.getMCPManifest());
  });

  // Service execution endpoint
  app.post('/mcp/execute', async (req, res) => {
    try {
      const { capability, input } = req.body;

      if (!capability) {
        return res.status(400).json({ error: 'capability is required' });
      }

      const result = await agent.serviceRegistry.executeService(capability, input);

      res.json({
        capability,
        result,
        timestamp: new Date().toISOString()
      });
    } catch (error) {
      logger.error('Service execution error:', error);
      res.status(400).json({ error: error.message });
    }
  });

  // Service info endpoint
  app.get('/services/:serviceId', (req, res) => {
    const info = agent.serviceRegistry.getServiceInfo(req.params.serviceId);

    if (info) {
      res.json(info);
    } else {
      res.status(404).json({ error: 'Service not found' });
    }
  });

  // List all services
  app.get('/services', (req, res) => {
    res.json({
      services: agent.serviceRegistry.listCapabilities()
    });
  });

  // Send message endpoint
  app.post('/messages/send', async (req, res) => {
    try {
      const { recipient, type, payload, metadata } = req.body;

      if (!recipient || !type) {
        return res.status(400).json({ error: 'recipient and type are required' });
      }

      const result = await agent.messageHandler.sendMessage(
        recipient,
        { type, payload, metadata }
      );

      res.json(result);
    } catch (error) {
      logger.error('Message send error:', error);
      res.status(500).json({ error: error.message });
    }
  });

  // Error handling
  app.use((err, req, res, next) => {
    logger.error('API Error:', err);
    res.status(500).json({
      error: 'Internal server error',
      message: err.message
    });
  });

  // Start server
  return new Promise((resolve) => {
    const server = app.listen(config.apiPort, config.apiHost, () => {
      logger.info(`API server listening on ${config.apiHost}:${config.apiPort}`);
      resolve(server);
    });
  });
}