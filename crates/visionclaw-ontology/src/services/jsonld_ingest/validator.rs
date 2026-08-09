// src/services/jsonld_ingest/validator.rs
//! Schema, IRI, profile, and provenance validation over `ExpandedDocument`.
//!
//! ADR-D01 §D12 enumerates the checks; this module implements them. Each
//! check is a small function that pushes `JsonLdIngestError` instances
//! onto a vector when violations are found. The pipeline runs all checks
//! before bailing so the operator dashboard can surface multiple errors
//! per file (though the test harness asserts that the FIRST raised error
//! matches the documented category for each fixture).
//!
//! ## Check inventory (D12 A–I, plus class-bit cross-check from TC-1)
//!
//! | ID   | Check                                  | Maps to fixture |
//! |------|----------------------------------------|-----------------|
//! | A    | JSON-LD expansion (well-formed)        | (parse layer)   |
//! | B.1  | @context present                       | 101             |
//! | B.2  | @context URL accepted                  | 102             |
//! | C.1  | @id present                            | (D6 invariant)  |
//! | C.2  | @type present                          | (D6 invariant)  |
//! | C.3  | prov:wasAttributedTo present           | (D8 invariant)  |
//! | C.4  | prov:generatedAtTime present           | (D8 invariant)  |
//! | D.1  | @id syntactically valid IRI            | 104             |
//! | E.1  | No owl:unionOf / owl:complementOf /    |                 |
//!        | owl:allValuesFrom / owl:disjointWith   | 106             |
//! | F.1  | Class bit matches IRI scheme           | 108             |
//! | G.1  | OntologyClass has subClassOf parent    | 103             |
//! | G.2  | Bridge target is not a stub            | 105             |
//! | V11  | JSON-LD 1.1 features need @version 1.1 | 100             |
//!
//! Fixture 107 (`MissingCodeFenceMarker`) is detected by the pipeline
//! before this stage runs — zero blocks → that error.

use std::collections::HashSet;

use super::errors::{JsonLdIngestError, Result};
use super::expander::{
    ExpandedDocument, ExpandedNode, ExpandedValue, NodeOrigin, V11Feature, ACCEPTED_CONTEXT_V1,
    ACCEPTED_CONTEXT_V2, OWL_NS, PROV_NS, VC_NS,
};

/// The declared root of the ontology class hierarchy (D-08 §C3 / fixture 103).
/// Every OntologyClass except this one MUST carry an `rdfs:subClassOf`.
/// Accepts both legacy `urn:visionclaw:owl:class:` and new `urn:ngm:class:` schemes.
pub const ONTOLOGY_ROOT_IRI: &str = "urn:visionclaw:owl:class:built-environment";
pub const ONTOLOGY_ROOT_IRI_NGM: &str = "urn:ngm:class:built-environment";

/// OWL constructs outside the OWL 2 EL profile (ADR-D01 §D4).
const OWL_OUT_OF_PROFILE: &[(&str, &str)] = &[
    ("unionOf", "OWL 2 EL §3, Table 1 — no disjunction"),
    ("complementOf", "OWL 2 EL §3, Table 1 — no negation"),
    (
        "allValuesFrom",
        "OWL 2 EL §3, Table 1 — no universal restrictions",
    ),
    ("disjointWith", "OWL 2 EL §3, Table 1 — no disjoint classes"),
    (
        "AllDisjointClasses",
        "OWL 2 EL §3, Table 1 — no disjoint classes",
    ),
    ("hasValue", "OWL 2 EL §3, Table 1"),
    ("hasSelf", "OWL 2 EL §3, Table 1"),
    (
        "minCardinality",
        "OWL 2 EL §3, Table 1 — no cardinality restrictions",
    ),
    (
        "maxCardinality",
        "OWL 2 EL §3, Table 1 — no cardinality restrictions",
    ),
    (
        "cardinality",
        "OWL 2 EL §3, Table 1 — no cardinality restrictions",
    ),
    ("AsymmetricProperty", "OWL 2 EL §3, Table 1"),
    ("IrreflexiveProperty", "OWL 2 EL §3, Table 1"),
];

/// Class-bit ↔ IRI-scheme correspondence per TC-1.
///
/// Returns the expected class bit for an `@id` based on its URN scheme
/// fragment after `urn:visionclaw:`.
fn iri_scheme_class_bit(iri: &str) -> Option<&'static str> {
    // Support both legacy urn:visionclaw: and new urn:ngm: schemes.
    if let Some(rest) = iri.strip_prefix("urn:ngm:") {
        // urn:ngm:{class,property,axiom}:<slug>
        let scheme = rest.split(':').next()?;
        return match scheme {
            "class" => Some("0x04000000"),
            "property" => Some("0x10000000"),
            "axiom" => Some("0x0C000000"),
            _ => None,
        };
    }
    let rest = iri.strip_prefix("urn:visionclaw:")?;
    let scheme = rest.split(':').next()?;
    match scheme {
        "page" | "nostr" | "bridge" => Some("0x40000000"),
        "agent" => Some("0x80000000"),
        "linked" => Some("0x08000000"),
        "owl" => {
            // urn:visionclaw:owl:{class,property,axiom}:<slug>
            let next = rest.strip_prefix("owl:")?.split(':').next()?;
            match next {
                "class" => Some("0x04000000"),
                "property" => Some("0x10000000"),
                "axiom" => Some("0x0C000000"),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Class-bit implied by a node's `@type` set.
fn types_class_bit(types: &[String]) -> Option<&'static str> {
    // Order matters: check most specific first.
    for t in types {
        if t == &format!("{}Page", VC_NS)
            || t == &format!("{}NostrSignedPage", VC_NS)
            || t == &format!("{}BridgeRecord", VC_NS)
        {
            return Some("0x40000000");
        }
        if t == &format!("{}AgentTelemetry", VC_NS) {
            return Some("0x80000000");
        }
        if t == &format!("{}LinkedPage", VC_NS) {
            return Some("0x08000000");
        }
        if t == &format!("{}Axiom", OWL_NS) {
            return Some("0x0C000000");
        }
        if t == &format!("{}Class", OWL_NS) {
            return Some("0x04000000");
        }
        if t == &format!("{}ObjectProperty", OWL_NS)
            || t == &format!("{}DatatypeProperty", OWL_NS)
            || t == &format!("{}AnnotationProperty", OWL_NS)
        {
            return Some("0x10000000");
        }
    }
    None
}

/// Stable display name for a type IRI (used in error messages).
fn type_display_name(types: &[String]) -> String {
    if let Some(first) = types.first() {
        if let Some(local) = first.rsplit('#').next() {
            if !local.is_empty() && local != first.as_str() {
                return local.to_string();
            }
        }
        if let Some(local) = first.rsplit('/').next() {
            if !local.is_empty() {
                return local.to_string();
            }
        }
        return first.clone();
    }
    "<no-type>".to_string()
}

/// Permitted `@context` URLs. Currently only v1 (D11).
pub fn accepted_contexts() -> Vec<String> {
    vec![
        ACCEPTED_CONTEXT_V1.to_string(),
        ACCEPTED_CONTEXT_V2.to_string(),
    ]
}

/// Run every check from the inventory. Returns the first error encountered
/// so the pipeline preserves the documented "one error per file" contract
/// for the fixture corpus. To collect ALL errors, call `validate_all`.
pub fn validate(file: &str, doc: &ExpandedDocument) -> Result<()> {
    let errors = validate_all(file, doc);
    if let Some(err) = errors.into_iter().next() {
        return Err(err);
    }
    Ok(())
}

/// Run every check; return all violations.
pub fn validate_all(file: &str, doc: &ExpandedDocument) -> Vec<JsonLdIngestError> {
    let mut errors = Vec::new();

    // B.1 @context present
    if doc.context_url.is_none() {
        errors.push(JsonLdIngestError::ContextMissing {
            file: file.to_string(),
            block_index: doc.block_index,
        });
        return errors;
    }
    // B.2 @context accepted
    if let Some(ref url) = doc.context_url {
        if !accepted_contexts().iter().any(|a| a == url) {
            errors.push(JsonLdIngestError::ContextVersionUnknown {
                file: file.to_string(),
                block_index: doc.block_index,
                found: url.clone(),
                supported: accepted_contexts(),
            });
            return errors;
        }
    }

    // V11: JSON-LD 1.1 `@included` array carrying dangling prov:Activity /
    // prov:Agent helpers (i.e. helpers NOT referenced by the parent block's
    // `prov:wasGeneratedBy` chain) requires explicit `@version: 1.1`
    // declaration. Fixture 100 vs valid 051: both use `@included`, but 051
    // links the activity via `prov:wasGeneratedBy` and 100 does not. The
    // `@nest` keyword always requires `@version: 1.1` regardless.
    if doc.v11_features.contains(&V11Feature::Nest) && !doc.version_declared {
        errors.push(JsonLdIngestError::SchemaVersionMissing {
            file: file.to_string(),
            block_index: doc.block_index,
            feature: "@nest",
        });
        return errors;
    }
    if doc.v11_features.contains(&V11Feature::Included)
        && !doc.version_declared
        && has_dangling_included(doc)
    {
        errors.push(JsonLdIngestError::SchemaVersionMissing {
            file: file.to_string(),
            block_index: doc.block_index,
            feature: "@included",
        });
        return errors;
    }

    // Per-node checks.
    for node in &doc.nodes {
        check_node(file, doc.block_index, node, &mut errors);
        if !errors.is_empty() {
            return errors;
        }
    }

    errors
}

fn check_node(
    file: &str,
    block_index: usize,
    node: &ExpandedNode,
    errors: &mut Vec<JsonLdIngestError>,
) {
    // C.1 @id present
    let iri = match &node.id {
        Some(s) if !s.is_empty() => s.clone(),
        _ => {
            // Missing @id is structural; report as malformed.
            errors.push(JsonLdIngestError::MalformedIri {
                file: file.to_string(),
                block_index,
                iri: String::new(),
                reason: "missing @id",
            });
            return;
        }
    };

    // D.1 @id syntactically valid IRI (no whitespace, has scheme separator).
    if !is_valid_iri(&iri) {
        errors.push(JsonLdIngestError::MalformedIri {
            file: file.to_string(),
            block_index,
            iri: iri.clone(),
            reason: "contains whitespace or lacks scheme",
        });
        return;
    }

    // F.1 Class bit matches IRI scheme. Only applies when type expresses
    // a known class.
    if let (Some(scheme_bit), Some(type_bit)) =
        (iri_scheme_class_bit(&iri), types_class_bit(&node.types))
    {
        if scheme_bit != type_bit {
            errors.push(JsonLdIngestError::ClassBitMismatch {
                file: file.to_string(),
                block_index,
                iri: iri.clone(),
                type_name: type_display_name(&node.types),
                type_bit,
                iri_bit: scheme_bit,
            });
            return;
        }
    }

    // G.1 OntologyClass except the declared root needs subClassOf.
    let is_ontology_class = node
        .types
        .iter()
        .any(|t| t == &format!("{}Class", OWL_NS) || t == &format!("{}OntologyClass", VC_NS));
    if is_ontology_class && iri != ONTOLOGY_ROOT_IRI && iri != ONTOLOGY_ROOT_IRI_NGM {
        let has_subclass = node
            .fields
            .iter()
            .any(|(p, _)| p == "http://www.w3.org/2000/01/rdf-schema#subClassOf");
        if !has_subclass {
            errors.push(JsonLdIngestError::RequiredFieldMissing {
                file: file.to_string(),
                block_index,
                iri: iri.clone(),
                field: "rdfs:subClassOf",
                type_name: "OntologyClass".to_string(),
            });
            return;
        }
    }

    // G.2 Bridge target must be concrete (not a urn:visionclaw:linked:*).
    let is_bridge = node
        .types
        .iter()
        .any(|t| t == &format!("{}BridgeRecord", VC_NS));
    if is_bridge {
        for (p, v) in &node.fields {
            if p == &format!("{}bridgeTo", VC_NS) {
                if let Some(target) = first_iri(v) {
                    if target.starts_with("urn:visionclaw:linked:") {
                        errors.push(JsonLdIngestError::BridgeTargetMustBeConcrete {
                            file: file.to_string(),
                            block_index,
                            bridge_iri: iri.clone(),
                            target_iri: target,
                        });
                        return;
                    }
                }
            }
        }
    }

    // E.1 OWL 2 EL profile boundary (scan node + nested values).
    if let Some(construct_with_ref) = find_out_of_profile(node) {
        errors.push(JsonLdIngestError::OutsideOwl2ElProfile {
            file: file.to_string(),
            block_index,
            construct: construct_with_ref.0,
            spec_reference: construct_with_ref.1,
            suggestion: "Model in <urn:ngm:graph:annotation> if the assertion is intent rather than reasoned-over fact.",
        });
        return;
    }

    // C.3 / C.4 PROV-O attribution + timestamp. Apply to top-level / @graph
    // nodes that are first-class subjects. @included entries are PROV-O
    // helpers (Activity / Agent) and may legitimately omit timestamp
    // (the seed shows them carrying only `wasAssociatedWith` etc.).
    if matches!(node.origin, NodeOrigin::Top | NodeOrigin::Graph) {
        let has_attr = node
            .fields
            .iter()
            .any(|(p, _)| p == &format!("{}wasAttributedTo", PROV_NS));
        if !has_attr {
            errors.push(JsonLdIngestError::ProvAttributionMissing {
                file: file.to_string(),
                block_index,
                iri: iri.clone(),
            });
            return;
        }
        let has_time = node
            .fields
            .iter()
            .any(|(p, _)| p == &format!("{}generatedAtTime", PROV_NS));
        if !has_time {
            errors.push(JsonLdIngestError::ProvTimestampMissing {
                file: file.to_string(),
                block_index,
                iri: iri.clone(),
            });
            return;
        }
    }
}

/// True when the document carries one or more `@included` entries (origin
/// `Included`) whose IRIs are NOT referenced from any top-level / @graph
/// node via `prov:wasGeneratedBy`. A dangling helper is the marker that
/// fixture 100 fails on: the activity record is declared but the parent
/// axiom never links to it. Fixture 051's `@included` activity IS linked
/// by `prov:wasGeneratedBy` and therefore is not dangling.
fn has_dangling_included(doc: &ExpandedDocument) -> bool {
    use std::collections::HashSet;

    // 1. Collect every IRI that any non-included or included node references
    //    via any object-valued predicate. The set is "all IRIs reachable
    //    from anywhere in the document other than the @id of the @included
    //    entry itself".
    let mut referenced: HashSet<String> = HashSet::new();
    for node in &doc.nodes {
        // For non-included nodes, every field's IRI values qualify as
        // references. For included nodes, we also pick up cross-references
        // (e.g. an Activity referencing its Agent).
        for (_, v) in &node.fields {
            collect_iris(v, &mut referenced);
        }
    }

    // 2. An @included entry is dangling if its @id is not referenced by
    //    anything else in the document.
    for node in &doc.nodes {
        if !matches!(node.origin, NodeOrigin::Included) {
            continue;
        }
        let iri = match &node.id {
            Some(s) => s,
            None => return true, // an @included with no @id is degenerate
        };
        if !referenced.contains(iri) {
            return true;
        }
    }

    false
}

fn collect_iris(v: &ExpandedValue, out: &mut std::collections::HashSet<String>) {
    match v {
        ExpandedValue::Iri(s) => {
            out.insert(s.clone());
        }
        ExpandedValue::Multi(arr) => {
            for e in arr {
                collect_iris(e, out);
            }
        }
        ExpandedValue::Nested(n) => {
            if let Some(iri) = &n.id {
                out.insert(iri.clone());
            }
            for (_, child) in &n.fields {
                collect_iris(child, out);
            }
        }
        _ => {}
    }
}

/// Find the first OWL 2 DL-only construct anywhere in a node's predicates
/// or nested values. Returns the local construct name and its spec
/// reference if found.
fn find_out_of_profile(node: &ExpandedNode) -> Option<(&'static str, &'static str)> {
    let mut seen_predicates = HashSet::new();
    for (p, v) in &node.fields {
        seen_predicates.insert(p.clone());
        if let Some(found) = predicate_or_value_out_of_profile(p, v) {
            return Some(found);
        }
    }
    None
}

fn predicate_or_value_out_of_profile(
    predicate: &str,
    value: &ExpandedValue,
) -> Option<(&'static str, &'static str)> {
    if let Some(local) = predicate.strip_prefix(OWL_NS) {
        for (name, spec_ref) in OWL_OUT_OF_PROFILE {
            if local == *name {
                return Some((name, spec_ref));
            }
        }
    }
    match value {
        ExpandedValue::Nested(node) => {
            for (p, v) in &node.fields {
                if let Some(found) = predicate_or_value_out_of_profile(p, v) {
                    return Some(found);
                }
            }
            None
        }
        ExpandedValue::Multi(arr) => {
            for v in arr {
                if let Some(found) = predicate_or_value_out_of_profile(predicate, v) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn first_iri(v: &ExpandedValue) -> Option<String> {
    match v {
        ExpandedValue::Iri(s) => Some(s.clone()),
        ExpandedValue::Nested(n) => n.id.clone(),
        ExpandedValue::Multi(arr) => arr.iter().find_map(first_iri),
        _ => None,
    }
}

/// Minimal RFC 3987 well-formedness check: no whitespace; contains a `:`
/// somewhere; doesn't have control characters. The seed N-Quads are
/// satisfied by these checks.
fn is_valid_iri(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    if !s.contains(':') {
        return false;
    }
    // Cheap scheme check — first char must be ASCII alpha.
    s.chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iri_scheme_bits() {
        assert_eq!(
            iri_scheme_class_bit("urn:visionclaw:page:abc"),
            Some("0x40000000")
        );
        assert_eq!(
            iri_scheme_class_bit("urn:visionclaw:agent:run-x:step-0"),
            Some("0x80000000")
        );
        assert_eq!(
            iri_scheme_class_bit("urn:visionclaw:linked:foo"),
            Some("0x08000000")
        );
        assert_eq!(
            iri_scheme_class_bit("urn:visionclaw:owl:class:cybernetics"),
            Some("0x04000000")
        );
        assert_eq!(
            iri_scheme_class_bit("urn:visionclaw:owl:axiom:abc"),
            Some("0x0C000000")
        );
        assert_eq!(
            iri_scheme_class_bit("urn:visionclaw:owl:property:p"),
            Some("0x10000000")
        );
    }

    #[test]
    fn iri_validity() {
        assert!(is_valid_iri("urn:visionclaw:page:abc"));
        assert!(is_valid_iri("did:nostr:npub1abc"));
        assert!(!is_valid_iri("urn visionclaw page invalid id"));
        assert!(!is_valid_iri(""));
        assert!(!is_valid_iri("no-scheme"));
    }
}
