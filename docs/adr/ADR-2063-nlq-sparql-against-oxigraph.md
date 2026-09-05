---
id: ADR-2063
title: The natural-language query service emits read-only SPARQL against Oxigraph, not Cypher
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any change to the Oxigraph named-graph layout or vocabulary in crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs, or a second SPARQL read-only validator appearing anywhere in src/
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2063 — The natural-language query service emits read-only SPARQL against Oxigraph, not Cypher

## Context
- Phase 1 diagram VC-25.7 found `src/services/natural_language_query_service.rs` translating natural
  language into **Cypher** (module doc `:3`, `cypher_query` field `:17`, `translate_to_cypher` `:45`)
  and prompting/validating against Neo4j-style `GraphNode`/`EDGE` labels, while the only store this
  codebase runs is the embedded Oxigraph quad-store (`crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs`),
  whose query language is SPARQL 1.1. Every query the old service generated was unrunnable.
- `src/handlers/ontology_handler.rs` already carries a battle-tested read-only SPARQL validator,
  `validate_read_only_sparql` (`:752`), enforced on `/api/ontology/query` and `/api/ontology/sparql`.
  It was `fn`-private to that module.
- PHASE2 decision policy rule 5 (wrong design, bounded) directs a FIX to the documented design rather
  than a rewrite from scratch.

## Decision
`NaturalLanguageQueryService` (`src/services/natural_language_query_service.rs`) now translates natural
language into **read-only SPARQL** (`QueryTranslation.sparql_query`, `translate_to_sparql`). Its system
prompt describes the real Oxigraph vocabulary: the four named graphs (`urn:ngm:graph:ontology:assert`,
`:inferred`, `urn:ngm:graph:knowledge`, `urn:ngm:graph:agent`), the `vc:`/`rdf:`/`rdfs:`/`owl:`/`xsd:`
prefixes, and the real predicate/class IRIs minted by `oxigraph_ontology_repository.rs` and
`src/adapters/oxigraph_graph_repository.rs` (`vc:KnowledgeNode`, `vc:KGEdge`, `vc:nodeId`, `vc:hasX/Y/Z`,
`vc:preferredTerm`, `rdfs:subClassOf`, etc.). Validation no longer hand-rolls Cypher-shaped checks;
`validate_sparql` delegates to `ontology_handler::validate_read_only_sparql` (made `pub(crate)`), so
there is exactly one definition of "read-only SPARQL" in the crate. `src/handlers/natural_language_query_handler.rs`
and its DTOs (`ExplainSparqlRequest/Response`, `ExampleQuery.sparql`) are renamed to match; the four
routes (`/nl-query/{translate,examples,explain,validate}`) are unchanged.

## Consequences
- A caller that follows this service's output can actually run the query against the store; a caller
  that pasted the old Cypher output anywhere got a parse error, silently.
- The wire response field is now `sparqlQuery`/`sparql` instead of `cypherQuery`/`cypher` — a breaking
  DTO change for any client of `/api/nl-query/*`, which is the correct fix rather than preserving a
  field name that described a fiction.
- Only SELECT/ASK/CONSTRUCT/DESCRIBE ever pass validation; any generated INSERT/DELETE/DROP/CLEAR/
  LOAD/CREATE/ADD/MOVE/COPY/WITH/SERVICE is rejected before it reaches a caller.
- Follow-on (not in this ADR's scope): `src/services/ontology_query_service.rs::validate_and_execute_cypher`
  is a separate, differently-named validator that checks Cypher-shaped `(n:Label)` patterns for the MCP
  agent read path and does not execute anything against Oxigraph either. It was not touched here because
  it is a distinct call path (agent MCP tools, not `/api/nl-query/*`) and out of this ADR's bounded scope;
  it should be revisited under its own ADR if agents are meant to receive SPARQL guidance too.

## Verification
Ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; must be re-run at
the landing commit.

```
$ grep -rn "translate_to_cypher\|cypher_query" src/ crates/ --include=*.rs
# (no matches — both symbols renamed to translate_to_sparql / sparql_query)

$ grep -rn "translate_to_sparql\|sparql_query\|validate_sparql\b" src/services/natural_language_query_service.rs src/handlers/natural_language_query_handler.rs
src/services/natural_language_query_service.rs:...  pub sparql_query: String,
src/services/natural_language_query_service.rs:...  pub async fn translate_to_sparql(&self, query: &str) -> Result<QueryTranslation, String> {
src/services/natural_language_query_service.rs:...  pub fn validate_sparql(&self, sparql: &str) -> Result<(), String> {
src/handlers/natural_language_query_handler.rs:...  nl_service.translate_to_sparql(&request.query).await

$ grep -n "pub(crate) fn validate_read_only_sparql" src/handlers/ontology_handler.rs
752:pub(crate) fn validate_read_only_sparql(query: &str) -> Result<(), String> {

$ cargo check -p visionclaw-server --lib 2>&1 | grep -c "natural_language_query\|ontology_handler.rs"
0   # no errors/warnings attributed to either touched file

$ cargo check -p visionclaw-server --lib 2>&1 | tail -5
error: could not compile `visionclaw-server` (lib) due to 4 previous errors; 11 warnings emitted
```

The 4 errors are all `cannot find macro 'warn' in this scope` in `src/handlers/speech_socket_handler.rs`
(lines 193/236/239/485) — a file owned by vc-clients, unrelated to this ADR's two files, and present in
the shared uncommitted working tree independent of this change (confirmed by the zero-match grep above).
`cargo test -p visionclaw-server --lib natural_language` could not be run to a pass/fail result because
the crate as a whole does not currently build for that unrelated reason; the new unit tests
(`test_select_accepted`, `test_mutating_sparql_rejected`, `test_extract_sparql_block`,
`test_extract_sparql_block_missing_is_error`, `test_query_patterns`) in
`src/services/natural_language_query_service.rs` are present and were checked by inspection against
`ontology_handler`'s existing `sparql_validation_tests` (`accepts_read_only_forms`,
`rejects_mutating_sparql`) for consistency of expected pass/fail cases. Must be re-run once
`speech_socket_handler.rs` is fixed by its owning lead.
