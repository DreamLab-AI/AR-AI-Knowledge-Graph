# Design: JSS as Nostr Relay++

## Overview

Replace the Solid Notifications Protocol with Nostr relay functionality. JSS becomes a Nostr relay that also serves LDP resources, unifying identity, storage, and real-time notifications.

## Motivation

**Solid Notifications Protocol problems:**
- Complex discovery mechanism
- JSON-LD channel descriptions
- No federation
- No existing ecosystem
- Reinvents pub/sub poorly

**Nostr advantages:**
- Simple WebSocket protocol (NIP-01)
- Cryptographic identity built-in
- Federation via relay gossip
- Millions of existing users
- Mobile push infrastructure exists
- Battle-tested

**JSS already has:**
- NIP-98 HTTP authentication
- WebSocket infrastructure (solid-0.1)
- JSON-LD storage

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         JSS Server                          │
├──────────────────────┬──────────────────────────────────────┤
│     LDP Layer        │         Nostr Relay Layer            │
│                      │                                      │
│  GET/PUT/POST/PATCH  │    EVENT/REQ/CLOSE/EOSE              │
│  DELETE/OPTIONS      │                                      │
│                      │                                      │
│  ┌────────────────┐  │  ┌─────────────────────────────────┐ │
│  │  Resources     │◄─┼─►│  Events (kind:30078)            │ │
│  │  /alice/doc.ttl│  │  │  Addressable by d-tag = URI     │ │
│  └────────────────┘  │  └─────────────────────────────────┘ │
│                      │                                      │
│  Auth: Solid-OIDC    │    Auth: NIP-98 / NIP-42             │
│        NIP-98        │                                      │
└──────────────────────┴──────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Other Relays   │
                    │  (Federation)   │
                    └─────────────────┘
```

## Protocol Mapping

### Resource ↔ Event Mapping

LDP resources map to Nostr replaceable events:

```
Resource URL: https://alice.solid.social/notes/idea.json
     ↓
Nostr Event:
{
  "kind": 30078,                    // Arbitrary JSON (NIP-78)
  "pubkey": "<alice-pubkey>",
  "created_at": 1703888888,
  "tags": [
    ["d", "https://alice.solid.social/notes/idea.json"],
    ["solid:type", "ldp:Resource"],
    ["solid:contentType", "application/ld+json"]
  ],
  "content": "{\"@context\": ..., \"title\": \"My Idea\"}",
  "sig": "<signature>"
}
```

### Kind Assignments

| Kind | Purpose | NIP |
|------|---------|-----|
| 30078 | LDP Resource (JSON content) | NIP-78 |
| 30079 | LDP Container listing | Custom |
| 30080 | ACL document | Custom |
| 10078 | Resource deletion marker | Custom |
| 1 | Social posts (optional integration) | NIP-01 |

Using 30xxx range for addressable replaceable events (d-tag = resource URI).

### Subscription Filters

Subscribe to resource changes:

```json
// Subscribe to single resource
["REQ", "sub1", {
  "kinds": [30078],
  "#d": ["https://alice.solid.social/notes/idea.json"]
}]

// Subscribe to container (all resources under path)
["REQ", "sub2", {
  "kinds": [30078, 30079],
  "#d": ["https://alice.solid.social/notes/"]
}]

// Subscribe to all changes by user
["REQ", "sub3", {
  "kinds": [30078],
  "authors": ["<alice-pubkey>"]
}]
```

## Implementation

### Phase 1: Basic Relay

Add NIP-01 relay functionality to existing WebSocket endpoint:

```javascript
// src/notifications/nostr-relay.js

export function handleNostrMessage(socket, message) {
  const [type, ...params] = JSON.parse(message);

  switch (type) {
    case 'EVENT':
      return handleEvent(socket, params[0]);
    case 'REQ':
      return handleSubscription(socket, params[0], params.slice(1));
    case 'CLOSE':
      return handleClose(socket, params[0]);
  }
}
```

### Phase 2: LDP-Event Bridge

When LDP resources change, emit Nostr events:

```javascript
// src/handlers/resource.js (modified)

export async function handlePut(request, reply) {
  // ... existing LDP logic ...

  // After successful write, emit Nostr event
  if (request.nostrPubkey) {
    await emitResourceEvent({
      pubkey: request.nostrPubkey,
      resourceUrl,
      content,
      contentType
    });
  }
}
```

### Phase 3: Federation

Connect to other relays for event propagation:

```javascript
// src/notifications/federation.js

const FEDERATION_RELAYS = [
  'wss://relay.damus.io',
  'wss://nos.lol',
  'wss://relay.nostr.band'
];

export async function federateEvent(event) {
  // Only federate public resources
  if (await isPublicResource(event.tags.find(t => t[0] === 'd')[1])) {
    for (const relay of FEDERATION_RELAYS) {
      publishToRelay(relay, event);
    }
  }
}
```

### Phase 4: Identity Unification

WebID document includes Nostr pubkey:

```json
{
  "@context": {...},
  "@id": "https://alice.solid.social/profile/card#me",
  "foaf:name": "Alice",
  "nostr:pubkey": "npub1abc...",
  "nostr:relays": ["wss://alice.solid.social"]
}
```

Nostr profile (kind:0) links to WebID:

```json
{
  "kind": 0,
  "content": "{\"name\":\"Alice\",\"webid\":\"https://alice.solid.social/profile/card#me\"}"
}
```

## WebSocket Endpoint

Single endpoint handles both protocols:

```
wss://alice.solid.social/.notifications

Protocol detection:
- If first message is JSON array starting with "EVENT"/"REQ" → Nostr
- If first message is "sub <uri>" → Legacy solid-0.1
```

```javascript
// src/notifications/websocket.js (modified)

export function handleWebSocket(socket, request) {
  socket.on('message', (message) => {
    const msg = message.toString().trim();

    // Detect protocol
    if (msg.startsWith('[')) {
      // Nostr protocol
      handleNostrMessage(socket, msg);
    } else if (msg.startsWith('sub ') || msg.startsWith('unsub ')) {
      // Legacy solid-0.1
      handleSolidMessage(socket, msg);
    }
  });
}
```

## Access Control

### Public Resources
- Events federate to other relays
- Anyone can subscribe

### Private Resources
- Events stay local (no federation)
- NIP-42 AUTH required to subscribe
- Subscription filter must match authorized pubkeys

```javascript
// NIP-42 AUTH flow
["AUTH", "<signed-event>"]

// Server validates and restricts subscriptions
// to resources the pubkey has access to
```

### ACL Mapping

```
acl:Read   → Can subscribe to events
acl:Write  → Can publish events (create/update)
acl:Control → Can modify ACL events
```

## Storage

Two options:

### Option A: Dual Storage (Recommended for Phase 1)
- LDP resources in filesystem (existing)
- Nostr events in SQLite/memory (relay state)
- Bridge syncs between them

### Option B: Event-Native Storage (Future)
- All resources stored as Nostr events
- LDP is a view over event history
- Full audit trail built-in
- Replaces filesystem storage

## Configuration

```json
{
  "nostr": {
    "enabled": true,
    "relay": {
      "nip01": true,
      "nip42": true,
      "nip78": true
    },
    "federation": {
      "enabled": false,
      "relays": [],
      "publicOnly": true
    },
    "kinds": {
      "resource": 30078,
      "container": 30079,
      "acl": 30080
    }
  }
}
```

## Migration Path

1. **Phase 1**: Add relay alongside existing WebSocket
   - Both protocols on same endpoint
   - No breaking changes

2. **Phase 2**: LDP-Event bridge
   - Changes emit events
   - Subscriptions work via Nostr

3. **Phase 3**: Federation (optional)
   - Public resources propagate
   - Discovery via relay network

4. **Phase 4**: Deprecate solid-0.1
   - Nostr becomes primary notification protocol
   - Mashlib adapter if needed

## Benefits

| Feature | Solid Notifications | Nostr Relay++ |
|---------|--------------------|--------------|
| Protocol complexity | High | Low |
| Existing clients | ~0 | Millions |
| Federation | No | Yes |
| Mobile push | Build it yourself | Existing infrastructure |
| Identity | Separate (WebID) | Integrated (npub) |
| Signatures | Optional | Every event |
| Ecosystem | Academic | Active |

## Open Questions

1. **Kind numbers**: Apply for official NIP allocation or use 30078-30080 range?

2. **Content encoding**: Store JSON-LD directly in content, or reference by hash?

3. **Large resources**: Nostr events have size limits. Use NIP-94/NIP-96 for large files?

4. **Container semantics**: How to represent ldp:contains in events?

5. **Conflict resolution**: Last-write-wins via created_at, or something smarter?

## References

- [NIP-01: Basic Protocol](https://github.com/nostr-protocol/nips/blob/master/01.md)
- [NIP-42: Authentication](https://github.com/nostr-protocol/nips/blob/master/42.md)
- [NIP-78: Arbitrary Custom App Data](https://github.com/nostr-protocol/nips/blob/master/78.md)
- [NIP-98: HTTP Auth](https://github.com/nostr-protocol/nips/blob/master/98.md)
- [Solid Protocol](https://solidproject.org/TR/protocol)
