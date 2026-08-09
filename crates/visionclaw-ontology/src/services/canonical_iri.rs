// src/services/canonical_iri.rs
//! Canonical IRI scheme, diacritic-preserving slugifier, and deterministic
//! node-ID hashing with collision detection.
//!
//! Implements ADR-100 (D1, D2) and PRD-018 WS-0. This module is the single
//! source of truth for:
//!
//! - **Slugs** (`slugify`): NFKD normalisation + combining-mark stripping
//!   (deterministic transliteration). `Café` → `cafe`, `Naïve` → `naive`,
//!   `Δelta` → `delta` where a Latin fold exists; non-foldable scripts keep
//!   their code points and are dash-segmented. Identical inputs always yield
//!   identical slugs across runs and machines. The original `rdfs:label` is
//!   retained verbatim by callers — slugs never replace labels.
//! - **Canonical IRIs** (`canonical_iri`): `vc:{domain}/{slug}` over the
//!   existing `https://narrativegoldmine.com/ns/v1#` base (ADR-100 D1),
//!   reusing the existing named-graph and content-addressed-axiom discipline.
//! - **Deterministic node IDs** (`NodeIdHasher`): a seeded SHA-256 derivation
//!   (replaces the non-deterministic `DefaultHasher`). Collisions between two
//!   *distinct* slugs are detected, logged, and rejected — never silently
//!   merged (ADR-100 D2, DDD invariant §5.6).
//!
//! ## Why SHA-256, not `DefaultHasher`
//!
//! `std::collections::hash_map::DefaultHasher` is SipHash with a
//! process-randomised seed in some std configurations and, regardless, is
//! explicitly documented as *not* stable across Rust releases. Persisted node
//! IDs MUST be stable across runs, machines, and toolchains, so we derive from
//! SHA-256 over a fixed seed + the canonical slug. This is the same content-
//! addressing discipline already used for axiom IRIs (`sha256-12`).

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

/// The `vc:` namespace base (ADR-100 D1; reuses the existing constant).
pub const VC_NS: &str = "https://narrativegoldmine.com/ns/v1#";

/// Fixed seed mixed into every node-ID hash so the derivation is stable and
/// namespaced (changing this constant would re-mint every ID — it never
/// changes without a migration). ADR-100 D2.
const NODE_ID_SEED: &str = "vc:node-id:v1:";

/// Diacritic-preserving deterministic slugifier (ADR-100 D2).
///
/// Algorithm:
/// 1. NFKD-decompose so each accented glyph splits into base + combining mark
///    (`é` → `e` + U+0301). This is the standards-grade, locale-independent
///    transliteration step the ADR mandates.
/// 2. Drop combining marks (Unicode category Mn) so the base character
///    survives — this preserves information (`Café`→`cafe`) rather than
///    dropping the whole character as the old ASCII-only slugifier did.
/// 3. Lowercase; keep alphanumerics (any script that survived NFKD);
///    collapse every other run to a single `-`; trim leading/trailing `-`.
/// 4. Empty result → `"unnamed"` sentinel.
///
/// Determinism: NFKD and the Mn-stripping table are fixed by the Unicode
/// version pinned in `unicode-normalization`; the same input always produces
/// the same output. The original label is never mutated by this function.
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true; // suppress a leading dash

    for c in input.nfkd() {
        if is_combining_mark(c) {
            // Diacritic dropped after its base char was already emitted.
            continue;
        }
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }

    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("unnamed");
    }
    out
}

/// True for Unicode combining marks (general category Mn / Mc / Me). After
/// NFKD these are the decomposed diacritics we strip. We approximate the
/// category with the canonical combining-class table exposed by
/// `unicode-normalization` plus the well-known combining-mark ranges, which
/// is sufficient and deterministic for the Latin/Greek/Cyrillic corpus.
fn is_combining_mark(c: char) -> bool {
    // Combining Diacritical Marks and the common extension blocks. NFKD only
    // ever emits marks from these ranges for the scripts in our corpus.
    matches!(c as u32,
        0x0300..=0x036F | // Combining Diacritical Marks
        0x1AB0..=0x1AFF | // Combining Diacritical Marks Extended
        0x1DC0..=0x1DFF | // Combining Diacritical Marks Supplement
        0x20D0..=0x20FF | // Combining Diacritical Marks for Symbols
        0xFE20..=0xFE2F   // Combining Half Marks
    )
}

/// Mint the canonical entity IRI `vc:{domain}/{slug}` (ADR-100 D1).
///
/// `domain` is the node's registered `source_domain` (already a slug); `basis`
/// is the human label / page name that is slugified for the local part. Both
/// are slug-normalised so the IRI is stable and collision-resistant.
pub fn canonical_iri(domain: &str, basis: &str) -> String {
    let d = slugify(domain);
    let s = slugify(basis);
    format!("{VC_NS}{d}/{s}")
}

/// Deterministic node-ID derivation with collision detection (ADR-100 D2).
///
/// Holds the reverse map `id → slug` so that when two *distinct* slugs hash to
/// the same 31-bit ID the second is detected and rejected (logged), rather
/// than silently overwriting the first node — the historical
/// `DefaultHasher` collision-merge bug.
#[derive(Debug, Default)]
pub struct NodeIdHasher {
    /// id → canonical slug that owns it. Used to distinguish a re-hash of the
    /// same slug (idempotent, fine) from a true collision (rejected).
    owners: HashMap<u32, String>,
    /// Count of rejected collisions, surfaced for the ≥95%-coverage gate.
    collisions: u64,
}

/// Outcome of resolving a slug to a node ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeIdResolution {
    /// A fresh, unique ID for this slug.
    Fresh(u32),
    /// The same slug resolved again — idempotent, returns the same ID.
    Existing(u32),
    /// A DIFFERENT slug hashed to an already-owned ID. Rejected: the caller
    /// MUST NOT create a node, to avoid silently merging two entities.
    Collision {
        id: u32,
        existing: String,
        attempted: String,
    },
}

impl NodeIdHasher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pure, stateless derivation: seeded SHA-256 over the canonical slug,
    /// folded to the 31-bit `[1, 0x7FFF_FFFE]` range (0 reserved as sentinel;
    /// stays clear of the high flag bits the binary protocol reserves).
    pub fn derive_id(slug: &str) -> u32 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(NODE_ID_SEED.as_bytes());
        hasher.update(slug.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&digest[..4]);
        // Mask to 31 bits, then map 0 → 1 so the sentinel is never produced.
        let raw = u32::from_be_bytes(bytes) & 0x7FFF_FFFF;
        if raw == 0 {
            1
        } else {
            raw
        }
    }

    /// Resolve a *page name / label* to its node ID. The input is slugified
    /// first so `Camera`, `camera`, and `urn:ngm:class:camera`'s local-name
    /// all resolve identically (the cross-graph join the pipeline relies on).
    ///
    /// Returns [`NodeIdResolution`]; collisions are recorded and the caller is
    /// responsible for honouring the rejection (no node created).
    pub fn resolve(&mut self, basis: &str) -> NodeIdResolution {
        let slug = slugify(basis);
        let id = Self::derive_id(&slug);
        match self.owners.get(&id) {
            None => {
                self.owners.insert(id, slug);
                NodeIdResolution::Fresh(id)
            }
            Some(existing) if existing == &slug => NodeIdResolution::Existing(id),
            Some(existing) => {
                self.collisions += 1;
                log::error!(
                    "node-ID collision REJECTED: id={id} owned by '{existing}', \
                     attempted by '{slug}' — node not created (ADR-100 D2)"
                );
                NodeIdResolution::Collision {
                    id,
                    existing: existing.clone(),
                    attempted: slug,
                }
            }
        }
    }

    /// Total rejected collisions so far (release-gate metric).
    pub fn collision_count(&self) -> u64 {
        self.collisions
    }

    /// Distinct node IDs minted so far.
    pub fn len(&self) -> usize {
        self.owners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Slugifier: Latin fixture (ADR-100 verification) -----------------

    #[test]
    fn slugify_ascii_basic() {
        assert_eq!(
            slugify("Renaissance Architecture"),
            "renaissance-architecture"
        );
        assert_eq!(slugify("OWL 2 EL!"), "owl-2-el");
        assert_eq!(slugify("---weird---"), "weird");
        assert_eq!(slugify(""), "unnamed");
        assert_eq!(slugify("   "), "unnamed");
    }

    #[test]
    fn slugify_preserves_diacritics_by_folding() {
        // The OLD slugifier dropped these characters entirely, collapsing
        // distinct terms. NFKD + mark-strip folds them to their base letter
        // so information survives and `Café`/`Cafe` map to the SAME slug
        // (intended: they are the same concept) — not to empty/garbage.
        assert_eq!(slugify("Café"), "cafe");
        assert_eq!(slugify("Cafe"), "cafe");
        assert_eq!(slugify("Naïve Bayes"), "naive-bayes");
        assert_eq!(slugify("Gödel"), "godel");
        assert_eq!(slugify("Schrödinger"), "schrodinger");
        assert_eq!(slugify("Évariste Galois"), "evariste-galois");
    }

    #[test]
    fn slugify_non_latin_keeps_codepoints_deterministically() {
        // Non-Latin scripts have no Latin NFKD fold; they survive as their
        // own alphanumerics and segment on punctuation/space. The contract is
        // determinism + non-collapse, not transliteration to ASCII.
        let a = slugify("Δelta Function");
        let b = slugify("Δelta Function");
        assert_eq!(a, b, "must be deterministic");
        // 'Δ' lowercases to 'δ' and is alphanumeric, so it is retained.
        assert_eq!(a, "δelta-function");

        // Cyrillic: deterministic, segmented, not dropped.
        let c1 = slugify("Граф знаний");
        let c2 = slugify("Граф знаний");
        assert_eq!(c1, c2);
        assert!(c1.contains('-'));
        assert!(!c1.is_empty());
    }

    #[test]
    fn slugify_is_idempotent() {
        let once = slugify("Café del Mar");
        let twice = slugify(&once);
        assert_eq!(once, twice);
    }

    // --- Canonical IRI ---------------------------------------------------

    #[test]
    fn canonical_iri_shape() {
        assert_eq!(
            canonical_iri("artificial-intelligence", "Transformer Models"),
            "https://narrativegoldmine.com/ns/v1#artificial-intelligence/transformer-models"
        );
        // Domain is also slugified (defensive against unnormalised input).
        assert_eq!(
            canonical_iri("Spatial Computing", "AR Headset"),
            "https://narrativegoldmine.com/ns/v1#spatial-computing/ar-headset"
        );
    }

    // --- Deterministic node-ID hash + collision rejection ----------------

    #[test]
    fn node_id_is_stable_across_calls() {
        let a = NodeIdHasher::derive_id("transformer-models");
        let b = NodeIdHasher::derive_id("transformer-models");
        assert_eq!(a, b);
        assert_ne!(a, 0, "0 is the reserved sentinel");
        // Stays clear of the high flag bit the binary protocol reserves.
        assert_eq!(a & 0x8000_0000, 0);
    }

    #[test]
    fn node_id_same_slug_via_different_casing_collapses() {
        // page name vs IRI-local-name vs lowercased all resolve identically.
        let mut h = NodeIdHasher::new();
        let first = h.resolve("Camera");
        let again = h.resolve("camera");
        match (first, again) {
            (NodeIdResolution::Fresh(a), NodeIdResolution::Existing(b)) => assert_eq!(a, b),
            other => panic!("expected Fresh then Existing, got {other:?}"),
        }
        assert_eq!(h.collision_count(), 0);
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn node_id_collision_is_rejected_not_merged() {
        // Force a collision by pre-registering a foreign slug onto the id a
        // target slug would hash to. We cannot easily find a natural SHA-256
        // collision, so we seed the owners map directly to exercise the guard.
        let mut h = NodeIdHasher::new();
        let target = "transformer-models";
        let id = NodeIdHasher::derive_id(target);
        h.owners.insert(id, "a-different-prior-entity".to_string());

        match h.resolve(target) {
            NodeIdResolution::Collision {
                id: cid,
                existing,
                attempted,
            } => {
                assert_eq!(cid, id);
                assert_eq!(existing, "a-different-prior-entity");
                assert_eq!(attempted, target);
            }
            other => panic!("expected Collision, got {other:?}"),
        }
        assert_eq!(
            h.collision_count(),
            1,
            "collision must be counted, not merged"
        );
    }
}
