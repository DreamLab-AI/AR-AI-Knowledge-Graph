//! PROV-O provenance reification emitter (PRD-022 WS-2, ADR-127 D2).
//!
//! Reifies URN-based activity records as RDF triples in the append-only
//! `urn:ngm:graph:provenance` named graph. This is the single store-writing
//! provenance primitive: every production path that mutates the ontology, runs
//! inference, or records a decision reifies its event here through
//! [`reify_activity`] (see `OxigraphOntologyRepository::emit_provenance`, the
//! one wiring seam). Each event becomes a set of quads using the W3C PROV-O
//! vocabulary — the full `prov:Entity` / `prov:Activity` / `prov:Agent` triad:
//!
//! ```turtle
//! <urn:visionclaw:execution:{sha256-12}> a prov:Activity ;
//!     prov:wasAssociatedWith <did:nostr:{hex-pubkey}> ;
//!     prov:startedAtTime "{iso-datetime}"^^xsd:dateTime ;
//!     prov:used <{source-iri}> ;
//!     vc:action "{verb}" ;
//!     vc:derivation "{asserted|inferred|proposed}" .
//! <did:nostr:{hex-pubkey}> a prov:Agent .
//! <{output-urn}> a prov:Entity ;
//!     prov:wasGeneratedBy <urn:visionclaw:execution:{sha256-12}> ;
//!     prov:wasAttributedTo <did:nostr:{hex-pubkey}> ;
//!     prov:generatedAtTime "{iso-datetime}"^^xsd:dateTime ;
//!     prov:wasDerivedFrom <{source-iri}> .
//! ```
//!
//! The URN scheme is preserved verbatim: activity/agent/entity/source URNs
//! minted elsewhere (`src/uri`, `provenance_writer`, the ingest/inference/
//! decision paths) become the RDF subjects and objects unchanged, so the
//! reified graph and the URN records agree — the RDF is a queryable projection
//! of the same identifiers, never a fork.
//!
//! The graph is append-only: only `INSERT DATA` is permitted. No
//! `DELETE`, `DROP`, or `CLEAR` operations are accepted.

use std::sync::Arc;

use oxigraph::model::vocab::xsd;
use oxigraph::model::{GraphNameRef, Literal, NamedNode, NamedNodeRef, QuadRef};
use oxigraph::store::Store;

use super::oxigraph_ontology_repository::GRAPH_PROVENANCE;

const PROV_NS: &str = "http://www.w3.org/ns/prov#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const VC_NS: &str = "https://narrativegoldmine.com/ns/v1#";

// Full PROV-O predicate/class IRIs used by the Entity/Activity/Agent triad.
const PROV_AGENT: &str = "http://www.w3.org/ns/prov#Agent";
const PROV_ENTITY: &str = "http://www.w3.org/ns/prov#Entity";
const PROV_WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
const PROV_WAS_ATTRIBUTED_TO: &str = "http://www.w3.org/ns/prov#wasAttributedTo";
const PROV_GENERATED_AT_TIME: &str = "http://www.w3.org/ns/prov#generatedAtTime";
const PROV_WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";

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

    // <agent> a prov:Agent — type the acting principal so the Entity/Activity/
    // Agent triad is complete and agent-scoped SPARQL (`?a a prov:Agent`) works.
    let prov_agent = NamedNodeRef::new_unchecked(PROV_AGENT);
    let p_type_agent = NamedNodeRef::new_unchecked(RDF_TYPE);
    store
        .insert(QuadRef::new(&agent, p_type_agent, prov_agent, graph))
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

    // prov:used <source> (optional) — the activity consumed this source.
    let used_node = match record.used {
        Some(ref used_urn) => {
            let node = make_named_node(used_urn)?;
            let p_used = NamedNodeRef::new_unchecked("http://www.w3.org/ns/prov#used");
            store
                .insert(QuadRef::new(&subject, p_used, &node, graph))
                .map_err(|e| ProvenanceError::Store(e.to_string()))?;
            count += 1;
            Some(node)
        }
        None => None,
    };

    // Generated entity block (optional): the output URN is a first-class
    // `prov:Entity`, attributed to the agent and generated by this activity.
    // This is the queryable end of the wasGeneratedBy/wasDerivedFrom chain.
    if let Some(ref gen_urn) = record.generated {
        let gen_node = make_named_node(gen_urn)?;

        // <generated> a prov:Entity
        let prov_entity = NamedNodeRef::new_unchecked(PROV_ENTITY);
        let p_type = NamedNodeRef::new_unchecked(RDF_TYPE);
        store
            .insert(QuadRef::new(&gen_node, p_type, prov_entity, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;

        // <generated> prov:wasGeneratedBy <activity>
        let p_gen_by = NamedNodeRef::new_unchecked(PROV_WAS_GENERATED_BY);
        store
            .insert(QuadRef::new(&gen_node, p_gen_by, &subject, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;

        // <generated> prov:wasAttributedTo <agent>
        let p_attr = NamedNodeRef::new_unchecked(PROV_WAS_ATTRIBUTED_TO);
        store
            .insert(QuadRef::new(&gen_node, p_attr, &agent, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;

        // <generated> prov:generatedAtTime "<ts>"^^xsd:dateTime
        let p_gen_at = NamedNodeRef::new_unchecked(PROV_GENERATED_AT_TIME);
        let gen_ts = Literal::new_typed_literal(&record.timestamp, xsd::DATE_TIME);
        store
            .insert(QuadRef::new(&gen_node, p_gen_at, &gen_ts, graph))
            .map_err(|e| ProvenanceError::Store(e.to_string()))?;
        count += 1;

        // <generated> prov:wasDerivedFrom <used> — the derivation edge the
        // entity-chain query walks (only when a concrete source was consumed).
        if let Some(ref used) = used_node {
            let p_derived = NamedNodeRef::new_unchecked(PROV_WAS_DERIVED_FROM);
            store
                .insert(QuadRef::new(&gen_node, p_derived, used, graph))
                .map_err(|e| ProvenanceError::Store(e.to_string()))?;
            count += 1;
        }
    }

    // prov:wasInformedBy (optional, activity→activity causal chain)
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
            OPTIONAL {{ ?generated prov:wasGeneratedBy ?act }}
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

/// Async wrapper over [`reify_activity`]: runs the synchronous store insert on
/// the blocking pool. This is the single emission entry point production
/// services (mutation, inference, decision) call so the `spawn_blocking` +
/// error mapping live in one place. Returns the number of triples written.
pub async fn emit_activity(
    store: Arc<Store>,
    record: ActivityRecord,
) -> Result<usize, ProvenanceError> {
    tokio::task::spawn_blocking(move || reify_activity(&store, &record))
        .await
        .map_err(|e| ProvenanceError::Store(format!("provenance join error: {e}")))?
}

/// Fire-and-forget emission: emit the activity and log the outcome, never
/// propagating a failure. Provenance is an append-only audit side-effect over
/// an already-committed action — a provenance failure must never fail the
/// caller's business operation (matches the inference path's contract).
pub async fn emit_activity_nonfatal(store: Arc<Store>, record: ActivityRecord) {
    let activity = record.activity_urn.clone();
    match emit_activity(store, record).await {
        Ok(n) => tracing::debug!(activity = %activity, triples = n, "reified PROV-O activity"),
        Err(e) => tracing::warn!(activity = %activity, "provenance emit failed (non-fatal): {e}"),
    }
}

/// Guard for an entity IRI interpolated as `<iri>` in a provenance query. The
/// value arrives from a request query-param, so reject anything that could
/// break out of the `<...>` token (mirrors the repository's `iri_is_safe`).
fn entity_iri_is_safe(s: &str) -> bool {
    (s.starts_with("urn:") || s.starts_with("http://") || s.starts_with("https://"))
        && !s.chars().any(|c| {
            c.is_whitespace()
                || matches!(c, '<' | '>' | '"' | '\\' | '{' | '}' | '`' | '|' | '^')
                || c.is_control()
        })
}

/// One node in a provenance chain: an entity, the activity that generated it,
/// the agent it was attributed to, when, and its direct `prov:wasDerivedFrom`
/// parents (the next hop the walk followed).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProvenanceNode {
    pub entity: String,
    /// `rdf:subject` of a portable-reification assertion-version (the reified
    /// statement's subject), when this node is an assertion version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(rename = "wasGeneratedBy", skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(rename = "wasAttributedTo", skip_serializing_if = "Option::is_none")]
    pub attributed_to: Option<String>,
    #[serde(rename = "generatedAtTime", skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
    #[serde(rename = "wasDerivedFrom")]
    pub derived_from: Vec<String>,
}

/// The provenance chain for one entity: the query root plus every reachable
/// node found by walking `prov:wasGeneratedBy` and `prov:wasDerivedFrom`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProvenanceChain {
    pub root: String,
    pub nodes: Vec<ProvenanceNode>,
}

/// Fetch the provenance metadata + direct derivation parents for a single
/// entity IRI. Returns `None` for the node when the entity carries no
/// provenance at all (no generatedBy / attributedTo / rdf:subject).
fn fetch_provenance_node(store: &Store, entity: &str) -> Result<ProvenanceNode, ProvenanceError> {
    let sparql = format!(
        r#"
        PREFIX prov: <http://www.w3.org/ns/prov#>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
        PREFIX vc: <{VC_NS}>
        SELECT ?subject ?activity ?agent ?generatedAt ?action ?derivation ?parent
        FROM <{graph}>
        WHERE {{
            OPTIONAL {{ <{entity}> rdf:subject ?subject }}
            OPTIONAL {{ <{entity}> prov:wasGeneratedBy ?activity .
                        OPTIONAL {{ ?activity vc:action ?action }}
                        OPTIONAL {{ ?activity vc:derivation ?derivation }}
                        OPTIONAL {{ ?activity prov:wasAssociatedWith ?actAgent }} }}
            OPTIONAL {{ <{entity}> prov:wasAttributedTo ?entAgent }}
            OPTIONAL {{ <{entity}> prov:generatedAtTime ?generatedAt }}
            OPTIONAL {{ <{entity}> prov:wasDerivedFrom ?parent }}
            BIND(COALESCE(?entAgent, ?actAgent) AS ?agent)
        }}
        "#,
        graph = GRAPH_PROVENANCE,
        entity = entity,
    );

    let results = store
        .query(&sparql)
        .map_err(|e| ProvenanceError::Store(e.to_string()))?;

    let mut node = ProvenanceNode {
        entity: entity.to_string(),
        subject: None,
        generated_by: None,
        attributed_to: None,
        generated_at: None,
        action: None,
        derivation: None,
        derived_from: Vec::new(),
    };

    if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let s = solution.map_err(|e| ProvenanceError::Store(e.to_string()))?;
            node.subject
                .get_or_insert_with(|| term_to_string(s.get("subject")));
            node.generated_by
                .get_or_insert_with(|| term_to_string(s.get("activity")));
            node.attributed_to
                .get_or_insert_with(|| term_to_string(s.get("agent")));
            node.generated_at
                .get_or_insert_with(|| term_to_string(s.get("generatedAt")));
            node.action
                .get_or_insert_with(|| term_to_string(s.get("action")));
            node.derivation
                .get_or_insert_with(|| term_to_string(s.get("derivation")));
            if let Some(parent) = optional_term(s.get("parent")) {
                if !node.derived_from.contains(&parent) {
                    node.derived_from.push(parent);
                }
            }
        }
    }

    // Collapse empty strings (from the `get_or_insert_with` seeding on absent
    // bindings) back to `None` so the serialised node omits them.
    normalise_empty(&mut node.subject);
    normalise_empty(&mut node.generated_by);
    normalise_empty(&mut node.attributed_to);
    normalise_empty(&mut node.generated_at);
    normalise_empty(&mut node.action);
    normalise_empty(&mut node.derivation);

    Ok(node)
}

fn normalise_empty(field: &mut Option<String>) {
    if matches!(field.as_deref(), Some("")) {
        *field = None;
    }
}

/// Walk the provenance chain for `entity_urn`: seed with the entity itself plus
/// any assertion-version entities whose `rdf:subject` is the entity (so a
/// governed statement subject — e.g. a decision or class URN — resolves to its
/// reified provenance), then follow `prov:wasDerivedFrom` breadth-first up to
/// `max_depth` hops (bounded by an absolute node cap so a cyclic graph cannot
/// loop). Answers "provenance for entity X" — the acceptance query.
pub fn provenance_for_entity(
    store: &Store,
    entity_urn: &str,
    max_depth: usize,
) -> Result<ProvenanceChain, ProvenanceError> {
    if !entity_iri_is_safe(entity_urn) {
        return Err(ProvenanceError::InvalidIri(format!(
            "unsafe entity IRI: {entity_urn}"
        )));
    }
    const MAX_NODES: usize = 256;

    // Seed frontier: the entity itself, plus assertion-versions reifying it.
    let mut frontier: Vec<String> = vec![entity_urn.to_string()];
    let subject_seed = format!(
        r#"
        PREFIX prov: <http://www.w3.org/ns/prov#>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
        SELECT ?e FROM <{graph}> WHERE {{ ?e rdf:subject <{entity}> ; a prov:Entity }}
        "#,
        graph = GRAPH_PROVENANCE,
        entity = entity_urn,
    );
    if let oxigraph::sparql::QueryResults::Solutions(sols) = store
        .query(&subject_seed)
        .map_err(|e| ProvenanceError::Store(e.to_string()))?
    {
        for sol in sols {
            let sol = sol.map_err(|e| ProvenanceError::Store(e.to_string()))?;
            if let Some(e) = optional_term(sol.get("e")) {
                if !frontier.contains(&e) {
                    frontier.push(e);
                }
            }
        }
    }

    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut nodes: Vec<ProvenanceNode> = Vec::new();
    let mut depth = 0usize;

    while !frontier.is_empty() && depth <= max_depth && nodes.len() < MAX_NODES {
        let mut next: Vec<String> = Vec::new();
        for entity in frontier.drain(..) {
            if !visited.insert(entity.clone()) {
                continue;
            }
            if !entity_iri_is_safe(&entity) {
                continue; // never interpolate an unsafe parent IRI
            }
            let node = fetch_provenance_node(store, &entity)?;
            for parent in &node.derived_from {
                if !visited.contains(parent) {
                    next.push(parent.clone());
                }
            }
            // Only record nodes that actually carry provenance (skip a bare
            // seed IRI that has no reified record — it is just the query root).
            let has_prov = node.generated_by.is_some()
                || node.attributed_to.is_some()
                || node.subject.is_some()
                || !node.derived_from.is_empty();
            if has_prov {
                nodes.push(node);
            }
            if nodes.len() >= MAX_NODES {
                break;
            }
        }
        frontier = next;
        depth += 1;
    }

    Ok(ProvenanceChain {
        root: entity_urn.to_string(),
        nodes,
    })
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
        // 6 activity/agent base (Activity, wasAssociatedWith, startedAtTime,
        // Agent typing, vc:action, vc:derivation) + prov:used + 4 entity block
        // (Entity, wasGeneratedBy, wasAttributedTo, generatedAtTime) +
        // wasDerivedFrom = 12.
        assert_eq!(count, 12, "6 base + used + 4 entity + wasDerivedFrom");

        let graph = NamedNodeRef::new_unchecked(GRAPH_PROVENANCE);
        let total = store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(graph)))
            .count();
        assert_eq!(total, 12);
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
        assert_eq!(count, 6, "6 activity/agent base fields only");
    }

    /// Acceptance: every emission carries the full PROV-O triad — prov:Entity,
    /// prov:Activity, prov:Agent, wasGeneratedBy, wasAttributedTo,
    /// wasDerivedFrom and generatedAtTime — over the URN identifiers verbatim.
    #[test]
    fn emits_full_prov_o_triad() {
        let store = mem_store();
        let record = test_record();
        reify_activity(&store, &record).expect("reify");

        let ask = |q: &str| -> bool {
            matches!(
                store.query(q),
                Ok(oxigraph::sparql::QueryResults::Boolean(true))
            )
        };
        let g = GRAPH_PROVENANCE;
        let act = &record.activity_urn;
        let agent = &record.agent_did;
        let gen = record.generated.as_ref().unwrap();
        let used = record.used.as_ref().unwrap();
        let p = "http://www.w3.org/ns/prov#";
        let rdft = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{act}> <{rdft}> <{p}Activity> }} }}"
        )));
        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{agent}> <{rdft}> <{p}Agent> }} }}"
        )));
        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{gen}> <{rdft}> <{p}Entity> }} }}"
        )));
        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{gen}> <{p}wasGeneratedBy> <{act}> }} }}"
        )));
        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{gen}> <{p}wasAttributedTo> <{agent}> }} }}"
        )));
        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{gen}> <{p}wasDerivedFrom> <{used}> }} }}"
        )));
        assert!(ask(&format!(
            "ASK {{ GRAPH <{g}> {{ <{gen}> <{p}generatedAtTime> ?t }} }}"
        )));
    }

    #[test]
    fn provenance_for_entity_walks_generated_and_derivation() {
        let store = mem_store();
        let record = test_record();
        reify_activity(&store, &record).expect("reify");

        // Query by the generated entity URN — the chain surfaces its activity,
        // agent and the wasDerivedFrom source.
        let gen = record.generated.as_ref().unwrap();
        let chain = provenance_for_entity(&store, gen, 8).expect("chain");
        assert_eq!(chain.root, *gen);
        let node = chain
            .nodes
            .iter()
            .find(|n| &n.entity == gen)
            .expect("generated entity node present");
        assert_eq!(
            node.generated_by.as_deref(),
            Some(record.activity_urn.as_str())
        );
        assert_eq!(
            node.attributed_to.as_deref(),
            Some(record.agent_did.as_str())
        );
        assert!(node.generated_at.is_some());
        assert!(node.derived_from.contains(record.used.as_ref().unwrap()));
    }

    #[test]
    fn provenance_for_entity_rejects_unsafe_iri() {
        let store = mem_store();
        let err = provenance_for_entity(&store, "not an iri > injection", 4).unwrap_err();
        assert!(matches!(err, ProvenanceError::InvalidIri(_)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn emit_activity_async_writes_and_is_nonfatal() {
        let store = Arc::new(mem_store());
        let n = emit_activity(Arc::clone(&store), test_record())
            .await
            .expect("emit");
        assert_eq!(n, 12);
        // Non-fatal path never panics/propagates even for a duplicate write.
        emit_activity_nonfatal(Arc::clone(&store), test_record()).await;
        assert!(count_provenance_triples(&store).unwrap() >= 12);
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
        assert_eq!(count_provenance_triples(&store).unwrap(), 12);
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
