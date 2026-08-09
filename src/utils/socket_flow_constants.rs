// Node and graph constants
pub const NODE_SIZE: f32 = 1.0;
pub const EDGE_WIDTH: f32 = 0.1;
pub const MIN_DISTANCE: f32 = 0.75;
pub const MAX_DISTANCE: f32 = 10.0;

// WebSocket constants - matching nginx configuration
pub const HEARTBEAT_INTERVAL: u64 = 30;
pub const CLIENT_TIMEOUT: u64 = 60;
pub const MAX_CLIENT_TIMEOUT: u64 = 3600;
pub const MAX_MESSAGE_SIZE: usize = 100 * 1024 * 1024;
pub const BINARY_CHUNK_SIZE: usize = 64 * 1024;

// Update rate constants
pub const POSITION_UPDATE_RATE: u32 = 5;
pub const METADATA_UPDATE_RATE: u32 = 1;

// Binary message constants
pub const NODE_POSITION_SIZE: usize = 24;
pub const BINARY_HEADER_SIZE: usize = 4;

// Compression constants
pub const COMPRESSION_THRESHOLD: usize = 1024;
pub const ENABLE_COMPRESSION: bool = true;
