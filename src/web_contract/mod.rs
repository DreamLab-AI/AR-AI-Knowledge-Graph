//! Web-contract substrate — the ADR-124 build-out 4-layer projection.
//!
//! ADR-124 build-out: `docs/adr/ADR-124-build-out-gitmark-blocktrails-substrate.md`.
//!
//! This module is the VisionClaw projection of the Carvalho-lineage web-contract
//! substrate. The single-substrate decision (ADR-124 §1) is adopted: there is
//! **no parallel design**. The four web-contract layers map onto the existing
//! VisionClaw partial substrate (the derived-ontology fence, the SQLite
//! enrichment store, the broker inbox, the 402 pay surface) and the upstream
//! solid-pod-rs engine (the reducer hash-chain / Bitcoin write-side).
//!
//! ## The four layers
//!
//! | Layer | Module | Carvalho artefact | Verbatim? |
//! |-------|--------|-------------------|-----------|
//! | 1 Reducer | [`reducer`] | `validate.js` + `ledger.js settle()` | reconstructed (C6) |
//! | 2 State   | [`state`]   | `data/*.json` + `schema/*` | reconstructed |
//! | 3 Ledger  | [`ledger`]  | `pool/ledger.json` (webledgers) | reconstructed (C6) |
//! | 4 Trail   | [`trail`]   | `gitmark.json` / `blocktrails.json` | **`gitmark.json` VERBATIM (C7)**; `blocktrails.json` reference shape (C6) |
//!
//! plus the deploy/audit [`ritual`] (`edit → validate → commit → git-mark →
//! push; verify`) and the trust spectrum (`L0` honest-or-caught → `L1`
//! single-use-seal → `L2`/`L3` trustless, hard-refused until audited).
//!
//! ## Verbatim discipline (C6/C7)
//!
//! Only the `gitmark.json` envelope is byte-verifiable against the create-agent
//! lineage (`microfed/gitmark.json`): the five keys `@id`, `genesis`, `nick`,
//! `package`, `repository` — and **nothing else** (no `@context`, `@type`,
//! `commit`, or `parent`). Everything else (`blocktrails.json`, the
//! validate/ship/verify flow) is reconstructed from the webcontracts.org
//! reference shape, and is labelled as such throughout — never "verbatim".
//!
//! ## Invariant boundary (I1–I4 hold trivially)
//!
//! The whole substrate is **identity-rail-agnostic above the ADR-125 `did:nostr`
//! Multikey layer**. It adds JSON-LD envelopes, a reducer trait, a ledger
//! projection, a trail, a verifier, a trust gate, and a substrate-disablement
//! flag — **none of which parse or re-encode the DID-document verification
//! method**, none of which read identity from anything other than the verified
//! `did:nostr:<hex>` string carried as opaque metadata.
//!
//!   * **I1** — no identity string changes; `did:nostr:<hex>` is carried unchanged.
//!   * **I2** — no key bytes are touched; nothing here encodes `publicKeyMultibase`.
//!   * **I3** — no auth path reads the verification method; auth stays NIP-98
//!     Schnorr over the raw event pubkey, untouched by this module.
//!   * **I4** — ADR-074 §D1 (x-only hex canonical identity) stays.

pub mod ledger;
pub mod reducer;
pub mod ritual;
pub mod state;
pub mod trail;

pub use ledger::{Ledger, LedgerEntry, SATS_PER_SHARE};
pub use reducer::{ContractReducer, ReducerError, TransitionError};
pub use ritual::{
    commit_gate, verify, AnchorConfirmer, Checks, Gate, TrustLevel, VerifyInput, VerifyReport,
};
pub use state::CanonicalState;
pub use trail::{Blocktrails, GitMark, GitMarkId, TxOut};

/// An assembled web-contract: the trail's git-mark identity plus the trust level
/// it commits to. This is the on-pod aggregate that the [`ritual`] anchors and
/// the [`ritual::verify`] audit replays.
///
/// `gitmark` is the verbatim five-key [`GitMark`] (C7); `trust_level` is the
/// on-seal immutable commitment (ADR-124 §4). The reducer/state/ledger live in
/// their own layers and are threaded through [`ritual::verify`] at audit time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebContract {
    /// The verbatim `gitmark.json` envelope (genesis or marked).
    pub gitmark: GitMark,
    /// The single-use-seal trail (`blocktrails.json` reference shape).
    pub trail: Blocktrails,
    /// The on-seal trust commitment (gates capability).
    pub trust_level: TrustLevel,
}

impl WebContract {
    /// Assemble a contract from a verbatim git-mark + trail at a trust level.
    /// Rejects a hard-refused (`L2`/`L3`) trust level at construction time so a
    /// non-deployable contract can never be assembled.
    pub fn new(
        gitmark: GitMark,
        trail: Blocktrails,
        trust_level: TrustLevel,
    ) -> Result<Self, &'static str> {
        trust_level.gate()?;
        Ok(Self { gitmark, trail, trust_level })
    }

    /// The `gitmark:<sha>:<vout>` `@id` of this contract's trail head.
    pub fn at_id(&self) -> &str {
        &self.gitmark.at_id
    }

    /// The genesis git-mark `@id` (the contract's stable identity across marks).
    pub fn genesis_id(&self) -> &str {
        &self.gitmark.genesis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "09689e988a2630e6904e6f53ddd6e1ab2f823b77ab0b160b4f98442cedb3e68c";

    #[test]
    fn assembles_a_contract_referencing_gitmark_at_id() {
        let id = GitMarkId::new(SHA, 0);
        let gitmark = GitMark::genesis(&id, "worldcup", "./pool.json", "./");
        let trail = Blocktrails::new("tbtc4", "02abcd");
        let contract = WebContract::new(gitmark, trail, TrustLevel::L0HonestOrCaught).unwrap();

        assert_eq!(contract.at_id(), format!("gitmark:{SHA}:0"));
        assert_eq!(contract.genesis_id(), format!("gitmark:{SHA}:0"));
    }

    #[test]
    fn cannot_assemble_a_hard_refused_trust_level() {
        let id = GitMarkId::new(SHA, 0);
        let gitmark = GitMark::genesis(&id, "worldcup", "./pool.json", "./");
        let trail = Blocktrails::new("tbtc4", "02abcd");
        assert!(WebContract::new(gitmark, trail, TrustLevel::L2AdaptorSigCet).is_err());
    }
}
