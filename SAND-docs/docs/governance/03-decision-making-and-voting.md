# 3. Decision-Making & Voting

Effective decision-making is the core function of the DAO. This document outlines the processes for making decisions and the specific voting mechanisms used to ensure that the outcomes are fair and representative of the community's will.

## The Proposal Process (NIPA/PIP)

All significant changes to the protocol, treasury, or governance model must go through a formal proposal process, known as a **Nostr Improvement Proposal for Agents (NIPA)** or **Protocol Improvement Proposal (PIP)**.

### Proposal Lifecycle

1.  **Phase 1: Ideation (Community Discussion)**
    *   **Goal**: To gauge community interest and gather initial feedback.
    *   **Process**: The author posts the initial idea on the community forum or other designated discussion channels. This is an informal stage to refine the concept.

2.  **Phase 2: Drafting (Formal Proposal)**
    *   **Goal**: To create a detailed, structured proposal.
    *   **Process**: The author, often with a group of collaborators, writes a formal proposal document using the official NIPA/PIP template. This template requires sections for the summary, motivation, technical specification, security considerations, and potential drawbacks.

3.  **Phase 3: Committee Review**
    *   **Goal**: To ensure the proposal is well-formed and ready for a public vote.
    *   **Process**: The proposal is submitted to the relevant committee (Technical, Economic, or Security). The committee reviews the proposal for feasibility, risks, and alignment with the project's goals. They may provide feedback and request revisions.

4.  **Phase 4: Community Vote**
    *   **Goal**: To reach a formal decision on the proposal.
    *   **Process**: Once approved by the committee, the proposal is put to a binding on-chain vote of all staked SAND token holders. The vote typically remains open for a fixed period (e.g., 7 days).

5.  **Phase 5: Implementation**
    *   **Goal**: To enact the approved change.
    *   **Process**: If the vote passes, the core development team or relevant parties are tasked with implementing the proposal.

## Voting Mechanisms

The DAO employs a variety of voting mechanisms, choosing the appropriate one based on the nature of the decision.

### 1. Token-Weighted Voting (1 Token, 1 Vote)
*   **How it works**: A voter's influence is directly proportional to the number of SAND tokens they have staked.
*   **Pros**: Simple, Sybil-resistant, and gives more weight to those with a larger economic stake in the network.
*   **Cons**: Can lead to plutocracy, where a few large token holders can dominate decisions.
*   **Use Case**: The default mechanism for most NIPA/PIP votes.

### 2. Quadratic Voting (QV)
*   **How it works**: The cost to cast additional votes for the same proposal increases quadratically (1 vote = 1 credit, 2 votes = 4 credits, etc.). Voters are allocated a budget of voice credits.
*   **Pros**: Allows participants to express the *intensity* of their preferences. Mitigates the power of large token holders and favors consensus.
*   **Cons**: More complex to implement and explain.
*   **Use Case**: Used for funding public goods through the treasury, where the goal is to find the proposals with the broadest support.

### 3. Delegation
*   **How it works**: Token holders can delegate their voting power to another user or a recognized "delegate" who votes on their behalf.
*   **Pros**: Allows passive token holders to participate in governance. Enables the rise of expert representatives who can make informed decisions.
*   **Cons**: Can centralize power in the hands of a few popular delegates.
*   **Use Case**: Available for all token-weighted votes.

### 4. Futarchy (Prediction Markets)
*   **How it works**: A more advanced form of governance where decisions are made by betting on the outcome. The policy that the market predicts will have the most positive impact on a key metric (e.g., the future price of the SAND token) is adopted.
*   **Pros**: Can lead to more objective, data-driven decisions.
*   **Cons**: Highly complex, requires a liquid prediction market, and can be gamed.
*   **Use Case**: Experimental, may be used for specific, high-stakes economic decisions in the future.

The choice of voting mechanism is determined by the relevant committee when a proposal is put to a vote, ensuring that the method matches the decision at hand.

---
**Previous:** [2. Economic Model](./02-economic-model.md)
**Next:** [4. Conflict Resolution](./04-conflict-resolution.md)