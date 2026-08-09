// src/services/ontology_file_cache.rs
//! Ontology File Cache with LRU eviction
//!
//! Caches parsed ontology data to avoid re-parsing files unnecessarily.
//! Similar to Python OntologyLoader's LRU cache pattern.

use crate::services::github::types::OntologyFileMetadata;
use crate::services::ontology_content_analyzer::ContentAnalysis;
use chrono::{DateTime, Utc};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for ontology file cache
#[derive(Debug, Clone)]
pub struct OntologyCacheConfig {
    /// Maximum number of cached files
    pub max_entries: usize,

    /// Whether to enable cache statistics
    pub enable_stats: bool,
}

impl Default for OntologyCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 500, // Cache up to 500 ontology files
            enable_stats: true,
        }
    }
}

/// Cached ontology file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOntologyFile {
    /// File metadata
    pub metadata: OntologyFileMetadata,

    /// Content analysis results
    pub analysis: ContentAnalysis,

    /// SHA1 hash for cache invalidation
    pub content_sha: String,

    /// Cached at timestamp
    pub cached_at: DateTime<Utc>,

    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,

    /// Access count
    pub access_count: u64,
}

impl CachedOntologyFile {
    pub fn new(
        metadata: OntologyFileMetadata,
        analysis: ContentAnalysis,
        content_sha: String,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            metadata,
            analysis,
            content_sha,
            cached_at: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// Check if cache entry is still valid for given SHA
    pub fn is_valid_for(&self, sha: &str) -> bool {
        self.content_sha == sha
    }

    /// Update access statistics
    pub fn touch(&mut self) {
        self.last_accessed = chrono::Utc::now();
        self.access_count += 1;
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OntologyCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub evictions: u64,
    pub current_size: usize,
    pub max_size: usize,
}

impl OntologyCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// LRU cache for ontology files
pub struct OntologyFileCache {
    /// LRU cache storage
    cache: Arc<RwLock<LruCache<String, CachedOntologyFile>>>,

    /// Configuration
    config: OntologyCacheConfig,

    /// Statistics
    stats: Arc<RwLock<OntologyCacheStats>>,
}

impl OntologyFileCache {
    pub fn new(config: OntologyCacheConfig) -> Self {
        let capacity =
            NonZeroUsize::new(config.max_entries.max(1)).expect("max(1) guarantees non-zero");
        let cache = Arc::new(RwLock::new(LruCache::new(capacity)));

        let stats = Arc::new(RwLock::new(OntologyCacheStats {
            max_size: config.max_entries,
            ..Default::default()
        }));

        Self {
            cache,
            config,
            stats,
        }
    }

    /// Get cached ontology file if valid
    pub async fn get(&self, file_path: &str, current_sha: &str) -> Option<CachedOntologyFile> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = cache.get_mut(file_path) {
            if entry.is_valid_for(current_sha) {
                entry.touch();
                stats.hits += 1;
                return Some(entry.clone());
            } else {
                // SHA mismatch - invalidate
                cache.pop(file_path);
                stats.invalidations += 1;
            }
        }

        stats.misses += 1;
        None
    }

    /// Put ontology file into cache
    pub async fn put(&self, file_path: String, entry: CachedOntologyFile) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        // Track eviction if cache is full
        if cache.len() >= self.config.max_entries && !cache.contains(&file_path) {
            stats.evictions += 1;
        }

        cache.put(file_path, entry);
        stats.current_size = cache.len();
    }

    /// Invalidate specific file
    pub async fn invalidate(&self, file_path: &str) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if cache.pop(file_path).is_some() {
            stats.invalidations += 1;
            stats.current_size = cache.len();
        }
    }

    /// Clear entire cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        let count = cache.len();
        cache.clear();
        stats.current_size = 0;
        stats.invalidations += count as u64;
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> OntologyCacheStats {
        self.stats.read().await.clone()
    }

    /// Get all cached file paths
    pub async fn get_cached_paths(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        cache.iter().map(|(path, _)| path.clone()).collect()
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Check if file is cached
    pub async fn contains(&self, file_path: &str) -> bool {
        let cache = self.cache.read().await;
        cache.contains(file_path)
    }

    /// Get cache entries sorted by priority
    pub async fn get_by_priority(&self) -> Vec<(String, CachedOntologyFile)> {
        let cache = self.cache.read().await;
        let mut entries: Vec<(String, CachedOntologyFile)> =
            cache.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        // Sort by priority (Priority1 first, then Priority2, etc.)
        entries.sort_by(|a, b| a.1.metadata.priority.cmp(&b.1.metadata.priority));

        entries
    }
}

impl Default for OntologyFileCache {
    fn default() -> Self {
        Self::new(OntologyCacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::github::types::{GitHubFileBasicMetadata, OntologyPriority};

    fn create_test_entry(sha: &str) -> CachedOntologyFile {
        let basic = GitHubFileBasicMetadata {
            name: "test.md".to_string(),
            path: "pages/test.md".to_string(),
            sha: sha.to_string(),
            size: 1024,
            download_url: "http://example.com/test.md".to_string(),
        };

        let mut metadata = OntologyFileMetadata::new(basic);
        metadata.has_public_flag = true;
        metadata.has_ontology_block = true;
        metadata.calculate_priority();

        let analysis = ContentAnalysis::default();

        CachedOntologyFile::new(metadata, analysis, sha.to_string())
    }

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let cache = OntologyFileCache::default();
        let entry = create_test_entry("sha123");

        cache.put("test.md".to_string(), entry.clone()).await;

        let retrieved = cache.get("test.md", "sha123").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_cache_invalidation_on_sha_mismatch() {
        let cache = OntologyFileCache::default();
        let entry = create_test_entry("sha123");

        cache.put("test.md".to_string(), entry).await;

        // Try to get with different SHA
        let retrieved = cache.get("test.md", "sha456").await;
        assert!(retrieved.is_none());

        // Check stats
        let stats = cache.get_stats().await;
        assert_eq!(stats.invalidations, 1);
    }

    #[tokio::test]
    async fn test_cache_hit_rate() {
        let cache = OntologyFileCache::default();
        let entry = create_test_entry("sha123");

        cache.put("test.md".to_string(), entry).await;

        // Hit
        cache.get("test.md", "sha123").await;

        // Miss
        cache.get("other.md", "sha456").await;

        let stats = cache.get_stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let config = OntologyCacheConfig {
            max_entries: 2,
            enable_stats: true,
        };

        let cache = OntologyFileCache::new(config);

        cache
            .put("file1.md".to_string(), create_test_entry("sha1"))
            .await;
        cache
            .put("file2.md".to_string(), create_test_entry("sha2"))
            .await;

        // This should evict file1
        cache
            .put("file3.md".to_string(), create_test_entry("sha3"))
            .await;

        let stats = cache.get_stats().await;
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.current_size, 2);
    }

    #[tokio::test]
    async fn test_get_by_priority() {
        let cache = OntologyFileCache::default();

        // Priority 1
        let mut entry1 = create_test_entry("sha1");
        entry1.metadata.priority = OntologyPriority::Priority1;

        // Priority 2
        let mut entry2 = create_test_entry("sha2");
        entry2.metadata.priority = OntologyPriority::Priority2;

        cache.put("file1.md".to_string(), entry1).await;
        cache.put("file2.md".to_string(), entry2).await;

        let sorted = cache.get_by_priority().await;
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].1.metadata.priority, OntologyPriority::Priority1);
        assert_eq!(sorted[1].1.metadata.priority, OntologyPriority::Priority2);
    }
}
