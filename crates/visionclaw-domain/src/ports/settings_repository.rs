// visionclaw-domain/src/ports/settings_repository.rs
//! Settings Repository Port
//!
//! Provides access to application, user, and developer configuration settings.
//! This port abstracts database operations for all settings management.
//! Promoted from webxr/src/ports/ per ADR-090 Phase A6 slice 3.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::AppFullSettings;
use crate::types::physics_config::PhysicsSettings;

pub type Result<T> = std::result::Result<T, SettingsRepositoryError>;

#[derive(Debug, thiserror::Error)]
pub enum SettingsRepositoryError {
    #[error("Setting not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Cache error: {0}")]
    CacheError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SettingValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
}

impl SettingValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            SettingValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SettingValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SettingValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SettingValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            SettingValue::Json(j) => Some(j),
            _ => None,
        }
    }
}

#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn get_setting(&self, key: &str) -> Result<Option<SettingValue>>;

    async fn set_setting(
        &self,
        key: &str,
        value: SettingValue,
        description: Option<&str>,
    ) -> Result<()>;

    async fn delete_setting(&self, key: &str) -> Result<()>;

    async fn has_setting(&self, key: &str) -> Result<bool>;

    async fn get_settings_batch(&self, keys: &[String]) -> Result<HashMap<String, SettingValue>>;

    async fn set_settings_batch(&self, updates: HashMap<String, SettingValue>) -> Result<()>;

    async fn list_settings(&self, prefix: Option<&str>) -> Result<Vec<String>>;

    async fn load_all_settings(&self) -> Result<Option<AppFullSettings>>;

    async fn save_all_settings(&self, settings: &AppFullSettings) -> Result<()>;

    async fn get_physics_settings(&self, profile_name: &str) -> Result<PhysicsSettings>;

    async fn save_physics_settings(
        &self,
        profile_name: &str,
        settings: &PhysicsSettings,
    ) -> Result<()>;

    async fn list_physics_profiles(&self) -> Result<Vec<String>>;

    async fn delete_physics_profile(&self, profile_name: &str) -> Result<()>;

    async fn export_settings(&self) -> Result<serde_json::Value>;

    async fn import_settings(&self, settings_json: &serde_json::Value) -> Result<()>;

    async fn clear_cache(&self) -> Result<()>;

    async fn health_check(&self) -> Result<bool>;
}
