//! Error categories emitted by the canonical JSON-LD validator.
//!
//! Each variant corresponds 1:1 to a documented rejection case in
//! `tests/fixtures/data-model/invalid/README.md`. The closed set is
//! intentional: callers (the parser pipeline, the pre-commit binary,
//! the operator dashboard) switch on the variant — extending the enum
//! requires adding a fixture and updating the README in lockstep.

use std::fmt;

/// Closed set of validator error categories.
///
/// See `tests/fixtures/data-model/invalid/README.md` for the canonical
/// catalogue. Each variant carries the structured context needed to
/// generate a useful authoring suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// `@version: 1.1` not declared on a block that uses 1.1-only
    /// features (e.g. `@included`, framing keywords, typed values).
    SchemaVersionMissing,

    /// `@context` is entirely absent from the block.
    ContextMissing,

    /// `@context` URL is not one of the accepted versioned context
    /// files. `found` carries the URL string that was rejected.
    ContextVersionUnknown { found: String },

    /// A schema-required field is missing. `what` carries the canonical
    /// alias name of the missing field (e.g. `"subClassOf"`).
    RequiredFieldMissing { what: String },

    /// An `@id` value is not a syntactically valid IRI per RFC 3987.
    /// Most commonly: whitespace, missing scheme separator, control
    /// characters.
    MalformedIri { value: String },

    /// `vc:bridgeTo` points at a `LinkedPage` placeholder
    /// (`urn:visionclaw:linked:*`) rather than a concrete entity.
    BridgeTargetMustBeConcrete { target: String },

    /// An OWL construct outside the OWL 2 EL profile appears in an
    /// asserted axiom. `construct` is the rejected predicate or class
    /// expression (e.g. `"owl:unionOf"`).
    OutsideOwl2ElProfile { construct: String },

    /// File contains JSON that looks like JSON-LD but is NOT wrapped in
    /// a ```json-ld fenced code block. The parser silently skips such
    /// content, so the file emits zero events — a likely author error.
    MissingCodeFenceMarker,

    /// `@type` declares a class whose implied class-bit conflicts with
    /// the class-bit implied by the `@id`'s URN scheme.
    ClassBitMismatch {
        declared: String,
        implied_by_iri: String,
    },

    /// `prov:wasAttributedTo` is absent. Every block MUST carry
    /// attribution.
    ProvAttributionMissing,

    /// `prov:generatedAtTime` is absent. Every block MUST carry a
    /// generation timestamp.
    ProvTimestampMissing,
}

impl ErrorCategory {
    /// Stable short code suitable for log lines and CLI output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::SchemaVersionMissing => "SchemaVersionMissing",
            Self::ContextMissing => "ContextMissing",
            Self::ContextVersionUnknown { .. } => "ContextVersionUnknown",
            Self::RequiredFieldMissing { .. } => "RequiredFieldMissing",
            Self::MalformedIri { .. } => "MalformedIri",
            Self::BridgeTargetMustBeConcrete { .. } => "BridgeTargetMustBeConcrete",
            Self::OutsideOwl2ElProfile { .. } => "OutsideOwl2ElProfile",
            Self::MissingCodeFenceMarker => "MissingCodeFenceMarker",
            Self::ClassBitMismatch { .. } => "ClassBitMismatch",
            Self::ProvAttributionMissing => "ProvAttributionMissing",
            Self::ProvTimestampMissing => "ProvTimestampMissing",
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaVersionMissing => write!(
                f,
                "SchemaVersionMissing: JSON-LD 1.1 feature used without `@version: 1.1`"
            ),
            Self::ContextMissing => write!(f, "ContextMissing: block has no `@context`"),
            Self::ContextVersionUnknown { found } => write!(
                f,
                "ContextVersionUnknown: `@context` URL `{}` is not in the accepted set",
                found
            ),
            Self::RequiredFieldMissing { what } => write!(
                f,
                "RequiredFieldMissing: required field `{}` is absent",
                what
            ),
            Self::MalformedIri { value } => {
                write!(
                    f,
                    "MalformedIri: `@id` value `{}` is not a valid IRI",
                    value
                )
            }
            Self::BridgeTargetMustBeConcrete { target } => write!(
                f,
                "BridgeTargetMustBeConcrete: `vc:bridgeTo` target `{}` is a LinkedPage stub",
                target
            ),
            Self::OutsideOwl2ElProfile { construct } => write!(
                f,
                "OutsideOwl2ElProfile: `{}` is not permitted by OWL 2 EL (§3, Table 1)",
                construct
            ),
            Self::MissingCodeFenceMarker => write!(
                f,
                "MissingCodeFenceMarker: JSON content is not wrapped in a ```json-ld fence"
            ),
            Self::ClassBitMismatch {
                declared,
                implied_by_iri,
            } => write!(
                f,
                "ClassBitMismatch: `@type` implies `{}` but `@id` IRI implies `{}`",
                declared, implied_by_iri
            ),
            Self::ProvAttributionMissing => {
                write!(
                    f,
                    "ProvAttributionMissing: `prov:wasAttributedTo` is absent"
                )
            }
            Self::ProvTimestampMissing => {
                write!(f, "ProvTimestampMissing: `prov:generatedAtTime` is absent")
            }
        }
    }
}

/// Severity of a validation issue. The pre-commit hook fails on any
/// `Error`; `Warning` is informational.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}
