// src/application/settings/queries.rs
//! Settings Domain - Read Operations (Queries)
//!
//! All queries for reading settings state following CQRS patterns.

use hexser::{HexResult, Hexserror, QueryHandler};
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::{AppFullSettings, PhysicsSettings};
use crate::ports::settings_repository::{SettingValue, SettingsRepository};

// ============================================================================
// GET SETTING
// ============================================================================

#[derive(Debug, Clone)]
pub struct GetSetting {
    pub key: String,
}

pub struct GetSettingHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl GetSettingHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl QueryHandler<GetSetting, Option<SettingValue>> for GetSettingHandler {
    fn handle(&self, query: GetSetting) -> HexResult<Option<SettingValue>> {
        log::debug!("Executing GetSetting query: key={}", query.key);

        let repository = self.repository.clone();
        let key = query.key.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.get_setting(&key).await.map_err(|e| {
                Hexserror::adapter("E_HEX_200", &format!("Failed to get setting: {}", e))
            })
        })
    }
}

// ============================================================================
// GET SETTINGS BATCH
// ============================================================================

#[derive(Debug, Clone)]
pub struct GetSettingsBatch {
    pub keys: Vec<String>,
}

pub struct GetSettingsBatchHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl GetSettingsBatchHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl QueryHandler<GetSettingsBatch, HashMap<String, SettingValue>> for GetSettingsBatchHandler {
    fn handle(&self, query: GetSettingsBatch) -> HexResult<HashMap<String, SettingValue>> {
        log::debug!(
            "Executing GetSettingsBatch query: {} keys",
            query.keys.len()
        );

        let repository = self.repository.clone();
        let keys = query.keys.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.get_settings_batch(&keys).await.map_err(|e| {
                Hexserror::adapter("E_HEX_200", &format!("Failed to get settings batch: {}", e))
            })
        })
    }
}

// ============================================================================
// LOAD ALL SETTINGS
// ============================================================================

#[derive(Debug, Clone)]
pub struct LoadAllSettings;

pub struct LoadAllSettingsHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl LoadAllSettingsHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl QueryHandler<LoadAllSettings, Option<AppFullSettings>> for LoadAllSettingsHandler {
    fn handle(&self, _query: LoadAllSettings) -> HexResult<Option<AppFullSettings>> {
        log::debug!("Executing LoadAllSettings query");

        let repository = self.repository.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.load_all_settings().await.map_err(|e| {
                Hexserror::adapter("E_HEX_200", &format!("Failed to load all settings: {}", e))
            })
        })
    }
}

// ============================================================================
// GET PHYSICS SETTINGS
// ============================================================================

#[derive(Debug, Clone)]
pub struct GetPhysicsSettings {
    pub profile_name: String,
}

pub struct GetPhysicsSettingsHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl GetPhysicsSettingsHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl QueryHandler<GetPhysicsSettings, PhysicsSettings> for GetPhysicsSettingsHandler {
    fn handle(&self, query: GetPhysicsSettings) -> HexResult<PhysicsSettings> {
        log::debug!(
            "Executing GetPhysicsSettings query: profile={}",
            query.profile_name
        );

        let repository = self.repository.clone();
        let profile_name = query.profile_name.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository
                .get_physics_settings(&profile_name)
                .await
                .map_err(|e| {
                    Hexserror::adapter(
                        "E_HEX_200",
                        &format!("Failed to get physics settings: {}", e),
                    )
                })
        })
    }
}

// ============================================================================
// LIST PHYSICS PROFILES
// ============================================================================

#[derive(Debug, Clone)]
pub struct ListPhysicsProfiles;

pub struct ListPhysicsProfilesHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl ListPhysicsProfilesHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl QueryHandler<ListPhysicsProfiles, Vec<String>> for ListPhysicsProfilesHandler {
    fn handle(&self, _query: ListPhysicsProfiles) -> HexResult<Vec<String>> {
        log::debug!("Executing ListPhysicsProfiles query");

        let repository = self.repository.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.list_physics_profiles().await.map_err(|e| {
                Hexserror::adapter(
                    "E_HEX_200",
                    &format!("Failed to list physics profiles: {}", e),
                )
            })
        })
    }
}
