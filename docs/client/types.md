# Client TypeScript Types and Interfaces

This document summarizes key TypeScript types and interfaces used throughout the LogseqXR client application. It focuses on data structures related to graph management, settings, and core application entities.

## Core Graph Data Structures

These types define the fundamental elements of the knowledge graph.

### `Node`

Represents a node in the graph, typically corresponding to a file or a concept.

```typescript
// Defined in client/src/features/graph/workers/graph.worker.ts
export interface Node {
  id: string; // Unique identifier for the node (string representation of a u32 from Rust)
  label: string; // Display name of the node
  position: {
    x: number;
    y: number;
    z: number;
  };
  metadata?: Record<string, any>; // Arbitrary metadata (e.g., file type, size, custom tags)
}
```
Note: The `data: BinaryNodeData` field mentioned in previous docs, which mirrored the server-side `BinaryNodeData` (with mass, flags etc.), is not directly part of the primary client-side `Node` interface. The client-side `Node` directly holds `position`. The separate `BinaryNodeData` type below is specifically for WebSocket communication.

```typescript
// From client/src/types/binaryProtocol.ts
// This structure is for WebSocket binary messages, mirroring the server's WireNodeDataItem.
export interface BinaryNodeData {
  nodeId: number; // The numeric ID of the Node (u32 from Rust)
  position: { x: number; y: number; z: number };
  velocity: { x: number; y: number; z: number };
}
```

### `Edge`

Represents a link or relationship between two nodes.

```typescript
// Defined in client/src/features/graph/workers/graph.worker.ts
export interface Edge {
  id: string; // Unique identifier for the edge (e.g., "sourceId_targetId")
  source: string; // ID of the source node
  target: string; // ID of the target node
  label?: string; // Optional display label for the edge
  weight?: number; // Strength or importance of the link
  metadata?: Record<string, any>; // Arbitrary metadata for the edge
  // Visual properties might be dynamically applied rather than stored directly on the edge object.
}
```

### `GraphData`

The primary container for all nodes and edges that constitute the graph.

```typescript
// Defined in client/src/features/graph/workers/graph.worker.ts
export interface GraphData {
  nodes: Node[];
  edges: Edge[];
  metadata?: Record<string, any>; // Global metadata about the graph (e.g., graph title, version)
}
```

## Settings Interfaces

These interfaces define the structure of the application settings, managed by `SettingsStore`.

### Main `Settings` Interface

The root interface for all application settings, located in `client/src/features/settings/config/settings.ts`.

```typescript
// The primary Settings interface with multi-graph support
export interface Settings {
  visualisation: VisualisationSettings;
  system: SystemSettings;
  xr: XRSettings;
  auth: AuthSettings;
  ragflow?: RAGFlowSettings;
  perplexity?: PerplexitySettings;
  openai?: OpenAISettings;
  kokoro?: KokoroSettings;
}

// Expanded VisualisationSettings with multi-graph structure
export interface VisualisationSettings {
  // NEW: Graph-specific settings namespace
  graphs: GraphsSettings;
  
  // Global visualization settings (shared across graphs)
  rendering: RenderingSettings;
  animations: AnimationSettings;
  bloom: BloomSettings;
  hologram: HologramSettings;
  camera?: CameraSettings;
  
  // DEPRECATED: Legacy flat structure (for backward compatibility)
  nodes?: NodeSettings;
  edges?: EdgeSettings;
  physics?: PhysicsSettings;
  labels?: LabelSettings;
}

// Multi-graph namespace structure
export interface GraphsSettings {
  logseq: GraphSettings;      // Settings for Logseq graph visualization
  visionflow: GraphSettings;   // Settings for VisionFlow graph visualization
}

// Graph-specific settings container
export interface GraphSettings {
  nodes: NodeSettings;         // Node appearance configuration
  edges: EdgeSettings;         // Edge/link appearance configuration
  labels: LabelSettings;       // Label display configuration
  physics: PhysicsSettings;    // Physics simulation parameters
}

// Example of accessing settings in the new structure:
// settings.visualisation.graphs.logseq.nodes.baseColor
// settings.visualisation.graphs.visionflow.edges.color
```

**Key Changes:**
1. **Multi-Graph Support**: Settings now support multiple graph visualizations simultaneously
2. **Nested Structure**: Graph-specific settings are nested under `visualisation.graphs`
3. **Legacy Compatibility**: Old flat structure remains for backward compatibility but is deprecated
4. **Theme Separation**: Each graph can have its own visual theme (e.g., blue for Logseq, green for VisionFlow)

**Important:** The above is a simplified representation. The definitive source for the `Settings` interface and all its nested types is [`client/src/features/settings/config/settings.ts`](../../client/src/features/settings/config/settings.ts). Please refer to this file for the complete and accurate structure.

## RAGFlow Specific Types

Types related to interactions with the RAGFlow API, defined in [`client/src/types/ragflowTypes.ts`](../../client/src/types/ragflowTypes.ts).

### `RagflowChatRequestPayload`

Defines the payload for sending a chat request to the RAGFlow service.
```typescript
// From client/src/types/ragflowTypes.ts
// This should align with the server's RagflowChatRequest model.
export interface RagflowChatRequestPayload {
  question: string;
  sessionId?: string; // Optional: for continuing a conversation
  stream?: boolean;   // Optional: to stream the response (default: false)
}
```

### `RagflowChatResponsePayload` (or similar name in `ragflowTypes.ts`)

Defines the expected structure of a response from the RAGFlow chat service.
```typescript
// From client/src/types/ragflowTypes.ts
// This should align with the server's RagflowChatResponse model.
export interface RagflowChatResponsePayload { // Name might vary slightly in the file
  answer: string;
  sessionId: string; // ID for the current conversation session
  // Note: The documentation mentioned 'conversation_id' but the plan specifies 'sessionId'.
  // Verify against ragflowTypes.ts and server model src/models/ragflow_chat.rs.
}
```

## Other Notable Types

### `BinaryProtocol` related types ([`client/src/types/binaryProtocol.ts`](../../client/src/types/binaryProtocol.ts))

This file contains types and constants related to the custom binary protocol used for WebSocket communication.
-   `BinaryNodeData`: As defined earlier (nodeId, position, velocity).
-   May include constants for message type identifiers if the binary protocol supports different message types, or for byte offsets/sizes.

### Feature-Specific Types

Many features have their own dedicated `types.ts` or `*.types.ts` files.

-   **Settings UI Types**:
    -   The primary types driving the current settings UI are `UISettingDefinition` and related types from [`client/src/features/settings/config/settingsUIDefinition.ts`](../../client/src/features/settings/config/settingsUIDefinition.ts).

-   **XR Types**:
    -   [`client/src/features/xr/types/xr.ts`](../../client/src/features/xr/types/xr.ts): Defines basic types specific to WebXR interactions, controller states, and hand tracking data. Note that the comprehensive XR settings used by the application are defined in the main `Settings` interface in [`client/src/features/settings/config/settings.ts`](../../client/src/features/settings/config/settings.ts).

-   **Visualisation Types**:
    -   The comprehensive and detailed visualisation settings are part of the main `Settings` interface in [`client/src/features/settings/config/settings.ts`](../../client/src/features/settings/config/settings.ts) under the `visualisation` key.

These feature-specific type files help in modularizing the codebase and ensuring type safety within their respective domains.

This document provides a high-level summary. For the most accurate and detailed definitions, always refer to the source TypeScript files linked.