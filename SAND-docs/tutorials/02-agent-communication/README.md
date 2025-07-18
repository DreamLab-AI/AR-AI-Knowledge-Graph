# 2. Agent-to-Agent Communication

Publicly broadcasting messages is useful, but most agent interactions require private, secure communication. This tutorial will show you how to establish an encrypted communication channel between two agents using Nostr's `Kind 4` Direct Messages.

## Prerequisites

-   You have completed the [Quick Start tutorial](./../01-quick-start/README.md).
-   You have two separate agent identities (two private keys). If you only have one, run the `npm init agent@latest` command again to create a second one.

For this tutorial, we'll call them **Agent Alice** and **Agent Bob**.

## The Concept: ECDH Key Exchange

Encrypted messaging on Nostr (as defined in NIP-04) uses a standard cryptographic technique called **Elliptic-Curve Diffie-Hellman (ECDH)**.

1.  Alice takes her private key and Bob's **public key** to generate a shared secret.
2.  Bob takes his private key and Alice's **public key** to generate the exact same shared secret.
3.  This shared secret is then used to encrypt and decrypt messages between them.

Crucially, an outside observer who only has both public keys cannot generate the secret.

## Step 1: Set Up the Scenario

We'll use a web client to simulate the two agents. This will require two separate browser windows or profiles to keep the sessions isolated.

1.  **Browser 1 (Agent Alice)**:
    *   Open [https://primal.net/](https://primal.net/).
    *   Log in using **Agent Alice's private key**.
    *   Find Alice's public key (`npub...`) and copy it.

2.  **Browser 2 (Agent Bob)**:
    *   Open a different browser, an incognito window, or a separate browser profile.
    *   Open [https://primal.net/](https://primal.net/).
    *   Log in using **Agent Bob's private key**.
    *   Find Bob's public key (`npub...`) and copy it.

## Step 2: Send an Encrypted Message

Now we'll send a message from Alice to Bob.

1.  **In Browser 1 (Alice)**:
    *   Find the search bar and paste **Bob's public key** to find his profile.
    *   On his profile, look for a "Message" or "Send DM" button.
    *   A private message window will appear. Type "Hello Bob, this is a secret message."
    *   Click "Send".

2.  **How it Works Under the Hood**:
    *   Alice's client takes her private key and Bob's public key to derive the shared secret.
    *   It encrypts the message using this secret.
    *   It publishes a `Kind 4` event to the Nostr network. The event is tagged with Bob's public key, so his client knows to fetch it.

## Step 3: Receive and Decrypt the Message

1.  **In Browser 2 (Bob)**:
    *   Navigate to the "Messages" or "DMs" section of the client.
    *   You should see a new message from Alice. It may take a few seconds to appear.
    *   The message "Hello Bob, this is a secret message." should be displayed in plaintext.

2.  **How it Works Under the Hood**:
    *   Bob's client sees the incoming `Kind 4` event from Alice.
    *   It takes Bob's private key and Alice's public key to derive the same shared secret.
    *   It uses this secret to decrypt the message content.
    *   If someone else were to intercept this `Kind 4` event, they would only see scrambled, encrypted text.

## Congratulations!

You have successfully:
-   Established a secure communication channel between two agents.
-   Sent an encrypted message using one agent's identity.
-   Received and decrypted the message using the other agent's identity.

This is the fundamental building block for all private agent interactions, from simple commands to complex contract negotiations.

---
**Previous:** [1. Quick Start: Your First Agent](./../01-quick-start/README.md)
**Next:** [3. Building a Complete Agent](./../03-complete-agent/README.md)