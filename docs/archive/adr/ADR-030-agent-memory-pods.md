# ADR-030: Agent Memory in Solid Pods

**Status**: Accepted
**Date**: 2026-04-03
**Deciders**: VisionClaw team

## Context

VisionClaw agents store memory patterns in RuVector (PostgreSQL with HNSW vector search). This works well for agent-side retrieval and semantic search, but users have no visibility into what agents remember about them or their sessions. The Solid Pod sidecar (JSS) already provides per-user LDP containers with WAC access control (see ADR-027 for graph views). We can extend this to sync agent memory entries into user Pods, giving users the ability to inspect, audit, and revoke agent decisions.

## Decision

Add agent memory CRUD and WAC methods to `SolidPodService.ts`. Each agent gets an isolated container inside the user's Pod at `/agents/{agentId}/memory/`. Memory entries are stored as JSON-LD documents following schema.org vocabulary.

### Pod Layout

```
/pods/{npub}/
  agents/
    {agentId}/
      memory/
        {key}.jsonld          # Individual memory entry
        .acl                  # WAC for this agent's memory container
      memory/.acl             # Container-level ACL
```

### JSON-LD Format

Each memory entry uses the `DigitalDocument` type from schema.org:

```json
{
  "@context": "https://schema.org",
  "@type": "DigitalDocument",
  "identifier": "pattern-auth-flow",
  "name": "Authentication Pattern",
  "text": "JWT with refresh tokens works best for...",
  "keywords": ["auth", "jwt", "pattern"],
  "dateCreated": "2026-04-03T10:00:00Z",
  "dateModified": "2026-04-03T10:00:00Z",
  "author": {"@id": "did:nostr:npub1..."},
  "additionalProperty": {
    "@type": "PropertyValue",
    "name": "namespace",
    "value": "patterns"
  }
}
```

### API Surface

Five new public methods on `SolidPodService`:

| Method | Purpose |
|--------|---------|
| `storeAgentMemory(agentId, entry)` | Write a memory entry to the Pod |
| `listAgentMemories(agentId)` | List all entries in an agent's memory container |
| `getAgentMemory(agentId, key)` | Retrieve a specific entry by key |
| `deleteAgentMemory(agentId, key)` | Remove an entry (user revocation) |
| `setAgentMemoryAccess(agentId, permissions)` | Configure WAC for an agent's container |

### Access Control

WAC ACL files control per-agent access:
- **Owner** (the user) always retains `Read`, `Write`, `Control`.
- **Agent** receives only the modes explicitly granted (`Read`, `Write`, `Append`).
- ACL is set at the container level with `acl:default` so it inherits to all resources within.

### Security Considerations

- Agent IDs and memory keys are sanitized to prevent path traversal (same pattern as `sanitizePreferenceKey`).
- Agents cannot escalate their own permissions; only the Pod owner can call `setAgentMemoryAccess`.
- `deleteAgentMemory` enables user-initiated revocation of any stored memory.
- All requests go through `fetchWithAuth` which signs with NIP-98.

## Consequences

### Positive
- Users gain full transparency into agent memory.
- Per-agent WAC isolation prevents cross-agent data leakage.
- JSON-LD format enables interoperability with other Solid apps and Linked Data tooling.
- Delete capability satisfies data sovereignty requirements.
- Consistent with existing Pod patterns (graph views, preferences, ontology contributions).

### Negative
- Listing memories requires N+1 fetches (container listing + one fetch per entry). This is acceptable for audit/inspection use cases, not for hot-path agent retrieval.
- RuVector remains the primary store for agent-side vector search; Pod storage is a sync target, not a replacement.
- Container creation adds latency on first write for each new agent.

### Neutral
- No changes to the RuVector pipeline. This is a complementary read-model for users.
- Future work may add WebSocket subscriptions for real-time memory change notifications (reusing the solid-0.1 protocol from ADR-027).

## Related Decisions

- ADR-048: Dual-tier identity model — uses agent memory pods for candidate scoring and detection signal persistence

## Alternatives Considered

1. **Direct SQL access to RuVector for users**: Rejected. Violates Pod data sovereignty and requires exposing database credentials.
2. **Batch sync via background job**: Considered for future optimization. Current approach of write-through on each store is simpler and gives immediate consistency.
3. **Storing vectors in Pod**: Rejected. HNSW embeddings are opaque blobs with no user-facing value. Only human-readable key/value/namespace fields are synced.
