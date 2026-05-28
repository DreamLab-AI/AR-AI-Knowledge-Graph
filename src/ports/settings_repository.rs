// src/ports/settings_repository.rs
//! Settings Repository Port — shim for ADR-090 Phase A6 slice 3.
//!
//! The canonical definition has moved to `visionclaw-domain`. This file
//! re-exports everything so existing `use crate::ports::settings_repository::*`
//! callers continue to compile unchanged.

pub use visionclaw_domain::ports::settings_repository::{
    Result, SettingValue, SettingsRepository, SettingsRepositoryError,
};

// Re-export AppFullSettings so callers that do
// `use crate::ports::settings_repository::AppFullSettings` still work.
pub use visionclaw_domain::config::AppFullSettings;
