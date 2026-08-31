//! Public-demo read-only guard (pre-demo security fix #1, audit 2026-08-21).
//!
//! When `PUBLIC_DEMO=read-only` (or `1`/`true`/`on`) is set, every mutating
//! HTTP method (anything other than GET/HEAD/OPTIONS) against the wrapped scope
//! is rejected with 403 before it reaches a handler. This closes the confirmed
//! CRITICAL + HIGH findings in one place: self-service NIP-98 auth means any
//! attacker keypair satisfies the "Authenticated" tier, so per-handler auth
//! cannot be trusted to keep the public surface read-only — the guard makes it
//! structural.
//!
//! Default OFF: with the env var unset the middleware is inert and passes every
//! request through unchanged, so non-demo deployments are unaffected.

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::Method,
    Error, HttpResponse,
};
use futures::future::LocalBoxFuture;
use log::warn;
use std::future::{ready, Ready};

const PUBLIC_DEMO_ENV: &str = "PUBLIC_DEMO";

/// True when the deployment is in public read-only demo mode.
fn public_demo_read_only() -> bool {
    std::env::var(PUBLIC_DEMO_ENV)
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "read-only" || v == "readonly" || v == "1" || v == "true" || v == "on"
        })
        .unwrap_or(false)
}

/// A method is safe (read-only) if it cannot mutate server state.
fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

pub struct PublicDemoGuard {
    enabled: bool,
}

impl PublicDemoGuard {
    /// Reads `PUBLIC_DEMO` once at construction (app start). Toggling the env
    /// var at runtime is intentionally not supported — the posture is fixed for
    /// the lifetime of the process.
    pub fn from_env() -> Self {
        let enabled = public_demo_read_only();
        if enabled {
            warn!(
                "PUBLIC_DEMO read-only guard ACTIVE: all non-GET/HEAD/OPTIONS requests to the guarded scope will be rejected with 403"
            );
        }
        Self { enabled }
    }
}

impl<S, B> Transform<S, ServiceRequest> for PublicDemoGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = PublicDemoGuardService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(PublicDemoGuardService {
            service,
            enabled: self.enabled,
        }))
    }
}

pub struct PublicDemoGuardService<S> {
    service: S,
    enabled: bool,
}

impl<S, B> Service<ServiceRequest> for PublicDemoGuardService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if self.enabled && !is_safe_method(req.method()) {
            let method = req.method().clone();
            let path = req.path().to_string();
            warn!("PUBLIC_DEMO: rejected {} {} (read-only mode)", method, path);
            let (request, _payload) = req.into_parts();
            let response = HttpResponse::Forbidden()
                .json(serde_json::json!({
                    "error": "read_only_demo",
                    "message": "This is a public read-only demo; mutating requests are disabled.",
                }))
                .map_into_right_body();
            return Box::pin(async move { Ok(ServiceResponse::new(request, response)) });
        }

        let fut = self.service.call(req);
        Box::pin(async move { fut.await.map(ServiceResponse::map_into_left_body) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    // The env var is process-global; these two paths are exercised by forcing
    // the guard's `enabled` flag directly via a small constructor shim rather
    // than mutating shared env (which races under the test harness).
    fn guard(enabled: bool) -> PublicDemoGuard {
        PublicDemoGuard { enabled }
    }

    #[actix_web::test]
    async fn blocks_post_when_enabled() {
        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .wrap(guard(true))
                    .route(
                        "/x",
                        web::post().to(|| async { HttpResponse::Ok().body("mutated") }),
                    )
                    .route(
                        "/x",
                        web::get().to(|| async { HttpResponse::Ok().body("read") }),
                    ),
            ),
        )
        .await;

        let resp =
            test::call_service(&app, test::TestRequest::post().uri("/api/x").to_request()).await;
        assert_eq!(resp.status(), 403);
    }

    #[actix_web::test]
    async fn allows_get_when_enabled() {
        let app = test::init_service(App::new().service(
            web::scope("/api").wrap(guard(true)).route(
                "/x",
                web::get().to(|| async { HttpResponse::Ok().body("read") }),
            ),
        ))
        .await;

        let resp =
            test::call_service(&app, test::TestRequest::get().uri("/api/x").to_request()).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn passthrough_when_disabled() {
        let app = test::init_service(App::new().service(
            web::scope("/api").wrap(guard(false)).route(
                "/x",
                web::post().to(|| async { HttpResponse::Ok().body("mutated") }),
            ),
        ))
        .await;

        let resp =
            test::call_service(&app, test::TestRequest::post().uri("/api/x").to_request()).await;
        assert!(resp.status().is_success());
    }
}
