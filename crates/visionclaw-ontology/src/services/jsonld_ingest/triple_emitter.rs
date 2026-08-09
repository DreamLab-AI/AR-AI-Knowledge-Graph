// src/services/jsonld_ingest/triple_emitter.rs
//! ExpandedNode → oxigraph::model::Quad emission.
//!
//! Implements the contract documented by
//! `tests/fixtures/data-model/seed/expected-triples.nq`:
//!
//! - `@type` array → one `rdf:type` triple per element.
//! - `OntologyClass` emits BOTH `owl:Class` AND `vc:OntologyClass`.
//! - `Page`/`LinkedPage`/`NostrSignedPage`/`BridgeRecord`/`AgentTelemetry`
//!   each emit a single `vc:<Type>` rdf:type.
//! - `Axiom` emits `owl:Axiom` (and only that — the dual `vc:Axiom` is NOT
//!   in the seed).
//! - `OntologyProperty` / `DataProperty` / `AnnotationProperty` emit the
//!   appropriate `owl:*Property` IRI plus optional `owl:FunctionalProperty`
//!   for functional cases (the validator pre-emits this when `vc:functional
//!   = true`; not in scope here).
//! - Booleans → typed `xsd:boolean` literals; integers → `xsd:integer`;
//!   floats → `xsd:float`; explicit `{@value, @type: xsd:dateTime}` → typed.
//! - Array values → one triple per element.
//! - `prov:generatedAtTime` is dropped from emission (the seed does not
//!   carry it). Provenance attribution survives.
//! - `vc:source: { @type: Asserted/Inferred, vc:definingPage: ..., vc:fromAxioms: [...] }`
//!   flattens to `vc:sourceKind "Asserted/Inferred"` + `vc:definingPage <iri>`
//!   / `vc:fromAxiom <iri>` (singular, one per array element).
//! - `vc:metrics: { ... }` flattens — child predicates emit on the parent.
//! - `vc:outboundWikilinks: [{ @id, vc:label }]` flattens to `vc:wikilink <iri>`.
//! - `@included` entries land in default graph unless they self-declare
//!   otherwise.
//!
//! ## Named-graph routing (ADR-D01 §D7)
//!
//! Routes by `@type`:
//!   - Page / LinkedPage / NostrSignedPage → `urn:ngm:graph:knowledge`
//!   - OntologyClass / OntologyProperty / Axiom / LinkResolved → `urn:ngm:graph:ontology:assert`
//!     EXCEPT when `vc:namedGraph` overrides to `:inferred`.
//!   - AgentTelemetry → `urn:ngm:graph:agent`
//!   - BridgeRecord → default graph (no named graph)
//!   - prov:Activity / prov:Agent (PROV-O nodes via @included) → default graph

use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Subject, Term};

use super::expander::{
    ExpandedDocument, ExpandedNode, ExpandedValue, OWL_NS, PROV_NS, SKOS_NS, VC_NS,
};

/// SKOS alignment predicates we capture+emit on nodes/axioms (ADR-100 D4).
/// We only *record* alignments the source ontology already provides
/// (typically to Wikidata / DBpedia) — we never invent them.
pub const SKOS_EXACT_MATCH: &str = "exactMatch";
pub const SKOS_CLOSE_MATCH: &str = "closeMatch";

pub const GRAPH_KNOWLEDGE: &str = "urn:ngm:graph:knowledge";
pub const GRAPH_ONTOLOGY_ASSERT: &str = "urn:ngm:graph:ontology:assert";
pub const GRAPH_ONTOLOGY_INFERRED: &str = "urn:ngm:graph:ontology:inferred";
pub const GRAPH_AGENT: &str = "urn:ngm:graph:agent";

/// Walk an `ExpandedDocument`, emitting one or more `Quad`s per node.
pub fn emit_quads(doc: &ExpandedDocument) -> Vec<Quad> {
    let mut out = Vec::new();
    for node in &doc.nodes {
        emit_node(node, &mut out);
    }
    out
}

fn emit_node(node: &ExpandedNode, out: &mut Vec<Quad>) {
    let iri = match &node.id {
        Some(s) => s.clone(),
        None => return,
    };
    let subject = match NamedNode::new(&iri) {
        Ok(n) => Subject::NamedNode(n),
        Err(_) => return,
    };

    let graph_iri = route_graph(node);
    let graph_name = match &graph_iri {
        Some(g) => match NamedNode::new(g) {
            Ok(n) => GraphName::NamedNode(n),
            Err(_) => return,
        },
        None => GraphName::DefaultGraph,
    };

    // 1. rdf:type triples — one per declared type, expanded per the
    //    seed contract (OntologyClass → owl:Class + vc:OntologyClass; etc).
    let type_iris = expand_types(&node.types);
    for t in &type_iris {
        out.push(Quad::new(
            subject.clone(),
            NamedNode::new_unchecked(format!("{}type", super::expander::RDF_NS)),
            Term::NamedNode(NamedNode::new_unchecked(t)),
            graph_name.clone(),
        ));
    }

    // 2. Predicate-value pairs.
    for (predicate, value) in &node.fields {
        emit_predicate(&subject, predicate, value, &graph_name, out);
    }
}

/// Route a node's emission to a named graph. ADR-D01 §D7.
fn route_graph(node: &ExpandedNode) -> Option<String> {
    // Explicit override wins.
    if let Some(g) = &node.explicit_graph {
        if g.is_empty() {
            return None;
        }
        return Some(g.clone());
    }

    for t in &node.types {
        if t == &format!("{}AgentTelemetry", VC_NS) {
            return Some(GRAPH_AGENT.to_string());
        }
        if t == &format!("{}BridgeRecord", VC_NS) {
            return None; // default graph
        }
        if t == &format!("{}Page", VC_NS)
            || t == &format!("{}LinkedPage", VC_NS)
            || t == &format!("{}NostrSignedPage", VC_NS)
        {
            return Some(GRAPH_KNOWLEDGE.to_string());
        }
        if t == &format!("{}Class", OWL_NS)
            || t == &format!("{}NamedIndividual", OWL_NS)
            || t == &format!("{}OntologyClass", VC_NS)
            || t == &format!("{}ObjectProperty", OWL_NS)
            || t == &format!("{}DatatypeProperty", OWL_NS)
            || t == &format!("{}AnnotationProperty", OWL_NS)
            || t == &format!("{}Axiom", OWL_NS)
            || t == &format!("{}LinkResolved", VC_NS)
        {
            return Some(GRAPH_ONTOLOGY_ASSERT.to_string());
        }
        if t.starts_with(PROV_NS) {
            // Activities / Agents from @included land in default graph
            // per the seed (lines 338-343).
            return None;
        }
    }
    // Unknown type — default graph is the safe fallback.
    None
}

/// Map JSON-LD friendly @type tokens to RDF-emission IRIs.
///
/// The seed N-Quads dictate the duplication: OntologyClass emits BOTH
/// `owl:Class` and `vc:OntologyClass`. Other types emit exactly one
/// rdf:type triple.
fn expand_types(types: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let owl_class = format!("{}Class", OWL_NS);
    let vc_ontology_class = format!("{}OntologyClass", VC_NS);

    let is_ontology_class = types
        .iter()
        .any(|t| t == &owl_class || t == &vc_ontology_class);
    if is_ontology_class {
        out.push(owl_class.clone());
        out.push(vc_ontology_class.clone());
    }

    for t in types {
        if t == &owl_class || t == &vc_ontology_class {
            continue; // already added above
        }
        // OntologyProperty was expanded to owl:ObjectProperty already.
        // Just push it (deduplicate at the end).
        if !out.contains(t) {
            out.push(t.clone());
        }
    }

    out
}

/// Emit zero or more quads for one (predicate, value) pair.
fn emit_predicate(
    subject: &Subject,
    predicate: &str,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    // Drop generatedAtTime per seed contract.
    if predicate == &format!("{}generatedAtTime", PROV_NS) {
        return;
    }
    // Drop our routing metadata.
    if predicate == &format!("{}namedGraph", VC_NS) {
        return;
    }
    // `vc:ontology: true` annotation flag — not a triple in the seed.
    if predicate == &format!("{}ontology", VC_NS) {
        return;
    }

    // ADR-100 D4 alignment hook: skos:exactMatch / skos:closeMatch capture.
    // Where the source ontology provides an alignment (e.g. to a Wikidata or
    // DBpedia entity), record it verbatim on the node/axiom as an object
    // property. We do NOT synthesise alignments — we only emit what the
    // source asserts. This is the single documented emission point for
    // cross-ontology alignment edges.
    if predicate == &format!("{}{}", SKOS_NS, SKOS_EXACT_MATCH)
        || predicate == &format!("{}{}", SKOS_NS, SKOS_CLOSE_MATCH)
    {
        emit_alignment(subject, predicate, value, graph_name, out);
        return;
    }

    // Predicate-specific flattening.
    if predicate == &format!("{}source", VC_NS) {
        emit_source_flattened(subject, value, graph_name, out);
        return;
    }
    if predicate == &format!("{}metrics", VC_NS) {
        emit_metrics_flattened(subject, value, graph_name, out);
        return;
    }
    if predicate == &format!("{}outboundWikilinks", VC_NS) {
        emit_outbound_wikilinks(subject, value, graph_name, out);
        return;
    }
    if predicate == &format!("{}tags", VC_NS) {
        emit_repeated_literal(subject, &format!("{}tag", VC_NS), value, graph_name, out);
        return;
    }
    if predicate == &format!("{}sourceDomain", VC_NS) {
        emit_repeated_literal(subject, predicate, value, graph_name, out);
        return;
    }

    // owl:propertyChainAxiom expects a list — but the seed flattens it
    // (one triple per list element). expand_value already exploded
    // @list → Multi, so a Multi here flattens naturally.
    emit_value_for_predicate(subject, predicate, value, graph_name, out);
}

/// Emit a value as zero or more quads on `(subject, predicate)`.
fn emit_value_for_predicate(
    subject: &Subject,
    predicate: &str,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    let pred = match NamedNode::new(predicate) {
        Ok(n) => n,
        Err(_) => return,
    };
    match value {
        ExpandedValue::Iri(iri) => {
            if NamedNode::new(iri).is_ok() {
                out.push(Quad::new(
                    subject.clone(),
                    pred,
                    Term::NamedNode(NamedNode::new_unchecked(iri)),
                    graph_name.clone(),
                ));
            }
        }
        ExpandedValue::Literal {
            value,
            datatype,
            language,
        } => {
            let lit = build_literal(value, datatype.as_deref(), language.as_deref());
            out.push(Quad::new(
                subject.clone(),
                pred,
                Term::Literal(lit),
                graph_name.clone(),
            ));
        }
        ExpandedValue::Multi(arr) => {
            for v in arr {
                emit_value_for_predicate(subject, predicate, v, graph_name, out);
            }
        }
        ExpandedValue::Nested(node) => {
            // Anonymous nested node: emit a reference to its @id if present,
            // then recursively emit the node's own triples in the same graph.
            if let Some(iri) = &node.id {
                if NamedNode::new(iri).is_ok() {
                    out.push(Quad::new(
                        subject.clone(),
                        pred,
                        Term::NamedNode(NamedNode::new_unchecked(iri)),
                        graph_name.clone(),
                    ));
                }
                let mut sub_out = Vec::new();
                emit_node(node, &mut sub_out);
                out.extend(sub_out);
            }
        }
        ExpandedValue::Null => {
            // No emission. (`vc:upgradedTo: null` is a placeholder for
            // dangling LinkedPages; the seed does not record it as a triple.)
        }
    }
}

/// Flatten `vc:source: { @type: Asserted, vc:definingPage: <iri>, ... }`.
/// Seed contract: emit `vc:sourceKind "Asserted"` + each inner predicate
/// promoted onto the parent. `vc:fromAxioms: [<iri>, ...]` → multiple
/// `vc:fromAxiom <iri>` triples (singular).
fn emit_source_flattened(
    subject: &Subject,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    let nested = match value {
        ExpandedValue::Nested(n) => n,
        _ => return,
    };

    // sourceKind from the nested node's @type values.
    let kind_iri = format!("{}sourceKind", VC_NS);
    let mut kind_value: Option<String> = None;
    for t in &nested.types {
        if t == "Asserted" || t == "Inferred" {
            kind_value = Some(t.clone());
        } else if let Some(local) = t.rsplit(':').next() {
            kind_value = Some(local.to_string());
        }
    }
    if let Some(k) = kind_value {
        out.push(Quad::new(
            subject.clone(),
            NamedNode::new_unchecked(kind_iri),
            Term::Literal(Literal::new_simple_literal(k)),
            graph_name.clone(),
        ));
    }

    for (p, v) in &nested.fields {
        if p == &format!("{}fromAxioms", VC_NS) {
            // Singularise: vc:fromAxioms → vc:fromAxiom
            let from_axiom = format!("{}fromAxiom", VC_NS);
            emit_value_for_predicate(subject, &from_axiom, v, graph_name, out);
        } else {
            emit_value_for_predicate(subject, p, v, graph_name, out);
        }
    }
}

/// Flatten `vc:metrics: { vc:elapsedMs: 23, vc:reasonerVersion: "...", ... }`
/// onto the parent. Per the seed, only `vc:elapsedMs` lands as a triple
/// for the agent-telemetry fixtures (other inner metric keys are not
/// asserted in the seed, so this is best-effort and uses the same
/// predicates).
fn emit_metrics_flattened(
    subject: &Subject,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    let nested = match value {
        ExpandedValue::Nested(n) => n,
        _ => return,
    };
    for (p, v) in &nested.fields {
        // Only surface predicates the seed records (elapsedMs). Other metric
        // keys remain in the JSON-LD source but do not produce triples in v1
        // to match the seed contract exactly.
        if p == &format!("{}elapsedMs", VC_NS) {
            emit_value_for_predicate(subject, p, v, graph_name, out);
        }
    }
}

/// `vc:outboundWikilinks: [{ @id, vc:label }]` → `vc:wikilink <iri>` per entry.
/// The `vc:label` on the nested object is discarded (LinkedPage record
/// carries its own `rdfs:label`).
fn emit_outbound_wikilinks(
    subject: &Subject,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    let wikilink_pred = format!("{}wikilink", VC_NS);
    let pred = NamedNode::new_unchecked(wikilink_pred.clone());

    let arr = match value {
        ExpandedValue::Multi(a) => a.clone(),
        single => vec![single.clone()],
    };
    for v in arr {
        match v {
            ExpandedValue::Iri(iri) => {
                if NamedNode::new(&iri).is_ok() {
                    out.push(Quad::new(
                        subject.clone(),
                        pred.clone(),
                        Term::NamedNode(NamedNode::new_unchecked(&iri)),
                        graph_name.clone(),
                    ));
                }
            }
            ExpandedValue::Nested(node) => {
                if let Some(iri) = &node.id {
                    if NamedNode::new(iri).is_ok() {
                        out.push(Quad::new(
                            subject.clone(),
                            pred.clone(),
                            Term::NamedNode(NamedNode::new_unchecked(iri)),
                            graph_name.clone(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Emit a SKOS alignment (`skos:exactMatch` / `skos:closeMatch`) as one
/// object-property triple per target IRI (ADR-100 D4). Targets are typically
/// `http://www.wikidata.org/entity/Q…` or `http://dbpedia.org/resource/…`.
/// Literal/blank values are ignored — an alignment must point at an IRI.
fn emit_alignment(
    subject: &Subject,
    predicate: &str,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    let pred = match NamedNode::new(predicate) {
        Ok(n) => n,
        Err(_) => return,
    };
    let arr = match value {
        ExpandedValue::Multi(a) => a.clone(),
        single => vec![single.clone()],
    };
    for v in arr {
        let iri = match v {
            ExpandedValue::Iri(iri) => Some(iri),
            ExpandedValue::Nested(node) => node.id.clone(),
            _ => None,
        };
        if let Some(iri) = iri {
            if NamedNode::new(&iri).is_ok() {
                out.push(Quad::new(
                    subject.clone(),
                    pred.clone(),
                    Term::NamedNode(NamedNode::new_unchecked(&iri)),
                    graph_name.clone(),
                ));
            }
        }
    }
}

fn emit_repeated_literal(
    subject: &Subject,
    predicate: &str,
    value: &ExpandedValue,
    graph_name: &GraphName,
    out: &mut Vec<Quad>,
) {
    let pred = match NamedNode::new(predicate) {
        Ok(n) => n,
        Err(_) => return,
    };
    let arr = match value {
        ExpandedValue::Multi(a) => a.clone(),
        single => vec![single.clone()],
    };
    for v in arr {
        match v {
            ExpandedValue::Literal {
                value,
                datatype,
                language,
            } => {
                let lit = build_literal(&value, datatype.as_deref(), language.as_deref());
                out.push(Quad::new(
                    subject.clone(),
                    pred.clone(),
                    Term::Literal(lit),
                    graph_name.clone(),
                ));
            }
            ExpandedValue::Iri(iri) => {
                if NamedNode::new(&iri).is_ok() {
                    out.push(Quad::new(
                        subject.clone(),
                        pred.clone(),
                        Term::NamedNode(NamedNode::new_unchecked(&iri)),
                        graph_name.clone(),
                    ));
                }
            }
            _ => {}
        }
    }
}

fn build_literal(value: &str, datatype: Option<&str>, language: Option<&str>) -> Literal {
    if let Some(lang) = language {
        if let Ok(lit) = Literal::new_language_tagged_literal(value, lang) {
            return lit;
        }
    }
    if let Some(dt) = datatype {
        if let Ok(dt_node) = NamedNode::new(dt) {
            return Literal::new_typed_literal(value, dt_node);
        }
    }
    Literal::new_simple_literal(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::jsonld_ingest::expander::expand_block;

    #[test]
    fn emits_page_quads() {
        let body = r#"{
            "@context": "https://narrativegoldmine.com/context/v1.jsonld",
            "@id": "urn:visionclaw:page:abc",
            "@type": "Page",
            "vc:slug": "minimal",
            "vc:public": true,
            "prov:wasAttributedTo": { "@id": "did:nostr:npub1abc" },
            "prov:generatedAtTime": { "@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime" }
        }"#;
        let doc = expand_block("test.md", 0, body).unwrap();
        let quads = emit_quads(&doc);
        assert!(quads.iter().any(
            |q| q.graph_name == GraphName::NamedNode(NamedNode::new_unchecked(GRAPH_KNOWLEDGE))
        ));
        // No generatedAtTime quad
        assert!(!quads
            .iter()
            .any(|q| q.predicate.as_str().contains("generatedAtTime")));
    }

    #[test]
    fn emits_skos_alignment_to_wikidata() {
        // ADR-100 D4: source-provided skos:exactMatch / skos:closeMatch are
        // captured+emitted verbatim (we record, never invent).
        let body = r#"{
            "@context": "https://narrativegoldmine.com/context/v1.jsonld",
            "@id": "urn:visionclaw:owl:class:cybernetics",
            "@type": "OntologyClass",
            "rdfs:subClassOf": { "@id": "urn:visionclaw:owl:class:built-environment" },
            "skos:exactMatch": { "@id": "http://www.wikidata.org/entity/Q193974" },
            "skos:closeMatch": [
                { "@id": "http://dbpedia.org/resource/Cybernetics" }
            ],
            "prov:wasAttributedTo": { "@id": "did:nostr:npub1abc" },
            "prov:generatedAtTime": { "@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime" }
        }"#;
        let doc = expand_block("align.md", 0, body).unwrap();
        let quads = emit_quads(&doc);

        let exact = format!("{}{}", SKOS_NS, SKOS_EXACT_MATCH);
        let close = format!("{}{}", SKOS_NS, SKOS_CLOSE_MATCH);
        assert!(
            quads
                .iter()
                .any(|q| q.predicate.as_str() == exact && q.object.to_string().contains("Q193974")),
            "exactMatch to Wikidata must be emitted"
        );
        assert!(
            quads.iter().any(|q| q.predicate.as_str() == close
                && q.object
                    .to_string()
                    .contains("dbpedia.org/resource/Cybernetics")),
            "closeMatch to DBpedia must be emitted"
        );
    }
}
