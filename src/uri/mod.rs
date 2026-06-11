//! Converged `urn:visionclaw` identifier minter (BC20 counterpart).
//!
//! This is the VisionClaw-side definition of the converged URN grammar whose
//! agentbox counterpart is `management-api/lib/bc20-provenance-bridge.js` +
//! `management-api/lib/uris.js`. Until this module merges to `main`, that
//! agentbox bridge was the *only* executable definition of the cross-namespace
//! contract — VisionClaw `main` still carries the legacy `urn:ngm:*` scheme,
//! which is left intact here to coexist (no rip-out yet).
//!
//! Grammar (per agentbox/CLAUDE.md "Parallel namespace"):
//!
//!   * `urn:visionclaw:concept:<domain>:<slug>`
//!       domain-scoped — a post-elevation shared ontology class.
//!   * `urn:visionclaw:kg:<hex-pubkey>:<sha256-12>`
//!       owner-scoped, content-addressed — a personal KG node.
//!   * `urn:visionclaw:bead:<hex-pubkey>:<sha256-12>`
//!       owner-scoped, content-addressed.
//!   * `urn:visionclaw:execution:<sha256-12>`
//!       content-addressed, **unscoped** — owner travels in `owner_did`.
//!   * `urn:visionclaw:group:<team>#members`
//!       team-scoped.
//!   * `urn:visionclaw:room:<sha256-12>`
//!       content-addressed, unscoped — an XR presence room (DDD-XR §7.2).
//!   * `urn:visionclaw:avatar:<hex-pubkey>`
//!       identity-bound 1:1 with the avatar's DID (DDD-XR §7.2).
//!   * identity is `did:nostr:<hex-pubkey>` — there is **no** `urn:visionclaw:agent`
//!     kind; an agent's identity *is* its DID.
//!
//! Conventions shared with the agentbox side:
//!   * content addressing → `sha256-12-<12 lowercase hex chars>`.
//!   * owner scope → the 64-char BIP-340 x-only hex pubkey (not bech32 npub).
//!
//! Discipline: every durable `urn:visionclaw` identifier MUST be minted through
//! the typed constructors below. Ad-hoc `format!()` construction is prohibited,
//! mirroring the `uris.js` mandate on the agentbox side.

use sha2::{Digest, Sha256};
use std::fmt;

/// URN namespace prefix for all converged VisionClaw identifiers.
pub const NS: &str = "urn:visionclaw";
/// Legacy URN namespace prefix (pre-convergence `urn:ngm:*`). New identifiers are
/// never minted under this prefix, but persisted IDs that predate the cutover must
/// keep resolving — see [`parse_dual`] and ADR-105 (urn convergence + ngm cutover).
pub const LEGACY_NGM_NS: &str = "urn:ngm";
/// DID method used for sovereign identity (shared with the VisionClaw substrate).
pub const DID_NOSTR_PREFIX: &str = "did:nostr:";
/// Content-address prefix (`sha256-12-<12 hex>`).
pub const CONTENT_ADDR_PREFIX: &str = "sha256-12-";

/// The set of converged URN kinds. There is deliberately NO `Agent` kind:
/// identity is `did:nostr:<pubkey>`, minted via [`did_nostr`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `concept:<domain>:<slug>` — domain-scoped shared ontology class.
    Concept,
    /// `kg:<hex-pubkey>:<sha256-12>` — owner-scoped, content-addressed KG node.
    Kg,
    /// `bead:<hex-pubkey>:<sha256-12>` — owner-scoped, content-addressed.
    Bead,
    /// `execution:<sha256-12>` — content-addressed, unscoped.
    Execution,
    /// `group:<team>#members` — team-scoped.
    Group,
    /// `room:<sha256-12>` — content-addressed, unscoped XR presence room.
    Room,
    /// `avatar:<hex-pubkey>` — identity-bound 1:1 with the avatar's DID.
    Avatar,
}

impl Kind {
    /// The wire token for this kind (the segment after `urn:visionclaw:`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Concept => "concept",
            Kind::Kg => "kg",
            Kind::Bead => "bead",
            Kind::Execution => "execution",
            Kind::Group => "group",
            Kind::Room => "room",
            Kind::Avatar => "avatar",
        }
    }

    fn from_token(tok: &str) -> Option<Self> {
        Some(match tok {
            "concept" => Kind::Concept,
            "kg" => Kind::Kg,
            "bead" => Kind::Bead,
            "execution" => Kind::Execution,
            "group" => Kind::Group,
            "room" => Kind::Room,
            "avatar" => Kind::Avatar,
            _ => return None,
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a mint or parse was rejected. Minting is fail-closed: a malformed input
/// yields an error rather than a structurally-invalid identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    /// A 64-char lowercase-hex BIP-340 x-only pubkey was required.
    InvalidPubkey(String),
    /// An empty or whitespace-only required segment.
    EmptySegment(&'static str),
    /// Input was not a recognised `urn:visionclaw` identifier.
    NotVisionclaw(String),
    /// The kind token was not one of the converged kinds.
    UnknownKind(String),
    /// The identifier was the right kind but structurally malformed.
    Malformed(String),
}

impl fmt::Display for UriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UriError::InvalidPubkey(s) => write!(f, "invalid 64-hex pubkey scope: {s}"),
            UriError::EmptySegment(s) => write!(f, "empty required segment: {s}"),
            UriError::NotVisionclaw(s) => write!(f, "not a urn:visionclaw identifier: {s}"),
            UriError::UnknownKind(s) => write!(f, "unknown urn:visionclaw kind: {s}"),
            UriError::Malformed(s) => write!(f, "malformed urn:visionclaw identifier: {s}"),
        }
    }
}

impl std::error::Error for UriError {}

/// True iff `s` is a 64-char lowercase-hex BIP-340 x-only pubkey.
pub fn is_pubkey_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `sha256-12-<12 lowercase hex>` content address over `input` bytes.
/// Matches the agentbox `sha12()` helper byte-for-byte.
pub fn content_address(input: impl AsRef<[u8]>) -> String {
    let digest = Sha256::digest(input.as_ref());
    let mut hex = String::with_capacity(12);
    for b in digest.iter().take(6) {
        // each byte → two lowercase hex chars; 6 bytes = 12 chars.
        hex.push(nibble(b >> 4));
        hex.push(nibble(b & 0x0f));
    }
    format!("{CONTENT_ADDR_PREFIX}{hex}")
}

#[inline]
fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

/// Lowercase, collapse non-`[a-z0-9._-]` runs to `-`, trim leading/trailing `-`.
/// Matches the agentbox `slugify()`.
pub fn slugify(s: &str) -> String {
    let lowered = s.trim().to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut last_dash = false;
    for ch in lowered.chars() {
        let keep = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
        if keep {
            out.push(ch);
            last_dash = ch == '-';
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

// ── Typed mint functions (one per kind) ──────────────────────────────────────

/// Mint identity: `did:nostr:<hex-pubkey>`. Not a `urn:visionclaw` kind.
pub fn did_nostr(pubkey: &str) -> Result<String, UriError> {
    if !is_pubkey_hex(pubkey) {
        return Err(UriError::InvalidPubkey(pubkey.to_string()));
    }
    Ok(format!("{DID_NOSTR_PREFIX}{pubkey}"))
}

/// Mint `urn:visionclaw:concept:<domain>:<slug>` (domain-scoped ontology class).
/// Both `domain` and `slug` are slugified.
pub fn concept(domain: &str, slug: &str) -> Result<String, UriError> {
    let d = slugify(domain);
    let s = slugify(slug);
    if d.is_empty() {
        return Err(UriError::EmptySegment("concept domain"));
    }
    if s.is_empty() {
        return Err(UriError::EmptySegment("concept slug"));
    }
    Ok(format!("{NS}:{}:{d}:{s}", Kind::Concept))
}

/// Mint `urn:visionclaw:kg:<hex-pubkey>:<sha256-12>` from owner + raw content.
pub fn kg(owner_pubkey: &str, content: impl AsRef<[u8]>) -> Result<String, UriError> {
    if !is_pubkey_hex(owner_pubkey) {
        return Err(UriError::InvalidPubkey(owner_pubkey.to_string()));
    }
    Ok(format!(
        "{NS}:{}:{owner_pubkey}:{}",
        Kind::Kg,
        content_address(content)
    ))
}

/// Mint `urn:visionclaw:kg:<hex-pubkey>:<sha256-12>` from an already-computed
/// content address (e.g. a value crossing the BC20 boundary).
pub fn kg_with_address(owner_pubkey: &str, content_addr: &str) -> Result<String, UriError> {
    if !is_pubkey_hex(owner_pubkey) {
        return Err(UriError::InvalidPubkey(owner_pubkey.to_string()));
    }
    if !content_addr.starts_with(CONTENT_ADDR_PREFIX) {
        return Err(UriError::Malformed(content_addr.to_string()));
    }
    Ok(format!("{NS}:{}:{owner_pubkey}:{content_addr}", Kind::Kg))
}

/// Mint `urn:visionclaw:bead:<hex-pubkey>:<sha256-12>` from owner + raw content.
pub fn bead(owner_pubkey: &str, content: impl AsRef<[u8]>) -> Result<String, UriError> {
    if !is_pubkey_hex(owner_pubkey) {
        return Err(UriError::InvalidPubkey(owner_pubkey.to_string()));
    }
    Ok(format!(
        "{NS}:{}:{owner_pubkey}:{}",
        Kind::Bead,
        content_address(content)
    ))
}

/// Mint `urn:visionclaw:execution:<sha256-12>` (unscoped; owner in `owner_did`).
pub fn execution(content: impl AsRef<[u8]>) -> String {
    format!("{NS}:{}:{}", Kind::Execution, content_address(content))
}

/// Mint `urn:visionclaw:group:<team>#members` (team-scoped membership ref).
pub fn group_members(team: &str) -> Result<String, UriError> {
    let t = slugify(team);
    if t.is_empty() {
        return Err(UriError::EmptySegment("group team"));
    }
    Ok(format!("{NS}:{}:{t}#members", Kind::Group))
}

/// Mint `urn:visionclaw:room:<sha256-12>` (unscoped XR presence room) from raw
/// room-defining content (e.g. the room descriptor).
pub fn room(content: impl AsRef<[u8]>) -> String {
    format!("{NS}:{}:{}", Kind::Room, content_address(content))
}

/// Mint `urn:visionclaw:avatar:<hex-pubkey>` — bound 1:1 with the avatar's DID.
pub fn avatar(pubkey: &str) -> Result<String, UriError> {
    if !is_pubkey_hex(pubkey) {
        return Err(UriError::InvalidPubkey(pubkey.to_string()));
    }
    Ok(format!("{NS}:{}:{pubkey}", Kind::Avatar))
}

// ── Parsing (round-trip + BC20 ingest) ───────────────────────────────────────

/// A parsed converged identifier. `did:nostr` is represented as
/// [`ParsedUri::DidNostr`]; the URN kinds carry their structural fields so
/// the ingest path can record a namespace-crossing rather than store an opaque
/// blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedUri {
    /// `did:nostr:<pubkey>` — sovereign identity.
    DidNostr { pubkey: String },
    /// `concept:<domain>:<slug>`.
    Concept { domain: String, slug: String },
    /// `kg:<pubkey>:<sha256-12>`.
    Kg { pubkey: String, address: String },
    /// `bead:<pubkey>:<sha256-12>`.
    Bead { pubkey: String, address: String },
    /// `execution:<sha256-12>` (unscoped).
    Execution { address: String },
    /// `group:<team>#members`.
    Group { team: String },
    /// `room:<sha256-12>` (unscoped XR presence room).
    Room { address: String },
    /// `avatar:<hex-pubkey>` (identity-bound).
    Avatar { pubkey: String },
    /// A legacy `urn:ngm:<sub>` identifier that predates the convergence cutover.
    /// Carried opaquely (the segment after `urn:ngm:`) so the dual-read resolve
    /// path can recognise and round-trip it without re-minting it under the
    /// converged grammar. New mints never produce this variant. See ADR-105.
    LegacyNgm { sub: String },
}

impl ParsedUri {
    /// The kind, if this is one of the `urn:visionclaw` URN kinds
    /// (identity has no `Kind`).
    pub fn kind(&self) -> Option<Kind> {
        Some(match self {
            // Identity and legacy-ngm carry no converged `Kind`.
            ParsedUri::DidNostr { .. } | ParsedUri::LegacyNgm { .. } => return None,
            ParsedUri::Concept { .. } => Kind::Concept,
            ParsedUri::Kg { .. } => Kind::Kg,
            ParsedUri::Bead { .. } => Kind::Bead,
            ParsedUri::Execution { .. } => Kind::Execution,
            ParsedUri::Group { .. } => Kind::Group,
            ParsedUri::Room { .. } => Kind::Room,
            ParsedUri::Avatar { .. } => Kind::Avatar,
        })
    }

    /// The owner pubkey scope, when the kind carries one.
    pub fn owner_pubkey(&self) -> Option<&str> {
        match self {
            ParsedUri::DidNostr { pubkey }
            | ParsedUri::Kg { pubkey, .. }
            | ParsedUri::Bead { pubkey, .. }
            | ParsedUri::Avatar { pubkey } => Some(pubkey),
            _ => None,
        }
    }

    /// Reconstruct the canonical string form (round-trip of [`parse`]).
    pub fn to_uri(&self) -> String {
        match self {
            ParsedUri::DidNostr { pubkey } => format!("{DID_NOSTR_PREFIX}{pubkey}"),
            ParsedUri::Concept { domain, slug } => {
                format!("{NS}:{}:{domain}:{slug}", Kind::Concept)
            }
            ParsedUri::Kg { pubkey, address } => format!("{NS}:{}:{pubkey}:{address}", Kind::Kg),
            ParsedUri::Bead { pubkey, address } => format!("{NS}:{}:{pubkey}:{address}", Kind::Bead),
            ParsedUri::Execution { address } => format!("{NS}:{}:{address}", Kind::Execution),
            ParsedUri::Group { team } => format!("{NS}:{}:{team}#members", Kind::Group),
            ParsedUri::Room { address } => format!("{NS}:{}:{address}", Kind::Room),
            ParsedUri::Avatar { pubkey } => format!("{NS}:{}:{pubkey}", Kind::Avatar),
            ParsedUri::LegacyNgm { sub } => format!("{LEGACY_NGM_NS}:{sub}"),
        }
    }

    /// True iff this is a legacy `urn:ngm:*` identifier carried for backward
    /// resolution rather than a converged `urn:visionclaw` / `did:nostr` one.
    pub fn is_legacy(&self) -> bool {
        matches!(self, ParsedUri::LegacyNgm { .. })
    }
}

/// Parse a converged identifier (`did:nostr:*` or `urn:visionclaw:*`). Returns
/// [`UriError`] for any other namespace (including the legacy `urn:ngm:*`).
pub fn parse(input: &str) -> Result<ParsedUri, UriError> {
    if let Some(pubkey) = input.strip_prefix(DID_NOSTR_PREFIX) {
        if !is_pubkey_hex(pubkey) {
            return Err(UriError::InvalidPubkey(pubkey.to_string()));
        }
        return Ok(ParsedUri::DidNostr {
            pubkey: pubkey.to_string(),
        });
    }

    let rest = input
        .strip_prefix(&format!("{NS}:"))
        .ok_or_else(|| UriError::NotVisionclaw(input.to_string()))?;

    // kind is the first ':'-delimited token.
    let (kind_tok, tail) = rest
        .split_once(':')
        .ok_or_else(|| UriError::Malformed(input.to_string()))?;
    let kind = Kind::from_token(kind_tok).ok_or_else(|| UriError::UnknownKind(kind_tok.to_string()))?;

    match kind {
        Kind::Concept => {
            let (domain, slug) = tail
                .split_once(':')
                .ok_or_else(|| UriError::Malformed(input.to_string()))?;
            if domain.is_empty() {
                return Err(UriError::EmptySegment("concept domain"));
            }
            if slug.is_empty() {
                return Err(UriError::EmptySegment("concept slug"));
            }
            Ok(ParsedUri::Concept {
                domain: domain.to_string(),
                slug: slug.to_string(),
            })
        }
        Kind::Kg | Kind::Bead => {
            let (pubkey, address) = tail
                .split_once(':')
                .ok_or_else(|| UriError::Malformed(input.to_string()))?;
            if !is_pubkey_hex(pubkey) {
                return Err(UriError::InvalidPubkey(pubkey.to_string()));
            }
            if !address.starts_with(CONTENT_ADDR_PREFIX) {
                return Err(UriError::Malformed(address.to_string()));
            }
            if kind == Kind::Kg {
                Ok(ParsedUri::Kg {
                    pubkey: pubkey.to_string(),
                    address: address.to_string(),
                })
            } else {
                Ok(ParsedUri::Bead {
                    pubkey: pubkey.to_string(),
                    address: address.to_string(),
                })
            }
        }
        Kind::Execution => {
            if !tail.starts_with(CONTENT_ADDR_PREFIX) || tail.contains(':') {
                return Err(UriError::Malformed(input.to_string()));
            }
            Ok(ParsedUri::Execution {
                address: tail.to_string(),
            })
        }
        Kind::Group => {
            let team = tail
                .strip_suffix("#members")
                .ok_or_else(|| UriError::Malformed(input.to_string()))?;
            if team.is_empty() {
                return Err(UriError::EmptySegment("group team"));
            }
            Ok(ParsedUri::Group {
                team: team.to_string(),
            })
        }
        Kind::Room => {
            if !tail.starts_with(CONTENT_ADDR_PREFIX) || tail.contains(':') {
                return Err(UriError::Malformed(input.to_string()));
            }
            Ok(ParsedUri::Room {
                address: tail.to_string(),
            })
        }
        Kind::Avatar => {
            if !is_pubkey_hex(tail) {
                return Err(UriError::InvalidPubkey(tail.to_string()));
            }
            Ok(ParsedUri::Avatar {
                pubkey: tail.to_string(),
            })
        }
    }
}

/// Dual-read parse for the resolve path (ADR-105). Accepts BOTH the converged
/// grammar (`did:nostr:*` / `urn:visionclaw:*`, via [`parse`]) AND legacy
/// `urn:ngm:*` identifiers persisted before the convergence cutover, which are
/// returned as [`ParsedUri::LegacyNgm`].
///
/// This is the function every *resolve/lookup* surface should call so that
/// already-stored `urn:ngm:node:*` / `urn:ngm:edge:*` / `urn:ngm:domain:*` IDs
/// keep resolving. Minting and strict validation continue to use [`parse`],
/// which deliberately rejects `urn:ngm:*` (new durable IDs are converged-only).
///
/// `urn:ngm:graph:*` named graphs are recognised here like any other legacy
/// sub-namespace; per ADR-100 they remain the persistence named-graph IRIs and
/// are not rewritten.
pub fn parse_dual(input: &str) -> Result<ParsedUri, UriError> {
    match parse(input) {
        Ok(p) => Ok(p),
        Err(UriError::NotVisionclaw(_)) => {
            if let Some(sub) = input.strip_prefix(&format!("{LEGACY_NGM_NS}:")) {
                if sub.is_empty() {
                    return Err(UriError::Malformed(input.to_string()));
                }
                return Ok(ParsedUri::LegacyNgm {
                    sub: sub.to_string(),
                });
            }
            Err(UriError::NotVisionclaw(input.to_string()))
        }
        Err(e) => Err(e),
    }
}

// ── BC20 cross-namespace ingest (urn:agentbox:* → urn:visionclaw:*) ──────────

/// A namespace crossing recorded at the federation boundary. Carries both ends
/// so the ingest path stores the translation rather than an opaque foreign blob,
/// and the audit surface can recover the agentbox source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrnCrossing {
    /// The original `urn:agentbox:*` (or `did:nostr:*`) identifier as received.
    pub agentbox_urn: String,
    /// The translated converged VisionClaw identifier.
    pub visionclaw_id: String,
    /// `did:nostr:<pubkey>` owner when the source carried a pubkey scope.
    pub owner_did: Option<String>,
}

/// Translate an inbound `urn:agentbox:<kind>:<scope>:<local>` (or `did:nostr:*`)
/// into its converged VisionClaw identifier — the VisionClaw-side counterpart of
/// agentbox `bc20-provenance-bridge.js::toVisionclaw`. The closed kind map:
///
///   * `agent`    → `did:nostr:<pubkey>` (identity, structural round-trip)
///   * `activity` → `urn:visionclaw:execution:<sha256-12>` (unscoped)
///   * `thing`    → `urn:visionclaw:kg:<pubkey>:<sha256-12>`
///   * `memory`   → `urn:visionclaw:concept:...` requires elevation {domain,slug},
///                  which the ingest hot path does not have, so it is recorded as
///                  a crossing-without-translation (returns `None`).
///
/// A `did:nostr:*` input is already converged and round-trips structurally.
/// Returns `None` (the caller records the raw string + an unmapped marker) for
/// any unmapped kind, mirroring the agentbox B04 closed-map discipline.
pub fn cross_from_agentbox(agentbox_urn: &str) -> Option<UrnCrossing> {
    // Already-converged identity passes through unchanged.
    if let Some(pk) = agentbox_urn.strip_prefix(DID_NOSTR_PREFIX) {
        if is_pubkey_hex(pk) {
            return Some(UrnCrossing {
                agentbox_urn: agentbox_urn.to_string(),
                visionclaw_id: agentbox_urn.to_string(),
                owner_did: Some(agentbox_urn.to_string()),
            });
        }
        return None;
    }

    let rest = agentbox_urn.strip_prefix("urn:agentbox:")?;
    let (kind, tail) = rest.split_once(':')?;
    // scope is the next token when it is a 64-hex pubkey; otherwise unscoped.
    let scope = tail.split(':').next().filter(|s| is_pubkey_hex(s));
    let owner_did = scope.and_then(|pk| did_nostr(pk).ok());

    let visionclaw_id = match kind {
        "agent" => {
            let pk = scope?;
            did_nostr(pk).ok()?
        }
        "activity" => execution(agentbox_urn),
        "thing" => {
            let pk = scope?;
            kg(pk, agentbox_urn).ok()?
        }
        // memory→concept needs the elevation {domain,slug} target, absent on the
        // hot path; the crossing is recorded raw rather than mis-mapped.
        _ => return None,
    };

    Some(UrnCrossing {
        agentbox_urn: agentbox_urn.to_string(),
        visionclaw_id,
        owner_did,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PK_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PK_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn pubkey_validation() {
        assert!(is_pubkey_hex(PK_A));
        assert!(!is_pubkey_hex("AAAA")); // too short
        assert!(!is_pubkey_hex(&PK_A.to_uppercase())); // must be lowercase
        assert!(!is_pubkey_hex(&format!("{}g", &PK_A[..63]))); // non-hex char
    }

    #[test]
    fn content_address_is_12_lowercase_hex() {
        let a = content_address(b"hello world");
        assert!(a.starts_with(CONTENT_ADDR_PREFIX));
        let hex = a.strip_prefix(CONTENT_ADDR_PREFIX).unwrap();
        assert_eq!(hex.len(), 12);
        assert!(hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        // deterministic
        assert_eq!(a, content_address(b"hello world"));
        // matches a hand-computed sha256-12 of "hello world".
        assert_eq!(a, "sha256-12-b94d27b9934d");
    }

    #[test]
    fn slugify_matches_agentbox_shape() {
        assert_eq!(slugify("  Hello World!! "), "hello-world");
        assert_eq!(slugify("Foo/Bar.baz_qux"), "foo-bar.baz_qux");
        assert_eq!(slugify("---trim---"), "trim");
    }

    // ── per-kind shape + round-trip ──────────────────────────────────────────

    #[test]
    fn did_nostr_shape_and_roundtrip() {
        let id = did_nostr(PK_A).unwrap();
        assert_eq!(id, format!("did:nostr:{PK_A}"));
        let p = parse(&id).unwrap();
        assert_eq!(p, ParsedUri::DidNostr { pubkey: PK_A.into() });
        assert_eq!(p.to_uri(), id);
        assert_eq!(p.kind(), None);
        assert_eq!(p.owner_pubkey(), Some(PK_A));
        assert!(did_nostr("nope").is_err());
    }

    #[test]
    fn concept_shape_and_roundtrip() {
        let id = concept("Knowledge Graph", "Spreading Activation").unwrap();
        assert_eq!(id, "urn:visionclaw:concept:knowledge-graph:spreading-activation");
        let p = parse(&id).unwrap();
        assert_eq!(p.kind(), Some(Kind::Concept));
        assert_eq!(p.to_uri(), id);
        assert!(concept("", "x").is_err());
        assert!(concept("d", "  ").is_err());
    }

    #[test]
    fn kg_shape_and_roundtrip() {
        let id = kg(PK_A, b"node-payload").unwrap();
        assert!(id.starts_with(&format!("urn:visionclaw:kg:{PK_A}:sha256-12-")));
        let p = parse(&id).unwrap();
        assert_eq!(p.kind(), Some(Kind::Kg));
        assert_eq!(p.owner_pubkey(), Some(PK_A));
        assert_eq!(p.to_uri(), id);
        // content-addressing is deterministic on the same owner + content.
        assert_eq!(id, kg(PK_A, b"node-payload").unwrap());
        assert!(kg("short", b"x").is_err());
    }

    #[test]
    fn kg_with_precomputed_address_roundtrip() {
        let addr = content_address(b"crossing-value");
        let id = kg_with_address(PK_B, &addr).unwrap();
        let p = parse(&id).unwrap();
        assert_eq!(p, ParsedUri::Kg { pubkey: PK_B.into(), address: addr });
        assert_eq!(p.to_uri(), id);
        assert!(kg_with_address(PK_B, "not-an-addr").is_err());
    }

    #[test]
    fn bead_shape_and_roundtrip() {
        let id = bead(PK_B, b"bead-content").unwrap();
        assert!(id.starts_with(&format!("urn:visionclaw:bead:{PK_B}:sha256-12-")));
        let p = parse(&id).unwrap();
        assert_eq!(p.kind(), Some(Kind::Bead));
        assert_eq!(p.owner_pubkey(), Some(PK_B));
        assert_eq!(p.to_uri(), id);
    }

    #[test]
    fn execution_shape_and_roundtrip() {
        let id = execution(b"urn:agentbox:activity:abc:run-1");
        assert!(id.starts_with("urn:visionclaw:execution:sha256-12-"));
        // unscoped — no pubkey segment.
        let p = parse(&id).unwrap();
        assert_eq!(p.kind(), Some(Kind::Execution));
        assert_eq!(p.owner_pubkey(), None);
        assert_eq!(p.to_uri(), id);
        // a stray scope segment must be rejected.
        assert!(parse("urn:visionclaw:execution:sha256-12-deadbeef0011:extra").is_err());
    }

    #[test]
    fn group_shape_and_roundtrip() {
        let id = group_members("Dream Lab").unwrap();
        assert_eq!(id, "urn:visionclaw:group:dream-lab#members");
        let p = parse(&id).unwrap();
        assert_eq!(p, ParsedUri::Group { team: "dream-lab".into() });
        assert_eq!(p.to_uri(), id);
        assert!(group_members("   ").is_err());
    }

    #[test]
    fn room_shape_and_roundtrip() {
        let id = room(b"room-descriptor");
        assert!(id.starts_with("urn:visionclaw:room:sha256-12-"));
        let p = parse(&id).unwrap();
        assert_eq!(p.kind(), Some(Kind::Room));
        assert_eq!(p.owner_pubkey(), None);
        assert_eq!(p.to_uri(), id);
        // deterministic on content
        assert_eq!(id, room(b"room-descriptor"));
        // a stray scope segment must be rejected.
        assert!(parse("urn:visionclaw:room:sha256-12-deadbeef0011:extra").is_err());
    }

    #[test]
    fn avatar_shape_and_roundtrip() {
        let id = avatar(PK_A).unwrap();
        assert_eq!(id, format!("urn:visionclaw:avatar:{PK_A}"));
        let p = parse(&id).unwrap();
        assert_eq!(p, ParsedUri::Avatar { pubkey: PK_A.into() });
        assert_eq!(p.kind(), Some(Kind::Avatar));
        assert_eq!(p.owner_pubkey(), Some(PK_A));
        assert_eq!(p.to_uri(), id);
        assert!(avatar("nope").is_err());
        assert!(parse(&format!("urn:visionclaw:avatar:{}", PK_A.to_uppercase())).is_err());
    }

    #[test]
    fn rejects_other_namespaces_including_legacy_ngm() {
        assert!(matches!(parse("urn:ngm:node:42"), Err(UriError::NotVisionclaw(_))));
        assert!(matches!(parse("urn:agentbox:thing:x:y"), Err(UriError::NotVisionclaw(_))));
        assert!(matches!(
            parse("urn:visionclaw:bogus:whatever"),
            Err(UriError::UnknownKind(_))
        ));
    }

    #[test]
    fn parse_dual_accepts_converged_and_legacy_ngm() {
        // Converged forms round-trip identically through the dual reader.
        let vc = kg(PK_A, b"x").unwrap();
        assert_eq!(parse_dual(&vc).unwrap(), parse(&vc).unwrap());
        let did = did_nostr(PK_A).unwrap();
        assert_eq!(parse_dual(&did).unwrap(), parse(&did).unwrap());

        // Legacy persisted IDs resolve as LegacyNgm and round-trip their string.
        for legacy in [
            "urn:ngm:node:42",
            "urn:ngm:edge:1:2:1-2",
            "urn:ngm:domain:knowledge",
            "urn:ngm:graph:knowledge",
        ] {
            let p = parse_dual(legacy).unwrap();
            assert!(p.is_legacy());
            assert_eq!(p.kind(), None);
            assert_eq!(p.owner_pubkey(), None);
            assert_eq!(p.to_uri(), legacy);
        }

        // Strict parse still rejects legacy (mint path is converged-only).
        assert!(matches!(parse("urn:ngm:node:42"), Err(UriError::NotVisionclaw(_))));
        // Neither reader accepts an empty legacy sub or a foreign namespace.
        assert!(parse_dual("urn:ngm:").is_err());
        assert!(matches!(
            parse_dual("urn:agentbox:thing:x:y"),
            Err(UriError::NotVisionclaw(_))
        ));
        // A malformed converged id still surfaces its specific error, not a downgrade.
        assert!(matches!(
            parse_dual("urn:visionclaw:bogus:whatever"),
            Err(UriError::UnknownKind(_))
        ));
    }

    #[test]
    fn bc20_crosses_agentbox_kinds_per_closed_map() {
        // agent → did:nostr (identity, structural round-trip)
        let c = cross_from_agentbox(&format!("urn:agentbox:agent:{PK_A}:planner")).unwrap();
        assert_eq!(c.visionclaw_id, format!("did:nostr:{PK_A}"));
        assert_eq!(c.owner_did.as_deref(), Some(&*format!("did:nostr:{PK_A}")));

        // thing → kg (owner-scoped, content-addressed)
        let c = cross_from_agentbox(&format!("urn:agentbox:thing:{PK_A}:proposal-7")).unwrap();
        assert!(c.visionclaw_id.starts_with(&format!("urn:visionclaw:kg:{PK_A}:sha256-12-")));
        assert!(parse(&c.visionclaw_id).is_ok());

        // activity → execution (unscoped)
        let c = cross_from_agentbox(&format!("urn:agentbox:activity:{PK_A}:run-3")).unwrap();
        assert!(c.visionclaw_id.starts_with("urn:visionclaw:execution:sha256-12-"));
        assert_eq!(c.owner_did.as_deref(), Some(&*format!("did:nostr:{PK_A}")));

        // already-converged did:nostr passes through unchanged
        let did = format!("did:nostr:{PK_A}");
        let c = cross_from_agentbox(&did).unwrap();
        assert_eq!(c.visionclaw_id, did);

        // memory→concept (no elevation target on the hot path) is unmapped → None
        assert!(cross_from_agentbox(&format!("urn:agentbox:memory:{PK_A}:lesson-x")).is_none());
        // unknown kind → None (closed map)
        assert!(cross_from_agentbox(&format!("urn:agentbox:credential:{PK_A}:vc-1")).is_none());
        // not a foreign agentbox urn → None
        assert!(cross_from_agentbox("urn:ngm:node:1").is_none());
    }

    #[test]
    fn matches_agentbox_kg_target_urn_fixture() {
        // The schema.rs cross-repo fixture carries this exact target_urn shape;
        // it must parse cleanly as a kg node on the VisionClaw side.
        let fixture =
            "urn:visionclaw:kg:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:sha256-12-deadbeef0011";
        let p = parse(fixture).unwrap();
        assert_eq!(p.kind(), Some(Kind::Kg));
        assert_eq!(p.owner_pubkey(), Some(PK_B));
        assert_eq!(p.to_uri(), fixture);
    }
}
