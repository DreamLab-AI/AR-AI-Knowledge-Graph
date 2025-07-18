# 3. Building a Complete Agent

This tutorial will guide you through the process of building a complete, standalone agent using Node.js. This agent will have its own identity, connect to Nostr relays, and respond to a specific command sent via an encrypted direct message.

We will be building a simple "Echo Agent". It will listen for incoming DMs, and when it receives a message that says "echo", it will reply with "echo back".

## Prerequisites

-   You have completed the previous tutorials.
-   You have Node.js and `npm` installed.
-   You have a text editor (e.g., VS Code).

## Step 1: Project Setup

1.  Create a new directory for your project and navigate into it:
    ```bash
    mkdir echo-agent
    cd echo-agent
    ```

2.  Initialize a new Node.js project:
    ```bash
    npm init -y
    ```

3.  Install the necessary libraries. We'll use `nostr-tools`, a popular library for interacting with the Nostr network.
    ```bash
    npm install nostr-tools
    ```

4.  Create an `index.js` file. This will be the main file for our agent's code.

## Step 2: Agent Identity

1.  Run the identity creation tool to generate a keypair for your new agent.
    ```bash
    npm init agent@latest -y
    ```
    This will print the private key to the console.

2.  Create a file named `.env` in your project directory. This file will store your agent's secret key. **Never commit this file to version control.**

3.  Add the private key to your `.env` file:
    ```
    NOSTR_PRIVATE_KEY=sk1...
    ```

4.  Add the `.env` file to a `.gitignore` file to prevent it from being accidentally published.
    ```
    echo ".env" > .gitignore
    ```

## Step 3: Writing the Agent Code

Open `index.js` and add the following code. Comments explain each part of the process.

```javascript
// Import necessary functions from nostr-tools
import {
  Relay,
  nip19,
  nip04,
  getPublicKey,
  getEventHash,
  signEvent,
} from 'nostr-tools';

// Load the private key from environment variables
const privateKey = process.env.NOSTR_PRIVATE_KEY;
if (!privateKey) {
  throw new Error("NOSTR_PRIVATE_KEY is not set in the .env file.");
}

// Derive the public key from the private key
const publicKey = getPublicKey(privateKey);
console.log(`Agent Public Key: ${nip19.npubEncode(publicKey)}`);

async function main() {
  // Define the relays we want to connect to
  const relays = [
    'wss://relay.damus.io',
    'wss://relay.primal.net',
    'wss://relay.snort.social',
  ];

  // Connect to the relays
  const relay = await Relay.connect(relays[0]); // Connect to the first one for now
  console.log(`Connected to ${relay.url}`);

  // Subscribe to encrypted direct messages (Kind 4) sent to our public key
  const sub = relay.subscribe(
    [
      {
        kinds: [4],
        '#p': [publicKey],
      },
    ],
    {
      onevent(event) {
        // This function is called every time a new event is received
        console.log('Received event:', event.id);
        handleEvent(event);
      },
    }
  );

  console.log('Listening for encrypted DMs...');

  async function handleEvent(event) {
    try {
      // The sender's public key is in the 'p' tag
      const senderPublicKey = event.pubkey;

      // Decrypt the message content
      const decryptedMessage = await nip04.decrypt(
        privateKey,
        senderPublicKey,
        event.content
      );

      console.log(`Decrypted message from ${nip19.npubEncode(senderPublicKey)}:`, decryptedMessage);

      // Check if the message is the command we're listening for
      if (decryptedMessage.trim().toLowerCase() === 'echo') {
        console.log('Received "echo" command. Sending reply...');
        await sendReply(senderPublicKey);
      }
    } catch (e) {
      console.error('Error handling event:', e);
    }
  }

  async function sendReply(recipientPublicKey) {
    // Create the reply message
    const replyContent = 'echo back';

    // Encrypt the reply
    const encryptedReply = await nip04.encrypt(
      privateKey,
      recipientPublicKey,
      replyContent
    );

    // Create the event object
    const replyEvent = {
      kind: 4,
      pubkey: publicKey,
      created_at: Math.floor(Date.now() / 1000),
      tags: [['p', recipientPublicKey]],
      content: encryptedReply,
    };

    // Sign the event
    replyEvent.id = getEventHash(replyEvent);
    replyEvent.sig = signEvent(replyEvent, privateKey);

    // Publish the event to the relay
    await relay.publish(replyEvent);
    console.log(`Sent reply to ${nip19.npubEncode(recipientPublicKey)}`);
  }
}

main().catch(console.error);
```

## Step 4: Running and Testing the Agent

1.  **Run the agent** from your terminal:
    ```bash
    node index.js
    ```
    You should see a message that it's connected and listening.

2.  **Test the agent**:
    *   Go to a web client like [https://primal.net/](https://primal.net/).
    *   Log in with a *different* Nostr private key (not your agent's key).
    *   Find your agent's profile by searching for its public key (`npub...` printed when you started the script).
    *   Send your agent a direct message with the exact text: `echo`.
    *   Check the terminal where your agent is running. You should see it log the decrypted message.
    *   Check the web client. You should receive a direct message back from your agent that says "echo back".

## Congratulations!

You have now built a fully autonomous, standalone agent that can:
-   Manage its own identity.
-   Connect to the decentralized Nostr network.
-   Listen for and decrypt incoming commands.
-   Process commands and send encrypted replies.

This simple echo bot forms the foundation for much more complex agents. You can now build on this by adding more commands, connecting to APIs, or interacting with other protocols like Solid or Lightning.

---
**Previous:** [2. Agent-to-Agent Communication](./../02-agent-communication/README.md)
**Next:** [4. Deploying Your Agent](./../04-agent-deployment/README.md)