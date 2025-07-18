import 'websocket-polyfill';
import { SimplePool, nip19, getPublicKey, finalizeEvent } from 'nostr-tools';
import { config } from './config.js';
import { IdentityManager } from './identity.js';
import { MessageHandler } from './messages.js';
import { ServiceRegistry } from './services.js';
import { logger } from './logger.js';
import { setupMetrics } from './metrics.js';
import { setupAPI } from './api.js';
import { BlocktrailsService } from './blocktrails.js';

class SandStackAgent {
  constructor() {
    this.config = config;
    this.pool = new SimplePool();
    this.subscriptions = new Map();
    this.state = {
      startTime: Date.now(),
      messagesProcessed: 0,
      servicesProvided: 0
    };
  }

  async initialize() {
    try {
      logger.info('Initializing SAND Stack Agent...');

      // Setup identity
      this.identity = new IdentityManager(this.config.privateKey);
      await this.identity.initialize();
      logger.info(`Agent DID: ${this.identity.did}`);

      // Setup message handler
      this.messageHandler = new MessageHandler(this.identity, this.pool);

      // Setup service registry
      this.serviceRegistry = new ServiceRegistry();
      this.registerDefaultServices();

      // Setup metrics
      this.metrics = setupMetrics();

      // Setup API
      this.api = await setupAPI(this);

      // Setup Blocktrails for state commitments
      this.blocktrails = new BlocktrailsService(this.config, logger);
      await this.blocktrails.initialize(this.identity.keypair);
      
      // Initialize blocktrail if not exists
      if (!this.blocktrails.currentBlocktrail) {
        await this.blocktrails.createGenesis({
          agentDid: this.identity.did,
          initialized: Date.now(),
          services: this.serviceRegistry.listCapabilities().map(c => c.id)
        });
      }

      // Connect to Nostr
      await this.connectToNostr();

      // Announce presence
      await this.announceAgent();

      logger.info('Agent initialization complete!');
    } catch (error) {
      logger.error('Failed to initialize agent:', error);
      throw error;
    }
  }

  async connectToNostr() {
    logger.info('Connecting to Nostr relays...');

    // Subscribe to direct messages
    const dmSub = this.pool.subscribeMany(
      this.config.relays,
      [{
        kinds: [4], // Encrypted DMs
        '#p': [this.identity.publicKey],
        since: Math.floor(Date.now() / 1000)
      }],
      {
        onevent: async (event) => {
          try {
            await this.handleDirectMessage(event);
            this.state.messagesProcessed++;
            this.metrics.messagesProcessed.inc({ type: 'dm', status: 'success' });
          } catch (error) {
            logger.error('Error handling DM:', error);
            this.metrics.messagesProcessed.inc({ type: 'dm', status: 'error' });
          }
        },
        oneose: () => {
          logger.info('Connected to relays for DMs');
        }
      }
    );

    this.subscriptions.set('dms', dmSub);

    // Subscribe to agent discovery
    const discoverySub = this.pool.subscribeMany(
      this.config.relays,
      [{
        kinds: [30617], // Agent announcements
        since: Math.floor(Date.now() / 1000) - 86400
      }],
      {
        onevent: (event) => {
          this.handleAgentDiscovery(event);
        }
      }
    );

    this.subscriptions.set('discovery', discoverySub);
  }

  async handleDirectMessage(event) {
    const message = await this.messageHandler.decryptAndParse(event);

    logger.info(`Received message type: ${message.type} from ${nip19.npubEncode(event.pubkey)}`);

    switch (message.type) {
      case 'PING':
        await this.handlePing(event.pubkey, message);
        break;

      case 'SERVICE_REQUEST':
        await this.handleServiceRequest(event.pubkey, message);
        break;

      case 'CAPABILITY_QUERY':
        await this.handleCapabilityQuery(event.pubkey);
        break;

      default:
        logger.warn(`Unknown message type: ${message.type}`);
    }
  }

  async handlePing(senderPubkey, message) {
    await this.messageHandler.sendMessage(senderPubkey, {
      type: 'PONG',
      payload: {
        echo: message.payload,
        timestamp: Date.now()
      },
      metadata: {
        replyTo: message.id
      }
    });
  }

  async handleServiceRequest(senderPubkey, message) {
    const { service, input } = message.payload;

    const handler = this.serviceRegistry.getService(service);
    if (!handler) {
      await this.messageHandler.sendMessage(senderPubkey, {
        type: 'SERVICE_ERROR',
        payload: { error: `Service ${service} not found` },
        metadata: { replyTo: message.id }
      });
      return;
    }

    try {
      const result = await handler(input);
      this.state.servicesProvided++;
      this.metrics.servicesProvided.inc({ service });

      await this.messageHandler.sendMessage(senderPubkey, {
        type: 'SERVICE_RESPONSE',
        payload: { result },
        metadata: { replyTo: message.id }
      });
    } catch (error) {
      logger.error(`Service ${service} error:`, error);
      await this.messageHandler.sendMessage(senderPubkey, {
        type: 'SERVICE_ERROR',
        payload: { error: error.message },
        metadata: { replyTo: message.id }
      });
    }
  }

  async handleCapabilityQuery(senderPubkey) {
    const capabilities = this.serviceRegistry.listCapabilities();

    await this.messageHandler.sendMessage(senderPubkey, {
      type: 'CAPABILITY_RESPONSE',
      payload: { capabilities }
    });
  }

  handleAgentDiscovery(event) {
    try {
      const agent = this.parseAgentAnnouncement(event);
      logger.info(`Discovered agent: ${agent.name} (${nip19.npubEncode(agent.pubkey)})`);
      this.metrics.agentsDiscovered.inc();
    } catch (error) {
      logger.error('Error parsing agent announcement:', error);
    }
  }

  parseAgentAnnouncement(event) {
    const agent = {
      pubkey: event.pubkey,
      timestamp: event.created_at
    };

    for (const tag of event.tags) {
      const [key, value] = tag;
      if (key === 'd') agent.name = value;
      if (key === 'description') agent.description = value;
      if (key === 'version') agent.version = value;
      if (key === 'capabilities') agent.capabilities = value.split(',');
    }

    return agent;
  }

  async checkpointState() {
    try {
      const checkpoint = {
        timestamp: Date.now(),
        stats: {
          messagesProcessed: this.state.messagesProcessed,
          servicesProvided: this.state.servicesProvided,
          uptime: Date.now() - this.state.startTime
        },
        services: this.serviceRegistry.listCapabilities().map(c => ({
          id: c.id,
          callCount: c.callCount || 0
        })),
        peers: Array.from(this.knownAgents?.keys() || [])
      };

      // Commit state to Blocktrail
      await this.blocktrails.commitState(checkpoint);
      
      // Announce state update via Nostr
      const metadata = this.blocktrails.getMetadata();
      const stateAnnouncement = {
        kind: 30618,
        created_at: Math.floor(Date.now() / 1000),
        tags: [
          ['d', metadata.genesis],
          ['type', 'blocktrail'],
          ['agent', this.identity.did]
        ],
        content: JSON.stringify({
          genesis: metadata.genesis,
          current: metadata.current,
          height: metadata.height,
          stateHash: metadata.stateHash,
          purpose: 'agent-checkpoint'
        })
      };

      const signedEvent = finalizeEvent(stateAnnouncement, this.identity.privateKey);
      await Promise.any(
        this.pool.publish(this.config.relays, signedEvent)
      );

      logger.info('State checkpoint committed to Blocktrail', {
        height: metadata.height,
        hash: metadata.stateHash
      });
    } catch (error) {
      logger.error('Failed to checkpoint state:', error);
    }
  }

  registerDefaultServices() {
    // Echo service
    this.serviceRegistry.registerService({
      id: 'echo',
      name: 'Echo Service',
      description: 'Echoes back the input',
      handler: async (input) => {
        return { echo: input, timestamp: Date.now() };
      }
    });

    // Time service
    this.serviceRegistry.registerService({
      id: 'time',
      name: 'Time Service',
      description: 'Returns current time',
      handler: async () => {
        return {
          unix: Date.now(),
          iso: new Date().toISOString(),
          timezone: Intl.DateTimeFormat().resolvedOptions().timeZone
        };
      }
    });

    // Hash service
    this.serviceRegistry.registerService({
      id: 'hash',
      name: 'Hash Service',
      description: 'Generates SHA256 hash of input',
      handler: async (input) => {
        const crypto = await import('crypto');
        const hash = crypto.createHash('sha256').update(String(input)).digest('hex');
        return { input, hash };
      }
    });

    // Blocktrail audit service
    this.serviceRegistry.registerService({
      id: 'blocktrail-audit',
      name: 'Blocktrail Audit Service',
      description: 'Returns the agent\'s blocktrail history',
      handler: async () => {
        const history = await this.blocktrails.auditChain();
        const metadata = this.blocktrails.getMetadata();
        return {
          genesis: metadata?.genesis,
          current: metadata?.current,
          height: metadata?.height,
          history: history.map(h => ({
            outpoint: h.outpoint,
            stateHash: h.tweak,
            timestamp: h.timestamp
          }))
        };
      }
    });

    // Blocktrail verify service
    this.serviceRegistry.registerService({
      id: 'blocktrail-verify',
      name: 'Blocktrail Verify Service',
      description: 'Verifies if a state hash exists in the blocktrail',
      inputSchema: {
        type: 'object',
        properties: {
          stateHash: { type: 'string', pattern: '^[a-f0-9]{64}$' }
        },
        required: ['stateHash']
      },
      handler: async (input) => {
        const exists = await this.blocktrails.verifyStateHash(input.stateHash);
        return {
          stateHash: input.stateHash,
          verified: exists,
          currentHash: this.blocktrails.getCurrentStateHash()
        };
      }
    });
  }

  async announceAgent() {
    const capabilities = this.serviceRegistry.listCapabilities();

    const announcement = {
      kind: 30617,
      created_at: Math.floor(Date.now() / 1000),
      tags: [
        ['d', this.config.agentName],
        ['description', this.config.agentDescription],
        ['version', this.config.version],
        ['capabilities', capabilities.map(c => c.id).join(',')]
      ],
      content: `${this.config.agentName} v${this.config.version} is online`
    };

    const signedEvent = finalizeEvent(announcement, this.identity.privateKey);

    await Promise.any(
      this.pool.publish(this.config.relays, signedEvent)
    );

    logger.info('Agent announced to network');
    this.metrics.announcements.inc();
  }

  async shutdown() {
    logger.info('Shutting down agent...');

    // Close subscriptions
    for (const [name, sub] of this.subscriptions) {
      sub.close();
      logger.info(`Closed subscription: ${name}`);
    }

    // Close relay connections
    this.pool.close(this.config.relays);

    // Stop API server
    if (this.api) {
      await new Promise((resolve) => {
        this.api.close(resolve);
      });
    }

    logger.info('Agent shutdown complete');
  }

  getStatus() {
    const blocktrailMeta = this.blocktrails.getMetadata();
    return {
      identity: {
        did: this.identity.did,
        npub: this.identity.npub
      },
      uptime: Date.now() - this.state.startTime,
      stats: {
        messagesProcessed: this.state.messagesProcessed,
        servicesProvided: this.state.servicesProvided
      },
      capabilities: this.serviceRegistry.listCapabilities(),
      blocktrail: blocktrailMeta ? {
        genesis: blocktrailMeta.genesis,
        current: blocktrailMeta.current,
        height: blocktrailMeta.height,
        lastCheckpoint: new Date(blocktrailMeta.timestamp).toISOString()
      } : null
    };
  }
}

// Main execution
async function main() {
  const agent = new SandStackAgent();

  try {
    await agent.initialize();

    // Setup periodic state checkpointing (every 5 minutes)
    const checkpointInterval = setInterval(async () => {
      await agent.checkpointState();
    }, 5 * 60 * 1000);

    // Initial checkpoint after startup
    setTimeout(async () => {
      await agent.checkpointState();
    }, 10000); // 10 seconds after startup

    // Handle graceful shutdown
    process.on('SIGINT', async () => {
      clearInterval(checkpointInterval);
      await agent.shutdown();
      process.exit(0);
    });

    process.on('SIGTERM', async () => {
      clearInterval(checkpointInterval);
      await agent.shutdown();
      process.exit(0);
    });

    // Keep process alive
    process.stdin.resume();

  } catch (error) {
    logger.error('Fatal error:', error);
    process.exit(1);
  }
}

// Run the agent
main();