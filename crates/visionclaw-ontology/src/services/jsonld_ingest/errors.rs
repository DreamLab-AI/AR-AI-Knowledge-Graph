// src/services/jsonld_ingest/errors.rs
//! Typed errors for the JSON-LD ingest pipeline.
//!
//! Every variant maps 1:1 to an entry in
//! `tests/fixtures/data-model/invalid/README.md`. The validator emits
//! exactly one of these for each rejection so the operator dashboard
//! can pivot on `error_code()` without string-matching.
//!
//! ## Mapping to fixture corpus
//!
//! | Variant                       | Invalid fixture                                  |
//! |-------------------------------|--------------------------------------------------|
//! | `SchemaVersionMissing`        | `100-missing-schema-version.md`                  |
//! | `ContextMissing`              | `101-missing-context.md`                         |
//! | `ContextVersionUnknown`       | `102-unknown-context-version.md`                 |
//! | `RequiredFieldMissing`        | `103-missing-required-field.md`                  |
//! | `MalformedIri`                | `104-malformed-iri.md`                           |
//! | `BridgeTargetMustBeConcrete`  | `105-bridgeTo-pointing-at-stub.md`               |
//! | `OutsideOwl2ElProfile`        | `106-disjunction-not-in-EL.md`                   |
//! | `MissingCodeFenceMarker`      | `107-bare-jsonld-without-block-marker.md`        |
//! | `ClassBitMismatch`            | `108-mismatched-class-bit.md`                    |
//! | `ProvAttributionMissing`      | (D8 invariant — no fixture, validator owns it)   |
//! | `ProvTimestampMissing`        | (D8 invariant — no fixture, validator owns it)   |

use thiserror::Error;

/// Closed set of validator + parser failure modes.
///
/// `Display` produces a one-line message including `error_code` so logs
/// stay grep-able. The structured payload is preserved on the variant.
#[derive(Debug, Error)]
pub enum JsonLdIngestError {
    /// JSON-LD 1.1 features (`@version`, `@included`, `@nest`, framing) used
    /// without declaring `@version: 1.1` on the document. Fixture 100.
    #[error(
        "[SchemaVersionMissing] at {file}:block#{block_index}: {feature} used without @version 1.1"
    )]
    SchemaVersionMissing {
        file: String,
        block_index: usize,
        feature: &'static str,
    },

    /// `@context` is absent entirely. Validator cannot resolve any term;
    /// fixture 101.
    #[error("[ContextMissing] at {file}:block#{block_index}: block has no @context")]
    ContextMissing { file: String, block_index: usize },

    /// `@context` URL not in the accepted set (currently only v1). Fixture 102.
    #[error("[ContextVersionUnknown] at {file}:block#{block_index}: unknown context URL {found:?}, supported: {supported:?}")]
    ContextVersionUnknown {
        file: String,
        block_index: usize,
        found: String,
        supported: Vec<String>,
    },

    /// Required schema field absent (e.g. OntologyClass without subClassOf
    /// parent, except the declared root). Fixture 103.
    #[error("[RequiredFieldMissing] at {file}:block#{block_index} on {iri}: field {field} required for type {type_name}")]
    RequiredFieldMissing {
        file: String,
        block_index: usize,
        iri: String,
        field: &'static str,
        type_name: String,
    },

    /// IRI value violates RFC 3987 (e.g. whitespace, missing scheme).
    /// Fixture 104.
    #[error("[MalformedIri] at {file}:block#{block_index}: IRI {iri:?} is malformed ({reason})")]
    MalformedIri {
        file: String,
        block_index: usize,
        iri: String,
        reason: &'static str,
    },

    /// A `BridgeRecord` whose `vc:bridgeTo` points to a `urn:visionclaw:linked:*`
    /// stub. Bridges must target concrete entities. Fixture 105.
    #[error("[BridgeTargetMustBeConcrete] at {file}:block#{block_index}: bridge {bridge_iri} points at stub {target_iri}")]
    BridgeTargetMustBeConcrete {
        file: String,
        block_index: usize,
        bridge_iri: String,
        target_iri: String,
    },

    /// Uses an OWL construct outside the OWL 2 EL profile (e.g. `owl:unionOf`,
    /// `owl:complementOf`, `owl:allValuesFrom`, `owl:disjointWith`). Fixture 106.
    #[error("[OutsideOwl2ElProfile] at {file}:block#{block_index}: construct {construct} is not in OWL 2 EL ({spec_reference})")]
    OutsideOwl2ElProfile {
        file: String,
        block_index: usize,
        construct: &'static str,
        spec_reference: &'static str,
        suggestion: &'static str,
    },

    /// JSON appears in a bare ``` fence (no `json-ld` language tag) or as
    /// indented prose, so the parser scans zero JSON-LD blocks. The file
    /// declared `expected-error:: MissingCodeFenceMarker` but yielded no
    /// events — itself the failure. Fixture 107.
    #[error("[MissingCodeFenceMarker] at {file}: no ```json-ld fenced blocks detected")]
    MissingCodeFenceMarker { file: String },

    /// `@type` declares a type whose class bit conflicts with the `@id` IRI
    /// scheme (e.g. `OntologyClass` with `urn:visionclaw:agent:*` IRI).
    /// Fixture 108.
    #[error("[ClassBitMismatch] at {file}:block#{block_index} on {iri}: type {type_name} (bit {type_bit}) conflicts with IRI scheme implying {iri_bit}")]
    ClassBitMismatch {
        file: String,
        block_index: usize,
        iri: String,
        type_name: String,
        type_bit: &'static str,
        iri_bit: &'static str,
    },

    /// `prov:wasAttributedTo` missing (D8 invariant).
    #[error("[ProvAttributionMissing] at {file}:block#{block_index} on {iri}: prov:wasAttributedTo required on every block")]
    ProvAttributionMissing {
        file: String,
        block_index: usize,
        iri: String,
    },

    /// `prov:generatedAtTime` missing (D8 invariant).
    #[error("[ProvTimestampMissing] at {file}:block#{block_index} on {iri}: prov:generatedAtTime required on every block")]
    ProvTimestampMissing {
        file: String,
        block_index: usize,
        iri: String,
    },

    /// JSON parse failure inside a fenced block.
    #[error("[JsonParseError] at {file}:block#{block_index}: {message}")]
    JsonParseError {
        file: String,
        block_index: usize,
        message: String,
    },

    /// Upstream repository adapter error.
    #[error("[RepositoryError] {0}")]
    RepositoryError(String),
}

impl JsonLdIngestError {
    /// Stable code string for telemetry / dashboards. One token, no spaces.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::SchemaVersionMissing { .. } => "SchemaVersionMissing",
            Self::ContextMissing { .. } => "ContextMissing",
            Self::ContextVersionUnknown { .. } => "ContextVersionUnknown",
            Self::RequiredFieldMissing { .. } => "RequiredFieldMissing",
            Self::MalformedIri { .. } => "MalformedIri",
            Self::BridgeTargetMustBeConcrete { .. } => "BridgeTargetMustBeConcrete",
            Self::OutsideOwl2ElProfile { .. } => "OutsideOwl2ElProfile",
            Self::MissingCodeFenceMarker { .. } => "MissingCodeFenceMarker",
            Self::ClassBitMismatch { .. } => "ClassBitMismatch",
            Self::ProvAttributionMissing { .. } => "ProvAttributionMissing",
            Self::ProvTimestampMissing { .. } => "ProvTimestampMissing",
            Self::JsonParseError { .. } => "JsonParseError",
            Self::RepositoryError(_) => "RepositoryError",
        }
    }
}

pub type Result<T> = std::result::Result<T, JsonLdIngestError>;
