//! Owner-pubkey visibility drop-set (ADR-060, ADR-059 §Phase-4).
//!
//! Pure, framework-free implementation of the binary-position-encoder
//! visibility filter. The handler layer (`socket_flow_handler::position_updates`)
//! owns the env-flag gate (`PUBKEY_VISIBILITY_FILTER`, default OFF) and the wire
//! encoding; this module owns the load-bearing set logic so it is unit-testable
//! without linking the CUDA-bearing monolith.
//!
//! ADR-059 §6 filter clause, evaluated per caller:
//! ```text
//! KEEP  WHERE visibility = 'public' OR owner_pubkey = $session_pubkey
//! DROP  everything else  (private-of-others, or private-when-anonymous)
//! ```
//! Filtering is **fail-closed**: a missing session pubkey drops every private
//! node (public-only graph), matching ADR-050 §Visibility transitions.

use std::collections::HashSet;

/// Per-node visibility metadata (ADR-050 primitives) projected onto the wire id.
///
/// `wire_id` is the flagged/opaque id exactly as it appears in the `(id, data)`
/// pairs the encoder consumes, so a computed drop set can be matched directly
/// against those pairs with no further translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeVisibility {
    /// Flagged/opaque wire id as it appears in the `(id, data)` pair.
    pub wire_id: u32,
    /// `true` when the node is public (ADR-050 `visibility = 'public'`).
    pub is_public: bool,
    /// ADR-050 `owner_pubkey`, when known. `None` ⇒ owner unknown ⇒ treated as
    /// not-owned-by-caller (fail-closed).
    pub owner_pubkey: Option<String>,
}

impl NodeVisibility {
    /// Convenience constructor for a public node (no owner needed).
    pub fn public(wire_id: u32) -> Self {
        Self {
            wire_id,
            is_public: true,
            owner_pubkey: None,
        }
    }

    /// Convenience constructor for a private node with a known owner.
    pub fn private_owned_by(wire_id: u32, owner_pubkey: impl Into<String>) -> Self {
        Self {
            wire_id,
            is_public: false,
            owner_pubkey: Some(owner_pubkey.into()),
        }
    }

    /// Convenience constructor for a private node whose owner is unknown.
    pub fn private_unowned(wire_id: u32) -> Self {
        Self {
            wire_id,
            is_public: false,
            owner_pubkey: None,
        }
    }

    /// Whether this node must be dropped for the given caller.
    ///
    /// Keep iff public, or owned by the caller. Everything else drops.
    #[inline]
    fn is_dropped_for(&self, session_pubkey: Option<&str>) -> bool {
        if self.is_public {
            return false;
        }
        match (session_pubkey, self.owner_pubkey.as_deref()) {
            // Caller is authenticated and owns this private node ⇒ keep.
            (Some(caller), Some(owner)) => caller != owner,
            // Anonymous caller, or unknown owner ⇒ drop (fail-closed).
            _ => true,
        }
    }
}

/// Compute the caller's private-opaque-id drop set per ADR-059 §6.
///
/// Returns the set of `wire_id`s that must **not** reach the wire for a caller
/// holding `session_pubkey` (`None` ⇒ anonymous ⇒ every private node drops).
pub fn compute_private_opaque_ids(
    nodes: &[NodeVisibility],
    session_pubkey: Option<&str>,
) -> HashSet<u32> {
    nodes
        .iter()
        .filter(|n| n.is_dropped_for(session_pubkey))
        .map(|n| n.wire_id)
        .collect()
}

/// Remove every `(id, _)` pair whose id is in `drop_set`, in place.
///
/// Returns the number of pairs removed. A no-op (and no reallocation) when the
/// drop set is empty — the flag-off / nothing-to-drop hot path stays free.
pub fn apply_drop_set<T>(nodes: &mut Vec<(u32, T)>, drop_set: &HashSet<u32>) -> usize {
    if drop_set.is_empty() {
        return 0;
    }
    let before = nodes.len();
    nodes.retain(|(id, _)| !drop_set.contains(id));
    before - nodes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A stand-in for the encoder payload; the filter is generic over the data half.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Payload(u32);

    fn pairs() -> Vec<(u32, Payload)> {
        vec![
            (10, Payload(10)), // public
            (11, Payload(11)), // private, owned by "alice"
            (12, Payload(12)), // private, owned by "bob"
            (13, Payload(13)), // private, unknown owner
        ]
    }

    fn meta() -> Vec<NodeVisibility> {
        vec![
            NodeVisibility::public(10),
            NodeVisibility::private_owned_by(11, "alice"),
            NodeVisibility::private_owned_by(12, "bob"),
            NodeVisibility::private_unowned(13),
        ]
    }

    #[test]
    fn empty_drop_set_is_passthrough_untouched() {
        // Simulates the flag-OFF path: no drop set is ever computed, so the
        // encoder input must be byte-for-byte identical.
        let mut nodes = pairs();
        let original = nodes.clone();
        let empty: HashSet<u32> = HashSet::new();
        let dropped = apply_drop_set(&mut nodes, &empty);
        assert_eq!(dropped, 0);
        assert_eq!(nodes, original, "empty drop set must not mutate the vec");
    }

    #[test]
    fn flag_on_drops_private_ids_for_non_owner() {
        // Caller "alice" keeps public (10) + her own private (11); bob's (12)
        // and the unknown-owner (13) private nodes drop.
        let drop = compute_private_opaque_ids(&meta(), Some("alice"));
        assert_eq!(drop, HashSet::from([12, 13]));

        let mut nodes = pairs();
        let dropped = apply_drop_set(&mut nodes, &drop);
        assert_eq!(dropped, 2);
        let kept: Vec<u32> = nodes.iter().map(|(id, _)| *id).collect();
        assert_eq!(kept, vec![10, 11], "non-owner private nodes must be dropped");
    }

    #[test]
    fn owner_sees_own_private_nodes() {
        // "bob" keeps public (10) + his own private (12); alice's (11) and the
        // unknown-owner (13) drop.
        let drop = compute_private_opaque_ids(&meta(), Some("bob"));
        assert_eq!(drop, HashSet::from([11, 13]));

        let mut nodes = pairs();
        apply_drop_set(&mut nodes, &drop);
        let kept: Vec<u32> = nodes.iter().map(|(id, _)| *id).collect();
        assert_eq!(kept, vec![10, 12], "owner must still see their own nodes");
    }

    #[test]
    fn anonymous_caller_is_fail_closed_public_only() {
        // No session pubkey ⇒ every private node drops regardless of owner.
        let drop = compute_private_opaque_ids(&meta(), None);
        assert_eq!(drop, HashSet::from([11, 12, 13]));

        let mut nodes = pairs();
        apply_drop_set(&mut nodes, &drop);
        let kept: Vec<u32> = nodes.iter().map(|(id, _)| *id).collect();
        assert_eq!(kept, vec![10], "anonymous callers get a public-only graph");
    }

    #[test]
    fn all_public_yields_empty_drop_set() {
        let all_public = vec![
            NodeVisibility::public(1),
            NodeVisibility::public(2),
            NodeVisibility::public(3),
        ];
        assert!(compute_private_opaque_ids(&all_public, None).is_empty());
        assert!(compute_private_opaque_ids(&all_public, Some("anyone")).is_empty());
    }

    #[test]
    fn drop_set_ids_absent_from_pairs_are_ignored() {
        // A drop set carrying ids not present in the frame removes nothing extra.
        let mut nodes = pairs();
        let drop = HashSet::from([99, 100]);
        let dropped = apply_drop_set(&mut nodes, &drop);
        assert_eq!(dropped, 0);
        assert_eq!(nodes.len(), 4);
    }
}
