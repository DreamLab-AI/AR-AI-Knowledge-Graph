use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::{HealthStatus, ServiceHealthInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DegradationLevel {
    Normal,

    Degraded,

    SeverelyDegraded,

    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DegradationStrategy {
    UseCachedData { max_age: Duration },

    UseFallbackService { fallback_service: String },

    ReducedFunctionality { features_disabled: Vec<String> },

    UseDefaults,

    QueueRequests { max_queue_size: usize },

    FailFast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracefulDegradationConfig {
    pub strategies: HashMap<DegradationLevel, Vec<DegradationStrategy>>,

    pub degradation_threshold: f64,

    pub recovery_threshold: f64,

    pub health_window: Duration,

    pub auto_degrade: bool,
}

impl Default for GracefulDegradationConfig {
    fn default() -> Self {
        let mut strategies = HashMap::new();

        strategies.insert(
            DegradationLevel::Degraded,
            vec![DegradationStrategy::UseCachedData {
                max_age: Duration::from_secs(300),
            }],
        );

        strategies.insert(
            DegradationLevel::SeverelyDegraded,
            vec![
                DegradationStrategy::UseCachedData {
                    max_age: Duration::from_secs(600),
                },
                DegradationStrategy::ReducedFunctionality {
                    features_disabled: vec!["non-essential".to_string()],
                },
            ],
        );

        strategies.insert(
            DegradationLevel::Unavailable,
            vec![
                DegradationStrategy::UseDefaults,
                DegradationStrategy::FailFast,
            ],
        );

        Self {
            strategies,
            degradation_threshold: 0.7,
            recovery_threshold: 0.9,
            health_window: Duration::from_secs(300),
            auto_degrade: true,
        }
    }
}

pub struct GracefulDegradationManager {
    config: GracefulDegradationConfig,
    service_levels: Arc<RwLock<HashMap<String, DegradationLevel>>>,
    cached_data: Arc<RwLock<HashMap<String, CachedResponse>>>,
    request_queues: Arc<RwLock<HashMap<String, Vec<QueuedRequest>>>>,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    data: serde_json::Value,
    timestamp: Instant,
    ttl: Duration,
}

#[derive(Debug)]
struct QueuedRequest {
    id: String,
    #[allow(dead_code)]
    data: serde_json::Value,
    #[allow(dead_code)]
    timestamp: Instant,
    #[allow(dead_code)]
    callback: Option<tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>>,
}

impl GracefulDegradationManager {
    pub fn new(config: GracefulDegradationConfig) -> Self {
        Self {
            config,
            service_levels: Arc::new(RwLock::new(HashMap::new())),
            cached_data: Arc::new(RwLock::new(HashMap::new())),
            request_queues: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_service_health(&self, service_name: &str, health: &ServiceHealthInfo) {
        let new_level = self.calculate_degradation_level(health);
        let mut levels = self.service_levels.write().await;

        if let Some(current_level) = levels.get(service_name) {
            if *current_level != new_level {
                info!(
                    "Service {} degradation level changed from {:?} to {:?}",
                    service_name, current_level, new_level
                );

                self.apply_degradation_strategies(service_name, new_level)
                    .await;
            }
        } else {
            info!(
                "Setting initial degradation level for service {}: {:?}",
                service_name, new_level
            );
        }

        levels.insert(service_name.to_string(), new_level);
    }

    pub async fn get_degradation_level(&self, service_name: &str) -> DegradationLevel {
        self.service_levels
            .read()
            .await
            .get(service_name)
            .copied()
            .unwrap_or(DegradationLevel::Normal)
    }

    pub async fn execute_with_degradation<T>(
        &self,
        service_name: &str,
        request_key: &str,
        operation: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String>
    where
        T: serde::Serialize + serde::de::DeserializeOwned + Clone + Default,
    {
        let degradation_level = self.get_degradation_level(service_name).await;

        match degradation_level {
            DegradationLevel::Normal => match operation.await {
                Ok(result) => {
                    self.cache_response(request_key, &result).await;
                    Ok(result)
                }
                Err(e) => Err(e),
            },
            DegradationLevel::Degraded | DegradationLevel::SeverelyDegraded => {
                match operation.await {
                    Ok(result) => {
                        self.cache_response(request_key, &result).await;
                        Ok(result)
                    }
                    Err(_) => {
                        self.apply_fallback_strategies(service_name, request_key, degradation_level)
                            .await
                    }
                }
            }
            DegradationLevel::Unavailable => {
                self.apply_fallback_strategies(service_name, request_key, degradation_level)
                    .await
            }
        }
    }

    pub async fn cache_response<T>(&self, key: &str, data: &T)
    where
        T: serde::Serialize,
    {
        if let Ok(json_data) = serde_json::to_value(data) {
            let cached = CachedResponse {
                data: json_data,
                timestamp: Instant::now(),
                ttl: Duration::from_secs(300),
            };

            self.cached_data
                .write()
                .await
                .insert(key.to_string(), cached);
            debug!("Cached response for key: {}", key);
        }
    }

    pub async fn get_cached_response<T>(&self, key: &str, max_age: Duration) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let cache = self.cached_data.read().await;

        if let Some(cached) = cache.get(key) {
            if cached.timestamp.elapsed() <= max_age {
                if let Ok(data) = serde_json::from_value(cached.data.clone()) {
                    debug!("Retrieved cached response for key: {}", key);
                    return Some(data);
                }
            } else {
                debug!("Cached response for key {} is too old", key);
            }
        }

        None
    }

    pub async fn queue_request(
        &self,
        service_name: &str,
        request_id: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        let mut queues = self.request_queues.write().await;
        let queue = queues
            .entry(service_name.to_string())
            .or_insert_with(Vec::new);

        if let Some(strategies) = self.config.strategies.get(&DegradationLevel::Degraded) {
            for strategy in strategies {
                if let DegradationStrategy::QueueRequests { max_queue_size } = strategy {
                    if queue.len() >= *max_queue_size {
                        warn!("Request queue for service {} is full", service_name);
                        return Err("Request queue is full".to_string());
                    }
                }
            }
        }

        let request = QueuedRequest {
            id: request_id.clone(),
            data,
            timestamp: Instant::now(),
            callback: None,
        };

        queue.push(request);
        info!("Queued request {} for service {}", request_id, service_name);
        Ok(())
    }

    pub async fn process_queued_requests(&self, service_name: &str) {
        let mut queues = self.request_queues.write().await;

        if let Some(queue) = queues.remove(service_name) {
            info!(
                "Processing {} queued requests for service {}",
                queue.len(),
                service_name
            );

            for request in queue {
                debug!("Processing queued request: {}", request.id);
            }
        }
    }

    pub async fn get_degradation_stats(&self) -> HashMap<String, DegradationLevel> {
        self.service_levels.read().await.clone()
    }

    pub async fn cleanup(&self) {
        {
            let mut cache = self.cached_data.write().await;
            cache.retain(|key, cached| {
                let expired = cached.timestamp.elapsed() > cached.ttl;
                if expired {
                    debug!("Removing expired cache entry: {}", key);
                }
                !expired
            });
        }

        {
            let mut queues = self.request_queues.write().await;
            for (service_name, queue) in queues.iter_mut() {
                let original_len = queue.len();
                queue.retain(|req| req.timestamp.elapsed() < Duration::from_secs(3600));

                if queue.len() != original_len {
                    debug!(
                        "Cleaned up {} expired requests for service {}",
                        original_len - queue.len(),
                        service_name
                    );
                }
            }
        }
    }

    fn calculate_degradation_level(&self, health: &ServiceHealthInfo) -> DegradationLevel {
        match health.current_status {
            HealthStatus::Healthy => {
                if health.uptime_percentage >= self.config.recovery_threshold * 100.0 {
                    DegradationLevel::Normal
                } else {
                    DegradationLevel::Degraded
                }
            }
            HealthStatus::Degraded => DegradationLevel::Degraded,
            HealthStatus::Unhealthy => {
                if health.uptime_percentage < self.config.degradation_threshold * 100.0 {
                    DegradationLevel::Unavailable
                } else {
                    DegradationLevel::SeverelyDegraded
                }
            }
            HealthStatus::Unknown => DegradationLevel::Degraded,
        }
    }

    async fn apply_degradation_strategies(&self, service_name: &str, level: DegradationLevel) {
        if let Some(strategies) = self.config.strategies.get(&level) {
            for strategy in strategies {
                match strategy {
                    DegradationStrategy::QueueRequests { max_queue_size: _ } => {
                        info!("Enabling request queueing for service: {}", service_name);
                    }
                    DegradationStrategy::ReducedFunctionality { features_disabled } => {
                        info!(
                            "Disabling features for service {}: {:?}",
                            service_name, features_disabled
                        );
                    }
                    _ => {
                        debug!("Applied degradation strategy: {:?}", strategy);
                    }
                }
            }
        }
    }

    async fn apply_fallback_strategies<T>(
        &self,
        service_name: &str,
        request_key: &str,
        level: DegradationLevel,
    ) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        if let Some(strategies) = self.config.strategies.get(&level) {
            for strategy in strategies {
                match strategy {
                    DegradationStrategy::UseCachedData { max_age } => {
                        if let Some(cached) = self.get_cached_response(request_key, *max_age).await
                        {
                            info!("Using cached data for service: {}", service_name);
                            return Ok(cached);
                        }
                    }
                    DegradationStrategy::UseDefaults => {
                        info!("Using default response for service: {}", service_name);
                        return Ok(T::default());
                    }
                    DegradationStrategy::FailFast => {
                        warn!(
                            "Fast-failing request for unavailable service: {}",
                            service_name
                        );
                        return Err(format!("Service {} is unavailable", service_name));
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }

        Err(format!(
            "No fallback strategy available for service: {}",
            service_name
        ))
    }
}

impl Default for GracefulDegradationManager {
    fn default() -> Self {
        Self::new(GracefulDegradationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_degradation_manager_creation() {
        let manager = GracefulDegradationManager::default();
        let level = manager.get_degradation_level("test-service").await;
        assert_eq!(level, DegradationLevel::Normal);
    }

    #[tokio::test]
    async fn test_cache_and_retrieve() {
        let manager = GracefulDegradationManager::default();
        let test_data = "test response";

        manager.cache_response("test-key", &test_data).await;

        let cached: Option<String> = manager
            .get_cached_response("test-key", Duration::from_secs(60))
            .await;

        assert_eq!(cached, Some(test_data.to_string()));
    }

    #[tokio::test]
    async fn test_queue_request() {
        let manager = GracefulDegradationManager::default();
        let request_data = serde_json::json!({"test": "data"});

        let result = manager
            .queue_request("test-service", "req-1".to_string(), request_data)
            .await;

        assert!(result.is_ok());
    }
}
