// tests/ports/test_settings_repository.rs
//! Contract tests for SettingsRepository port

use super::mocks::MockSettingsRepository;
use visionclaw_server::config::PhysicsSettings;
use visionclaw_server::ports::{SettingsRepository, SettingValue};
use std::collections::HashMap;

#[tokio::test]
async fn test_get_set_setting() {
    let repo = MockSettingsRepository::new();

    // Set a string setting
    repo.set_setting("log_level", SettingValue::String("debug".into()), Some("Log level"))
        .await
        .unwrap();

    // Get the setting
    let value = repo.get_setting("log_level").await.unwrap();
    assert_eq!(value, Some(SettingValue::String("debug".into())));
}

#[tokio::test]
async fn test_setting_types() {
    let repo = MockSettingsRepository::new();

    // String
    repo.set_setting("str", SettingValue::String("value".into()), None)
        .await
        .unwrap();

    // Integer
    repo.set_setting("int", SettingValue::Integer(42), None)
        .await
        .unwrap();

    // Float
    repo.set_setting("float", SettingValue::Float(3.14), None)
        .await
        .unwrap();

    // Boolean
    repo.set_setting("bool", SettingValue::Boolean(true), None)
        .await
        .unwrap();

    // Verify all types
    assert_eq!(
        repo.get_setting("str").await.unwrap(),
        Some(SettingValue::String("value".into()))
    );
    assert_eq!(
        repo.get_setting("int").await.unwrap(),
        Some(SettingValue::Integer(42))
    );
    assert_eq!(
        repo.get_setting("float").await.unwrap(),
        Some(SettingValue::Float(3.14))
    );
    assert_eq!(
        repo.get_setting("bool").await.unwrap(),
        Some(SettingValue::Boolean(true))
    );
}

#[tokio::test]
async fn test_delete_setting() {
    let repo = MockSettingsRepository::new();

    repo.set_setting("temp", SettingValue::String("value".into()), None)
        .await
        .unwrap();

    assert!(repo.has_setting("temp").await.unwrap());

    repo.delete_setting("temp").await.unwrap();

    assert!(!repo.has_setting("temp").await.unwrap());
    assert_eq!(repo.get_setting("temp").await.unwrap(), None);
}

#[tokio::test]
async fn test_batch_operations() {
    let repo = MockSettingsRepository::new();

    // Batch set
    let mut updates = HashMap::new();
    updates.insert("k1".to_string(), SettingValue::Integer(1));
    updates.insert("k2".to_string(), SettingValue::Integer(2));
    updates.insert("k3".to_string(), SettingValue::Integer(3));

    repo.set_settings_batch(updates).await.unwrap();

    // Batch get
    let keys = vec!["k1".to_string(), "k2".to_string(), "k3".to_string()];
    let batch = repo.get_settings_batch(&keys).await.unwrap();

    assert_eq!(batch.len(), 3);
    assert_eq!(batch.get("k1"), Some(&SettingValue::Integer(1)));
    assert_eq!(batch.get("k2"), Some(&SettingValue::Integer(2)));
    assert_eq!(batch.get("k3"), Some(&SettingValue::Integer(3)));
}

#[tokio::test]
async fn test_list_settings() {
    let repo = MockSettingsRepository::new();

    repo.set_setting("app.log_level", SettingValue::String("info".into()), None)
        .await
        .unwrap();
    repo.set_setting("app.max_nodes", SettingValue::Integer(1000), None)
        .await
        .unwrap();
    repo.set_setting("user.name", SettingValue::String("Alice".into()), None)
        .await
        .unwrap();

    // List all
    let all = repo.list_settings(None).await.unwrap();
    assert_eq!(all.len(), 3);

    // List with prefix
    let app_settings = repo.list_settings(Some("app.")).await.unwrap();
    assert_eq!(app_settings.len(), 2);
}

#[tokio::test]
async fn test_physics_profiles() {
    let repo = MockSettingsRepository::new();

    // Create a physics settings profile
    let physics = PhysicsSettings {
        time_step: 0.016,
        damping: 0.8,
        spring_strength: 0.01,
        repulsion_strength: 100.0,
        max_velocity: 10.0,
        ..Default::default()
    };

    repo.save_physics_settings("knowledge", &physics).await.unwrap();

    // Load it back
    let loaded = repo.get_physics_settings("knowledge").await.unwrap();
    assert_eq!(loaded.time_step, 0.016);
    assert_eq!(loaded.damping, 0.8);

    // List profiles
    let profiles = repo.list_physics_profiles().await.unwrap();
    assert!(profiles.contains(&"knowledge".to_string()));

    // Delete profile
    repo.delete_physics_profile("knowledge").await.unwrap();
    let profiles = repo.list_physics_profiles().await.unwrap();
    assert!(!profiles.contains(&"knowledge".to_string()));
}

#[tokio::test]
async fn test_export_import() {
    let repo = MockSettingsRepository::new();

    // Set some settings
    repo.set_setting("key1", SettingValue::String("value1".into()), None)
        .await
        .unwrap();
    repo.set_setting("key2", SettingValue::Integer(42), None)
        .await
        .unwrap();

    // Export
    let exported = repo.export_settings().await.unwrap();

    // Clear and import
    repo.delete_setting("key1").await.unwrap();
    repo.delete_setting("key2").await.unwrap();

    repo.import_settings(&exported).await.unwrap();

    // Verify
    assert_eq!(
        repo.get_setting("key1").await.unwrap(),
        Some(SettingValue::String("value1".into()))
    );
    assert_eq!(
        repo.get_setting("key2").await.unwrap(),
        Some(SettingValue::Integer(42))
    );
}

#[tokio::test]
async fn test_health_check() {
    let repo = MockSettingsRepository::new();
    assert!(repo.health_check().await.unwrap());
}
