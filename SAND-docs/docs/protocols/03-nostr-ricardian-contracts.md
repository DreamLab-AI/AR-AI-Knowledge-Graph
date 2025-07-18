# 3. Nostr Ricardian Contracts (NRC) Specification

**Status**: DRAFT

## 1. Abstract

A Ricardian Contract is a digital document that is both a legally binding contract and a machine-readable protocol. This specification defines the **Nostr Ricardian Contract (NRC)**, a standard for creating such contracts as Nostr events. An NRC allows agents to form clear, verifiable, and non-repudiable agreements for commerce and collaboration.

## 2. Core Principles

An NRC must be:
1.  **Human-Readable**: The terms of the contract must be clearly readable by a human.
2.  **Machine-Readable**: The key terms of the contract must be parsable by a machine.
3.  **Identifiable**: The contract must have a unique, content-addressable identifier.
4.  **Signed**: The contract must be cryptographically signed by all parties.

## 3. The NRC Event (`Kind 30100`)

An NRC is defined as a replaceable Nostr event with `Kind 30100`. The event's `content` field contains the human-readable text of the contract. The machine-readable components are defined in the event's `tags`.

### 3.1. Event Structure

```json
{
  "kind": 30100,
  "pubkey": "pubkey_of_proposer",
  "created_at": 1678886400,
  "tags": [
    ["d", "unique_contract_id"],
    ["p", "pubkey_of_party_1"],
    ["p", "pubkey_of_party_2"],
    ["t", "payment_hash_for_htlc"],
    ["expiration", "1678972800"],
    ["param", "service", "image_analysis"],
    ["param", "price", "1000_sats"]
  ],
  "content": "This contract is an agreement between Party 1 and Party 2 for the service of image analysis..."
}
```

### 3.2. Standard Tags

-   **`d` tag (required)**: A unique identifier for the contract instance. This allows the contract to be replaceable. It is recommended to use a random 32-byte string.
-   **`p` tag (required, one or more)**: The public key of each party to the contract. The event MUST be signed by all parties to be considered valid.
-   **`t` tag (optional)**: A tag related to the transaction, often a payment hash for an associated HTLC.
-   **`expiration` tag (optional)**: A Unix timestamp indicating when the contract offer expires.
-   **`param` tag (optional, one or more)**: A key-value pair defining a specific parameter of the contract. This is the primary mechanism for making the contract machine-readable.

## 4. The Contract Lifecycle

An NRC typically follows a state transition model, managed by a separate event kind.

### 4.1. State Transition Event (`Kind 30101`)

To change the state of a contract, a party publishes a `Kind 30101` event.

-   **`e` tag**: This tag MUST reference the `id` of the `Kind 30100` NRC event.
-   **`a` tag**: This tag MAY reference the `d` tag of the `Kind 30100` NRC event.
-   **`status` tag**: The new status of the contract (e.g., "accepted", "rejected", "fulfilled", "disputed").

```json
{
  "kind": 30101,
  "pubkey": "pubkey_of_party_2",
  "tags": [
    ["e", "id_of_nrc_event"],
    ["a", "unique_contract_id"],
    ["status", "accepted"]
  ],
  "content": "I accept the terms of the contract."
}
```

### 4.2. Typical Lifecycle

1.  **Proposal**: Party 1 proposes the contract by publishing the `Kind 30100` event, signed with their key.
2.  **Acceptance**: Party 2 accepts the contract by publishing a `Kind 30101` event with `status: "accepted"`, signed with their key. A fully valid contract requires both signatures on the original `Kind 30100` event, which can be achieved by Party 2 re-publishing the event with their signature added.
3.  **Fulfillment**: Once the service is rendered and payment is made, a party publishes a `Kind 30101` event with `status: "fulfilled"`.
4.  **Dispute**: If there is a disagreement, a party can publish a `Kind 30101` event with `status: "disputed"`, which can trigger a formal conflict resolution process.

## 5. Dispute Resolution (`Kind 30103`)

If a dispute arises, a `Kind 30103` event can be published, referencing the NRC and providing evidence for the dispute. This event serves as a formal record for arbitrators (such as the Community Council) to review.

NRCs provide a powerful primitive for building a robust and trustworthy machine-to-machine economy on Nostr, enabling complex agreements to be formed and executed in a decentralized way.

---
**Previous:** [2. Model Context Protocol (MCP)](./02-model-context-protocol.md)
**Next:** [4. Protocol Integration Guide](./04-protocol-integration-guide.md)