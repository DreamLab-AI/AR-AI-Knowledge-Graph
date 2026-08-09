use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug as trace_debug, info as trace_info};
use uuid::Uuid;

use crate::config::AppFullSettings;
use crate::utils::time;

// Global cache for user settings
static USER_SETTINGS_CACHE: Lazy<Arc<RwLock<HashMap<String, CachedUserSettings>>>> =
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

// Cache expiration time (10 minutes)
const CACHE_EXPIRATION: Duration = Duration::from_secs(10 * 60);

// Cache entry with timestamp
struct CachedUserSettings {
    settings: UserSettings,
    timestamp: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub pubkey: String,
    pub settings: AppFullSettings,
    pub last_modified: i64,
}

impl UserSettings {
    pub fn new(pubkey: &str, settings: AppFullSettings) -> Self {
        Self {
            pubkey: pubkey.to_string(),
            settings,
            last_modified: time::timestamp_seconds(),
        }
    }

    pub fn load(pubkey: &str) -> Option<Self> {
        let request_id = Uuid::new_v4();

        {
            let cache = match USER_SETTINGS_CACHE.read() {
                Ok(cache) => cache,
                Err(e) => {
                    error!("Failed to read user settings cache: {}", e);
                    return None;
                }
            };
            if let Some(cached) = cache.get(pubkey) {
                if cached.timestamp.elapsed() < CACHE_EXPIRATION {
                    debug!("Using cached settings for user {}", pubkey);
                    trace_debug!(
                        request_id = %request_id,
                        user_id = %pubkey,
                        cache_hit = true,
                        cache_age_secs = cached.timestamp.elapsed().as_secs(),
                        "Loading user settings - cache hit"
                    );
                    return Some(cached.settings.clone());
                }

                debug!("Cache expired for user {}, reloading from disk", pubkey);
                trace_debug!(
                    request_id = %request_id,
                    user_id = %pubkey,
                    cache_hit = false,
                    cache_age_secs = cached.timestamp.elapsed().as_secs(),
                    reason = "cache_expired",
                    "Cache expired, reloading from disk"
                );
            } else {
                trace_debug!(
                    request_id = %request_id,
                    user_id = %pubkey,
                    cache_hit = false,
                    reason = "not_in_cache",
                    "User not in cache, loading from disk"
                );
            }
        }

        let path = Self::get_settings_path(pubkey);
        match fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str::<UserSettings>(&content) {
                Ok(settings) => {
                    let settings_clone = settings.clone();
                    {
                        let mut cache = match USER_SETTINGS_CACHE.write() {
                            Ok(cache) => cache,
                            Err(e) => {
                                error!("Failed to write to user settings cache: {}", e);

                                return Some(settings);
                            }
                        };
                        cache.insert(
                            pubkey.to_string(),
                            CachedUserSettings {
                                settings: settings_clone,
                                timestamp: Instant::now(),
                            },
                        );
                    }
                    info!("Loaded settings for user {} and added to cache", pubkey);
                    Some(settings)
                }
                Err(e) => {
                    error!("Failed to parse settings for user {}: {}", pubkey, e);
                    None
                }
            },
            Err(e) => {
                debug!("No settings file found for user {}: {}", pubkey, e);
                None
            }
        }
    }

    /// Save user settings to cache and disk synchronously.
    /// The disk write completes before returning Ok to prevent race conditions.
    pub fn save(&self) -> Result<(), String> {
        // Update cache first
        {
            let mut cache = match USER_SETTINGS_CACHE.write() {
                Ok(cache) => cache,
                Err(e) => {
                    warn!("Failed to write to user settings cache during save: {}", e);
                    return self.save_to_disk();
                }
            };
            cache.insert(
                self.pubkey.clone(),
                CachedUserSettings {
                    settings: self.clone(),
                    timestamp: Instant::now(),
                },
            );
            debug!("Updated cache for user {}", self.pubkey);
        }

        // Write to disk synchronously (Fix #6: no fire-and-forget thread)
        self.save_to_disk()
    }

    fn save_to_disk(&self) -> Result<(), String> {
        let path = Self::get_settings_path(&self.pubkey);

        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("Failed to create settings directory: {}", e));
            }
        }

        match serde_yaml::to_string(self) {
            Ok(content) => match std::fs::write(&path, content) {
                Ok(_) => {
                    debug!("Saved settings to disk for user {}", self.pubkey);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to write settings file: {}", e)),
            },
            Err(e) => Err(format!("Failed to serialize settings: {}", e)),
        }
    }

    fn get_settings_path(pubkey: &str) -> PathBuf {
        PathBuf::from("/app/user_settings").join(format!("{}.yaml", pubkey))
    }

    pub fn clear_cache(pubkey: &str) {
        let mut cache = match USER_SETTINGS_CACHE.write() {
            Ok(cache) => cache,
            Err(e) => {
                error!(
                    "Failed to write to cache for clearing user {}: {}",
                    pubkey, e
                );
                return;
            }
        };
        if cache.remove(pubkey).is_some() {
            debug!("Cleared cache for user {}", pubkey);
            trace_info!(
                user_id = %pubkey,
                "User settings cache invalidated"
            );
        }
    }

    pub fn clear_all_cache() {
        let mut cache = match USER_SETTINGS_CACHE.write() {
            Ok(cache) => cache,
            Err(e) => {
                error!("Failed to write to cache for clearing all settings: {}", e);
                return;
            }
        };
        let count = cache.len();
        cache.clear();
        debug!("Cleared all cached settings ({} entries)", count);
        trace_info!(entries_cleared = count, "All user settings cache cleared");
    }

    pub fn invalidate_user_cache(pubkey: &str) {
        Self::clear_cache(pubkey);
        trace_info!(
            user_id = %pubkey,
            "User cache invalidated due to auth state change"
        );
    }

    pub fn get_cache_stats() -> (usize, Vec<(String, Duration)>) {
        let cache = match USER_SETTINGS_CACHE.read() {
            Ok(cache) => cache,
            Err(_) => return (0, Vec::new()),
        };

        let entries = cache.len();
        let ages: Vec<(String, Duration)> = cache
            .iter()
            .map(|(key, value)| (key.clone(), value.timestamp.elapsed()))
            .collect();

        (entries, ages)
    }
}
