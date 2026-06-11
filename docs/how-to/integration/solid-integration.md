---
title: Solid Pod Integration
description: Comprehensive guide for VisionClaw's Solid Pod integration — decentralized user data, graph views, ontology governance, agent memory, and Type Index discovery via JavaScript Solid Server (JSS).
category: how-to
tags:
  - solid
  - decentralized
  - storage
  - pods
  - nostr
  - authentication
  - ldp
  - rdf
  - agent-memory
  - sparql-patch
  - type-index
updated-date: 2026-04-03
difficulty-level: intermediate
dependencies:
  - Nostr authentication enabled
  - Docker deployment with JSS sidecar
---

# Solid Pod Integration

## Overview

VisionClaw integrates with [Solid](https://solidproject.org/) (Social Linked Data) via the [JavaScript Solid Server (JSS)](https://github.com/JavaScriptSolidServer/JavaScriptSolidServer) to provide decentralized, user-controlled data storage. Each user gets a personal Pod where they own their graph views, ontology contributions, agent memory, and preferences.

### About JSS

**JavaScriptSolidServer** (v0.0.86), maintained by the JavaScriptSolidServer contributors, is a minimal, fast, JSON-LD native Solid server. Unlike heavier alternatives, JSS is ~1MB with ~15 dependencies, runs on Node.js (including Android/Termux), and stores data as plain files — no database required.

JSS provides capabilities far beyond basic LDP storage:

| Feature | Status | Used by VisionClaw |
|---------|--------|-------------------|
| LDP CRUD (GET/PUT/POST/DELETE/PATCH) | Implemented | Yes |
| Web Access Control (WAC) with `.acl` files | Implemented | Yes — agent memory ACLs |
| WebSocket notifications (solid-0.1) | Implemented | Yes — cross-device sync |
| SPARQL Update PATCH | Implemented | Yes — ontology mutations |
| N3 Patch with `solid:where` | Implemented | Yes — conflict detection |
| Nostr NIP-98 authentication | Implemented | Yes — primary auth |
| `did:nostr` → WebID resolution | Implemented | Yes — identity mapping |
| Schnorr SSO / Passkeys | Implemented | Available |
| Git HTTP backend | Implemented | Future — ontology versioning |
| ActivityPub federation | Implemented | Future — federated KG |
| HTTP 402 micropayments | Implemented | Future — data marketplace |
| WebRTC signaling | Implemented | Future — P2P collaboration |
| Content negotiation (JSON-LD/Turtle) | Implemented | Yes |
| Multi-user pod support | Implemented | Yes |

**Repository:** [github.com/JavaScriptSolidServer/JavaScriptSolidServer](https://github.com/JavaScriptSolidServer/JavaScriptSolidServer)
**License:** AGPL-3.0
**Documentation:** [javascriptsolidserver.github.io/docs/](https://javascriptsolidserver.github.io/docs/)

## Architecture

```
Browser (React + Three.js)
    │
    ├── SolidPodService.ts ──── NIP-98 Auth ────┐
    │   ├── Graph Views (save/load/list)         │
    │   ├── Type Index (discover/register)       │
    │   └── Agent Memory (store/list/audit)      │
    │                                            ▼
    ├── Nginx (/solid/* proxy) ──────────► JSS (port 3030)
    │   ├── /solid/{pod}/settings/               │
    │   ├── /solid/{pod}/agents/{id}/memory/     │
    │   └── /solid/.notifications (WebSocket)    │
    │                                            │
    └── JssOntologyService.ts                    │
        ├── SPARQL PATCH mutations ──────────────┘
        └── N3 Patch with solid:where
```

### Docker Sidecar

JSS runs as a Docker sidecar service alongside VisionClaw:

```yaml
# docker-compose.unified.yml
jss:
  container_name: visionclaw-jss
  build:
    context: ./JavaScriptSolidServer
    dockerfile: Dockerfile.jss
  environment:
    - JSS_HOST=0.0.0.0
    - JSS_PORT=3030
    - JSS_NOTIFICATIONS=true    # WebSocket real-time updates
    - JSS_CONNEG=true           # Content negotiation
    - JSS_MULTIUSER=true        # Multi-pod support
    - JSS_IDP=false             # Uses Nostr auth, not built-in IdP
    - JSS_MASHLIB_CDN=true      # Browser-based data navigation
  ports:
    - "3030:3030"
  volumes:
    - jss-data:/data
```

### Authentication Flow

VisionClaw uses Nostr NIP-98 for all Solid operations:

1. User logs in with Nostr (NIP-07 browser extension: nos2x, Alby, etc.)
2. Client signs each HTTP request with NIP-98 event signature
3. VisionClaw proxy forwards the `Authorization: Nostr <token>` header to JSS
4. JSS resolves `did:nostr:{pubkey}` to a WebID
5. WAC `.acl` files control per-resource access using WebIDs

## Features

### 1. Pod-Backed Graph Views (ADR-027)

Save, load, and share named graph views across devices.

```typescript
import solidPodService from '@/services/SolidPodService';

// Save current view
await solidPodService.saveGraphView('AI Research', {
  camera: { x: 10, y: 20, z: 50 },
  physics: { repelK: 5000, springK: 100, restLength: 300 },
  clusters: { algorithm: 'louvain', count: 8 },
  nodeTypeVisibility: { knowledge: true, ontology: true, agent: false },
});

// Load a saved view
const view = await solidPodService.loadGraphView('AI Research');

// List all saved views
const views = await solidPodService.listGraphViews();

// Subscribe to cross-device sync
solidPodService.subscribeToGraphViewChanges((viewName) => {
  console.log(`View "${viewName}" updated on another device`);
});
```

**Natural language:** Collapse the control panel and type `save view AI Research` or `load view default` in the command input.

### 2. SPARQL PATCH for Ontology Mutations (ADR-028)

Surgical RDF edits instead of full-document replacement:

```typescript
import { jssOntologyService } from '@/features/ontology/services/JssOntologyService';

// Add a triple
await jssOntologyService.addOntologyTriple(
  { type: 'iri', value: 'http://example.org/Person' },
  { type: 'iri', value: 'http://www.w3.org/2000/01/rdf-schema#label' },
  { type: 'literal', value: 'Person' }
);

// Update with conflict detection (N3 Patch)
await jssOntologyService.patchOntologyN3(`
  @prefix solid: <http://www.w3.org/ns/solid/terms#>.
  _:patch a solid:InsertDeletePatch;
    solid:where { <#Person> rdfs:label "Old Name" . } ;
    solid:deletes { <#Person> rdfs:label "Old Name" . } ;
    solid:inserts { <#Person> rdfs:label "New Name" . } .
`);
```

### 3. Type Index Discovery (ADR-029)

Discover shared views and agent capabilities across users:

```typescript
// Register your views in the public Type Index
await solidPodService.registerViewInTypeIndex('AI Research', viewUrl);

// Discover another user's shared views
const views = await solidPodService.discoverSharedViews('https://pod.example/alice/profile/card#me');

// Discover available agents
const agents = await solidPodService.discoverAgents('https://pod.example/bob/profile/card#me');
```

### 4. Agent Memory in Pods (ADR-030)

Auditable, user-controlled AI agent memory:

```typescript
// Store agent memory
await solidPodService.storeAgentMemory('coder-agent', {
  key: 'auth-pattern',
  value: 'JWT with refresh tokens works best for this codebase',
  namespace: 'patterns',
  tags: ['auth', 'jwt'],
});

// List agent memories
const memories = await solidPodService.listAgentMemories('coder-agent');

// Set WAC permissions (user controls agent access)
await solidPodService.setAgentMemoryAccess('coder-agent', {
  agentWebId: 'did:nostr:npub1agent...',
  modes: ['Read', 'Append'],  // Agent can read and add, but not delete
});

// Revoke agent access
await solidPodService.deleteAgentMemory('coder-agent', 'auth-pattern');
```

## Pod Structure

Each user's Pod follows this directory structure:

```
/{npub}/
  profile/
    card                           # WebID profile (JSON-LD)
  settings/
    graph-views/                   # Saved graph views
      ai-research.jsonld
      default.jsonld
    publicTypeIndex.jsonld         # Type Index for discovery
    preferences/                   # App preferences
  ontology_contributions/          # Proposed ontology changes
  ontology_proposals/              # Formal proposals
  ontology_annotations/            # Node/edge annotations
  agents/
    coder-agent/
      memory/                      # Agent memory entries
        auth-pattern.jsonld
        debug-strategy.jsonld
      .acl                         # WAC: agent Read+Append, user full Control
  inbox/                           # Linked Data Notifications
  .acl                             # Root access control
```

## API Reference

### Pod Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/solid/pods/init` | Create/initialize pod (requires NIP-98) |
| GET | `/api/solid/pods/check` | Check if pod exists |
| POST | `/api/solid/pods/init-nip98` | Init with fresh NIP-98 signature |
| GET | `/solid/health` | JSS health check |

### LDP Operations (via proxy)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/solid/{path}` | Read resource (JSON-LD or Turtle via Accept header) |
| PUT | `/solid/{path}` | Create or replace resource |
| POST | `/solid/{container}/` | Create resource in container (Slug header for name) |
| DELETE | `/solid/{path}` | Delete resource |
| PATCH | `/solid/{path}` | Partial update (SPARQL Update or N3 Patch) |
| HEAD | `/solid/{path}` | Metadata only |

### WebSocket Notifications

Connect to `/solid/.notifications` for real-time updates:

```
Protocol: solid-0.1
Subscribe:   sub {resourceUrl}
Unsubscribe: unsub {resourceUrl}
Notification: pub {resourceUrl}  (resource changed)
Acknowledge:  ack {resourceUrl}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `JSS_URL` | `http://jss:3030` | JSS internal URL |
| `JSS_WS_URL` | `ws://jss:3030/.notifications` | JSS WebSocket URL |
| `VITE_JSS_URL` | `/solid` | Client-side Solid proxy path |
| `SOLID_PROXY_SECRET_KEY` | (none) | Server-side signing fallback |

## Troubleshooting

### "Login Required" in Pod tab
You must be authenticated with Nostr. Install a NIP-07 browser extension (nos2x, Alby, or Nostr Connect) and log in via the System tab → Nostr Login.

### JSS shows "unhealthy"
JSS requires authentication for all requests including health checks. This is expected — the container shows "unhealthy" in Docker but the server is running. Check `/solid/health` with auth headers.

### Pod creation fails with 401
Ensure your Nostr session is active and the NIP-98 signature is valid. Try logging out and back in to refresh the signing keys.

## Credits

- **[JavaScriptSolidServer (JSS)](https://github.com/JavaScriptSolidServer/JavaScriptSolidServer)** by the JavaScriptSolidServer contributors — AGPL-3.0-only
- **[Solid Project](https://solidproject.org/)** — W3C Community Group specifications
- **[Nostr Protocol](https://nostr.com/)** — NIP-98 HTTP Auth, NIP-07 browser signing
- **VisionClaw Solid Integration** — Rust proxy, TypeScript client, Nostr↔WebID bridge

## Architecture Decision Records

- [ADR-027: Pod-backed Graph Views](../../adr/ADR-027-pod-backed-graph-views.md)
- [ADR-106: SPARQL PATCH for Ontology](../../adr/ADR-106-sparql-patch-ontology.md) (renumbered from ADR-028)
- [ADR-029: Type Index Discovery](../../adr/ADR-029-type-index-discovery.md)
- [ADR-030: Agent Memory in Pods](../../adr/ADR-030-agent-memory-pods.md)
