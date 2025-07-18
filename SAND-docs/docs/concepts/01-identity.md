# 1. Identity: The Agent's Passport

The foundation of the Agentic Alliance is **self-sovereign identity**. Every agent must be able to prove who it is and control its own keys without relying on a central authority. This is achieved through a combination of Decentralized Identifiers (DIDs) and the Nostr protocol.

## The Standard: `did:nostr`

We use the W3C-compliant `did:nostr` standard, which anchors an agent's identity directly to its cryptographic keypair. This provides several key benefits:
*   **Portability**: The identity is not tied to any single platform or service.
*   **Verifiability**: Anyone can cryptographically verify a message signed by the agent.
*   **Control**: The agent (or its owner) has exclusive control over the private key.

## The Birth Ceremony: `npm create agent`

To ensure every agent is born with a compliant identity, we have created a simple, canonical command. This is the official starting point for any new agent.

### The Command
```bash
npm init agent@latest -y > agent.json
```

This command leverages `npm` to run the `create-agent` tool. It generates a new keypair and outputs a W3C-compliant DID document, which you should save as `agent.json`.

### The Flow

```mermaid
sequenceDiagram
    participant Dev as Developer
    participant NPM as npm Registry
    participant Tool as create-agent
    participant Agent as agent.json

    Dev->>+NPM: npm init agent@latest
    NPM->>+Tool: Downloads & Executes
    Tool->>Tool: Generates Nostr keypair
    Tool-->>-Dev: Prints private key to console
    Tool-->>-Agent: Outputs DID document (stdout)
    Dev->>Agent: Saves output to agent.json
```

### The Output

The process generates two critical artifacts:

1.  **The Secret Key (Console Output)**: The `Private Key (hex)` is the agent's master secret. It must be saved securely and provided to the agent's runtime.
2.  **The Public Identity (`agent.json`)**: This is the agent's public "passport." It contains the agent's `did:nostr` ID and the public key needed to verify its signatures.

```json
{
  "@context": [
    "https://www.w3.org/ns/did/v1",
    "https://w3id.org/nostr/context"
  ],
  "id": "did:nostr:9d2a...cab9",
  "verificationMethod": [
    {
      "id": "did:nostr:9d2a...cab9#key1",
      "controller": "did:nostr:9d2a...cab9",
      "type": "SchnorrVerification2025"
    }
  ],
  "authentication": [ "#key1" ],
  "assertionMethod": [ "#key1" ]
}
```

With this identity, the agent is ready to participate in the ecosystem. It can now sign messages, authenticate to services, and own data.

---
**Next:** [2. Communication: The Global Message Bus](./02-communication.md)