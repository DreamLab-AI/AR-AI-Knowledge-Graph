use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use validator::{Validate, ValidationError};

use super::field_mappings::{convert_empty_strings_to_null, merge_json_values, normalize_field_names_to_camel_case};
use super::services::{
    AuthSettings, KokoroSettings, OntologyAgentSettings, OpenAISettings, PerplexitySettings,
    RagFlowSettings, VoiceRoutingSettings, WhisperSettings,
};
use super::system::SystemSettings;
use super::validation::{to_camel_case, validate_bloom_glow_settings};
use super::visualisation::VisualisationSettings;
use super::xr::XRSettings;
use crate::types::physics_config::PhysicsSettings;

#[derive(Debug, Clone, Deserialize, Serialize, Type, Validate, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    #[serde(default)]
    pub comfort_level: Option<f32>,
    #[serde(default)]
    pub interaction_style: Option<String>,
    #[serde(default)]
    pub ar_preference: Option<bool>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlags {
    #[serde(default)]
    pub gpu_clustering: bool,
    #[serde(default)]
    pub ontology_validation: bool,
    #[serde(default)]
    pub gpu_anomaly_detection: bool,
    #[serde(default)]
    pub real_time_insights: bool,
    #[serde(default)]
    pub advanced_visualizations: bool,
    #[serde(default)]
    pub performance_monitoring: bool,
    #[serde(default)]
    pub stress_majorization: bool,
    #[serde(default)]
    pub semantic_constraints: bool,
    #[serde(default)]
    pub sssp_integration: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperConfig {
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default)]
    pub show_performance_stats: bool,
    #[serde(default)]
    pub enable_profiling: bool,
    #[serde(default)]
    pub verbose_logging: bool,
    #[serde(default)]
    pub dev_tools_enabled: bool,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, Type, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AppFullSettings {
    #[validate(nested)]
    #[serde(alias = "visualisation")]
    pub visualisation: VisualisationSettings,
    #[validate(nested)]
    #[serde(alias = "system")]
    pub system: SystemSettings,
    #[validate(nested)]
    #[serde(alias = "xr")]
    pub xr: XRSettings,
    #[validate(nested)]
    #[serde(alias = "auth")]
    pub auth: AuthSettings,
    #[serde(skip_serializing_if = "Option::is_none", alias = "ragflow")]
    pub ragflow: Option<RagFlowSettings>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "perplexity")]
    pub perplexity: Option<PerplexitySettings>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "openai")]
    pub openai: Option<OpenAISettings>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "kokoro")]
    pub kokoro: Option<KokoroSettings>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "whisper")]
    pub whisper: Option<WhisperSettings>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "voice_routing")]
    pub voice_routing: Option<VoiceRoutingSettings>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "ontology_agent")]
    pub ontology_agent: Option<OntologyAgentSettings>,
    #[serde(default = "default_version", alias = "version")]
    pub version: String,
    #[serde(default, alias = "user_preferences")]
    #[validate(nested)]
    pub user_preferences: UserPreferences,
    #[serde(default, alias = "physics")]
    #[validate(nested)]
    pub physics: PhysicsSettings,
    #[serde(default, alias = "feature_flags")]
    pub feature_flags: FeatureFlags,
    #[serde(default, alias = "developer_config")]
    pub developer_config: DeveloperConfig,
}

impl Default for AppFullSettings {
    fn default() -> Self {
        Self {
            visualisation: VisualisationSettings::default(),
            system: SystemSettings::default(),
            xr: XRSettings::default(),
            auth: AuthSettings::default(),
            ragflow: None,
            perplexity: None,
            openai: None,
            kokoro: None,
            whisper: None,
            voice_routing: None,
            ontology_agent: None,
            version: default_version(),
            user_preferences: UserPreferences::default(),
            physics: PhysicsSettings::default(),
            feature_flags: FeatureFlags::default(),
            developer_config: DeveloperConfig::default(),
        }
    }
}

impl AppFullSettings {
    pub fn new() -> Result<Self, String> {
        debug!("Initializing AppFullSettings with defaults (database-first architecture)");
        info!("IMPORTANT: Settings should be loaded from database via DatabaseService");
        info!("Legacy YAML file loading has been removed — all settings are now in SQLite (ADR-11)");
        Ok(Self::default())
    }

    pub fn save(&self) -> Result<(), String> {
        debug!("save() called but ignored — settings are now automatically persisted to database");
        info!("Legacy YAML file saving has been removed — all settings are now in SQLite (ADR-11)");
        Ok(())
    }

    pub fn get_physics(&self, graph: &str) -> &PhysicsSettings {
        match graph {
            "logseq" | "knowledge" => &self.visualisation.graphs.logseq.physics,
            "visionclaw" | "agent" | "bots" => &self.visualisation.graphs.visionclaw.physics,
            _ => {
                log::debug!(
                    "Unknown graph type '{}', defaulting to logseq (knowledge graph)",
                    graph
                );
                &self.visualisation.graphs.logseq.physics
            }
        }
    }

    pub fn merge_update(&mut self, update: serde_json::Value) -> Result<(), String> {
        let debug_enabled = log::log_enabled!(log::Level::Debug);

        if debug_enabled {
            if let Ok(s) = serde_json::to_string_pretty(&update) {
                debug!("merge_update: Incoming update (camelCase): {}", s);
            }
        }

        let processed_update = convert_empty_strings_to_null(update.clone());
        if debug_enabled {
            if let Ok(s) = serde_json::to_string_pretty(&processed_update) {
                debug!("merge_update: After null conversion: {}", s);
            }
        }

        let current_value = serde_json::to_value(&self)
            .map_err(|e| format!("Failed to serialize current settings: {}", e))?;

        let normalized_current = normalize_field_names_to_camel_case(current_value)?;
        let normalized_update = normalize_field_names_to_camel_case(processed_update)?;

        if debug_enabled {
            if let Ok(s) = serde_json::to_string_pretty(&normalized_current) {
                debug!("merge_update: After field normalization (current): {}", s);
            }
            if let Ok(s) = serde_json::to_string_pretty(&normalized_update) {
                debug!("merge_update: After field normalization (update): {}", s);
            }
        }

        let merged = merge_json_values(normalized_current, normalized_update);
        if debug_enabled {
            if let Ok(s) = serde_json::to_string_pretty(&merged) {
                debug!("merge_update: After merge: {}", s);
            }
        }

        *self = serde_json::from_value(merged.clone()).map_err(|e| {
            if debug_enabled {
                if let Ok(s) = serde_json::to_string_pretty(&merged) {
                    error!("merge_update: Failed to deserialize merged JSON: {}", s);
                }
                if let Ok(s) = serde_json::to_string_pretty(&update) {
                    error!("merge_update: Original update was: {}", s);
                }
            }
            format!("Failed to deserialize merged settings: {}", e)
        })?;

        Ok(())
    }

    pub fn validate_config_camel_case(&self) -> Result<(), validator::ValidationErrors> {
        self.validate()?;
        self.validate_cross_field_constraints()?;
        Ok(())
    }

    fn validate_cross_field_constraints(&self) -> Result<(), validator::ValidationErrors> {
        let mut errors = validator::ValidationErrors::new();

        if self.visualisation.graphs.logseq.physics.gravity != 0.0
            && !self.visualisation.graphs.logseq.physics.enabled
        {
            errors.add("physics", ValidationError::new("physics_enabled_required"));
        }

        if let Err(validation_error) =
            validate_bloom_glow_settings(&self.visualisation.glow, &self.visualisation.bloom)
        {
            errors.add("visualisation.bloom_glow", validation_error);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn get_validation_errors_camel_case(
        errors: &validator::ValidationErrors,
    ) -> HashMap<String, Vec<String>> {
        let mut result = HashMap::new();

        for (field, field_errors) in errors.field_errors() {
            let camel_case_field = to_camel_case(field);
            let messages: Vec<String> = field_errors
                .iter()
                .map(|error| match error.code.as_ref() {
                    "invalid_hex_color" => {
                        "Must be a valid hex color (#RRGGBB or #RRGGBBAA)".to_string()
                    }
                    "width_range_length" => "Width range must have exactly 2 values".to_string(),
                    "width_range_order" => {
                        "Width range minimum must be less than maximum".to_string()
                    }
                    "invalid_port" => "Port must be between 1 and 65535".to_string(),
                    "invalid_percentage" => "Value must be between 0 and 100".to_string(),
                    "physics_enabled_required" => {
                        "Physics must be enabled when gravity is configured".to_string()
                    }
                    _ => format!("Invalid value for {}", camel_case_field),
                })
                .collect();

            result.insert(camel_case_field, messages);
        }

        result
    }
}
