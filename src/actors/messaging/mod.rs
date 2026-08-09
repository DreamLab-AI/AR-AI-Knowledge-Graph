//! Actor Message Acknowledgment Protocol
//!
//! Provides reliable message delivery for critical actor operations with:
//! - Message tracking with correlation IDs
//! - Automatic retry with exponential backoff
//! - Timeout handling
//! - Comprehensive metrics

pub mod message_ack;
pub mod message_id;
pub mod message_tracker;
pub mod metrics;

pub use message_ack::{AckStatus, MessageAck};
pub use message_id::MessageId;
pub use message_tracker::{MessageKind, MessageTracker, PendingMessage};
pub use metrics::MessageMetrics;
