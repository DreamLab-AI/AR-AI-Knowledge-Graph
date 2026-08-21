//! `visionclaw-gpu` — ADR-090 Phase 3 GPU/CUDA infrastructure crate.
//!
//! This crate owns:
//! - CUDA kernel source files (`cuda_sources/`)
//! - Pre-compiled PTX data (`ptx/`)
//! - PTX loader / runtime compilation utilities (`ptx_loader`)
//! - Legacy GPU buffer management helper (`memory`) — deprecated, superseded by
//!   `crate::gpu::memory_manager` in the webxr monolith; kept here for API compat.
//!
//! ## What lives here (Phase 3)
//! - CUDA `.cu` sources and pre-compiled `.ptx` binaries
//! - `ptx_loader`: runtime PTX acquisition, CUDA arch detection
//! - `memory`: `ManagedDeviceBuffer`, `MultiStreamManager`, `LabelMappingCache`
//!
//! ## What is deferred to Phase 4
//! The GPU *actor* tree (`src/actors/gpu/` in webxr) could not be extracted in
//! Phase 3 because it depends on modules still inside the webxr monolith:
//! `crate::actors::messages`, `crate::gpu::*`, `crate::telemetry::*`,
//! `crate::utils::socket_flow_messages`, `crate::utils::unified_gpu_compute`.
//! Those cross-cutting dependencies must be resolved first.
//! See the Phase 3 implementation report for details.

/// ADR-070 CUDA integration hardening — host-side (CPU) logic for the
/// stability third criterion (D2.2), input-edge NaN guard (D2.3), and the
/// sparse compute mask (D3.1 / P2). Pure and GPU-free so it is unit-testable
/// without a device.
pub mod hardening;
pub mod memory;
pub mod ptx_loader;
