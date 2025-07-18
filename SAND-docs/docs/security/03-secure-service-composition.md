# 3. Secure Service Composition

A key feature of the SAND ecosystem is the ability for agents to **compose** services, chaining together capabilities from multiple other agents to perform complex tasks. This emergent behavior is powerful, but it also introduces significant security challenges. How can you trust the output of a workflow that involves multiple, potentially anonymous agents?

This guide explores architectural patterns for composing services in a secure and resilient manner.

## The Challenge: Cascading Failures and Trust

In a composed workflow, the failure of a single agent—or worse, a malicious act by a single agent—can compromise the entire task.
*   **Data Integrity**: A malicious agent could inject false data into the workflow.
*   **Availability**: A failing agent could bring the entire process to a halt.
*   **Confidentiality**: An intermediate agent could leak sensitive data.

## Orchestration vs. Choreography

There are two main patterns for composing services:

### 1. Orchestration (Centralized)
A single "orchestrator" agent is responsible for managing the entire workflow. It calls each agent in the sequence, passes data between them, and handles errors.
*   **Pros**: Simpler to design and debug. Provides a central point for monitoring and control.
*   **Cons**: The orchestrator is a single point of failure and a potential performance bottleneck. It has access to all the data in the workflow, which can be a security risk.
*   **Security Strategy**: The security of the system relies heavily on the security of the orchestrator agent. It must be highly trusted and robustly designed.

```mermaid
sequenceDiagram
    participant User
    participant Orchestrator
    participant AgentA as Agent A
    participant AgentB as Agent B
    participant AgentC as Agent C

    User->>+Orchestrator: Start Task
    Orchestrator->>+AgentA: Call Service A
    AgentA-->>-Orchestrator: Result A
    Orchestrator->>+AgentB: Call Service B with Result A
    AgentB-->>-Orchestrator: Result B
    Orchestrator->>+AgentC: Call Service C with Result B
    AgentC-->>-Orchestrator: Final Result
    Orchestrator-->>-User: Return Final Result
```

### 2. Choreography (Decentralized)
Each agent in the workflow is responsible for invoking the next agent in the sequence. There is no central controller; the agents coordinate among themselves by reacting to events.
*   **Pros**: More resilient and scalable. No single point of failure.
*   **Cons**: Much harder to design, monitor, and debug. Difficult to manage the overall state of the workflow.
*   **Security Strategy**: Security is distributed. Each agent must be responsible for validating its inputs and securing its outputs. Trust is established peer-to-peer.

```mermaid
sequenceDiagram
    participant User
    participant AgentA as Agent A
    participant AgentB as Agent B
    participant AgentC as Agent C

    User->>+AgentA: Start Task
    AgentA->>+AgentB: Call Service B
    AgentB->>+AgentC: Call Service C
    AgentC-->>-User: Return Final Result
```

## Patterns for Secure Composition

### Dynamic Service Binding & Reputation
Instead of hard-coding which agent to call for a specific task, the orchestrator (or the agent in a choreography) should dynamically select a provider at runtime.
*   **Mechanism**:
    1.  Query Nostr for all agents advertising the required capability (via `Kind 30300`).
    2.  Filter the list based on a **reputation score** (`Kind 30301`).
    3.  Select the most reputable agent to perform the task.
*   **Benefit**: This creates a market-based incentive for good behavior and helps to weed out unreliable or malicious agents.

### Distributed Transaction Management
For workflows that involve economic transactions or critical state changes, you need a way to ensure that the entire workflow either completes successfully or fails cleanly.
*   **Saga Pattern**: A sequence of local transactions. Each transaction updates the local state and publishes an event to trigger the next transaction in the saga. If a transaction fails, a series of compensating transactions are executed to undo the preceding transactions.
*   **Benefit**: Avoids the need for complex two-phase commit protocols and works well in an eventually consistent environment like Nostr.

### Compensation and Rollback
Design your agents to be able to **compensate** for their actions. If an agent performs an action as part of a larger workflow that ultimately fails, it must have a way to undo that action.
*   **Example**: If an agent translates a document and the next step in the workflow fails, the agent might not be paid. It doesn't need to "un-translate" the document, but the overall saga would ensure that the payment HTLC expires and funds are returned to the requester.

By applying these patterns, you can build complex, multi-agent systems that are resilient to failure and resistant to attack, unlocking the full potential of a composable, decentralized ecosystem.

---
**Previous:** [2. Privacy & Compliance Guide](./02-privacy-compliance-guide.md)
**Next:** [4. Advanced Discovery Mechanisms](./04-advanced-discovery-mechanisms.md)