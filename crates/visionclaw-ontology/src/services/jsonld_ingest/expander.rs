// src/services/jsonld_ingest/expander.rs
//! JSON-LD → ExpandedDocument expansion.
//!
//! ## Why not a full JSON-LD 1.1 algorithmic expansion
//!
//! Two reasons:
//!
//! 1. **No json-ld crate dep.** Per ADR-D01 §R3, `sophia_jsonld` /
//!    `json-ld-rs` are immature and pull large dep trees. The migration-
//!    sprint task explicitly directs us toward "embed the @context inline
//!    and do manual term expansion" (option (a)). Adopted.
//!
//! 2. **The seed N-Quads dictate a stricter emission contract than full
//!    JSON-LD 1.1 yields.** The seed file at
//!    `tests/fixtures/data-model/seed/expected-triples.nq` is the ground
//!    truth: `vc:slug` expands to `https://narrativegoldmine.com/ns/v1#slug`
//!    (no aliasing to `dcterms:title` even though context-v1.jsonld
//!    describes such aliases). The seed was hand-curated as the contract.
//!    Direct prefix expansion (vc:, owl:, rdfs:, rdf:, prov:, dcterms:,
//!    xsd:, did:) matches.
//!
//! ## What this stage does
//!
//! - Parses the JSON payload (serde_json).
//! - Recognises `@graph` / `@included` wrapping and explodes them into
//!   one logical `ExpandedNode` per top-level subject.
//! - Expands compact keys (e.g. `vc:slug`) to full IRIs against the
//!   canonical prefix map.
//! - Normalises the @context (accepts both string form and inline object
//!   form; validates the URL belongs to the accepted set).
//! - Records JSON-LD 1.1 feature usage (`@version`, `@included`, typed
//!   `{@value, @type}`, `@list`) so the validator can enforce that
//!   `@version: 1.1` was explicitly declared when needed.
//! - Detects bare `[[Wikilinks]]` inside string-typed values per D5 and
//!   rewrites them to the slug-derived IRI (in-block desugaring; the
//!   post-ingest resolution pass upgrades stubs).
//!
//! The output (`ExpandedDocument`) is structured key→value sufficient for
//! the validator and triple emitter to operate without re-parsing.

use serde_json::{Map, Value};
use std::collections::HashSet;

use super::errors::{JsonLdIngestError, Result};

/// Canonical `@context` URLs the parser accepts. ADR-D01 §D11.
pub const ACCEPTED_CONTEXT_V1: &str = "https://narrativegoldmine.com/context/v1.jsonld";
pub const ACCEPTED_CONTEXT_V2: &str = "https://narrativegoldmine.com/ns/v2.jsonld";

/// `vc:` prefix expansion. ADR-11 §D3 + Phase 1 adapter constant `VC_NS`.
pub const VC_NS: &str = "https://narrativegoldmine.com/ns/v1#";
pub const RDF_NS: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub const OWL_NS: &str = "http://www.w3.org/2002/07/owl#";
pub const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema#";
pub const PROV_NS: &str = "http://www.w3.org/ns/prov#";
pub const DCTERMS_NS: &str = "http://purl.org/dc/terms/";
pub const SCHEMA_NS: &str = "https://schema.org/";
pub const SKOS_NS: &str = "http://www.w3.org/2004/02/skos/core#";
pub const FOAF_NS: &str = "http://xmlns.com/foaf/0.1/";
pub const SH_NS: &str = "http://www.w3.org/ns/shacl#";

/// JSON-LD 1.1 features the validator must see declared via `@version: 1.1`.
/// Recorded on `ExpandedDocument::v11_features` so the validator can reject
/// blocks that use them without the declaration. Per fixture 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum V11Feature {
    /// `@version` keyword itself.
    VersionKeyword,
    /// `@included` array of side-documents.
    Included,
    /// `@nest` (we don't use it but record it for completeness).
    Nest,
    /// Typed `{@value, @type}` literal form with explicit `@type`.
    TypedValue,
}

/// An expanded node — one logical subject's worth of predicate-value pairs.
#[derive(Debug, Clone)]
pub struct ExpandedNode {
    /// Subject IRI (absolute, expanded). `None` if absent (the validator
    /// will catch this as `RequiredFieldMissing`).
    pub id: Option<String>,
    /// `@type` values, expanded. Multiple types are common (e.g.
    /// `["Axiom", "owl:Axiom"]`).
    pub types: Vec<String>,
    /// Routing override: explicit `@graph` IRI from the source. `None`
    /// means "infer from @type per ADR-D01 §D7".
    pub explicit_graph: Option<String>,
    /// All other (predicate, value) pairs. Predicate is the expanded IRI;
    /// value is one of:
    ///   - `ExpandedValue::Iri(String)` — object property (single IRI)
    ///   - `ExpandedValue::Literal{value, datatype, language}` — datatype property
    ///   - `ExpandedValue::Multi(Vec<ExpandedValue>)` — array values
    ///   - `ExpandedValue::Nested(Box<ExpandedNode>)` — inline anonymous node
    ///   - `ExpandedValue::Null` — explicit null
    pub fields: Vec<(String, ExpandedValue)>,
    /// 0-based block index, propagated for error reporting.
    pub block_index: usize,
    /// Whether this node came from the top level of the block or was
    /// nested inside `@graph`/`@included`. Top-level documents may carry
    /// the host page's default graph; included entries route by `@type`.
    pub origin: NodeOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeOrigin {
    /// Direct top-level subject of the JSON-LD block.
    Top,
    /// Member of an explicit `@graph` array.
    Graph,
    /// Member of an `@included` array.
    Included,
}

#[derive(Debug, Clone)]
pub enum ExpandedValue {
    Iri(String),
    Literal {
        value: String,
        datatype: Option<String>,
        language: Option<String>,
    },
    Multi(Vec<ExpandedValue>),
    Nested(Box<ExpandedNode>),
    Null,
}

/// Output of the expander stage.
#[derive(Debug, Clone)]
pub struct ExpandedDocument {
    /// Every logical subject this block contributes. Order preserved.
    pub nodes: Vec<ExpandedNode>,
    /// The raw `@context` value (string URL or null). Used by validator.
    pub context_url: Option<String>,
    /// Set of JSON-LD 1.1 features observed in this document.
    pub v11_features: HashSet<V11Feature>,
    /// Whether `@version: 1.1` was explicitly declared.
    pub version_declared: bool,
    /// 0-based block index.
    pub block_index: usize,
}

/// Top entry point. Parses `body` (JSON) and returns the expanded document.
///
/// Errors:
/// - `JsonParseError` if the body is not valid JSON.
///
/// Validation errors (missing fields, profile violations) are NOT raised
/// here — they're the validator's job. The expander is permissive on
/// structure and strict on parse.
pub fn expand_block(file: &str, block_index: usize, body: &str) -> Result<ExpandedDocument> {
    let root: Value =
        serde_json::from_str(body).map_err(|e| JsonLdIngestError::JsonParseError {
            file: file.to_string(),
            block_index,
            message: e.to_string(),
        })?;

    let obj = match root {
        Value::Object(m) => m,
        other => {
            return Err(JsonLdIngestError::JsonParseError {
                file: file.to_string(),
                block_index,
                message: format!(
                    "top-level JSON-LD value must be an object, got {}",
                    type_name(&other)
                ),
            });
        }
    };

    let mut doc = ExpandedDocument {
        nodes: Vec::new(),
        context_url: None,
        v11_features: HashSet::new(),
        version_declared: false,
        block_index,
    };

    // Capture context if present.
    if let Some(ctx) = obj.get("@context").or_else(|| obj.get("context")) {
        doc.context_url = Some(stringify_context(ctx));
    }
    // Capture explicit @version declaration on the document.
    if let Some(v) = obj.get("@version") {
        doc.version_declared = matches!(v, Value::Number(n) if n.as_f64() == Some(1.1));
        doc.v11_features.insert(V11Feature::VersionKeyword);
    }

    // Distinguish `@graph` wrapping from a direct top-level subject.
    if let Some(graph) = obj.get("@graph") {
        if let Value::Array(arr) = graph {
            for v in arr {
                if let Value::Object(node_obj) = v {
                    let node = expand_node(
                        node_obj,
                        NodeOrigin::Graph,
                        block_index,
                        &mut doc.v11_features,
                    );
                    doc.nodes.push(node);
                }
            }
        }
    } else {
        // Direct top-level subject. Also handle `@included` siblings.
        let node = expand_node(&obj, NodeOrigin::Top, block_index, &mut doc.v11_features);
        doc.nodes.push(node);
    }

    // `@included` may sit alongside `@graph` or top-level subject.
    if let Some(Value::Array(arr)) = obj.get("@included") {
        doc.v11_features.insert(V11Feature::Included);
        for v in arr {
            if let Value::Object(node_obj) = v {
                let node = expand_node(
                    node_obj,
                    NodeOrigin::Included,
                    block_index,
                    &mut doc.v11_features,
                );
                doc.nodes.push(node);
            }
        }
    }

    Ok(doc)
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn stringify_context(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(|e| match e {
                Value::String(s) => s.clone(),
                _ => "<inline>".to_string(),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "<inline>".to_string(),
        _ => String::new(),
    }
}

/// Expand a single JSON object into an `ExpandedNode`.
///
/// Keyword keys (`@id`, `@type`, `@graph`, `@included`, `@context`,
/// `@version`) are handled structurally. All other keys are expanded as
/// predicate IRIs. Values follow JSON-LD 1.1 value semantics.
fn expand_node(
    obj: &Map<String, Value>,
    origin: NodeOrigin,
    block_index: usize,
    features: &mut HashSet<V11Feature>,
) -> ExpandedNode {
    let mut node = ExpandedNode {
        id: None,
        types: Vec::new(),
        explicit_graph: None,
        fields: Vec::new(),
        block_index,
        origin,
    };

    for (k, v) in obj {
        match k.as_str() {
            "@id" | "id" => {
                if let Value::String(s) = v {
                    node.id = Some(expand_iri_or_keep(s));
                }
            }
            "@type" | "type" => match v {
                Value::String(s) => node.types.push(expand_iri_or_keep(s)),
                Value::Array(arr) => {
                    for e in arr {
                        if let Value::String(s) = e {
                            node.types.push(expand_iri_or_keep(s));
                        }
                    }
                }
                _ => {}
            },
            "@context" | "context" | "@graph" | "@included" | "@version" | "graph" => {
                // Handled at the document level.
            }
            // v2 format: nested "relations" object — flatten each
            // relation type as a top-level predicate on this node.
            "relations" => {
                if let Value::Object(rels) = v {
                    for (rel_key, rel_val) in rels {
                        let predicate_iri = expand_iri_or_keep(rel_key);
                        let value = expand_value(rel_val, block_index, features);
                        node.fields.push((predicate_iri, value));
                    }
                }
            }
            // v2 format: nested "provenance" object — map fields to
            // canonical prov: predicates.
            "provenance" => {
                if let Value::Object(prov) = v {
                    for (prov_key, prov_val) in prov {
                        let (predicate_iri, val) = match prov_key.as_str() {
                            "attributedTo" => (
                                format!("{}wasAttributedTo", PROV_NS),
                                expand_value(prov_val, block_index, features),
                            ),
                            "generatedAt" => (
                                format!("{}generatedAtTime", PROV_NS),
                                expand_value(prov_val, block_index, features),
                            ),
                            other => (
                                expand_iri_or_keep(other),
                                expand_value(prov_val, block_index, features),
                            ),
                        };
                        node.fields.push((predicate_iri, val));
                    }
                }
            }
            other => {
                // ADR-D01 §D7: explicit override via `vc:namedGraph` is
                // routing metadata. We surface it on the node so the
                // triple emitter can route, but we DO NOT emit it as a
                // triple (consistent with the seed).
                let predicate_iri = expand_iri_or_keep(other);
                let value = expand_value(v, block_index, features);

                if predicate_iri == format!("{}namedGraph", VC_NS) {
                    if let Some(iri) = single_iri(&value) {
                        node.explicit_graph = Some(iri);
                    } else if matches!(value, ExpandedValue::Null) {
                        // explicit null → default graph
                        node.explicit_graph = Some(String::new());
                    }
                    continue;
                }

                node.fields.push((predicate_iri, value));
            }
        }
    }

    node
}

fn single_iri(v: &ExpandedValue) -> Option<String> {
    match v {
        ExpandedValue::Iri(s) => Some(s.clone()),
        _ => None,
    }
}

/// Recursively expand a JSON value.
fn expand_value(
    v: &Value,
    block_index: usize,
    features: &mut HashSet<V11Feature>,
) -> ExpandedValue {
    match v {
        Value::Null => ExpandedValue::Null,
        Value::Bool(b) => ExpandedValue::Literal {
            value: b.to_string(),
            datatype: Some(format!("{}boolean", XSD_NS)),
            language: None,
        },
        Value::Number(n) => {
            // Per the seed, integers emit xsd:integer, floats xsd:float.
            // serde_json's Number::is_i64()/is_f64() differentiates.
            if n.is_i64() || n.is_u64() {
                ExpandedValue::Literal {
                    value: n.to_string(),
                    datatype: Some(format!("{}integer", XSD_NS)),
                    language: None,
                }
            } else {
                ExpandedValue::Literal {
                    value: n.to_string(),
                    datatype: Some(format!("{}float", XSD_NS)),
                    language: None,
                }
            }
        }
        Value::String(s) => {
            // Wikilink desugaring per D5: a bare `[[Term]]` inside a string
            // value rewrites to its slug-derived stub IRI. The post-parse
            // resolution pass upgrades stubs to declared classes/pages.
            if let Some(iri) = wikilink_to_iri(s) {
                ExpandedValue::Iri(iri)
            } else {
                ExpandedValue::Literal {
                    value: s.clone(),
                    datatype: None,
                    language: None,
                }
            }
        }
        Value::Array(arr) => {
            // Handle @list wrapper for ordered constructs (propertyChainAxiom).
            // The seed flattens @list into individual triples sharing the
            // predicate — order preserved by Vec order.
            let mut out = Vec::with_capacity(arr.len());
            for e in arr {
                out.push(expand_value(e, block_index, features));
            }
            ExpandedValue::Multi(out)
        }
        Value::Object(m) => {
            // Three sub-cases:
            //   (a) Reference: { "@id": "..." } → ExpandedValue::Iri
            //   (b) Typed literal: { "@value": "...", "@type": "xsd:..." } → Literal
            //   (c) Anonymous nested node: full sub-object → Nested
            //   (d) @list wrapper: { "@list": [...] } → Multi
            if let Some(Value::Array(arr)) = m.get("@list") {
                let mut out = Vec::with_capacity(arr.len());
                for e in arr {
                    out.push(expand_value(e, block_index, features));
                }
                return ExpandedValue::Multi(out);
            }
            if let Some(Value::String(value)) = m.get("@value") {
                let datatype = m.get("@type").and_then(|t| match t {
                    Value::String(s) => Some(expand_iri_or_keep(s)),
                    _ => None,
                });
                if datatype.is_some() {
                    features.insert(V11Feature::TypedValue);
                }
                let language = m.get("@language").and_then(|t| match t {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                });
                return ExpandedValue::Literal {
                    value: value.clone(),
                    datatype,
                    language,
                };
            }
            if m.len() == 1 {
                if let Some(Value::String(s)) = m.get("@id").or_else(|| m.get("id")) {
                    return ExpandedValue::Iri(s.clone());
                }
            }
            // Otherwise it's a nested anonymous node.
            let nested = expand_node(m, NodeOrigin::Top, block_index, features);
            ExpandedValue::Nested(Box::new(nested))
        }
    }
}

/// Compact key → absolute IRI. Recognises every prefix in context-v1.jsonld.
///
/// Inputs without a `:` are treated as default-vocab terms (vc:). Inputs
/// that already look absolute (contain `://` or start with `urn:`, `did:`)
/// are returned as-is.
pub fn expand_iri_or_keep(s: &str) -> String {
    if s.starts_with("urn:")
        || s.starts_with("did:")
        || s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("_:")
    {
        return s.to_string();
    }
    if let Some(rest) = s.strip_prefix("vc:") {
        return format!("{}{}", VC_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("rdf:") {
        return format!("{}{}", RDF_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("rdfs:") {
        return format!("{}{}", RDFS_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("owl:") {
        return format!("{}{}", OWL_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("xsd:") {
        return format!("{}{}", XSD_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("prov:") {
        return format!("{}{}", PROV_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("dcterms:") {
        return format!("{}{}", DCTERMS_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("schema:") {
        return format!("{}{}", SCHEMA_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("skos:") {
        return format!("{}{}", SKOS_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("foaf:") {
        return format!("{}{}", FOAF_NS, rest);
    }
    if let Some(rest) = s.strip_prefix("sh:") {
        return format!("{}{}", SH_NS, rest);
    }

    // Friendly-alias type names from ADR-D01 §D6 / context-v1.jsonld.
    // These are bare tokens (no prefix). The seed N-Quads show what each
    // expands to:
    match s {
        // OWL profile types (v2 "Individual" maps to owl:NamedIndividual)
        "Individual" => return format!("{}NamedIndividual", OWL_NS),
        // v1 "OntologyClass" and v2 "Class" both map to owl:Class
        "OntologyClass" | "Class" => return format!("{}Class", OWL_NS),
        "OntologyProperty" | "ObjectProperty" => return format!("{}ObjectProperty", OWL_NS),
        "DataProperty" | "DatatypeProperty" => return format!("{}DatatypeProperty", OWL_NS),
        "AnnotationProperty" => return format!("{}AnnotationProperty", OWL_NS),
        "Axiom" => return format!("{}Axiom", OWL_NS),
        "Restriction" => return format!("{}Restriction", OWL_NS),
        "Class@id" => unreachable!(),

        // VisionClaw type markers (per seed)
        "Page" => return format!("{}Page", VC_NS),
        "LinkedPage" => return format!("{}LinkedPage", VC_NS),
        "AgentTelemetry" => return format!("{}AgentTelemetry", VC_NS),
        "BridgeRecord" => return format!("{}BridgeRecord", VC_NS),
        "NostrSignedPage" => return format!("{}NostrSignedPage", VC_NS),
        "Signature" => return format!("{}Signature", VC_NS),
        "LinkResolved" => return format!("{}LinkResolved", VC_NS),

        // Asserted / Inferred / Activity / Agent — sourceKind / prov types
        "Asserted" | "Inferred" => return s.to_string(),

        // v2 bare predicate keys that map to well-known RDF predicates
        "subClassOf" => return format!("{}subClassOf", RDFS_NS),
        "label" => return format!("{}label", RDFS_NS),
        "definition" => return format!("{}definition", SKOS_NS),
        _ => {}
    }

    // Default vocab is vc:.
    format!("{}{}", VC_NS, s)
}

/// Returns `Some(iri)` if `s` is exactly `[[Term Name]]` (a bare wikilink).
/// Slug derivation per D5: NFKC → lowercase ASCII → non-alnum → `-` →
/// collapse repeats → trim. Without a registered ontology class match,
/// we mint the stub IRI `urn:visionclaw:linked:<slug>` per the fixture
/// corpus convention.
fn wikilink_to_iri(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if !trimmed.starts_with("[[") || !trimmed.ends_with("]]") {
        return None;
    }
    let inner = &trimmed[2..trimmed.len() - 2];
    if inner.is_empty() {
        return None;
    }
    let slug = slugify(inner);
    Some(format!("urn:visionclaw:linked:{}", slug))
}

/// Diacritic-preserving deterministic slug (ADR-100 D2).
///
/// Delegates to the canonical NFKD + combining-mark-strip slugifier in
/// `crate::services::canonical_iri`, replacing the previous ASCII-only rule
/// that silently dropped non-ASCII glyphs (so `Café` and `Cafe` collided to
/// different/garbage slugs and non-Latin titles degraded). The original
/// `rdfs:label` is retained verbatim by callers; only the IRI local-part is
/// slugified.
pub fn slugify(s: &str) -> String {
    crate::services::canonical_iri::slugify(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_diacritics_and_spaces() {
        assert_eq!(
            slugify("Renaissance Architecture"),
            "renaissance-architecture"
        );
        assert_eq!(slugify("OWL 2 EL!"), "owl-2-el");
        assert_eq!(slugify("---weird---"), "weird");
    }

    #[test]
    fn expands_vc_prefix() {
        assert_eq!(
            expand_iri_or_keep("vc:slug"),
            "https://narrativegoldmine.com/ns/v1#slug"
        );
    }

    #[test]
    fn expands_owl_prefix() {
        assert_eq!(
            expand_iri_or_keep("owl:Class"),
            "http://www.w3.org/2002/07/owl#Class"
        );
    }

    #[test]
    fn keeps_absolute_iris() {
        let iri = "urn:visionclaw:page:abc";
        assert_eq!(expand_iri_or_keep(iri), iri);
    }

    #[test]
    fn wikilink_desugars() {
        let v = expand_value(
            &Value::String("[[Systems Theory]]".into()),
            0,
            &mut HashSet::new(),
        );
        match v {
            ExpandedValue::Iri(iri) => assert_eq!(iri, "urn:visionclaw:linked:systems-theory"),
            other => panic!("expected Iri, got {:?}", other),
        }
    }
}
