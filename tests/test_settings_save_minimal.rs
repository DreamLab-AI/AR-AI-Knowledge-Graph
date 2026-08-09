// Minimal test to verify settings save functionality
// Note: YAML file persistence was removed — settings are now in Oxigraph (ADR-11).
// save() is a no-op that returns Ok(()) for backwards compatibility.

use visionclaw_server::config::AppFullSettings;

#[test]
fn test_settings_save_returns_ok() {
    let mut settings = AppFullSettings::default();
    settings.system.persist_settings = true;

    // save() is a no-op (legacy YAML removed) — should return Ok
    let result = settings.save();
    assert!(
        result.is_ok(),
        "save() should return Ok: {:?}",
        result.err()
    );
}

#[test]
fn test_settings_save_disabled() {
    let mut settings = AppFullSettings::default();
    settings.system.persist_settings = false;

    let result = settings.save();
    assert!(result.is_ok(), "Save should return Ok even when disabled");
}

#[test]
fn test_settings_merge_update() {
    use serde_json::json;

    let mut settings = AppFullSettings::default();
    settings.system.persist_settings = true;

    let update = json!({
        "visualisation": {
            "glow": {
                "intensity": 3.5,
                "threshold": 0.9
            },
            "rendering": {
                "directionalLightIntensity": 2.0
            }
        }
    });

    let merge_result = settings.merge_update(update);
    assert!(
        merge_result.is_ok(),
        "Failed to merge update: {:?}",
        merge_result.err()
    );

    // save() is a no-op but should still succeed
    let save_result = settings.save();
    assert!(
        save_result.is_ok(),
        "Failed to save merged settings: {:?}",
        save_result.err()
    );
}

#[test]
fn test_settings_validation_boundaries() {
    use serde_json::json;

    let mut settings = AppFullSettings::default();

    // Test edge cases for glow settings
    let valid_edge_cases = json!({
        "visualisation": {
            "glow": {
                "intensity": 10.0,  // Max allowed
                "radius": 10.0,     // Max allowed
                "threshold": 1.0,   // Max allowed
                "opacity": 1.0      // Max allowed
            }
        }
    });

    let result = settings.merge_update(valid_edge_cases);
    assert!(result.is_ok(), "Valid edge cases should be accepted");

    assert_eq!(settings.visualisation.glow.intensity, 10.0);
    assert_eq!(settings.visualisation.glow.radius, 10.0);
    assert_eq!(settings.visualisation.glow.threshold, 1.0);
    assert_eq!(settings.visualisation.glow.opacity, 1.0);
}
