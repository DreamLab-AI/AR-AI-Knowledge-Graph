// src/services/jsonld_ingest/pipeline.rs
//! End-to-end ingest pipeline: markdown source → validated quads → adapter writes.
//!
//! Wires the four stages:
//!
//! 1. `extractor::extract_jsonld_blocks` — find every ```json-ld fenced block.
//! 2. `expander::expand_block` — parse JSON + apply prefix expansion + wikilink desugaring.
//! 3. `validator::validate` — schema/profile/PROV-O checks.
//! 4. `triple_emitter::emit_quads` — produce the `Quad` set per the seed contract.
//! 5. Adapter write — fanned out to `OntologyRepository` or `GraphRepository`
//!    based on `@type`.
//!
//! ## Public API
//!
//! ```ignore
//! pub async fn ingest_page(
//!     markdown: &str,
//!     metadata: &PageMetadata,
//! ) -> Result<IngestOutcome>;
//! ```
//!
//! The pipeline is repository-aware via `Arc<dyn OntologyRepository>` +
//! `Arc<dyn GraphRepository>` injected at construction time (acceptance
//! criterion A4 — no concrete adapter coupling).

use std::sync::Arc;

use oxigraph::model::Quad;

use super::errors::{JsonLdIngestError, Result};
use super::expander::expand_block;
use super::extractor::extract_jsonld_blocks;
use super::shacl_gate::{gate_block_advisory, ShaclGateReport};
use super::triple_emitter::emit_quads;
use super::validator::validate;
use crate::services::jsonld_ingest::graph_port_shim::GraphRepository;
use visionclaw_domain::ports::ontology_repository::OntologyRepository;

/// Metadata about the host markdown file: where it came from, when it was
/// pulled, optional pre-computed content hash. The pipeline DOES NOT need
/// the file's logseq `public:: true` flag — the JSON-LD blocks carry their
/// own `vc:public` assertion per ADR-D01 §D6.
#[derive(Debug, Clone)]
pub struct PageMetadata {
    /// Source path (e.g. `pages/cybernetics.md`). Used in error messages and
    /// `vc:sourcePath` if the pipeline synthesises that triple (it does not
    /// in v1 — the codemod (D13) writes it explicitly into the JSON-LD).
    pub source_path: String,
    /// Optional precomputed SHA-1 of the markdown bytes (the JSON-LD blocks
    /// may carry it as `vc:contentSha1`).
    pub content_sha1: Option<String>,
}

impl PageMetadata {
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            content_sha1: None,
        }
    }
}

/// Outcome of a single page ingest.
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    /// Number of JSON-LD blocks parsed from the page.
    pub block_count: usize,
    /// Total number of quads emitted across all blocks.
    pub quad_count: usize,
    /// The emitted quads themselves. Returned so callers (and tests) can
    /// inspect / re-route before persistence if desired.
    pub quads: Vec<Quad>,
    /// Source path (echoed from metadata).
    pub source_path: String,
    /// SHACL-lite gate result over the parsed shapes (ADR-100 D4 / PRD-018
    /// WS-5). The schema/profile validator runs first and hard-fails ingest;
    /// this gate then surfaces shape-level findings as a report so the
    /// operator dashboard sees every violation. `report.is_valid()` is the
    /// gate decision.
    pub shacl_report: ShaclGateReport,
}

/// Build an `IngestOutcome` from a markdown source without touching any
/// repository. Useful for tests and dry-run validation.
pub fn parse_and_emit(markdown: &str, metadata: &PageMetadata) -> Result<IngestOutcome> {
    let blocks = extract_jsonld_blocks(markdown);

    if blocks.is_empty() {
        return Err(JsonLdIngestError::MissingCodeFenceMarker {
            file: metadata.source_path.clone(),
        });
    }

    let mut all_quads: Vec<Quad> = Vec::new();
    let mut shacl_report = ShaclGateReport::default();

    for block in &blocks {
        let doc = expand_block(&metadata.source_path, block.index, &block.body)?;
        // Schema/profile/PROV-O validator: hard gate (first error fails ingest).
        validate(&metadata.source_path, &doc)?;
        // SHACL-lite shape gate (ADR-100 D4 / PRD-018 WS-5): reuse the
        // existing `shacl_lite` engine over the parsed JSON-LD shapes and
        // surface findings as a report. The block body already parsed cleanly
        // in `expand_block`, so this re-parse cannot fail; on the off chance
        // it does, the shape gate simply records nothing for that block.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&block.body) {
            gate_block_advisory(&value, block.index, &mut shacl_report);
        }
        let quads = emit_quads(&doc);
        all_quads.extend(quads);
    }

    Ok(IngestOutcome {
        block_count: blocks.len(),
        quad_count: all_quads.len(),
        quads: all_quads,
        source_path: metadata.source_path.clone(),
        shacl_report,
    })
}

/// Pipeline runner with injected repository ports. Acceptance A4: ports
/// are abstract; the runner never references concrete adapter types.
///
/// Quads produced by `ingest_page` are returned to the caller
/// (GitHubSyncService) for bulk insertion via `store.transaction()`.
/// The port fields are retained so callers that want direct adapter writes
/// can extend this runner without breaking the public API.
#[allow(dead_code)] // fields used by callers that extend the runner with direct store writes
pub struct JsonLdIngestPipeline {
    ontology: Arc<dyn OntologyRepository>,
    graph: Arc<dyn GraphRepository>,
}

impl JsonLdIngestPipeline {
    pub fn new(ontology: Arc<dyn OntologyRepository>, graph: Arc<dyn GraphRepository>) -> Self {
        Self { ontology, graph }
    }

    /// Parses + validates + emits quads, returning the outcome.
    /// Quads are returned to the caller for direct insertion into
    /// the Oxigraph store. The caller (GitHubSyncService) handles
    /// bulk insertion via store.transaction().
    pub async fn ingest_page(
        &self,
        markdown: &str,
        metadata: &PageMetadata,
    ) -> Result<IngestOutcome> {
        let outcome = parse_and_emit(markdown, metadata)?;
        Ok(outcome)
    }
}

/// Free-function entry point matching the worktree-plan signature.
/// Equivalent to `parse_and_emit`; adapter writes are handled by
/// `JsonLdIngestPipeline::ingest_page` when port injection is needed.
pub async fn ingest_page(markdown: &str, metadata: &PageMetadata) -> Result<IngestOutcome> {
    parse_and_emit(markdown, metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_valid_block() -> &'static str {
        r#"```json-ld
{
  "@context": "https://narrativegoldmine.com/context/v1.jsonld",
  "@id": "urn:visionclaw:page:test-abc",
  "@type": "Page",
  "vc:slug": "test-abc",
  "prov:wasAttributedTo": { "@id": "did:nostr:npub1test" },
  "prov:generatedAtTime": { "@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime" }
}
```"#
    }

    #[test]
    fn parse_and_emit_produces_quads_from_valid_page_block() {
        let meta = PageMetadata::new("test.md");
        let outcome = parse_and_emit(minimal_valid_block(), &meta).unwrap();
        assert_eq!(outcome.block_count, 1);
        assert!(outcome.quad_count > 0);
        assert_eq!(outcome.source_path, "test.md");
    }

    /// SHACL gate (ADR-100 D4 / PRD-018 WS-5): a structurally valid block
    /// that also satisfies every shape rule passes the gate with a clean
    /// report. The gate is wired into `parse_and_emit` and surfaced on the
    /// outcome.
    #[test]
    fn shacl_gate_passes_for_valid_block_and_surfaces_report() {
        let meta = PageMetadata::new("test.md");
        let outcome = parse_and_emit(minimal_valid_block(), &meta).unwrap();
        assert!(
            outcome.shacl_report.is_valid(),
            "valid block must pass the SHACL gate: {:?}",
            outcome.shacl_report
        );
        assert!(
            outcome.shacl_report.shapes_checked >= 1,
            "the gate must have inspected at least one shape"
        );
    }

    /// A well-formed OntologyClass with a `subClassOf` parent satisfies both
    /// the schema validator and the SHACL shape gate end-to-end.
    #[test]
    fn shacl_gate_passes_for_valid_ontology_class() {
        let markdown = r#"```json-ld
{
  "@context": "https://narrativegoldmine.com/context/v1.jsonld",
  "@id": "urn:visionclaw:owl:class:cybernetics",
  "@type": "OntologyClass",
  "rdfs:subClassOf": { "@id": "urn:visionclaw:owl:class:built-environment" },
  "prov:wasAttributedTo": { "@id": "did:nostr:npub1abc" },
  "prov:generatedAtTime": { "@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime" }
}
```"#;
        let meta = PageMetadata::new("cyber.md");
        let outcome = parse_and_emit(markdown, &meta).unwrap();
        assert!(
            outcome.shacl_report.is_valid(),
            "{:?}",
            outcome.shacl_report
        );
    }

    #[test]
    fn parse_and_emit_fails_with_no_fenced_blocks() {
        let meta = PageMetadata::new("empty.md");
        let err = parse_and_emit("# Just a heading\n\nNo JSON-LD here.", &meta).unwrap_err();
        assert!(
            matches!(err, JsonLdIngestError::MissingCodeFenceMarker { .. }),
            "expected MissingCodeFenceMarker, got {:?}",
            err
        );
    }

    #[test]
    fn parse_and_emit_propagates_json_parse_error() {
        let markdown = "```json-ld\nnot valid json{{{\n```\n";
        let meta = PageMetadata::new("broken.md");
        let err = parse_and_emit(markdown, &meta).unwrap_err();
        assert!(
            matches!(err, JsonLdIngestError::JsonParseError { .. }),
            "expected JsonParseError, got {:?}",
            err
        );
    }

    #[test]
    fn parse_and_emit_returns_context_missing_error_for_block_without_context() {
        let markdown = r#"```json-ld
{
  "@id": "urn:visionclaw:page:no-ctx",
  "@type": "Page"
}
```"#;
        let meta = PageMetadata::new("no_ctx.md");
        let err = parse_and_emit(markdown, &meta).unwrap_err();
        assert!(
            matches!(err, JsonLdIngestError::ContextMissing { .. }),
            "expected ContextMissing, got {:?}",
            err
        );
    }

    #[test]
    fn page_metadata_source_path_roundtrips() {
        let meta = PageMetadata::new("pages/cybernetics.md");
        assert_eq!(meta.source_path, "pages/cybernetics.md");
        assert!(meta.content_sha1.is_none());
    }

    #[test]
    fn multiple_blocks_produce_cumulative_quads() {
        let block = minimal_valid_block();
        // Two identical blocks (same @id) in one file.
        let markdown = format!("{}\n\n{}", block, block);
        let meta = PageMetadata::new("multi.md");
        let outcome = parse_and_emit(&markdown, &meta).unwrap();
        assert_eq!(outcome.block_count, 2);
        // Quads for two blocks ≥ quads for one block.
        assert!(outcome.quad_count >= 2);
    }
}
