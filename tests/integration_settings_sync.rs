// Test disabled - references deprecated/removed modules (visionclaw_server::actors::settings_actor, visionclaw_server::utils::validation::rate_limit)
// Actor module structure has changed per ADR-001; settings_actor has moved
/*
//! Integration tests for settings sync functionality
//!
//! Tests the complete settings synchronization flow:
//! - REST API endpoints with bloom field validation
//! - Server acceptance and processing of bloom settings
//! - Bidirectional sync between client and server
//! - Nostr authentication and settings persistence
//! - Rate limiting and security measures
//! - Error handling and recovery scenarios

use actix_web::{test, web, App};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

// Import project modules
use visionclaw_server::actors::settings_actor::SettingsActor;
use visionclaw_server::config::AppFullSettings;
use visionclaw_server::handlers::nostr_handler;
use visionclaw_server::utils::validation::rate_limit::RateLimiter;
use visionclaw_server::{app_state::AppState, handlers::settings_handler::config};

/// Test data for bloom/glow settings with comprehensive validation
mod test_data {
    use super::*;

    pub fn valid_bloom_settings() -> Value {
        json!({
            "visualisation": {
                "glow": {
                    "enabled": true,
                    "intensity": 2.0
                }
            }
        })
    }

    pub fn invalid_bloom_settings() -> Vec<(Value, &'static str)> {
        vec![
            (
                json!({
                    "visualisation": {
                        "glow": {
                            "intensity": -1.0
                        }
                    }
                }),
                "negative intensity",
            ),
        ]
    }

    pub fn nostr_test_event() -> Value {
        json!({
            "id": "test_event_id_12345",
            "pubkey": "test_pubkey_abcdef1234567890",
            "content": "Authenticate to VisionClaw",
            "sig": "test_signature_fedcba0987654321",
            "created_at": 1640995200,
            "kind": 22242,
            "tags": [
                ["relay", "wss://relay.damus.io"],
                ["challenge", "test_challenge_uuid"]
            ]
        })
    }
}

/// Integration test suite for settings sync functionality
#[cfg(test)]
mod settings_sync_tests {
    use super::*;
    use actix_web::http::StatusCode;

    /// Helper to create test app with full settings handler
    async fn create_test_app() -> impl actix_web::dev::Service<
        actix_web::dev::ServiceRequest,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        let app_settings = AppFullSettings::new().expect("Failed to create default settings");
        let settings_actor = Arc::new(Mutex::new(SettingsActor::new(app_settings)));

        let app_state = AppState::new_test_state(settings_actor);

        test::init_service(
            App::new()
                .app_data(web::Data::new(app_state))
                .configure(config)
                .configure(nostr_handler::config),
        )
        .await
    }

    #[tokio::test]
    async fn test_get_settings_endpoint() {
        let app = create_test_app().await;

        let req = test::TestRequest::get().uri("/settings").to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::OK);
    }
}

/// Tests for Nostr authentication with settings persistence
#[cfg(test)]
mod nostr_auth_tests {
    use super::*;

    #[tokio::test]
    async fn test_nostr_auth_flow() {
        // Test implementation
    }
}

/// Advanced integration tests for error handling and edge cases
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_malformed_json_handling() {
        // Test implementation
    }
}

/// Performance and stress tests
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_settings_response_time() {
        // Test implementation
    }
}
*/
