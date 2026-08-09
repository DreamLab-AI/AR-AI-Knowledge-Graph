//! GOV-4 reachability guard: `POST /api/ingest/writeback` must be registered.
//!
//! The agentbox git-bridge WriteBackSaga POSTs an approved-enrichment write-back
//! to `POST /api/ingest/writeback` (`git-bridge.js:733`). That route was never
//! registered on `main`, so every WriteBackSaga call 404'd and the approve →
//! write-back loop was broken (GOV-4). ADR-130 Decision 2 made the
//! enrichment-decide handler the single decision core; the route now adapts the
//! git-bridge payload onto that core.
//!
//! Idiom mirrors `tests/resd_class_count_route.rs`: an `actix` `test::init_service`
//! App with the writeback scope but NO `AppState`, so a reached handler fails at
//! `web::Data<AppState>` extraction (→ 500) rather than 404-ing. A non-404 proves
//! the route MATCHED; a 404 would mean it is unregistered (the exact GOV-4 bug).

use actix_web::{http::StatusCode, test, web, App};
use visionclaw_server::handlers::configure_ingest_writeback_routes;

#[actix_web::test]
async fn ingest_writeback_route_is_reachable() {
    let app = test::init_service(
        App::new().service(web::scope("/api").configure(configure_ingest_writeback_routes)),
    )
    .await;

    // A well-formed git-bridge write-back body so JSON extraction succeeds and
    // the request reaches the handler (which then 500s only because AppState /
    // the ClientCoordinator Data are intentionally absent in this harness).
    let body = serde_json::json!({
        "remoteId": "remote-1",
        "enrichment": {"targetPath": "pages/foo.md", "content": "x"},
        "decision": {
            "caseId": "vc-elev-foo",
            "decision": "approve",
            "approvedBy": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "reasoning": "looks good"
        }
    });

    let req = test::TestRequest::post()
        .uri("/api/ingest/writeback")
        .set_json(&body)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "POST /api/ingest/writeback must be reachable (the git-bridge WriteBackSaga target), got {}",
        resp.status()
    );
}

#[actix_web::test]
async fn ingest_writeback_rejects_get() {
    // The route is POST-only; a GET must not match it (405/404 both acceptable —
    // the assertion is only that GET does not reach the POST handler).
    let app = test::init_service(
        App::new().service(web::scope("/api").configure(configure_ingest_writeback_routes)),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/api/ingest/writeback")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "GET must not reach the POST write-back handler, got {}",
        resp.status()
    );
}
