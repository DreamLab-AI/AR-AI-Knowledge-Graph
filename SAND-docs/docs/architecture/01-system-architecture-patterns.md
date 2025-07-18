# 1. System Architecture Patterns

Building a single agent is one thing; building a resilient, scalable, and interoperable system of many agents requires a higher-level architectural perspective. This guide explores patterns for designing robust multi-agent systems on the SAND stack.

## Multi-Agent System (MAS) Topologies

How agents are organized and interact defines the system's overall topology. There is no one-size-fits-all answer, and the choice of topology depends heavily on the specific problem domain.

### 1. Hierarchical (Hub-and-Spoke)

In this model, a central "orchestrator" agent delegates tasks to a set of specialized "worker" agents. This is a common pattern for solving complex problems that can be broken down into smaller, independent sub-tasks.

*   **Pros**: Simple to design and manage, clear lines of communication.
*   **Cons**: The orchestrator can become a bottleneck or a single point of failure.
*   **SAND Implementation**: The orchestrator discovers workers via `Kind 30300` (Capability) announcements and communicates tasks via `Kind 4` (Encrypted DM) messages.

### 2. Decentralized (Peer-to-Peer)

In a decentralized model, agents are equals and interact directly with one another without a central coordinator. This topology is more resilient and scalable but can be more complex to design.

*   **Pros**: High resilience, no single point of failure, scalable.
*   **Cons**: Discovery and coordination can be challenging. Service composition requires more sophisticated logic.
*   **SAND Implementation**: Agents use a combination of `Kind 30300` (Capability) and `Kind 30200` (MCP Service) announcements to discover each other and negotiate interactions.

### 3. Hybrid Models

Many real-world systems use a hybrid approach, combining hierarchical and decentralized patterns. For example, a system might have several decentralized "squads" of agents, each with its own internal orchestrator.

## Inter-Agent Communication Strategies

Given that Nostr is the primary communication bus, the choice of communication strategy is critical.

*   **Direct Messaging (1-to-1)**: Using `Kind 4` events for private, directed communication. This is the most common pattern for task-specific interactions.
*   **Pub/Sub (1-to-Many)**: Using public `Kind 1` events or custom tagged events for broadcasting information to any interested agent. Useful for announcements or public data streams.
*   **Shared State (Many-to-Many)**: Using `Kind 3xxxx` replaceable events to maintain a shared understanding of a system's state, such as the status of a collaborative task.

## Service Mesh for Agents

As the number of agents and services grows, managing their interactions becomes complex. A **Service Mesh** is a pattern that abstracts the complexity of inter-service communication into a dedicated infrastructure layer.

In the context of SAND, a service mesh can be implemented as a set of specialized "proxy" agents or a client-side library that handles:
*   **Service Discovery**: Automatically finding available services via Nostr.
*   **Load Balancing**: Distributing requests across multiple instances of an agent.
*   **Resilience**: Implementing patterns like retries and circuit breakers.
*   **Security**: Enforcing authentication and authorization for service calls.
*   **Observability**: Collecting metrics and traces on inter-agent communication.

## API Gateway Pattern

For systems that need to interact with the outside world (e.g., traditional web applications), an **API Gateway** agent can serve as a single entry point. This agent is responsible for:
*   **Authentication**: Verifying incoming requests (e.g., using NIP-98).
*   **Request Routing**: Forwarding requests to the appropriate internal agent(s).
*   **Protocol Translation**: Translating between external protocols (e.g., HTTP/REST) and the internal Nostr-based communication.
*   **Rate Limiting**: Protecting the internal system from abuse.

This pattern provides a clean separation between the internal agent ecosystem and external consumers.

---
**Next:** [2. Agent Architecture Best Practices](./02-agent-architecture-best-practices.md)