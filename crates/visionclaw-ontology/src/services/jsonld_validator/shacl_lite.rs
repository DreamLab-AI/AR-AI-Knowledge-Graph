//! SHACL-lite shape checks for VisionClaw domain types.
//!
//! ADR-D01 §D12 G: the per-type required-field rules. We do NOT load
//! an external SHACL graph — the constraints are inlined here for
//! transparency and zero-dep validation. Adding a constraint requires
//! a fixture under `tests/fixtures/data-model/invalid/`.

use serde_json::Value;

use super::errors::ErrorCategory;
use super::iri;

/// Per-type required-field check.
///
/// Returns the list of `RequiredFieldMissing` and shape-violation
/// errors discovered on this entry. Frame-level errors
/// (context/id/prov) are not duplicated — `frame.rs` handles those.
pub fn validate_entry_shape(entry: &Value) -> Vec<ErrorCategory> {
    let mut issues = Vec::new();
    let Value::Object(map) = entry else {
        return issues;
    };
    let types = collect_types(entry);

    // OntologyClass shape (ADR-D01 §D6, invariant C3 in DDD-08):
    // - Every `OntologyClass` except the declared root
    //   (`urn:visionclaw:owl:class:built-environment`) must carry a
    //   `subClassOf` (or `rdfs:subClassOf`) link.
    if types.iter().any(|t| is_ontology_class_type(t)) {
        let is_root = map
            .get("@id")
            .or_else(|| map.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| {
                s == "urn:visionclaw:owl:class:built-environment"
                    || s == "urn:ngm:class:built-environment"
            })
            .unwrap_or(false);
        let has_parent = has_any_key(entry, &["subClassOf", "rdfs:subClassOf", "parentClasses"]);
        if !is_root && !has_parent {
            issues.push(ErrorCategory::RequiredFieldMissing {
                what: "subClassOf".to_string(),
            });
        }
    }

    // BridgeRecord shape (ADR-D01 §"Example 5", invariant G2 in
    // ADR-08): `vc:bridgeTo` must target a concrete entity, not a
    // `LinkedPage` stub.
    if types.iter().any(|t| is_bridge_type(t)) {
        if let Some(target_iri) =
            extract_iri_reference(map.get("vc:bridgeTo").or_else(|| map.get("bridgeTo")))
        {
            if iri::is_linked_page_iri(&target_iri) {
                issues.push(ErrorCategory::BridgeTargetMustBeConcrete { target: target_iri });
            }
        }
    }

    issues
}

/// Collect every `@type` declaration (string or array form).
pub fn collect_types(entry: &Value) -> Vec<String> {
    let Value::Object(map) = entry else {
        return vec![];
    };
    let raw = map.get("@type").or_else(|| map.get("type"));
    match raw {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => vec![],
    }
}

fn has_any_key(entry: &Value, keys: &[&str]) -> bool {
    let Value::Object(map) = entry else {
        return false;
    };
    keys.iter().any(|k| map.contains_key(*k))
}

fn is_ontology_class_type(t: &str) -> bool {
    matches!(t, "OntologyClass" | "owl:Class" | "Class")
}

fn is_bridge_type(t: &str) -> bool {
    matches!(t, "BridgeRecord" | "Bridge" | "vc:BridgeRecord")
}

/// Pull an `@id` reference out of a value that may be either a string
/// or `{"@id": "..."}` shape.
pub fn extract_iri_reference(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => map
            .get("@id")
            .or_else(|| map.get("id"))
            .and_then(|v| v.as_str().map(|s| s.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ontology_class_missing_parent_rejected() {
        let entry = json!({
            "@id": "urn:visionclaw:owl:class:orphan",
            "@type": ["OntologyClass", "owl:Class"],
            "rdfs:label": "Orphan"
        });
        let issues = validate_entry_shape(&entry);
        assert!(issues.iter().any(|c| matches!(
            c,
            ErrorCategory::RequiredFieldMissing { what } if what == "subClassOf"
        )));
    }

    #[test]
    fn ontology_class_root_does_not_require_parent() {
        let entry = json!({
            "@id": "urn:visionclaw:owl:class:built-environment",
            "@type": "OntologyClass",
            "rdfs:label": "Built Environment"
        });
        let issues = validate_entry_shape(&entry);
        assert!(issues.is_empty());
    }

    #[test]
    fn bridge_pointing_at_stub_rejected() {
        let entry = json!({
            "@id": "urn:visionclaw:bridge:abc",
            "@type": "BridgeRecord",
            "vc:bridgeTo": { "@id": "urn:visionclaw:linked:tempietto" }
        });
        let issues = validate_entry_shape(&entry);
        assert!(issues
            .iter()
            .any(|c| matches!(c, ErrorCategory::BridgeTargetMustBeConcrete { .. })));
    }
}
