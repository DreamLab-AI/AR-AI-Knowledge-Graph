Excellent. This error message is a classic and highly informative clue. Let's break it down.

### The Diagnosis

The error `SyntaxError: Unexpected token ' ', " "` is unambiguous. It means `JSON.parse()` was called with a string that is not valid JSON—specifically, a string that is just a single space character (`" "`).

Your custom logger pinpoints the exact location: this is happening inside the `MCPWebSocketService`.

**Conclusion:** The WebSocket connection for the **VisionFlow** graph is receiving messages that are just empty space. This is almost certainly a **keep-alive or ping message** from the server that isn't being handled correctly on the client.

### Likely Causes

1.  **Server Keep-Alive Mechanism (Most Likely Cause):** Many WebSocket servers send a small, non-JSON message (like a space or a special character) periodically to keep the connection alive and prevent proxies or firewalls from closing it due to inactivity. Your `onmessage` handler in `MCPWebSocketService` expects *every* text message to be valid JSON, so it fails when it receives this keep-alive packet.

2.  **Dual-Graph WebSocket Interference:** You have two primary WebSocket services now: `WebSocketService` (for Logseq) and `MCPWebSocketService` (for VisionFlow).
    *   Looking at your `vite.config.ts`, you have a proxy for `/wss`.
    *   Both `WebSocketService.ts` and `MCPWebSocketService.ts` attempt to connect to `/wss` by default.
    *   It's highly likely that both services are connecting to the **same backend WebSocket endpoint**. This means the `MCPWebSocketService` is receiving messages intended for the Logseq graph, and vice-versa. The " " message could be a keep-alive from the Logseq backend that the MCP service doesn't know how to handle.

3.  **Server-Side Bug:** There could be a condition on the server where it sends an empty or whitespace-only payload instead of a properly structured message (e.g., when there are no agent updates to send).

### How to Fix It

The solution involves making the client more resilient and ensuring your WebSocket connections are properly separated.

#### Step 1: Make the `MCPWebSocketService` More Resilient (Immediate Fix)

You should guard the `JSON.parse` call to handle non-JSON messages gracefully. This is good practice for any WebSocket client.

**File**: `client/src/features/swarm/services/MCPWebSocketService.ts`

Modify the `handleMessage` method:

```typescript
// Around line 99 in MCPWebSocketService.ts

private handleMessage(message: MCPMessage) {
    // This is the original line that's causing the error
    // const message: MCPMessage = JSON.parse(data);

    // The fix is in the onmessage handler that calls this.
    // Let's modify the onmessage handler instead.

    // ... (rest of the switch statement)
}

// Modify the onmessage handler around line 81
this.ws.onmessage = async (event) => {
  try {
    let data: string;

    if (event.data instanceof Blob) {
      data = await event.data.text();
    } else {
      data = event.data;
    }

    // --- START OF THE FIX ---
    const trimmedData = data.trim();
    if (trimmedData === '') {
      logger.debug('Received an empty or whitespace-only message, likely a keep-alive. Ignoring.');
      return; // Ignore empty messages
    }
    // --- END OF THE FIX ---

    // Now, parse the sanitized data
    const message: MCPMessage = JSON.parse(trimmedData);
    this.handleMessage(message);

  } catch (error) {
    logger.error('Failed to parse MCP message:', error);
  }
};
```
**Reasoning:** This simple check prevents the `JSON.parse` from ever seeing the empty space. It correctly identifies it as a probable keep-alive message and ignores it, preventing the crash.

#### Step 2: Separate the WebSocket Endpoints (Architectural Fix)

The root of the problem is likely that both services are listening on the same channel. You need to differentiate them.

1.  **Server-Side:** Ensure your backend exposes two distinct WebSocket endpoints, for example:
    *   `/wss/logseq`
    *   `/wss/visionflow` (or `/wss/mcp`)

2.  **Client-Side:** Update your service files to connect to the correct endpoints.

    **File**: `client/src/services/WebSocketService.ts` (for Logseq)
    ```typescript
    private determineWebSocketUrl(): string {
      const url = '/wss/logseq'; // Explicitly for Logseq
      logger.info(`Determined Logseq WebSocket URL (relative): ${url}`);
      return url;
    }
    ```

    **File**: `client/src/features/swarm/services/MCPWebSocketService.ts` (for VisionFlow)
    ```typescript
    // In the constructor or connect method
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    this.wsUrl = `${protocol}//${host}/wss/visionflow`; // Explicitly for VisionFlow
    ```

    You will also need to update your Nginx proxy configuration (if used in development) to handle these new paths correctly.

### Summary of Progress and Next Steps

This is excellent progress. Finding and fixing an integration bug like this is a critical part of the process.

*   **You've successfully implemented the multi-graph structure.** The bug itself is proof that both systems are running in parallel.
*   **Your logging is effective.** It immediately told you *which* service was failing and *why*.

Here is your updated, very short to-do list:

1.  **[BUGFIX] Implement the resilient parsing** in `MCPWebSocketService.ts` to immediately stop the crashing.
2.  **[REFACTOR] Separate the WebSocket endpoints** on both the server and client to ensure clean data channels for each graph. This is the correct long-term solution.
3.  **[CLEANUP] Finalize Component Consolidation**. Now that the core systems are working, you can safely delete the old/unused graph and layout components (`GraphManager.tsx`, the `TwoPaneLayout` variants, etc.) to complete Phase 2.

You're very close to a clean, stable, and feature-complete milestone. Great work