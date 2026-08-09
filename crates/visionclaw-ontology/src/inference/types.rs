// src/inference/types.rs
//! Inference Result Types
//!
//! Domain types for representing inference results, explanations, and validation outcomes.

use crate::utils::time;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use visionclaw_domain::ports::ontology_repository::OwlAxiom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceType {
    ClassAssertion,

    SubClassOf,

    EquivalentClass,

    DisjointClasses,

    PropertyAssertion,

    PropertyDomain,

    PropertyRange,

    InverseProperty,

    TransitiveProperty,
}

impl std::fmt::Display for InferenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClassAssertion => write!(f, "Class Assertion"),
            Self::SubClassOf => write!(f, "SubClass Of"),
            Self::EquivalentClass => write!(f, "Equivalent Class"),
            Self::DisjointClasses => write!(f, "Disjoint Classes"),
            Self::PropertyAssertion => write!(f, "Property Assertion"),
            Self::PropertyDomain => write!(f, "Property Domain"),
            Self::PropertyRange => write!(f, "Property Range"),
            Self::InverseProperty => write!(f, "Inverse Property"),
            Self::TransitiveProperty => write!(f, "Transitive Property"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inference {
    pub id: Option<String>,

    pub inference_type: InferenceType,

    pub subject: String,

    pub predicate: String,

    pub object: String,

    pub confidence: f32,

    pub explanation: Vec<OwlAxiom>,

    pub metadata: HashMap<String, String>,

    pub computed_at: DateTime<Utc>,
}

impl Inference {
    pub fn new(
        inference_type: InferenceType,
        subject: String,
        predicate: String,
        object: String,
    ) -> Self {
        Self {
            id: None,
            inference_type,
            subject,
            predicate,
            object,
            confidence: 1.0,
            explanation: Vec::new(),
            metadata: HashMap::new(),
            computed_at: time::now(),
        }
    }

    pub fn with_explanation(mut self, axioms: Vec<OwlAxiom>) -> Self {
        self.explanation = axioms;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceExplanation {
    pub axiom: OwlAxiom,

    pub axiom_chain: Vec<OwlAxiom>,

    pub description: String,

    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub consistent: bool,

    pub unsatisfiable: Vec<UnsatisfiableClass>,

    pub warnings: Vec<String>,

    pub errors: Vec<String>,

    pub validation_time_ms: u64,
}

impl ValidationResult {
    pub fn consistent() -> Self {
        Self {
            consistent: true,
            unsatisfiable: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            validation_time_ms: 0,
        }
    }

    pub fn inconsistent(unsatisfiable: Vec<UnsatisfiableClass>) -> Self {
        Self {
            consistent: false,
            unsatisfiable,
            warnings: Vec::new(),
            errors: Vec::new(),
            validation_time_ms: 0,
        }
    }

    pub fn has_issues(&self) -> bool {
        !self.warnings.is_empty() || !self.errors.is_empty() || !self.consistent
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsatisfiableClass {
    pub class_iri: String,

    pub reason: String,

    pub conflicting_axioms: Vec<OwlAxiom>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub hierarchy: Vec<(String, String)>,

    pub equivalent_classes: Vec<Vec<String>>,

    pub classification_time_ms: u64,

    pub inferred_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyReport {
    pub is_consistent: bool,

    pub unsatisfiable_classes: Vec<UnsatisfiableClass>,

    pub classes_checked: usize,

    pub axioms_checked: usize,

    pub check_time_ms: u64,

    pub reasoner_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_creation() {
        let inf = Inference::new(
            InferenceType::SubClassOf,
            "ex:Dog".to_string(),
            "rdfs:subClassOf".to_string(),
            "ex:Animal".to_string(),
        );

        assert_eq!(inf.inference_type, InferenceType::SubClassOf);
        assert_eq!(inf.confidence, 1.0);
        assert!(inf.explanation.is_empty());
    }

    #[test]
    fn test_inference_builder() {
        let inf = Inference::new(
            InferenceType::ClassAssertion,
            "ex:fido".to_string(),
            "rdf:type".to_string(),
            "ex:Dog".to_string(),
        )
        .with_confidence(0.95)
        .with_metadata("source".to_string(), "ml_classifier".to_string());

        assert_eq!(inf.confidence, 0.95);
        assert_eq!(
            inf.metadata
                .get("source")
                .expect("Missing required key: source"),
            "ml_classifier"
        );
    }

    #[test]
    fn test_validation_result_consistent() {
        let result = ValidationResult::consistent();
        assert!(result.consistent);
        assert!(!result.has_issues());
    }

    #[test]
    fn test_validation_result_inconsistent() {
        let unsat = UnsatisfiableClass {
            class_iri: "ex:Square  Circle".to_string(),
            reason: "Conflicting axioms".to_string(),
            conflicting_axioms: Vec::new(),
        };

        let result = ValidationResult::inconsistent(vec![unsat]);
        assert!(!result.consistent);
        assert!(result.has_issues());
        assert_eq!(result.unsatisfiable.len(), 1);
    }
}
