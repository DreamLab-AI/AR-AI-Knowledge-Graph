import { nip04, nip19, finalizeEvent } from 'nostr-tools';
import crypto from 'crypto';
import { logger } from './logger.js';

export class MessageHandler {
  constructor(identity, pool) {
    this.identity = identity;
    this.pool = pool;
    this.messageCache = new Map();
  }

  async sendMessage(recipientPubkey, message) {
    try {
      // Create message structure
      const structuredMessage = {
        version: '1.0',
        id: message.id || crypto.randomBytes(16).toString('hex'),
        type: message.type,
        timestamp: Date.now(),
        payload: message.payload,
        metadata: message.metadata || {}
      };

      // Encrypt message
      const encrypted = await nip04.encrypt(
        this.identity.privateKey,
        recipientPubkey,
        JSON.stringify(structuredMessage)
      );

      // Create Nostr event
      const event = {
        kind: 4, // Encrypted DM
        created_at: Math.floor(Date.now() / 1000),
        tags: [
          ['p', recipientPubkey]
        ],
        content: encrypted
      };

      // Add reply reference if present
      if (structuredMessage.metadata.replyTo) {
        event.tags.push(['e', structuredMessage.metadata.replyTo, 'reply']);
      }

      // Sign and publish
      const signedEvent = finalizeEvent(event, this.identity.privateKey);

      const relays = this.pool.publish(
        ['wss://relay.damus.io', 'wss://relay.nostr.band', 'wss://nostr.fmt.wiz.biz'],
        signedEvent
      );

      // Wait for at least one relay to accept
      await Promise.any(relays);

      logger.info(`Message sent to ${nip19.npubEncode(recipientPubkey)}: ${message.type}`);

      return {
        messageId: structuredMessage.id,
        eventId: signedEvent.id
      };
    } catch (error) {
      logger.error('Failed to send message:', error);
      throw error;
    }
  }

  async decryptAndParse(event) {
    try {
      // Check cache first
      if (this.messageCache.has(event.id)) {
        return this.messageCache.get(event.id);
      }

      // Decrypt message
      const decrypted = await nip04.decrypt(
        this.identity.privateKey,
        event.pubkey,
        event.content
      );

      // Parse message
      const message = JSON.parse(decrypted);

      // Validate message structure
      if (!this.validateMessage(message)) {
        throw new Error('Invalid message format');
      }

      // Cache parsed message
      this.messageCache.set(event.id, message);

      // Limit cache size
      if (this.messageCache.size > 1000) {
        const firstKey = this.messageCache.keys().next().value;
        this.messageCache.delete(firstKey);
      }

      return message;
    } catch (error) {
      logger.error('Failed to decrypt/parse message:', error);
      throw error;
    }
  }

  validateMessage(message) {
    // Check required fields
    if (!message.version || !message.id || !message.type || !message.timestamp) {
      return false;
    }

    // Check message age (prevent replay attacks)
    const age = Date.now() - message.timestamp;
    const maxAge = message.metadata?.ttl || 3600000; // 1 hour default

    if (age > maxAge) {
      logger.warn(`Message ${message.id} expired (age: ${age}ms)`);
      return false;
    }

    return true;
  }

  // Broadcast message to multiple recipients
  async broadcastMessage(recipientPubkeys, message) {
    const results = await Promise.allSettled(
      recipientPubkeys.map(pubkey => this.sendMessage(pubkey, message))
    );

    const successful = results.filter(r => r.status === 'fulfilled').length;
    const failed = results.filter(r => r.status === 'rejected').length;

    logger.info(`Broadcast complete: ${successful} sent, ${failed} failed`);

    return {
      successful,
      failed,
      results
    };
  }
}