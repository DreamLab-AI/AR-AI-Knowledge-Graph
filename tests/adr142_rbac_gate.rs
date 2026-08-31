//! ADR-142 multi-user RBAC route-guard tests.
//!
//! Exercises the central `RbacGate` wrapping a representative `/api` scope plus
//! the RBAC admin surface, asserting the policy that closes the AUTH-001
//! unauthenticated-mutation gap:
//!   - public reads pass through,
//!   - unauthenticated mutations are rejected,
//!   - the auth endpoints stay allowlisted,
//!   - `/api/admin/*` is rejected for unauthenticated callers on every method.
//!
//! Idiom mirrors `tests/rec1_route_guard.rs`: a default `NostrService` runs the
//! REAL verification path, so rejection comes from the gate, not a shortcut.

use actix_web::{http::StatusCode, test, web, App, HttpResponse};
use visionclaw_server::handlers::admin_rbac_handler;
use visionclaw_server::middleware::RbacGate;
use visionclaw_server::services::nostr_service::NostrService;

fn nostr_data() -> web::Data<NostrService> {
    web::Data::new(NostrService::default())
}

fn is_auth_rejected(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}

/// Build an `/api` scope wrapped by the gate, with a couple of trivial handlers
/// so we observe whether a request reaches the handler (200) or is stopped by
/// the gate (401/403).
macro_rules! api_app {
    () => {{
        // Match the current single-operator deployment: anonymous reads on.
        // (Default is fail-closed; the deployment sets this explicitly.)
        std::env::set_var("RBAC_PUBLIC_READS", "1");
        App::new().app_data(nostr_data()).service(
            web::scope("/api")
                .wrap(RbacGate::from_env())
                .configure(admin_rbac_handler::configure_routes)
                .route(
                    "/graph/data",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                )
                .route(
                    "/graph/update",
                    web::post().to(|| async { HttpResponse::Ok().finish() }),
                )
                .route(
                    "/auth/nostr",
                    web::post().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
    }};
}

#[actix_web::test]
async fn public_read_passes_through_gate() {
    // RBAC_GATE_MODE defaults to enforce (unset); a public GET must still reach
    // its handler and return 200 rather than being auth-rejected.
    let app = test::init_service(api_app!()).await;
    let req = test::TestRequest::get().uri("/api/graph/data").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "public GET /api/graph/data must pass the gate"
    );
}

#[actix_web::test]
async fn unauthenticated_mutation_is_rejected() {
    let app = test::init_service(api_app!()).await;
    let req = test::TestRequest::post()
        .uri("/api/graph/update")
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        is_auth_rejected(resp.status()),
        "unauthenticated POST /api/graph/update must be gated, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn auth_endpoint_is_allowlisted() {
    let app = test::init_service(api_app!()).await;
    // The login endpoint itself must NOT require a session — otherwise no one
    // could ever authenticate.
    let req = test::TestRequest::post()
        .uri("/api/auth/nostr")
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "POST /api/auth/nostr must be allowlisted, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn admin_surface_rejects_unauthenticated_reads_and_writes() {
    let app = test::init_service(api_app!()).await;

    let get = test::TestRequest::get()
        .uri("/api/admin/rbac/users")
        .to_request();
    assert!(
        is_auth_rejected(test::call_service(&app, get).await.status()),
        "unauthenticated GET /api/admin/rbac/users must be rejected"
    );

    let put = test::TestRequest::put()
        .uri("/api/admin/rbac/users/deadbeef/role")
        .set_json(serde_json::json!({ "role": "admin" }))
        .to_request();
    assert!(
        is_auth_rejected(test::call_service(&app, put).await.status()),
        "unauthenticated PUT /api/admin/rbac/users/*/role must be rejected"
    );
}
