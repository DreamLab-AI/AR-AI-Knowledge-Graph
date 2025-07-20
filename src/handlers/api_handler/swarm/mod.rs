use actix_web::web;
use crate::handlers::swarm_handler::{
    update_swarm_data as swarm_update,
    get_swarm_data as swarm_get,
};

// Re-export the handlers
pub use crate::handlers::swarm_handler::{
    update_swarm_data,
    get_swarm_data,
    get_swarm_positions,
};

// Configure swarm API routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/swarm")
            .route("/data", web::get().to(swarm_get))
            .route("/data", web::post().to(swarm_update))
            .route("/update", web::post().to(swarm_update))
    );
}