// src/settings/mod.rs
//! Settings Management Module
//!
//! Provides persistent settings management for the control center including:
//! - Database persistence layer (settings_repository)
//! - Runtime settings actor (settings_actor)
//! - REST API endpoints (api/settings_routes)
//! - Authentication extractors (auth_extractor)

pub mod api;
pub mod auth_extractor;
pub mod models;
pub mod settings_actor;

// SettingsActor is retained for backward compatibility but routes now use OptimizedSettingsActor.
pub use auth_extractor::{AuthenticatedUser, OptionalAuth};
pub use models::{AllSettings, ConstraintSettings, PriorityWeighting, SettingsProfile};
pub use settings_actor::{
    GetPhysicsSettings, LoadProfile, SaveProfile, SettingsActor, UpdatePhysicsSettings,
};
