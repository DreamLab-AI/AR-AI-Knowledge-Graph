# ADR-027: Pod-backed Graph Views

## Status

Implemented 2026-04-20 — `SolidPodService.saveGraphView/loadGraphView/listGraphViews/deleteGraphView/subscribeToGraphViewChanges` already present from ADR-027 design; `shareGraphView` added in this wave.
Proposed

## Context
VisionClaw users configure graph views (camera position, active filters, physics parameters, cluster settings, node visibility) through the control panel. These configurations are volatile — lost on page reload unless manually persisted to Neo4j (shared, not per-user) or localStorage (device-bound, not portable).

The JSS sidecar already supports per-user Pods with LDP CRUD, WebSocket notifications, and WAC access control. SolidPodService.ts has `setPreference()`/`getPreference()` for basic key-value storage.

## Decision
Store named graph views as JSON-LD documents in each user's Pod at `/settings/graph-views/{view-name}.jsonld`. Use WebSocket notifications (solid-0.1 protocol) for cross-device sync. Extend CommandInput with "save view" / "load view" NL commands. Use WAC `.acl` files for sharing views with other users.

### Data Model (JSON-LD)
```json
{
  "@context": "https://schema.org",
  "@type": "ViewAction",
  "@id": "#my-ai-research-view",
  "name": "AI Research",
  "dateCreated": "2026-04-03T10:00:00Z",
  "camera": { "x": 10, "y": 20, "z": 50, "fov": 75 },
  "filters": { "nodeTypes": ["knowledge", "ontology"], "qualityThreshold": 0.7 },
  "physics": { "repelK": 5000, "springK": 100, "restLength": 300 },
  "clusters": { "algorithm": "louvain", "count": 8, "showHulls": true },
  "pinnedNodes": [42, 137, 501]
}
```

### Architecture
- **Client**: `SolidPodService.saveGraphView(name, viewData)` → PUT to Pod
- **Client**: `SolidPodService.loadGraphView(name)` → GET from Pod
- **Client**: `SolidPodService.listGraphViews()` → GET container listing
- **Client**: `SolidPodService.shareGraphView(name, targetWebId)` → PUT ACL
- **Sync**: WebSocket subscription to `/settings/graph-views/` container
- **NL**: CommandInput parses "save view [name]" → `saveGraphView(name, currentState)`

### Why not localStorage or Neo4j?
- localStorage: device-bound, no sharing, no audit trail
- Neo4j: shared database, no per-user isolation, no WAC
- Pod: user-owned, cross-device, shareable, auditable, standards-based

## Consequences
- **Positive**: Users own their view data, cross-device sync, shareable views
- **Positive**: No server-side schema changes (LDP handles storage)
- **Negative**: Requires JSS sidecar running (graceful degradation to localStorage)
- **Negative**: Additional WebSocket connection to JSS for sync

## Related Decisions

- ADR-048: Dual-tier identity model — extends the Pod-backed graph view with a KGNode / OntologyClass split
