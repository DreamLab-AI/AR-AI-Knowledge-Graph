pub mod files;
pub mod graph;
pub mod visualisation;
pub mod swarm;

// Re-export specific types and functions
// Re-export specific types and functions
pub use files::{
    fetch_and_process_files,
    get_file_content,
};

pub use graph::{
    get_graph_data,
    get_paginated_graph_data,
    refresh_graph,
    update_graph,
};

pub use visualisation::get_visualisation_settings;

use actix_web::web;

// Configure all API routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("") // Removed redundant /api prefix
            .configure(files::config)
            .configure(graph::config)
            .configure(visualisation::config)
            .configure(swarm::config)
            .configure(crate::handlers::nostr_handler::config)
            .configure(crate::handlers::settings_handler::config)
            .configure(crate::handlers::ragflow_handler::config) // Add this line
    );
}
