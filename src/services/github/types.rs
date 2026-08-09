use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub limit: u32,
    pub reset_time: DateTime<Utc>,
}

#[derive(Debug)]
pub enum GitHubError {
    ApiError(String),

    NetworkError(reqwest::Error),

    SerializationError(serde_json::Error),

    ValidationError(String),

    Base64Error(base64::DecodeError),

    RateLimitExceeded(RateLimitInfo),

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
                write!(
                    f,
                    "Rate limit exceeded. Remaining: {}/{}, Reset time: {}",
                    info.remaining, info.limit, info.reset_time
                )
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubFile {
    pub name: String,

    pub path: String,

    pub sha: String,

    pub size: usize,

    pub url: String,

    pub download_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct GitHubFileMetadata {
    pub name: String,

    pub sha: String,

    pub download_url: String,

    pub etag: Option<String>,

    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_checked: Option<DateTime<Utc>>,

    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_modified: Option<DateTime<Utc>>,

    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_content_change: Option<DateTime<Utc>>,

    pub file_blob_sha: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubFileBasicMetadata {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub download_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ContentResponse {
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct PullRequestResponse {
    pub html_url: String,
    pub number: u32,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct CreateBranchRequest {
    pub ref_name: String,
    pub sha: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePullRequest {
    pub title: String,
    pub head: String,
    pub base: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateFileRequest {
    pub message: String,
    pub content: String,
    pub sha: String,
    pub branch: String,
}

/// Ontology-specific file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyFileMetadata {
    /// Basic file metadata
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub download_url: String,

    /// Git commit information
    pub git_commit_date: Option<DateTime<Utc>>,

    /// Ontology-specific flags
    pub has_public_flag: bool,
    pub has_ontology_block: bool,
    pub priority: OntologyPriority,

    /// Extracted metadata
    pub source_domain: Option<String>,
    pub topics: Vec<String>,
    pub relationship_count: usize,
    pub class_count: usize,
    pub property_count: usize,
}

/// Priority levels for ontology file processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OntologyPriority {
    /// Files with both public:: true AND OntologyBlock
    Priority1 = 1,
    /// Files with OntologyBlock only
    Priority2 = 2,
    /// Files with public:: true only (knowledge graph)
    Priority3 = 3,
    /// No special flags
    None = 99,
}

impl OntologyFileMetadata {
    pub fn new(basic: GitHubFileBasicMetadata) -> Self {
        Self {
            name: basic.name,
            path: basic.path,
            sha: basic.sha,
            size: basic.size,
            download_url: basic.download_url,
            git_commit_date: None,
            has_public_flag: false,
            has_ontology_block: false,
            priority: OntologyPriority::None,
            source_domain: None,
            topics: Vec::new(),
            relationship_count: 0,
            class_count: 0,
            property_count: 0,
        }
    }

    pub fn calculate_priority(&mut self) {
        self.priority = match (self.has_public_flag, self.has_ontology_block) {
            (true, true) => OntologyPriority::Priority1,
            (false, true) => OntologyPriority::Priority2,
            (true, false) => OntologyPriority::Priority3,
            (false, false) => OntologyPriority::None,
        };
    }
}
