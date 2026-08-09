// src/inference/mod.rs
//! Inference Module
//!
//! Provides OWL 2 DL ontology reasoning and inference capabilities using whelk-rs.
//! This module includes OWL parsers, inference types, caching, and optimization.

pub mod cache;
pub mod optimization;
pub mod owl_parser;
pub mod types;

pub use cache::{CacheConfig, CacheEntry, CacheStatistics, InferenceCache};
pub use optimization::{
    BatchInferenceRequest, IncrementalInference, InferenceOptimizer, OptimizationMetrics,
    ParallelClassification,
};
pub use owl_parser::{OWLFormat, OWLParser, ParseError, ParseResult};
pub use types::{
    ClassificationResult, ConsistencyReport, Inference, InferenceExplanation, InferenceType,
    UnsatisfiableClass, ValidationResult,
};
