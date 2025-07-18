import { getPublicKey, nip19 } from 'nostr-tools';
import fs from 'fs/promises';
import crypto from 'crypto';

export class IdentityManager {
  constructor(privateKey) {
    this.privateKey = privateKey;
    this.publicKey = getPublicKey(privateKey);
    this.npub = nip19.npubEncode(this.publicKey);
    this.did = `did:nostr:${this.publicKey}`;
  }

  async initialize() {
    // Load or create identity document
    this.identityDocument = await this.loadOrCreateIdentityDocument();
  }

  async loadOrCreateIdentityDocument() {
    const identityPath = './agent.json';

    try {
      const data = await fs.readFile(identityPath, 'utf8');
      return JSON.parse(data);
    } catch (error) {
      // Create new identity document
      const document = this.createIdentityDocument();
      await fs.writeFile(identityPath, JSON.stringify(document, null, 2));
      return document;
    }
  }

  createIdentityDocument() {
    return {
      "@context": [
        "https://www.w3.org/ns/did/v1",
        "https://w3id.org/nostr/context"
      ],
      "id": this.did,
      "verificationMethod": [{
        "id": `${this.did}#key1`,
        "controller": this.did,
        "type": "SchnorrVerification2025",
        "publicKeyHex": this.publicKey
      }],
      "authentication": ["#key1"],
      "assertionMethod": ["#key1"],
      "service": [{
        "id": `${this.did}#nostr`,
        "type": "NostrRelay",
        "serviceEndpoint": [
          "wss://relay.damus.io",
          "wss://relay.nostr.band"
        ]
      }]
    };
  }

  // Generate NIP-98 authentication header
  async generateNIP98Auth(url, method = 'GET') {
    const { finalizeEvent } = await import('nostr-tools');

    const event = {
      kind: 27235,
      created_at: Math.floor(Date.now() / 1000),
      tags: [
        ['u', url],
        ['method', method]
      ],
      content: ''
    };

    const signedEvent = finalizeEvent(event, this.privateKey);
    const base64Event = Buffer.from(JSON.stringify(signedEvent)).toString('base64');

    return `Nostr ${base64Event}`;
  }

  // Sign arbitrary data
  async sign(data) {
    const { schnorr } = await import('@noble/secp256k1');
    const message = typeof data === 'string' ? data : JSON.stringify(data);
    const messageHash = crypto.createHash('sha256').update(message).digest();

    return schnorr.sign(messageHash, this.privateKey);
  }

  // Verify signature
  async verify(signature, data, publicKey) {
    const { schnorr } = await import('@noble/secp256k1');
    const message = typeof data === 'string' ? data : JSON.stringify(data);
    const messageHash = crypto.createHash('sha256').update(message).digest();

    return schnorr.verify(signature, messageHash, publicKey);
  }
}