use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::error::Error;
use std::fmt;

/// Rate limit information from GitHub API
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub limit: u32,
    pub reset_time: DateTime<Utc>,
}

/// Represents errors that can occur during GitHub API operations
#[derive(Debug)]
pub enum GitHubError {
    /// Error returned by the GitHub API itself
    ApiError(String),
    /// Network-related errors during API calls
    NetworkError(reqwest::Error),
    /// JSON serialization/deserialization errors
    SerializationError(serde_json::Error),
    /// Input validation errors
    ValidationError(String),
    /// Base64 encoding/decoding errors
    Base64Error(base64::DecodeError),
    /// Rate limit exceeded
    RateLimitExceeded(RateLimitInfo),
    /// Resource not found
    NotFound(String),
}

impl fmt::Display for GitHubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitHubError::ApiError(msg) => write!(f, "GitHub API error: {}", msg),
            GitHubError::NetworkError(e) => write!(f, "Network error: {}", e),
            GitHubError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            GitHubError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            GitHubError::Base64Error(e) => write!(f, "Base64 encoding error: {}", e),
            GitHubError::RateLimitExceeded(info) => {
                write!(f, "Rate limit exceeded. Remaining: {}/{}, Reset time: {}",
                    info.remaining, info.limit, info.reset_time)
            }
            GitHubError::NotFound(path) => {
                write!(f, "Resource not found: {}", path)
            }
        }
    }
}

impl Error for GitHubError {}

impl From<reqwest::Error> for GitHubError {
    fn from(err: reqwest::Error) -> Self {
        GitHubError::NetworkError(err)
    }
}

impl From<serde_json::Error> for GitHubError {
    fn from(err: serde_json::Error) -> Self {
        GitHubError::SerializationError(err)
    }
}

impl From<base64::DecodeError> for GitHubError {
    fn from(err: base64::DecodeError) -> Self {
        GitHubError::Base64Error(err)
    }
}

/// Represents a file in the GitHub repository
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubFile {
    /// Name of the file
    pub name: String,
    /// Full path to the file in the repository
    pub path: String,
    /// SHA hash of the file content
    pub sha: String,
    /// Size of the file in bytes
    pub size: usize,
    /// GitHub API URL for the file
    pub url: String,
    /// Direct download URL for the file content
    pub download_url: String,
}

/// Metadata about a file from GitHub including tracking information
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct GitHubFileMetadata {
    /// Name of the file
    pub name: String,
    /// SHA hash of the file content
    pub sha: String,
    /// Direct download URL for the file content
    pub download_url: String,
    /// ETag for caching
    pub etag: Option<String>,
    /// When this metadata was last checked
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_checked: Option<DateTime<Utc>>,
    /// When the file was last modified on GitHub (any commit)
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_modified: Option<DateTime<Utc>>,
    /// When the file content was last actually changed
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_content_change: Option<DateTime<Utc>>,
    /// File blob SHA from GitHub
    pub file_blob_sha: Option<String>,
}

/// Response from content-related API calls
#[derive(Debug, Deserialize)]
pub struct ContentResponse {
    pub sha: String,
}

/// Response from pull request creation
#[derive(Debug, Deserialize)]
pub struct PullRequestResponse {
    pub html_url: String,
    pub number: u32,
    pub state: String,
}

/// Request to create a new branch
#[derive(Debug, Serialize)]
pub struct CreateBranchRequest {
    pub ref_name: String,
    pub sha: String,
}

/// Request to create a pull request
#[derive(Debug, Serialize)]
pub struct CreatePullRequest {
    pub title: String,
    pub head: String,
    pub base: String,
    pub body: String,
}

/// Request to update a file
#[derive(Debug, Serialize)]
pub struct UpdateFileRequest {
    pub message: String,
    pub content: String,
    pub sha: String,
    pub branch: String,
}