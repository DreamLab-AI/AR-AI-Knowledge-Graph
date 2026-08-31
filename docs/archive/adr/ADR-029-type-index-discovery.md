# ADR-029: Type Index for Agent and View Discovery

## Status
Implemented 2026-04-20

Implementation notes:
- `src/services/type_index_discovery.rs` — parse/serialise + `ensure_public_type_index`, `register_agent_in_type_index`, `discover_peer_registrations`. Recognised class URIs: `urn:solid:AgentSkill`, `urn:solid:ContributorProfile` (open to extension).
- `src/actors/dojo_discovery_actor.rs` — supervised read-side crawler with stub periodic scheduler; fans out `PeerRegistrationsDiscovered` events for `SkillRegistrySupervisor` (agent C2) to consume.
- `src/services/pod_client.rs` — added `get_resource` helper for profile + Type Index reads.
- Registration payload is JSON-LD with `solid:TypeIndex` / `solid:TypeRegistration` matching the structure below; registrations can point at a `solid:instance` container OR carry inline agent metadata (capabilities, label) via extension fields, which this module round-trips verbatim.
- WAC enforcement already landed under ADR-052; unauthorised peer fetches silently yield empty results (best-effort discovery per §Mitigations).
- Follow-ups: (C2) wire `PeerRegistrationsDiscovered` into `SkillIndex` read model + calibrate crawl scheduler; (X1) expose `dojo_refresh` MCP tool mapping to `CrawlNow`.

## Context
VisionClaw users can save graph views to their Solid Pods (ADR-027) and interact with agents, but there is no standardized way for users to discover what views or agents another user has made available. Without a discovery mechanism, collaboration requires out-of-band URL sharing.

The Solid specification defines a Type Index -- a well-known document linked from a user's WebID profile that registers RDF types and their storage locations. Applications use Type Indexes to advertise and discover data without crawling entire Pods.

VisionClaw already has `SolidPodService.ts` with full LDP CRUD, WebSocket notifications, and Nostr-based authentication. The JSS sidecar supports JSON-LD content negotiation.

## Decision
Implement a public Type Index at each user's Pod (`/settings/publicTypeIndex.jsonld`) that registers two resource types:

1. **`schema:ViewAction`** -- graph views the user has published for sharing
2. **`vf:Agent`** -- agent capabilities the user exposes for collaboration

### Type Index Document Structure (JSON-LD)
```json
{
  "@context": {
    "solid": "http://www.w3.org/ns/solid/terms#",
    "schema": "https://schema.org/",
    "vf": "https://narrativegoldmine.com/ontology#"
  },
  "@type": "solid:TypeIndex",
  "solid:typeRegistration": [
    {
      "@type": "solid:TypeRegistration",
      "solid:forClass": "schema:ViewAction",
      "solid:instance": "/pods/abc123/settings/graph-views/ai-research.jsonld",
      "vf:label": "AI Research",
      "vf:registeredAt": "2026-04-03T10:00:00Z"
    },
    {
      "@type": "solid:TypeRegistration",
      "solid:forClass": "vf:Agent",
      "vf:agentId": "code-reviewer-v2",
      "vf:capabilities": ["code-review", "security-audit", "performance-analysis"],
      "vf:registeredAt": "2026-04-03T10:00:00Z"
    }
  ]
}
```

### API Surface (SolidPodService)

| Method | Purpose |
|--------|---------|
| `ensurePublicTypeIndex()` | Get or create the Type Index document, link from WebID profile |
| `registerViewInTypeIndex(name, url)` | Add a view registration (idempotent by URL) |
| `registerAgentInTypeIndex(id, capabilities)` | Add or update an agent registration (upsert by agent ID) |
| `discoverSharedViews(webId)` | Fetch a remote user's Type Index and extract view registrations |
| `discoverAgents(webId)` | Fetch a remote user's Type Index and extract agent registrations |

### Discovery Flow

1. User A saves a graph view and calls `registerViewInTypeIndex("AI Research", viewUrl)`
2. User B knows User A's WebID (from the graph, from Nostr contacts, etc.)
3. User B calls `discoverSharedViews(userAWebId)`
4. Service fetches User A's WebID profile, resolves `solid:publicTypeIndex`, fetches it
5. Service filters registrations for `schema:ViewAction`, returns `{name, url}[]`
6. User B can then `loadGraphView()` using the discovered URL (subject to WAC)

### Profile Linking

The Type Index is linked from the user's WebID profile document via:
```json
{
  "solid:publicTypeIndex": { "@id": "/settings/publicTypeIndex.jsonld" }
}
```

This linking is performed automatically by `ensurePublicTypeIndex()` on first creation.

### Why Public Type Index (not Private)?

- Graph views and agent capabilities are inherently social -- their purpose is to be discovered
- Actual access control is enforced by WAC on the view resources themselves, not on the index
- A user may list a view in the Type Index that requires ACL approval to read
- Private Type Index would defeat the discovery purpose

## Consequences

### Positive
- Users can discover each other's shared views without exchanging URLs manually
- Agent capabilities become discoverable, enabling automated agent-to-agent collaboration
- Follows the Solid Type Index specification -- interoperable with other Solid apps
- Idempotent registration prevents duplicate entries
- Graceful degradation: discovery returns empty arrays if remote Pod is unavailable

### Negative
- Public Type Index reveals what types of data a user stores (view names, agent IDs) to anyone who can fetch their WebID profile
- Additional network requests for cross-user discovery (fetch WebID profile + Type Index)
- Requires the remote user's JSS to be reachable from the client (CORS, network)

### Mitigations
- Users choose what to register; no automatic registration without explicit API call
- View labels can be generic (users control what metadata is exposed)
- Agent capability lists are coarse-grained (no sensitive config exposed)
- Failed discovery silently returns empty results (no error propagation to UI)
