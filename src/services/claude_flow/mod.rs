// Declare the modules within this new service
pub mod client;
pub mod error;
pub mod transport;
pub mod types;

// Re-export the primary components for use within the backend
pub use client::{ClaudeFlowClient, ClaudeFlowClientBuilder};
pub use error::{ConnectorError, Result};
pub use types::*;