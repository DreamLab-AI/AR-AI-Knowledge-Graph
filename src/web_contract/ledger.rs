//! Layer 3 — **Ledger**: the `ledger.json` envelope.
//!
//! ADR-124 build-out §2 layer 3. The Carvalho-lineage ledger is
//! `pool/ledger.json` carrying `@context https://w3id.org/webledgers`, with the
//! reference unit `1 share = 1000 sats`. The VisionClaw substrate already has a
//! Web-Ledger credit/debit engine (in the solid-pod-rs `payments::WebLedger`
//! upstream and mirrored on the VisionClaw 402 surface in `pay_handler`); this
//! layer is the **serialization envelope** over that engine, not a second money
//! model (ADR-124 §1, "no parallel design").
//!
//! The ledger is a pure projection of the [`super::reducer::ContractReducer`]
//! output: the `verify` ritual recomputes balances from the reducer and asserts
//! the stored `ledger.json` equals the replay (§2.4 step 2). Integer sats only —
//! no floats — so the replay is byte-stable.
//!
//! ## Invariant boundary
//!
//! Account identifiers are opaque `did:nostr:<hex>` strings (I1). Nothing here
//! parses a verification method (I3) or touches key bytes (I2).

use serde::{Deserialize, Serialize};

/// The webledgers reference: 1 share = 1000 sats (ADR-124 §2 ledger row).
pub const SATS_PER_SHARE: u64 = 1000;

/// One ledger entry — a balance for one `did:nostr` account, in integer sats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// The account — an opaque `did:nostr:<hex>` string (I1: never parsed).
    pub account: String,
    /// Balance in integer sats (no floats — replay-stable).
    pub balance_sats: i64,
}

/// The `ledger.json` envelope (`@context https://w3id.org/webledgers`).
///
/// A deterministic projection of reducer output. `balances` is kept sorted by
/// `account` so the canonical serialisation is stable for the `verify` replay
/// equality check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// `@context` — fixed `https://w3id.org/webledgers`.
    #[serde(rename = "@context")]
    pub context: String,
    /// Per-account balances, sorted by `account` (canonical ordering).
    pub balances: Vec<LedgerEntry>,
}

impl Ledger {
    /// The fixed webledgers context IRI.
    pub const CONTEXT: &'static str = "https://w3id.org/webledgers";

    /// Build a ledger from `(account, balance_sats)` pairs. The entries are
    /// sorted by account so two ledgers with the same content serialise to the
    /// same bytes (required for the `verify` replay equality assertion).
    pub fn from_balances(mut entries: Vec<LedgerEntry>) -> Self {
        entries.sort_by(|a, b| a.account.cmp(&b.account));
        Self {
            context: Self::CONTEXT.to_string(),
            balances: entries,
        }
    }

    /// Convert a share count to integer sats (1 share = [`SATS_PER_SHARE`]).
    pub fn shares_to_sats(shares: u64) -> u64 {
        shares.saturating_mul(SATS_PER_SHARE)
    }

    /// The balance for an account, or 0 if absent.
    pub fn balance_of(&self, account: &str) -> i64 {
        self.balances
            .iter()
            .find(|e| e.account == account)
            .map(|e| e.balance_sats)
            .unwrap_or(0)
    }

    /// Sum of all balances — a conservation check the verifier can assert
    /// against the reducer's total pot.
    pub fn total_sats(&self) -> i64 {
        self.balances.iter().map(|e| e.balance_sats).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn ledger_emits_webledgers_context_and_sorted_balances() {
        let ledger = Ledger::from_balances(vec![
            LedgerEntry {
                account: "did:nostr:bb".into(),
                balance_sats: 2000,
            },
            LedgerEntry {
                account: "did:nostr:aa".into(),
                balance_sats: 1000,
            },
        ]);

        // Sorted canonical order (aa before bb).
        assert_eq!(ledger.balances[0].account, "did:nostr:aa");
        assert_eq!(ledger.balances[1].account, "did:nostr:bb");

        let v: Value = serde_json::to_value(&ledger).unwrap();
        assert_eq!(v["@context"], "https://w3id.org/webledgers");
        assert_eq!(ledger.total_sats(), 3000);
        assert_eq!(ledger.balance_of("did:nostr:aa"), 1000);
        assert_eq!(ledger.balance_of("did:nostr:missing"), 0);
    }

    #[test]
    fn shares_convert_at_one_thousand_sats() {
        assert_eq!(Ledger::shares_to_sats(3), 3000);
        assert_eq!(SATS_PER_SHARE, 1000);
    }

    #[test]
    fn equal_content_serialises_byte_identically() {
        // Same balances built in different input order must serialise identically
        // (the verify replay equality check depends on this).
        let a = Ledger::from_balances(vec![
            LedgerEntry {
                account: "did:nostr:bb".into(),
                balance_sats: 2000,
            },
            LedgerEntry {
                account: "did:nostr:aa".into(),
                balance_sats: 1000,
            },
        ]);
        let b = Ledger::from_balances(vec![
            LedgerEntry {
                account: "did:nostr:aa".into(),
                balance_sats: 1000,
            },
            LedgerEntry {
                account: "did:nostr:bb".into(),
                balance_sats: 2000,
            },
        ]);
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
    }
}
