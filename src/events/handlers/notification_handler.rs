use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::events::types::{EventHandler, EventResult, StoredEvent};
use crate::utils::time;

const MAX_NOTIFICATIONS: usize = 5_000;

pub struct NotificationEventHandler {
    handler_id: String,
    notifications: Arc<RwLock<VecDeque<Notification>>>,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub notification_id: String,
    pub event_type: String,
    pub aggregate_id: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub sent: bool,
}

impl NotificationEventHandler {
    pub fn new(handler_id: impl Into<String>) -> Self {
        Self {
            handler_id: handler_id.into(),
            notifications: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    pub async fn get_notifications(&self) -> Vec<Notification> {
        self.notifications.read().await.iter().cloned().collect()
    }

    pub async fn get_unsent_notifications(&self) -> Vec<Notification> {
        self.notifications
            .read()
            .await
            .iter()
            .filter(|n| !n.sent)
            .cloned()
            .collect()
    }

    pub async fn mark_sent(&self, notification_id: &str) {
        let mut notifications = self.notifications.write().await;
        if let Some(notif) = notifications
            .iter_mut()
            .find(|n| n.notification_id == notification_id)
        {
            notif.sent = true;
        }
    }

    pub async fn clear_notifications(&self) {
        self.notifications.write().await.clear();
    }

    fn create_message(event: &StoredEvent) -> String {
        match event.metadata.event_type.as_str() {
            "NodeAdded" => format!("Node {} was added", event.metadata.aggregate_id),
            "NodeUpdated" => format!("Node {} was updated", event.metadata.aggregate_id),
            "NodeRemoved" => format!("Node {} was removed", event.metadata.aggregate_id),
            "EdgeAdded" => format!("Edge {} was added", event.metadata.aggregate_id),
            "EdgeRemoved" => format!("Edge {} was removed", event.metadata.aggregate_id),
            "GraphSaved" => format!("Graph {} was saved", event.metadata.aggregate_id),
            "GraphCleared" => "Graph was cleared".to_string(),
            "ClassAdded" => format!("Ontology class {} was added", event.metadata.aggregate_id),
            "PropertyAdded" => format!(
                "Ontology property {} was added",
                event.metadata.aggregate_id
            ),
            "OntologyImported" => format!("Ontology {} was imported", event.metadata.aggregate_id),
            "InferenceCompleted" => format!(
                "Inference completed for ontology {}",
                event.metadata.aggregate_id
            ),
            "SimulationStarted" => {
                format!("Physics simulation {} started", event.metadata.aggregate_id)
            }
            "SimulationStopped" => {
                format!("Physics simulation {} stopped", event.metadata.aggregate_id)
            }
            "LayoutOptimized" => format!("Layout {} was optimized", event.metadata.aggregate_id),
            "SettingUpdated" => format!("Setting {} was updated", event.metadata.aggregate_id),
            _ => format!("Event {} occurred", event.metadata.event_type),
        }
    }
}

#[async_trait]
impl EventHandler for NotificationEventHandler {
    fn event_type(&self) -> &'static str {
        "*"
    }

    fn handler_id(&self) -> &str {
        &self.handler_id
    }

    async fn handle(&self, event: &StoredEvent) -> EventResult<()> {
        let notification = Notification {
            notification_id: uuid::Uuid::new_v4().to_string(),
            event_type: event.metadata.event_type.clone(),
            aggregate_id: event.metadata.aggregate_id.clone(),
            message: Self::create_message(event),
            timestamp: time::now(),
            sent: false,
        };

        let mut notifications = self.notifications.write().await;
        notifications.push_back(notification);
        while notifications.len() > MAX_NOTIFICATIONS {
            notifications.pop_front();
        }

        Ok(())
    }

    fn max_retries(&self) -> u32 {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventMetadata;

    #[tokio::test]
    async fn test_notification_creation() {
        let handler = NotificationEventHandler::new("notification-handler");

        let event = StoredEvent {
            metadata: EventMetadata::new(
                "node-1".to_string(),
                "Node".to_string(),
                "NodeAdded".to_string(),
            ),
            data: "{}".to_string(),
            sequence: 1,
        };

        handler.handle(&event).await.unwrap();

        let notifications = handler.get_notifications().await;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].event_type, "NodeAdded");
        assert!(notifications[0].message.contains("node-1"));
    }

    #[tokio::test]
    async fn test_unsent_notifications() {
        let handler = NotificationEventHandler::new("notification-handler");

        let event = StoredEvent {
            metadata: EventMetadata::new(
                "node-1".to_string(),
                "Node".to_string(),
                "NodeAdded".to_string(),
            ),
            data: "{}".to_string(),
            sequence: 1,
        };

        handler.handle(&event).await.unwrap();

        let unsent = handler.get_unsent_notifications().await;
        assert_eq!(unsent.len(), 1);
        assert!(!unsent[0].sent);

        let notification_id = unsent[0].notification_id.clone();
        handler.mark_sent(&notification_id).await;

        let unsent_after = handler.get_unsent_notifications().await;
        assert_eq!(unsent_after.len(), 0);
    }

    #[tokio::test]
    async fn test_message_formatting() {
        let handler = NotificationEventHandler::new("notification-handler");

        let test_cases = vec![
            ("NodeAdded", "node-1", "Node node-1 was added"),
            ("GraphSaved", "graph-1", "Graph graph-1 was saved"),
            (
                "InferenceCompleted",
                "onto-1",
                "Inference completed for ontology onto-1",
            ),
        ];

        for (event_type, aggregate_id, expected_msg) in test_cases {
            let event = StoredEvent {
                metadata: EventMetadata::new(
                    aggregate_id.to_string(),
                    "Test".to_string(),
                    event_type.to_string(),
                ),
                data: "{}".to_string(),
                sequence: 1,
            };

            handler.clear_notifications().await;
            handler.handle(&event).await.unwrap();

            let notifications = handler.get_notifications().await;
            assert_eq!(notifications[0].message, expected_msg);
        }
    }
}
