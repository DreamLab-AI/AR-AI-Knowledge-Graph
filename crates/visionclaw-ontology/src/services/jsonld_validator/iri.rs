//! IRI well-formedness checks for the VisionClaw canonical schema.
//!
//! ADR-D01 §D2 enumerates the accepted URN schemes for subjects
//! (`urn:visionclaw:*`) and asserters (`did:nostr:*`). This module
//! provides predicate functions over IRI strings without pulling in a
//! full RFC 3987 parser — the rules are deliberately narrow because the
//! canonical schema only mints a handful of scheme prefixes.

/// Classification of a recognised VisionClaw IRI scheme. Used by
/// `class_bit.rs` to cross-check `@type` against `@id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IriScheme {
    /// `urn:visionclaw:page:<sha256>`
    Page,
    /// `urn:visionclaw:linked:<slug>` — LinkedPage placeholder.
    LinkedPage,
    /// `urn:visionclaw:owl:class:<slug>`
    OwlClass,
    /// `urn:visionclaw:owl:property:<slug>`
    OwlProperty,
    /// `urn:visionclaw:owl:axiom:<hash>` or `urn:visionclaw:axiom:<hash>`
    Axiom,
    /// `urn:visionclaw:agent:<run>:<step>`
    Agent,
    /// `urn:ngm:graph:<name>` (rarely used as `@id`)
    Graph,
    /// `urn:visionclaw:bridge:<id>`
    Bridge,
    /// `urn:visionclaw:nostr:event:<id>` — signed envelope.
    NostrEvent,
    /// `did:nostr:<pubkey>`
    DidNostr,
    /// Any other syntactically-valid IRI (schema/foaf/etc. or unknown
    /// `urn:visionclaw:*` variant).
    OtherValid,
}

/// True if the string is a syntactically valid IRI for this schema.
///
/// Rule set (deliberately conservative — the canonical schema does not
/// mint exotic IRIs):
///
/// - No whitespace anywhere.
/// - No control characters (`< 0x20` or `0x7F`).
/// - Must contain at least one `:` separator (scheme present).
/// - First scheme byte must be ASCII alphabetic (RFC 3986 §3.1).
/// - Cannot be empty.
pub fn is_well_formed(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if !value.is_ascii() {
        // Be conservative: the fixtures only mint ASCII IRIs.
        return false;
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    // Must have a scheme separator.
    let Some(colon_at) = value.find(':') else {
        return false;
    };
    if colon_at == 0 {
        return false;
    }
    let scheme = &value[..colon_at];
    // Scheme: ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
    let mut scheme_chars = scheme.chars();
    let Some(first) = scheme_chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    if !scheme_chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return false;
    }
    // Path / rest cannot be empty.
    let rest = &value[colon_at + 1..];
    !rest.is_empty()
}

/// Classify a recognised IRI scheme. Returns `None` if the IRI is
/// malformed or its scheme is unknown to this validator.
///
/// The function ONLY recognises schemes used by the canonical schema.
/// Schemes like `http://`, `https://`, `schema:`, `urn:isbn:` parse as
/// `OtherValid` if well-formed.
pub fn classify(value: &str) -> Option<IriScheme> {
    if !is_well_formed(value) {
        return None;
    }
    // Order matters: `urn:visionclaw:owl:property:` must be tested
    // before `urn:visionclaw:owl:class:` etc.
    let prefixes: &[(&str, IriScheme)] = &[
        ("urn:visionclaw:page:", IriScheme::Page),
        ("urn:visionclaw:linked:", IriScheme::LinkedPage),
        ("urn:visionclaw:linkedpage:", IriScheme::LinkedPage),
        ("urn:visionclaw:owl:class:", IriScheme::OwlClass),
        ("urn:visionclaw:owl:property:", IriScheme::OwlProperty),
        ("urn:visionclaw:owl:axiom:", IriScheme::Axiom),
        ("urn:visionclaw:axiom:", IriScheme::Axiom),
        ("urn:visionclaw:agent:", IriScheme::Agent),
        ("urn:ngm:graph:", IriScheme::Graph),
        ("urn:visionclaw:bridge:", IriScheme::Bridge),
        ("urn:visionclaw:nostr:event:", IriScheme::NostrEvent),
        // v2 urn:ngm: scheme variants
        ("urn:ngm:class:", IriScheme::OwlClass),
        ("urn:ngm:property:", IriScheme::OwlProperty),
        ("urn:ngm:axiom:", IriScheme::Axiom),
        ("did:nostr:", IriScheme::DidNostr),
    ];
    for (prefix, scheme) in prefixes {
        if value.starts_with(prefix) && value.len() > prefix.len() {
            return Some(*scheme);
        }
    }
    Some(IriScheme::OtherValid)
}

/// True if the IRI is a `LinkedPage` placeholder, per the
/// `urn:visionclaw:linked:` (or legacy `urn:visionclaw:linkedpage:`)
/// scheme.
pub fn is_linked_page_iri(value: &str) -> bool {
    matches!(classify(value), Some(IriScheme::LinkedPage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_iri_with_spaces_rejected() {
        assert!(!is_well_formed("urn visionclaw page malformed id"));
        assert!(!is_well_formed(""));
        assert!(!is_well_formed("no-scheme-separator"));
        assert!(!is_well_formed("1bad:scheme"));
    }

    #[test]
    fn canonical_iris_accepted() {
        assert!(is_well_formed("urn:visionclaw:page:abc"));
        assert!(is_well_formed("did:nostr:npub1alice"));
        assert!(is_well_formed("https://example.org/x"));
    }

    #[test]
    fn classify_visionclaw_iris() {
        assert_eq!(classify("urn:visionclaw:page:abc"), Some(IriScheme::Page));
        assert_eq!(
            classify("urn:visionclaw:linked:tempietto"),
            Some(IriScheme::LinkedPage)
        );
        assert_eq!(
            classify("urn:visionclaw:agent:r1:s2"),
            Some(IriScheme::Agent)
        );
        assert_eq!(
            classify("urn:visionclaw:owl:class:cybernetics"),
            Some(IriScheme::OwlClass)
        );
    }

    #[test]
    fn linked_page_detection() {
        assert!(is_linked_page_iri("urn:visionclaw:linked:tempietto"));
        assert!(!is_linked_page_iri("urn:visionclaw:page:abc"));
    }
}
