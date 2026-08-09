use actix_web::{web, HttpResponse, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::ok_json;
use crate::utils::network::CircuitBreakerStats;
use crate::AppState;

/// Wrapper around `Instant` so it can be registered as Actix app data.
#[derive(Clone)]
pub struct ProcessStartTime(pub Instant);

#[derive(Serialize)]
pub struct MetricsResponse {
    pub uptime_secs: u64,
    pub active_connections: usize,
    pub event_bus: EventBusMetrics,
    pub circuit_breakers: HashMap<String, CircuitBreakerStats>,
}

#[derive(Serialize)]
pub struct EventBusMetrics {
    pub published_counts: HashMap<String, usize>,
    pub handler_counts: HashMap<String, usize>,
    pub error_counts: HashMap<String, usize>,
}

/// GET /api/metrics
///
/// Returns JSON with process uptime, active WebSocket connections,
/// event bus publish/handler/error counters, and circuit breaker states.
pub async fn get_metrics(
    app_state: web::Data<AppState>,
    start_time: web::Data<ProcessStartTime>,
) -> Result<HttpResponse> {
    let uptime_secs = start_time.0.elapsed().as_secs();
    let active_connections = app_state.active_connections.load(Ordering::Relaxed);

    // Collect event bus metrics from the middleware chain.
    // The MetricsMiddleware stores per-type counters that we snapshot here.
    let event_bus_metrics = collect_event_bus_metrics(&app_state).await;

    // Circuit breaker stats are not currently registered as shared app data.
    // Return an empty map; when a global registry is wired into AppState this
    // will automatically populate.
    let circuit_breakers: HashMap<String, CircuitBreakerStats> = HashMap::new();

    let response = MetricsResponse {
        uptime_secs,
        active_connections,
        event_bus: event_bus_metrics,
        circuit_breakers,
    };

    ok_json!(response)
}

/// Walk the EventBus middleware list, downcast any MetricsMiddleware instances,
/// and snapshot their counters. If no MetricsMiddleware is registered the
/// response fields will be empty maps.
async fn collect_event_bus_metrics(app_state: &web::Data<AppState>) -> EventBusMetrics {
    use crate::events::middleware::MetricsMiddleware;
    use std::any::Any;

    let bus = app_state.event_bus.read().await;
    let middlewares = bus.middlewares().await;

    let mut published_counts = HashMap::new();
    let mut handler_counts = HashMap::new();
    let mut error_counts = HashMap::new();

    for mw in &middlewares {
        // Attempt to downcast the trait object to MetricsMiddleware.
        let any_ref: &dyn Any = mw.as_any();
        if let Some(metrics) = any_ref.downcast_ref::<MetricsMiddleware>() {
            published_counts = metrics.get_all_published_counts().await;
            handler_counts = metrics.get_all_handler_counts().await;
            error_counts = metrics.get_all_error_counts().await;
        }
    }

    EventBusMetrics {
        published_counts,
        handler_counts,
        error_counts,
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics", web::get().to(get_metrics));
}
