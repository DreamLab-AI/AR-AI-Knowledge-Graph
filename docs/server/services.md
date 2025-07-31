# Services Architecture

## Overview
The services layer provides core business logic, external integrations, and data processing capabilities.

## GitHub Service

### GitHub Service Configuration ([`src/services/github/config.rs`](../../src/services/github/config.rs))
The `GitHubConfig` struct (renamed from `GitHubServiceConfig` in the plan for clarity, assuming it's the primary config for GitHub interactions) defines parameters for connecting to the GitHub API.

```rust
// In src/services/github/config.rs
pub struct GitHubConfig {
    pub owner: String,
    pub repo: String,
    pub branch: Option<String>, // Or default branch if None
    pub personal_access_token: Option<String>, // GitHub PAT
    pub app_id: Option<u64>, // For GitHub App authentication
    pub private_key_path: Option<String>, // Path to private key for GitHub App
    pub base_path: Option<String>, // Subdirectory within the repo to scan
    pub user_agent: String,
    // Potentially other fields like timeout, retries
}
```
This configuration is typically part of `AppFullSettings.auth.github_config` or a similar path. The actual `GitHubService` or `GitHubClient` would use this config.

### File Service ([`src/services/file_service.rs`](../../src/services/file_service.rs))
The `FileService` is responsible for interactions with the local file system and orchestrating content fetching (which might involve a `GitHubService` or `GitHubClient` using `ContentAPI` traits). It manages Markdown files and their metadata.

```rust
// In src/services/file_service.rs
pub struct FileService {
    // settings_addr: Addr<SettingsActor>, // To access paths via actor messages
    // github_service: Option<Arc<GitHubService>>, // If GitHub is a source
    // metadata_addr: Addr<MetadataActor>, // To update metadata via actor messages
    // node_id_counter: AtomicU32, // For generating unique node IDs if needed
}

impl FileService {
    // pub fn new(settings_addr: Addr<SettingsActor>, metadata_addr: Addr<MetadataActor>, github_service: Option<Arc<GitHubService>>) -> Self;
    
    // Manages local file operations (reading, writing, listing).
    // pub async fn process_local_files(&self) -> Result<Vec<ProcessedFile>, FileServiceError>;
    
    // Fetches files from configured sources (local, GitHub via ContentAPI).
    // pub async fn fetch_and_process_all_content(&self) -> Result<Vec<ProcessedFile>, FileServiceError>;
    
    // Creates/updates metadata in the MetadataStore based on processed files.
    // pub async fn update_metadata_store(&self, processed_files: Vec<ProcessedFile>) -> Result<(), FileServiceError>;
    
    // Saves the MetadataStore to disk.
    // pub fn save_metadata_store(&self) -> Result<(), FileServiceError>;
    
    // Loads the MetadataStore from disk.
    // pub fn load_metadata_store(&self) -> Result<MetadataStore, FileServiceError>;

    // Handles file uploads if that feature is routed to FileService.
    // pub async fn handle_file_upload(&self, payload: web::Bytes) -> Result<GraphData, FileServiceError>;
    
    // Initializes local storage directories if needed.
    // pub async fn initialize_storage(&self) -> Result<(), FileServiceError>;
}
```
- Manages local file system operations.
- Interacts with a `ContentAPI` implementor (like `GitHubService` or a local file system accessor) to fetch raw content.
- Processes files to extract information and update the `MetadataStore`.
- Handles metadata persistence (loading/saving).

### Graph Service ([`src/services/graph_service.rs`](../../src/services/graph_service.rs))
The `GraphService` is central to managing the graph's structure, layout, and real-time updates.

```rust
// In src/services/graph_service.rs
// Note: GraphService is now implemented as GraphServiceActor
pub struct GraphService {
    // graph_data: GraphData, // Holds the current nodes and edges (internal to actor)
    // node_map: HashMap<String, crate::utils::socket_flow_messages::Node>, // For quick node lookup by ID
    // gpu_compute_addr: Option<Addr<GPUComputeActor>>, // GPU acceleration via actor messages
    // settings_addr: Addr<SettingsActor>, // To access simulation parameters via messages
    // client_manager_addr: Addr<ClientManagerActor>, // To broadcast updates via messages
    // shutdown_signal: Arc<AtomicBool>, // For graceful shutdown
    // ... other fields for caching, simulation state, etc.
}

impl GraphService {
    // pub async fn new(settings_addr: Addr<SettingsActor>, gpu_compute_addr: Option<Addr<GPUComputeActor>>, client_manager_addr: Addr<ClientManagerActor>) -> Self;
    
    // Builds the graph from the MetadataStore.
    // pub async fn build_graph_from_metadata(&self, metadata_store: &MetadataStore) -> Result<(), GraphServiceError>;
    
    // Starts the continuous physics simulation and broadcast loop.
    // pub fn start_simulation_loop(&self);
    
    // Calculates a layout iteration (CPU or GPU).
    // async fn calculate_layout_iteration(&self);
    
    // Provides access to graph data (full, paginated, specific nodes/edges).
    // pub async fn get_graph_data(&self) -> GraphData;
    // pub async fn get_paginated_graph_data(&self, page: usize, page_size: usize) -> PaginatedGraphResponse;
    
    // Handles requests to update or refresh the graph.
    // pub async fn trigger_graph_rebuild(&self, file_service: Arc<FileService>) -> Result<(), GraphServiceError>; // Re-fetches and re-processes
    // pub async fn trigger_graph_refresh(&self) -> Result<(), GraphServiceError>; // Rebuilds from existing metadata

    // pub async fn shutdown(&self);
}
```
- Manages the in-memory `GraphData` (nodes, edges).
- Handles physics simulation, either via `GPUCompute` or a CPU fallback, using parameters from `AppFullSettings.visualisation.physics` and `SimulationParams`.
- Builds the graph from `MetadataStore` (provided by `FileService` or `AppState`).
- Provides methods for accessing graph data.
- Broadcasts updates to clients via the static `APP_CLIENT_MANAGER`.
- The `settings` field is not directly on `GraphService` in the plan, but it needs access to `AppFullSettings` (likely passed during construction or via `AppState`) for simulation parameters.

## AI Services

The application architecture includes several distinct AI-related services, rather than a single monolithic `AIService`. These are typically held as optional fields within `AppState`.

### Perplexity Service ([`src/services/perplexity_service.rs`](../../src/services/perplexity_service.rs))
Integrates with the Perplexity AI API.
```rust
// In src/services/perplexity_service.rs
pub struct PerplexityService {
    // client: reqwest::Client,
    // api_key: String,
    // config: PerplexityConfig, // From AppFullSettings.perplexity
}
impl PerplexityService {
    // pub fn new(config: PerplexityConfig, client: reqwest::Client) -> Self;
    // pub async fn query(&self, request: QueryRequest) -> Result<PerplexityResponse, PerplexityError>;
}
```

### RAGFlow Service ([`src/services/ragflow_service.rs`](../../src/services/ragflow_service.rs))
Integrates with a RAGFlow instance. Configuration (API key, base URL) is typically loaded from environment variables or `AppFullSettings.ragflow`.
```rust
// In src/services/ragflow_service.rs
pub struct RAGFlowService {
    // client: reqwest::Client,
    // api_key: String,
    // api_base_url: String,
    // config: RAGFlowConfig, // From AppFullSettings.ragflow
}
impl RAGFlowService {
    // pub fn new(config: RAGFlowConfig, client: reqwest::Client) -> Self;
    // pub async fn chat(&self, request: RagflowChatRequest) -> Result<RagflowChatResponse, RagFlowError>;
}
```

### Speech Service ([`src/services/speech_service.rs`](../../src/services/speech_service.rs))
Orchestrates Speech-to-Text (STT) and Text-to-Speech (TTS) functionalities. It interacts with configured STT/TTS providers (e.g., OpenAI, Kokoro).
```rust
// In src/services/speech_service.rs
pub struct SpeechService {
    // settings_addr: Addr<SettingsActor>, // To access OpenAIConfig, KokoroConfig via messages
    // http_client: reqwest::Client,
    // audio_broadcast_tx: tokio::sync::broadcast::Sender<Vec<u8>>, // For broadcasting TTS audio to speech_socket_handler
    // command_tx: tokio::sync::mpsc::Sender<SpeechCommand>, // For internal command processing
}
impl SpeechService {
    // pub fn new(settings_addr: Addr<SettingsActor>, client: reqwest::Client, audio_tx: tokio::sync::broadcast::Sender<Vec<u8>>) -> Self;
    // pub fn start_processing_loop(&self); // Handles commands from command_tx
    // pub async fn process_stt_request(&self, audio_data: Vec<u8>) -> Result<String, SpeechError>;
    // pub async fn process_tts_request(&self, text: String, options: TTSSpeechOptions) -> Result<(), SpeechError>; // Sends audio via audio_broadcast_tx
}
```
- Manages audio streaming via its dedicated WebSocket handler (`speech_socket_handler.rs`).
- Performs STT using configured providers (e.g., OpenAI Whisper, if its API key is in `AppFullSettings.openai`).
- Performs TTS using configured providers (e.g., OpenAI TTS, Kokoro TTS).
- **Clarification**: `WhisperSttService` is not a separate struct in `AppState`. STT functionality, including Whisper if used, is integrated within `SpeechService` or called directly using an OpenAI client configured with keys from `AppFullSettings.openai`.

## Error Handling & State Management
- Each service typically defines its own error types (e.g., `GraphServiceError`, `FileServiceError`).
- Shared state (like `AppFullSettings`, `MetadataStore`) is managed within `AppState` using the **Actix actor system**. Services communicate with actors via message passing using actor addresses (`Addr<...Actor>`). This eliminates lock contention and provides better concurrency than `Arc<RwLock<T>>`.

## Performance Optimization
- **Caching**: `GraphService` uses caching for node positions.
- **Batch Processing**: `FileService` processes GitHub files in batches.
- **GPU Acceleration**: `GraphService` uses `GPUCompute` for physics.