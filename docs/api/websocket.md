# WebSocket API Reference

## Overview
The WebSocket implementation in LogseqXR provides real-time graph updates using an optimized binary protocol.

## Connection

Connect to: `wss://your-domain/wss` (Note: The actual path is `/wss` as handled by `src/handlers/socket_flow_handler.rs`)

### Connection Flow
1. Client connects to WebSocket endpoint (`/wss`)
2. Server sends: `{"type": "connection_established", "timestamp": <timestamp>}`
3. Client sends authentication (if required, typically handled via HTTP session before WebSocket upgrade)
4. Client sends: `{"type": "requestInitialData"}`
5. Server begins binary updates (configured by `binary_update_rate`)
6. Server sends: `{"type": "updatesStarted", "timestamp": <timestamp>}`
7. Server sends: `{"type": "loading", "message": "Calculating initial layout..."}` (if applicable)

### Authentication Requirements

WebSocket connections may require authentication depending on the server configuration:

1. **Session-based Auth**: If the user has an active Nostr session, the session cookie is sent during the WebSocket handshake
2. **Token-based Auth**: Authentication tokens can be passed via query parameters: `wss://your-domain/wss?token=<session-token>`
3. **Public Access**: Some deployments may allow unauthenticated WebSocket connections with limited features

## Authentication

Authentication for WebSocket connections in LogseqXR is primarily handled during the initial HTTP handshake that upgrades to a WebSocket connection. This means that user authentication (e.g., via Nostr) should occur before or during the establishment of the WebSocket connection, typically through standard HTTP mechanisms (like cookies or authorization headers). The server's `socket_flow_handler.rs` does not process an explicit `{"type": "auth", "token": "..."}` message over the WebSocket itself.

## Message Types

### Control Messages

#### 1. Connection Established
```json
{
  "type": "connection_established",
  "timestamp": 1679417762000
}
```

#### 2. Request Initial Data
```json
{
  "type": "requestInitialData"
}
```

#### 3. Updates Started
```json
{
  "type": "updatesStarted",
  "timestamp": 1679417763000
}
```

#### 4. Loading State
```json
{
  "type": "loading",
  "message": "Calculating initial layout..."
}
```

### Binary Messages - Position Updates

Position updates are transmitted as binary messages in both directions:

#### Wire Format (28 bytes per node)

```
┌─────────────┬────────────────┬────────────────┐
│  Node ID    │    Position    │    Velocity    │
│  (4 bytes)  │   (12 bytes)   │   (12 bytes)   │
└─────────────┴────────────────┴────────────────┘
```

- **Node ID**: u32 (4 bytes) - Unique identifier for the node
- **Position**: Vec3 (12 bytes) - X, Y, Z coordinates as f32 values
- **Velocity**: Vec3 (12 bytes) - X, Y, Z velocity components as f32 values

#### Implementation Details

- Server-side `BinaryNodeData` includes additional fields (`mass`, `flags`, `padding`) for physics simulation that are **NOT** transmitted
- The `WireNodeDataItem` in `binary_protocol.rs` defines the exact 28-byte wire format
- All multi-byte values use little-endian byte order
- Compression (zlib) is applied to messages larger than the configured threshold (default: 1KB)
- Client-side decompression is handled by `graph.worker.ts` off the main UI thread

#### Example Binary Data

For a single node with ID=1, position=(10.0, 20.0, 30.0), velocity=(0.1, 0.2, 0.3):
```
01 00 00 00  // Node ID: 1 (little-endian u32)
00 00 20 41  // X position: 10.0 (little-endian f32)
00 00 A0 41  // Y position: 20.0 (little-endian f32)
00 00 F0 41  // Z position: 30.0 (little-endian f32)
CD CC CC 3D  // X velocity: 0.1 (little-endian f32)
CD CC 4C 3E  // Y velocity: 0.2 (little-endian f32)
9A 99 99 3E  // Z velocity: 0.3 (little-endian f32)
```

#### Server → Client Updates

The server continuously sends position updates to all connected clients:

1. Updates are pre-computed by the server's continuous physics engine
2. Only nodes that changed significantly are included
3. Update frequency varies based on graph activity (5-60 updates/sec)
4. Each update can contain multiple node positions in a single binary message
5. When the physics simulation stabilizes, update frequency is reduced

#### Client → Server Updates

Clients can send position updates back to the server:

1. Position updates use the same binary format as server messages
2. Updates are processed by the server's physics system
3. Changes are validated and broadcast to all other connected clients
4. Modifications that violate physics constraints may be adjusted by the server

### Position Synchronization Protocol

The bidirectional synchronization protocol ensures consistent graph state:

1. Server maintains the authoritative graph state
2. Any client can send position updates during user interaction
3. Server processes updates and applies physics constraints
4. All clients receive the same set of position updates
5. Late-joining clients receive the complete current graph state


## Control Messages (JSON) - Revisited

The `socket_flow_handler.rs` primarily handles the following JSON messages:

**Server -> Client:**
- `{"type": "connection_established", "timestamp": <timestamp>}`
- `{"type": "updatesStarted", "timestamp": <timestamp>}`
- `{"type": "loading", "message": "Calculating initial layout..."}`
- `{"type": "pong"}` (in response to client's ping)

**Client -> Server:**
- `{"type": "ping"}`
- `{"type": "requestInitialData"}`: This message starts the binary update stream. This is the primary method to initiate position updates from the server.

## Optimization Features

### Binary Protocol Optimizations

1. **Fixed-Size Records**: 28 bytes per node enables fast parsing without delimiters
2. **Zero-Copy Serialization**: Uses Rust's `bytemuck` for direct memory mapping
3. **Batch Updates**: Multiple nodes in a single WebSocket frame
4. **Compression**: Zlib compression for messages > 1KB (configurable)
5. **Differential Updates**: Only nodes with significant position changes are sent

### Performance Characteristics

| Node Count | Uncompressed Size | Compressed Size (typical) | Bandwidth at 5Hz |
|------------|-------------------|---------------------------|------------------|
| 100        | 2.8 KB           | ~2 KB                    | 10 KB/s          |
| 1,000      | 28 KB            | ~15 KB                   | 75 KB/s          |
| 10,000     | 280 KB           | ~120 KB                  | 600 KB/s         |

### Client-Side Optimizations

- Web Worker processing keeps the main thread responsive
- TypedArray views for efficient binary data access
- Object pooling for Vector3 instances
- Throttled update application based on frame rate

## Error Handling

The `socket_flow_handler.rs` does not explicitly send these structured JSON error messages.
- Errors encountered during WebSocket communication (e.g., deserialization issues, unexpected message types) are typically logged on the server-side.
- The WebSocket connection might be closed by the server if unrecoverable errors occur.
- Clients should implement their own timeout and error detection logic for the WebSocket connection itself (e.g., detecting a closed connection).

## Rate Limiting

This section refers to the server's dynamic management of binary position update frequency and client-side handling, rather than strict message rate limiting (e.g., X messages per second).

- **Server-Side Update Rate:** The server dynamically adjusts the rate of binary position updates based on graph activity and physics simulation stability. This is controlled by settings in `settings.yaml` under `system.websocket`:
    - `min_update_rate`: Minimum updates per second when the graph is stable.
    - `max_update_rate`: Maximum updates per second during high activity.
    - `motion_threshold`: Sensitivity to node movement for determining activity.
- **Client-Side Throttling:** The client (`client/src/features/graph/managers/graphDataManager.ts`) implements a `lastBinaryUpdateTime` check to avoid processing updates too rapidly if they arrive faster than the client can render, effectively throttling the application of received binary messages.
- **Debug Logging:** `socket_flow_handler.rs` includes a `DEBUG_LOG_SAMPLE_RATE` to control how frequently detailed debug logs about message handling are produced, which is a diagnostic aid rather than a rate limit.

## Diagnostics

### Common Issues

1. **Connection Issues**
   - Mixed Content: Ensure WebSocket uses WSS with HTTPS
   - CORS: Check server configuration for cross-origin
   - Proxy/Firewall: Verify WebSocket ports are open
   - Authentication: Verify session tokens are valid

2. **Binary Protocol Issues**
   - Message Size: Must be multiple of 28 bytes
   - Byte Order: Ensure little-endian encoding
   - Node IDs: Must be valid u32 values
   - Float Values: Check for NaN or Infinity

3. **Performance Issues**
   - High CPU: Reduce update frequency or node count
   - Memory Growth: Check for message queue buildup
   - Network Latency: Enable compression for large graphs

### Debug Logging

Enable detailed logging:

**Server-side**:
```bash
RUST_LOG=logseq_spring_thing::handlers::socket_flow_handler=debug,\
logseq_spring_thing::utils::binary_protocol=trace
```

**Client-side**:
```javascript
localStorage.setItem('debug', 'websocket:*,binary:*');
```

### Binary Message Inspection

To inspect binary messages in browser DevTools:

```javascript
// In console, before connecting:
const originalSend = WebSocket.prototype.send;
WebSocket.prototype.send = function(data) {
    if (data instanceof ArrayBuffer) {
        console.log('Binary message:', new Uint8Array(data));
    }
    return originalSend.call(this, data);
};
```

## Security Considerations

### Binary Protocol Security

1. **Input Validation**
   - All node IDs are validated against known nodes
   - Position/velocity values are bounds-checked
   - Message size limits prevent memory exhaustion

2. **Rate Limiting**
   - Client update frequency is throttled server-side
   - Maximum nodes per update is enforced
   - Connection count limits prevent DoS

3. **Authentication**
   - Binary updates require valid session
   - Node modifications are access-controlled
   - Audit logging for suspicious patterns

For more details on the binary protocol implementation, see [Binary Protocol Documentation](./binary-protocol.md).
