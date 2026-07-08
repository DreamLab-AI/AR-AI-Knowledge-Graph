//! REC-1a / REC-1b regression guard (PRD-023 WP-12, CANARY-VC-REC1-ROUTE).
//!
//! These auth fixes ALREADY landed on `main` — `/ontology-agent/propose` is
//! gated `RequireAuth::authenticated()` (ontology_agent_handler.rs:344-362) and
//! `/ontology/{load,load-axioms}` are gated `power_user().mutations_only()`
//! (api_handler/ontology/mod.rs:1361-1423). This guard is the one-shot regression
//! canary: it asserts the ontology INGEST routes stay behind their auth gate and
//! that the read side stays anonymous, so a later refactor cannot silently drop
//! the gate. Idiom mirrors the actix `test::init_service` handler tests already
//! in `ontology/mod.rs`.

use actix_web::{http::StatusCode, test, web, App};
use visionclaw_server::handlers::api_handler::ontology as ontology_routes;
use visionclaw_server::handlers::configure_ontology_agent_routes;
use visionclaw_server::services::nostr_service::NostrService;

/// A default `NostrService` with no valid sessions: the auth middleware runs its
/// REAL verification path against it, so an unauthenticated request is rejected
/// by the gate itself — not by a "service missing" shortcut.
fn nostr_data() -> web::Data<NostrService> {
    web::Data::new(NostrService::default())
}

fn is_auth_rejected(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}

#[actix_web::test]
async fn ontology_agent_propose_rejects_unauthenticated_ingest() {
    let app = test::init_service(
        App::new()
            .app_data(nostr_data())
            .configure(configure_ontology_agent_routes),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/ontology-agent/propose")
        .set_json(serde_json::json!({ "note": "x" }))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert!(
        is_auth_rejected(resp.status()),
        "unauthenticated POST /ontology-agent/propose must be auth-rejected, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn ontology_agent_read_side_is_not_auth_gated() {
    // Positive control: /propose's gate is SPECIFIC. The read routes stay
    // anonymous (WS-1/ADR-120), so GET /status reaches its handler (which then
    // 5xx's without AppState) rather than being auth-rejected.
    let app = test::init_service(
        App::new()
            .app_data(nostr_data())
            .configure(configure_ontology_agent_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/ontology-agent/status")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert!(
        !is_auth_rejected(resp.status()),
        "read-side GET /ontology-agent/status must not be auth-gated, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn ontology_load_rejects_unauthenticated_ingest() {
    let app = test::init_service(
        App::new()
            .app_data(nostr_data())
            .configure(ontology_routes::config),
    )
    .await;

    for path in ["/ontology/load", "/ontology/load-axioms"] {
        let req = test::TestRequest::post()
            .uri(path)
            .set_json(serde_json::json!({}))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            is_auth_rejected(resp.status()),
            "unauthenticated POST {path} must be power_user-gated, got {}",
            resp.status()
        );
    }
}

#[actix_web::test]
async fn ontology_read_get_stays_public() {
    // mutations_only bypasses safe GETs: /ontology/classes reaches its handler
    // (5xx without AppState) rather than being auth-rejected — proving the gate
    // is mutation-specific, not a blanket scope lock.
    let app = test::init_service(
        App::new()
            .app_data(nostr_data())
            .configure(ontology_routes::config),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/ontology/classes")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert!(
        !is_auth_rejected(resp.status()),
        "read-side GET /ontology/classes must stay public, got {}",
        resp.status()
    );
}
