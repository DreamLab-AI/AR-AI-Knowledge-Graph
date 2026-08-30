//! Layout types — re-exported from `visionclaw-domain` per ADR-090.
//!
//! Originally defined here; moved to the domain crate so changes to the
//! `LayoutMode` enum (etc.) only recompile the domain crate plus the linker.

pub use visionclaw_domain::types::layout::{
    ConstraintZone, LayoutMode, LayoutModeConfig, LayoutStatus, RadialMode,
};
