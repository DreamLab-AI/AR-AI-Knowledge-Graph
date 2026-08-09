//! PROV-O provenance reification emitter (PRD-022 WS-2, ADR-127 D2).
//!
//! Reifies URN-based activity records as RDF triples in the append-only
//! `urn:ngm:graph:provenance` named graph. Each activity becomes a set of
//! quads using the W3C PROV-O vocabulary:
//!
//! ```turtle
//! <urn:visionclaw:execution:{sha256-12}> a prov:Activity ;
//!     prov:wasAssociatedWith <did:nostr:{hex-pubkey}> ;
//!     prov:startedAtTime "{iso-datetime}"^^xsd:dateTime ;
//!     prov:used <{source-iri}> ;
//!     prov:generated <{output-urn}> ;
//!     vc:action "{verb}" ;
//!     vc:derivation "{asserted|inferred|proposed}" .
//! ```
//!
//! The graph is append-only: only `INSERT DATA` is permitted. No
//! `DELETE`, `DROP`, or `CLEAR` operations are accepted.

use oxigraph::model::vocab::xsd;
use oxigraph::model::{GraphNameRef, Literal, NamedNode, NamedNodeRef, QuadRef};
use oxigraph::store::Store;

use super::oxigraph_ontology_repository::GRAPH_PROVENANCE;

const PROV_NS: &str = "http://www.w3.org/ns/prov#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const VC_NS: &str = "https://narrativegoldmine.com/ns/v1#";

/// A structured activity record ready for reification into PROV-O triples.
#[derive(Debug, Clone)]
pub struct ActivityRecord {
    /// The activity URN (e.g. `urn:visionclaw:execution:sha256-12-abc`).
    pub activity_urn: String,
    /// The agent DID (e.g. `did:nostr:deadbeef...`).
    pub agent_did: String,
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// The verb (e.g. "propose", "infer", "ingest", "enrich").
    pub action: String,
    /// The derivation scope: "asserted", "inferred", or "proposed".
    pub derivation: String,
    /// Optional: the source IRI that was used/consumed.
    pub used: Option<String>,
    /// Optional: the output URN that was generated.
    pub generated: Option<String>,
    /// Optional: prior decision URN for causal chaining.
    pub informed_by: Option<String>,
}

/// Errors from the provenance emitter.
#[derive(Debug)]
pub enum ProvenanceError {
    Store(String),
    InvalidIri(String),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::Store(e) => write!(f, "provenance store error: {e}"),
            ProvenanceError::InvalidIri(e) => write!(f, "invalid IRI in provenance record: {e}"),
        }
    }
}

impl std::error::Error for ProvenanceError {}

/// Reify an activity record as PROV-O triples in the provenance graph.
///
/// Returns the number of triples inserted (5–8 depending on optional fields).
pub fn reify_activity(store: &Store, record: &ActivityRecord) -> Result<usize, ProvenanceError> {
    let graph = NamedNodeRef::new_unchecked(GRAPH_PROVENANCE);
    let mut count = 0;

    let subject = make_named_node(&record.activity_urn)?;

    // rdf:type prov:Activity
    let prov_activity = NamedNode::new(format!("{PROV_NS}Activity"))
        .map_err(|e| ProvenanceError::InvalidIri(e.to_string()))?;
    let p_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    store
        .insert(QuadRef::new(&subject, p_type, &prov_activity, graph))
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    count += 1;

    // prov:wasAssociatedWith <agent_did>
    let agent = make_named_node(&record.agent_did)?;
    let p_associated = NamedNodeRef::new_unchecked("http://www.w3.org/ns/prov#wasAssociatedWith");
    store
        .insert(QuadRef::new(&subject, p_associated, &agent, graph))
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    count += 1;

    // prov:startedAtTime
    let p_started = NamedNodeRef::new_unchecked("http://www.w3.org/ns/prov#startedAtTime");
    let ts_lit = Literal::new_typed_literal(&record.timestamp, xsd::DATE_TIME);
    store
        .insert(QuadRef::new(&subject, p_started, &ts_lit, graph))
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    count += 1;

    // vc:action
    let p_action = make_named_node(&format!("{VC_NS}action"))?;
    let action_lit = Literal::new_simple_literal(&record.action);
    store
        .insert(QuadRef::new(
            &subject,
            p_action.as_ref(),
            &action_lit,
            graph,
        ))
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    count += 1;

    // vc:derivation
    let p_derivation = make_named_node(&format!("{VC_NS}derivation"))?;
    let deriv_lit = Literal::new_simple_literal(&record.derivation);
    store
        .insert(QuadRef::new(
            &subject,
            p_derivation.as_ref(),
            &deriv_lit,
            graph,
        ))
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    count += 1;

    // prov:used (optional)
    if let Some(ref used_urn) = record.used {
        let used_node = make_named_node(used_urn)?;
        let p_used = NamedNodeRef::new_unchecked("http://www.w3.org/ns/prov#used");
        store
            .insert(QuadRef::new(&subject, p_used, &used_node, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;
    }

    // prov:generated (optional)
    if let Some(ref gen_urn) = record.generated {
        let gen_node = make_named_node(gen_urn)?;
        let p_gen = NamedNodeRef::new_unchecked("http://www.w3.org/ns/prov#generated");
        store
            .insert(QuadRef::new(&subject, p_gen, &gen_node, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;
    }

    // prov:wasInformedBy (optional, causal chain)
    if let Some(ref prior_urn) = record.informed_by {
        let prior_node = make_named_node(prior_urn)?;
        let p_informed = NamedNodeRef::new_unchecked("http://www.w3.org/ns/prov#wasInformedBy");
        store
            .insert(QuadRef::new(&subject, p_informed, &prior_node, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;
    }

    tracing::debug!(
        activity = %record.activity_urn,
        agent = %record.agent_did,
        action = %record.action,
        triples = count,
        "reified PROV-O activity"
    );

    Ok(count)
}

/// Query the provenance graph for activities by a specific agent.
pub fn query_agent_activities(
    store: &Store,
    agent_did: &str,
    limit: usize,
) -> Result<Vec<ActivityRecord>, ProvenanceError> {
    let sparql = format!(
        r#"
        PREFIX prov: <http://www.w3.org/ns/prov#>
        PREFIX vc: <https://narrativegoldmine.com/ns/v1#>
        SELECT ?act ?agent ?time ?action ?derivation ?used ?generated ?prior
        FROM <{graph}>
        WHERE {{
            ?act a prov:Activity ;
                 prov:wasAssociatedWith <{agent}> ;
                 prov:startedAtTime ?time ;
                 vc:action ?action ;
                 vc:derivation ?derivation .
            OPTIONAL {{ ?act prov:used ?used }}
            OPTIONAL {{ ?act prov:generated ?generated }}
            OPTIONAL {{ ?act prov:wasInformedBy ?prior }}
        }}
        ORDER BY DESC(?time)
        LIMIT {limit}
        "#,
        graph = GRAPH_PROVENANCE,
        agent = agent_did,
        limit = limit,
    );

    let results = store
        .query(&sparql)
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;

    let mut records = Vec::new();
    if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let s = solution.map_err(|e| ProvenanceError::Store(e.to_string()))?;
            let activity_urn = term_to_string(s.get("act"));
            let agent_did_val = term_to_string(s.get("agent"));
            let timestamp = term_to_string(s.get("time"));
            let action = term_to_string(s.get("action"));
            let derivation = term_to_string(s.get("derivation"));

            records.push(ActivityRecord {
                activity_urn,
                agent_did: agent_did_val,
                timestamp,
                action,
                derivation,
                used: optional_term(s.get("used")),
                generated: optional_term(s.get("generated")),
                informed_by: optional_term(s.get("prior")),
            });
        }
    }
    Ok(records)
}

/// Count total provenance triples in the graph.
pub fn count_provenance_triples(store: &Store) -> Result<usize, ProvenanceError> {
    let sparql = format!(
        "SELECT (COUNT(*) as ?n) FROM <{}> WHERE {{ ?s ?p ?o }}",
        GRAPH_PROVENANCE,
    );
    let results = store
        .query(&sparql)
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    if let oxigraph::sparql::QueryResults::Solutions(mut solutions) = results {
        if let Some(Ok(row)) = solutions.next() {
            if let Some(oxigraph::model::Term::Literal(lit)) = row.get("n") {
                return lit
                    .value()
                    .parse::<usize>()
                    .map_err(|e| ProvenanceError::Store(e.to_string()));
            }
        }
    }
    Ok(0)
}

/// Count shapes loaded in the shapes graph.
pub fn count_shapes_loaded(store: &Store) -> Result<usize, ProvenanceError> {
    let sparql = format!(
        "SELECT (COUNT(DISTINCT ?shape) as ?n) FROM <{}> WHERE {{ ?shape a <http://www.w3.org/ns/shacl#NodeShape> }}",
        super::oxigraph_ontology_repository::GRAPH_SHAPES,
    );
    let results = store
        .query(&sparql)
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;
    if let oxigraph::sparql::QueryResults::Solutions(mut solutions) = results {
        if let Some(Ok(row)) = solutions.next() {
            if let Some(oxigraph::model::Term::Literal(lit)) = row.get("n") {
                return lit
                    .value()
                    .parse::<usize>()
                    .map_err(|e| ProvenanceError::Store(e.to_string()));
            }
        }
    }
    Ok(0)
}

fn make_named_node(iri: &str) -> Result<NamedNode, ProvenanceError> {
    NamedNode::new(iri).map_err(|e| ProvenanceError::InvalidIri(format!("{iri}: {e}")))
}

fn term_to_string(term: Option<&oxigraph::model::Term>) -> String {
    match term {
        Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
        Some(oxigraph::model::Term::Literal(l)) => l.value().to_string(),
        _ => String::new(),
    }
}

fn optional_term(term: Option<&oxigraph::model::Term>) -> Option<String> {
    match term {
        Some(oxigraph::model::Term::NamedNode(n)) => Some(n.as_str().to_string()),
        Some(oxigraph::model::Term::Literal(l)) if !l.value().is_empty() => {
            Some(l.value().to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_store() -> Store {
        Store::new().expect("in-memory store")
    }

    fn test_record() -> ActivityRecord {
        ActivityRecord {
            activity_urn: "urn:visionclaw:execution:sha256-12-abcdef012345".to_string(),
            agent_did: "did:nostr:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            timestamp: "2026-06-21T14:30:00Z".to_string(),
            action: "propose".to_string(),
            derivation: "asserted".to_string(),
            used: Some("urn:ngm:axiom:sha256-12-111222333444".to_string()),
            generated: Some("urn:visionclaw:bead:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:sha256-12-fed987654321".to_string()),
            informed_by: None,
        }
    }

    #[test]
    fn reifies_activity_with_all_fields() {
        let store = mem_store();
        let record = test_record();
        let count = reify_activity(&store, &record).expect("reify");
        assert_eq!(count, 7, "5 required + 2 optional (used + generated)");

        let graph = NamedNodeRef::new_unchecked(GRAPH_PROVENANCE);
        let total = store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(graph)))
            .count();
        assert_eq!(total, 7);
    }

    #[test]
    fn reifies_minimal_activity() {
        let store = mem_store();
        let record = ActivityRecord {
            activity_urn: "urn:visionclaw:execution:sha256-12-minimal000".to_string(),
            agent_did: "did:nostr:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
            timestamp: "2026-06-21T15:00:00Z".to_string(),
            action: "ingest".to_string(),
            derivation: "asserted".to_string(),
            used: None,
            generated: None,
            informed_by: None,
        };
        let count = reify_activity(&store, &record).expect("reify");
        assert_eq!(count, 5, "5 required fields only");
    }

    #[test]
    fn query_agent_activities_returns_records() {
        let store = mem_store();
        let record = test_record();
        reify_activity(&store, &record).expect("reify");

        let results = query_agent_activities(&store, &record.agent_did, 10).expect("query");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].activity_urn, record.activity_urn);
        assert_eq!(results[0].action, "propose");
        assert_eq!(results[0].derivation, "asserted");
        assert!(results[0].used.is_some());
        assert!(results[0].generated.is_some());
    }

    #[test]
    fn count_provenance_triples_works() {
        let store = mem_store();
        assert_eq!(count_provenance_triples(&store).unwrap(), 0);
        reify_activity(&store, &test_record()).unwrap();
        assert_eq!(count_provenance_triples(&store).unwrap(), 7);
    }

    #[test]
    fn append_only_verified() {
        let store = mem_store();
        reify_activity(&store, &test_record()).unwrap();

        let graph = NamedNodeRef::new_unchecked(GRAPH_PROVENANCE);
        let before = store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(graph)))
            .count();

        // Reify a second record — total grows, never shrinks.
        let record2 = ActivityRecord {
            activity_urn: "urn:visionclaw:execution:sha256-12-second000000".to_string(),
            agent_did: "did:nostr:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
            timestamp: "2026-06-21T16:00:00Z".to_string(),
            action: "enrich".to_string(),
            derivation: "proposed".to_string(),
            used: None,
            generated: None,
            informed_by: Some("urn:visionclaw:execution:sha256-12-abcdef012345".to_string()),
        };
        reify_activity(&store, &record2).unwrap();

        let after = store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(graph)))
            .count();
        assert!(after > before, "provenance graph must only grow");
    }
}
