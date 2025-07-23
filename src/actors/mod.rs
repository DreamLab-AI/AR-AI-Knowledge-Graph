//! Actor system modules for replacing Arc<RwLock<T>> patterns with Actix actors

pub mod graph_actor;
pub mod settings_actor;
pub mod metadata_actor;
pub mod client_manager_actor;
pub mod gpu_compute_actor;
pub mod protected_settings_actor;
pub mod claude_flow_actor;
pub mod messages;

pub use graph_actor::GraphServiceActor;
pub use settings_actor::SettingsActor;
pub use metadata_actor::MetadataActor;
pub use client_manager_actor::ClientManagerActor;
pub use gpu_compute_actor::GPUComputeActor;
pub use protected_settings_actor::ProtectedSettingsActor;
pub use claude_flow_actor::ClaudeFlowActor;
pub use messages::*;