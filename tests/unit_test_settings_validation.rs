// Unit tests for settings validation functions
// These test the individual validation functions without needing full integration setup
//
// NOTE: These tests are commented out because they access private functions
// from the settings_handler module. To re-enable, either:
// 1. Make the validation functions public with #[cfg(test)]
// 2. Move these tests into the settings_handler module as unit tests

use serde_json::json;
// Private functions - cannot be accessed from external test crate
// use visionclaw_server::handlers::settings_handler::{
//     extract_physics_updates, validate_glow_settings, validate_node_settings,
//     validate_physics_settings, validate_rendering_settings, validate_settings_update,
// };

// #[test]
// fn test_validate_settings_update() {
//     // Test valid settings
//     let valid_update = json!({
//         "visualisation": {
//             "glow": {
//                 "intensity": 1.5,
//                 "radius": 0.5,
//                 "threshold": 0.8
//             }
//         }
//     });
//
//     assert!(validate_settings_update(&valid_update).is_ok());
//
//     // Test invalid glow intensity
//     let invalid_glow = json!({
//         "visualisation": {
//             "glow": {
//                 "intensity": -1.0 // Invalid negative value
//             }
//         }
//     });
//
//     assert!(validate_settings_update(&invalid_glow).is_err());
//
//     // Test invalid node size
//     let invalid_node = json!({
//         "visualisation": {
//             "graphs": {
//                 "knowledge": {
//                     "nodes": {
//                         "nodeSize": -0.5 // Invalid negative value
//                     }
//                 }
//             }
//         }
//     });
//
//     assert!(validate_settings_update(&invalid_node).is_err());
//
//     // Test invalid physics settings
//     let invalid_physics = json!({
//         "visualisation": {
//             "graphs": {
//                 "knowledge": {
//                     "physics": {
//                         "damping": 2.0 // Invalid > 1.0
//                     }
//                 }
//             }
//         }
//     });
//
//     assert!(validate_settings_update(&invalid_physics).is_err());
// }

// #[test]
// fn test_validate_physics_settings() {
//     // Test valid physics
//     let valid_physics = json!({
//         "damping": 0.85,
//         "temperature": 0.01,
//         "iterations": 50,
//         "maxVelocity": 5.0
//     });
//
//     assert!(validate_physics_settings(&valid_physics).is_ok());
//
//     // Test invalid damping
//     let invalid_damping = json!({
//         "damping": 1.5 // > 1.0
//     });
//
//     assert!(validate_physics_settings(&invalid_damping).is_err());
//
//     // Test invalid temperature
//     let invalid_temp = json!({
//         "temperature": -0.1 // negative
//     });
//
//     assert!(validate_physics_settings(&invalid_temp).is_err());
//
//     // Test invalid iterations
//     let invalid_iterations = json!({
//         "iterations": 0 // must be > 0
//     });
//
//     assert!(validate_physics_settings(&invalid_iterations).is_err());
// }

// #[test]
// fn test_validate_glow_settings() {
//     // Test valid glow settings
//     let valid_glow = json!({
//         "intensity": 1.5,
//         "radius": 0.5,
//         "threshold": 0.8,
//         "opacity": 0.9
//     });
//
//     assert!(validate_glow_settings(&valid_glow).is_ok());
//
//     // Test invalid intensity
//     let invalid_intensity = json!({
//         "intensity": 15.0 // > 10.0
//     });
//
//     assert!(validate_glow_settings(&invalid_intensity).is_err());
//
//     // Test invalid opacity
//     let invalid_opacity = json!({
//         "opacity": 1.5 // > 1.0
//     });
//
//     assert!(validate_glow_settings(&invalid_opacity).is_err());
//
//     // Test NaN value
//     let nan_value = json!({
//         "intensity": f64::NAN
//     });
//
//     assert!(validate_glow_settings(&nan_value).is_err());
// }

// #[test]
// fn test_validate_node_settings() {
//     // Test valid node settings
//     let valid_nodes = json!({
//         "nodeSize": 1.0,
//         "opacity": 0.95,
//         "metalness": 0.85,
//         "roughness": 0.15
//     });
//
//     assert!(validate_node_settings(&valid_nodes).is_ok());
//
//     // Test invalid node size
//     let invalid_size = json!({
//         "nodeSize": -1.0 // negative
//     });
//
//     assert!(validate_node_settings(&invalid_size).is_err());
//
//     // Test invalid opacity
//     let invalid_opacity = json!({
//         "opacity": 1.5 // > 1.0
//     });
//
//     assert!(validate_node_settings(&invalid_opacity).is_err());
// }

#[test]
fn test_merge_auto_balance_physics() {
    let update = json!({
        "visualisation": {
            "graphs": {
                "knowledge": {
                    "physics": {
                        "autoBalance": true
                    }
                }
            }
        }
    });

    let auto_balance = update
        .get("visualisation")
        .and_then(|v| v.get("graphs"))
        .and_then(|g| g.get("knowledge"))
        .and_then(|l| l.get("physics"))
        .and_then(|p| p.get("autoBalance"));

    assert!(auto_balance.is_some());
    assert_eq!(auto_balance.unwrap(), true);
}

// #[test]
// fn test_extract_physics_updates() {
//     let update = json!({
//         "visualisation": {
//             "graphs": {
//                 "knowledge": {
//                     "physics": {
//                         "damping": 0.9,
//                         "iterations": 100
//                     }
//                 },
//                 "visionclaw": {
//                     "physics": {
//                         "temperature": 0.02
//                     }
//                 }
//             }
//         }
//     });
//
//     let physics_updates = extract_physics_updates(&update);
//
//     assert_eq!(physics_updates.len(), 2);
//     assert!(physics_updates.contains_key("knowledge"));
//     assert!(physics_updates.contains_key("visionclaw"));
//     assert_eq!(physics_updates["knowledge"]["damping"], 0.9);
//     assert_eq!(physics_updates["visionclaw"]["temperature"], 0.02);
// }

#[test]
fn test_file_io_error_scenarios() {
    // Legacy YAML file persistence was removed — save() is now a no-op
    // that returns Ok(()) for backwards compatibility (settings live in Neo4j).
    let settings = visionclaw_server::config::AppFullSettings::default();
    let result = settings.save();
    assert!(result.is_ok(), "save() should be a no-op returning Ok");
}
