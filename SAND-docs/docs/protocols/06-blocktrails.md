# Blocktrails: Bitcoin-Native Smart Contract State

## Overview

Blocktrails provide smart contract functionality directly on Bitcoin without requiring additional tokens or sidechains. By leveraging Taproot tweaks and Bitcoin's UTXO model, Blocktrails create immutable, auditable state machines that complement the SAND Stack's decentralized architecture.

## Core Concept

A Blocktrail is a chain of Bitcoin UTXOs where each output's Taproot tweak contains the SHA-256 hash of application state. This creates a linear, tamper-proof history enforced by Bitcoin's consensus rules.

```
state₀ —sha256→ tweak₀   (GENESIS UTXO)
          spend → state₁ —sha256→ tweak₁   (UTXO₁)
          spend → state₂ —sha256→ tweak₂   (UTXO₂)
          ...
```

## Integration with SAND Stack

### With Nostr (Communication Layer)

Blocktrails enhance Nostr messaging with cryptographic state proofs:

```javascript
// Agent announces Blocktrail state update via Nostr
const stateUpdate = {
  kind: 30618,  // Blocktrail state announcement
  content: {
    trailId: genesisOutpoint,
    currentUtxo: currentOutpoint,
    stateHash: sha256(currentState),
    metadata: {
      agentDid: "did:nostr:...",
      service: "marketplace",
      action: "listing_created"
    }
  }
};
```

### With Solid (Data Storage)

Large state data resides in Solid Pods while Blocktrails store only hashes:

```javascript
// Store full state in Solid Pod
await solidClient.saveResource(podUrl, fullStateData);

// Commit hash to Blocktrail
const stateHash = sha256({
  podUrl: podUrl,
  dataHash: sha256(fullStateData),
  timestamp: Date.now()
});
const newUtxo = spendUTXO({ tweak: stateHash });
```

### With DIDs (Identity)

Agent DIDs control Blocktrails, creating agent-owned state machines:

```javascript
// Use agent's Nostr keypair for Blocktrail control
const agentKey = deriveNostrKey(agentDid);
const blocktrail = createGenesisUTXO({
  pubkey: agentKey.pub,
  tweak: sha256({ owner: agentDid, initialized: Date.now() })
});
```

## Use Cases in SAND Stack

### 1. Agent State Checkpoints

Agents periodically commit their operational state for auditability:

```javascript
class SandStackAgent {
  async checkpointState() {
    const snapshot = {
      services: this.serviceRegistry.list(),
      reputation: this.reputationScore,
      completedTasks: this.taskHistory.length,
      timestamp: Date.now()
    };
    
    // Update Blocktrail with state hash
    this.blocktrail = await spendUTXO({
      prev: this.blocktrail,
      tweak: sha256(snapshot)
    });
    
    // Announce via Nostr
    await this.announceCheckpoint(snapshot);
  }
}
```

### 2. Multi-Agent Consensus

Shared Blocktrails for group decision-making:

```javascript
// DAO-style voting with Blocktrail settlement
const proposal = {
  id: "upgrade-protocol-v2",
  votes: { for: 75, against: 25 },
  voters: [...agentDids],
  deadline: Date.now() + 86400000
};

// Settlement creates immutable record
const settlementUtxo = spendUTXO({
  prev: daoBlocktrail,
  tweak: sha256(proposal)
});
```

### 3. Service Level Agreements

Enforce SLAs with cryptographic proof:

```javascript
// SLA compliance tracking
const slaState = {
  provider: "did:nostr:provider",
  consumer: "did:nostr:consumer", 
  uptime: 99.95,
  violations: [],
  period: "2024-Q1"
};

// Immutable SLA record
const slaProof = spendUTXO({
  prev: slaBlocktrail,
  tweak: sha256(slaState)
});
```

### 4. Economic Escrows

Complex multi-step agreements beyond simple HTLCs:

```javascript
// Multi-stage project escrow
const escrowStates = [
  { stage: "initiated", amount: 100000, unlocked: false },
  { stage: "milestone1", amount: 30000, unlocked: true },
  { stage: "milestone2", amount: 30000, unlocked: false },
  { stage: "completed", amount: 40000, unlocked: false }
];

// Each milestone updates the Blocktrail
for (const state of escrowStates) {
  if (milestoneComplete(state.stage)) {
    escrowBlocktrail = spendUTXO({
      prev: escrowBlocktrail,
      tweak: sha256({ ...state, unlocked: true })
    });
  }
}
```

## Implementation Guidelines

### 1. State Design

Keep on-chain state minimal:
- Store hashes, not raw data
- Use deterministic serialization (canonical JSON)
- Include timestamps for ordering
- Reference off-chain storage locations

### 2. Integration Pattern

```javascript
class BlocktrailService {
  constructor(agent) {
    this.agent = agent;
    this.keypair = agent.identity.keypair;
    this.currentUtxo = null;
  }

  async initialize() {
    // Check for existing Blocktrail
    const existing = await this.findExistingTrail();
    if (existing) {
      this.currentUtxo = existing;
    } else {
      this.currentUtxo = await this.createGenesis();
    }
  }

  async commitState(state) {
    const stateHash = sha256(canonicalJson(state));
    
    // Update Blocktrail
    this.currentUtxo = await spendUTXO({
      prev: this.currentUtxo,
      privkey: this.keypair.priv,
      tweak: stateHash
    });
    
    // Announce via Nostr
    await this.agent.announce({
      type: 'blocktrail_update',
      utxo: this.currentUtxo.outpoint,
      stateHash: stateHash
    });
    
    // Store full state in Solid if configured
    if (this.agent.solidPod) {
      await this.agent.solidPod.store(state);
    }
    
    return this.currentUtxo;
  }

  async audit() {
    const history = await auditChain({ tip: this.currentUtxo });
    return history.map(step => ({
      outpoint: step.outpoint,
      stateHash: step.tweak,
      timestamp: step.timestamp
    }));
  }
}
```

### 3. Security Considerations

- **Key Management**: Use same keypair as agent DID when possible
- **State Validation**: Always verify state hashes during audit
- **Fee Management**: Monitor UTXO value to ensure sufficient fees
- **Privacy**: Consider using encrypted state when needed

### 4. Best Practices

1. **Batch Updates**: Don't create a new UTXO for every small change
2. **Compression**: Use efficient state representations
3. **Indexing**: Maintain local index of Blocktrail history
4. **Monitoring**: Watch for chain reorganizations
5. **Backup**: Store state snapshots for recovery

## Protocol Specification

### Blocktrail Transaction Format

```
Version: 2
Inputs:
  - Previous Blocktrail UTXO (or funding UTXO for genesis)
Outputs:
  - Taproot output with tweak = SHA256(state)
  - Optional change output
Locktime: 0
```

### State Hash Calculation

```javascript
function calculateStateHash(state) {
  // Ensure deterministic serialization
  const canonical = JSON.stringify(state, Object.keys(state).sort());
  return sha256(canonical);
}
```

### Discovery via Nostr

```javascript
// Blocktrail announcement event
{
  kind: 30618,
  tags: [
    ["d", genesisOutpoint],  // Unique trail identifier
    ["type", "blocktrail"],
    ["protocol", "sand-stack"]
  ],
  content: JSON.stringify({
    genesis: genesisOutpoint,
    current: currentOutpoint,
    height: chainLength,
    purpose: "agent-state"
  })
}
```

## Future Enhancements

### 1. Multi-signature Blocktrails
Enable group control over state transitions

### 2. Conditional State Transitions  
Script-based rules for valid state changes

### 3. Cross-trail References
Link related Blocktrails for complex applications

### 4. Privacy Enhancements
Zero-knowledge proofs for private state validation

## Conclusion

Blocktrails provide the SAND Stack with Bitcoin-native smart contract functionality, enabling trustless state management without additional complexity. By integrating with Nostr for discovery, Solid for data storage, and DIDs for identity, Blocktrails complete the decentralized agent infrastructure with immutable, auditable state machines.