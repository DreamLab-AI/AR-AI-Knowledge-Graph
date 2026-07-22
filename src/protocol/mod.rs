//! Wire-level protocol modules.
//!
//! The canonical broadcast frame is the 52-byte `WireNodeDataItemV3` encoder in
//! [`crate::utils::binary_protocol`], used by the `/wss` WebSocket broadcast and
//! the `GET /api/graph/positions` REST endpoint. This module holds no submodules.
