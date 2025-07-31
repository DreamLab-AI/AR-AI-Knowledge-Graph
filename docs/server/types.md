# Types Architecture

## Overview
The types module defines core data structures, type aliases, and common enums used throughout the application.

## Core Types

### Graph Types
```rust
// From src/models/graph.rs
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<Node>, // Uses Node from src/utils/socket_flow_messages.rs
    pub edges: Vec<Edge>, // Uses Edge from src/models/edge.rs
    // pub metadata: Option<MetadataStore>, // MetadataStore is HashMap<String, Metadata> from src/models/metadata.rs
                                       // This field might not be directly on GraphData but associated elsewhere.
    // pub id_to_metadata: HashMap<String, String>, // This seems to be an internal mapping, not part of the primary GraphData model.
}

// From src/types/vec3.rs
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Pod, Zeroable, Default)] // Pod, Zeroable from bytemuck
pub struct Vec3Data {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3Data {
    pub fn new(x: f32, y: f32, z: f32) -> Self;
    pub fn zero() -> Self;
    pub fn as_array(&self) -> [f32; 3];
    pub fn as_vec3(&self) -> Vec3;  // Converts to glam::Vec3
}

// Conversion implementations
impl From<Vec3> for Vec3Data;      // From glam::Vec3
impl From<Vec3Data> for Vec3;      // To glam::Vec3
impl From<[f32; 3]> for Vec3Data;  // From array
impl From<Vec3Data> for [f32; 3];  // To array

// From src/utils/socket_flow_messages.rs - Server-side version for physics and binary protocol
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Pod, Zeroable, Default)] // Pod, Zeroable from bytemuck
pub struct BinaryNodeData {
    pub position: Vec3Data,
    pub velocity: Vec3Data,
    pub mass: u8,       // Used in server-side physics
    pub flags: u8,      // Used in server-side physics (e.g., is_fixed)
    pub padding: [u8; 2], // For alignment
                          // Note: The wire format uses a 28-byte structure with u32 node ID
}

// The primary `Node` model is defined in `src/models/node.rs`. It uses a `u32` for its `id` and contains a `metadata_id: String` field to link back to the original file/metadata entry.

// From src/models/edge.rs
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String, // Unique identifier for the edge (e.g., "source_target")
    pub source: String, // ID of the source node
    pub target: String, // ID of the target node
    pub weight: Option<f32>, // Strength or importance of the connection
    pub label: Option<String>, // Optional display label for the edge
    // pub edge_type: Option<String>, // Type of relationship
    pub metadata: Option<HashMap<String, serde_json::Value>>, // Arbitrary metadata
}
```

### Simulation Types
```rust
pub enum SimulationPhase {
    Dynamic,
    Static,
    Paused,
}

pub enum SimulationMode {
    Local,
    Remote,
    Hybrid,
}
```

## Models

### Settings Models
The definitive source for all server-side settings structures is `src/config/mod.rs`.

-   **`AppFullSettings`**: The root struct for all server-side configurations, loaded from `settings.yaml` (snake_case) and environment variables. It contains:
    -   `visualisation: VisualisationSettings`
    -   `system: ServerSystemConfigFromFile` (which includes `NetworkSettings`, `ServerFullWebSocketSettings`, `SecuritySettings`, `DebugSettings`)
    -   `xr: XRSettings`
    -   `auth: AuthSettings`
    -   Optional AI configurations: `ragflow: Option<RAGFlowConfig>`, `perplexity: Option<PerplexityConfig>`, `openai: Option<OpenAIConfig>`, `kokoro: Option<KokoroConfig>`. (Note: `WhisperSettings` is not a direct field; Whisper STT is usually part of `OpenAIConfig` or handled within `SpeechService`).

-   **`Settings`**: This is the client-facing settings structure, typically serialized as camelCase JSON. It's derived from `AppFullSettings`. It includes:
    -   `visualisation: VisualisationSettings` (same as server's)
    -   `system: ClientSystemSettings` (a subset/transformation of `ServerSystemConfigFromFile`, containing `ClientWebSocketSettings` and `DebugSettings` relevant to the client)
    -   `xr: XRSettings` (same as server's)
    -   `auth: AuthSettings` (same as server's, though client primarily uses tokens/features derived from this)
    -   Optional AI configurations (mirrored from `AppFullSettings` if present, but client usually doesn't get API keys directly this way).

-   **Specific Sub-Structs** (all defined in `src/config/mod.rs`):
    -   `VisualisationSettings` (and its nested `NodeSettings`, `EdgeSettings`, `PhysicsSettings`, `RenderingSettings`, `AnimationSettings`, `LabelSettings`, `BloomSettings`, `HologramSettings`, `CameraSettings`)
    -   `ServerSystemConfigFromFile`
    -   `NetworkSettings`
    -   `ServerFullWebSocketSettings`
    -   `SecuritySettings`
    -   `DebugSettings`
    -   `ClientWebSocketSettings` (derived for client)
    -   `ClientSystemSettings` (derived for client)
    -   `XRSettings`
    -   `AuthSettings`
    -   `RAGFlowConfig`, `PerplexityConfig`, `OpenAIConfig`, `KokoroConfig` (or similar names for AI service configs)

### Protected Settings Models
These are defined in [`src/models/protected_settings.rs`](../../src/models/protected_settings.rs) and are managed separately from `settings.yaml`, often in a distinct JSON file for sensitive data.

-   **`ProtectedSettings`**: The root struct.
    -   `network: ProtectedNetworkConfig` (server operational network settings)
    -   `security: ServerSecurityConfig` (re-uses `SecuritySettings` from `src/config/mod.rs` for consistency)
    -   `websocket_server: ProtectedWebSocketServerConfig`
    -   `users: HashMap<String, NostrUser>` (stores user profiles, keyed by Nostr pubkey)
    -   `default_api_keys: ApiKeys` (fallback API keys)

-   **`NostrUser`**: Contains user-specific details.
    -   `pubkey: String`
    -   `npub: Option<String>`
    -   `is_power_user: bool`
    -   `api_keys: Option<ApiKeys>` (user's own API keys for AI services)
    -   `user_settings_path: Option<String>` (path to their persisted `UserSettings` YAML)

-   **`ApiKeys`**: Struct for holding API keys for various services (Perplexity, OpenAI, RAGFlow).
    -   `perplexity: Option<String>`
    -   `openai: Option<String>`
    -   `ragflow: Option<String>`

-   `ProtectedNetworkConfig`, `ProtectedWebSocketServerConfig` are also defined here.
```

### Metadata Models
Refer to [`src/models/metadata.rs`](../../src/models/metadata.rs).
-   **`MetadataStore`**: Type alias for `HashMap<String, Metadata>`.
-   **`Metadata`**: Struct containing `fileName`, `fileSize`, `hyperlinkCount`, `sha1`, `nodeId`, `lastModified`, `perplexityLink`, `lastPerplexityProcessTime`, `topicCounts`.
    (Note: `node_size` is not typically stored here server-side; visual size is client-determined).
```rust
// Example from src/models/metadata.rs
// pub type MetadataStore = HashMap<String, Metadata>;
// pub struct Metadata { /* ... fields ... */ }
```

## Error Types

### Service Errors
```rust
pub enum ServiceError {
    IO(std::io::Error),
    Graph(String),
    Config(String),
    GitHub(String),
    AI(String),
    Speech(SpeechError),
    // ... other service-specific errors
}

impl From<std::io::Error> for ServiceError {
    fn from(err: std::io::Error) -> Self {
        ServiceError::IO(err)
    }
}

impl From<reqwest::Error> for ServiceError {
    fn from(err: reqwest::Error) -> Self {
        ServiceError::GitHub(format!("GitHub API error: {}", err))
    }
}

## Speech Types

### Speech Types
Refer to [`src/types/speech.rs`](../../src/types/speech.rs).
-   **`SpeechCommand`**: Enum for commands like `ProcessTTS`, `ProcessSTTChunk`, `SetTTSProvider`, etc.
-   **`SpeechError`**: Enum for errors related to STT, TTS, provider issues, etc.
-   **`TTSProvider`**: Enum for selecting TTS providers (e.g., `OpenAI`, `Kokoro`).
-   **`SpeechOptions`**: Struct for TTS options like voice, speed, format.
```rust
// Example from src/types/speech.rs
// pub enum SpeechCommand { /* ... */ }
// pub enum SpeechError { /* ... */ }
// pub enum TTSProvider { /* ... */ }
// pub struct SpeechOptions { /* ... */ }
```

## AI Service Models

The application interacts with various AI services, each potentially having its own specific request and response structures. This section outlines key data structures used for these interactions, as defined within the application. If a service utilizes an external SDK (e.g., `async-openai`), its service documentation will refer to the types from that SDK, and they will not be redefined here.

### RAGFlow Service Models
Refer to [`src/models/ragflow_chat.rs`](../../src/models/ragflow_chat.rs).
-   **`RagflowChatRequest`**: Contains `question`, `session_id` (optional), `stream` (optional boolean).
-   **`RagflowChatResponse`**: Contains `answer`, `session_id`.
```rust
// Example from src/models/ragflow_chat.rs
// pub struct RagflowChatRequest { /* ... */ }
// pub struct RagflowChatResponse { /* ... */ }
```

### Perplexity Service Models
Refer to [`src/services/perplexity_service.rs`](../../src/services/perplexity_service.rs) (these might be defined directly in the service module if not in a separate models file).
-   **`QueryRequest`**: Contains fields like `query`, `model`, `max_tokens`, `temperature`, etc.
-   **`PerplexityResponse`**: Contains `content` (the answer) and `link` (source/page link).
```rust
// Example from src/services/perplexity_service.rs
// pub struct QueryRequest { /* ... */ }
// pub struct PerplexityResponse { /* ... */ }
```

### API Errors
```rust
pub enum APIError {
    NotFound(String),
    Unauthorized,
    RateLimit,
    Internal(String),
}
```

## Type Aliases

### Common Aliases
```rust
pub type Result<T> = std::result::Result<T, Error>;
pub type NodeMap = HashMap<String, Node>;
pub type MetadataMap = HashMap<String, Metadata>;
pub type SafeAppState = Arc<AppState>;
// Note: Settings and metadata are now managed by actors, not Arc<RwLock<T>>
// pub type SafeSettings = Addr<SettingsActor>;
// pub type SafeMetadataManager = Addr<MetadataActor>;
```

## Constants

### System Constants
```rust
pub const MAX_NODES: usize = 10000;
pub const DEFAULT_BATCH_SIZE: usize = 100;
pub const CACHE_DURATION: Duration = Duration::from_secs(3600);
```

### Configuration Constants
```rust
pub const DEFAULT_PORT: u16 = 8080;
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const API_VERSION: &str = "v1";