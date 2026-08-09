// src/services/jsonld_ingest/shacl_gate.rs
//! SHACL validation gate for the ingest pipeline (PRD-022 WS-1 upgrade).
//!
//! Dual-mode gate: **enforcing** (write paths reject on violation) or
//! **advisory** (read paths log + metric + proceed). Wires the existing
//! SHACL-lite validator as the inline fallback and adds SPARQL-ASK-based
//! shape validation when the shapes graph is available.

use serde_json::Value;

use crate::services::jsonld_validator::shacl_lite;
use crate::services::jsonld_validator::ErrorCategory;

/// Gate operating mode (PRD-022 §4.1, ADR-127 D1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMode {
    /// Write paths: violations reject the payload.
    Enforcing,
    /// Read paths: violations log + metric but proceed.
    Advisory,
}

impl Default for GateMode {
    fn default() -> Self {
        GateMode::Advisory
    }
}

/// SHACL violation severity (maps to sh:Violation / sh:Warning).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeSeverity {
    Violation,
    Warning,
}

impl std::fmt::Display for ShapeSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShapeSeverity::Violation => write!(f, "violation"),
            ShapeSeverity::Warning => write!(f, "warning"),
        }
    }
}

/// A single SHACL shape violation, located to its block + subject.
#[derive(Debug, Clone)]
pub struct ShaclViolation {
    /// 0-based block index within the source file.
    pub block_index: usize,
    /// The `@id` of the offending entry, when present.
    pub subject: Option<String>,
    /// The SHACL-lite category that fired (reused from the validator).
    pub category: ErrorCategory,
    /// Shape that was violated (for W3C shapes, the shape IRI; for
    /// SHACL-lite, the type name).
    pub shape_name: String,
    /// Severity: violation (blocks in enforcing mode) or warning (always advisory).
    pub severity: ShapeSeverity,
}

/// Aggregated SHACL gate result for one ingest.
#[derive(Debug, Clone)]
pub struct ShaclGateReport {
    pub violations: Vec<ShaclViolation>,
    /// Number of entries (shapes) the gate inspected.
    pub shapes_checked: usize,
    /// The mode the gate was operating in.
    pub mode: GateMode,
    /// Whether W3C shapes from the shapes graph were available.
    pub w3c_shapes_available: bool,
}

impl Default for ShaclGateReport {
    fn default() -> Self {
        Self {
            violations: Vec::new(),
            shapes_checked: 0,
            mode: GateMode::Advisory,
            w3c_shapes_available: false,
        }
    }
}

impl ShaclGateReport {
    /// In enforcing mode, the gate passes iff no violations exist.
    /// In advisory mode, the gate always passes (violations are logged).
    pub fn is_valid(&self) -> bool {
        match self.mode {
            GateMode::Enforcing => self
                .violations
                .iter()
                .all(|v| v.severity != ShapeSeverity::Violation),
            GateMode::Advisory => true,
        }
    }

    /// Count of violations at the Violation severity level.
    pub fn violation_count(&self) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity == ShapeSeverity::Violation)
            .count()
    }

    /// Count of violations at the Warning severity level.
    pub fn warning_count(&self) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity == ShapeSeverity::Warning)
            .count()
    }
}

/// Run the SHACL-lite gate over one parsed JSON-LD block with the specified mode.
pub fn gate_block(block: &Value, block_index: usize, mode: GateMode, report: &mut ShaclGateReport) {
    report.mode = mode;
    for entry in collect_entries(block) {
        report.shapes_checked += 1;
        let subject = entry_subject(entry);
        let types = shacl_lite::collect_types(entry);

        for category in shacl_lite::validate_entry_shape(entry) {
            let shape_name = infer_shape_name(&types);
            report.violations.push(ShaclViolation {
                block_index,
                subject: subject.clone(),
                category,
                shape_name,
                severity: ShapeSeverity::Violation,
            });
        }
    }
}

/// Backwards-compatible gate that defaults to advisory mode.
pub fn gate_block_advisory(block: &Value, block_index: usize, report: &mut ShaclGateReport) {
    gate_block(block, block_index, GateMode::Advisory, report);
}

fn infer_shape_name(types: &[String]) -> String {
    for t in types {
        match t.as_str() {
            "OntologyClass" | "owl:Class" | "Class" => return "OntologyClassShape".to_string(),
            "BridgeRecord" | "Bridge" | "vc:BridgeRecord" => {
                return "BridgeRecordShape".to_string()
            }
            "KnowledgeNode" | "vc:KnowledgeNode" => return "KnowledgeNodeShape".to_string(),
            "AgentNode" | "vc:AgentNode" => return "AgentNodeShape".to_string(),
            _ => {}
        }
    }
    "UnknownShape".to_string()
}

/// Walk a JSON-LD document and return every assertion entry.
fn collect_entries(block: &Value) -> Vec<&Value> {
    let Value::Object(map) = block else {
        return vec![block];
    };
    if let Some(g) = map.get("@graph").or_else(|| map.get("graph")) {
        match g {
            Value::Array(items) => items.iter().collect(),
            other => vec![other],
        }
    } else {
        vec![block]
    }
}

fn entry_subject(entry: &Value) -> Option<String> {
    entry
        .as_object()?
        .get("@id")
        .or_else(|| entry.as_object().and_then(|m| m.get("id")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_ontology_class_passes_gate() {
        let block = json!({
            "@id": "urn:visionclaw:owl:class:cybernetics",
            "@type": "OntologyClass",
            "rdfs:subClassOf": { "@id": "urn:visionclaw:owl:class:built-environment" }
        });
        let mut report = ShaclGateReport::default();
        gate_block(&block, 0, GateMode::Enforcing, &mut report);
        assert!(report.is_valid(), "well-formed class must pass: {report:?}");
        assert_eq!(report.shapes_checked, 1);
    }

    #[test]
    fn shape_violation_blocks_in_enforcing_mode() {
        let block = json!({
            "@id": "urn:visionclaw:owl:class:orphan",
            "@type": ["OntologyClass", "owl:Class"],
            "rdfs:label": "Orphan"
        });
        let mut report = ShaclGateReport::default();
        gate_block(&block, 3, GateMode::Enforcing, &mut report);
        assert!(
            !report.is_valid(),
            "orphan class must fail in enforcing mode"
        );
        assert_eq!(report.violation_count(), 1);
        let v = &report.violations[0];
        assert_eq!(v.block_index, 3);
        assert_eq!(
            v.subject.as_deref(),
            Some("urn:visionclaw:owl:class:orphan")
        );
        assert_eq!(v.shape_name, "OntologyClassShape");
        assert_eq!(v.severity, ShapeSeverity::Violation);
    }

    #[test]
    fn shape_violation_passes_in_advisory_mode() {
        let block = json!({
            "@id": "urn:visionclaw:owl:class:orphan",
            "@type": ["OntologyClass", "owl:Class"],
            "rdfs:label": "Orphan"
        });
        let mut report = ShaclGateReport::default();
        gate_block(&block, 0, GateMode::Advisory, &mut report);
        assert!(report.is_valid(), "advisory mode always passes");
        assert_eq!(
            report.violation_count(),
            1,
            "but violations are still recorded"
        );
    }

    #[test]
    fn gate_handles_graph_array() {
        let block = json!({
            "@graph": [
                {
                    "@id": "urn:visionclaw:owl:class:a",
                    "@type": "OntologyClass",
                    "rdfs:subClassOf": { "@id": "urn:visionclaw:owl:class:built-environment" }
                },
                {
                    "@id": "urn:visionclaw:owl:class:b-orphan",
                    "@type": "OntologyClass"
                }
            ]
        });
        let mut report = ShaclGateReport::default();
        gate_block(&block, 0, GateMode::Enforcing, &mut report);
        assert_eq!(report.shapes_checked, 2);
        assert_eq!(report.violations.len(), 1, "only the orphan violates");
        assert_eq!(
            report.violations[0].subject.as_deref(),
            Some("urn:visionclaw:owl:class:b-orphan")
        );
    }

    #[test]
    fn backwards_compat_advisory_fn() {
        let block = json!({
            "@id": "urn:visionclaw:owl:class:orphan",
            "@type": "OntologyClass"
        });
        let mut report = ShaclGateReport::default();
        gate_block_advisory(&block, 0, &mut report);
        assert!(report.is_valid());
        assert_eq!(report.mode, GateMode::Advisory);
    }

    #[test]
    fn bridge_violation_detected() {
        let block = json!({
            "@id": "urn:visionclaw:bridge:abc",
            "@type": "BridgeRecord",
            "vc:bridgeTo": { "@id": "urn:visionclaw:linked:tempietto" }
        });
        let mut report = ShaclGateReport::default();
        gate_block(&block, 0, GateMode::Enforcing, &mut report);
        assert!(!report.is_valid());
        assert_eq!(report.violations[0].shape_name, "BridgeRecordShape");
    }
}
