// src/settings/settings_actor.rs
//! Settings Actor - Actix runtime settings management

use super::models::{
    AllSettings, ConstraintSettings, NodeFilterSettings, QualityGateSettings, SettingsProfile,
};
use crate::config::{PhysicsSettings, RenderingSettings};
use crate::ports::settings_repository::SettingsRepository;
use actix::dev::{MessageResponse, MessageResult, OneshotSender};
use actix::prelude::*;
use anyhow::Result;
use log::{error, info};
use std::sync::Arc;

pub struct SettingsActor {
    repository: Arc<dyn SettingsRepository>,
    current_physics: PhysicsSettings,
    current_constraints: ConstraintSettings,
    current_rendering: RenderingSettings,
    current_node_filter: NodeFilterSettings,
    current_quality_gates: QualityGateSettings,
}

impl SettingsActor {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self {
            repository,
            current_physics: PhysicsSettings::default(),
            current_constraints: ConstraintSettings::default(),
            current_rendering: RenderingSettings::default(),
            current_node_filter: NodeFilterSettings::default(),
            current_quality_gates: QualityGateSettings::default(),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        info!("Settings actor initialized with defaults (async load will occur on start)");
        Ok(())
    }
}

impl Actor for SettingsActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("SettingsActor started with default settings");

        // Load node filter settings from repository on startup
        let repository = self.repository.clone();
        ctx.spawn(async move {
            // Try to load node filter settings
            match repository.get_setting("node_filter").await {
                Ok(Some(crate::ports::settings_repository::SettingValue::Json(json))) => {
                    match serde_json::from_value::<NodeFilterSettings>(json) {
                        Ok(settings) => {
                            info!("Loaded node filter settings from DB: enabled={}, threshold={}",
                                  settings.enabled, settings.quality_threshold);
                        }
                        Err(e) => {
                            info!("Failed to parse node filter settings, will initialize defaults: {}", e);
                        }
                    }
                }
                Ok(_) => {
                    // No node filter settings exist, initialize with defaults
                    info!("No node filter settings found, initializing defaults (enabled=true, quality_threshold=0.9)");
                    let default_settings = NodeFilterSettings::default();
                    let settings_json = serde_json::to_value(&default_settings).unwrap_or_default();

                    if let Err(e) = repository.set_setting(
                        "node_filter",
                        crate::ports::settings_repository::SettingValue::Json(settings_json),
                        Some("Node confidence filter settings - filters out low quality nodes"),
                    ).await {
                        error!("Failed to initialize node filter settings: {}", e);
                    } else {
                        info!("Initialized default node filter settings in database");
                    }
                }
                Err(e) => {
                    error!("Failed to check node filter settings: {}", e);
                }
            }
        }.into_actor(self));
    }
}

// ============================================================================
// Message Types
// ============================================================================

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UpdatePhysicsSettings(pub PhysicsSettings);

#[derive(Message)]
#[rtype(result = "PhysicsSettings")]
pub struct GetPhysicsSettings;

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UpdateConstraintSettings(pub ConstraintSettings);

#[derive(Message)]
#[rtype(result = "ConstraintSettings")]
pub struct GetConstraintSettings;

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UpdateRenderingSettings(pub RenderingSettings);

#[derive(Message)]
#[rtype(result = "RenderingSettings")]
pub struct GetRenderingSettings;

#[derive(Message)]
#[rtype(result = "Result<AllSettings>")]
pub struct LoadProfile(pub i64);

#[derive(Message)]
#[rtype(result = "Result<i64>")]
pub struct SaveProfile {
    pub name: String,
}

#[derive(Message)]
#[rtype(result = "Result<Vec<SettingsProfile>>")]
pub struct ListProfiles;

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct DeleteProfile(pub i64);

#[derive(Message)]
#[rtype(result = "AllSettings")]
pub struct GetAllSettings;

/// Message to update node filter settings
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UpdateNodeFilterSettings(pub NodeFilterSettings);

/// Message to get node filter settings
#[derive(Message)]
#[rtype(result = "NodeFilterSettings")]
pub struct GetNodeFilterSettings;

/// Message to update quality gate settings
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UpdateQualityGateSettings(pub QualityGateSettings);

/// Message to get quality gate settings
#[derive(Message)]
#[rtype(result = "QualityGateSettings")]
pub struct GetQualityGateSettings;

// ============================================================================
// MessageResponse Implementations
// ============================================================================

// PhysicsSettings is now a foreign type (visionclaw-domain), so we can't impl
// MessageResponse for it directly (orphan rule). Handlers return MessageResult instead.

impl<A, M> MessageResponse<A, M> for ConstraintSettings
where
    A: Actor,
    M: Message<Result = ConstraintSettings>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            let _ = tx.send(self);
        }
    }
}

// RenderingSettings is now a foreign type (visionclaw-domain) post Phase A6.3,
// so we can't impl MessageResponse for it directly (orphan rule). Handler
// uses MessageResult instead.

impl<A, M> MessageResponse<A, M> for AllSettings
where
    A: Actor,
    M: Message<Result = AllSettings>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            let _ = tx.send(self);
        }
    }
}

impl<A, M> MessageResponse<A, M> for NodeFilterSettings
where
    A: Actor,
    M: Message<Result = NodeFilterSettings>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            let _ = tx.send(self);
        }
    }
}

impl<A, M> MessageResponse<A, M> for QualityGateSettings
where
    A: Actor,
    M: Message<Result = QualityGateSettings>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            let _ = tx.send(self);
        }
    }
}

// ============================================================================
// Message Handlers
// ============================================================================

impl Handler<UpdatePhysicsSettings> for SettingsActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: UpdatePhysicsSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_physics = msg.0.clone();
        let repository = self.repository.clone();
        let settings = msg.0;

        Box::pin(async move {
            repository
                .save_physics_settings("default", &settings)
                .await?;
            info!("Physics settings updated and persisted");
            Ok(())
        })
    }
}

impl Handler<GetPhysicsSettings> for SettingsActor {
    type Result = MessageResult<GetPhysicsSettings>;

    fn handle(&mut self, _msg: GetPhysicsSettings, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.current_physics.clone())
    }
}

impl Handler<UpdateConstraintSettings> for SettingsActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: UpdateConstraintSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_constraints = msg.0.clone();
        let repository = self.repository.clone();
        let settings = msg.0;

        Box::pin(async move {
            let settings_json = serde_json::to_value(&settings)
                .map_err(|e| anyhow::anyhow!("Failed to serialize constraint settings: {}", e))?;

            repository
                .set_setting(
                    "constraints",
                    crate::ports::settings_repository::SettingValue::Json(settings_json),
                    Some("Constraint settings for physics simulation"),
                )
                .await?;

            info!("Constraint settings updated and persisted");
            Ok(())
        })
    }
}

impl Handler<GetConstraintSettings> for SettingsActor {
    type Result = ConstraintSettings;

    fn handle(&mut self, _msg: GetConstraintSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_constraints.clone()
    }
}

impl Handler<UpdateRenderingSettings> for SettingsActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: UpdateRenderingSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_rendering = msg.0.clone();
        let repository = self.repository.clone();
        let settings = msg.0;

        Box::pin(async move {
            let settings_json = serde_json::to_value(&settings)
                .map_err(|e| anyhow::anyhow!("Failed to serialize rendering settings: {}", e))?;

            repository
                .set_setting(
                    "rendering",
                    crate::ports::settings_repository::SettingValue::Json(settings_json),
                    Some("Rendering settings for visualization"),
                )
                .await?;

            info!("Rendering settings updated and persisted");
            Ok(())
        })
    }
}

impl Handler<GetRenderingSettings> for SettingsActor {
    type Result = MessageResult<GetRenderingSettings>;

    fn handle(&mut self, _msg: GetRenderingSettings, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.current_rendering.clone())
    }
}

impl Handler<LoadProfile> for SettingsActor {
    type Result = ResponseFuture<Result<AllSettings>>;

    fn handle(&mut self, msg: LoadProfile, _ctx: &mut Self::Context) -> Self::Result {
        let _profile_id = msg.0;
        info!("Profile loading not implemented, returning defaults");
        Box::pin(async move { Ok(AllSettings::default()) })
    }
}

impl Handler<SaveProfile> for SettingsActor {
    type Result = ResponseFuture<Result<i64>>;

    fn handle(&mut self, msg: SaveProfile, _ctx: &mut Self::Context) -> Self::Result {
        let name = msg.name.clone();
        info!("Profile saving not implemented for '{}'", name);
        Box::pin(async move { Ok(1) })
    }
}

impl Handler<ListProfiles> for SettingsActor {
    type Result = ResponseFuture<Result<Vec<SettingsProfile>>>;

    fn handle(&mut self, _msg: ListProfiles, _ctx: &mut Self::Context) -> Self::Result {
        info!("Profile listing not implemented");
        Box::pin(async move { Ok(Vec::new()) })
    }
}

impl Handler<DeleteProfile> for SettingsActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: DeleteProfile, _ctx: &mut Self::Context) -> Self::Result {
        let profile_id = msg.0;
        info!("Profile deletion not implemented for ID {}", profile_id);
        Box::pin(async move { Ok(()) })
    }
}

impl Handler<GetAllSettings> for SettingsActor {
    type Result = AllSettings;

    fn handle(&mut self, _msg: GetAllSettings, _ctx: &mut Self::Context) -> Self::Result {
        AllSettings {
            physics: self.current_physics.clone(),
            constraints: self.current_constraints.clone(),
            rendering: self.current_rendering.clone(),
            node_filter: self.current_node_filter.clone(),
            quality_gates: self.current_quality_gates.clone(),
            visual: serde_json::json!({}),
        }
    }
}

impl Handler<UpdateNodeFilterSettings> for SettingsActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: UpdateNodeFilterSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_node_filter = msg.0.clone();
        let repository = self.repository.clone();
        let settings = msg.0;

        Box::pin(async move {
            // Persist to SQLite as JSON
            let settings_json = serde_json::to_value(&settings)
                .map_err(|e| anyhow::anyhow!("Failed to serialize node filter settings: {}", e))?;

            repository
                .set_setting(
                    "node_filter",
                    crate::ports::settings_repository::SettingValue::Json(settings_json),
                    Some("Node confidence filter settings"),
                )
                .await?;

            info!(
                "Node filter settings updated and persisted: enabled={}, quality_threshold={}",
                settings.enabled, settings.quality_threshold
            );
            Ok(())
        })
    }
}

impl Handler<GetNodeFilterSettings> for SettingsActor {
    type Result = NodeFilterSettings;

    fn handle(&mut self, _msg: GetNodeFilterSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_node_filter.clone()
    }
}

impl Handler<UpdateQualityGateSettings> for SettingsActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: UpdateQualityGateSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_quality_gates = msg.0.clone();
        let repository = self.repository.clone();
        let settings = msg.0;

        Box::pin(async move {
            // Persist to SQLite as JSON
            let settings_json = serde_json::to_value(&settings)
                .map_err(|e| anyhow::anyhow!("Failed to serialize quality gate settings: {}", e))?;

            repository
                .set_setting(
                    "quality_gates",
                    crate::ports::settings_repository::SettingValue::Json(settings_json),
                    Some("Quality gate settings for feature toggles and performance thresholds"),
                )
                .await?;

            info!("Quality gate settings updated and persisted: gpu={}, ontology={}, semantic={}, layout={}",
                  settings.gpu_acceleration, settings.ontology_physics, settings.semantic_forces, settings.layout_mode);
            Ok(())
        })
    }
}

impl Handler<GetQualityGateSettings> for SettingsActor {
    type Result = QualityGateSettings;

    fn handle(&mut self, _msg: GetQualityGateSettings, _ctx: &mut Self::Context) -> Self::Result {
        self.current_quality_gates.clone()
    }
}
