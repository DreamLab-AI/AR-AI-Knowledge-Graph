// src/settings/mod.rs
//! Settings Management Module
//!
//! Provides persistent settings management for the control center including:
//! - Database persistence layer (settings_repository)
//! - REST API endpoints (api/settings_routes)
//! - Authentication extractors (auth_extractor)

pub mod api;
pub mod auth_extractor;
pub mod models;

// ADR-2046: `settings_actor` (SettingsActor + its GetPhysicsSettings/LoadProfile/
// SaveProfile/UpdatePhysicsSettings messages) removed. It was never started at
// runtime (OptimizedSettingsActor is the live actor, started in app_state.rs) and
// had no importers of this module's re-exports.
pub use auth_extractor::{AuthenticatedUser, OptionalAuth};
pub use models::{AllSettings, ConstraintSettings, PriorityWeighting, SettingsProfile};
