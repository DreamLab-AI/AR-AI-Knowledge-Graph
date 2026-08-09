// src/ports/mod.rs
//! Hexagonal Architecture Ports
//!
//! This module defines the port interfaces (traits) that represent
//! the core application boundaries. These are technology-agnostic
//! interfaces that the domain logic depends on.

// Legacy ports (to be refactored)
pub mod graph_repository;
pub mod physics_simulator;
pub mod semantic_analyzer;

// Webxr-local ports
pub mod knowledge_graph_repository;
pub mod settings_repository;

// Legacy exports
pub use graph_repository::GraphRepository;
pub use physics_simulator::PhysicsSimulator;
pub use semantic_analyzer::SemanticAnalyzer;

// New hexser-based exports (canonical paths in visionclaw-domain)
pub use knowledge_graph_repository::KnowledgeGraphRepository;
pub use settings_repository::SettingsRepository;
pub use visionclaw_domain::ports::inference_engine::InferenceEngine;
pub use visionclaw_domain::ports::ontology_repository::OntologyRepository;

// Module-path re-exports so existing callers writing
// `use crate::ports::gpu_physics_adapter::Foo` keep resolving without
// rewriting to the visionclaw_domain path. The TYPES themselves are
// canonical in visionclaw-domain; this just preserves the legacy module
// alias as a compatibility surface inside webxr.
pub use visionclaw_domain::ports::gpu_physics_adapter;
pub use visionclaw_domain::ports::gpu_semantic_analyzer;
pub use visionclaw_domain::ports::inference_engine;
pub use visionclaw_domain::ports::ontology_repository;

// GPU port trait exports (canonical paths in visionclaw-domain)
pub use visionclaw_domain::ports::gpu_physics_adapter::{
    GpuDeviceInfo, GpuPhysicsAdapter, GpuPhysicsAdapterError, NodeForce, PhysicsParameters,
    PhysicsStatistics, PhysicsStepResult,
};
pub use visionclaw_domain::ports::gpu_semantic_analyzer::{
    ClusteringAlgorithm, CommunityDetectionResult, GpuSemanticAnalyzer, GpuSemanticAnalyzerError,
    ImportanceAlgorithm, OptimizationResult, PathfindingResult, SemanticConstraintConfig,
    SemanticStatistics,
};
