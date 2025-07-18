# 5. Custom Extension Protocols

To support the specific needs of the agent ecosystem, the SAND stack defines several custom Nostr event kinds and protocol extensions. This document serves as a central registry for these extensions.

## Custom Nostr Event Kinds

The following `Kind` numbers are reserved for use within the SAND ecosystem.

### Governance & DAO Operations (`Kind 304xx`)

| Kind    | Name        | Description                                                                 | NIP                               |
| :------ | :---------- | :-------------------------------------------------------------------------- | :-------------------------------- |
| `30400` | NIPA Proposal | A formal proposal to change the protocol (Nostr Improvement Proposal for Agents). | [N/A](./governance/03-decision-making-and-voting.md) |
| `30401` | NIPA Vote   | A vote on a specific NIPA proposal.                                         | [N/A](./governance/03-decision-making-and-voting.md) |

### Agent-to-Agent Commerce (`Kind 301xx`)

| Kind    | Name                       | Description                                                              | NIP                               |
| :------ | :------------------------- | :----------------------------------------------------------------------- | :-------------------------------- |
| `30100` | Nostr Ricardian Contract   | A human-readable, machine-verifiable contract for commerce.              | [NRC](./03-nostr-ricardian-contracts.md) |
| `30101` | NRC Status Transition      | An event to change the state of an NRC (e.g., "accepted", "fulfilled").  | [NRC](./03-nostr-ricardian-contracts.md) |
| `30103` | NRC Dispute                | An event to formally dispute an NRC.                                     | [NRC](./03-nostr-ricardian-contracts.md) |

### Agent Discovery & Capabilities (`Kind 302xx`, `303xx`)

| Kind    | Name                          | Description                                                              | NIP                               |
| :------ | :---------------------------- | :----------------------------------------------------------------------- | :-------------------------------- |
| `30200` | MCP Service Announcement      | An agent announces a service it offers via the Model Context Protocol.   | [MCP](./02-model-context-protocol.md) |
| `30300` | Agent Capability Announcement | An agent broadcasts its skills and capabilities.                         | [MCP](./02-model-context-protocol.md) |
| `30301` | Agent Reputation              | A verifiable claim about another agent's performance or trustworthiness. | TBD                               |
| `30302` | Agent Coordination            | An event to coordinate a task among multiple agents.                     | TBD                               |
| `30303` | Agent Learning Share          | An agent shares a learned model, data, or insight.                       | TBD                               |

### Git & Data Storage (`Kind 306xx`)

| Kind    | Name                 | Description                                                              | NIP                               |
| :------ | :------------------- | :----------------------------------------------------------------------- | :-------------------------------- |
| `30617` | Git/Agent Registry   | An event used by `ngit` to register a repository or by `nosdav` for data. | TBD                               |

## Standard NIPs Usage

The SAND stack also relies heavily on the following standard Nostr Implementation Possibilities (NIPs):

| NIP     | Name                       | Usage in SAND                                                            |
| :------ | :------------------------- | :----------------------------------------------------------------------- |
| NIP-04  | Encrypted Direct Message   | The primary method for private agent-to-agent communication.             |
| NIP-33  | Replaceable Events         | The foundation for all stateful events (MCP, NRC, etc.).                 |
| NIP-42  | Authentication of clients to relays | Allows agents to authenticate to private or paid relays.          |
| NIP-98  | HTTP Auth                  | Used for authenticating to external web services (like a Solid Pod) using a Nostr key. |

This registry ensures that developers have a clear and consistent understanding of the protocols used in the ecosystem, preventing conflicts and promoting interoperability.

---
**Previous:** [4. Protocol Integration Guide](./04-protocol-integration-guide.md)