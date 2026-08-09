// Route configuration and read-only route handler functions

use crate::actors::messages::GetSettings;
use crate::app_state::AppState;
use crate::config::path_access::JsonPathAccessible;
use crate::{error_json, not_found, ok_json, service_unavailable};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use log::{error, warn};
use serde_json::{json, Value};
use std::borrow::Cow;

use super::enhanced::EnhancedSettingsHandler;
use super::types::SettingsResponseDTO;
use super::write_handlers;

pub fn config(cfg: &mut web::ServiceConfig) {
    let handler = web::Data::new(EnhancedSettingsHandler::new());

    cfg.app_data(handler.clone())
        .service(
            web::scope("/settings")
                .route("/path", web::get().to(get_setting_by_path))
                .route("/path", web::put().to(update_setting_by_path))
                .route("/schema", web::get().to(get_settings_schema))
                .route("/current", web::get().to(get_current_settings))
                .route("", web::get().to(get_settings))
                .route("", web::post().to(write_handlers::update_settings))
                .route("/reset", web::post().to(write_handlers::reset_settings))
                .route("/save", web::post().to(write_handlers::save_settings))
                .route(
                    "/validation/stats",
                    web::get().to(
                        |req, handler: web::Data<EnhancedSettingsHandler>| async move {
                            handler.get_validation_stats(req).await
                        },
                    ),
                ),
        )
        .service(web::scope("/api/physics").route(
            "/compute-mode",
            web::post().to(super::physics::update_compute_mode),
        ))
        .service(web::scope("/api/constraints").route(
            "/update",
            web::post().to(super::physics::update_constraints),
        ))
        .service(web::scope("/api/stress").route(
            "/optimization",
            web::post().to(super::physics::update_stress_optimization),
        ));
}

async fn get_setting_by_path(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let path = req
        .query_string()
        .split('&')
        .find(|param| param.starts_with("path="))
        .and_then(|p| p.strip_prefix("path="))
        .map(|p| {
            urlencoding::decode(p)
                .unwrap_or(Cow::Borrowed(p))
                .to_string()
        })
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'path' query parameter"))?;

    let app_settings = match state.settings_addr.send(GetSettings).await {
        Ok(Ok(settings)) => settings,
        Ok(Err(e)) => {
            error!("Failed to get settings: {}", e);
            return error_json!("Failed to retrieve settings");
        }
        Err(e) => {
            error!("Settings actor error: {}", e);
            return service_unavailable!("Settings service unavailable");
        }
    };

    match app_settings.get_json_by_path(&path) {
        Ok(value_json) => ok_json!(json!({
            "success": true,
            "path": path,
            "value": value_json
        })),
        Err(e) => {
            warn!("Path not found '{}': {}", path, e);
            not_found!("Path not found", e)
        }
    }
}

async fn update_setting_by_path(
    _req: HttpRequest,
    _auth: crate::settings::auth_extractor::AuthenticatedUser,
    state: web::Data<AppState>,
    payload: web::Json<Value>,
) -> Result<HttpResponse, Error> {
    use super::physics::propagate_physics_to_gpu;
    use crate::actors::messages::UpdateSettings;
    use crate::bad_request;
    use log::info;

    let update = payload.into_inner();
    let path = update
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'path' in request body"))?
        .to_string();
    let value = update
        .get("value")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'value' in request body"))?
        .clone();

    let mut app_settings = match state.settings_addr.send(GetSettings).await {
        Ok(Ok(settings)) => settings,
        Ok(Err(e)) => {
            error!("Failed to get settings: {}", e);
            return error_json!("Failed to retrieve settings");
        }
        Err(e) => {
            error!("Settings actor error: {}", e);
            return service_unavailable!("Settings service unavailable");
        }
    };

    let previous_value = app_settings.get_json_by_path(&path).ok();

    match app_settings.set_json_by_path(&path, value.clone()) {
        Ok(()) => {
            match state
                .settings_addr
                .send(UpdateSettings {
                    settings: app_settings.clone(),
                })
                .await
            {
                Ok(Ok(())) => {
                    info!("Updated setting at path: {}", path);

                    if path.contains(".physics.")
                        || path.contains(".graphs.logseq.")
                        || path.contains(".graphs.visionclaw.")
                    {
                        info!("Physics setting changed, propagating to GPU actors");

                        let graph_name = if path.contains(".graphs.logseq.") {
                            "logseq"
                        } else if path.contains(".graphs.visionclaw.") {
                            "visionclaw"
                        } else {
                            "logseq"
                        };

                        propagate_physics_to_gpu(&state, &app_settings, graph_name).await;
                    }

                    ok_json!(json!({
                        "success": true,
                        "path": path,
                        "value": update.get("value").cloned().unwrap_or(Value::Null),
                        "previousValue": previous_value
                    }))
                }
                Ok(Err(e)) => {
                    error!("Failed to save settings: {}", e);
                    Ok(HttpResponse::InternalServerError().json(json!({
                        "error": format!("Failed to save settings: {}", e),
                        "path": path
                    })))
                }
                Err(e) => {
                    error!("Settings actor error: {}", e);
                    service_unavailable!("Settings service unavailable")
                }
            }
        }
        Err(e) => {
            warn!("Failed to update path '{}': {}", path, e);
            bad_request!("Invalid path or value", e)
        }
    }
}

async fn get_settings_schema(
    req: HttpRequest,
    _state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let path = req
        .query_string()
        .split('&')
        .find(|param| param.starts_with("path="))
        .and_then(|p| p.strip_prefix("path="))
        .map(|p| {
            urlencoding::decode(p)
                .unwrap_or(Cow::Borrowed(p))
                .to_string()
        })
        .unwrap_or_default();

    let schema = json!({
        "type": "object",
        "properties": {
            "damping": { "type": "number", "description": "Physics damping factor (0.0-1.0)" },
            "gravity": { "type": "number", "description": "Physics gravity strength" },
        },
        "path": path
    });

    ok_json!(json!({
        "success": true,
        "path": path,
        "schema": schema
    }))
}

/// S3 (secret-leak hardening): the `SettingsResponseDTO` From-conversion copies
/// the real provider API keys (`openai.api_key`, `ragflow.api_key`,
/// `perplexity.api_key`) verbatim from `AppFullSettings`. These full-settings GET
/// routes serve config to unauthenticated callers, so any secret-bearing field
/// MUST be stripped before the body leaves the process. We serialize the DTO to a
/// JSON value and null out the secret fields. Non-secret config (URLs, models,
/// timeouts, physics params) is preserved so unauthenticated clients are not
/// broken — they simply never see a key. Redaction is preferred over an auth gate
/// here because the client interceptor only attaches a token when the user is
/// logged in (a logged-out browser sends no `Authorization` header at all).
fn redact_settings_secrets(mut value: Value) -> Value {
    // The secret fields live under these provider sub-objects (camelCase keys
    // because SettingsResponseDTO uses `#[serde(rename_all = "camelCase")]`).
    const SECRET_BEARING_SECTIONS: [&str; 3] = ["openai", "ragflow", "perplexity"];
    if let Some(obj) = value.as_object_mut() {
        for section in SECRET_BEARING_SECTIONS {
            if let Some(Value::Object(section_obj)) = obj.get_mut(section) {
                // Only redact when a key is actually present; leave the field
                // absent (skip_serializing_if = Option::is_none) otherwise.
                if section_obj.contains_key("apiKey") {
                    section_obj.insert("apiKey".to_string(), Value::Null);
                }
            }
        }
    }
    value
}

async fn get_settings(
    _req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let app_settings = match state.settings_addr.send(GetSettings).await {
        Ok(Ok(settings)) => settings,
        Ok(Err(e)) => {
            error!("Failed to get settings: {}", e);
            return error_json!("Failed to retrieve settings");
        }
        Err(e) => {
            error!("Settings actor error: {}", e);
            return service_unavailable!("Settings service unavailable");
        }
    };

    let response_dto: SettingsResponseDTO = (&app_settings).into();
    let redacted =
        redact_settings_secrets(serde_json::to_value(&response_dto).unwrap_or_else(|_| json!({})));

    ok_json!(redacted)
}

async fn get_current_settings(
    _req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let app_settings = match state.settings_addr.send(GetSettings).await {
        Ok(Ok(settings)) => settings,
        Ok(Err(e)) => {
            error!("Failed to get settings: {}", e);
            return error_json!("Failed to retrieve settings");
        }
        Err(e) => {
            error!("Settings actor error: {}", e);
            return service_unavailable!("Settings service unavailable");
        }
    };

    let response_dto: SettingsResponseDTO = (&app_settings).into();
    let redacted =
        redact_settings_secrets(serde_json::to_value(&response_dto).unwrap_or_else(|_| json!({})));

    ok_json!(json!({
        "settings": redacted,
        "version": app_settings.version,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }))
}

#[cfg(test)]
mod redaction_tests {
    use super::redact_settings_secrets;
    use serde_json::json;

    #[test]
    fn redacts_provider_api_keys_but_keeps_non_secret_config() {
        let input = json!({
            "openai": { "apiKey": "sk-live-SECRET", "baseUrl": "https://api.openai.com", "timeout": 30 },
            "ragflow": { "apiKey": "rag-SECRET", "apiBaseUrl": "https://rag.example", "agentId": "a1" },
            "perplexity": { "apiKey": "pplx-SECRET", "model": "sonar" },
            "visualisation": { "rendering": { "enableShadows": true } }
        });

        let out = redact_settings_secrets(input);

        // Secrets nulled out.
        assert!(out["openai"]["apiKey"].is_null());
        assert!(out["ragflow"]["apiKey"].is_null());
        assert!(out["perplexity"]["apiKey"].is_null());

        // Non-secret config preserved.
        assert_eq!(out["openai"]["baseUrl"], "https://api.openai.com");
        assert_eq!(out["openai"]["timeout"], 30);
        assert_eq!(out["ragflow"]["agentId"], "a1");
        assert_eq!(out["perplexity"]["model"], "sonar");
        assert_eq!(out["visualisation"]["rendering"]["enableShadows"], true);
    }

    #[test]
    fn no_panic_when_provider_section_absent_or_keyless() {
        // Section missing entirely.
        let out = redact_settings_secrets(json!({ "visualisation": {} }));
        assert!(out.get("openai").is_none());

        // Section present without apiKey (skip_serializing_if dropped it).
        let out = redact_settings_secrets(json!({ "openai": { "baseUrl": "x" } }));
        assert!(out["openai"].get("apiKey").is_none());
        assert_eq!(out["openai"]["baseUrl"], "x");
    }
}
