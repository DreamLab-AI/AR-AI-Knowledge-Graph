use super::api::GitHubClient;
use super::types::{GitHubFileMetadata, GitHubError, RateLimitInfo};
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use std::error::Error;
use std::sync::Arc;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::time::Duration;
use std::pin::Pin;
use std::future::Future;
use urlencoding;

const BATCH_SIZE: usize = 5;
const BATCH_DELAY: Duration = Duration::from_millis(500);

/// Handles GitHub content API operations
#[derive(Clone)]
pub struct ContentAPI {
    client: Arc<GitHubClient>,
    rate_limits: Arc<RwLock<HashMap<String, RateLimitInfo>>>,
}

impl ContentAPI {
    /// Create a new ContentAPI instance
    pub fn new(client: Arc<GitHubClient>) -> Self {
        Self {
            client,
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /*
    /// Ensure consistent URL encoding for paths
    async fn encode_path(&self, path: &str) -> String {
        let settings = self.client.settings().read().await;
        let debug_enabled = settings.system.debug.enabled;
        drop(settings);

        if debug_enabled {
            debug!("Encoding path: '{}'", path);
        }

        // First decode to prevent double-encoding
        let decoded = urlencoding::decode(path)
            .unwrap_or(std::borrow::Cow::Owned(path.to_string()))
            .into_owned();
        
        if debug_enabled {
            debug!("Decoded path: '{}'", decoded);
        }
        
        // Clean the path
        let cleaned = decoded
            .trim_matches('/')
            .replace("//", "/")
            .replace('\\', "/");

        if debug_enabled {
            debug!("Cleaned path: '{}'", cleaned);
        }

        // Encode using form URL encoding for consistent handling
        let encoded = url::form_urlencoded::byte_serialize(cleaned.as_bytes())
            .collect::<String>();

        if debug_enabled {
            debug!("Final encoded path: '{}'", encoded);
        }

        encoded
    }
    */

    /// Extract and update rate limit information from response headers
    async fn update_rate_limits(&self, headers: &HeaderMap) {
        let settings = self.client.settings().read().await;
        let debug_enabled = settings.system.debug.enabled;
        drop(settings);

        if debug_enabled {
            debug!("Processing rate limit headers: {:?}", headers);
        }

        if let (Some(remaining), Some(limit), Some(reset)) = (
            headers.get("x-ratelimit-remaining"),
            headers.get("x-ratelimit-limit"),
            headers.get("x-ratelimit-reset")
        ) {
            let remaining = remaining.to_str().unwrap_or("0").parse().unwrap_or(0);
            let limit = limit.to_str().unwrap_or("0").parse().unwrap_or(0);
            let reset = reset.to_str().unwrap_or("0").parse().unwrap_or(0);
            
            if debug_enabled {
                debug!("Rate limit values - Remaining: {}, Limit: {}, Reset: {}",
                    remaining, limit, reset);
            }
            
            let reset_time = DateTime::from_timestamp(reset, 0)
                .unwrap_or_else(|| Utc::now());

            let info = RateLimitInfo {
                remaining,
                limit,
                reset_time,
            };

            if debug_enabled {
                debug!("Updating rate limits - New info: {:?}", info);
            }

            let mut limits = self.rate_limits.write().await;
            limits.insert("core".to_string(), info);
        } else if debug_enabled {
            debug!("No rate limit headers found in response");
        }
    }

    /// Check rate limits and handle backoff if needed
    fn check_rate_limit(&self) -> Pin<Box<dyn Future<Output = Result<(), GitHubError>> + '_>> {
        Box::pin(async move {
            let settings = self.client.settings().read().await;
            let debug_enabled = settings.system.debug.enabled;
            drop(settings);

            if debug_enabled {
                debug!("Checking rate limits...");
            }

            let limits = self.rate_limits.read().await;
            if let Some(info) = limits.get("core") {
                if debug_enabled {
                    debug!("Current rate limit info: {:?}", info);
                }

                if info.remaining == 0 {
                    let now = Utc::now();
                    if debug_enabled {
                        debug!("Rate limit exhausted. Current time: {}, Reset time: {}",
                            now, info.reset_time);
                    }

                    if now < info.reset_time {
                        let wait_time = info.reset_time - now;
                        let backoff = wait_time.num_seconds().min(30) as u64;
                        
                        if debug_enabled {
                            debug!("Rate limited. Wait time: {}s, Using backoff: {}s",
                                wait_time.num_seconds(), backoff);
                        }
                        
                        // Drop the read lock before sleeping
                        drop(limits);
                        
                        // Sleep with exponential backoff, max 30 seconds
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                        
                        if debug_enabled {
                            debug!("Backoff complete, rechecking rate limits");
                        }
                        
                        // Recursively check rate limit
                        return self.check_rate_limit().await;
                    }

                    if debug_enabled {
                        debug!("Rate limit exceeded and reset time passed");
                    }
                    return Err(GitHubError::RateLimitExceeded(info.clone()));
                }

                if debug_enabled {
                    debug!("Rate limit check passed. Remaining: {}/{}",
                        info.remaining, info.limit);
                }
            } else if debug_enabled {
                debug!("No rate limit information available");
            }
            Ok(())
        })
    }

    /// Check if a file is public by reading just the first line
    pub async fn check_file_public(&self, download_url: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Check rate limits before making request
        self.check_rate_limit().await?;

        // First try a HEAD request to get content length
        let head_response = self.client.client()
            .head(download_url)
            .header("Authorization", format!("Bearer {}", self.client.token()))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        // Update rate limits from HEAD response
        self.update_rate_limits(head_response.headers()).await;

        // Get content length, default to 1024 if not available
        let content_length: u64 = head_response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(1024);

        // Calculate appropriate range based on content length
        let range = if content_length < 100 {
            format!("bytes=0-{}", content_length - 1)
        } else {
            "bytes=0-100".to_string()
        };

        debug!("Using range {} for file of size {}", range, content_length);

        let response = self.client.client()
            .get(download_url)
            .header("Authorization", format!("Bearer {}", self.client.token()))
            .header("Accept", "application/vnd.github+json")
            .header("Range", range)
            .send()
            .await?;

        // Update rate limits from response headers
        self.update_rate_limits(response.headers()).await;

        let status = response.status();
        match status.as_u16() {
            200 | 206 => { // Success or Partial Content
                let content = response.text().await?;
                debug!("First line check ({}): '{}'", download_url, content.trim());
                Ok(content.trim().starts_with("public:: true"))
            },
            404 => {
                error!("File not found: {}", download_url);
                Err(Box::new(GitHubError::NotFound(download_url.to_string())))
            },
            416 => { // Range Not Satisfiable
                debug!("File exists but is empty or too small: {}", download_url);
                Ok(false)
            },
            429 => {
                let limits = self.rate_limits.read().await;
                if let Some(info) = limits.get("core") {
                    Err(Box::new(GitHubError::RateLimitExceeded(info.clone())))
                } else {
                    Err("Rate limit exceeded without limit info".into())
                }
            },
            _ => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!("Failed to check file public status. Status: {}, Error: {}", status, error_text);
                Err(Box::new(GitHubError::ApiError(format!("{} - {}", status, error_text))))
            }
        }
    }

    /// Fetch full content of a file
    pub async fn fetch_file_content(&self, download_url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        // Check rate limits before making request
        self.check_rate_limit().await?;

        let response = self.client.client()
            .get(download_url)
            .header("Authorization", format!("Bearer {}", self.client.token()))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        // Update rate limits from response headers
        self.update_rate_limits(response.headers()).await;

        let status = response.status();
        match status.as_u16() {
            200 => {
                let content = response.text().await?;
                Ok(content)
            },
            404 => {
                error!("File not found: {}", download_url);
                Err(Box::new(GitHubError::NotFound(download_url.to_string())))
            },
            429 => {
                let limits = self.rate_limits.read().await;
                if let Some(info) = limits.get("core") {
                    Err(Box::new(GitHubError::RateLimitExceeded(info.clone())))
                } else {
                    Err("Rate limit exceeded without limit info".into())
                }
            },
            _ => {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!("Failed to fetch file content. Status: {}, Error: {}", status, error_text);
                Err(Box::new(GitHubError::ApiError(format!("{} - {}", status, error_text))))
            }
        }
    }

    /// Get the last modified time for a file (when it appeared in any commit)
    pub async fn get_file_last_modified(&self, file_path: &str) -> Result<DateTime<Utc>, Box<dyn Error + Send + Sync>> {
        self.get_file_last_modified_internal(file_path, false).await
    }
    
    /// Get the last time a file's content was actually modified
    pub async fn get_file_content_last_modified(&self, file_path: &str) -> Result<DateTime<Utc>, Box<dyn Error + Send + Sync>> {
        self.get_file_last_modified_internal(file_path, true).await
    }
    
    /// Internal method to get file modification time with option to check actual changes
    async fn get_file_last_modified_internal(&self, file_path: &str, check_actual_changes: bool) -> Result<DateTime<Utc>, Box<dyn Error + Send + Sync>> {
        // Check rate limits before making request
        self.check_rate_limit().await?;

        // Use GitHubClient's path handling
        let encoded_path = self.client.get_full_path(file_path).await;
        let url = format!(
            "https://api.github.com/repos/{}/{}/commits",
            self.client.owner(), self.client.repo()
        );

        debug!("GitHub API URL: {}", url);
        debug!("Query parameters: path={}, per_page=1", encoded_path);
        debug!("Getting last modified time - Original path: {}, Encoded path: {}",
            file_path, encoded_path);

        let response = self.client.client()
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.client.token()))
            .header("Accept", "application/vnd.github+json")
            .query(&[
                ("path", encoded_path.as_str()), 
                ("per_page", if check_actual_changes { "10" } else { "1" })
            ])
            .send()
            .await?;

        // Update rate limits from response headers
        self.update_rate_limits(response.headers()).await;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Failed to get last modified time. Status: {}, Error: {}", status, error_text);
            
            return match status.as_u16() {
                404 => Err(Box::new(GitHubError::NotFound(file_path.to_string()))),
                429 => {
                    let limits = self.rate_limits.read().await;
                    if let Some(info) = limits.get("core") {
                        Err(Box::new(GitHubError::RateLimitExceeded(info.clone())))
                    } else {
                        Err(format!("Rate limit exceeded without limit info").into())
                    }
                },
                _ => Err(format!("GitHub API error: {} - {}", status, error_text).into())
            };
        }

        let response_text = response.text().await?;
        debug!("GitHub API Response for commits: {}", response_text);
        
        let commits: Vec<serde_json::Value> = serde_json::from_str(&response_text)?;
        
        if commits.is_empty() {
            error!("Empty commits array returned for path: {} (encoded: {})", file_path, encoded_path);
            return Err(Box::new(GitHubError::NotFound(format!("No commit history found for {}", file_path))));
        }
        
        // If not checking actual changes, return the first commit's date
        if !check_actual_changes {
            if let Some(last_commit) = commits.first() {
                debug!("Found commit data: {}", serde_json::to_string_pretty(last_commit)?);
                return self.extract_commit_date(last_commit);
            } else {
                error!("No commits found for file: {} (encoded path: {})", file_path, encoded_path);
                return Err(format!("No commit history found for file: {} (API path: {})", file_path, encoded_path).into());
            }
        }
        
        // Check each commit to see if the file was actually modified
        for commit in &commits {
            let sha = commit["sha"]
                .as_str()
                .ok_or("Missing commit SHA")?;
            
            match self.was_file_modified_in_commit(sha, file_path).await {
                Ok(true) => {
                    debug!("File {} was actually modified in commit: {}", file_path, sha);
                    return self.extract_commit_date(commit);
                }
                Ok(false) => {
                    debug!("File {} was not modified in commit: {} (likely a merge commit)", file_path, sha);
                }
                Err(e) => {
                    debug!("Error checking commit {}: {} - assuming file was modified", sha, e);
                    // On error, assume the file was modified to be conservative
                    return self.extract_commit_date(commit);
                }
            }
        }
        
        // If no actual modifications found in recent commits, return an error
        error!("No actual content changes found in recent commits for file: {}", file_path);
        Err(format!("No actual content changes found for file: {} in recent history", file_path).into())
    }

    /// Extract commit date from commit JSON
    fn extract_commit_date(&self, commit: &serde_json::Value) -> Result<DateTime<Utc>, Box<dyn Error + Send + Sync>> {
        // Try committer date first, then author date
        let date_str = commit["commit"]["committer"]["date"]
            .as_str()
            .or_else(|| commit["commit"]["author"]["date"].as_str())
            .ok_or("No commit date found")?;

        DateTime::parse_from_rfc3339(date_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| format!("Failed to parse date {}: {}", date_str, e).into())
    }
    
    /// Check if a specific file was actually modified in a commit
    async fn was_file_modified_in_commit(
        &self,
        commit_sha: &str,
        file_path: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Check rate limits before making request
        self.check_rate_limit().await?;
        
        let commit_url = format!(
            "https://api.github.com/repos/{}/{}/commits/{}",
            self.client.owner(),
            self.client.repo(),
            commit_sha
        );

        debug!("Checking commit {} for file changes to {}", commit_sha, file_path);

        let response = self.client.client()
            .get(&commit_url)
            .header("Authorization", format!("Bearer {}", self.client.token()))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        // Update rate limits from response headers
        self.update_rate_limits(response.headers()).await;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            debug!("Failed to get commit details: {}", error_text);
            // Return error instead of assuming file was modified
            return Err(format!("Failed to get commit details: {}", error_text).into());
        }

        let commit_data: serde_json::Value = response.json().await?;
        
        // Check if this commit has file changes
        if let Some(files) = commit_data["files"].as_array() {
            // Get the full path that would be in the commit
            let full_path = self.client.get_full_path(file_path).await;
            let decoded_full_path = urlencoding::decode(&full_path)?.into_owned();
            
            for file in files {
                if let Some(filename) = file["filename"].as_str() {
                    // Check if this is our file (need to match the path format)
                    if filename == decoded_full_path || filename == file_path || 
                       filename.ends_with(&format!("/{}", file_path)) {
                        // Check if there were actual changes
                        let additions = file["additions"].as_u64().unwrap_or(0);
                        let deletions = file["deletions"].as_u64().unwrap_or(0);
                        let changes = file["changes"].as_u64().unwrap_or(0);
                        
                        debug!(
                            "File {} in commit {}: +{} -{} (total: {} changes)",
                            filename, commit_sha, additions, deletions, changes
                        );
                        
                        // File was actually modified if there were any changes
                        return Ok(changes > 0);
                    }
                }
            }
            debug!("File {} was not found in commit {} changed files list", file_path, commit_sha);
        } else {
            debug!("No files array in commit data for {}", commit_sha);
        }

        // File was not in the changed files list
        Ok(false)
    }

    /// List all markdown files in a directory
    pub async fn list_markdown_files(&self, path: &str) -> Result<Vec<GitHubFileMetadata>, Box<dyn Error + Send + Sync>> {
        // Use GitHubClient's contents URL construction
        let url = self.client.get_contents_url(path).await;
        
        info!("GitHub API Request: URL={}, Original Path={}",
            url, path);

        let response = self.client.client()
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.client.token()))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        let status = response.status();
        let headers = response.headers().clone();
        
        info!("GitHub API Response: Status={}, Headers={:?}", status, headers);

        let body = response.text().await?;
        info!("GitHub API Response Body (first 1000 chars): {}", &body[..body.len().min(1000)]);

        if !status.is_success() {
            let error_msg = match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(error_json) => {
                    let msg = error_json["message"].as_str().unwrap_or("Unknown error");
                    format!("GitHub API error: {} - {}", status, msg)
                },
                Err(_) => format!("GitHub API error: {} - {}", status, body)
            };
            error!("{}", error_msg);
            return Err(error_msg.into());
        }

        let contents: Vec<serde_json::Value> = serde_json::from_str(&body)?;

        let settings = self.client.settings().read().await;
        let debug_enabled = settings.system.debug.enabled;
        drop(settings);

        if debug_enabled {
            debug!("Found {} total items in directory", contents.len());
            debug!("Batch size: {}, Expected batches: {}",
                BATCH_SIZE,
                (contents.len() + BATCH_SIZE - 1) / BATCH_SIZE
            );
            
            // Log file types distribution
            let file_count = contents.iter()
                .filter(|item| item["type"].as_str().unwrap_or("") == "file")
                .count();
            let md_count = contents.iter()
                .filter(|item| {
                    item["type"].as_str().unwrap_or("") == "file" &&
                    item["name"].as_str().unwrap_or("").ends_with(".md")
                })
                .count();
            debug!("Content distribution - Total: {}, Files: {}, Markdown: {}",
                contents.len(), file_count, md_count);
        }
        
        let mut markdown_files = Vec::new();
        let mut current_idx = 0;
        
        // Process files in batches
        while current_idx < contents.len() {
            let end_idx = (current_idx + BATCH_SIZE).min(contents.len());
            let batch_number = current_idx / BATCH_SIZE + 1;
            let total_batches = (contents.len() + BATCH_SIZE - 1) / BATCH_SIZE;
            
            if debug_enabled {
                debug!("Starting batch {}/{} (items {}-{} of {})",
                    batch_number,
                    total_batches,
                    current_idx + 1,
                    end_idx,
                    contents.len()
                );
            }
            
            for item in &contents[current_idx..end_idx] {
                let item_type = item["type"].as_str().unwrap_or("");
                let item_name = item["name"].as_str().unwrap_or("");
                
                if debug_enabled {
                    debug!("Examining item: type='{}', name='{}'", item_type, item_name);
                }

                if item_type == "file" && item_name.ends_with(".md") {
                    let name = item_name.to_string();
                    
                    if debug_enabled {
                        if !name.contains("Debug Test Page") && !name.contains("debug linked node") {
                            debug!("Skipping non-debug file in debug mode: {}", name);
                            continue;
                        }
                        debug!("Processing debug markdown file: {}", name);
                    } else {
                        debug!("Processing markdown file: {}", name);
                    }
                
                // Use the file name directly since base path is already handled
                debug!("Repository path for commits query: {}", name);
                
                // Combine with base path and get last modified time
                let full_path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", path.trim_matches('/'), name)
                };
                // Add delay between API calls within batch
                tokio::time::sleep(BATCH_DELAY).await;
                
                if debug_enabled {
                    debug!("Fetching last modified time for: {}", full_path);
                }

                // Get both content change time and any commit time
                let last_content_change = match self.get_file_content_last_modified(&full_path).await {
                    Ok(time) => {
                        if debug_enabled {
                            debug!("Got content last modified time for {}: {}", name, time);
                        }
                        Some(time)
                    },
                    Err(e) => {
                        warn!("Failed to get content modification time for {}: {}", name, e);
                        None
                    }
                };
                
                let last_modified = match self.get_file_last_modified(&full_path).await {
                    Ok(time) => {
                        if debug_enabled {
                            debug!("Got last commit time for {}: {}", name, time);
                        }
                        Some(time)
                    },
                    Err(e) => {
                        error!("Failed to get any last modified time for {}: {}", name, e);
                        if debug_enabled {
                            debug!("Using current time as fallback for {}", name);
                        }
                        Some(Utc::now())
                    }
                };

                let sha = item["sha"].as_str().unwrap_or("").to_string();
                let download_url = item["download_url"].as_str().unwrap_or("").to_string();
                
                if debug_enabled {
                    debug!("Collecting metadata - Name: {}, SHA: {}, URL: {}",
                        name, sha, download_url);
                }
                
                markdown_files.push(GitHubFileMetadata {
                    name,
                    sha: sha.clone(),
                    download_url,
                    etag: None,
                    last_checked: Some(Utc::now()),
                    last_modified,
                    last_content_change: last_content_change.or(last_modified), // Use last_modified as fallback
                    file_blob_sha: Some(sha),
                });
                }
            }
            
            // Move to next batch
            current_idx = end_idx;
            
            let batch_number = current_idx / BATCH_SIZE;
            let total_batches = (contents.len() + BATCH_SIZE - 1) / BATCH_SIZE;
            let progress = (current_idx * 100) / contents.len();
            
            // Log batch completion with detailed stats
            info!("Completed batch {}/{} - {}% complete ({} files processed)",
                batch_number,
                total_batches,
                progress,
                markdown_files.len()
            );
            
            if debug_enabled {
                let remaining_items = contents.len() - current_idx;
                let est_remaining_batches = (remaining_items + BATCH_SIZE - 1) / BATCH_SIZE;
                let est_remaining_time = est_remaining_batches as u64 * BATCH_DELAY.as_secs();
                
                debug!("Batch performance - Remaining items: {}, Est. remaining batches: {}, Est. time: {}s",
                    remaining_items,
                    est_remaining_batches,
                    est_remaining_time
                );
            }
            
            // Add delay between batches if not the last batch
            if current_idx < contents.len() {
                if debug_enabled {
                    debug!("Adding inter-batch delay of {}ms", BATCH_DELAY.as_millis());
                }
                tokio::time::sleep(BATCH_DELAY).await;
            }
        }

        if debug_enabled {
            info!("Debug mode: Processing only debug test files");
        }

        info!("Found {} markdown files in {} batches",
            markdown_files.len(),
            (contents.len() + BATCH_SIZE - 1) / BATCH_SIZE
        );
        Ok(markdown_files)
    }
}
