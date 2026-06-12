# VisionClaw — Dual-Graph Population & WebSocket Handoff

> Static analysis: 2026-06-03 | Backend root: `src/` | Client root: `client/src/`
> Live verification: GET /api/graph/data → 10,676 nodes, 2,551 divergent (23.9%)

> **RESOLVED 2026-06-03 (QE-T1 SSOT collapse).** The two parallel classification
> authorities have been collapsed into ONE. `metadata["type"]` is now the single
> authoritative origin for population, exposed by the centralised helper
> `Node::population()` / `Node::population_type()` in
> `crates/visionclaw-domain/src/models/node.rs`. Every reader — the GPU disc
> projection (`force_compute_actor.rs`), both `GraphStateActor` classifiers, the
> server filter gate (`client_filter.rs`), the client visual-mode resolver
> (`useGraphVisualState.ts`) and the client filter (`useGraphFiltering.ts`) — now
> reads through this one authority; `node_type` / top-level `type` is demoted to
> non-classifying elevation scaffold (a legacy fallback only when `metadata["type"]`
> is absent). The premature fake-elevation writer in
> `ontology_enrichment_service.rs` no longer rewrites `node_type` to spoof ontology
> origin. With one authority feeding all readers, GPU disc placement and client
> geometry/colour can no longer disagree, so the ~23.5% "Z-spray" is eliminated.
> The forked-authority flows below are kept for historical context; the collapsed
> model is in §7.
>
> **UPDATED 2026-06-12.** The canonical GitHub sync path now classifies by
> `EntityKind` (`entity.kind.as_node_type()`), setting `node_type` and
> `metadata["type"]` together at write time; wikilink stubs no longer become
> knowledge-graph nodes (stub purge), and `owl_class` frontier stubs are
> legitimate ontology population (the elevation work queue, ADR-110). The node
> counts in the header (10,676 / 2,551 divergent) are a snapshot of the
> 2026-06-03 bug era — the current corpus is ~196 working pages, ~5.9k authored
> classes, ~9.8k frontier stubs.

---

## 1. Graph Data Load → GPU Upload → Classification

```mermaid
sequenceDiagram
    autonumber
    participant GH as GitHub Sync<br/>github_sync_service.rs
    participant LF as Local File Sync<br/>local_file_sync_service.rs
    participant KGP as KnowledgeGraphParser<br/>parsers/knowledge_graph_parser.rs
    participant OES as OntologyEnrichmentService<br/>ontology_enrichment_service.rs
    participant GSA as GraphStateActor<br/>actors/graph_state_actor.rs
    participant FCA as ForceComputeActor<br/>actors/gpu/force_compute_actor.rs

    note over GH: Canonical path (JSON-LD files)
    GH->>GH: parse_canonical_entity() →<br/>EntityKind = KgPage | OntologyClass | OntologyIndividual
    GH->>GH: build_node_from_entity():<br/>node_type = entity.kind.as_node_type()<br/>metadata["type"] = entity.kind.as_node_type()<br/>(SET TOGETHER — agree at write time)
    GH->>GH: ensure_stub_from_link() for wikilink targets:<br/>node_type = "linked_page"<br/>metadata["type"] = "linked_page"<br/>(SET TOGETHER — agree at write time)

    note over GH: Plain logseq path (no JSON-LD)
    GH->>KGP: process_plain_logseq_file() → KGP.parse()
    KGP->>KGP: create_page_node():<br/>node_type = "page" OR "ontology_node" (if owl_class_iri)<br/>metadata["type"] = "page" (HARDCODED — never "ontology_node")
    note right of KGP: DIVERGENCE SOURCE A<br/>node_type may be "ontology_node"<br/>but metadata["type"] stays "page"

    note over LF: Local file path
    LF->>KGP: parse(content, filename)
    KGP-->>LF: GraphData (page/linked_page nodes)
    LF->>OES: enrich_graph()
    OES->>OES: infer_class() → set owl_class_iri<br/>update_node_visuals_by_class():<br/>node_type = "ontology_node"<br/>(metadata["type"] NOT updated)
    note right of OES: DIVERGENCE SOURCE B<br/>node_type promoted to "ontology_node"<br/>metadata["type"] left as "linked_page"

    GH-->>GSA: nodes HashMap (merged canonical + stubs)
    LF-->>GSA: GraphData (enriched nodes)

    GSA->>GSA: remap_to_compact_ids()<br/>node.id → 0..N-1 sequential<br/>compact_to_persistent[] = original ids
    note over GSA: ID MAPPING 1: persistent store id → compact id

    GSA->>GSA: reclassify_all_nodes():<br/>reads metadata["type"] FIRST, falls back to node_type<br/>→ fills knowledge_node_ids, ontology_class_ids,<br/>ontology_individual_ids, agent_node_ids

    GSA->>FCA: UploadGraphData { nodes, edges }

    FCA->>FCA: try_upload_pending_graph_data():<br/>node_indices HashMap = node.id → gpu_index<br/>gpu_index_to_node_id[gpu_index] = gpu_index (compact wire ID)
    note over FCA: ID MAPPING 2: gpu_index_to_node_id<br/>ALWAYS stores gpu_index as value<br/>(not persistent store id — see line 600)<br/>comment says "compact wire ID"

    FCA->>FCA: classify to GraphPopulation:<br/>reads metadata["type"] FIRST (authoritative)<br/>falls back to node_type<br/>→ node_population[gpu_index] = Knowledge|Ontology|Agent
    note over FCA: CLASSIFICATION:<br/>metadata["type"]="linked_page" → Knowledge disc (z=−sep)<br/>node_type="ontology_node" ignored by GPU population<br/>→ GPU disc assignment AGREES with metadata["type"]
```

---

## 2. Physics Loop → Disc Projection → Position Broadcast

```mermaid
sequenceDiagram
    autonumber
    participant FCA as ForceComputeActor<br/>actors/gpu/force_compute_actor.rs
    participant CUD as CUDA Kernel<br/>unified_gpu_compute/
    participant GSA as GraphStateActor<br/>actors/graph_state_actor.rs
    participant CCA as ClientCoordinatorActor<br/>actors/client_coordinator_actor.rs
    participant BP as BinaryProtocol<br/>utils/binary_protocol.rs

    loop Physics tick (≥ 2 ms interval)
        FCA->>CUD: step positions (force-directed layout)
        CUD-->>FCA: position_velocity_buffer[] (raw xyz per gpu_index)

        FCA->>FCA: Divergence guard: clamp infinite positions<br/>restore from last_good_positions if needed

        alt disc projection enabled (sep > 0 || flatten > 0)
            FCA->>FCA: population_centroids_xy():<br/>median (x,y) per population bucket [K,O,A]
            FCA->>FCA: per gpu_index: project_node_xy():<br/>Knowledge → z = −sep<br/>Ontology  → z = +sep<br/>Agent     → z = 0
            note right of FCA: Projection reads node_population[gpu_index]<br/>which was classified from metadata["type"]<br/>→ 2551 nodes land on KNOWLEDGE disc (z=−sep)<br/>despite node_type = "ontology_node"/"owl_class"
        end

        FCA->>FCA: build node_updates: gpu_index → BinaryNodeDataClient<br/>node_id = gpu_index_to_node_id[i] (compact wire id)<br/>xyz = projected positions
        FCA->>GSA: UpdateNodePositions { positions: Vec<(u32, BinaryNodeData)> }

        GSA->>CCA: BroadcastPositions (via graph_service_addr)

        CCA->>BP: encode_node_data_extended_with_sssp():<br/>for each node_id → check NodeTypeArrays:<br/>  agent_ids → AGENT_NODE_FLAG (bit 31)<br/>  knowledge_ids → KNOWLEDGE_NODE_FLAG (bit 30)<br/>  else (ontology) → ONTOLOGY_*_FLAG (bits 26-28)
        note right of BP: NodeTypeArrays populated from GraphStateActor<br/>which also reads metadata["type"] first<br/>→ type flags AGREE with disc placement<br/>→ both wrong about the 2551 divergent nodes

        BP-->>CCA: binary frame: [node_id_with_flags u32][x f32][y f32][z f32][vx][vy][vz]
        CCA-->>CCA: broadcast_with_filter() to all registered clients
    end
```

---

## 3. Client WebSocket Connect → Initial Load → Render

```mermaid
sequenceDiagram
    autonumber
    participant CLI as React Client
    participant WSH as SocketFlowServer<br/>socket_flow_handler/
    participant GSA as GraphStateActor
    participant CCA as ClientCoordinatorActor
    participant GMN as GemNodes.tsx
    participant GVS as useGraphVisualState.ts

    CLI->>WSH: WebSocket connect (ws://host/ws)
    WSH->>WSH: send_full_state_sync()

    WSH->>GSA: GetGraphData
    GSA-->>WSH: Arc<GraphData> (10,676 nodes)

    WSH->>WSH: Sort nodes by quality_score DESC<br/>take first 200 (DEFAULT_INITIAL_NODE_LIMIT)

    WSH->>CLI: JSON: "initialGraphLoad" {<br/>  nodes: [{id, label, x,y,z, node_type, metadata, owl_class_iri}],<br/>  edges: [...]<br/>}
    note right of WSH: Wire carries BOTH:<br/>top-level "type" = node.node_type (serde rename)<br/>metadata["type"] = node.metadata.get("type")<br/>These DISAGREE for 2551 nodes

    WSH->>GSA: GetNodeTypeArrays
    GSA-->>WSH: NodeTypeArrays {knowledge_ids, agent_ids, ontology_*_ids}

    WSH->>CLI: Binary: position frame for 200 nodes<br/>(node_id with type flags | x | y | z | vx | vy | vz)

    CLI->>CLI: textMessageHandler.handleInitialGraphLoad():<br/>node.type = n.node_type ?? n.nodeType ?? n.type<br/>node.metadata = { ...n.metadata }
    note right of CLI: Client stores BOTH fields.<br/>node.type = top-level "type" (serde'd from node_type)<br/>node.metadata.type = metadata["type"]<br/>— they disagree for 2551 nodes

    CLI->>WSH: "requestInitialData" (full snapshot)
    WSH->>WSH: handle_request_initial_data()

    note over CLI,CCA: Client registers with ClientCoordinatorActor<br/>(RegisterClient message via send_full_state_sync)

    CCA-->>CLI: Binary position broadcasts (ongoing)

    CLI->>GVS: useGraphVisualState(): build perNodeVisualModeMap
    GVS->>GVS: Priority 1: binaryNodeTypeMap (from binary flag bits)<br/>Priority 2: node.type (TOP-LEVEL — node_type field)<br/>Priority 3: node.metadata.nodeType heuristics
    note right of GVS: Priority 2 reads TOP-LEVEL type<br/>→ 1871 nodes classed as "ontology" visual mode<br/>→ rendered with Ontology geometry/colour<br/>→ but GPU placed them on Knowledge disc (z=−sep)

    CLI->>GMN: GemNodes.tsx: per-node colour
    GMN->>GMN: colorScheme='type': reads node.metadata?.type<br/>colorScheme='community': reads communityId (analytics)
    GMN->>GMN: isClass = node.metadata?.type === 'owl_class'<br/>(reads METADATA type, not top-level)
    note right of GMN: THIRD reader.<br/>colour branch reads metadata.type<br/>visual-mode branch reads top-level type<br/>→ colour and visual-mode can DISAGREE for same node
```

---

## 4. Single-Source-of-Truth Violation Map

```mermaid
flowchart TD
    subgraph WRITE["Write Sites — where node_type and metadata.type are set"]
        W1["KnowledgeGraphParser.create_page_node()<br/>parsers/knowledge_graph_parser.rs:109,152<br/>node_type = 'page'|'ontology_node'<br/>metadata['type'] = 'page' only (NEVER ontology_node)"]
        W2["KnowledgeGraphParser.extract_links() / stub nodes<br/>parsers/knowledge_graph_parser.rs:258,279<br/>node_type = 'linked_page'<br/>metadata['type'] = 'linked_page' — AGREE"]
        W3["build_node_from_entity() canonical path<br/>github_sync_service.rs:122,130<br/>node_type = entity.kind.as_node_type()<br/>metadata['type'] = entity.kind.as_node_type() — AGREE"]
        W4["ensure_stub_from_link() / ensure_stub_from_iri()<br/>github_sync_service.rs:190-191, 223-224<br/>node_type = 'linked_page'|'owl_class'<br/>metadata['type'] = same — AGREE"]
        W5["OntologyEnrichmentService.update_node_visuals_by_class()<br/>ontology_enrichment_service.rs:240<br/>node_type = 'ontology_node'<br/>metadata['type'] NOT UPDATED — DIVERGES"]
        W6["FileService.load_graph_data_with_metadata()<br/>file_service.rs:1194,1198<br/>node_type = 'ontology_node'|'page'<br/>metadata['type'] NOT SET HERE"]
        W7["OxigraphGraphRepository read path<br/>adapters/oxigraph_graph_repository.rs:1315-1323<br/>node_type = from vc:nodeType triple<br/>metadata = from vc:meta key=val pairs (independent)"]
    end

    subgraph READ["Read Sites — who reads which field"]
        R1["ForceComputeActor.try_upload_pending_graph_data()<br/>force_compute_actor.rs:607-608<br/>READS: metadata['type'] first, falls back to node_type<br/>OUTPUT: node_population[gpu_index] = K|O|A<br/>→ disc Z assignment"]
        R2["GraphStateActor.classify_node() / reclassify_all_nodes()<br/>graph_state_actor.rs:239-240, 284<br/>READS: metadata['type'] first, falls back to node_type<br/>OUTPUT: knowledge_node_ids, ontology_class_ids, ...<br/>→ binary protocol type flags"]
        R3["client_filter.rs:44<br/>READS: node_type (top-level only)<br/>OUTPUT: linked_page visibility gate<br/>ANOMALY: if node_type='linked_page' but meta='ontology_node',<br/>the gate hides it (wrong direction rarely occurs)"]
        R4["useGraphVisualState.ts:146-151<br/>READS: node.type (top-level, from node_type field)<br/>OUTPUT: perNodeVisualModeMap (knowledge|ontology|agent)<br/>→ Three.js geometry tier selection"]
        R5["GemNodes.tsx:462,473<br/>colorScheme='type': reads node.metadata?.type<br/>isClass check: reads node.metadata?.type<br/>OUTPUT: RGB colour"]
        R6["api_handler/graph/mod.rs:163-172<br/>READS: node_type (top-level) + metadata key presence<br/>OUTPUT: graph_type filter (knowledge|ontology|agent)<br/>for GET /api/graph/data?graph_type=X"]
        R7["useGraphFiltering.ts:101<br/>READS: nodeType (top-level field mapped as node.type)<br/>OUTPUT: linked_page visibility gate (client side)"]
    end

    W1 -- "DIVERGENCE: owl:class pages get<br/>node_type=ontology_node but meta stays page" --> R1
    W5 -- "DIVERGENCE: enriched stubs get<br/>node_type=ontology_node but meta stays linked_page" --> R1
    W3 --> R1
    W4 --> R1
    W1 -- same divergence" --> R2
    W5 -- "same divergence" --> R2
    R2 -- "NodeTypeArrays used for<br/>binary wire flags" --> R4
    R4 -- "visual mode = 'ontology'<br/>for 2551 nodes on Knowledge disc" --> CONFLICT

    R1 -- "disc = Knowledge z=−sep<br/>for 2551 divergent nodes" --> CONFLICT

    R5 -- "colour from metadata.type='linked_page'<br/>→ blue (#4FC3F7) not amber/violet" --> CONFLICT

    CONFLICT["VISIBLE ANOMALY<br/>2551 nodes:<br/>• GPU disc: Knowledge (z=−sep)<br/>• Visual mode: 'ontology' geometry<br/>• Colour (type scheme): blue (linked_page)<br/>• Binary flag: ONTOLOGY_*_FLAG<br/>→ Z-spray: Ontology geometry<br/>floating on Knowledge disc"]
```

---

## 5. Parallel Implementation Inventory

| Concern | Implementation 1 | Implementation 2 | Implementation 3 |
|---------|-----------------|-----------------|-----------------|
| **Population classification** | `ForceComputeActor::try_upload_pending_graph_data` (line 607) | `GraphStateActor::classify_node` (line 239) | `GraphStateActor::reclassify_all_nodes` (line 284) |
| **"metadata.type first" policy** | `force_compute_actor.rs:607` | `graph_state_actor.rs:239-240` | `graph_state_actor.rs:284` — policy comment duplicated |
| **node_type write** | `knowledge_graph_parser.rs:152` | `ontology_enrichment_service.rs:240` | `github_sync_service.rs:122` + `file_service.rs:1194` |
| **metadata["type"] write** | `knowledge_graph_parser.rs:109,258` | `github_sync_service.rs:130,191,224` | NOT written by enrichment service or file_service |
| **ID mapping** | `gpu_index_to_node_id` (force_compute_actor) — gpu_index→compact_wire_id | `compact_to_persistent` (graph_state_actor) — compact→original_store_id | `node_indices` HashMap (force_compute, local) — node.id→gpu_index |
| **Position upload** | `try_upload_pending_graph_data` (initial) | `UpdateNodePositions` message handler | `send_full_state_sync` binary frame (types.rs:401) |
| **First broadcast path** | `send_full_state_sync` JSON InitialGraphLoad (200 nodes) | `send_full_state_sync` binary position frame (same 200) | GPU tick → `UpdateNodePositions` → `BroadcastPositions` |
| **linked_page visibility gate** | `client_filter.rs:44` (server, reads `node_type`) | `useGraphFiltering.ts:101` (client, reads mapped `node.type`) | `bots_visualization_handler.rs:377` (reads `node_type`) |
| **Ontology filter** | `api_handler/graph/mod.rs:167-168` (reads `node_type` OR `metadata.contains_key("owl_class_iri")`) | `useGraphVisualState.ts:147` (reads top-level `node.type`) | Binary flag path via `NodeTypeArrays` |

---

## 6. Root Cause: The Divergence Mechanism

```mermaid
flowchart LR
    subgraph INGEST["Ingest Pipeline"]
        P1["KnowledgeGraphParser\nparses logseq page"]
        P2["Sees 'owl:class:: urn:...' in content"]
        P3["Sets node_type = 'ontology_node'\nSets metadata['type'] = 'page'\n(create_page_node: meta hardcoded to 'page')"]
        P4["OR: ensure_stub_from_link/iri\nsets both = 'linked_page' (AGREE)"]
        P5["OntologyEnrichmentService\nfinds owl_class_iri match"]
        P6["Sets node_type = 'ontology_node'\nDoes NOT set metadata['type']\n(update_node_visuals_by_class: only node_type)"]
        P1-->P2-->P3
        P1-->P4
        P3-->P5-->P6
    end

    subgraph RESULT["Result in wire / GPU"]
        R1["1871 nodes: meta='linked_page', type='ontology_node'\n→ GPU: Knowledge disc (z=−sep)\n→ Client visual-mode: ontology geometry\n→ Client colour: blue not amber"]
        R2["628 nodes: meta='linked_page', type='owl_class'\n→ GPU: Knowledge disc (z=−sep)\n→ Client visual-mode: ontology geometry\n→ Client isClass check: false (meta says linked_page)"]
        R3["27 nodes: meta='linked_page', type='page'\n→ GPU: Knowledge disc (metadata wins)\n→ Client visual-mode: knowledge_graph (top-level wins)\n→ AGREE on disc, agree on visual mode"]
    end

    P3-->R1
    P6-->R1
    P4-->R2
```

### Authoritative field decision

The backend comments at `force_compute_actor.rs:603` and `graph_state_actor.rs:236` explicitly declare `metadata["type"]` as **authoritative** (ABox/TBox origin truth). The GPU disc assignment and NodeTypeArrays both honour this. The client's `useGraphVisualState.ts` priority-2 path reads the **top-level `type`** field (which serde renames from `node_type`), contradicting the declared authority.

**The fix** (applied 2026-06-03) took option 3, generalised into a single source of
truth: every reader now classifies through `Node::population()`, which reads the
authoritative `metadata["type"]`. The client `useGraphVisualState.ts` and
`useGraphFiltering.ts` read `node.metadata?.type` first (mirroring the server's
`population_type` legacy fallback to top-level `type`); the server
`client_filter.rs` reads `Node::population_type()`; the GPU maps `Node::population()`
into its local `GraphPopulation`. The fake-elevation writer in
`ontology_enrichment_service.rs:240` no longer sets `node_type` — a node's ontology
signal travels in `owl_class_iri`, and origin migration is left to the future
elevation process (which alone may rewrite `metadata["type"]`).

---

## 7. Resolved Model — Single `metadata.type` Authority (2026-06-03)

```mermaid
flowchart TD
    SSOT["SINGLE SOURCE OF TRUTH<br/>Node::population() / Node::population_type()<br/>crates/visionclaw-domain/src/models/node.rs<br/>reads metadata['type'] (authoritative origin)<br/>node_type = legacy fallback ONLY when meta absent"]

    subgraph SCAFFOLD["Non-classifying scaffold (cannot flip origin)"]
        NT["node_type / top-level 'type'<br/>reserved for future elevation process<br/>(forum → brokers → agent panels)<br/>which alone rewrites metadata['type']"]
        ENR["OntologyEnrichmentService<br/>ontology_enrichment_service.rs<br/>sets owl_class_iri + visuals only<br/>NO node_type / metadata['type'] rewrite"]
    end

    subgraph READERS["All readers — one authority"]
        G1["ForceComputeActor<br/>force_compute_actor.rs<br/>GraphPopulation::from(node.population())<br/>→ disc Z assignment"]
        G2["GraphStateActor.classify_node /<br/>reclassify_all_nodes<br/>node.population_type()<br/>→ NodeTypeArrays / binary flags"]
        G3["client_filter.rs<br/>node.population_type()<br/>→ server linked_page gate"]
        C1["useGraphVisualState.ts<br/>node.metadata?.type<br/>→ perNodeVisualModeMap (geometry)"]
        C2["useGraphFiltering.ts<br/>node.metadata?.type<br/>→ client linked_page gate"]
        C3["GemNodes.tsx<br/>node.metadata?.type<br/>→ colour (already correct)"]
    end

    SSOT --> G1
    SSOT --> G2
    SSOT --> G3
    SSOT --> C1
    SSOT --> C2
    SSOT --> C3
    NT -. "fallback only<br/>when metadata['type'] absent" .-> SSOT
    ENR -. "owl_class_iri secondary signal<br/>(unknown-origin arm)" .-> SSOT

    G1 --> AGREE
    C1 --> AGREE
    C3 --> AGREE
    AGREE["NO SPRAY<br/>GPU disc, client geometry and colour<br/>all derive from one metadata['type'] origin<br/>→ they cannot disagree"]
```

---

## References

| File | Lines | Concern |
|------|-------|---------|
| `src/actors/gpu/force_compute_actor.rs` | 24-113 | GraphPopulation enum, centroid/projection helpers |
| `src/actors/gpu/force_compute_actor.rs` | 595-633 | GPU upload + population classification |
| `src/actors/gpu/force_compute_actor.rs` | 1810-1831 | Disc projection before broadcast |
| `src/actors/graph_state_actor.rs` | 233-325 | classify_node / reclassify_all_nodes (dual impl) |
| `src/actors/graph_state_actor.rs` | 170-178 | get_node_type_arrays → binary protocol flags |
| `src/handlers/socket_flow_handler/types.rs` | 276-434 | send_full_state_sync (JSON + binary initial load) |
| `src/handlers/api_handler/graph/mod.rs` | 31-58, 130-177 | NodeWithPosition wire struct + graph_type filter |
| `crates/visionclaw-protocol/src/socket_flow_messages.rs` | 180-199 | InitialNodeData (carries both node_type and metadata) |
| `src/services/parsers/knowledge_graph_parser.rs` | 107-152, 257-293 | Divergence source A (page vs ontology_node + meta) |
| `src/services/ontology_enrichment_service.rs` | 220-241 | Divergence source B (node_type only, no meta update) |
| `src/services/github_sync_service.rs` | 111-196 | Canonical path (agree) + stubs (agree) |
| `src/utils/binary_protocol.rs` | 16-27, 137-225 | Flag constants and set_*_flag helpers |
| `src/actors/client_coordinator_actor.rs` | 343-413 | Binary broadcast with NodeTypeArrays flags |
| `client/src/store/websocket/textMessageHandler.ts` | 70-148 | Client initial graph load decode |
| `client/src/features/graph/hooks/useGraphVisualState.ts` | 135-179 | Top-level type for visual-mode (Priority 2) |
| `client/src/features/graph/components/GemNodes.tsx` | 456-478 | metadata.type for colour (different field from above) |
| `client/src/features/graph/hooks/useGraphNodeColors.ts` | 100-121 | TYPE_THREE_COLORS palette |
| `client/src/features/graph/hooks/useGraphFiltering.ts` | 93-101 | linked_page visibility gate (client) |
| `src/actors/client_filter.rs` | 22-44 | linked_page visibility gate (server) |
| `crates/visionclaw-domain/src/models/canonical_entity.rs` | 24-44 | EntityKind::as_node_type() mapping |
