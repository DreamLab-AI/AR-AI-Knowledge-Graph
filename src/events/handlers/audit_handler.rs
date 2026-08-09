use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::events::types::{EventHandler, EventResult, StoredEvent};

const MAX_AUDIT_ENTRIES: usize = 10_000;

pub struct AuditEventHandler {
    handler_id: String,
    log: Arc<RwLock<VecDeque<AuditLogEntry>>>,
}

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub event_id: String,
    pub event_type: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<String>,
    pub data_summary: String,
}

impl AuditEventHandler {
    pub fn new(handler_id: impl Into<String>) -> Self {
        Self {
            handler_id: handler_id.into(),
            log: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    pub async fn get_log_entries(&self) -> Vec<AuditLogEntry> {
        self.log.read().await.iter().cloned().collect()
    }

    pub async fn get_log_count(&self) -> usize {
        self.log.read().await.len()
    }

    pub async fn get_entries_for_aggregate(&self, aggregate_id: &str) -> Vec<AuditLogEntry> {
        self.log
            .read()
            .await
            .iter()
            .filter(|e| e.aggregate_id == aggregate_id)
            .cloned()
            .collect()
    }

    pub async fn get_entries_by_type(&self, event_type: &str) -> Vec<AuditLogEntry> {
        self.log
            .read()
            .await
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    pub async fn clear_log(&self) {
        self.log.write().await.clear();
    }

    fn summarize_data(data: &str) -> String {
        const MAX_LEN: usize = 100;
        if data.len() <= MAX_LEN {
            data.to_string()
        } else {
            format!("{}... ({} bytes)", &data[..MAX_LEN], data.len())
        }
    }
}

#[async_trait]
impl EventHandler for AuditEventHandler {
    fn event_type(&self) -> &'static str {
        "*"
    }

    fn handler_id(&self) -> &str {
        &self.handler_id
    }

    async fn handle(&self, event: &StoredEvent) -> EventResult<()> {
        let entry = AuditLogEntry {
            event_id: event.metadata.event_id.clone(),
            event_type: event.metadata.event_type.clone(),
            aggregate_id: event.metadata.aggregate_id.clone(),
            aggregate_type: event.metadata.aggregate_type.clone(),
            timestamp: event.metadata.timestamp,
            user_id: event.metadata.user_id.clone(),
            data_summary: Self::summarize_data(&event.data),
        };

        let mut log = self.log.write().await;
        log.push_back(entry);
        while log.len() > MAX_AUDIT_ENTRIES {
            log.pop_front();
        }

        Ok(())
    }

    fn max_retries(&self) -> u32 {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventMetadata;

    #[tokio::test]
    async fn test_audit_logging() {
        let handler = AuditEventHandler::new("audit-handler");

        let event = StoredEvent {
            metadata: EventMetadata::new(
                "node-1".to_string(),
                "Node".to_string(),
                "NodeAdded".to_string(),
            ),
            data: r#"{"node_id":"node-1","label":"Test"}"#.to_string(),
            sequence: 1,
        };

        handler.handle(&event).await.unwrap();
        assert_eq!(handler.get_log_count().await, 1);

        let entries = handler.get_log_entries().await;
        assert_eq!(entries[0].event_type, "NodeAdded");
        assert_eq!(entries[0].aggregate_id, "node-1");
    }

    #[tokio::test]
    async fn test_get_entries_for_aggregate() {
        let handler = AuditEventHandler::new("audit-handler");

        for i in 0..5 {
            let event = StoredEvent {
                metadata: EventMetadata::new(
                    format!("node-{}", i),
                    "Node".to_string(),
                    "NodeAdded".to_string(),
                ),
                data: "{}".to_string(),
                sequence: i as i64,
            };
            handler.handle(&event).await.unwrap();
        }

        let entries = handler.get_entries_for_aggregate("node-1").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].aggregate_id, "node-1");
    }

    #[tokio::test]
    async fn test_data_summarization() {
        let handler = AuditEventHandler::new("audit-handler");

        let long_data = "x".repeat(200);
        let event = StoredEvent {
            metadata: EventMetadata::new(
                "node-1".to_string(),
                "Node".to_string(),
                "NodeAdded".to_string(),
            ),
            data: long_data,
            sequence: 1,
        };

        handler.handle(&event).await.unwrap();
        let entries = handler.get_log_entries().await;
        assert!(entries[0].data_summary.contains("... (200 bytes)"));
    }
}
