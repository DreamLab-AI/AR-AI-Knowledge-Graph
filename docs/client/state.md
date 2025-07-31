# State Management

This document details the state management patterns and mechanisms used throughout the client application. The application uses several complementary approaches to state management to handle different types of state.

## State Management Overview

The client application manages several types of state:

1.  **Application Settings** - User preferences and application configuration
2.  **Graph Data** - Nodes, edges, and metadata for the visualisation
3.  **UI State** - Control panel state, selected items, and UI configuration
4.  **Rendering State** - Camera position, visibility settings, and rendering options
5.  **XR State** - XR session status, controller positions, and interaction state

```mermaid
flowchart TB
    subgraph ApplicationState
        Settings[Settings Store]
        GraphData[Graph Data]
        UIState[UI State]
        RenderState[Rendering State]
        XRState[XR State]
    end

    subgraph StateConsumers
        GraphManager[Graph Manager]
        ControlPanel[Control Panel]
        XRComponents[XR Components]
        VisualisationComponents[Visualisation Components]
    end

    Settings --> VisualisationComponents
    Settings --> ControlPanel
    Settings --> XRComponents

    GraphData --> GraphManager
    GraphData --> VisualisationComponents

    UIState --> ControlPanel
    RenderState --> VisualisationComponents
    XRState --> XRComponents
```

## Key State Management Components

### Settings Store ([`client/src/store/settingsStore.ts`](../../client/src/store/settingsStore.ts))

It uses Zustand for state management, `persist` middleware for saving to local storage, and `immer`'s `produce` utility for safe, immutable updates.

**Key Features:**
- Persistence to local storage (and potentially server-side sync via `settingsService.ts`).
- Observable changes through Zustand's subscription mechanism.
- Default values loaded from [`client/src/features/settings/config/defaultSettings.ts`](../../client/src/features/settings/config/defaultSettings.ts).
- Uses `immer` middleware for convenient immutable updates.
- Employs a `deepMerge` utility ([`client/src/utils/deepMerge.ts`](../../client/src/utils/deepMerge.ts)) for merging settings updates.

**Implementation Pattern (Zustand with Immer):**
The `set` and `updateSettings` methods in the store handle immutable updates for nested properties. The `set` method uses a path-based string to update specific values, while `updateSettings` accepts an Immer-style producer function for more complex mutations.

**Settings Validation:**
Settings validation primarily relies on **TypeScript's static type checking** during development and the structure enforced by UI components ([`SettingControlComponent.tsx`](../../client/src/features/settings/components/SettingControlComponent.tsx) based on [`settingsUIDefinition.ts`](../../client/src/features/settings/config/settingsUIDefinition.ts)). There is **no explicit runtime validation using Zod schemas** (like from `client/src/features/settings/types/settingsSchema.ts`) directly within the `settingsStore`'s `set` or `updateSettings` methods. Input validation is expected to occur in the UI components before attempting to update the store.

### Graph Data Manager ([`client/src/features/graph/managers/graphDataManager.ts`](../../client/src/features/graph/managers/graphDataManager.ts))

The Graph Data Manager maintains the state of the graph visualisation data.

**Key Features:**
- Loads and processes graph data from server
- Manages node and edge collections
- Handles real-time position updates via binary protocol
- Provides subscription mechanism for changes

**State Transitions:**
```mermaid
stateDiagram-v2
    [*] --> Empty
    Empty --> Loading: fetchInitialData()
    Loading --> PartiallyLoaded: First page loaded
    PartiallyLoaded --> FullyLoaded: All pages loaded
    FullyLoaded --> LiveUpdates: WebSocket connected
    LiveUpdates --> FullyLoaded: WebSocket disconnected
    LiveUpdates --> LiveUpdates: Position update
    FullyLoaded --> Empty: clear()
    LiveUpdates --> Empty: clear()
```

### Settings Observer
The file `SettingsObserver.ts` is **not used** in the current architecture. Zustand itself provides the subscription mechanism. Components subscribe directly to `useSettingsStore` (often using selectors to pick specific parts of the state) to react to changes.

## State Persistence

The application persists state in several ways:

1.  **Local Storage** - User preferences and UI state (managed by Zustand's `persist` middleware).
2.  **Server Storage** - User settings synchronized to server (for authenticated users).
3.  **URL Parameters** - Shareable state in URL (not extensively used for persistence, more for initial configuration).

### Persistence Flow

```mermaid
flowchart TD
    StateChange[State Change] --> ValidState{Is Valid?}
    ValidState -->|Yes| LocalStorage[Store in Local Storage]
    ValidState -->|Yes| SyncToServer{Sync to Server?}
    ValidState -->|No| LogError[Log Error]

    SyncToServer -->|Yes| APICall[POST to API]
    SyncToServer -->|No| Complete[Complete]

    APICall --> ServerResponse{Success?}
    ServerResponse -->|Yes| Complete
    ServerResponse -->|No| RetryStrategy[Apply Retry Strategy]

    RetryStrategy --> APICall
```

## State Change Propagation

The application uses several mechanisms to propagate state changes:

### Event Emitter
A dedicated global event emitter (e.g., `client/utils/eventEmitter.ts`) is **not present** in the current codebase. Communication and event-like propagation are handled by:
- **Zustand store subscriptions**: For changes in global state like settings.
- **React Context API**: For more localized state or function sharing.
- **Callbacks and Props**: Standard React patterns for component communication.
- **WebSocketService event handlers**: For server-sent messages (e.g., `onMessage`, `onBinaryMessage`).
- `graphDataManager` might expose its own subscription mechanism for graph-specific updates (e.g., `onGraphDataChange`, `onPositionUpdate`).

### Direct Subscriptions

Components can subscribe directly to state stores.

**Example:**
```typescript
// Example: Subscribe to graph data changes from GraphDataManager
// (Assuming graphDataManager instance has an onGraphDataChange method)
const graphDataManager = GraphDataManager.getInstance(); // Or however it's accessed
const unsubscribeGraph = graphDataManager.onGraphDataChange((newGraphData) => {
  console.log('Graph data changed:', newGraphData);
  // Update component based on new graph data
});

// Example: Subscribe to settings changes from useSettingsStore
const unsubscribeSettings = useSettingsStore.subscribe(
  (newSettings) => {
    console.log('Settings changed (entire settings object):', newSettings);
    // Update component based on new settings
  },
  state => state.settings // Selector for the entire settings object
);

// Example: Subscribe to a specific setting value
const unsubscribeSpecificSetting = useSettingsStore.subscribe(
  (newNodeSize) => {
    console.log('Node size changed:', newNodeSize);
  },
  state => state.settings.visualisation.nodes.nodeSize // Selector for a specific value
);

// Remember to call unsubscribe functions on component unmount
// useEffect(() => {
//   return () => {
//     unsubscribeGraph();
//     unsubscribeSettings();
//     unsubscribeSpecificSetting();
//   };
// }, []);
```

## Settings Structure

### Multi-Graph Architecture

The application now supports multiple graph visualizations simultaneously, with each graph having its own independent visual settings. This allows for different visual themes and configurations for different data sources.

```typescript
// Simplified conceptual representation.
// For the complete and accurate structure, see:
// client/src/features/settings/config/settings.ts

interface Settings {
  visualisation: {
    // Graph-specific settings (NEW structure)
    graphs: {
      logseq: GraphSettings;      // Blue/purple theme for Logseq data
      visionflow: GraphSettings;   // Green theme for VisionFlow data
    };
    
    // Global visualization settings (shared across all graphs)
    rendering: RenderingSettings;
    animations: AnimationSettings;
    bloom: BloomSettings;
    hologram: HologramSettings;
    camera?: CameraSettings;
    
    // Legacy compatibility (deprecated)
    nodes?: NodeSettings;
    edges?: EdgeSettings;
    physics?: PhysicsSettings;
    labels?: LabelSettings;
  };
  system: {
    websocket: ClientWebSocketSettings;
    debug: DebugSettings;
    persistSettings: boolean;
  };
  xr: XRSettings;
  auth: AuthSettings;
  // Optional AI Service Settings
  ragflow?: RAGFlowSettings;
  perplexity?: PerplexitySettings;
  openai?: OpenAISettings;
  kokoro?: KokoroSettings;
}

// Graph-specific settings structure
interface GraphSettings {
  nodes: NodeSettings;     // Node appearance and behavior
  edges: EdgeSettings;     // Edge/link appearance
  labels: LabelSettings;   // Text label configuration
  physics: PhysicsSettings; // Physics simulation parameters
}
```

### Settings Migration

The application includes automatic migration from the legacy flat structure to the new multi-graph structure:

**Legacy Structure (before migration):**
```typescript
settings.visualisation.nodes.baseColor
settings.visualisation.edges.color
settings.visualisation.physics.enabled
settings.visualisation.labels.fontSize
```

**New Multi-Graph Structure (after migration):**
```typescript
settings.visualisation.graphs.logseq.nodes.baseColor
settings.visualisation.graphs.visionflow.nodes.baseColor
settings.visualisation.graphs.logseq.edges.color
settings.visualisation.graphs.visionflow.edges.color
```

The migration utility (`settingsMigration.ts`) handles:
- Automatic detection of legacy settings
- Migration of existing settings to the `logseq` graph
- Initialization of `visionflow` graph with default green theme
- Cleanup of deprecated fields
- Backward compatibility during transition

### Settings Validation
As mentioned earlier, settings validation primarily relies on TypeScript's static type checking and UI component constraints. There is no explicit Zod validation layer directly within the `settingsStore`'s update methods.

## State Immutability

The application uses immutability patterns to prevent unexpected state changes, primarily facilitated by the `immer` middleware in Zustand. This ensures that state updates always result in new state objects, simplifying change detection and preventing unintended side effects.

**Example of Immutable Update (handled by Immer in Zustand):**
```typescript
// When using Immer with Zustand, direct modification within the 'set' callback is safe:
set((state) => {
  state.user.name = "New Name"; // This is automatically converted to an immutable update
});

// Without Immer, you would manually create new objects:
set((state) => ({
  ...state,
  user: {
    ...state.user,
    name: "New Name"
  }
}));
```

## Related Documentation

- [Components](components.md) - Component relationships and dependencies
- [Architecture](architecture.md) - Overall system architecture
- [WebSocket Communication](websocket.md) - Real-time state updates