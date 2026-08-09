use log::info;
use std::env;

pub struct FeatureAccess {
    pub approved_pubkeys: Vec<String>,

    pub perplexity_enabled: Vec<String>,
    pub openai_enabled: Vec<String>,
    pub ragflow_enabled: Vec<String>,

    pub power_users: Vec<String>,
    pub settings_sync_enabled: Vec<String>,
}

impl FeatureAccess {
    pub fn from_env() -> Self {
        Self {
            approved_pubkeys: Self::load_pubkeys_from_env("APPROVED_PUBKEYS"),

            perplexity_enabled: Self::load_pubkeys_from_env("PERPLEXITY_ENABLED_PUBKEYS"),
            openai_enabled: Self::load_pubkeys_from_env("OPENAI_ENABLED_PUBKEYS"),
            ragflow_enabled: Self::load_pubkeys_from_env("RAGFLOW_ENABLED_PUBKEYS"),

            power_users: Self::load_pubkeys_from_env("POWER_USER_PUBKEYS"),
            settings_sync_enabled: Self::load_pubkeys_from_env("SETTINGS_SYNC_ENABLED_PUBKEYS"),
        }
    }

    fn load_pubkeys_from_env(var_name: &str) -> Vec<String> {
        env::var(var_name)
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    pub fn register_new_user(&mut self, pubkey: &str) -> bool {
        let pubkey = pubkey.to_string();

        if self.approved_pubkeys.contains(&pubkey) {
            return false;
        }

        self.approved_pubkeys.push(pubkey.clone());

        self.ragflow_enabled.push(pubkey.clone());

        self.openai_enabled.push(pubkey.clone());

        // Feature access persisted in-memory only. Use database for production persistence.

        info!("Registered new user: {}", pubkey);
        true
    }

    pub fn has_access(&self, pubkey: &str) -> bool {
        self.approved_pubkeys.contains(&pubkey.to_string())
    }

    pub fn has_perplexity_access(&self, pubkey: &str) -> bool {
        self.perplexity_enabled.contains(&pubkey.to_string())
    }

    pub fn has_openai_access(&self, pubkey: &str) -> bool {
        self.openai_enabled.contains(&pubkey.to_string())
    }

    pub fn has_ragflow_access(&self, pubkey: &str) -> bool {
        self.ragflow_enabled.contains(&pubkey.to_string())
    }

    pub fn is_power_user(&self, pubkey: &str) -> bool {
        self.power_users.contains(&pubkey.to_string())
    }

    pub fn can_sync_settings(&self, pubkey: &str) -> bool {
        self.is_power_user(pubkey) || self.settings_sync_enabled.contains(&pubkey.to_string())
    }

    pub fn has_feature_access(&self, pubkey: &str, feature: &str) -> bool {
        match feature {
            "perplexity" => self.has_perplexity_access(pubkey),
            "openai" => self.has_openai_access(pubkey),
            "ragflow" => self.has_ragflow_access(pubkey),
            "settings_sync" => self.can_sync_settings(pubkey),
            _ => false,
        }
    }

    pub fn get_available_features(&self, pubkey: &str) -> Vec<String> {
        let mut features = Vec::new();

        if self.has_perplexity_access(pubkey) {
            features.push("perplexity".to_string());
        }
        if self.has_openai_access(pubkey) {
            features.push("openai".to_string());
        }
        if self.has_ragflow_access(pubkey) {
            features.push("ragflow".to_string());
        }
        if self.can_sync_settings(pubkey) {
            features.push("settings_sync".to_string());
        }
        if self.is_power_user(pubkey) {
            features.push("power_user".to_string());
        }

        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_test_env() {
        env::set_var("APPROVED_PUBKEYS", "pub1,pub2");
        env::set_var("POWER_USER_PUBKEYS", "pub1");
        env::set_var("PERPLEXITY_ENABLED_PUBKEYS", "pub1,pub2");
        env::set_var("OPENAI_ENABLED_PUBKEYS", "pub1");
        env::set_var("SETTINGS_SYNC_ENABLED_PUBKEYS", "pub2");
    }

    #[test]
    fn test_basic_access() {
        setup_test_env();
        let access = FeatureAccess::from_env();

        assert!(access.has_access("pub1"));
        assert!(access.has_access("pub2"));
        assert!(!access.has_access("pub3"));
    }

    #[test]
    fn test_power_user_status() {
        setup_test_env();
        let access = FeatureAccess::from_env();

        assert!(access.is_power_user("pub1"));
        assert!(!access.is_power_user("pub2"));
    }

    #[test]
    fn test_feature_access() {
        setup_test_env();
        let access = FeatureAccess::from_env();

        assert!(access.has_perplexity_access("pub1"));
        assert!(access.has_openai_access("pub1"));
        assert!(access.can_sync_settings("pub1"));

        assert!(access.has_perplexity_access("pub2"));
        assert!(!access.has_openai_access("pub2"));
        assert!(access.can_sync_settings("pub2"));
    }

    #[test]
    fn test_available_features() {
        setup_test_env();
        let access = FeatureAccess::from_env();

        let pub1_features = access.get_available_features("pub1");
        assert!(pub1_features.contains(&"power_user".to_string()));
        assert!(pub1_features.contains(&"perplexity".to_string()));
        assert!(pub1_features.contains(&"openai".to_string()));
        assert!(pub1_features.contains(&"settings_sync".to_string()));

        let pub2_features = access.get_available_features("pub2");
        assert!(!pub2_features.contains(&"power_user".to_string()));
        assert!(pub2_features.contains(&"perplexity".to_string()));
        assert!(pub2_features.contains(&"settings_sync".to_string()));
    }
}
