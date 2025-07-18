import dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

// Load environment variables
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
dotenv.config({ path: join(__dirname, '..', '.env') });

export const config = {
  // Agent identity
  privateKey: process.env.AGENT_PRIVATE_KEY,

  // Agent metadata
  agentName: process.env.AGENT_NAME || 'SAND Stack Agent',
  agentDescription: process.env.AGENT_DESCRIPTION || 'A basic autonomous agent built on the SAND Stack',
  version: process.env.AGENT_VERSION || '1.0.0',

  // Nostr configuration
  relays: (process.env.NOSTR_RELAYS || 'wss://relay.damus.io,wss://relay.nostr.band,wss://nostr.fmt.wiz.biz').split(','),

  // API configuration
  apiPort: parseInt(process.env.API_PORT || '3000'),
  apiHost: process.env.API_HOST || '0.0.0.0',

  // Solid Pod configuration
  solidPodUrl: process.env.SOLID_POD_URL,
  solidApiKey: process.env.SOLID_API_KEY,

  // Lightning configuration
  lightningNodeUrl: process.env.LIGHTNING_NODE_URL,
  lightningMacaroon: process.env.LIGHTNING_MACAROON,

  // Logging
  logLevel: process.env.LOG_LEVEL || 'info',

  // Environment
  nodeEnv: process.env.NODE_ENV || 'development'
};

// Validate required configuration
if (!config.privateKey) {
  console.error('ERROR: AGENT_PRIVATE_KEY is required in environment variables');
  process.exit(1);
}