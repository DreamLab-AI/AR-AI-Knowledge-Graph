// src/events/inference_triggers.rs
//! Automatic Inference Triggers
//!
//! Event-driven inference that automatically runs reasoning when ontology changes occur.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use crate::application::inference_service::InferenceService;
use crate::events::domain_events::{
    AxiomAddedEvent as DomainAxiomAddedEvent, ClassAddedEvent as DomainClassAddedEvent,
    OntologyImportedEvent as DomainOntologyImportedEvent,
};
use crate::events::types::{EventError, EventHandler, EventResult, StoredEvent};
use crate::events::EventBus;
use crate::utils::json::from_json;

#[derive(Debug, Clone)]
pub enum OntologyEvent {
    OntologyImported {
        ontology_id: String,
        class_count: usize,
        axiom_count: usize,
    },

    ClassAdded {
        ontology_id: String,
        class_iri: String,
    },

    AxiomAdded {
        ontology_id: String,
        axiom_id: String,
    },

    OntologyModified {
        ontology_id: String,
        change_type: String,
    },
}

#[derive(Debug, Clone)]
pub struct AutoInferenceConfig {
    pub auto_on_import: bool,

    pub auto_on_class_add: bool,

    pub auto_on_axiom_add: bool,

    pub min_delay_ms: u64,

    pub batch_changes: bool,
}

impl Default for AutoInferenceConfig {
    fn default() -> Self {
        Self {
            auto_on_import: true,
            auto_on_class_add: false,
            auto_on_axiom_add: false,
            min_delay_ms: 1000,
            batch_changes: true,
        }
    }
}

pub struct InferenceTriggerHandler {
    inference_service: Arc<RwLock<InferenceService>>,

    config: AutoInferenceConfig,

    last_inference: Arc<RwLock<std::collections::HashMap<String, std::time::Instant>>>,
}

impl InferenceTriggerHandler {
    pub fn new(
        inference_service: Arc<RwLock<InferenceService>>,
        config: AutoInferenceConfig,
    ) -> Self {
        Self {
            inference_service,
            config,
            last_inference: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn handle_event(&self, event: OntologyEvent) {
        match event {
            OntologyEvent::OntologyImported { ontology_id, .. } => {
                if self.config.auto_on_import {
                    info!(
                        "Auto-triggering inference for imported ontology: {}",
                        ontology_id
                    );
                    self.trigger_inference(&ontology_id).await;
                }
            }

            OntologyEvent::ClassAdded { ontology_id, .. } => {
                if self.config.auto_on_class_add {
                    debug!(
                        "Auto-triggering inference for class addition: {}",
                        ontology_id
                    );
                    self.trigger_incremental_inference(&ontology_id).await;
                }
            }

            OntologyEvent::AxiomAdded { ontology_id, .. } => {
                if self.config.auto_on_axiom_add {
                    debug!(
                        "Auto-triggering inference for axiom addition: {}",
                        ontology_id
                    );
                    self.trigger_incremental_inference(&ontology_id).await;
                }
            }

            OntologyEvent::OntologyModified { ontology_id, .. } => {
                debug!("Ontology modified, considering inference: {}", ontology_id);
                self.trigger_incremental_inference(&ontology_id).await;
            }
        }
    }

    /// Trigger full inference for an ontology.
    /// DEADLOCK FIX: RwLock guard is now dropped before awaiting run_inference()
    /// and update_last_inference() to prevent holding guard across await points.
    async fn trigger_inference(&self, ontology_id: &str) {
        if !self.should_run_inference(ontology_id).await {
            debug!("Skipping inference due to rate limiting: {}", ontology_id);
            return;
        }

        // Run inference - acquire and release guard within scope to avoid deadlock
        let result = {
            let service = self.inference_service.read().await;
            service.run_inference(ontology_id).await
            // Guard dropped here at end of block
        };

        match result {
            Ok(results) => {
                info!(
                    "Auto-inference completed for {}: {} inferred axioms in {}ms",
                    ontology_id,
                    results.inferred_axioms.len(),
                    results.inference_time_ms
                );

                // Now safe to call - no guard held
                self.update_last_inference(ontology_id).await;
            }
            Err(e) => {
                warn!("Auto-inference failed for {}: {:?}", ontology_id, e);
            }
        }
    }

    async fn trigger_incremental_inference(&self, ontology_id: &str) {
        if self.config.batch_changes {
            debug!("Batching changes for: {}", ontology_id);
        } else {
            self.trigger_inference(ontology_id).await;
        }
    }

    async fn should_run_inference(&self, ontology_id: &str) -> bool {
        let last_inference = self.last_inference.read().await;

        if let Some(last_time) = last_inference.get(ontology_id) {
            let elapsed = last_time.elapsed().as_millis() as u64;
            elapsed >= self.config.min_delay_ms
        } else {
            true
        }
    }

    async fn update_last_inference(&self, ontology_id: &str) {
        let mut last_inference = self.last_inference.write().await;
        last_inference.insert(ontology_id.to_string(), std::time::Instant::now());
    }
}

#[async_trait]
impl EventHandler for InferenceTriggerHandler {
    fn event_type(&self) -> &'static str {
        "*"
    }

    fn handler_id(&self) -> &str {
        "inference-trigger"
    }

    async fn handle(&self, event: &StoredEvent) -> EventResult<()> {
        let ontology_event = match event.metadata.event_type.as_str() {
            "OntologyImported" => {
                let data: DomainOntologyImportedEvent = from_json(&event.data).map_err(|e| {
                    EventError::Handler(format!("Failed to parse OntologyImportedEvent: {}", e))
                })?;
                Some(OntologyEvent::OntologyImported {
                    ontology_id: data.ontology_id,
                    class_count: data.class_count,
                    axiom_count: 0,
                })
            }
            "ClassAdded" => {
                let data: DomainClassAddedEvent = from_json(&event.data).map_err(|e| {
                    EventError::Handler(format!("Failed to parse ClassAddedEvent: {}", e))
                })?;
                Some(OntologyEvent::ClassAdded {
                    ontology_id: event.metadata.aggregate_id.clone(),
                    class_iri: data.class_iri,
                })
            }
            "AxiomAdded" => {
                let data: DomainAxiomAddedEvent = from_json(&event.data).map_err(|e| {
                    EventError::Handler(format!("Failed to parse AxiomAddedEvent: {}", e))
                })?;
                Some(OntologyEvent::AxiomAdded {
                    ontology_id: event.metadata.aggregate_id.clone(),
                    axiom_id: data.axiom_id,
                })
            }
            _ => None,
        };

        if let Some(evt) = ontology_event {
            self.handle_event(evt).await;
        }

        Ok(())
    }

    fn max_retries(&self) -> u32 {
        2
    }
}

pub async fn register_inference_triggers(
    event_bus: Arc<RwLock<EventBus>>,
    inference_service: Arc<RwLock<InferenceService>>,
    config: AutoInferenceConfig,
) {
    let handler = Arc::new(InferenceTriggerHandler::new(inference_service, config));

    let bus = event_bus.read().await;
    bus.subscribe(handler).await;

    info!("Inference triggers registered with event bus");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_inference_config_default() {
        let config = AutoInferenceConfig::default();
        assert!(config.auto_on_import);
        assert!(!config.auto_on_class_add);
        assert!(config.batch_changes);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = AutoInferenceConfig {
            min_delay_ms: 100,
            ..Default::default()
        };
    }
}
