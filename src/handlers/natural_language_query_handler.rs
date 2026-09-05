//! Natural Language Query Handler
//!
//! REST API endpoints for translating natural language to read-only SPARQL
//! queries against the embedded Oxigraph store (ADR-2063).

use actix_web::{web, HttpResponse, Responder};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::services::natural_language_query_service::{
    NaturalLanguageQueryService, QueryPatterns, QueryTranslation as SparqlTranslation,
};

// Response macros
use crate::{error_json, ok_json};

/// Natural language query request
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NaturalLanguageQueryRequest {
    /// Natural language query
    pub query: String,
    /// Whether to return multiple suggestions
    #[serde(default)]
    pub suggest_alternatives: bool,
}

/// Query translation response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryTranslationResponse {
    /// Translated query/queries
    pub translations: Vec<SparqlTranslation>,
    /// Example queries for reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<ExampleQuery>>,
}

/// Example query
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleQuery {
    /// Natural language description
    pub description: String,
    /// SPARQL query
    pub sparql: String,
}

/// SPARQL explanation request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainSparqlRequest {
    /// SPARQL query to explain
    pub sparql: String,
}

/// SPARQL explanation response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainSparqlResponse {
    /// Original SPARQL query
    pub sparql: String,
    /// Natural language explanation
    pub explanation: String,
}

/// Translate natural language to SPARQL
/// POST /api/nl-query/translate
/// Translates a natural language query into one or more read-only SPARQL
/// queries (SELECT/ASK/CONSTRUCT/DESCRIBE) against the Oxigraph store.
/// Uses the current graph schema to generate contextually appropriate queries.
/// # Request Body
/// ```json
/// {
///   "query": "Show me all knowledge-graph nodes connected to Project X",
///   "suggestAlternatives": false
/// }
/// ```
/// # Response
/// ```json
/// {
///   "translations": [{
///     "originalQuery": "Show me all knowledge-graph nodes connected to Project X",
///     "sparqlQuery": "SELECT ?n WHERE { GRAPH <urn:ngm:graph:knowledge> { ?n a vc:KnowledgeNode } } LIMIT 50",
///     "explanation": "Finds all knowledge-graph nodes",
///     "confidence": 0.85,
///     "warnings": []
///   }]
/// }
/// ```
pub async fn translate_query(
    nl_service: web::Data<Arc<NaturalLanguageQueryService>>,
    request: web::Json<NaturalLanguageQueryRequest>,
) -> impl Responder {
    info!("Translating natural language query: {}", request.query);

    let result = if request.suggest_alternatives {
        // Get multiple suggestions
        nl_service.suggest_queries(&request.query).await
    } else {
        // Get single best translation
        nl_service
            .translate_to_sparql(&request.query)
            .await
            .map(|t| vec![t])
    };

    let result: Result<Vec<SparqlTranslation>, String> = result;
    match result {
        Ok(translations) => {
            let response = QueryTranslationResponse {
                translations,
                examples: None,
            };
            ok_json!(response)
        }
        Err(e) => {
            error_json!("Translation failed", e)
        }
    }
}

/// Get example queries
/// GET /api/nl-query/examples
/// Returns a list of example natural language queries and their SPARQL translations.
/// Useful for helping users understand what kinds of queries they can ask.
/// # Response
/// ```json
/// {
///   "examples": [
///     {
///       "description": "Show me all knowledge-graph nodes",
///       "sparql": "SELECT ?n WHERE { GRAPH <urn:ngm:graph:knowledge> { ?n a vc:KnowledgeNode } } LIMIT 50"
///     }
///   ]
/// }
/// ```
pub async fn get_examples() -> Result<HttpResponse, actix_web::Error> {
    debug!("Retrieving example queries");

    let examples: Vec<ExampleQuery> = QueryPatterns::examples()
        .into_iter()
        .map(|(desc, sparql)| ExampleQuery {
            description: desc.to_string(),
            sparql: sparql.to_string(),
        })
        .collect();

    ok_json!(serde_json::json!({ "examples": examples }))
}

/// Explain SPARQL query in natural language
/// POST /api/nl-query/explain
/// Takes a SPARQL query and generates a natural language explanation
/// of what it does.
/// # Request Body
/// ```json
/// {
///   "sparql": "SELECT ?n ?m WHERE { GRAPH <urn:ngm:graph:knowledge> { ?edge vc:source ?n ; vc:target ?m } } LIMIT 10"
/// }
/// ```
/// # Response
/// ```json
/// {
///   "sparql": "SELECT ?n ?m WHERE { GRAPH <urn:ngm:graph:knowledge> { ?edge vc:source ?n ; vc:target ?m } } LIMIT 10",
///   "explanation": "This query finds pairs of nodes connected by an edge..."
/// }
/// ```
pub async fn explain_sparql(
    nl_service: web::Data<Arc<NaturalLanguageQueryService>>,
    request: web::Json<ExplainSparqlRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Explaining SPARQL query");

    // Validate syntax first
    if let Err(e) = nl_service.validate_sparql(&request.sparql) {
        return error_json!("Invalid SPARQL syntax", e);
    }

    match nl_service.explain_sparql(&request.sparql).await {
        Ok(explanation) => {
            let response = ExplainSparqlResponse {
                sparql: request.sparql.clone(),
                explanation,
            };
            ok_json!(response)
        }
        Err(e) => {
            error_json!("Explanation failed", e)
        }
    }
}

/// Validate SPARQL syntax
/// POST /api/nl-query/validate
/// Validates that a SPARQL query is a permitted read-only form
/// (SELECT/ASK/CONSTRUCT/DESCRIBE); rejects SPARQL Update operations.
/// # Request Body
/// ```json
/// {
///   "sparql": "SELECT ?n WHERE { ?n a vc:KnowledgeNode } LIMIT 10"
/// }
/// ```
/// # Response
/// ```json
/// {
///   "valid": true,
///   "errors": []
/// }
/// ```
pub async fn validate_sparql(
    nl_service: web::Data<Arc<NaturalLanguageQueryService>>,
    request: web::Json<ExplainSparqlRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Validating SPARQL query");

    let validation_result: Result<(), String> = nl_service.validate_sparql(&request.sparql);
    match validation_result {
        Ok(()) => {
            ok_json!(serde_json::json!({
                "valid": true,
                "errors": []
            }))
        }
        Err(e) => {
            ok_json!(serde_json::json!({
                "valid": false,
                "errors": [e]
            }))
        }
    }
}

/// Configure natural language query routes
pub fn configure_nl_query_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/nl-query")
            .route("/translate", web::post().to(translate_query))
            .route("/examples", web::get().to(get_examples))
            .route("/explain", web::post().to(explain_sparql))
            .route("/validate", web::post().to(validate_sparql)),
    );
}
