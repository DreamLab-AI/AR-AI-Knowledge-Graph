# Server-Side Data Models

This document outlines the core data structures (models) used on the server-side of the LogseqXR application. These models define how data is structured, stored, and manipulated.

## Simulation Parameters (`SimulationParams`)

Defines parameters for the physics-based graph layout simulation.

### Core Structure (from [`src/models/simulation_params.rs`](../../src/models/simulation_params.rs))
```rust
// In src/models/simulation_params.rs
#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub struct SimulationParams {
    pub iterations: u32,
    pub spring_strength: f32,
    pub repulsion: f32,  // Note: field name is 'repulsion', not 'repulsion_strength'
    pub damping: f32,
    pub time_step: f32,
    pub max_repulsion_distance: f32,
    pub mass_scale: f32,
    pub boundary_damping: f32,
    pub enable_bounds: bool,
    pub viewport_bounds: f32,
    pub phase: SimulationPhase,
    pub mode: SimulationMode,
}

// Simulation phases for different computation strategies
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SimulationPhase {
    Initial,    // Heavy computation for initial layout
    Dynamic,    // Lighter computation for dynamic updates
    Finalize,   // Final positioning and cleanup
}

// Simulation modes for computation backend
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SimulationMode {
    Remote,  // GPU-accelerated remote computation (default)
    GPU,     // Local GPU computation (deprecated)
    Local,   // CPU-based computation (disabled)
}
```
Note: The actual field name in the struct is `repulsion`, not `repulsion_strength`. The `collision_radius` and `max_velocity` fields shown in the original documentation do not exist in the actual implementation.

### Usage
-   Configuring the physics engine for graph layout.
-   Allowing real-time adjustment of simulation behavior.
-   Defining boundary conditions for the simulation space.

## UI Settings (`UserSettings` and `UISettings`)

The server defines two main structures for managing UI-related settings:

1.  **`UserSettings`** (from [`src/models/user_settings.rs`](../../src/models/user_settings.rs)): This structure is user-specific and primarily stores a user's `pubkey` and their personalized `UISettings` (which itself contains `visualisation`, `system`, and `xr` settings relevant to the client). It's used for persisting individual user preferences.

    ```rust
    // In src/models/user_settings.rs
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UserSettings {
        pub pubkey: String,
        pub settings: UISettings, // The actual client-facing settings structure
        pub last_modified: i64, // Unix timestamp
    }
    ```

2.  **`UISettings`** (from [`src/config/mod.rs`](../../src/config/mod.rs)): This structure represents the actual set of UI configurations that are sent to the client (serialized as camelCase JSON). It's derived from the global `AppFullSettings` for public/default views or from a specific user's `UserSettings`.

    ```rust
    // In src/config/mod.rs
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct UISettings {
        pub visualisation: VisualisationSettings, // Multi-graph support
        pub system: UISystemSettings,             // Contains client-relevant parts of AppFullSettings.system
        pub xr: XRSettings,                       // Sourced from AppFullSettings.xr
        // Note: AuthSettings from AppFullSettings are used server-side; client gets tokens/features.
        // AI service configurations (like API keys) are NOT part of UISettings.
        // Client interacts with AI services via API endpoints; server uses ProtectedSettings for keys.
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct VisualisationSettings {
        // NEW: Multi-graph support with graph-specific settings
        pub graphs: GraphsSettings,
        
        // Global visualization settings (shared across graphs)
        pub rendering: RenderingSettings,
        pub animations: AnimationSettings,
        pub bloom: BloomSettings,
        pub hologram: HologramSettings,
        pub camera: Option<CameraSettings>,
        
        // DEPRECATED: Legacy flat structure (for backward compatibility)
        pub nodes: Option<NodeSettings>,
        pub edges: Option<EdgeSettings>,
        pub physics: Option<PhysicsSettings>,
        pub labels: Option<LabelSettings>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct GraphsSettings {
        pub logseq: GraphSettings,      // Blue/purple theme for Logseq graphs
        pub visionflow: GraphSettings,   // Green theme for VisionFlow graphs
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct GraphSettings {
        pub nodes: NodeSettings,
        pub edges: EdgeSettings,
        pub labels: LabelSettings,
        pub physics: PhysicsSettings,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct UISystemSettings {
        // Contains only the client-relevant parts of ServerSystemConfigFromFile
        pub websocket: ClientWebSocketSettings, // Derived from ServerFullWebSocketSettings
        pub debug: DebugSettings,               // Client-safe debug flags
        // persistSettings and customBackendUrl are client-side settings,
        // but their server-side counterparts might influence these.
    }
    ```
    
### Multi-Graph Architecture

The server now supports multiple graph visualizations with independent settings:

1. **Graph Namespaces**: Each graph type (`logseq`, `visionflow`) has its own namespace with complete visual settings
2. **Settings Migration**: The server handles automatic migration from the legacy flat structure to the multi-graph structure
3. **Backward Compatibility**: Legacy settings paths are preserved but marked as deprecated
4. **Theme Separation**: Each graph can maintain its own visual theme and physics parameters

**Migration Path Examples:**
- Legacy: `visualisation.nodes.baseColor` 
- New: `visualisation.graphs.logseq.nodes.baseColor`
- New: `visualisation.graphs.visionflow.nodes.baseColor`

The server automatically migrates user settings when they are loaded, ensuring a smooth transition to the multi-graph architecture.

### Persistence
-   **User-Specific Settings (`UserSettings`)**: Saved to individual YAML files (e.g., `/app/user_settings/<pubkey>.yaml`).
-   **Global/Default Settings (`AppFullSettings` from which `UISettings` can be derived)**: Saved in `settings.yaml`.

## Protected Settings (`ProtectedSettings`)

This structure holds sensitive server-side configurations that are not directly exposed to clients but are used internally by the server.

### Core Structure (from [`src/models/protected_settings.rs`](../../src/models/protected_settings.rs))
```rust
// In src/models/protected_settings.rs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedSettings {
    pub network: NetworkSettings,      // Contains bind_address, port, domain, TLS settings, rate limiting
    pub security: SecuritySettings,    // Contains allowed_origins, session_timeout, CSRF, audit logging
    pub websocket_server: WebSocketServerSettings, // Contains max_connections, max_message_size, url
    pub users: std::collections::HashMap<String, NostrUser>, // Keyed by Nostr pubkey (hex)
    pub default_api_keys: ApiKeys,     // Default API keys for services if no user-specific key
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NostrUser {
    pub pubkey: String,              // Hex public key
    pub npub: String,                // Nostr npub format (not optional)
    pub is_power_user: bool,
    pub api_keys: ApiKeys,           // User-specific API keys (not optional)
    pub last_seen: i64,              // Unix timestamp of last activity
    pub session_token: Option<String>, // Session token for authentication
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeys {
    pub perplexity: Option<String>,
    pub openai: Option<String>,
    pub ragflow: Option<String>,
    // Potentially other AI service keys
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSettings {
    pub bind_address: String,
    pub domain: String,
    pub port: u16,
    pub enable_http2: bool,
    pub enable_tls: bool,
    pub min_tls_version: String,
    pub max_request_size: usize,
    pub enable_rate_limiting: bool,
    pub rate_limit_requests: u32,
    pub rate_limit_window: u32,
    pub tunnel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySettings {
    pub allowed_origins: Vec<String>,
    pub audit_log_path: String,
    pub cookie_httponly: bool,
    pub cookie_samesite: String,
    pub cookie_secure: bool,
    pub csrf_token_timeout: u32,
    pub enable_audit_logging: bool,
    pub enable_request_validation: bool,
    pub session_timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSocketServerSettings {
    pub max_connections: usize,
    pub max_message_size: usize,
    pub url: String,
}
```

### Features
-   Management of server network configurations.
-   Security policies (CORS, session timeouts).
-   WebSocket server parameters.
-   Storage of Nostr user profiles, including their individual API keys for AI services.
-   Default API keys for services if no user-specific key is available.

## Metadata Store (`MetadataStore` and `Metadata`)

The metadata store is responsible for holding information about each processed file (node) in the knowledge graph.

### Core Structure (from [`src/models/metadata.rs`](../../src/models/metadata.rs))
The `MetadataStore` is a type alias for `HashMap<String, Metadata>`, where the key is typically a unique identifier for the content (e.g., file path or a derived ID).

```rust
// In src/models/metadata.rs
pub type MetadataStore = std::collections::HashMap<String, Metadata>;
```

The `Metadata` struct contains details for each processed file/node:
```rust
// In src/models/metadata.rs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub file_name: String, // Original file name
    pub file_size: u64,    // File size in bytes (ensure type matches actual usage, e.g., u64)
    pub node_size: f64,
    pub hyperlink_count: usize,
    pub sha1: Option<String>, // SHA1 hash of the file content, optional
    pub node_id: String,      // Unique identifier for the graph node (often derived from file_name or path)
    pub last_modified: Option<i64>, // Unix timestamp (seconds), optional

    // AI Service related fields
    pub perplexity_link: Option<String>, // Link to Perplexity discussion/page if available
    pub last_perplexity_process_time: Option<i64>, // Timestamp of last Perplexity processing, optional
    pub topic_counts: Option<std::collections::HashMap<String, usize>>, // Counts of topics/keywords, optional

    // Other potential fields:
    // pub title: Option<String>,
    // pub tags: Option<Vec<String>>,
    // pub content_type: Option<String>, // e.g., "markdown", "pdf"
    // pub created_at: Option<i64>,
}
```
-   The `MetadataStore` itself is a `HashMap`. Relationships between nodes (edges) are typically stored separately in `GraphData` within `GraphService`. Statistics are usually computed on-the-fly or by dedicated analysis processes rather than being stored directly in `MetadataStore`.
-   The `node_size` field is calculated on the server based on file size and stored in the metadata for potential use by the client or other services.

### Operations
-   The `MetadataStore` (as a `HashMap`) supports standard CRUD operations for `Metadata` entries.
-   Relationship management and statistics tracking are typically handled by services like `GraphService` or `FileService` by processing the contents of the `MetadataStore`.

## Implementation Details

### Thread Safety
Shared mutable data structures like `MetadataStore` and settings objects are managed by **Actix actors** within `AppState`. Instead of using `Arc<RwLock<T>>`, the application uses actor addresses (`Addr<...Actor>`) for thread-safe access through message passing.
```rust
// Example from app_state.rs
// pub metadata_addr: Addr<MetadataActor>,
// pub settings_addr: Addr<SettingsActor>,
```

### Serialization
Data models are designed to be serializable and deserializable using `serde` for various formats like JSON (for API communication) and YAML (for configuration files). The `#[serde(rename_all = "camelCase")]` attribute is often used for client compatibility.

### Validation
Validation logic is typically implemented within the services that manage these models or during the deserialization process (e.g., using `serde` attributes or custom validation functions).