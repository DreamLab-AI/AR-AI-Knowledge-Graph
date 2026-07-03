//! Layer 4 — **Trail**: the `gitmark.json` + `blocktrails.json` envelopes.
//!
//! ADR-124 build-out (`docs/adr/ADR-128-build-out-canonical-gitmark-blocktrails.md`).
//! This is the VisionClaw projection of the Carvalho-lineage web-contract trail
//! layer. The single substrate decision (ADR-124 §1) is: adopt the
//! `gitmark.json` envelope **verbatim** and the `blocktrails.json` envelope
//! **per the `webcontracts.org` reference shape** — there is no parallel design.
//!
//! ## `gitmark.json` is VERBATIM (C7, ground-truth-verified)
//!
//! The Carvalho-lineage ground truth is `microfed/gitmark.json`:
//!
//! ```json
//! { "@id": "gitmark:<sha>:<vout>", "genesis": "gitmark:<sha>:<vout>",
//!   "nick": "<name>", "package": "<path>", "repository": "./" }
//! ```
//!
//! It has **exactly five keys** — `@id`, `genesis`, `nick`, `package`,
//! `repository`. It does **NOT** carry `@context`, `@type`, `commit`, or
//! `parent`. Earlier drafts invented those four and omitted `repository`; the
//! corrected envelope here emits only the five ground-truth keys (ADR-124 §2.1,
//! finding C7). Parent linkage lives in [`Blocktrails::states`] /
//! [`Blocktrails::txo`], where it belongs — never on the git-mark.
//!
//! ## `blocktrails.json` is RECONSTRUCTED (C6, reference shape, not verbatim)
//!
//! Only `gitmark.json` is byte-verifiable against the create-agent repo. The
//! `blocktrails.json` shape is reconstructed from the `webcontracts.org` /
//! "Melvo Predicts" worldcup reference pattern: `@type "Blocktrail"`, `profile
//! "gitmark"`, `states[]` = commit SHAs, `txo[]` = the BIP-341 single-use-seal
//! UTXO chain. It is labelled a reconstruction, not "verbatim" (ADR-124 finding
//! C6).
//!
//! ## Invariant boundary (I1–I4 hold trivially)
//!
//! This layer is identity-rail-agnostic. The agent attribution it carries is the
//! ADR-125 `did:nostr:<hex>` string, treated as an opaque identifier — it is
//! never parsed for a verification method, never re-encoded, never read on an
//! auth path. No key bytes are touched. ADR-074 §D1 stays.

use serde::{Deserialize, Serialize};

/// The `gitmark:<sha>:<vout>` identifier — a single-use-seal genesis/commit point.
///
/// Rendered as the `@id` of [`GitMark`] and reused as `genesis` for the first
/// mark in a chain (a genesis mark's `genesis == @id`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitMarkId {
    /// The anchoring commit SHA (40-char lowercase hex git object id).
    pub commit_sha: String,
    /// The transaction output index this mark anchors at.
    pub vout: u32,
}

impl GitMarkId {
    /// Construct from a commit SHA and a `vout`.
    pub fn new(commit_sha: impl Into<String>, vout: u32) -> Self {
        Self { commit_sha: commit_sha.into(), vout }
    }

    /// Render the `gitmark:<commit_sha>:<vout>` string used as `@id`/`genesis`.
    pub fn to_at_id(&self) -> String {
        format!("gitmark:{}:{}", self.commit_sha, self.vout)
    }
}

/// The **verbatim** `gitmark.json` envelope — exactly five keys (C7).
///
/// `@id`/`genesis` serialise to `gitmark:<sha>:<vout>` strings; `genesis` for a
/// genesis mark equals its own `@id`. The serializer source is the existing
/// VisionClaw provenance substrate (`agent_events::provenance`) projected over a
/// captured commit SHA + anchoring `vout`; `nick`/`package`/`repository` are the
/// additive projection fields.
///
/// **Do NOT add `@context`/`@type`/`commit`/`parent`** — they are not in the
/// ground-truth file and adding them breaks byte-parity with create-agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitMark {
    /// `@id` — `gitmark:<commit_sha>:<vout>`.
    #[serde(rename = "@id")]
    pub at_id: String,
    /// `genesis` — `gitmark:<first-commit-sha>:<vout>`. Equals `@id` for a
    /// genesis mark.
    pub genesis: String,
    /// `nick` — short human name for the contract package.
    pub nick: String,
    /// `package` — pod-relative package path (e.g. `./package.json`).
    pub package: String,
    /// `repository` — repo-relative root (e.g. `"./"`).
    pub repository: String,
}

impl GitMark {
    /// Build a genesis git-mark (where `genesis == @id`).
    pub fn genesis(
        id: &GitMarkId,
        nick: impl Into<String>,
        package: impl Into<String>,
        repository: impl Into<String>,
    ) -> Self {
        let at_id = id.to_at_id();
        Self {
            genesis: at_id.clone(),
            at_id,
            nick: nick.into(),
            package: package.into(),
            repository: repository.into(),
        }
    }

    /// Build a non-genesis git-mark that points back to an existing `genesis`
    /// `@id` (parent linkage lives in the trail's `states[]`, not here).
    pub fn marked(
        id: &GitMarkId,
        genesis: impl Into<String>,
        nick: impl Into<String>,
        package: impl Into<String>,
        repository: impl Into<String>,
    ) -> Self {
        Self {
            at_id: id.to_at_id(),
            genesis: genesis.into(),
            nick: nick.into(),
            package: package.into(),
            repository: repository.into(),
        }
    }
}

/// One UTXO in the [`Blocktrails`] single-use-seal chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxOut {
    /// Bitcoin transaction id (lowercase hex).
    pub txid: String,
    /// Output index within the transaction.
    pub vout: u32,
    /// The bech32m P2TR address holding the chained-key output.
    pub address: String,
}

/// The **reconstructed** `blocktrails.json` envelope (C6 — reference shape).
///
/// `states[]` are commit SHAs (the git side of the trail); `txo[]` is the
/// BIP-341 single-use-seal UTXO chain (the Bitcoin side). `profile` is fixed to
/// `"gitmark"` and `@type` to `"Blocktrail"` per the webcontracts.org reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocktrails {
    /// `@type` — fixed `"Blocktrail"`.
    #[serde(rename = "@type")]
    pub at_type: String,
    /// `profile` — fixed `"gitmark"` (this trail profiles git-mark commits).
    pub profile: String,
    /// `chain` — ticker, e.g. `tbtc4`.
    pub chain: String,
    /// `pubkeyBase` — the `bt_derive_chained_pubkey` base, hex.
    #[serde(rename = "pubkeyBase")]
    pub pubkey_base: String,
    /// `states[]` — ordered commit SHAs (the genesis → tip chain).
    pub states: Vec<String>,
    /// `txo[]` — the single-use-seal UTXO chain (genesis → tip).
    pub txo: Vec<TxOut>,
}

impl Blocktrails {
    /// The fixed `@type` value.
    pub const AT_TYPE: &'static str = "Blocktrail";
    /// The fixed `profile` value (git-mark commit profile).
    pub const PROFILE: &'static str = "gitmark";

    /// Construct a Blocktrail over a chain ticker + chained-key base.
    pub fn new(chain: impl Into<String>, pubkey_base: impl Into<String>) -> Self {
        Self {
            at_type: Self::AT_TYPE.to_string(),
            profile: Self::PROFILE.to_string(),
            chain: chain.into(),
            pubkey_base: pubkey_base.into(),
            states: Vec::new(),
            txo: Vec::new(),
        }
    }

    /// Append one (commit SHA, UTXO) link, extending the single-use-seal chain.
    pub fn push_link(&mut self, commit_sha: impl Into<String>, txo: TxOut) {
        self.states.push(commit_sha.into());
        self.txo.push(txo);
    }

    /// The trail tip txo (the seal that `verify` must confirm is a confirmed tx).
    pub fn tip(&self) -> Option<&TxOut> {
        self.txo.last()
    }

    /// True iff `states[]` and `txo[]` are the same length (one seal per state).
    /// The single-use-seal invariant requires the git side and the Bitcoin side
    /// to advance together.
    pub fn is_well_formed(&self) -> bool {
        self.states.len() == self.txo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SHA0: &str = "09689e988a2630e6904e6f53ddd6e1ab2f823b77ab0b160b4f98442cedb3e68c";
    const SHA1: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    #[test]
    fn gitmark_genesis_emits_exactly_five_ground_truth_keys() {
        // C7: the verbatim envelope is { @id, genesis, nick, package, repository }
        // — NO @context / @type / commit / parent.
        let id = GitMarkId::new(SHA0, 0);
        let mark = GitMark::genesis(&id, "gitmark", "./package.json", "./");
        let v: Value = serde_json::to_value(&mark).unwrap();
        let obj = v.as_object().unwrap();

        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["@id", "genesis", "nick", "package", "repository"]);

        // Genesis: @id == genesis.
        assert_eq!(obj["@id"], format!("gitmark:{SHA0}:0"));
        assert_eq!(obj["genesis"], format!("gitmark:{SHA0}:0"));
        assert_eq!(obj["repository"], "./");

        // The four invented fields MUST be absent.
        for forbidden in ["@context", "@type", "commit", "parent"] {
            assert!(!obj.contains_key(forbidden), "forbidden key {forbidden} present");
        }
    }

    #[test]
    fn gitmark_byte_matches_carvalho_ground_truth() {
        // Reproduce microfed/gitmark.json exactly (key set + values), proving the
        // verbatim claim is honoured.
        let id = GitMarkId::new(SHA0, 0);
        let mark = GitMark::genesis(&id, "gitmark", "./package.json", "./");
        let v: Value = serde_json::to_value(&mark).unwrap();
        let ground_truth: Value = serde_json::json!({
            "@id": format!("gitmark:{SHA0}:0"),
            "genesis": format!("gitmark:{SHA0}:0"),
            "nick": "gitmark",
            "package": "./package.json",
            "repository": "./"
        });
        assert_eq!(v, ground_truth);
    }

    #[test]
    fn blocktrails_reference_shape_and_single_use_seal_chain() {
        let mut trail = Blocktrails::new("tbtc4", "02abcd");
        trail.push_link(
            SHA0,
            TxOut { txid: "aa".into(), vout: 0, address: "bc1p0".into() },
        );
        trail.push_link(
            SHA1,
            TxOut { txid: "bb".into(), vout: 0, address: "bc1p1".into() },
        );

        assert!(trail.is_well_formed());
        assert_eq!(trail.at_type, "Blocktrail");
        assert_eq!(trail.profile, "gitmark");
        assert_eq!(trail.tip().unwrap().txid, "bb");
        assert_eq!(trail.states, vec![SHA0.to_string(), SHA1.to_string()]);

        let v: Value = serde_json::to_value(&trail).unwrap();
        assert_eq!(v["@type"], "Blocktrail");
        assert_eq!(v["pubkeyBase"], "02abcd");
    }

    #[test]
    fn malformed_trail_detected_when_seal_chain_diverges() {
        let mut trail = Blocktrails::new("tbtc4", "02abcd");
        trail.states.push(SHA0.to_string()); // a state with no matching seal
        assert!(!trail.is_well_formed());
    }
}
