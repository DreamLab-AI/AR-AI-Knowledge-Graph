//! RES-d reachability guard (PRD-023 WP-12, CANARY-VC-RESD-COUNT).
//!
//! `GET /api/ontology/class-count` is the script-queryable source the canon
//! `DriftCounter` polls. It was registered AFTER the broad `/ontology` scope
//! (inside `api_handler::ontology::config`), so actix — which matches scopes by
//! registration order and does NOT fall through a matched scope prefix — routed
//! `/ontology/class-count` into the `/ontology` scope, found no inner route, and
//! 404'd every time. The endpoint was unreachable and the canary could never
//! fire.
//!
//! The fix registers the more-specific `/ontology/class-count` scope BEFORE the
//! broad `/ontology` scope (mirroring the WS-9 `/ontology/derived` idiom). These
//! tests pin BOTH halves of the mechanism so a future reorder cannot silently
//! reintroduce the 404:
//!   1. registered before the broad scope ⇒ reachable (not 404);
//!   2. registered after  the broad scope ⇒ shadowed (404) — the exact bug.
//!
//! Idiom mirrors `tests/rec1_route_guard.rs`: an `actix` `test::init_service`
//! App with no `AppState`, so a reached handler 500s (Data extraction fails)
//! rather than 404-ing — a non-404 status proves the route MATCHED.

use actix_web::{http::StatusCode, test, web, App};
use visionclaw_server::handlers::api_handler::ontology as ontology_routes;
use visionclaw_server::handlers::configure_ontology_class_count_routes;
use visionclaw_server::services::nostr_service::NostrService;

/// A default `NostrService` so the broad `/ontology` scope's `RequireAuth`
/// middleware can construct; a GET bypasses the `mutations_only()` gate.
fn nostr_data() -> web::Data<NostrService> {
    web::Data::new(NostrService::default())
}

#[actix_web::test]
async fn class_count_is_reachable_when_registered_before_broad_ontology_scope() {
    // The FIX: the specific /ontology/class-count scope is registered BEFORE the
    // broad /ontology scope, exactly as main.rs now orders it. GET reaches its
    // handler (which 500s here only because AppState is intentionally absent)
    // rather than being 404-shadowed.
    let app = test::init_service(
        App::new()
            .app_data(nostr_data())
            .configure(configure_ontology_class_count_routes)
            .configure(ontology_routes::config),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/ontology/class-count")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "GET /ontology/class-count must be reachable (not shadowed by /ontology), got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn class_count_is_shadowed_when_registered_after_broad_scope() {
    // Regression documentation: the ORIGINAL (buggy) order — broad /ontology
    // scope FIRST — shadows /ontology/class-count to a 404, because actix scopes
    // match by registration order and do not fall through a matched prefix. This
    // is the exact failure RES-d fixes by reordering; pinning the mechanism means
    // a future reorder that re-buries class-count fails this test loudly.
    let app = test::init_service(
        App::new()
            .app_data(nostr_data())
            .configure(ontology_routes::config)
            .configure(configure_ontology_class_count_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/ontology/class-count")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "buggy order: the broad /ontology scope shadows class-count → 404, got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn class_count_route_alone_reaches_its_handler() {
    // Sanity control: with ONLY the class-count scope mounted, the path matches
    // its route (→ non-404). Isolates the routing from the shadowing question.
    let app = test::init_service(
        App::new().configure(configure_ontology_class_count_routes),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/ontology/class-count")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "the class-count route in isolation must match its own path, got {}",
        resp.status()
    );
}
