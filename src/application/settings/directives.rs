// src/application/settings/directives.rs
//! Settings Domain - Write Operations (Directives)
//!
//! All directives for modifying settings state following CQRS patterns.

use hexser::{Directive, DirectiveHandler, HexResult, Hexserror};
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::{AppFullSettings, PhysicsSettings};
use crate::ports::settings_repository::{SettingValue, SettingsRepository};

// ============================================================================
// UPDATE SETTING
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdateSetting {
    pub key: String,
    pub value: SettingValue,
    pub description: Option<String>,
}

impl Directive for UpdateSetting {
    fn validate(&self) -> HexResult<()> {
        if self.key.is_empty() {
            return Err(Hexserror::validation("Setting key cannot be empty"));
        }
        Ok(())
    }
}

pub struct UpdateSettingHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl UpdateSettingHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<UpdateSetting> for UpdateSettingHandler {
    fn handle(&self, directive: UpdateSetting) -> HexResult<()> {
        log::info!("Executing UpdateSetting directive: key={}", directive.key);

        let repository = self.repository.clone();
        let key = directive.key.clone();
        let value = directive.value.clone();
        let description = directive.description.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository
                .set_setting(&key, value, description.as_deref())
                .await
                .map_err(|e| {
                    Hexserror::adapter("E_HEX_200", &format!("Failed to update setting: {}", e))
                })?;

            log::info!("Setting '{}' updated successfully", key);
            Ok(())
        })
    }
}

// ============================================================================
// UPDATE SETTINGS BATCH
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdateSettingsBatch {
    pub updates: HashMap<String, SettingValue>,
}

impl Directive for UpdateSettingsBatch {
    fn validate(&self) -> HexResult<()> {
        if self.updates.is_empty() {
            return Err(Hexserror::validation(
                "Cannot update empty batch of settings",
            ));
        }
        Ok(())
    }
}

pub struct UpdateSettingsBatchHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl UpdateSettingsBatchHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<UpdateSettingsBatch> for UpdateSettingsBatchHandler {
    fn handle(&self, directive: UpdateSettingsBatch) -> HexResult<()> {
        log::info!(
            "Executing UpdateSettingsBatch directive: {} updates",
            directive.updates.len()
        );

        let repository = self.repository.clone();
        let updates = directive.updates.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.set_settings_batch(updates).await.map_err(|e| {
                Hexserror::adapter(
                    "E_HEX_200",
                    &format!("Failed to update settings batch: {}", e),
                )
            })?;

            log::info!("Settings batch updated successfully");
            Ok(())
        })
    }
}

// ============================================================================
// SAVE ALL SETTINGS
// ============================================================================

#[derive(Debug, Clone)]
pub struct SaveAllSettings {
    pub settings: AppFullSettings,
}

impl Directive for SaveAllSettings {
    fn validate(&self) -> HexResult<()> {
        Ok(())
    }
}

pub struct SaveAllSettingsHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl SaveAllSettingsHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<SaveAllSettings> for SaveAllSettingsHandler {
    fn handle(&self, directive: SaveAllSettings) -> HexResult<()> {
        log::info!("Executing SaveAllSettings directive");

        let repository = self.repository.clone();
        let settings = directive.settings.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.save_all_settings(&settings).await.map_err(|e| {
                Hexserror::adapter("E_HEX_200", &format!("Failed to save all settings: {}", e))
            })?;

            log::info!("All settings saved successfully");
            Ok(())
        })
    }
}

// ============================================================================
// UPDATE PHYSICS SETTINGS
// ============================================================================

#[derive(Debug, Clone)]
pub struct UpdatePhysicsSettings {
    pub profile_name: String,
    pub settings: PhysicsSettings,
}

impl Directive for UpdatePhysicsSettings {
    fn validate(&self) -> HexResult<()> {
        if self.profile_name.is_empty() {
            return Err(Hexserror::validation(
                "Physics profile name cannot be empty",
            ));
        }
        Ok(())
    }
}

pub struct UpdatePhysicsSettingsHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl UpdatePhysicsSettingsHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<UpdatePhysicsSettings> for UpdatePhysicsSettingsHandler {
    fn handle(&self, directive: UpdatePhysicsSettings) -> HexResult<()> {
        log::info!(
            "Executing UpdatePhysicsSettings directive: profile={}",
            directive.profile_name
        );

        let repository = self.repository.clone();
        let profile_name = directive.profile_name.clone();
        let settings = directive.settings.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository
                .save_physics_settings(&profile_name, &settings)
                .await
                .map_err(|e| {
                    Hexserror::adapter(
                        "E_HEX_200",
                        &format!("Failed to update physics settings: {}", e),
                    )
                })?;

            log::info!(
                "Physics settings for profile '{}' updated successfully",
                profile_name
            );
            Ok(())
        })
    }
}

// ============================================================================
// DELETE PHYSICS PROFILE
// ============================================================================

#[derive(Debug, Clone)]
pub struct DeletePhysicsProfile {
    pub profile_name: String,
}

impl Directive for DeletePhysicsProfile {
    fn validate(&self) -> HexResult<()> {
        if self.profile_name.is_empty() {
            return Err(Hexserror::validation("Profile name cannot be empty"));
        }
        Ok(())
    }
}

pub struct DeletePhysicsProfileHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl DeletePhysicsProfileHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<DeletePhysicsProfile> for DeletePhysicsProfileHandler {
    fn handle(&self, directive: DeletePhysicsProfile) -> HexResult<()> {
        log::info!(
            "Executing DeletePhysicsProfile directive: profile={}",
            directive.profile_name
        );

        let repository = self.repository.clone();
        let profile_name = directive.profile_name.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository
                .delete_physics_profile(&profile_name)
                .await
                .map_err(|e| {
                    Hexserror::adapter(
                        "E_HEX_200",
                        &format!("Failed to delete physics profile: {}", e),
                    )
                })?;

            log::info!("Physics profile '{}' deleted successfully", profile_name);
            Ok(())
        })
    }
}

// ============================================================================
// CLEAR SETTINGS CACHE
// ============================================================================

#[derive(Debug, Clone)]
pub struct ClearSettingsCache;

impl Directive for ClearSettingsCache {
    fn validate(&self) -> HexResult<()> {
        Ok(())
    }
}

pub struct ClearSettingsCacheHandler {
    repository: Arc<dyn SettingsRepository>,
}

impl ClearSettingsCacheHandler {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
}

impl DirectiveHandler<ClearSettingsCache> for ClearSettingsCacheHandler {
    fn handle(&self, _directive: ClearSettingsCache) -> HexResult<()> {
        log::info!("Executing ClearSettingsCache directive");

        let repository = self.repository.clone();

        tokio::runtime::Handle::current().block_on(async move {
            repository.clear_cache().await.map_err(|e| {
                Hexserror::adapter("E_HEX_200", &format!("Failed to clear cache: {}", e))
            })?;

            log::info!("Settings cache cleared successfully");
            Ok(())
        })
    }
}
