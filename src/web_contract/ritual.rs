//! The deploy ritual + the `verify` audit (ADR-124 build-out §2.3/§2.4).
//!
//! ## The deploy ritual (verbatim shape, reconstructed impl)
//!
//! `edit → validate → commit → git-mark → push; verify` — adopted from the
//! Carvalho deploy ritual. The `gitmark.json` step is the only verbatim
//! artefact (C7); the `validate-cli` 3-gate registry and the `ship`/`verify`
//! flow are reconstructed per the webcontracts.org reference shape (C6), not
//! lifted from a fetchable create-agent artefact.
//!
//! | Step      | This impl |
//! |-----------|-----------|
//! | edit      | LDP write to the pod (WAC/402-gated) — existing substrate |
//! | validate  | [`super::reducer::ContractReducer::validate`] + the 3-gate [`Checks`] registry |
//! | commit    | git commit capturing the SHA + `agent_did` (ADR-125 `did:nostr`) author |
//! | git-mark  | emit the verbatim [`super::trail::GitMark`] (C7 five-key envelope) |
//! | anchor    | BIP-341 taproot tx (the existing Bitcoin write-side substrate) |
//! | push      | git push |
//! | verify    | [`verify`] below |
//!
//! ## The `verify` audit (§2.4)
//!
//! Given a contract package, [`verify`]:
//!   1. **recomputes the reducer** — replays `transition()` from `genesis` over
//!      the event log; asserts the stored canonical state hash == the replay;
//!   2. **replays the ledger** — recomputes balances from the reducer output;
//!      asserts the stored `ledger.json` == the replay;
//!   3. **asserts git-clean** — the working tree matches the last `gitmark.json`
//!      commit SHA;
//!   4. **confirms the trail tip is a confirmed tx** — the last `txo[]` entry is
//!      a confirmed anchor, and (L1 seam) the prior chained-key prevout was spent
//!      exactly once (the single-use-seal close).
//!
//! ## Trust model (ADR-124 §4) — honest-or-caught → single-use-seal → trustless
//!
//! [`TrustLevel`] is the on-seal immutable commitment and the capability gate.
//!   * `L0` (honest-or-caught): public pure reducer + published verifier +
//!     per-state block-anchor + oracle ACL = operator `did:nostr`.
//!   * `L1` (single-use-seal): anchor becomes a true spent-exactly-once seal.
//!   * `L2`/`L3` (RGB/DLC trustless): **HARD-REFUSED** by [`TrustLevel::gate`]
//!     until the adaptor-sig CET engine is built AND independently audited.
//!
//! ## Invariant boundary
//!
//! The whole ritual is identity-rail-agnostic. The `agent_did` author is the
//! ADR-125 `did:nostr:<hex>` string, carried unchanged (I1). Nothing here parses
//! a verification method (I3) or re-encodes a key (I2). ADR-074 §D1 stays (I4).

use serde::Serialize;

use super::ledger::Ledger;
use super::reducer::{ContractReducer, ReducerError, TransitionError};
use super::state::CanonicalState;
use super::trail::Blocktrails;

/// The trust spectrum + capability gate (ADR-124 §4 / build-out §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// honest-or-caught: public reducer + published verifier + block-anchor.
    L0HonestOrCaught,
    /// single-use-seal: anchor confirmed spent-exactly-once.
    L1SingleUseSeal,
    /// RGB/DLC trustless endgame (adaptor-sig CET) — gated off.
    L2AdaptorSigCet,
    /// RGB consignment trustless endgame — gated off.
    L3Rgb,
}

impl TrustLevel {
    /// The capability gate: a hard pre-condition on `transition()` commit.
    ///
    /// `L0`/`L1` are available. `L2`/`L3` are **HARD-REFUSED** until the
    /// adaptor-sig CET engine is built and independently audited (ADR-124 §2.4 /
    /// R3). The upgrade seam is the single-use-seal chain: `L0 → L1` is in-place
    /// over the same chain; `L2 → L3` is a layer rewrite.
    pub fn gate(self) -> Result<(), &'static str> {
        match self {
            TrustLevel::L0HonestOrCaught | TrustLevel::L1SingleUseSeal => Ok(()),
            TrustLevel::L2AdaptorSigCet | TrustLevel::L3Rgb => Err(
                "trustless (RGB/DLC) trust levels are hard-refused until the \
                 adaptor-signature CET engine is built and independently audited \
                 (ADR-124 §2.4 R3)",
            ),
        }
    }

    /// True iff this level is currently deployable.
    pub fn is_available(self) -> bool {
        self.gate().is_ok()
    }
}

/// The 3-gate CHECKS registry (port of `validate-cli.js`, C6 reconstruction).
///
/// One schema, three gates — browser/wasm, `ship`, and CI — all running the same
/// pure validation so the answer cannot diverge across environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// In-browser / wasm validation (the author's edit-time check).
    BrowserWasm,
    /// The `ship` ritual gate (pre-commit).
    Ship,
    /// The CI gate (post-push).
    Ci,
}

/// Runs the single reducer-validate across all three gates and reports any
/// divergence — the determinism guarantee the three-gate registry exists to
/// protect.
pub struct Checks;

impl Checks {
    /// All three gates.
    pub const GATES: [Gate; 3] = [Gate::BrowserWasm, Gate::Ship, Gate::Ci];

    /// Validate a state at all three gates. Because the reducer is pure, every
    /// gate must return identical findings; this asserts that and returns the
    /// (single) finding set. A divergence is a determinism bug and is surfaced as
    /// an `Err`.
    pub fn run_all<R: ContractReducer>(
        reducer: &R,
        state: &R::State,
    ) -> Result<Vec<ReducerError>, &'static str> {
        let mut prior: Option<Vec<ReducerError>> = None;
        for _gate in Self::GATES {
            let findings = reducer.validate(state);
            match &prior {
                None => prior = Some(findings),
                Some(p) if *p != findings => {
                    return Err("reducer validation diverged across gates (non-deterministic)")
                }
                Some(_) => {}
            }
        }
        Ok(prior.unwrap_or_default())
    }
}

/// A confirmed-anchor probe — the `verify` ritual's notary-clock check (§2.4
/// step 4). The substrate's Bitcoin verify-side (`verify_mrc20_anchor` upstream)
/// is the production implementation; this trait is the seam VisionClaw consumes
/// so the verifier is testable without a live node.
pub trait AnchorConfirmer {
    /// True iff the given `(txid, vout)` is a confirmed UTXO.
    fn is_confirmed(&self, txid: &str, vout: u32) -> bool;

    /// True iff the prior chained-key prevout was spent **exactly once** — the
    /// L1 single-use-seal close (ADR-124 R2). At L0 this is not required.
    fn prevout_spent_once(&self, txid: &str, vout: u32) -> bool;
}

/// The outcome of the [`verify`] audit (§2.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyReport {
    /// Step 1 — replayed reducer state hash matched the stored hash.
    pub reducer_replay_ok: bool,
    /// Step 2 — replayed ledger matched the stored ledger.
    pub ledger_replay_ok: bool,
    /// Step 3 — working tree matched the last git-mark commit SHA.
    pub git_clean_ok: bool,
    /// Step 4 — trail tip is a confirmed tx (and, at L1, a closed single-use seal).
    pub trail_tip_confirmed_ok: bool,
}

impl VerifyReport {
    /// True iff every gate passed.
    pub fn is_valid(&self) -> bool {
        self.reducer_replay_ok
            && self.ledger_replay_ok
            && self.git_clean_ok
            && self.trail_tip_confirmed_ok
    }
}

/// Inputs to the [`verify`] audit: the stored artefacts to check the replay
/// against.
pub struct VerifyInput<'a, R: ContractReducer> {
    /// The reducer (pure contract).
    pub reducer: &'a R,
    /// The recorded, ordered event log.
    pub events: &'a [R::Event],
    /// The stored canonical state hash (from the State layer / trail).
    pub stored_state_hash: &'a str,
    /// The stored `ledger.json` (recomputed and compared).
    pub stored_ledger: &'a Ledger,
    /// A pure projection from the replayed reducer state to the expected ledger.
    pub project_ledger: &'a dyn Fn(&R::State) -> Ledger,
    /// The trail being verified.
    pub trail: &'a Blocktrails,
    /// True iff the working tree matches the last git-mark commit SHA.
    pub git_clean: bool,
    /// The declared trust level (gates capability + sets the seal check).
    pub trust_level: TrustLevel,
    /// The confirmed-anchor probe.
    pub confirmer: &'a dyn AnchorConfirmer,
}

/// Run the `verify` audit (§2.4): recompute the reducer, replay the ledger,
/// assert git-clean, confirm the trail tip.
///
/// Returns `Err` if the trust level is hard-refused (capability gate). Otherwise
/// a [`VerifyReport`] with one bit per audit step.
pub fn verify<R: ContractReducer>(input: VerifyInput<'_, R>) -> Result<VerifyReport, &'static str> {
    // Capability gate first — a hard-refused trust level never verifies.
    input.trust_level.gate()?;

    // Step 1: recompute the reducer and compare the canonical state hash.
    let reducer_replay_ok = match input.reducer.replay(input.events) {
        Ok(state) => CanonicalState::from_state(&state)
            .map(|cs| cs.matches(input.stored_state_hash))
            .unwrap_or(false),
        Err(_) => false,
    };

    // Step 2: replay the ledger from the (re)computed reducer state.
    let ledger_replay_ok = match input.reducer.replay(input.events) {
        Ok(state) => (input.project_ledger)(&state) == *input.stored_ledger,
        Err(_) => false,
    };

    // Step 3: git-clean assertion (caller-supplied; the substrate's
    // ShellGitMarker computes it from the working tree vs the last mark SHA).
    let git_clean_ok = input.git_clean;

    // Step 4: trail tip is a confirmed tx. At L1 also require the single-use-seal
    // close (prevout spent exactly once).
    let trail_tip_confirmed_ok = match input.trail.tip() {
        Some(tip) => {
            let confirmed = input.confirmer.is_confirmed(&tip.txid, tip.vout);
            let seal_ok = match input.trust_level {
                TrustLevel::L1SingleUseSeal => {
                    input.confirmer.prevout_spent_once(&tip.txid, tip.vout)
                }
                _ => true, // L0: notary-clock UTXO-exists is sufficient.
            };
            input.trail.is_well_formed() && confirmed && seal_ok
        }
        None => false,
    };

    Ok(VerifyReport {
        reducer_replay_ok,
        ledger_replay_ok,
        git_clean_ok,
        trail_tip_confirmed_ok,
    })
}

/// The transition-commit capability gate (build-out §3): a hard pre-condition on
/// committing a `transition()` result. Refuses if the trust level is hard-refused.
/// `substrate_disabled` hard-disables money-moving transitions (`.swap`/`.pool`/
/// `.withdraw`/cash-out) until trust-level AND owner+legal sign-off authorise
/// (ADR-124 §7 R5).
pub fn commit_gate(
    trust_level: TrustLevel,
    moves_money: bool,
    substrate_disabled: bool,
) -> Result<(), TransitionError> {
    trust_level.gate().map_err(|m| TransitionError::Rejected {
        code: "trust_level_refused".into(),
        message: m.into(),
    })?;
    if moves_money && substrate_disabled {
        return Err(TransitionError::Rejected {
            code: "substrate_disabled".into(),
            message: "money-moving transitions are disabled until trust-level AND \
                      owner+legal Judgment-Broker sign-off authorise (ADR-124 §7 R5)"
                .into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_contract::ledger::LedgerEntry;
    use crate::web_contract::reducer::ContractReducer;
    use crate::web_contract::trail::{Blocktrails, TxOut};
    use serde::Serialize;

    #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
    struct PoolState {
        pot_sats: u64,
        owner_did: String,
    }

    #[derive(Clone, Debug, Serialize)]
    struct Stake {
        sats: u64,
    }

    struct Pool {
        owner_did: String,
    }

    impl ContractReducer for Pool {
        type State = PoolState;
        type Event = Stake;
        fn genesis(&self) -> PoolState {
            PoolState {
                pot_sats: 0,
                owner_did: self.owner_did.clone(),
            }
        }
        fn validate(&self, _s: &PoolState) -> Vec<ReducerError> {
            Vec::new()
        }
        fn transition(&self, s: &PoolState, e: &Stake) -> Result<PoolState, TransitionError> {
            Ok(PoolState {
                pot_sats: s.pot_sats + e.sats,
                owner_did: s.owner_did.clone(),
            })
        }
    }

    struct AlwaysConfirmed;
    impl AnchorConfirmer for AlwaysConfirmed {
        fn is_confirmed(&self, _t: &str, _v: u32) -> bool {
            true
        }
        fn prevout_spent_once(&self, _t: &str, _v: u32) -> bool {
            true
        }
    }
    struct NeverSpentOnce;
    impl AnchorConfirmer for NeverSpentOnce {
        fn is_confirmed(&self, _t: &str, _v: u32) -> bool {
            true
        }
        fn prevout_spent_once(&self, _t: &str, _v: u32) -> bool {
            false
        }
    }

    fn project_ledger(s: &PoolState) -> Ledger {
        Ledger::from_balances(vec![LedgerEntry {
            account: s.owner_did.clone(),
            balance_sats: s.pot_sats as i64,
        }])
    }

    fn trail_with_tip() -> Blocktrails {
        let mut t = Blocktrails::new("tbtc4", "02abcd");
        t.push_link(
            "aa",
            TxOut {
                txid: "tip".into(),
                vout: 0,
                address: "bc1p".into(),
            },
        );
        t
    }

    #[test]
    fn full_verify_passes_at_l0() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let events = vec![Stake { sats: 1000 }, Stake { sats: 2000 }];
        let final_state = pool.replay(&events).unwrap();
        let stored_hash = CanonicalState::from_state(&final_state).unwrap().state_hash;
        let stored_ledger = project_ledger(&final_state);
        let trail = trail_with_tip();
        let confirmer = AlwaysConfirmed;

        let report = verify(VerifyInput {
            reducer: &pool,
            events: &events,
            stored_state_hash: &stored_hash,
            stored_ledger: &stored_ledger,
            project_ledger: &project_ledger,
            trail: &trail,
            git_clean: true,
            trust_level: TrustLevel::L0HonestOrCaught,
            confirmer: &confirmer,
        })
        .unwrap();

        assert!(report.is_valid(), "{report:?}");
    }

    #[test]
    fn l1_requires_single_use_seal_close() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let events = vec![Stake { sats: 1000 }];
        let final_state = pool.replay(&events).unwrap();
        let stored_hash = CanonicalState::from_state(&final_state).unwrap().state_hash;
        let stored_ledger = project_ledger(&final_state);
        let trail = trail_with_tip();
        let confirmer = NeverSpentOnce; // confirmed but NOT spent-exactly-once

        let report = verify(VerifyInput {
            reducer: &pool,
            events: &events,
            stored_state_hash: &stored_hash,
            stored_ledger: &stored_ledger,
            project_ledger: &project_ledger,
            trail: &trail,
            git_clean: true,
            trust_level: TrustLevel::L1SingleUseSeal,
            confirmer: &confirmer,
        })
        .unwrap();

        // L1 fails the seal step even though the anchor is confirmed.
        assert!(!report.trail_tip_confirmed_ok);
        assert!(!report.is_valid());
    }

    #[test]
    fn tampered_state_hash_fails_reducer_replay() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let events = vec![Stake { sats: 1000 }];
        let final_state = pool.replay(&events).unwrap();
        let stored_ledger = project_ledger(&final_state);
        let trail = trail_with_tip();
        let confirmer = AlwaysConfirmed;

        let report = verify(VerifyInput {
            reducer: &pool,
            events: &events,
            stored_state_hash: "deadbeef", // wrong
            stored_ledger: &stored_ledger,
            project_ledger: &project_ledger,
            trail: &trail,
            git_clean: true,
            trust_level: TrustLevel::L0HonestOrCaught,
            confirmer: &confirmer,
        })
        .unwrap();

        assert!(!report.reducer_replay_ok);
        assert!(!report.is_valid());
    }

    #[test]
    fn trustless_levels_are_hard_refused() {
        assert!(TrustLevel::L0HonestOrCaught.is_available());
        assert!(TrustLevel::L1SingleUseSeal.is_available());
        assert!(!TrustLevel::L2AdaptorSigCet.is_available());
        assert!(!TrustLevel::L3Rgb.is_available());

        // commit_gate refuses an L2 transition outright.
        let err = commit_gate(TrustLevel::L2AdaptorSigCet, false, false).unwrap_err();
        assert!(matches!(err, TransitionError::Rejected { .. }));
    }

    #[test]
    fn substrate_disablement_blocks_money_moves() {
        // L0, but money-moving with the substrate disabled → refused.
        let err = commit_gate(TrustLevel::L0HonestOrCaught, true, true).unwrap_err();
        match err {
            TransitionError::Rejected { code, .. } => assert_eq!(code, "substrate_disabled"),
            _ => panic!("expected Rejected"),
        }
        // Non-money transition is allowed even when disabled.
        assert!(commit_gate(TrustLevel::L0HonestOrCaught, false, true).is_ok());
        // Money move allowed when substrate enabled.
        assert!(commit_gate(TrustLevel::L0HonestOrCaught, true, false).is_ok());
    }

    #[test]
    fn three_gate_checks_agree_when_deterministic() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let findings = Checks::run_all(&pool, &pool.genesis()).unwrap();
        assert!(findings.is_empty());
    }
}
