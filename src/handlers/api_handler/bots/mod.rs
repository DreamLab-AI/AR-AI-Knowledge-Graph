use crate::handlers::bots_handler::{
    get_bots_agents, get_bots_connection_status, get_bots_data as bots_get, get_task_status,
    initialize_hive_mind_swarm as initialize_swarm, interrupt_task, process_settings_command,
    remove_task, spawn_agent_hybrid, submit_task, update_bots_graph as bots_update,
};
use actix_web::web;

// Configure bots API routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/bots")
            .route("/data", web::get().to(bots_get))
            .route("/data", web::post().to(bots_update))
            .route("/update", web::post().to(bots_update))
            .route("/initialize-swarm", web::post().to(initialize_swarm))
            .route("/settings-command", web::post().to(process_settings_command))
            .route("/status", web::get().to(get_bots_connection_status))
            .route("/agents", web::get().to(get_bots_agents))
            .route("/spawn-agent-hybrid", web::post().to(spawn_agent_hybrid))
            // D2 steering surface (PRD-023 WP-3): per-agent submit-task /
            // interrupt, plus the task-status poll the panel drives.
            .route("/submit-task", web::post().to(submit_task))
            .route("/interrupt", web::post().to(interrupt_task))
            .route("/task-status/{id}", web::get().to(get_task_status))
            .route("/remove-task/{id}", web::delete().to(remove_task)),
    );
}
