// src/services/jsonld_ingest/shacl_gate.rs
//! SHACL validation gate for the ingest pipeline (PRD-022 WS-1 — enforcing).
//!
//! The gate is now **shape-driven**: it validates each parsed JSON-LD entry
//! against the five canonical `.shacl.ttl` NodeShapes (parsed by
//! [`crate::services::jsonld_validator::shacl`]) rather than the old hard-coded
//! SHACL-lite matcher. Constraints carrying `sh:severity sh:Violation` block a
//! write in [`GateMode::Enforcing`]; `sh:Warning` stays advisory in every mode.
//!
//! ## Mode selection
//!
//! The effective mode is process-wide config ([`global_gate_mode`]), seeded at
//! startup from `ontology.shacl_mode` (default `enforcing`; see the settings
//! model). Callers may still pass an explicit [`GateMode`] for testing.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::Serialize;
use serde_json::Value;

use crate::services::jsonld_validator::shacl;
pub use crate::services::jsonld_validator::ShaclSeverity as ShapeSeverity;

/// Gate operating mode (PRD-022 §4.1, ADR-127 D1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMode {
    /// Write paths: `sh:Violation` findings reject the payload.
    Enforcing,
    /// Read/dry-run paths: findings log + metric but proceed.
    Advisory,
}

impl Default for GateMode {
    /// PRD-022 WS-1 posture: enforcing by default (rollback via config).
    fn default() -> Self {
        GateMode::Enforcing
    }
}

impl GateMode {
    /// Parse a config string (`"enforcing"` | `"advisory"`). Unknown / empty
    /// values fall back to the safe default (`Enforcing`).
    pub fn from_config_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "advisory" => GateMode::Advisory,
            _ => GateMode::Enforcing,
        }
    }

    /// Canonical lower-case string form (for config echo / trust-status).
    pub fn as_str(&self) -> &'static str {
        match self {
            GateMode::Enforcing => "enforcing",
            GateMode::Advisory => "advisory",
        }
    }
}

const MODE_ENFORCING: u8 = 0;
const MODE_ADVISORY: u8 = 1;

/// Process-wide effective gate mode. Defaults to enforcing so a binary that
/// never calls [`set_global_gate_mode`] is still fail-closed.
static GLOBAL_GATE_MODE: AtomicU8 = AtomicU8::new(MODE_ENFORCING);

/// Set the process-wide gate mode (called once at startup from settings).
pub fn set_global_gate_mode(mode: GateMode) {
    let v = match mode {
        GateMode::Enforcing => MODE_ENFORCING,
        GateMode::Advisory => MODE_ADVISORY,
    };
    GLOBAL_GATE_MODE.store(v, Ordering::Relaxed);
}

/// The current process-wide gate mode.
pub fn global_gate_mode() -> GateMode {
    match GLOBAL_GATE_MODE.load(Ordering::Relaxed) {
        MODE_ADVISORY => GateMode::Advisory,
        _ => GateMode::Enforcing,
    }
}

/// A single SHACL shape violation, located to block + focus node + path.
#[derive(Debug, Clone)]
pub struct ShaclViolation {
    /// 0-based block index within the source file.
    pub block_index: usize,
    /// The `@id` of the offending entry, when present.
    pub focus_node: Option<String>,
    /// The property path that failed (compact form, e.g. `vc:agent_pubkey`).
    pub path: String,
    /// The SHACL constraint component that fired (e.g. `sh:minCount`).
    pub constraint: String,
    /// The shape that owns the failed constraint (e.g. `AgentNodeShape`).
    pub shape_name: String,
    /// Human-readable `sh:message`.
    pub message: String,
    /// Severity: `Violation` blocks in enforcing mode; `Warning` is advisory.
    pub severity: ShapeSeverity,
}

/// Structured, serialisable violation payload for the rejection error and for
/// operator dashboards.
#[derive(Debug, Clone, Serialize)]
pub struct ShaclViolationDetail {
    pub block_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_node: Option<String>,
    pub path: String,
    pub constraint: String,
    pub shape: String,
    pub message: String,
    pub severity: String,
}

/// Aggregated SHACL gate result for one ingest.
#[derive(Debug, Clone)]
pub struct ShaclGateReport {
    pub violations: Vec<ShaclViolation>,
    /// Number of entries (shapes) the gate inspected.
    pub shapes_checked: usize,
    /// The mode the gate was operating in.
    pub mode: GateMode,
    /// Whether the shapes graph loaded ≥1 NodeShape (false = misconfigured).
    pub w3c_shapes_available: bool,
}

impl Default for ShaclGateReport {
    fn default() -> Self {
        Self {
            violations: Vec::new(),
            shapes_checked: 0,
            mode: GateMode::default(),
            w3c_shapes_available: shacl::builtin().shape_count() > 0,
        }
    }
}

impl ShaclGateReport {
    /// In enforcing mode the gate passes iff no `sh:Violation` exists.
    /// In advisory mode the gate always passes (findings are recorded).
    pub fn is_valid(&self) -> bool {
        match self.mode {
            GateMode::Enforcing => self
                .violations
                .iter()
                .all(|v| v.severity != ShapeSeverity::Violation),
            GateMode::Advisory => true,
        }
    }

    /// Count of `sh:Violation`-severity findings.
    pub fn violation_count(&self) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity == ShapeSeverity::Violation)
            .count()
    }

    /// Count of `sh:Warning`-severity findings.
    pub fn warning_count(&self) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity == ShapeSeverity::Warning)
            .count()
    }

    /// Structured payload of the blocking (`sh:Violation`) findings — attached
    /// to the rejection error and surfaced to dashboards.
    pub fn violation_details(&self) -> Vec<ShaclViolationDetail> {
        self.violations
            .iter()
            .filter(|v| v.severity == ShapeSeverity::Violation)
            .map(|v| ShaclViolationDetail {
                block_index: v.block_index,
                focus_node: v.focus_node.clone(),
                path: v.path.clone(),
                constraint: v.constraint.clone(),
                shape: v.shape_name.clone(),
                message: v.message.clone(),
                severity: v.severity.to_string(),
            })
            .collect()
    }
}

/// Run the shape-driven gate over one parsed JSON-LD block in the given mode.
///
/// Every entry (including `@graph` members) is validated against the builtin
/// shapes; findings are appended to `report` with their declared severity.
pub fn gate_block(block: &Value, block_index: usize, mode: GateMode, report: &mut ShaclGateReport) {
    report.mode = mode;
    let shapes = shacl::builtin();
    report.w3c_shapes_available = shapes.shape_count() > 0;

    for entry in collect_entries(block) {
        report.shapes_checked += 1;
        for finding in shapes.validate_entry(entry) {
            report.violations.push(ShaclViolation {
                block_index,
                focus_node: finding.focus_node,
                path: finding.path,
                constraint: finding.constraint,
                shape_name: finding.shape,
                message: finding.message,
                severity: finding.severity,
            });
        }
    }
}

/// Convenience wrapper that runs the gate in advisory mode.
pub fn gate_block_advisory(block: &Value, block_index: usize, report: &mut ShaclGateReport) {
    gate_block(block, block_index, GateMode::Advisory, report);
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_agent() -> Value {
        json!({
            "@id": "urn:visionclaw:agent:run-x",
            "@type": "AgentNode",
            "vc:agent_pubkey": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "rdfs:label": "Agent X"
        })
    }

    fn orphan_agent() -> Value {
        // Missing vc:agent_pubkey → sh:Violation.
        json!({
            "@id": "urn:visionclaw:agent:run-x",
            "@type": "AgentNode",
            "rdfs:label": "Agent X"
        })
    }

    #[test]
    fn default_mode_is_enforcing() {
        assert_eq!(GateMode::default(), GateMode::Enforcing);
    }

    #[test]
    fn mode_config_roundtrip() {
        assert_eq!(GateMode::from_config_str("advisory"), GateMode::Advisory);
        assert_eq!(GateMode::from_config_str("enforcing"), GateMode::Enforcing);
        assert_eq!(GateMode::from_config_str("garbage"), GateMode::Enforcing);
        assert_eq!(GateMode::Advisory.as_str(), "advisory");
        assert_eq!(GateMode::Enforcing.as_str(), "enforcing");
    }

    #[test]
    fn valid_agent_passes_gate() {
        let mut report = ShaclGateReport::default();
        gate_block(&valid_agent(), 0, GateMode::Enforcing, &mut report);
        assert!(report.is_valid(), "well-formed agent must pass: {report:?}");
        assert_eq!(report.shapes_checked, 1);
        assert_eq!(report.violation_count(), 0);
    }

    #[test]
    fn violation_blocks_in_enforcing_mode() {
        let mut report = ShaclGateReport::default();
        gate_block(&orphan_agent(), 3, GateMode::Enforcing, &mut report);
        assert!(!report.is_valid(), "missing pubkey must block in enforcing mode");
        assert_eq!(report.violation_count(), 1);
        let v = &report
            .violations
            .iter()
            .find(|v| v.severity == ShapeSeverity::Violation)
            .unwrap();
        assert_eq!(v.block_index, 3);
        assert_eq!(v.focus_node.as_deref(), Some("urn:visionclaw:agent:run-x"));
        assert_eq!(v.shape_name, "AgentNode shape");
        assert_eq!(v.path, "vc:agent_pubkey");
    }

    #[test]
    fn violation_passes_in_advisory_mode() {
        let mut report = ShaclGateReport::default();
        gate_block(&orphan_agent(), 0, GateMode::Advisory, &mut report);
        assert!(report.is_valid(), "advisory mode always passes");
        assert_eq!(report.violation_count(), 1, "but the violation is recorded");
    }

    #[test]
    fn warning_never_blocks() {
        // Agent with a valid pubkey but no rdfs:label → sh:Warning only.
        let entry = json!({
            "@id": "urn:visionclaw:agent:run-x",
            "@type": "AgentNode",
            "vc:agent_pubkey": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        });
        let mut report = ShaclGateReport::default();
        gate_block(&entry, 0, GateMode::Enforcing, &mut report);
        assert!(report.is_valid(), "a warning must not block even in enforcing mode");
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.violation_count(), 0);
    }

    #[test]
    fn gate_handles_graph_array() {
        let block = json!({
            "@graph": [
                valid_agent(),
                {
                    "@id": "urn:visionclaw:agent:run-y",
                    "@type": "AgentNode",
                    "rdfs:label": "Agent Y"
                }
            ]
        });
        let mut report = ShaclGateReport::default();
        gate_block(&block, 0, GateMode::Enforcing, &mut report);
        assert_eq!(report.shapes_checked, 2);
        assert_eq!(report.violation_count(), 1, "only the second agent violates");
        assert_eq!(
            report.violations[0].focus_node.as_deref(),
            Some("urn:visionclaw:agent:run-y")
        );
    }

    #[test]
    fn violation_details_are_structured() {
        let mut report = ShaclGateReport::default();
        gate_block(&orphan_agent(), 2, GateMode::Enforcing, &mut report);
        let details = report.violation_details();
        assert_eq!(details.len(), 1);
        let d = &details[0];
        assert_eq!(d.block_index, 2);
        assert_eq!(d.focus_node.as_deref(), Some("urn:visionclaw:agent:run-x"));
        assert_eq!(d.path, "vc:agent_pubkey");
        assert_eq!(d.constraint, "sh:minCount");
        assert_eq!(d.severity, "violation");
        assert!(!d.message.is_empty());
        // Serialises cleanly for the dashboard / error payload.
        let js = serde_json::to_value(d).unwrap();
        assert_eq!(js["path"], "vc:agent_pubkey");
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
        assert_eq!(report.violations[0].shape_name, "BridgeRecord shape");
    }

    #[test]
    fn global_mode_toggle_roundtrips() {
        set_global_gate_mode(GateMode::Advisory);
        assert_eq!(global_gate_mode(), GateMode::Advisory);
        set_global_gate_mode(GateMode::Enforcing);
        assert_eq!(global_gate_mode(), GateMode::Enforcing);
    }
}
