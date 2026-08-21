//! Re-export shim (ADR-031 D7).
//!
//! The analytics fixtures + CPU reference implementations moved out of this
//! CUDA-linking root crate into the dependency-free `visionclaw-analytics-oracle`
//! crate so the ubuntu `CPU_CRATES` CI job can actually run the correctness
//! known-answer gate (it previously could not — the root crate links libcuda and
//! is excluded from that job, so the suite gated nothing).
//!
//! This file remains only as a thin re-export so the root crate's GPU-gated and
//! wire-contract tests keep pulling the fixtures in via the established
//! `#[path = "analytics_fixtures.rs"] mod fx;` idiom without a churn of imports:
//!
//! ```ignore
//! #[path = "analytics_fixtures.rs"]
//! mod fx;
//! use fx::*;
//! ```
//!
//! The single source of truth is now `crates/visionclaw-analytics-oracle/src/lib.rs`.

pub use visionclaw_analytics_oracle::*;
