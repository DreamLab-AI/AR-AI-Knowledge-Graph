//! Domain port traits — ADR-090 Phase 1b (shipped 2026-05-28).
//!
//! Ports extracted here (all shipped):
//! - `InferenceEngine` — Phase 2
//! - `OntologyRepository`, `owl_types` — Phase 2
//! - `GpuPhysicsAdapter`, `GpuSemanticAnalyzer` — Phase 1b (unblocked once
//!   GraphData became a canonical domain type)
//!
//! Ports that remain in `src/ports/` of the webxr crate:
//! - `GraphRepository` — signature depends on `PhysicsState` and
//!   `AutoBalanceNotification`, which are webxr-internal types not yet promoted.
//! - `SettingsRepository` — signature depends on `AppFullSettings`, which
//!   lives in `webxr/src/config/` and has not yet been promoted to domain.
//!
//! These will move when their dependencies are extracted in a future phase.

pub mod gpu_physics_adapter;
pub mod gpu_semantic_analyzer;
pub mod inference_engine;
pub mod ontology_repository;
pub mod owl_types;
pub mod settings_repository;

// Convenience re-exports
pub use gpu_physics_adapter::{
    GpuDeviceInfo, GpuPhysicsAdapter, GpuPhysicsAdapterError, NodeForce, PhysicsParameters,
    PhysicsStatistics, PhysicsStepResult,
};
pub use gpu_semantic_analyzer::{
    ClusteringAlgorithm, CommunityDetectionResult, GpuSemanticAnalyzer, GpuSemanticAnalyzerError,
    ImportanceAlgorithm, OptimizationResult, PathfindingResult, SemanticConstraintConfig,
    SemanticStatistics,
};
pub use inference_engine::{InferenceEngine, InferenceEngineError, InferenceStatistics};
pub use ontology_repository::{
    OntologyMetrics, OntologyRepository, OntologyRepositoryError, PathfindingCacheEntry,
    ValidationReport,
};
pub use owl_types::{
    AxiomType, InferenceResults, OwlAxiom, OwlClass, OwlCrossReference, OwlProperty,
    OwlRelationship, PropertyType,
};
pub use settings_repository::{
    SettingValue, SettingsRepository, SettingsRepositoryError,
};
