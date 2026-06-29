# 05 — VisionClaw Extended Analytics Wire Types (V3, 52 B)

ADR-031 Analytics Extension. Protocol version byte = `3` (PROTOCOL_V3). All fields little-endian.

---

## A. Binary wire layout — 52-byte per-node record

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  V3 NODE RECORD  —  52 bytes, little-endian, one record per node            │
├────────┬──────┬──────┬──────────────────────────────────────────────────────┤
│ Offset │ Size │ Type │ Field                                                 │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @0    │  4 B │ u32  │ node_id  (bits 31=Agent, 30=Knowledge, 26-28=Ontology │
│        │      │      │          type, bits 0-25 = compact node id)           │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @4    │ 12 B │ f32× │ position  [x, y, z]                                  │
│        │      │    3 │                                                        │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @16   │ 12 B │ f32× │ velocity  [vx, vy, vz]                               │
│        │      │    3 │                                                        │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @28   │  4 B │ f32  │ sssp_distance  (INFINITY if no SSSP result)          │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @32   │  4 B │ i32  │ sssp_parent    (-1 if no SSSP result)                │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @36   │  4 B │ u32  │ cluster_id     (1-based; 0 = unclustered)            │
│        │      │      │  written by: ClusteringActor (K-means/DBSCAN)        │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @40   │  4 B │ f32  │ anomaly_score  (LOF / z-score, 0.0–1.0)             │
│        │      │      │  written by: AnomalyDetectionActor                   │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @44   │  4 B │ u32  │ community_id   (Louvain partition label)             │
│        │      │      │  written by: ClusteringActor (community detection)   │
├────────┼──────┼──────┼──────────────────────────────────────────────────────┤
│  @48   │  4 B │ f32  │ centrality     (PageRank, normalised)                │
│        │      │      │  written by: PageRankActor  [ADR-031 D2]             │
└────────┴──────┴──────┴──────────────────────────────────────────────────────┘
  Total = 52 B.  Preceded by a 1-byte protocol-version header (value = 3).
  V5 frames add an 8-byte broadcast-sequence prefix before the V3 node data.
```

```mermaid
classDiagram
    class WireNodeDataItemV3 {
        +u32  id         %% @0  bits31=Agent,30=Knowledge,26-28=OntologyType,0-25=compact_id
        +f32  position_x %% @4
        +f32  position_y %% @8
        +f32  position_z %% @12
        +f32  velocity_x %% @16
        +f32  velocity_y %% @20
        +f32  velocity_z %% @24
        +f32  sssp_distance  %% @28  INFINITY when absent
        +i32  sssp_parent    %% @32  -1 when absent
        +u32  cluster_id     %% @36  1-based; 0=unclustered
        +f32  anomaly_score  %% @40  0.0-1.0
        +u32  community_id   %% @44  Louvain label
        +f32  centrality     %% @48  PageRank normalised [ADR-031 D2]
    }
    note for WireNodeDataItemV3 "52 B total per node (server: WIRE_V3_ITEM_SIZE)\nClient: BINARY_NODE_SIZE_V3 = 52\nPreceded by 1-byte version=3 header"

    class NodeAnalytics {
        +u32  cluster_id
        +u32  community_id
        +f32  anomaly
        +f32  centrality
    }
    note for NodeAnalytics "crates/visionclaw-domain/src/analytics.rs\nShared consolidation store key: compact node_id\nsssp_distance is NOT a member — sourced separately"

    class BinaryNodeData_TS {
        +number  nodeId
        +Vec3    position
        +Vec3    velocity
        +number  ssspDistance
        +number  ssspParent
        +number  clusterId      %% optional
        +number  anomalyScore   %% optional
        +number  communityId    %% optional
        +number  centrality     %% optional [ADR-031 D2]
    }
    note for BinaryNodeData_TS "client/src/types/binaryProtocol.ts\nDecoded from 52 B record via parseBinaryNodeData()"

    WireNodeDataItemV3 --> NodeAnalytics : "analytics fields sourced from"
    WireNodeDataItemV3 --> BinaryNodeData_TS : "decoded into (client)"
```

---

## B. Analytics data-flow — GPU kernels to render channel

```mermaid
sequenceDiagram
    participant PR   as PageRankActor<br/>(GPU)
    participant CL   as ClusteringActor<br/>(GPU)
    participant AD   as AnomalyDetectionActor<br/>(GPU)
    participant MAP  as node_analytics<br/>Arc<RwLock<HashMap<u32,NodeAnalytics>>>
    participant CC   as ClientCoordinatorActor<br/>(READER — broadcast encoder, NOT a writer)
    participant ENC  as encode_node_data_extended_with_sssp()<br/>src/utils/binary_protocol.rs
    participant WS   as WebSocket<br/>(V5 frame)
    participant DEC  as parseBinaryNodeData()<br/>client/src/types/binaryProtocol.ts
    participant WRK  as graph.worker.ts<br/>binary-processor.ts
    participant GEM  as GemNodes.tsx<br/>(render channel)

    Note over PR,AD: GPU kernel results read back to CPU.<br/>WRITE OWNERSHIP: each analytics actor is the sole writer of its OWN<br/>field(s) into node_analytics. ClusteringActor owns cluster_id +<br/>community_id (ADR-031 D3 single writer, write_cluster_id_from_assignments);<br/>PageRankActor owns centrality; AnomalyDetectionActor owns anomaly.<br/>ClientCoordinatorActor does NOT write — it only reads at broadcast time.
    PR  ->> MAP: write entry.centrality = normalised_pagerank<br/>(sole writer of centrality, resets stale nodes to 0.0)
    CL  ->> MAP: write entry.cluster_id (1-based, 0=unclustered)<br/>write entry.community_id (Louvain)<br/>(sole writer of cluster_id + community_id)
    AD  ->> MAP: write entry.anomaly (LOF/z-score, 0.0–1.0)<br/>(sole writer of anomaly)

    Note over CC: Physics tick broadcast — CC READS only
    CC  ->> MAP: node_analytics.read().ok()<br/>(client_coordinator_actor.rs:813,954,1491 — reader)
    MAP -->> CC: Option<&HashMap<u32, NodeAnalytics>>
    CC  ->> ENC: encode_node_data_extended_with_sssp(<br/>  nodes, …, sssp_data=None, analytics_data)
    Note over ENC: Writes 52 B per node into Vec<u8>.<br/>Strips flag bits from node_id before<br/>HashMap lookup (base_id = id & NODE_ID_MASK).
    ENC -->> CC: Vec<u8> (V3 body)
    CC  ->> WS: [version=5][8B seq][V3 node data]<br/>(V5 frame = V3 + broadcast sequence prefix)

    WS  -->> DEC: ArrayBuffer (browser)
    Note over DEC: Reads version byte.<br/>V5→strip 8B seq→re-wrap as V3.<br/>stride = BINARY_NODE_SIZE_V3 = 52.<br/>Reads @36 clusterId, @40 anomalyScore,<br/>@44 communityId, @48 centrality.
    DEC -->> WRK: BinaryNodeData[] (all fields populated)
    WRK ->> WRK: analyticsBuffer[nodeIndex*5 + 0..4]<br/>= [clusterId, anomalyScore, communityId,<br/>   centrality, ssspDistance]
    Note over WRK: WORKER_ANALYTICS_STRIDE = 5 (Float32Array)
    WRK -->> GEM: analyticsBuffer (SharedArrayBuffer)

    Note over GEM: Per-frame render loop
    GEM ->> GEM: a = nodeIndex * ANALYTICS_STRIDE (=5)<br/>clusterId   = buf[a + 0]<br/>anomalyScore= buf[a + 1]<br/>communityId = buf[a + 2]<br/>centrality  = buf[a + 3]<br/>ssspDistance= buf[a + 4]
    GEM ->> GEM: colorScheme=='community' → hue from communityId<br/>colorScheme=='cluster'    → hue from clusterId<br/>colorScheme=='centrality' → blue-red ramp (PageRank)<br/>colorScheme=='sssp'       → distance ramp<br/>showAnomalies → red overlay (anomalyScore intensity)<br/>glow → emissiveIntensity from settings
```

---

## C. Encoder / decoder inventory

> **Resolved (task #101 T6 — shadow module deleted).** The live 52 B broadcast
> encoder is `src/utils/binary_protocol.rs::encode_node_data_extended_with_sssp`
> (reached via `encode_node_data_with_live_analytics` from the `/wss` path —
> `position_updates.rs`, `fastwebsockets_handler.rs`, `actor_messages.rs`,
> `client_coordinator_actor.rs`). The former crate copy at
> `crates/visionclaw-protocol/src/binary_protocol.rs` — a stale 48 B shadow
> (`WIRE_V3_ITEM_SIZE = 48`, no centrality@48 slot) with zero callers outside
> its own crate/tests — **has since been removed** (see the rationale note in
> `crates/visionclaw-protocol/src/lib.rs`), leaving a single source of truth.
> The `visionclaw-protocol` crate itself remains a live dependency:
> `src/utils/socket_flow_messages.rs` re-exports
> `visionclaw_protocol::socket_flow_messages`, and the crate's
> `BinaryV3Frame`/`NodeRow` 28 B frame (`protocol/v3_frame.rs`) is the
> `BroadcastActor` hot path (`src/actors/broadcast_actor.rs`). The 52 B encoder
> stays in the webxr crate because it depends on the webxr-local
> `BinaryNodeDataClient` type.

| File | Function / path | Role | Status |
|------|----------------|------|--------|
| `src/utils/binary_protocol.rs` | `encode_node_data_extended_with_sssp()` | **LIVE** — sole full-feature 52 B writer (main crate) | Active |
| `src/utils/binary_protocol.rs` | `encode_node_data_with_live_analytics()` | Thin wrapper → `encode_node_data_extended_with_sssp` | Active |
| `src/utils/binary_protocol.rs` | `encode_node_data_with_types()`, `encode_node_data_extended()`, `encode_node_data()`, `encode_node_data_with_flags()` | Wrappers with `None` analytics — all delegate to live writer | Active |
| `src/utils/binary_protocol.rs` | `encode_node_data_with_analytics`, `encode_node_data_with_all` | **REMOVED** (task #70 D8b) — confirmed absent from `src/` | Deleted |
| `crates/visionclaw-protocol/src/binary_protocol.rs` | entire 48 B shadow module (`encode_node_data_with_analytics`, `encode_node_data_with_all`, `encode_node_data_extended_with_sssp`, `decode_node_data_v3`) | Stale 48 B copy (no centrality@48) | **DELETED** (task #101 T6) — rationale recorded in `crates/visionclaw-protocol/src/lib.rs` |
| `crates/visionclaw-protocol/src/protocol/v3_frame.rs` | `BinaryV3Frame::encode_slice()` | 28 B frame (`V3_NODE_BYTES=28`) — `BroadcastActor` hot path | **LIVE** (separate broadcast channel; not the 52 B analytics path) |
| `src/utils/binary_protocol.rs:decode_node_data_v3()` | server-side decoder | Reads 52 B; centrality parsed but discarded (prefixed `_`) | Active |
| `client/src/types/binaryProtocol.ts:parseBinaryNodeData()` | client decoder | Reads 52 B; all nine fields including centrality@48 | Active |
| `client/src/app/AppInitializer.tsx` | debug size probe | Uses magic number `nodeSize = 26` for logging only | Debug-only stale value |
