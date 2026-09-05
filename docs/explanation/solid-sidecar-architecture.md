---
title: Solid Sidecar Architecture
description: Technical architecture for VisionClaw's Solid/LDP integration using JSON Solid Server (JSS) as a sidecar container
category: explanation
tags: [architecture, solid, ldp, sidecar, jss, pods]
updated-date: 2026-04-09
---

# Solid Sidecar Architecture
## Technical Architecture for Decentralized Data Storage

> **ADR-11 update**: The "Neo4j (Graph DB)" component shown throughout this design is now
> the **embedded Oxigraph** RDF triple store running in-process inside the Rust backend —
> there is no separate graph-database container, Bolt URI, or DB password. The sync tool
> names below (`neo4j-to-solid` / `solid-to-neo4j`) are historical; the live equivalents
> export from / import to the embedded Oxigraph store. RDF export to Solid is now a
> direct serialization (Oxigraph is already RDF-native).
>
> **UPDATED 2026-06-12**: The Solid surface shipped **embedded**, not as a JSS
> Node.js sidecar: the Rust backend serves `/api/solid/*` and `/pods/*` itself
> via the `solid-pod-rs` crate (`configure_solid_routes` in `src/main.rs`;
> nginx proxies `/solid/` and `/pods/` to the backend). The current compose
> defines **no `jss` container**. Diagrams below showing a separate
> "JSS Sidecar (:3030)" are design-era intent — read "JSS" as the embedded
> solid-pod-rs LDP layer. References to "Binary V2" predate the V3 52 B/node
> wire (`docs/binary-protocol.md`).

**Version:** 1.0
**Date:** 2025-12-29
**Target:** VisionClaw with Solid/LDP Integration
**Status:** Architecture Design (graph store updated per ADR-11)

---

## Executive Summary

This document defines the technical architecture for VisionClaw's Solid integration, enabling decentralized data ownership through Linked Data Platform (LDP) compliance. The system uses JSON Solid Server (JSS) as a sidecar container, providing:

1. **Decentralized Storage** - User-owned Solid pods for graph data
2. **LDP Compliance** - Standard Linked Data Platform operations
3. **Real-time Notifications** - WebSocket-based resource change notifications
4. **NIP-98 Authentication** - Nostr-based decentralized identity

---

## 1. System Architecture Overview

### 1.1 High-Level Component Diagram

```
                                    VisionClaw System
    +------------------------------------------------------------------------+
    |                                                                        |
    |  +------------------+     +------------------+     +------------------+ |
    |  |                  |     |                  |     |                  | |
    |  |   React Client   |<--->|  Rust Backend    |<--->| Oxigraph (embed) | |
    |  |   (Three.js)     |     |  (Actix-web)     |     | RDF triple store | |
    |  |                  |     |                  |     |                  | |
    |  +--------+---------+     +--------+---------+     +------------------+ |
    |           |                        |                                   |
    |           | WebSocket              | HTTP/REST                         |
    |           | (Binary V2)            |                                   |
    |           |                        |                                   |
    |  +--------v------------------------v---------+                         |
    |  |                                           |                         |
    |  |        JSS Sidecar (Node.js)              |                         |
    |  |        +---------------------------+      |                         |
    |  |        |  Solid Server             |      |                         |
    |  |        |  - LDP Container Mgmt     |      |                         |
    |  |        |  - RDF Serialization      |      |                         |
    |  |        |  - WebSocket Notifications|      |                         |
    |  |        |  - NIP-98 Auth Handler    |      |                         |
    |  |        +---------------------------+      |                         |
    |  |                    |                      |                         |
    |  |        +-----------v-----------+          |                         |
    |  |        |    Pod Storage        |          |                         |
    |  |        |    /data/pods/        |          |                         |
    |  |        |    +-- user1/         |          |                         |
    |  |        |    |   +-- graph/     |          |                         |
    |  |        |    |   +-- profile/   |          |                         |
    |  |        |    +-- user2/         |          |                         |
    |  |        +-----------------------+          |                         |
    |  |                                           |                         |
    |  +-------------------------------------------+                         |
    |                                                                        |
    +------------------------------------------------------------------------+
```

### 1.2 Docker Compose Profile

```yaml
# docker-compose.solid.yml
version: '3.8'

services:
  jss:
    image: solidproject/community-server:latest
    container_name: visionclaw-jss
    profiles:
      - solid
    ports:
      - "${JSS_PORT:-3000}:3000"
      - "${SOLID_WS_PORT:-3001}:3001"
    volumes:
      - solid-pods:/data/pods
      - ./config/solid.json:/config/config.json:ro
    environment:
      - CSS_CONFIG=/config/config.json
      - CSS_BASE_URL=${SOLID_POD_BASE_URL:-http://localhost:3000}
      - CSS_LOGGING_LEVEL=${JSS_LOG_LEVEL:-info}
    networks:
      - visionclaw-net
    depends_on:
      - backend
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/.well-known/solid"]
      interval: 30s
      timeout: 10s
      retries: 3

  backend:
    # ... existing backend config
    environment:
      - JSS_ENABLED=${JSS_ENABLED:-false}
      - JSS_HOST=jss
      - JSS_PORT=3000

volumes:
  solid-pods:
    driver: local

networks:
  visionclaw-net:
    driver: bridge
```

---

## 2. Data Flow Architecture

### 2.1 Graph Store to Solid Synchronization

```mermaid
flowchart TB
    subgraph GraphStore["Oxigraph (embedded RDF store)"]
        N1[Node triples]
        N2[Edge triples]
        N3[Metadata triples]
    end

    subgraph Backend["Rust Backend"]
        B1[Sync Service]
        B2[RDF Serializer]
        B3[Batch Processor]
    end

    subgraph JSS["JSS Sidecar"]
        J1[LDP Server]
        J2[Pod Manager]
        J3[Notification Hub]
    end

    subgraph Storage["Pod Storage"]
        S1["/pods/user1/graph/"]
        S2["node-1.ttl"]
        S3["node-2.ttl"]
    end

    N1 --> B1
    N2 --> B1
    N3 --> B1
    B1 --> B2
    B2 --> B3
    B3 -->|"PUT /pods/{user}/graph/{id}.ttl"| J1
    J1 --> J2
    J2 --> S1
    S1 --> S2
    S1 --> S3
    J2 --> J3
    J3 -->|"WebSocket: notification"| Client
```

### 2.2 Client Request Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant B as Backend
    participant J as JSS
    participant N as Oxigraph (embedded)

    Note over C,N: Read Operation (Solid-first)
    C->>J: GET /pods/user/graph/node-1.ttl
    J->>J: Check authorization
    J-->>C: 200 OK (Turtle RDF)

    Note over C,N: Write Operation (Backend-first)
    C->>B: POST /api/nodes
    B->>N: CREATE (n:Node {...})
    N-->>B: Node created
    B->>J: PUT /pods/user/graph/node-{id}.ttl
    J-->>B: 201 Created
    J->>C: WebSocket: notification (create)
    B-->>C: 201 Created
```

### 2.3 WebSocket Notification Flow

```mermaid
sequenceDiagram
    participant C1 as Client 1
    participant C2 as Client 2
    participant J as JSS
    participant B as Backend

    C1->>J: WebSocket: subscribe /pods/user/graph/
    J-->>C1: subscribed

    C2->>J: WebSocket: subscribe /pods/user/graph/
    J-->>C2: subscribed

    B->>J: PATCH /pods/user/graph/node-1.ttl
    J->>J: Update resource
    J->>C1: notification (update, node-1.ttl)
    J->>C2: notification (update, node-1.ttl)
```

---

## 3. Component Specifications

### 3.1 JSS Configuration

```json
{
  "@context": "https://linkedsoftwaredependencies.org/bundles/npm/@solid/community-server/^7.0.0/components/context.jsonld",
  "import": [
    "css:config/app/main/default.json",
    "css:config/file/storage/file.json",
    "css:config/http/handler/default.json",
    "css:config/http/middleware/websockets.json",
    "css:config/identity/handler/default.json",
    "css:config/ldp/authorization/webacl.json",
    "css:config/ldp/handler/default.json",
    "css:config/storage/backend/file.json"
  ],
  "@graph": [
    {
      "@id": "urn:solid-server:default:ServerConfigurator",
      "comment": "VisionClaw JSS Configuration",
      "baseUrl": "http://localhost:3000/",
      "rootFilePath": "/data/pods/"
    },
    {
      "@id": "urn:solid-server:default:WebSocketHandler",
      "comment": "Enable WebSocket notifications",
      "WebSocketHandler:_protocol": "solid-0.1"
    }
  ]
}
```

### 3.2 RDF Serialization Format

VisionClaw graph nodes are serialized to Turtle RDF:

```turtle
@prefix vf: <https://visionclaw.example.com/ontology#> .
@prefix ldp: <http://www.w3.org/ns/ldp#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix dcterms: <http://purl.org/dc/terms/> .

<> a vf:KGNode ;
   vf:nodeId "node-550e8400-e29b-41d4-a716-446655440000"^^xsd:string ;
   vf:label "Example Concept"^^xsd:string ;
   vf:type "concept"^^xsd:string ;
   vf:position "1.5,-2.3,0.8"^^xsd:string ;
   vf:velocity "0.0,0.0,0.0"^^xsd:string ;
   vf:weight "1.0"^^xsd:decimal ;
   vf:group "ontology"^^xsd:string ;
   dcterms:created "2025-12-29T10:00:00Z"^^xsd:dateTime ;
   dcterms:modified "2025-12-29T12:30:00Z"^^xsd:dateTime .
```

### 3.3 Container Structure

```
/pods/
  +-- {userId}/
      +-- profile/
      |   +-- card#me           # WebID document
      |   +-- preferences.ttl   # User preferences
      +-- graph/
      |   +-- .meta             # Container metadata
      |   +-- node-{id}.ttl     # Individual node RDF
      |   +-- edge-{id}.ttl     # Edge relationships
      +-- workspaces/
      |   +-- {workspaceId}/
      |       +-- settings.ttl
      |       +-- graph/
      +-- .acl                  # Access control
```

---

## 4. Authentication Architecture

### 4.1 NIP-98 Integration

```mermaid
sequenceDiagram
    participant C as Client
    participant B as Backend
    participant J as JSS
    participant R as Nostr Relay

    C->>C: Generate NIP-98 event (kind 27235)
    C->>C: Sign with Nostr private key

    C->>B: POST /api/auth/nostr
    Note over C,B: Authorization: Nostr base64(event)

    B->>B: Decode event from header
    B->>B: Verify Schnorr signature
    B->>B: Validate timestamp (60s window)
    B->>B: Check URL and method tags

    opt Reputation Check
        B->>R: GET pubkey metadata
        R-->>B: NIP-01 profile
    end

    B->>B: Generate JWT session token
    B->>J: Create/verify pod access
    B-->>C: 200 OK + JWT + Pod WebID

    Note over C,J: Subsequent requests use JWT
    C->>J: GET /pods/{user}/graph/
    Note over C,J: Authorization: Bearer {jwt}
    J-->>C: 200 OK + RDF data
```

### 4.2 Access Control (WAC)

```turtle
# /pods/user123/.acl
@prefix acl: <http://www.w3.org/ns/auth/acl#> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .

<#owner>
    a acl:Authorization ;
    acl:agent <profile/card#me> ;
    acl:accessTo <./> ;
    acl:default <./> ;
    acl:mode acl:Read, acl:Write, acl:Control .

<#public-read>
    a acl:Authorization ;
    acl:agentClass foaf:Agent ;
    acl:accessTo <graph/> ;
    acl:default <graph/> ;
    acl:mode acl:Read .
```

---

## 5. Synchronization Strategy

### 5.1 Sync Modes

| Mode | Direction | Use Case |
|------|-----------|----------|
| `graph-to-solid` | Oxigraph -> Solid | Export graph for sharing |
| `solid-to-graph` | Solid -> Oxigraph | Import external data |
| `bidirectional` | Both | Collaborative editing |

### 5.2 Conflict Resolution

```mermaid
flowchart TD
    A[Detect Conflict] --> B{Compare Timestamps}
    B -->|Graph store newer| C[Graph store wins]
    B -->|Solid newer| D[Solid wins]
    B -->|Same time| E{Compare checksums}
    E -->|Different| F[Create merge node]
    E -->|Same| G[No conflict]

    C --> H[Update Solid]
    D --> I[Update graph store]
    F --> J[Manual resolution required]
```

### 5.3 Batch Sync Process

```rust
// Pseudocode for batch synchronization
async fn sync_batch(nodes: Vec<Node>, pod_id: &str) -> Result<SyncResult> {
    let batch_size = config.solid_sync_batch_size;
    let mut results = SyncResult::default();

    for chunk in nodes.chunks(batch_size) {
        let rdf_batch = serialize_to_turtle(chunk)?;

        for (node, turtle) in chunk.iter().zip(rdf_batch) {
            let path = format!("/pods/{}/graph/node-{}.ttl", pod_id, node.id);

            match jss_client.put(&path, turtle).await {
                Ok(_) => results.created += 1,
                Err(e) if e.status() == 409 => {
                    // Conflict - attempt merge
                    handle_conflict(node, &path).await?;
                    results.merged += 1;
                }
                Err(e) => results.errors.push(e.to_string()),
            }
        }

        // Yield to event loop between batches
        tokio::task::yield_now().await;
    }

    Ok(results)
}
```

---

## 6. Performance Considerations

### 6.1 Caching Strategy

| Layer | Cache Type | TTL | Purpose |
|-------|------------|-----|---------|
| Client | Browser cache | 5 min | Reduce requests |
| Backend | Redis | 1 min | RDF serialization |
| JSS | In-memory | 30 sec | Hot resources |

### 6.2 Scaling Recommendations

| Deployment Size | Pods | JSS Instances | Memory |
|-----------------|------|---------------|--------|
| Small | < 100 | 1 | 512 MB |
| Medium | 100-1000 | 2-3 | 1 GB each |
| Large | > 1000 | 4+ | 2 GB each |

### 6.3 Resource Limits

```yaml
# docker-compose.solid.yml
services:
  jss:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

---

## 7. Security Considerations

### 7.1 Security Checklist

- [ ] Enable HTTPS for production JSS
- [ ] Configure WAC for all containers
- [ ] Set CORS origins explicitly
- [ ] Enable DPoP token binding
- [ ] Implement rate limiting
- [ ] Regular security audits

### 7.2 Network Isolation

```yaml
networks:
  visionclaw-internal:
    internal: true  # No external access
  visionclaw-public:
    driver: bridge

services:
  jss:
    networks:
      - visionclaw-internal
      - visionclaw-public
  backend:
    networks:
      - visionclaw-internal
    # The graph store is embedded in the backend process (Oxigraph) — no separate
    # graph-database service or network entry is required (ADR-11).
```

---

## 8. Monitoring and Observability

### 8.1 Health Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /.well-known/solid` | Solid discovery |
| `GET /health` | JSS health check |
| `GET /metrics` | Prometheus metrics |

### 8.2 Key Metrics

- `solid_pods_total` - Total pod count
- `solid_requests_total` - Request count by method
- `solid_ws_connections` - Active WebSocket connections
- `solid_sync_duration_seconds` - Sync operation duration
- `solid_storage_bytes` - Storage usage per pod

---

## 9. Related Documentation

| Topic | Documentation |
|-------|---------------|
| Protocol Specification | [PROTOCOL_REFERENCE.md](../reference/mcp-tools.md) |
| API Endpoints | [rest-api.md](../reference/rest-api.md) |
| Configuration | [CONFIGURATION_REFERENCE.md](../reference/configuration.md#solid) |

---

**Architecture Version**: 1.0
**VisionClaw Version**: v0.1.0
**Maintainer**: VisionClaw Architecture Team
**Last Updated**: December 29, 2025
