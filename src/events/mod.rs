pub mod bus;
pub mod domain_events;
pub mod handlers;
pub mod middleware;
pub mod store;
pub mod types;

// Phase 7: Inference triggers
pub mod inference_triggers;

pub use types::{
    DomainEvent, EventError, EventHandler, EventMetadata, EventMiddleware, EventResult,
    EventSnapshot, StoredEvent,
};

pub use domain_events::*;

pub use bus::{DeadLetterEntry, DeadLetterQueue, EventBus};

// Re-export EventBus in event_bus module for backward compatibility
pub mod event_bus {
    pub use super::bus::EventBus;
}

pub use store::{EventRepository, EventStore, FileEventRepository, InMemoryEventRepository};

pub use middleware::{
    EnrichmentMiddleware, LoggingMiddleware, MetricsMiddleware, RetryMiddleware,
    ValidationMiddleware,
};

pub use handlers::{
    AuditEventHandler, GraphEventHandler, NotificationEventHandler, OntologyEventHandler,
};

pub use inference_triggers::{
    register_inference_triggers, AutoInferenceConfig, InferenceTriggerHandler, OntologyEvent,
};
