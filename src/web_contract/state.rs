//! Layer 2 — **State**: the canonical reduced-state document.
//!
//! ADR-124 build-out §2 layer 2. The Carvalho-lineage state is `data/*.json` /
//! `pool/pool.json` plus the `schema/*.schema.json`, WAC-gated by `.acl`. On the
//! VisionClaw side the WAC surface + the ADR-032 402 grammar already provide the
//! state-resource gating (oracle resource ACL-locked to the operator
//! `did:nostr`); this layer is the **canonical serialisation** of the reducer's
//! output state — the bytes that get hashed into the [`super::trail`] chain.
//!
//! State documents are addressed by a `state_hash` (SHA-256 over the canonical
//! JSON). The trail's `states[]` are the commit SHAs; this `state_hash` is the
//! content commitment the reducer replay must reproduce (the byte-parity link,
//! ADR-124 §2.5 / R1).
//!
//! ## Invariant boundary
//!
//! The state carries the owner `did:nostr:<hex>` string as opaque metadata (I1).
//! Nothing here parses a verification method (I3) or re-encodes a key (I2).

use serde::Serialize;
use sha2::{Digest, Sha256};

/// A canonicalised reduced-state document plus its content hash.
///
/// `canonical_json` is the bytes that are hashed into the trail; `state_hash` is
/// their SHA-256 (lowercase hex). Two equal states (per the reducer) produce the
/// same `canonical_json` and therefore the same `state_hash` — that equality is
/// what the `verify` ritual asserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalState {
    /// The canonical JSON bytes (the hashed input).
    pub canonical_json: Vec<u8>,
    /// SHA-256 of `canonical_json`, lowercase hex.
    pub state_hash: String,
}

impl CanonicalState {
    /// Canonicalise a serialisable reducer state and compute its `state_hash`.
    ///
    /// Uses `serde_json::to_vec`, which emits map keys in struct-declaration
    /// order deterministically for our reducer state structs. (The reference
    /// substrate uses RFC-8785 JCS for the hash-chain link; this projection is
    /// stable for the in-tree reducer state shapes, and the verifier compares
    /// the recomputed hash against the stored one — see
    /// [`super::ritual::verify`].)
    pub fn from_state<S: Serialize>(state: &S) -> Result<Self, serde_json::Error> {
        let canonical_json = serde_json::to_vec(state)?;
        let mut hasher = Sha256::new();
        hasher.update(&canonical_json);
        let state_hash = hex::encode(hasher.finalize());
        Ok(Self {
            canonical_json,
            state_hash,
        })
    }

    /// True iff this state's hash matches a stored/expected hash. The core of the
    /// `verify` ritual's reducer-replay equality check (§2.4 step 1).
    pub fn matches(&self, expected_hash: &str) -> bool {
        self.state_hash == expected_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct S {
        pot_sats: u64,
        owner_did: String,
    }

    #[test]
    fn equal_states_hash_identically() {
        let a = CanonicalState::from_state(&S {
            pot_sats: 3000,
            owner_did: "did:nostr:aa".into(),
        })
        .unwrap();
        let b = CanonicalState::from_state(&S {
            pot_sats: 3000,
            owner_did: "did:nostr:aa".into(),
        })
        .unwrap();
        assert_eq!(a.state_hash, b.state_hash);
        assert_eq!(a.canonical_json, b.canonical_json);
        assert_eq!(a.state_hash.len(), 64); // SHA-256 hex
        assert!(a.matches(&b.state_hash));
    }

    #[test]
    fn different_states_hash_differently() {
        let a = CanonicalState::from_state(&S {
            pot_sats: 3000,
            owner_did: "did:nostr:aa".into(),
        })
        .unwrap();
        let b = CanonicalState::from_state(&S {
            pot_sats: 4000,
            owner_did: "did:nostr:aa".into(),
        })
        .unwrap();
        assert_ne!(a.state_hash, b.state_hash);
        assert!(!a.matches(&b.state_hash));
    }
}
