import { getPublicKey, finalizeEvent } from 'nostr-tools';
import axios from 'axios';
import crypto from 'crypto';

/**
 * NIP-98 HTTP Authentication Implementation
 *
 * This example demonstrates how to authenticate HTTP requests using Nostr events
 * as specified in NIP-98.
 */

class NIP98Client {
  constructor(privateKey) {
    this.privateKey = privateKey;
    this.publicKey = getPublicKey(privateKey);
  }

  /**
   * Generate a NIP-98 authorization header for HTTP requests
   *
   * @param {string} url - The absolute URL being requested
   * @param {string} method - HTTP method (GET, POST, etc.)
   * @param {Buffer} [body] - Optional request body for signing
   * @returns {string} Authorization header value
   */
  async generateAuthHeader(url, method = 'GET', body = null) {
    // Create the auth event
    const event = {
      kind: 27235, // NIP-98 kind (reference to RFC 7235)
      created_at: Math.floor(Date.now() / 1000),
      tags: [
        ['u', url],
        ['method', method.toUpperCase()]
      ],
      content: ''
    };

    // Add payload hash if body is provided
    if (body) {
      const payloadHash = crypto
        .createHash('sha256')
        .update(body)
        .digest('hex');

      event.tags.push(['payload', payloadHash]);
    }

    // Sign the event
    const signedEvent = finalizeEvent(event, this.privateKey);

    // Base64 encode the event
    const encodedEvent = Buffer.from(
      JSON.stringify(signedEvent)
    ).toString('base64');

    // Return the authorization header
    return `Nostr ${encodedEvent}`;
  }

  /**
   * Make an authenticated HTTP request
   */
  async request(config) {
    const { url, method = 'GET', data, ...otherConfig } = config;

    // Generate auth header
    const authHeader = await this.generateAuthHeader(
      url,
      method,
      data ? Buffer.from(JSON.stringify(data)) : null
    );

    // Make the request
    return axios({
      url,
      method,
      data,
      ...otherConfig,
      headers: {
        ...otherConfig.headers,
        'Authorization': authHeader
      }
    });
  }
}

/**
 * NIP-98 Server Middleware for Express
 */
export function nip98Middleware(options = {}) {
  const { maxAge = 60 } = options; // Max age in seconds

  return async (req, res, next) => {
    try {
      // Extract authorization header
      const authHeader = req.headers.authorization;
      if (!authHeader || !authHeader.startsWith('Nostr ')) {
        return res.status(401).json({ error: 'Missing Nostr authorization' });
      }

      // Decode the event
      const base64Event = authHeader.substring(6); // Remove "Nostr " prefix
      const eventJson = Buffer.from(base64Event, 'base64').toString();
      const event = JSON.parse(eventJson);

      // Validate event structure
      if (event.kind !== 27235) {
        return res.status(401).json({ error: 'Invalid event kind' });
      }

      // Check timestamp (prevent replay attacks)
      const now = Math.floor(Date.now() / 1000);
      const age = now - event.created_at;

      if (age > maxAge) {
        return res.status(401).json({ error: 'Event too old' });
      }

      if (event.created_at > now + 60) {
        return res.status(401).json({ error: 'Event timestamp in future' });
      }

      // Validate tags
      const tags = Object.fromEntries(event.tags);

      // Check URL matches
      const fullUrl = `${req.protocol}://${req.get('host')}${req.originalUrl}`;
      if (tags.u !== fullUrl) {
        return res.status(401).json({ error: 'URL mismatch' });
      }

      // Check method matches
      if (tags.method !== req.method.toUpperCase()) {
        return res.status(401).json({ error: 'Method mismatch' });
      }

      // Verify payload hash if present
      if (tags.payload && req.body) {
        const bodyString = JSON.stringify(req.body);
        const payloadHash = crypto
          .createHash('sha256')
          .update(bodyString)
          .digest('hex');

        if (tags.payload !== payloadHash) {
          return res.status(401).json({ error: 'Payload mismatch' });
        }
      }

      // Verify event signature
      const { verifyEvent } = await import('nostr-tools');
      if (!verifyEvent(event)) {
        return res.status(401).json({ error: 'Invalid signature' });
      }

      // Add authenticated pubkey to request
      req.nostrPubkey = event.pubkey;
      req.nostrAuth = event;

      next();
    } catch (error) {
      console.error('NIP-98 auth error:', error);
      res.status(401).json({ error: 'Authentication failed' });
    }
  };
}

/**
 * Example: Authenticated Solid Pod Access
 */
class AuthenticatedSolidClient {
  constructor(privateKey, podUrl) {
    this.authClient = new NIP98Client(privateKey);
    this.podUrl = podUrl.replace(/\/$/, '');
  }

  async read(path) {
    const url = `${this.podUrl}${path}`;

    const response = await this.authClient.request({
      url,
      method: 'GET',
      headers: {
        'Accept': 'application/json'
      }
    });

    return response.data;
  }

  async write(path, data) {
    const url = `${this.podUrl}${path}`;

    const response = await this.authClient.request({
      url,
      method: 'PUT',
      data,
      headers: {
        'Content-Type': 'application/json'
      }
    });

    return response.status === 201 || response.status === 200;
  }

  async delete(path) {
    const url = `${this.podUrl}${path}`;

    const response = await this.authClient.request({
      url,
      method: 'DELETE'
    });

    return response.status === 200 || response.status === 204;
  }
}

/**
 * Example: Protected API Server
 */
import express from 'express';

export function createProtectedServer() {
  const app = express();

  app.use(express.json());

  // Public endpoint
  app.get('/public', (req, res) => {
    res.json({ message: 'This is public' });
  });

  // Protected endpoints with NIP-98
  app.use('/api', nip98Middleware({ maxAge: 300 })); // 5 minute max age

  app.get('/api/profile', (req, res) => {
    res.json({
      pubkey: req.nostrPubkey,
      message: 'Authenticated successfully'
    });
  });

  app.post('/api/data', (req, res) => {
    res.json({
      pubkey: req.nostrPubkey,
      received: req.body,
      timestamp: new Date().toISOString()
    });
  });

  return app;
}

/**
 * Example Usage
 */
async function example() {
  // Client side
  const privateKey = 'your-private-key-hex';
  const client = new NIP98Client(privateKey);

  // Make authenticated request
  try {
    const response = await client.request({
      url: 'https://api.example.com/api/profile',
      method: 'GET'
    });

    console.log('Profile:', response.data);
  } catch (error) {
    console.error('Request failed:', error.response?.data || error.message);
  }

  // Solid Pod access
  const solidClient = new AuthenticatedSolidClient(
    privateKey,
    'https://pod.example.com'
  );

  // Read data
  const data = await solidClient.read('/private/data.json');
  console.log('Pod data:', data);

  // Write data
  await solidClient.write('/private/notes.json', {
    notes: ['Note 1', 'Note 2'],
    updated: new Date().toISOString()
  });
}

// Export for use in other modules
export { NIP98Client, AuthenticatedSolidClient };

// Run example if called directly
if (import.meta.url === `file://${process.argv[1]}`) {
  example().catch(console.error);
}