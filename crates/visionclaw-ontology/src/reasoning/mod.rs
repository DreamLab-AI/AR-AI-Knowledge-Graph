/// Reasoning module for VisionClaw ontology inference
/// This module provides custom OWL reasoning capabilities using the Whelk reasoner.
/// The reasoning system validates ontologies, infers new axioms, and detects contradictions.
pub mod custom_reasoner;

// Re-export main types
pub use custom_reasoner::{CustomReasoner, InferredAxiom, OntologyReasoner};

/// Type alias for reasoning operations
pub type ReasoningResult<T> = Result<T, ReasoningError>;

/// Error types for reasoning operations
#[derive(Debug, thiserror::Error)]
pub enum ReasoningError {
    #[error("Ontology parsing error: {0}")]
    Parsing(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Actor error: {0}")]
    Actor(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
