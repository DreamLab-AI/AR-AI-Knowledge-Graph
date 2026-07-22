//! Wire-protocol encode/decode for VisionClaw.
//!
//! ADR-090 Phase A5: extracted from the webxr monolith.
//!
//! # Modules
//!
//! - [`protocols`] — Binary settings protocol with delta encoding + zlib compression.
//! - [`socket_flow_messages`] — Wire message types (`BinaryNodeDataClient`, `Message`,
//!   `Ping`/`Pong`, initial graph payloads) shared with the webxr crate.
//!
//! # Single 52-byte node-data encoder (ADR-031 D2 / task #101 T6)
//!
//! The canonical per-node analytics wire encoder is `src/utils/binary_protocol.rs` in
//! the webxr crate (`visionclaw-server`): one 52-byte `WireNodeDataItemV3` frame with
//! `centrality@48`, stride 52. Every live broadcast site calls that module.
//!
//! This crate previously carried a parallel `binary_protocol` module — a stale 48-byte
//! copy (no `centrality`, `WIRE_V3_ITEM_SIZE == 48`) extracted during ADR-090 Phase A5
//! but never advanced to the 52-byte format. It had zero callers outside this crate
//! (verified by grep across `src/` and `crates/`) and diverged from the wire contract,
//! so it was removed to leave a single source of truth. The webxr encoder depends on
//! `socket_flow_messages::BinaryNodeDataClient` (a webxr-local type), which is why it
//! stays in the webxr crate rather than moving here.

pub mod protocols;
pub mod socket_flow_messages;

// Convenience re-exports.
pub use protocols::binary_settings_protocol::{
    BinaryMessage, BinarySettingsProtocol, BinaryValue, PathRegistry,
};
