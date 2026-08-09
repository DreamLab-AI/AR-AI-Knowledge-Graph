//! Layer 1 — **Reducer (Contract)**: the pure, deterministic state machine.
//!
//! ADR-124 build-out §2 layer 1. The Carvalho-lineage reducer is the public,
//! deterministic `validate()` + `transition()` pair (`validate.js` +
//! `ledger.js settle()` in the worldcup reference). This is the VisionClaw
//! `ContractReducer` trait: integer-only arithmetic, no I/O, no clock, no RNG —
//! so the same bytes in produce the same bytes out, on any host, and a published
//! `verify` can replay it (§2.4).
//!
//! ## Determinism contract
//!
//! `transition` MUST be a pure function of `(state, event)`. The reducer plugs
//! into the hash-chain link check (the verbatim engine in the solid-pod-rs
//! substrate, `verify_state_link`): the canonical JCS serialisation of the
//! post-state is what gets hashed into the trail. Re-running `transition` from
//! `genesis` over the recorded event log MUST reproduce the stored canonical
//! state byte-for-byte (ADR-124 R1 — the byte-parity golden test is the headline
//! risk and is asserted here).
//!
//! ## Invariant boundary
//!
//! The reducer never reads identity from anything other than an opaque
//! `did:nostr:<hex>` string carried on an event. It never parses a verification
//! method (I3). It changes no key bytes (I1/I2). ADR-074 §D1 stays (I4).

use serde::Serialize;
use std::fmt::Debug;

/// A validation finding produced by [`ContractReducer::validate`]. Pure data —
/// no side effects, no logging — so the reducer stays replayable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReducerError {
    /// Stable machine-readable code (e.g. `negative_balance`).
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

impl ReducerError {
    /// Construct a finding.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// The error returned when [`ContractReducer::transition`] refuses an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TransitionError {
    /// The pre-state failed [`ContractReducer::validate`].
    InvalidPreState(Vec<ReducerError>),
    /// The event is not applicable to the current state.
    Rejected {
        /// Stable machine-readable code.
        code: String,
        /// Human-readable explanation.
        message: String,
    },
}

/// A pure, deterministic web-contract reducer (Layer 1).
///
/// Implementors MUST keep `validate` and `transition` free of I/O, clocks, and
/// randomness. The state and event types must serialise canonically (JCS,
/// RFC-8785) so that the trail hash-chain and the published verifier agree
/// byte-for-byte.
pub trait ContractReducer {
    /// The contract state — the reduced value persisted to the State layer.
    type State: Clone + Debug + Serialize;
    /// The event — one applied input.
    type Event: Clone + Debug + Serialize;

    /// The genesis (initial) state. Deterministic; no inputs.
    fn genesis(&self) -> Self::State;

    /// Validate a state, returning all findings (empty == valid). Pure.
    fn validate(&self, state: &Self::State) -> Vec<ReducerError>;

    /// Apply one event to a state, returning the post-state. Pure and total
    /// except for explicit [`TransitionError`] refusals. The implementation MUST
    /// reject (not panic) on an inapplicable event.
    fn transition(
        &self,
        state: &Self::State,
        event: &Self::Event,
    ) -> Result<Self::State, TransitionError>;

    /// Replay an ordered event log from `genesis`, validating the pre-state of
    /// every step. This is the core of the `verify` ritual (§2.4 step 1): the
    /// returned state must equal the trail's stored canonical state.
    fn replay(&self, events: &[Self::Event]) -> Result<Self::State, TransitionError> {
        let mut state = self.genesis();
        for event in events {
            let errs = self.validate(&state);
            if !errs.is_empty() {
                return Err(TransitionError::InvalidPreState(errs));
            }
            state = self.transition(&state, event)?;
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    /// A minimal integer-only parimutuel-style reducer used to exercise the
    /// determinism + replay contract (the worldcup reference is a parimutuel
    /// pool). Pure: no clock, no RNG, no I/O.
    #[derive(Clone, Debug, Serialize, PartialEq, Eq)]
    struct PoolState {
        /// Total staked, in integer sats (no floats — replay-stable).
        pot_sats: u64,
        /// Number of accepted entries.
        entries: u32,
        /// Owner attribution — an opaque `did:nostr:<hex>` string (I1: never
        /// parsed for a key, never re-encoded).
        owner_did: String,
    }

    #[derive(Clone, Debug, Serialize)]
    enum PoolEvent {
        Stake { sats: u64 },
        Close,
    }

    struct Pool {
        owner_did: String,
    }

    impl ContractReducer for Pool {
        type State = PoolState;
        type Event = PoolEvent;

        fn genesis(&self) -> PoolState {
            PoolState {
                pot_sats: 0,
                entries: 0,
                owner_did: self.owner_did.clone(),
            }
        }

        fn validate(&self, state: &PoolState) -> Vec<ReducerError> {
            let mut errs = Vec::new();
            // Owner attribution must remain the genesis did:nostr string (I1).
            if state.owner_did != self.owner_did {
                errs.push(ReducerError::new("owner_drift", "owner_did changed"));
            }
            errs
        }

        fn transition(
            &self,
            state: &PoolState,
            event: &PoolEvent,
        ) -> Result<PoolState, TransitionError> {
            match event {
                PoolEvent::Stake { sats } => {
                    let pot_sats = state.pot_sats.checked_add(*sats).ok_or_else(|| {
                        TransitionError::Rejected {
                            code: "overflow".into(),
                            message: "pot overflow".into(),
                        }
                    })?;
                    Ok(PoolState {
                        pot_sats,
                        entries: state.entries + 1,
                        ..state.clone()
                    })
                }
                PoolEvent::Close => Ok(state.clone()),
            }
        }
    }

    #[test]
    fn replay_is_deterministic_and_byte_stable() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let log = vec![
            PoolEvent::Stake { sats: 1000 },
            PoolEvent::Stake { sats: 2000 },
            PoolEvent::Close,
        ];

        let a = pool.replay(&log).unwrap();
        let b = pool.replay(&log).unwrap();
        assert_eq!(a, b, "replay must be deterministic");
        assert_eq!(a.pot_sats, 3000);
        assert_eq!(a.entries, 2);

        // Byte-parity (ADR-124 R1): canonical JSON of two independent replays
        // is byte-identical.
        let ja = serde_json::to_vec(&a).unwrap();
        let jb = serde_json::to_vec(&b).unwrap();
        assert_eq!(ja, jb, "canonical serialisation must be byte-identical");
    }

    #[test]
    fn transition_rejects_overflow_without_panicking() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let state = PoolState {
            pot_sats: u64::MAX,
            entries: 0,
            owner_did: "did:nostr:aa".into(),
        };
        let err = pool
            .transition(&state, &PoolEvent::Stake { sats: 1 })
            .unwrap_err();
        assert!(matches!(err, TransitionError::Rejected { .. }));
    }

    #[test]
    fn validate_flags_owner_drift_i1_guard() {
        let pool = Pool {
            owner_did: "did:nostr:aa".into(),
        };
        let drifted = PoolState {
            pot_sats: 0,
            entries: 0,
            owner_did: "did:nostr:bb".into(),
        };
        let errs = pool.validate(&drifted);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, "owner_drift");
    }
}
