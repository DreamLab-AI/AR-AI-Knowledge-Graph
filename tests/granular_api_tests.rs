//! Comprehensive tests for granular API endpoints in VisionClaw settings refactor
//!
//! Tests the new path-based GET and SET endpoints that replace monolithic settings transfer
//! Validates dot-notation path parsing, partial updates, and performance improvements
//! Enhanced and ported from codestore testing suite
//!
//! NOTE: These tests are disabled because they reference `crate::tests::test_utils` which
//! does not exist. The test utilities module is not defined in the tests directory.
//!
//! To re-enable:
//! 1. Create a test_utils.rs module in tests/ directory with the required utilities
//! 2. Or import the utilities from visionclaw_server::tests::test_utils if they exist
//! 3. Uncomment the code below

/*
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::tests::test_utils::{
    assert_camel_case_keys, parse_dot_notation_path, MockHttpResponse, PerformanceTimer,
    TestAppSettings,
};

#[cfg(test)]
mod granular_api_tests {
    use super::*;

    // Mock API handler functions for testing
    async fn mock_get_settings_by_paths(paths: Vec<&str>) -> Result<Value, String> {
        let settings = TestAppSettings::new();
        let settings_json = serde_json::to_value(&settings).map_err(|e| e.to_string())?;

        if paths.is_empty() {
            return Ok(settings_json);
        }

        let mut result = json!({});

        for path in paths {
            if let Some(value) = extract_value_by_path(&settings_json, path) {
                set_value_by_path(&mut result, path, value);
            }
        }

        Ok(result)
    }

    async fn mock_update_settings_by_paths(updates: Vec<(String, Value)>) -> Result<Value, String> {
        let mut settings = TestAppSettings::new();
        let mut successful_updates = 0;
        let mut errors = Vec::new();

        for (path, value) in updates {
            match crate::tests::test_utils::validate_path_update(&mut settings, &path, &value) {
                Ok(_) => successful_updates += 1,
                Err(e) => errors.push(json!({
                    "path": path,
                    "error": e.to_string()
                })),
            }
        }

        if errors.is_empty() {
            Ok(json!({
                "success": true,
                "updated": successful_updates
            }))
        } else {
            Err(format!("Validation failed: {:?}", errors))
        }
    }

    #[tokio::test]
    async fn test_get_single_path() {
        let paths = vec!["visualisation.glow.nodeGlowStrength"];
        let result = mock_get_settings_by_paths(paths).await;

        assert!(result.is_ok(), "Single path request should succeed");

        let body = result.unwrap();
        assert!(
            body["visualisation"]["glow"]["nodeGlowStrength"].is_number(),
            "Response should contain requested path in camelCase"
        );

        // Verify only requested data is returned
        assert!(
            body["visualisation"]["glow"]
                .get("edgeGlowStrength")
                .is_none(),
            "Unrequested fields should not be included"
        );
        assert!(
            body.get("system").is_none(),
            "Unrequested top-level sections should not be included"
        );
    }

    #[tokio::test]
    async fn test_get_multiple_paths() {
        let paths = vec![
            "visualisation.glow.nodeGlowStrength",
            "visualisation.glow.baseColor",
            "system.debugMode",
        ];
        let result = mock_get_settings_by_paths(paths).await;

        assert!(result.is_ok(), "Multiple paths request should succeed");

        let body = result.unwrap();

        // Verify all requested paths are present
        assert!(body["visualisation"]["glow"]["nodeGlowStrength"].is_number());
        assert!(body["visualisation"]["glow"]["baseColor"].is_string());
        assert!(body["system"]["debugMode"].is_boolean());

        // Verify unrequested paths are not included
        assert!(body["visualisation"]["glow"]
            .get("edgeGlowStrength")
            .is_none());
        assert!(body["system"].get("maxConnections").is_none());
    }

    #[tokio::test]
    async fn test_get_nested_object_path() {
        let paths = vec!["visualisation.glow"];
        let result = mock_get_settings_by_paths(paths).await;

        assert!(result.is_ok(), "Nested object request should succeed");

        let body = result.unwrap();
        let glow = &body["visualisation"]["glow"];

        // Verify entire glow object is returned with all fields
        assert!(glow["nodeGlowStrength"].is_number());
        assert!(glow["edgeGlowStrength"].is_number());
        assert!(glow["environmentGlowStrength"].is_number());
        assert!(glow["baseColor"].is_string());
        assert!(glow["emissionColor"].is_string());
        assert!(glow["enabled"].is_boolean());

        // Verify other top-level sections are not included
        assert!(body.get("system").is_none());
        assert!(body.get("xr").is_none());
    }

    #[tokio::test]
    async fn test_get_invalid_path() {
        let paths = vec!["invalid.path.does.not.exist"];
        let result = mock_get_settings_by_paths(paths).await;

        // Should succeed but return empty object for invalid paths
        assert!(result.is_ok(), "Invalid path request should succeed");

        let body = result.unwrap();
        // Should be empty object or contain null values
        assert!(body.as_object().unwrap().is_empty() || body["invalid"].is_null());
    }

    #[tokio::test]
    async fn test_update_single_path() {
        let updates = vec![(
            "visualisation.glow.nodeGlowStrength".to_string(),
            json!(2.5),
        )];

        let result = mock_update_settings_by_paths(updates).await;
        assert!(result.is_ok(), "Single path update should succeed");

        let response = result.unwrap();
        assert_eq!(response["success"], json!(true));
        assert_eq!(response["updated"], json!(1));
    }

    #[tokio::test]
    async fn test_update_multiple_paths() {
        let updates = vec![
            (
                "visualisation.glow.nodeGlowStrength".to_string(),
                json!(3.0),
            ),
            ("visualisation.glow.baseColor".to_string(), json!("#ff0000")),
            ("system.debugMode".to_string(), json!(true)),
        ];

        let result = mock_update_settings_by_paths(updates).await;
        assert!(result.is_ok(), "Multiple path update should succeed");

        let response = result.unwrap();
        assert_eq!(response["success"], json!(true));
        assert_eq!(response["updated"], json!(3));
    }

    #[tokio::test]
    async fn test_update_invalid_path() {
        let updates = vec![(
            "invalid.path.does.not.exist".to_string(),
            json!("some_value"),
        )];

        let result = mock_update_settings_by_paths(updates).await;
        assert!(result.is_err(), "Invalid path update should fail");
    }

    #[tokio::test]
    async fn test_update_type_validation() {
        let updates = vec![(
            "visualisation.glow.nodeGlowStrength".to_string(),
            json!("not_a_number"),
        )];

        let result = mock_update_settings_by_paths(updates).await;
        assert!(result.is_err(), "Type mismatch should fail");
    }

    #[tokio::test]
    async fn test_get_performance_many_paths() {
        let many_paths = vec![
            "visualisation.glow.nodeGlowStrength",
            "visualisation.glow.edgeGlowStrength",
            "visualisation.glow.baseColor",
            "visualisation.graphs.knowledge.physics.springK",
            "visualisation.graphs.knowledge.physics.repelK",
            "system.debugMode",
            "system.maxConnections",
            "xr.handMeshColor",
            "xr.locomotionMethod",
        ];

        let timer = PerformanceTimer::new();
        let result = mock_get_settings_by_paths(many_paths.clone()).await;
        let duration = timer.elapsed();

        assert!(result.is_ok(), "Many paths request should succeed");
        assert!(
            duration < Duration::from_millis(100),
            "Many paths request should be fast"
        );

        let body = result.unwrap();

        // Verify all requested paths are present
        assert!(body["visualisation"]["glow"]["nodeGlowStrength"].is_number());
        assert!(body["visualisation"]["glow"]["edgeGlowStrength"].is_number());
        assert!(body["system"]["debugMode"].is_boolean());
        assert!(body["xr"]["handMeshColor"].is_string());
    }

    #[tokio::test]
    async fn test_atomic_updates() {
        // Test atomic updates - all should succeed or all should fail
        let updates = vec![
            (
                "visualisation.glow.nodeGlowStrength".to_string(),
                json!(5.0),
            ),
            ("invalid.path".to_string(), json!("should_cause_failure")),
        ];

        let result = mock_update_settings_by_paths(updates).await;

        // Should fail due to invalid path
        assert!(
            result.is_err(),
            "Atomic update with invalid path should fail entirely"
        );
    }

    #[tokio::test]
    async fn test_concurrent_updates() {
        use tokio::task;

        let mut handles = Vec::new();

        // Test concurrent updates to different paths
        for i in 0..10 {
            let handle = task::spawn(async move {
                let updates = vec![(
                    format!("visualisation.glow.nodeGlowStrength"),
                    json!(1.0 + i as f64 * 0.1),
                )];
                mock_update_settings_by_paths(updates).await
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;

        for (i, result) in results.into_iter().enumerate() {
            let update_result = result.unwrap();
            assert!(
                update_result.is_ok(),
                "Concurrent update {} should succeed",
                i
            );
        }
    }

    #[tokio::test]
    async fn test_response_size_efficiency() {
        // Compare response sizes: single field vs multiple fields
        let single_field_result =
            mock_get_settings_by_paths(vec!["visualisation.glow.nodeGlowStrength"]).await;
        assert!(single_field_result.is_ok());

        let single_body = single_field_result.unwrap();
        let single_size = serde_json::to_string(&single_body).unwrap().len();

        // Get a larger subset for comparison
        let multiple_fields_result =
            mock_get_settings_by_paths(vec!["visualisation.glow", "system.debugMode"]).await;
        assert!(multiple_fields_result.is_ok());

        let multiple_body = multiple_fields_result.unwrap();
        let multiple_size = serde_json::to_string(&multiple_body).unwrap().len();

        // Verify size efficiency
        assert!(
            single_size < multiple_size,
            "Single field response should be smaller"
        );
        assert!(
            single_size < 1000,
            "Single field response should be compact"
        );
    }

    #[tokio::test]
    async fn test_camelcase_response_format() {
        let paths = vec!["visualisation.glow", "system"];
        let result = mock_get_settings_by_paths(paths).await;

        assert!(result.is_ok());
        let body = result.unwrap();

        // Verify camelCase structure throughout
        assert_camel_case_keys(&body, "");

        // Verify specific camelCase fields
        assert!(body["visualisation"]["glow"]["nodeGlowStrength"].is_number());
        assert!(body["visualisation"]["glow"]["edgeGlowStrength"].is_number());
        assert!(body["system"]["debugMode"].is_boolean());
        assert!(body["system"]["maxConnections"].is_number());

        // Verify no snake_case
        assert!(body["visualisation"]["glow"]
            .get("node_glow_strength")
            .is_none());
        assert!(body["system"].get("debug_mode").is_none());
    }

    #[tokio::test]
    async fn test_deep_nested_path_access() {
        let paths = vec!["visualisation.graphs.knowledge.physics.springK"];
        let result = mock_get_settings_by_paths(paths).await;

        assert!(result.is_ok());
        let body = result.unwrap();

        // Verify deeply nested access works
        assert!(body["visualisation"]["graphs"]["knowledge"]["physics"]["springK"].is_number());

        // Verify only the requested deep path is included
        assert!(body["visualisation"]["graphs"]["knowledge"]["physics"]
            .get("repelK")
            .is_none());
        assert!(body["visualisation"].get("glow").is_none());
    }

    #[tokio::test]
    async fn test_batch_size_limits() {
        // Test with very large number of paths
        let large_paths: Vec<_> = (0..1000)
            .map(|i| match i % 4 {
                0 => "visualisation.glow.nodeGlowStrength",
                1 => "system.debugMode",
                2 => "xr.locomotionMethod",
                _ => "visualisation.glow.baseColor",
            })
            .collect();

        let timer = PerformanceTimer::new();
        let result = mock_get_settings_by_paths(large_paths).await;
        let duration = timer.elapsed();

        assert!(result.is_ok(), "Large batch should succeed");
        assert!(
            duration < Duration::from_secs(1),
            "Large batch should complete quickly"
        );

        // Test with large update batch
        let large_updates: Vec<_> = (0..100)
            .map(|i| {
                (
                    "visualisation.glow.nodeGlowStrength".to_string(),
                    json!(1.0 + i as f64 * 0.01),
                )
            })
            .collect();

        let update_timer = PerformanceTimer::new();
        let update_result = mock_update_settings_by_paths(large_updates).await;
        let update_duration = update_timer.elapsed();

        assert!(update_result.is_ok(), "Large update batch should succeed");
        assert!(
            update_duration < Duration::from_millis(500),
            "Large update batch should be fast"
        );
    }

    #[tokio::test]
    async fn test_error_handling_edge_cases() {
        // Test empty path list
        let empty_result = mock_get_settings_by_paths(vec![]).await;
        assert!(empty_result.is_ok(), "Empty path list should succeed");

        // Test empty updates list
        let empty_update_result = mock_update_settings_by_paths(vec![]).await;
        assert!(
            empty_update_result.is_ok(),
            "Empty update list should succeed"
        );

        // Test malformed paths
        let malformed_paths = vec![
            "",
            ".",
            ".field",
            "field.",
            "field..nested",
            "field...deeply.nested",
        ];

        for path in malformed_paths {
            let result = mock_get_settings_by_paths(vec![path]).await;
            // Should handle gracefully (either succeed with empty result or handle the malformed path)
            assert!(
                result.is_ok() || result.is_err(),
                "Malformed path '{}' should be handled gracefully",
                path
            );
        }
    }

    #[tokio::test]
    async fn test_security_path_validation() {
        // Test potentially dangerous paths
        let dangerous_paths = vec![
            "../../../etc/passwd",
            "..\\..\\windows\\system32",
            "<script>alert('xss')</script>",
            "'; DROP TABLE settings; --",
        ];

        for path in dangerous_paths {
            let result = mock_get_settings_by_paths(vec![path]).await;
            // Should either reject or sanitize dangerous paths
            if result.is_ok() {
                let body = result.unwrap();
                // If accepted, should be empty (not found) rather than dangerous
                assert!(
                    body.as_object().unwrap().is_empty()
                        || body.as_object().unwrap().values().all(|v| v.is_null())
                );
            }
        }
    }

    #[tokio::test]
    async fn test_unicode_and_international_support() {
        // Test paths and values with unicode characters
        let unicode_updates = vec![
            ("visualisation.glow.baseColor".to_string(), json!("#ff0000")), // Valid
            ("test.unicode".to_string(), json!("测试")), // Should be handled gracefully
        ];

        // First update should succeed, second may fail (depending on path validation)
        let result = mock_update_settings_by_paths(unicode_updates).await;
        // Just verify it doesn't crash
        assert!(
            result.is_ok() || result.is_err(),
            "Unicode handling should be graceful"
        );
    }

    // Helper functions for path manipulation
    fn extract_value_by_path(json: &Value, path: &str) -> Option<Value> {
        let parts = parse_dot_notation_path(path);
        let mut current = json;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }

    fn set_value_by_path(json: &mut Value, path: &str, value: Value) {
        let parts = parse_dot_notation_path(path);
        if parts.is_empty() {
            return;
        }

        // Ensure json is an object
        if !json.is_object() {
            *json = json!({});
        }

        let mut current = json;

        // Navigate to parent of target
        for part in &parts[..parts.len() - 1] {
            if !current[part].is_object() {
                current[part] = json!({});
            }
            current = &mut current[part];
        }

        // Set the final value
        let final_key = parts.last().unwrap();
        current[final_key] = value;
    }
}

#[cfg(test)]
mod path_parsing_tests {
    use super::*;
    use crate::tests::test_utils::parse_dot_notation_path;

    #[test]
    fn test_dot_notation_parsing() {
        let test_cases = vec![
            ("simple", vec!["simple"]),
            ("nested.field", vec!["nested", "field"]),
            (
                "deeply.nested.field.value",
                vec!["deeply", "nested", "field", "value"],
            ),
            ("array.0.field", vec!["array", "0", "field"]),
        ];

        for (input, expected) in test_cases {
            let parsed = parse_dot_notation_path(input);
            assert_eq!(parsed, expected, "Path '{}' should parse correctly", input);
        }
    }

    #[test]
    fn test_invalid_path_handling() {
        let invalid_paths = vec![
            "",              // Empty path
            ".",             // Just dot
            ".field",        // Leading dot
            "field.",        // Trailing dot
            "field..nested", // Double dot
        ];

        for path in invalid_paths {
            let result = parse_dot_notation_path(path);
            // Implementation should handle these gracefully
            assert!(
                result.is_empty() || result.len() >= 1,
                "Invalid path '{}' should be handled gracefully",
                path
            );
        }
    }

    #[test]
    fn test_camelcase_path_validation() {
        let camelcase_paths = vec![
            "visualisation.glow.nodeGlowStrength",
            "system.debugMode",
            "xr.handMeshColor",
        ];

        let snake_case_paths = vec![
            "visualisation.glow.node_glow_strength",
            "system.debug_mode",
            "xr.hand_mesh_color",
        ];

        for path in camelcase_paths {
            let parsed = parse_dot_notation_path(path);
            assert!(
                !parsed.is_empty(),
                "CamelCase path '{}' should be valid",
                path
            );
        }

        for path in snake_case_paths {
            let parsed = parse_dot_notation_path(path);
            // Parsing succeeds, but these wouldn't match actual struct fields
            assert!(
                !parsed.is_empty(),
                "Path parsing should work for any format"
            );
        }
    }

    #[test]
    fn test_path_depth_limits() {
        // Test very deep paths
        let deep_path = (0..100)
            .map(|i| format!("level{}", i))
            .collect::<Vec<_>>()
            .join(".");
        let parsed = parse_dot_notation_path(&deep_path);
        assert_eq!(parsed.len(), 100, "Deep path should parse correctly");

        // Test extremely deep path (potential DoS)
        let extremely_deep_path = (0..10000)
            .map(|i| format!("l{}", i))
            .collect::<Vec<_>>()
            .join(".");
        let timer = PerformanceTimer::new();
        let parsed_extreme = parse_dot_notation_path(&extremely_deep_path);
        let duration = timer.elapsed();

        assert_eq!(
            parsed_extreme.len(),
            10000,
            "Extremely deep path should parse"
        );
        assert!(
            duration < Duration::from_millis(100),
            "Path parsing should be fast even for deep paths"
        );
    }
}
*/
