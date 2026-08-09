// src/inference/cache.rs
//! Inference Caching System
//!
//! LRU cache for inference results with TTL support and database persistence.
//! Automatically invalidates cache on ontology changes.

use chrono::{DateTime, Duration, Utc};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::utils::time;
use visionclaw_domain::ports::ontology_repository::InferenceResults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_entries: usize,

    pub ttl_seconds: i64,

    pub persist_to_db: bool,

    pub enable_stats: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl_seconds: 3600,
            persist_to_db: true,
            enable_stats: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub results: InferenceResults,

    pub ontology_checksum: String,

    pub created_at: DateTime<Utc>,

    pub accessed_at: DateTime<Utc>,

    pub access_count: u64,
}

impl CacheEntry {
    pub fn is_expired(&self, ttl: Duration) -> bool {
        time::now() - self.created_at > ttl
    }

    pub fn is_valid_for(&self, checksum: &str) -> bool {
        self.ontology_checksum == checksum
    }

    pub fn touch(&mut self) {
        self.accessed_at = time::now();
        self.access_count += 1;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub evictions: u64,
    pub current_size: usize,
    pub max_size: usize,
}

impl CacheStatistics {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

pub struct InferenceCache {
    cache: Arc<RwLock<LruCache<String, CacheEntry>>>,

    config: CacheConfig,

    stats: Arc<RwLock<CacheStatistics>>,
}

impl InferenceCache {
    pub fn new(config: CacheConfig) -> Self {
        let capacity =
            NonZeroUsize::new(config.max_entries.max(1)).expect("max(1) guarantees non-zero");
        let cache = Arc::new(RwLock::new(LruCache::new(capacity)));

        let stats = Arc::new(RwLock::new(CacheStatistics {
            max_size: config.max_entries,
            ..Default::default()
        }));

        Self {
            cache,
            config,
            stats,
        }
    }

    pub async fn get(&self, ontology_id: &str, checksum: &str) -> Option<InferenceResults> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = cache.get_mut(ontology_id) {
            let ttl = Duration::seconds(self.config.ttl_seconds);

            if entry.is_valid_for(checksum) && !entry.is_expired(ttl) {
                entry.touch();
                stats.hits += 1;
                return Some(entry.results.clone());
            } else {
                cache.pop(ontology_id);
                stats.invalidations += 1;
            }
        }

        stats.misses += 1;
        None
    }

    pub async fn put(&self, ontology_id: String, checksum: String, results: InferenceResults) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        let entry = CacheEntry {
            results,
            ontology_checksum: checksum,
            created_at: time::now(),
            accessed_at: time::now(),
            access_count: 0,
        };

        if cache.len() >= self.config.max_entries && !cache.contains(&ontology_id) {
            stats.evictions += 1;
        }

        cache.put(ontology_id, entry);
        stats.current_size = cache.len();
    }

    pub async fn invalidate(&self, ontology_id: &str) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        if cache.pop(ontology_id).is_some() {
            stats.invalidations += 1;
            stats.current_size = cache.len();
        }
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        cache.clear();
        stats.current_size = 0;
        stats.invalidations += cache.len() as u64;
    }

    pub async fn get_statistics(&self) -> CacheStatistics {
        self.stats.read().await.clone()
    }

    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        let ttl = Duration::seconds(self.config.ttl_seconds);

        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired(ttl))
            .map(|(key, _)| key.clone())
            .collect();

        let mut stats = self.stats.write().await;
        for key in expired_keys {
            cache.pop(&key);
            stats.invalidations += 1;
        }

        stats.current_size = cache.len();
    }

    pub async fn get_cached_ids(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        cache.iter().map(|(key, _)| key.clone()).collect()
    }
}

impl Default for InferenceCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use visionclaw_domain::ports::ontology_repository::InferenceResults;

    fn create_test_results() -> InferenceResults {
        InferenceResults {
            timestamp: time::now(),
            inferred_axioms: Vec::new(),
            inference_time_ms: 100,
            reasoner_version: "test-1.0".to_string(),
        }
    }

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let cache = InferenceCache::default();
        let results = create_test_results();

        cache
            .put("ont1".to_string(), "checksum1".to_string(), results.clone())
            .await;

        let retrieved = cache.get("ont1", "checksum1").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = InferenceCache::default();
        let retrieved = cache.get("nonexistent", "checksum").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cache_invalidation_on_checksum_mismatch() {
        let cache = InferenceCache::default();
        let results = create_test_results();

        cache
            .put("ont1".to_string(), "checksum1".to_string(), results.clone())
            .await;

        let retrieved = cache.get("ont1", "checksum2").await;
        assert!(retrieved.is_none());

        let stats = cache.get_statistics().await;
        assert_eq!(stats.invalidations, 1);
    }

    #[tokio::test]
    async fn test_cache_invalidate() {
        let cache = InferenceCache::default();
        let results = create_test_results();

        cache
            .put("ont1".to_string(), "checksum1".to_string(), results.clone())
            .await;

        cache.invalidate("ont1").await;

        let retrieved = cache.get("ont1", "checksum1").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = InferenceCache::default();
        let results = create_test_results();

        cache
            .put("ont1".to_string(), "checksum1".to_string(), results.clone())
            .await;
        cache
            .put("ont2".to_string(), "checksum2".to_string(), results.clone())
            .await;

        cache.clear().await;

        let stats = cache.get_statistics().await;
        assert_eq!(stats.current_size, 0);
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let cache = InferenceCache::default();
        let results = create_test_results();

        cache
            .put("ont1".to_string(), "checksum1".to_string(), results.clone())
            .await;

        cache.get("ont1", "checksum1").await;

        cache.get("ont2", "checksum2").await;

        let stats = cache.get_statistics().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!(stats.hit_rate() > 0.0);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let config = CacheConfig {
            max_entries: 2,
            ..Default::default()
        };

        let cache = InferenceCache::new(config);
        let results = create_test_results();

        cache
            .put("ont1".to_string(), "checksum1".to_string(), results.clone())
            .await;
        cache
            .put("ont2".to_string(), "checksum2".to_string(), results.clone())
            .await;

        cache
            .put("ont3".to_string(), "checksum3".to_string(), results.clone())
            .await;

        let stats = cache.get_statistics().await;
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.current_size, 2);
    }
}
