# Security Documentation

This document covers the security architecture, practices, and considerations for the VisionClaw Vircadia integration.

## SQL Parameterization

All SQL queries sent from the client to the Vircadia World Server use parameterized placeholders (`$1`, `$2`, ...) to prevent SQL injection. The `QueryOptions` interface enforces this pattern:

```typescript
interface QueryOptions {
    query: string;           // SQL with $1, $2, ... placeholders
    parameters?: unknown[];  // Bound parameter values
    timeoutMs?: number;
}
```

### Parameterized Services

The following services use fully parameterized queries:

| Service | Operations | Status |
|:--------|:-----------|:-------|
| **VircadiaClientCore** | All `query()` calls | Parameterized |
| **ThreeJSAvatarRenderer** | Avatar INSERT, UPDATE, SELECT | Parameterized |
| **EntitySyncManager** | Entity INSERT, UPDATE, DELETE, SELECT | Parameterized |
| **GraphEntityMapper** | All generated SQL | Parameterized |

**Example -- avatar position broadcast:**

```typescript
const query = `
    UPDATE entity.entities
    SET meta__data = jsonb_set(
        jsonb_set(
            jsonb_set(meta__data, '{position}', $1::jsonb),
            '{rotation}', $2::jsonb
        ),
        '{timestamp}', $3::text::jsonb
    )
    WHERE general__entity_name = $4
`;

await client.Utilities.Connection.query({
    query,
    parameters: [
        JSON.stringify({ x: pos.x, y: pos.y, z: pos.z }),
        JSON.stringify({ x: rot.x, y: rot.y, z: rot.z, w: rot.w }),
        String(Date.now()),
        `avatar_${agentId}`
    ]
});
```

### Known Non-Parameterized Queries

The following services contain string-interpolated SQL. These queries construct entity names or JSON payloads via template literals rather than bound parameters:

| Service | Methods | Risk |
|:--------|:--------|:-----|
| **SpatialAudioManager** | `sendOffer`, `sendAnswer`, `sendICECandidate`, `handleSignalingMessages` | Entity names and JSON payloads interpolated |
| **NetworkOptimizer** | `flushBatch` (non-compressed path) | Position values and entity names interpolated |
| **Quest3Optimizer** | `broadcastHandData`, `broadcastControllerState` | JSON payloads and entity names interpolated |

These methods interpolate local agent IDs and serialized JSON into SQL strings. While the values originate from trusted client-side state (not user input), this pattern should be migrated to parameterized queries for defense-in-depth.

## WebSocket Authentication

### Connection Flow

Authentication happens at WebSocket connection time. The client passes credentials as URL query parameters:

```
wss://host:3020/world/ws?token=<auth_token>&provider=<auth_provider>
```

The Vircadia World Server validates the token against the configured auth provider and returns a `SESSION_INFO_RESPONSE` with:
- `agentId` -- unique identifier for the connected agent
- `sessionId` -- session identifier for the connection

### Supported Auth Providers

| Provider | Description |
|:---------|:------------|
| `system` | Internal token-based authentication |
| `nostr` | Nostr identity-based authentication (NIP-07) |

### Session Management

- Sessions have a configurable timeout (default: 86400 seconds / 24 hours)
- The client maintains a heartbeat every 30 seconds using a `SELECT 1 as heartbeat` query
- If the heartbeat fails or the WebSocket closes, the client enters reconnection mode
- Maximum reconnection attempts and delay are configurable via `ClientCoreConfig`

### Token Security Considerations

- Auth tokens are transmitted over the WebSocket URL, which means they appear in server access logs and browser history
- For production deployments, use WSS (WebSocket Secure) to encrypt the connection
- Tokens should have a limited lifetime and be rotated regularly
- The `VIRCADIA_JWT_SECRET` environment variable must be changed from its default value before production deployment

## WebRTC Security

### ICE/STUN/TURN

Spatial audio uses WebRTC peer connections. The default configuration uses Google's public STUN servers:

```typescript
iceServers: [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' }
]
```

For production deployments behind NATs or firewalls, configure a TURN server:

```typescript
iceServers: [
    { urls: 'stun:stun.example.com:3478' },
    {
        urls: 'turn:turn.example.com:3478',
        username: 'user',
        credential: 'password'
    }
]
```

### DTLS Encryption

WebRTC mandates DTLS (Datagram Transport Layer Security) for all peer connections. Audio streams between peers are encrypted by default. This is handled by the browser's WebRTC implementation and requires no additional configuration.

### Signaling Security

WebRTC signaling (offer/answer/ICE candidate exchange) is performed through the Vircadia entity store. Signaling messages are stored as entities with names like:

- `webrtc_offer_{fromAgent}_{toAgent}`
- `webrtc_answer_{fromAgent}_{toAgent}`
- `webrtc_ice_{fromAgent}_{toAgent}_{timestamp}`

These entities are readable by any connected client querying the entity store. In a multi-tenant deployment, signaling entities should be scoped to prevent cross-world information leakage.

## Input Validation Boundaries

### Client-Side Validation

| Boundary | Validation |
|:---------|:-----------|
| WebSocket messages | JSON.parse with try/catch; malformed messages are logged and discarded |
| Binary protocol | Header size validation; payload length verification; protocol version check |
| Query responses | Request ID matching; timeout enforcement per query |
| Remote avatar data | Position and rotation values parsed from entity metadata with null checks |
| Hand tracking joints | Joint array bounds checking before mesh updates |
| Feature flags | `rolloutPercentage` clamped to 0-100 range; localStorage parse errors caught |

### Server-Side Validation (Vircadia World Server)

The Vircadia World Server enforces:

- SQL query parsing and validation before execution
- Entity name uniqueness constraints
- Sync group access control
- Maximum entities per user (configurable, default: 1000)
- Connection authentication via token verification

## Binary Protocol Validation

The `BinaryWebSocketProtocol` class performs defensive validation on all incoming binary messages:

1. **Header size check** -- buffer must be at least `MESSAGE_HEADER_SIZE` (4 bytes)
2. **Protocol version check** -- only V2 and V3 are accepted; unsupported versions are rejected
3. **Payload length verification** -- declared payload length must match actual buffer size
4. **Per-record size validation** -- position updates must be exact multiples of `AGENT_POSITION_SIZE` (21 bytes)
5. **Truncation detection** -- partial records at the end of a buffer are logged and skipped

## Known Security Considerations

### 1. Client-Side SQL Execution

The SQL-over-WebSocket pattern means the client sends raw SQL queries to the server. While the server validates and executes these queries, any connected client can submit arbitrary queries within its permission scope. The server-side query validator and PostgreSQL role permissions are the primary defense.

### 2. Entity Store as Signaling Medium

WebRTC signaling data (SDP offers/answers, ICE candidates) is stored in the same entity table as application data. Any connected client can read signaling entities. For sensitive deployments, signaling should use a dedicated channel or be encrypted at the application layer.

### 3. Feature Flag Storage

Feature flags are persisted to `localStorage`, which is accessible to any JavaScript running on the same origin. Feature flags should not be used as a security boundary -- they control UX behavior, not access control.

### 4. Default Credentials

The Docker Compose configuration ships with default credentials that must be changed before production deployment:

| Credential | Default Value | Environment Variable |
|:-----------|:-------------|:---------------------|
| PostgreSQL password | `visionclaw_secure` | `POSTGRES_PASSWORD` |
| JWT secret | `change_this_in_production` | `VIRCADIA_JWT_SECRET` |

### 5. Microphone Access

The `SpatialAudioManager` requests microphone access via `navigator.mediaDevices.getUserMedia()`. The browser will prompt the user for permission. Audio streams are only shared with established WebRTC peers and are encrypted via DTLS.

## Recommendations

1. **Migrate all SQL queries to parameterized form** -- Prioritize SpatialAudioManager, NetworkOptimizer, and Quest3Optimizer
2. **Deploy with WSS** -- Always use TLS-encrypted WebSocket connections in production
3. **Configure a TURN server** -- Required for reliable connectivity behind NATs and firewalls
4. **Change all default credentials** -- PostgreSQL password, JWT secret, and any API keys
5. **Scope signaling entities** -- Add access control to prevent cross-world signaling leakage
6. **Rate-limit query submissions** -- Server-side rate limiting on the SQL-over-WebSocket interface
7. **Audit entity permissions** -- Ensure PostgreSQL roles restrict what queries each auth provider can execute
