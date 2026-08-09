//! GitHub adapter contract.
//!
//! Canonical specification: ADR-10 §D11 (transport, env vars, error envelope)
//! + DDD-08 §"To Section 10 (GitHub adapter)" (value-object fields).
//!
//! Section 10 is the transport; Section 8 owns the parse and the domain.
//! This module defines the wire shape that crosses the boundary — the
//! `ParsedMarkdown` value object that Section 10 produces and Section 8
//! consumes via the `IngestPage` / `IngestOntologyOnly` commands.
//!
//! ## Auth and sync gating
//!
//! - Transport: `octocrab` REST client.
//! - Auth: `GITHUB_TOKEN` environment variable.
//! - Sync gating: `GitHubSyncService::sync_graphs()` SHA1-compares each
//!   file's blob against the cached hash and skips unchanged files.
//! - `FORCE_FULL_SYNC=1` bypasses gating and forces full reparse.
//!
//! ## Error reporting
//!
//! Parse errors do not fail the sync. The failing file is retained at its
//! previous good version in the triple store and a `ParseErrorReport`
//! envelope is surfaced via metrics
//! (`github_sync_parse_errors_total{error_kind}`).

use serde::{Deserialize, Serialize};

#[cfg(feature = "typescript-export")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// ParsedMarkdown value object
// ---------------------------------------------------------------------------

/// Output of the GitHub adapter for one source file.
///
/// The domain receives this via `IngestPage` / `IngestOntologyOnly`
/// commands and never sees raw HTTP responses, `octocrab` types, or
/// Logseq-specific frontmatter / wikilink syntax. If the corpus migrates
/// off Logseq, only the adapter changes; this value object stays stable.
///
/// `frontmatter_json` and `jsonld_blocks` are deliberately `serde_json::Value`
/// because:
///
/// - Frontmatter is open-ended (Logseq accepts arbitrary keys + tag lists).
/// - JSON-LD blocks must round-trip without loss for the ontology parser.
///
/// The richer DDD-08 fields (`prose_blocks`, `ontology_blocks`,
/// `outbound_wikilinks`) are domain projections built *from* this raw shape;
/// they belong in the Section 8 parser, not in this cross-boundary contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct ParsedMarkdown {
    /// Repo-relative path, normalised to forward slashes.
    /// (Section 8 calls this `path`; we use `canonical_path` to make the
    /// normalisation guarantee explicit at the boundary.)
    pub canonical_path: String,
    /// Raw file body, UTF-8.
    pub raw: String,
    /// Parsed Logseq frontmatter as a JSON object. Keys preserved verbatim
    /// except for `public:: true` which the adapter normalises into
    /// `{"public": true}` per DDD-08 §"To Section 10".
    #[cfg_attr(feature = "typescript-export", ts(type = "Record<string, unknown>"))]
    pub frontmatter_json: serde_json::Value,
    /// `### OntologyBlock` JSON-LD bodies, one per block. Order preserved.
    #[cfg_attr(
        feature = "typescript-export",
        ts(type = "Array<Record<string, unknown>>")
    )]
    pub jsonld_blocks: Vec<serde_json::Value>,
    /// Git blob SHA1 (40 hex chars). Used by the SHA1-gated sync.
    pub commit_sha: String,
}

// ---------------------------------------------------------------------------
// Parse-error envelope
// ---------------------------------------------------------------------------

/// One parse failure surfaced by the GitHub adapter.
///
/// Errors are logged but do not fail the sync; the failed file is retained
/// at its previous good version. The shape matches ADR-10 §D11 verbatim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
pub struct ParseErrorReport {
    pub path: String,
    pub sha: String,
    pub error_kind: ParseErrorKind,
    pub message: String,
}

/// Discriminated parse-error category. Drives the metric label
/// `github_sync_parse_errors_total{error_kind}`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript-export", derive(TS), ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum ParseErrorKind {
    /// Frontmatter YAML failed to parse.
    Yaml,
    /// `[[Wikilink]]` syntax malformed (unbalanced brackets, empty label, …).
    Wikilink,
    /// `### OntologyBlock` body failed JSON-LD parse.
    OntologyBlock,
    /// I/O level failure — file unreadable, blob missing, etc.
    Io,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parsed_markdown_round_trips() {
        let v = ParsedMarkdown {
            canonical_path: "mainKnowledgeGraph/pages/example.md".into(),
            raw: "public:: true\n\n# Example\n".into(),
            frontmatter_json: json!({ "public": true }),
            jsonld_blocks: vec![json!({"@id": "x", "@type": "Thing"})],
            commit_sha: "0".repeat(40),
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: ParsedMarkdown = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn parse_error_kind_uses_kebab_case() {
        let r = ParseErrorReport {
            path: "x.md".into(),
            sha: "deadbeef".into(),
            error_kind: ParseErrorKind::OntologyBlock,
            message: "missing @type".into(),
        };
        let s = serde_json::to_value(&r).unwrap();
        assert_eq!(s["error_kind"], "ontology-block");
    }
}
