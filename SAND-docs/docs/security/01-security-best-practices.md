# 1. Security Best Practices

Building on a decentralized stack requires a proactive, defense-in-depth approach to security. This guide outlines fundamental best practices for securing agents at every layer.

## Foundational Principles

*   **Zero Trust**: Do not automatically trust any agent or network component, internal or external. Verify every request and assume the network is hostile.
*   **Defense in Depth**: Implement multiple layers of security controls. If one layer fails, others are in place to mitigate the threat.
*   **Principle of Least Privilege**: Grant agents, users, and processes the minimum level of access required to perform their function.

## Key Management

An agent's private key is its ultimate source of authority. The compromise of a private key is a critical security failure.

*   **Secure Storage**: **NEVER** store private keys in plaintext in source code, configuration files, or databases.
*   **Recommended Solutions**:
    *   **Hardware Security Module (HSM)**: For the highest level of security, store keys in a dedicated hardware device.
    *   **Managed KMS**: Use a cloud provider's Key Management Service (e.g., AWS KMS, Google Cloud KMS, Azure Key Vault).
    *   **Secrets Management Software**: Use a tool like HashiCorp Vault.
*   **Key Rotation**: Implement a policy for regularly rotating keys to limit the window of opportunity for an attacker who has compromised a key.
*   **Cryptographic Agility**: Design your agent to support multiple cryptographic algorithms so you can easily migrate to stronger ones in the future.

## Secure Communication

*   **End-to-End Encryption**: Use `NIP-04` (or the emerging `NIP-44`) for all private agent-to-agent communication. This ensures that relay operators and network eavesdroppers cannot read the content of messages.
*   **Relay Authentication (`NIP-42`)**: When connecting to private or paid relays, use `NIP-42` to authenticate your agent. This prevents unauthorized clients from connecting to the relay.
*   **Input Validation**: Treat all data received from the Nostr network as untrusted. Rigorously validate and sanitize all inputs to prevent injection attacks, cross-site scripting (if applicable), and other vulnerabilities.

## Access Control

*   **Solid Pod Permissions (WAC)**: Use Web Access Control to enforce granular permissions on data stored in Solid Pods. Only grant the specific `Read` or `Write` access required by a requesting agent, and only for the specific resource needed.
*   **HTTP Authentication (`NIP-98`)**: When exposing an HTTP interface (e.g., for a Solid Pod or an API Gateway), use `NIP-98` to authenticate requests using Nostr keys. This provides a strong, cryptographic method of verifying the identity of the client.

## Smart Contract & Economic Security

*   **Audit NRCs**: Before an agent signs a Nostr Ricardian Contract, it should have logic to carefully audit the human-readable and machine-readable parameters to ensure they match its intent.
*   **HTLC Security**: Ensure that the preimage (secret) for an HTLC is only revealed *after* the agreed-upon service has been delivered and verified.

## Runtime Security

*   **Sandboxing**: Run agent code in a sandboxed environment (e.g., using containers like Docker or WebAssembly runtimes) to limit its access to the underlying host system.
*   **Resource Limiting**: Configure resource limits (CPU, memory, network) for your agent's processes to prevent denial-of-service attacks or resource exhaustion bugs.
*   **Dependency Scanning**: Regularly scan your agent's dependencies for known vulnerabilities using tools like `npm audit` or Snyk.

## Threat Modeling

Before deploying any complex agent, perform a threat modeling exercise.
1.  **Identify Assets**: What are you trying to protect? (e.g., private keys, user data, funds).
2.  **Identify Threats**: Who are the potential attackers and what are their goals?
3.  **Identify Vulnerabilities**: What are the weaknesses in your system?
4.  **Implement Mitigations**: What security controls will you put in place to address the vulnerabilities?

By embedding these best practices into the development lifecycle, you can build agents that are resilient, trustworthy, and secure participants in the decentralized ecosystem.

---
**Next:** [2. Privacy & Compliance Guide](./02-privacy-compliance-guide.md)