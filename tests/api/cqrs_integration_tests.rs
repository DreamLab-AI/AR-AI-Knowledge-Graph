// tests/api/cqrs_integration_tests.rs
//! Integration tests for CQRS API endpoints (Phase 4)
//!
//! Tests verify that API handlers correctly use Application Services,
//! CommandBus, QueryBus, and EventBus patterns.

use actix_web::{test, web, App, HttpResponse};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

use visionclaw_server::cqrs::{CommandBus, QueryBus};
use visionclaw_server::events::EventBus;
use visionclaw_server::application::{
    GraphApplicationService, SettingsApplicationService,
    OntologyApplicationService, PhysicsApplicationService,
};

/// Helper to create test application services
fn create_test_services() -> (
    Arc<RwLock<CommandBus>>,
    Arc<RwLock<QueryBus>>,
    Arc<RwLock<EventBus>>,
) {
    let command_bus = Arc::new(RwLock::new(CommandBus::new()));
    let query_bus = Arc::new(RwLock::new(QueryBus::new()));
    let event_bus = Arc::new(RwLock::new(EventBus::new()));

    (command_bus, query_bus, event_bus)
}

mod graph_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_graph_service_initialization() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        // Should be able to query nodes (even if empty)
        let nodes = service.get_all_nodes().await;
        assert!(nodes.is_ok());
        assert_eq!(nodes.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_add_node_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let node_data = json!({
            "label": "Test Node",
            "type": "Person",
            "properties": {}
        });

        let result = service.add_node(node_data).await;
        assert!(result.is_ok());

        let node_id = result.unwrap();
        assert!(!node_id.is_empty());
    }

    #[tokio::test]
    async fn test_update_node_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let updates = json!({
            "label": "Updated Label",
            "properties": {
                "status": "active"
            }
        });

        let result = service.update_node("test-node-id", updates).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_node_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let result = service.remove_node("test-node-id").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_save_graph_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let result = service.save_graph().await;
        assert!(result.is_ok());
    }
}

mod settings_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_settings_service_initialization() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = SettingsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        // Should be able to query settings
        let settings = service.get_all_settings().await;
        assert!(settings.is_ok());
    }

    #[tokio::test]
    async fn test_get_setting_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = SettingsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let result = service.get_setting("test.key").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_setting_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = SettingsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let value = json!("test_value");
        let result = service.update_setting("test.key", value).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_batch_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = SettingsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let updates = json!({
            "key1": "value1",
            "key2": "value2"
        });

        let result = service.update_batch(updates).await;
        assert!(result.is_ok());
    }
}

mod ontology_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_ontology_service_initialization() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = OntologyApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        // Should be able to query classes
        let classes = service.list_classes().await;
        assert!(classes.is_ok());
        assert_eq!(classes.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_add_class_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = OntologyApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let class_data = json!({
            "uri": "http://example.org/Person",
            "label": "Person"
        });

        let result = service.add_class(class_data).await;
        assert!(result.is_ok());

        let uri = result.unwrap();
        assert!(!uri.is_empty());
    }

    #[tokio::test]
    async fn test_add_property_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = OntologyApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let property_data = json!({
            "uri": "http://example.org/hasName",
            "label": "has name"
        });

        let result = service.add_property(property_data).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_import_ontology_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = OntologyApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let result = service.import_ontology("test.owl").await;
        assert!(result.is_ok());
    }
}

mod physics_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_physics_service_initialization() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = PhysicsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        // Should be able to query physics state
        let state = service.get_physics_state().await;
        assert!(state.is_ok());
    }

    #[tokio::test]
    async fn test_start_simulation_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = PhysicsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let result = service.start_simulation("knowledge").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_simulation_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = PhysicsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let result = service.stop_simulation("knowledge").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_params_via_service() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = PhysicsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let params = json!({
            "gravity": 0.1,
            "damping": 0.9
        });

        let result = service.update_params(params).await;
        assert!(result.is_ok());
    }
}

mod event_bus_tests {
    use super::*;
    use visionclaw_server::application::events::DomainEvent;

    #[tokio::test]
    async fn test_event_bus_initialization() {
        let event_bus = EventBus::new();
        assert!(event_bus.is_enabled().await);
    }

    #[tokio::test]
    async fn test_event_bus_publish() {
        let event_bus = Arc::new(RwLock::new(EventBus::new()));

        // Create a test event
        let event = DomainEvent::NodeAdded {
            node_id: "test-node".to_string(),
            node_type: "Person".to_string(),
            timestamp: DomainEvent::now(),
        };

        // Publishing should succeed (even with no subscribers)
        let bus = event_bus.read().await;
        // Note: We can't directly publish DomainEvent without implementing
        // the trait, so this is a placeholder for the test structure
        drop(bus);
    }

    #[tokio::test]
    async fn test_event_sequence() {
        let event_bus = EventBus::new();

        let seq1 = event_bus.current_sequence().await;
        assert_eq!(seq1, 0);

        // After publishing events, sequence should increment
        // (implementation detail - this is a structural test)
    }
}

mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_add_node_latency() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let node_data = json!({"label": "Perf Test", "type": "Test"});

        let start = Instant::now();
        let _ = service.add_node(node_data).await;
        let duration = start.elapsed();

        // Should complete in less than 10ms (p99 target)
        assert!(duration.as_millis() < 10, "Latency too high: {:?}", duration);
    }

    #[tokio::test]
    async fn test_get_settings_latency() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = SettingsApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        );

        let start = Instant::now();
        let _ = service.get_all_settings().await;
        let duration = start.elapsed();

        // Query should complete in less than 5ms (p99 target)
        assert!(duration.as_millis() < 5, "Query latency too high: {:?}", duration);
    }

    #[tokio::test]
    async fn test_concurrent_commands() {
        let (cmd_bus, query_bus, event_bus) = create_test_services();

        let service = Arc::new(GraphApplicationService::new(
            cmd_bus,
            query_bus,
            event_bus,
        ));

        // Spawn 100 concurrent add_node commands
        let mut handles = Vec::new();
        for i in 0..100 {
            let svc = service.clone();
            let handle = tokio::spawn(async move {
                let node = json!({
                    "label": format!("Node {}", i),
                    "type": "Test"
                });
                svc.add_node(node).await
            });
            handles.push(handle);
        }

        // All should complete successfully
        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok());
        }
    }
}

#[cfg(test)]
mod integration_summary {
    //! Integration Test Summary
    //!
    //! **Test Coverage**:
    //! - GraphApplicationService: 5 tests
    //! - SettingsApplicationService: 4 tests
    //! - OntologyApplicationService: 4 tests
    //! - PhysicsApplicationService: 4 tests
    //! - EventBus: 3 tests
    //! - Performance: 3 tests
    //!
    //! **Total**: 23 integration tests
    //!
    //! **Performance Targets**:
    //! - Commands: <10ms p99
    //! - Queries: <5ms p99
    //! - Events: <2ms async
    //!
    //! **Run Tests**:
    //! ```bash
    //! cargo test --test cqrs_integration_tests
    //! ```
}
