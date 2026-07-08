//! Transport-agnostic XR presence library for VisionClaw.
//!
//! This crate is the single source of truth for the avatar pose wire format
//! (opcode 0x43), the `PresenceRoom` aggregate invariants, and pose validators.
//! It is consumed by both the Rust server (`src/handlers/presence_handler.rs`,
//! `src/actors/presence_actor.rs`) and the Godot client gdext crate
//! (`xr-client/rust/`).
//!
//! No transport (Actix, tokio-tungstenite, godot signal) is assumed — see
//! [`ports`] for the trait abstractions injected by each consumer.

pub mod agent_presence;
pub mod delta;
pub mod error;
pub mod ports;
pub mod room;
pub mod types;
pub mod validate;
pub mod wire;

pub use agent_presence::{
    decode_agent_presence, dequantise_dir, encode_agent_presence, quantise_dir, AgentActivity,
    AgentPresence, AgentPresenceBatch, AgentPresenceDelta, AttentionTarget, PresenceChannel,
    OPCODE_AGENT_PRESENCE,
};
pub use delta::{PoseDelta, TransformMask};
pub use error::{RoomError, ValidationError, WireError};
pub use ports::{Broadcaster, IdentityVerifier, SignedChallenge};
pub use room::{AvatarState, PresenceRoom};
pub use types::{Aabb, AvatarId, AvatarMetadata, Did, HandPose, PoseFrame, RoomId, Transform};
pub use validate::{joint_anatomy, monotonic_timestamp, velocity_gate, world_bounds};
pub use wire::{decode, encode, OPCODE_AVATAR_POSE};
