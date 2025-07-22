use actix_web::web;
use crate::handlers::bots_handler::{
    update_bots_data as bots_update,
    get_bots_data as bots_get,
};

// Re-export the handlers
pub use crate::handlers::bots_handler::{
    update_bots_data,
    get_bots_data,
    get_bots_positions,
};

// Configure bots API routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/bots")
            .route("/data", web::get().to(bots_get))
            .route("/data", web::post().to(bots_update))
            .route("/update", web::post().to(bots_update))
    );
}