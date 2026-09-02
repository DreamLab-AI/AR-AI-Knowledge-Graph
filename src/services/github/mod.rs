//! GitHub service module providing API interactions for content and pull requests
//!
//! This module is split into:
//! - Content API: Handles fetching and checking markdown files
//! - Pull Request API: Manages creation and updates of pull requests
//! - Common types and error handling
//! - Configuration: Environment-based configuration

pub mod api;
pub mod config;
pub mod content_enhanced;
pub mod pr;
pub mod types;
pub mod url_path;

pub use api::GitHubClient;
pub use config::GitHubConfig;
pub use content_enhanced::EnhancedContentAPI as ContentAPI;
pub use pr::PullRequestAPI;
pub use types::{GitHubError, GitHubFile, GitHubFileMetadata};

// Re-export commonly used types for convenience
pub use types::{ContentResponse, PullRequestResponse};
